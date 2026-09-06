# @trybotster/terminal-protocol

Core-owned terminal protocol scheme 2 for Web and other TypeScript consumers.

`terminal-protocol.ts` is generated from `botster-terminal-protocol-client`.
It is the only codec source in this package. `index.js` carries package
metadata only. Pin this package, not a Hub commit, for terminal frame codecs.

Protocol name and version are `botster-terminal-v2` / `2`. Conformance fixture
revision is `4`. Required feature tokens are `terminal_streaming`, `resize`,
and `transport=duplex_binary`; `snapshot_delivery=ready_then_history` is always
advertised.

## Stream frames

`decodeTerminalBody(bytes)` decodes one complete binary `TerminalBody`
(`[u8 scheme=2][u8 kind][u16 LE flags][u32 LE body_len][body]`) into a typed
`TerminalEvent`. Payload kinds (`output`, `snapshot_ready`, `snapshot_history`)
return a `Uint8Array` view of the input; there is no base64 and no copy. Route
id, the fixed attachment generation, and the stream epoch travel outside the
body in the host routing header. `encodeTerminalBody(event)` is the inverse;
fixtures and test doubles use it so no consumer hand-writes the layout.

Per-route order: `attach_state` attached, `modes`, `snapshot_ready`, live
`output` interleaved with `snapshot_history` (the GHOSTSNP finish record is the
last page), `snapshot_finish`, then `output`, and `process_exit` last.
`history_unavailable capture_failed` may replace the remaining pages after
`snapshot_ready`; `snapshot_finish` still follows and live output continues.
A capture failure before `snapshot_ready` ends the route with `attach_state`
failed.

`route_resync { from_epoch, to_epoch }` means the route overflowed. The
accepted epoch is 0 after `attach_state` attached. Accept a resync only when
`from_epoch` equals the accepted epoch and the routing header epoch equals
`to_epoch`; then reset the decoder and expect `modes` and a fresh
`snapshot_ready` under `to_epoch`. Drop visual frames whose header epoch
differs from the accepted epoch. `input_result` is attachment-scoped and
exempt from that filter: correlate it by `operation_id`.

## Input frames

`encodeRawBytes`, `encodeKey`, `encodeMouse`, `encodeFocus`, `encodeResize`,
`encodePaste`, and `encodePasteAbort` produce 12-byte-header scheme 2 input
frames. Every frame carries a client-chosen `operation_id` (`bigint | number`)
that strictly increases per attach, starting at 1. Paste continuation frames
repeat the id of the active paste. `encodePaste(operationId, allowUnsafe,
data)` returns `PASTE_BEGIN`, ordered `PASTE_CHUNK` frames, and
`PASTE_COMMIT`. Paste content is 1 through 1,048,576 bytes.

Keys use `TerminalKey`, keyed by W3C `KeyboardEvent.code`;
`terminalKeyFromCode` maps unknown codes to `Unidentified`. Modifier bits are
`TerminalMods`. Mouse buttons are `TerminalMouseButton`; wheel steps are
`press` of `wheel_up` through `wheel_right`. Every `input_result` carries an
`InputOutcome` name; do not retry `partial_write`, `write_failed`,
`cancelled`, or `outcome_unknown`.

Hub adapters must not import this package. Hub depends only on the Rust crate
`botster-terminal-protocol`.
