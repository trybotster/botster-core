# Unix hard close versus a partially written envelope

Date: 2026-09-04. Author: Fable. Bounded contract review for root. No builds. Sources: Core `93acae3` `crates/botster-core/src/contract/terminal_adapter.rs:25-62`; Hub `11facec` `src/transport/unix/mux_write.rs`, `src/transport/unix/adapter.rs`, `src/transport/shared/adapter_slot.rs`.

## What the source says

- Contract: `Ok(())` from `try_write` means the frame occupies the one slot until the transport finishes that write. Close abandons any in-flight frame, must return without waiting on transport I/O or the writer lock, and the abandoned frame must not be delivered later (`terminal_adapter.rs:25-40, 49-56`).
- Hub slot: `AdapterSlot::close` clears the slot with `try_lock` and never waits (`adapter_slot.rs:71-92`). The writer does not write from the slot. `snapshot_writes` copies the slot bytes into `PendingMuxFrame.bytes`, a writer-owned buffer, and the socket write proceeds from that copy with an `offset` (`adapter.rs:552-580`, `mux_write.rs:223-244, 336-362`).
- Replay: `park_late_egress` copies the slot into `late_egress` on close, and `snapshot_writes` later hands that copy back to the writer as a new write (`adapter.rs:77-89, 571-580`). That is a new transport write of an abandoned frame after close. It violates the contract. Root's removal is correct, and its unit tests at `mux_write.rs:997-1098` go with it. No isolation test depends on it.
- Early completion: the `Pending` branch calls `complete_active` when `offset > 0` (`mux_write.rs:320-329`). The adapter then reports `Ready` while the envelope is still on the wire. That contradicts "occupies the slot until the transport finishes that write." Removal is correct.

## The physical boundary

Once the writer has passed the first byte of an envelope to the kernel, the connection carries a partial JSON line. There is no way to withdraw it. Only two futures exist for that line: the remaining bytes follow it, or the connection ends. Dropping the remaining bytes and continuing produces a corrupt frame that the client parser rejects as a protocol error, which ends the connection anyway (`read_async_frame` fails closed). So "drop remaining bytes but keep the socket" is not a third option. It is connection death with extra steps.

Therefore the honest distinction is by wire state, not by adapter state:

| Frame state at close | Rule |
| --- | --- |
| In the adapter slot, not copied by the writer, or copied with `offset == 0` | Abandoned. Never written. |
| Copied by the writer and `offset > 0` (first byte already in the kernel) | The writer completes the envelope from its own buffer. This is completion of a write the transport already started, not a later delivery of a new frame. |
| Parked copy after close (`late_egress`) | Forbidden. Delete the mechanism. |

Close itself does nothing new. It clears the slot and returns, as today. The writer never touches the slot after close: `complete_active` is already skipped when `is_closed()` (`mux_write.rs:306-308`). The rule lives entirely in the writer's handling of its own buffer.

## Is the narrow rule honest and sufficient

Honest: yes. The contract already says close must not wait on the writer, which means the transport's in-progress write is outside close's reach by design. Completing the started envelope is what the contract's own non-blocking requirement implies. The frame reaches the client before the route's closed event, because `resume_pending_mux_write` runs before host frames on every flush (`mux_write.rs:166-170`). From the client's view the frame arrived, then the route closed. That ordering is truthful. No writable wake follows, because `emit_writable` is suppressed on a closed slot (`adapter_slot.rs:94-103`).

Sufficient for route isolation with usable control: yes, with one addition. Control frames wait behind the remaining bytes of at most one envelope, bounded by `DAEMON_MAX_FRAME_BYTES`, and only while the peer reads. A peer that stops reading stalls the shared stream regardless of any route rule; that is the topology, and it is already handled by `DAEMON_CLIENT_WRITE_TIMEOUT` in `flush_pending_responses`, which ends the connection. That is the correct outcome for a dead peer and is not a route-isolation failure.

Required addition: a first-byte gate. Before the writer issues the first socket write of a terminal envelope (`offset == 0`), it checks `handle.is_closed()`. If closed, it abandons the copy without writing. Without this gate, a copy taken before close could start writing after close, which would be a new delivery of an abandoned frame. `abandon_zero_offset_terminal_for_response` already has the same shape for a different trigger (`mux_write.rs:275-284`).

## Answers to root's two questions

1. The strict requirements are not mutually incompatible, and no wire or topology change is needed. Hard close stays non-blocking and abandons the slot. The transport finishes only an envelope it already started, never starts one after close, and never replays. Control stays usable on a reading peer. A non-reading peer is a connection failure by the existing deadline, not a route case.
2. The narrowly explicit already-started-envelope completion rule is physically honest and sufficient, provided the first-byte gate is included and `late_egress` and the early `complete_active` are removed.

## Scope

Hub only: `mux_write.rs` (remove early completion, add the first-byte gate) and `adapter.rs` (remove `late_egress` and its replay in `snapshot_writes`). Core behavior does not change. If root wants the contract text to say this in words, one sentence suffices in `terminal_adapter.rs` under close: "A transport may complete a frame whose first byte it wrote before close, to preserve stream framing; it must not start or repeat a write of an abandoned frame after close." That is a documentation clarification of the existing non-blocking rule, not a weakening, and it is root's decision.
