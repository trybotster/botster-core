# Review: Hub hard-close implementation bbecda6

Date: 2026-09-04. Reviewer: Fable. Source and existing logs only, no builds. Candidate `bbecda6b4b505b25e43e05baa013b8daaec923d6` on `project-pipelines/ticket_1787600679_990088`, parent `11facec`, worktree clean. Evidence: `/tmp/hub-hard-close/{final-focused,ablations,natural-exit,partial-conflict}`. Dependency source read at the locked versions: `rtc 0.21.0-beta.2`, `rtc-datachannel 0.21.0-beta.2`, `rtc-sctp 0.21.0-beta.2`, `webrtc 0.21.0-beta.2`.

## Verdict

**Not approved for delivery while the combined ProcessExit test fails.** The hard-close rule is implemented correctly in both transports and the six negative controls are real. One new source defect must be fixed regardless of the ProcessExit outcome (Finding 1). The ProcessExit loss is not attributed by source; Finding 2 gives the bounded check that will attribute it.

## Verified correct

- Late egress replay is gone from both adapters. Unix `snapshot_writes` and `has_occupied_adapter_slot` read only the live slot. No test depends on the removed replay; its former tests were rewritten.
- Unix partial-envelope rule matches the ruling. `resume_pending_mux_write` abandons a copied frame at `offset == 0` when the handle is closed, both before the first write and inside `write_frame_bytes_resumable` through `poll_fn`, so no first byte starts after close. A started envelope finishes once from the writer's own buffer; `complete_active` is skipped when closed; the early `complete_active` on partial progress is removed, and the new assertion proves the slot stays `Full` with `try_write` returning `Full` during a partial write. `partial_envelope_finishes_once_after_close_before_host_and_sibling_frames` proves the order: closing envelope, then response and closed event, then sibling, and no replay on a later flush.
- WebRTC close is non-blocking with respect to the permit lock (`try_lock`, contended holders release after unlocking), proven by `close_does_not_wait_for_the_aggregate_permit_lock`. Budget transfer publishes the wire bound into usage before releasing authorization, and `ConnectionAggregate::buffered` reads authorization first; the snapshot test forces a transfer between the reads. A cancelled send keeps accepted bytes counted until the usage refresh.
- Six negative controls: each log shows exactly the intended test failing with 562 filtered (`cancel`, `failed_close`, `partial_full`, `snapshot_order`, `transfer`, `unsent_copy`). Restoration patch present.
- Final focused results match the report: fmt, clippy, budget, Unix and WebRTC units, Unix printf ProcessExit, WebRTC exact bytes pass; the combined WebRTC test fails; the two later checks did not run.

## Finding 1 (source defect, must fix): an already-closed channel is treated as a peer failure

`close_subscription_channel_or_fail_peer` escalates any result other than `Ok(Ok(()))` to `publish_peer_terminal(ChannelError)` plus `cleanup_once`, which tears down every channel on the peer. It is now called on every driver exit, including the `OnClose | OnError | None` arm that previously only returned (`subscription_channel.rs:548-552`, versus `11facec:540-544`).

In the locked dependency, a remote or already-completed close removes the channel from the peer map when `SCTPStreamClosed` is handled (`rtc/peer_connection/handler/datachannel.rs:396`). The async `close()` then fails with `ErrDataChannelClosed` because `data_channel(self.id)` is `None` (`webrtc/data_channel/mod.rs:595-601`). So a single subscription channel closed by the client, or observed as closed after its stream was reset, now becomes whole-peer cleanup. The unit tests only exercise a fake channel whose close fails or hangs; none exercises close on an absent or already-closed channel.

Correction: treat "channel already closed or absent" as a successful close. Either skip `local_close` on the `OnClose | None` arm, or map `ErrDataChannelClosed` to success inside `close_subscription_channel_or_fail_peer`, and keep escalation for a timeout or a real error. Add one fake-channel test where `local_close` returns the closed error and assert no `LocalWebrtcPeerClosed` is queued and the sibling stays live.

