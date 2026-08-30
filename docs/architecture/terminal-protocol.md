# Terminal protocol plane

Core owns a types-only terminal protocol plane. The plane is independent of
the Hub host-control protocol.

The production entry points are the two crate public APIs, the committed
generated package, CI, and `CoreDaemon::drain` → `apply_terminal_input` for
duplex input. Hub-safe crates stay content-blind: they carry opaque
`TerminalInputFrame` bytes and do not decode payloads.

## Two-crate opacity rule

| Coordinate | Consumers | Public surface |
| --- | --- | --- |
| `botster-terminal-protocol` 0.1.0 | Hub adapters and any content-blind forwarder | Compatibility descriptors, forwardable requests, opaque `TerminalFrame` |
| `botster-terminal-protocol-client` 0.2.0 | TUI Rust and the TypeScript generator | Semantic Snapshot, phase, AttachState, TerminalOutput, ProcessExit, and `input_result` types, plus semantic input encode/decode |
| `@trybotster/terminal-protocol` 0.2.0 | Web and other Node consumers | Generated TypeScript, metadata, encode helpers, and the ready-then-history event-order fixture |

`botster-terminal-protocol-client` depends on `botster-terminal-protocol`.
Hub must depend only on `botster-terminal-protocol`. Hub must not depend on
`botster-terminal-protocol-client` and must not import generated TypeScript to
inspect frames.

`TerminalFrame` serializes and deserializes. It has no public `phase`, `state`,
`history`, `payload`, or Snapshot-body accessor.

The content-blind write/close/pressure adapter that consumes these opaque
frames lives in `botster-core`, not this crate. See
[`terminal-adapter.md`](terminal-adapter.md).

Neither crate depends on `botster-core`, `botster-core-daemon`, `botster-hub`,
or `botster-hub-client`.

## Pinned vocabulary

| Item | Value |
| --- | --- |
| Protocol name | `botster-terminal-v1` |
| Protocol version | `1` |
| Conformance fixture revision | `2` |
| Default required features | `terminal_streaming`, `resize` |
| Advertised features | `terminal_streaming`, `resize`, `transport=duplex_binary`, `snapshot_delivery=ready_then_history` |
| Request tags | `attach`, `detach`, `send_input`, `resize` |
| Event tags | `snapshot`, `terminal_output`, `process_exit`, `attach_state`, `input_result` |
| Snapshot phases | `ready`, `history`, `finish` |
| AttachState values | `attaching`, `attached`, `snapshot_history_incomplete`, `attach_failed` |

`snapshot_delivery=ready_then_history` is a compatibility feature. It is not an
Attach field. The default client requirement does not include it.
`TerminalCompatibilityRequirement::for_ready_then_history_attach()` adds it.

`transport=duplex_binary` is advertised. The default requirement stays lower
until the Hub WebRTC, Hub Unix, Web, and TUI cutovers land.
`TerminalCompatibilityRequirement::for_duplex_binary_transport()` is the
explicit requirement for consumers that have completed that cutover.

`TerminalCapabilitySet` is the Hub-safe opaque token set for a bound
subscription. Hub constructs it from advertised feature tokens, including an
empty intersection. Unknown tokens fail at construction. Core bind stores the
set and does not inspect host grants. Snapshot encode uses
`snapshot_delivery=ready_then_history` from this set. `resize` and
`terminal_streaming` remain protocol tokens for Hub negotiation.

Snapshot `phase` is required on this plane. Current Hub Snapshot JSON is not
this plane.

`process_exit.code` is omitted when `None`. Generated TypeScript marks `code?`.

Public enums are not `non_exhaustive`. Adding a variant at `0.1.0` is a
breaking change for exhaustive downstream matches.

## Consumer direction

- Hub pins `botster-terminal-protocol` and forwards opaque frames.
- TUI pins `botster-terminal-protocol-client`.
- Web pins `@trybotster/terminal-protocol`.
- None of those pins is a Hub Git revision.

Ghostty remains the snapshot-byte authority. These crates do not decode
GHOSTSNP records. Frozen late-attach goldens live in
`crates/botster-terminal-protocol/fixtures/ghostsnp/` and are not regenerated
at build time. Hub still contains copies until
`ticket_1786664495_777899` deletes those goldens and the Hub generator and
consumes the Core-owned files.

The npm package ships the ready-then-history event-order JSON fixture. It
does not ship GHOSTSNP bytes.
