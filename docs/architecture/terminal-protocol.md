# Terminal protocol plane

Core owns a types-only terminal protocol plane. The plane is independent of
the Hub host-control protocol.

This ticket is scaffold-for-consumers. Later runtime tickets emit these frames.
The production entry points in this repository are the two crate public APIs,
the committed generated package, and CI.

## Two-crate opacity rule

| Coordinate | Consumers | Public surface |
| --- | --- | --- |
| `botster-terminal-protocol` 0.1.0 | Hub adapters and any content-blind forwarder | Compatibility descriptors, forwardable requests, opaque `TerminalFrame` |
| `botster-terminal-protocol-client` 0.1.0 | TUI Rust and the TypeScript generator | Semantic Snapshot, phase, AttachState, TerminalOutput, and ProcessExit types |
| `@trybotster/terminal-protocol` 0.1.0 | Web and other Node consumers | Generated TypeScript, metadata, and terminal fixtures |

`botster-terminal-protocol-client` depends on `botster-terminal-protocol`.
Hub must depend only on `botster-terminal-protocol`. Hub must not depend on
`botster-terminal-protocol-client` and must not import generated TypeScript to
inspect frames.

`TerminalFrame` serializes and deserializes. It has no public `phase`, `state`,
`history`, `payload`, or Snapshot-body accessor.

Neither crate depends on `botster-core`, `botster-core-daemon`, `botster-hub`,
or `botster-hub-client`.

## Pinned vocabulary

| Item | Value |
| --- | --- |
| Protocol name | `botster-terminal-v1` |
| Protocol version | `1` |
| Conformance fixture revision | `1` |
| Default required features | `terminal_streaming`, `resize` |
| Advertised optional feature | `snapshot_delivery=ready_then_history` |
| Request tags | `attach`, `detach`, `send_input`, `resize` |
| Event tags | `snapshot`, `terminal_output`, `process_exit`, `attach_state` |
| Snapshot phases | `ready`, `history`, `finish` |
| AttachState values | `attaching`, `attached`, `snapshot_history_incomplete`, `attach_failed` |

`snapshot_delivery=ready_then_history` is a compatibility feature. It is not an
Attach field. The default client requirement does not include it.
`TerminalCompatibilityRequirement::for_ready_then_history_attach()` adds it.

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
at build time.