This is not established as the cause of the failing test, which fails before any detach or peer death. It is a delivery blocker on its own because the combined test's later phases and the detach path can trigger it.

## Finding 2: the ProcessExit loss, what the source shows, and the bounded check

Failure shape from `final-focused/webrtc_exit.log`: after the release input, `sub-exit` delivered `input_result` and the 19-byte output `wnx-release\r\ndone\r\n`, then the client saw that channel end (`channel_closed`, no partial chunk), and the 8-second wait timed out. No other channel errored. No detach, peer death, or shutdown had run yet. So the exit route's channel was closed by the Hub driver, and the client never received `ProcessExit`.

Send completion versus close, in the locked source:

- `send_text` returning `Ok` means the payload was enqueued into the sans-io pipeline and `outstanding_bytes` was incremented (`rtc/data_channel/mod.rs:299-309`). It is not transmitted. `outstanding_bytes` decrements only on `SCTPBufferReleased`, which is acknowledgement or abandonment (`handler/datachannel.rs:419-424`). Core's "delivered" (`complete_active` after `send_text`) therefore means accepted by the transport pipeline, not received.
- In this version the close marker does not overtake data inside the rtc core. Data and the close marker enter the same `write_outs` FIFO of the datachannel object (`rtc-datachannel/data_channel/mod.rs:445, 251-289`), flow through the handler's single `write_outs` FIFO (`handler/datachannel.rs:267-343`), and at SCTP the reset is an end-of-stream chunk pushed behind the data in `pending_queue` (`rtc-sctp/association/mod.rs:2839-2857`). `stream.close()` only marks the stream unwritable for later writes (`association/stream.rs:255-293`). I could not find a lower-priority queue for close in `0.21.0-beta.2`. The Hub writer's inference cites the same handler lines; the reordering it describes is not visible to me in this version, so I do not treat it as attributed.
- The new Hub cancellation (`flush_subscription_adapter_frames`, `select! biased` on `wait_for_write` versus the pinned send) aborts a send only when `handle.is_closed()`. Core closes the exit route only after it observes the adapter `Ready`, and the WebRTC slot stays occupied until `complete_active` after `sent` and `published` succeed. A cancellation of the ProcessExit send therefore needs a close from another cause. I found no host-side close on session exit in the WebRTC path; `close_from_host` is reached only through peer removal, `close_all`, and detach.

What remains unexplained is exactly one fact: whether the ProcessExit frame reached `send_text` at all before the channel close. Bounded check, one rerun of the single test with `--nocapture`, test-only instrumentation:

1. In `run_bound_terminal_channel`, at the flush-error branch and at the cancellation branch, print for the closing route: `host_closed()`, whether `sent` completed, the frame byte length, and `local_outstanding_bytes()` immediately before `local_close`.
2. In the fixture, record the `(session_id, subscription_id)` for each label at subscribe time and print it with the receive error, plus the mailbox overflow counter for the removed channel (bounds are 128 frames and 512 KB, so overflow is unlikely, but the counter settles it).
3. Read the two prints together. `sent` completed with `outstanding_bytes > 0` at close means the close overtook transmission and the proper fix is to wait for `outstanding_bytes` to reach zero, bounded by `LOCAL_WEBRTC_PEER_CLOSE_BOUND`, before `local_close` on a route that Core closed after delivery. `sent` cancelled with `host_closed` true means a host close raced the exit and the caller must be found. `sent` never entered means Core hard-stopped before writing the exit frame, which is a Core-side question.

Do not add a sleep, a replay buffer, or an early delivery claim. The drain-before-close option in step 3 is transport ordering with an existing bound, not a delay.

## Notes, no action required

- `WebRtcTerminalAdapterInner::complete_active` calls `slot.notify_writable()` after `AdapterSlot::complete_active` already emitted a writable wake, so each completion emits two writable wakes. Harmless, wasteful.
- `transfer_aggregate_permit` returns `false` when the permit is `None` while an aggregate exists, which closes the channel. Every accepted frame carries a permit today, so this is unreachable; an assertion or comment would make that explicit.
- The fixture change to record labels and errors is correct and is what made this failure legible.
