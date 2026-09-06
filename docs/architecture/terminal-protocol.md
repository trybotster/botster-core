# Terminal protocol plane (scheme 2)

Core owns a types-only terminal protocol plane. The plane is independent of
the Hub host-control protocol. Every hot hop carries binary frames: the
worker parses PTY output once, Core shares one immutable output body across
every subscriber of a session, Hub forwards opaque bodies, and clients decode
fixed-layout headers without JSON or base64.

## Crates and packages

| Coordinate | Consumers | Public surface |
| --- | --- | --- |
| `botster-terminal-protocol` 0.4.0 | Hub adapters and any content-blind forwarder | Compatibility descriptors, `TerminalFrame`, `RoutedTerminalFrame`, `RouteId`, `TerminalInputFrame`, wire enums, limits, the stream body codec |
| `botster-terminal-protocol-client` 0.5.0 | TUI Rust, Core, and the TypeScript generator | Semantic `TerminalEvent`, `TerminalInputCommand`, input and paste encoders, `decode_input_body` |
| `@trybotster/terminal-protocol` 0.5.0 | Web and other Node consumers | Generated TypeScript: constants, enum tables, key table, `decodeTerminalBody`, `encodeTerminalBody`, input encoders |

`botster-terminal-protocol-client` depends on `botster-terminal-protocol`.
Hub depends only on `botster-terminal-protocol` and never decodes a body.
Neither crate depends on `botster-core`, `botster-core-daemon`,
`botster-hub`, or `botster-hub-client`.

The generated TypeScript is produced by the emitter in
`botster-terminal-protocol-client` and committed at
`crates/botster-terminal-protocol-client/generated/terminal-protocol.ts` with
a verbatim mirror under `packages/terminal-protocol/`. Regenerate with:

```sh
cargo run -p botster-terminal-protocol-client --example generate_typescript \
  > crates/botster-terminal-protocol-client/generated/terminal-protocol.ts
```

## Pinned vocabulary

| Item | Value |
| --- | --- |
| Protocol name | `botster-terminal-v2` |
| Protocol version | `2` |
| Conformance fixture revision | `4` |
| Stream scheme | `2` |
| Input scheme | `2` |

## Stream frame: `TerminalBody`

Every server-to-client frame is one `TerminalBody`:

```text
[u8 scheme = 2][u8 kind][u16 LE flags = 0][u32 LE body_len][body]
```

`flags` is reserved and always zero. The header is 8 bytes; the body is at
most 4 MiB minus 8 bytes.

| Kind | Value | Body |
| --- | --- | --- |
| `OUTPUT` | 1 | Raw PTY bytes |
| `SNAPSHOT_READY` | 2 | GHOSTSNP bytes through the READY record |
| `SNAPSHOT_HISTORY` | 3 | One GHOSTSNP history page; the finish record is the last page |
| `SNAPSHOT_FINISH` | 4 | Empty |
| `PROCESS_EXIT` | 5 | `[u8 has_code][i32 LE code]` |
| `MODES` | 6 | `[u32 LE mode_bits][u16 LE rows][u16 LE cols]` |
| `ATTACH_STATE` | 16 | `[u8 code]` (1 attaching, 2 attached, 3 detached, 4 failed) |
| `INPUT_RESULT` | 17 | See input results |
| `HISTORY_UNAVAILABLE` | 18 | `[u8 reason]` (1 evicted, 2 restart, 3 oversize, 4 capture_failed) |
| `ROUTE_RESYNC` | 19 | `[u32 LE from_epoch][u32 LE to_epoch]` |

`mode_bits`: `KITTY_KEYBOARD 1<<0`, `CURSOR_VISIBLE 1<<1`, `BRACKETED_PASTE
1<<2`, `MOUSE_NORMAL 1<<3`, `MOUSE_ANY 1<<4`, `MOUSE_BUTTON 1<<5`, `MOUSE_SGR
1<<6`, `ALT_SCREEN 1<<7`, `FOCUS_REPORTING 1<<8`, `APPLICATION_CURSOR 1<<9`.

## Route envelope: `RoutedTerminalFrame`

Core hands adapters `RoutedTerminalFrame { route, generation, stream_epoch,
frame }`.

- `route` is the subscription id as a validated `RouteId` (1..=1024 UTF-8
  bytes, no control characters).
- `generation` is the fixed attachment generation Core assigned on attach.
  It never changes for the life of the reservation. Hub validates it and
  copies it into its routing header verbatim. A new attachment is a new
  reservation with a new generation.
- `stream_epoch` is the route's snapshot/live continuity epoch inside that
  attachment. It starts at 0 and changes only through `ROUTE_RESYNC`. Core
  captures the epoch when it queues each frame and never re-stamps output or
  snapshot data.
- `frame` is the shared or personalized `TerminalBody`.

Hub carries the same routing header in both directions. Client-to-Hub input
carries `stream_epoch = 0`; Hub validates route and the fixed generation only
and never rejects input on the epoch field.

## Attach sequence

For one route on a live session, in order:

1. `ATTACH_STATE attaching` (optional, sent as soon as the route is recorded).
2. `ATTACH_STATE attached`.
3. `MODES` with the worker's current modes and size.
4. `SNAPSHOT_READY`. The terminal is renderable after this frame.
5. `OUTPUT` may interleave from here on.
6. `SNAPSHOT_HISTORY` pages. The GHOSTSNP finish record travels as the last
   page.
7. `SNAPSHOT_FINISH` (empty). The client releases history decoder state.

Live output produced before the worker capture boundary is inside the
snapshot and is never sent to the attaching route.

## Failure and unavailability

- Capture failure before `SNAPSHOT_READY` on a live route ends the route
  with `ATTACH_STATE failed` and teardown. No `OUTPUT` and no
  `HISTORY_UNAVAILABLE` follow on that route. The client may attach once more
  with a new subscription id; Core never replays input.
- Capture failure after `SNAPSHOT_READY` sends `HISTORY_UNAVAILABLE
  capture_failed` in place of the remaining pages, then `SNAPSHOT_FINISH`.
  The visible screen is valid and live output continues. `SNAPSHOT_FINISH`
  closes partial history assembly without the finish record; the client calls
  `abort_ghostsnp_history` and keeps the READY terminal.
- `evicted`, `restart`, and `oversize` appear only on ended-session readback
  through host control. They never appear on a live route.
- `HISTORY_UNAVAILABLE` never precedes `SNAPSHOT_READY` on a live route.

## Resync and the stream epoch

A route egress queue holds at most 64 frames or 4 MiB. On overflow Core
recovers only that route:

1. Unsent obsolete `OUTPUT`, `MODES`, and snapshot frames are dropped. A
   frame already handed to the adapter completes under its captured epoch.
2. The route enters `to_epoch = from_epoch + 1`.
3. `ROUTE_RESYNC { from_epoch, to_epoch }` is queued with envelope
   `stream_epoch = to_epoch`.
4. Accepted `INPUT_RESULT` frames that were unsent are preserved in order
   behind the transition and re-stamped with `to_epoch`.
5. Core requests a fresh worker capture. `MODES`, `SNAPSHOT_READY`, pages,
   and `SNAPSHOT_FINISH` follow under `to_epoch`.

Epoch exhaustion (`u32::MAX`) ends the route with `ATTACH_STATE failed`.
If a preserved result cannot be queued, Core closes the route and outstanding
operations resolve as unknown on the client.

Client rule:

- The accepted epoch is 0 after `ATTACH_STATE attached`.
- A `ROUTE_RESYNC` is accepted only when `from_epoch` equals the accepted
  epoch and the envelope `stream_epoch` equals `to_epoch`. Any other
  `ROUTE_RESYNC` is dropped as stale.
- Visual frames (`OUTPUT`, `MODES`, `SNAPSHOT_*`, `HISTORY_UNAVAILABLE`) whose
  envelope epoch differs from the accepted epoch are dropped. Frames already
  in flight may still carry the previous epoch.
- Epochs are never compared numerically and never assigned from a data frame.

## Input frames

Every client-to-Core input frame is one `TerminalInputFrame`:

```text
[u8 scheme = 2][u8 kind][u16 BE body_len][u64 BE operation_id][body]
```

| Kind | Value | Body |
| --- | --- | --- |
| `RAW_BYTES` | 1 | Raw PTY bytes; never mode-encoded |
| `KEY` | 2 | `[u8 action][u16 BE key][u16 BE mods][u16 BE consumed_mods][u8 composing][u32 BE unshifted_codepoint][text]` |
| `MOUSE` | 3 | `[u8 action][u8 button][u16 BE mods][u16 BE col][u16 BE row][u32 BE x_px][u32 BE y_px]` |
| `FOCUS` | 4 | `[u8 focused]` |
| `RESIZE` | 5 | `[u16 BE rows][u16 BE cols][u32 BE width_px][u32 BE height_px]` |
| `PASTE_BEGIN` | 6 | `[u32 BE total_len][u8 allow_unsafe]` |
| `PASTE_CHUNK` | 7 | `[u32 BE index][data]` |
| `PASTE_COMMIT` | 8 | Empty |
| `PASTE_ABORT` | 9 | Empty |

Operation ids are per attachment, start at 1, and increase strictly. Paste
continuation frames repeat the active paste id. Ids continue across resync.
The worker encodes key, mouse, focus, and paste bodies against its current
terminal modes; Core never keeps a second parser.

## Input results

`INPUT_RESULT` body:

```text
[u64 LE operation_id][u8 outcome]
[u8 has_accepted][u64 LE accepted_payload_bytes]
[u8 has_written][u64 LE written_pty_bytes]
[u32 LE mode_bits][u16 LE detail_len][detail UTF-8]
```

Outcomes: 1 written, 2 partial_write, 3 write_failed, 4 cancelled, 5
rejected_not_writable, 6 rejected_too_large, 7 rejected_unsafe_paste, 8
rejected_lane_full, 9 rejected_protocol, 10 session_ended, 11
outcome_unknown.

Rules:

- Exactly one result per admitted operation. Core never emits two.
- `INPUT_RESULT` is attachment-scoped and exempt from the visual epoch
  filter. A client correlates it by operation id within the current
  attachment regardless of epoch.
- A result for an unknown or already completed operation id is a protocol
  violation that the client ignores and reports.
- Route close resolves every outstanding operation as unknown on the client.
  Core emits `outcome_unknown` when the worker link fails after admission and
  `session_ended` when the session ends before the operation runs.
- Admission lanes: 32 operations or 2 MiB retained per session, 128
  operations or 8 MiB per client. A full lane yields `rejected_lane_full`,
  which is retryable.

## Generated artifact

`decodeTerminalBody` and `encodeTerminalBody` are inverse functions over
`TerminalEvent`. Fixtures and test doubles use `encodeTerminalBody` so no
consumer hand-writes the layout. The hex fixture
`crates/botster-terminal-protocol-client/fixtures/ready-then-history-event-order.json`
pins the attach sequence.
