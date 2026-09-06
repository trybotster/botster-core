# Durable session worker protocol

`botster-session-worker` owns one PTY and the only server-side terminal
parser for its session. The parent daemon talks to it over a length-prefixed
frame stream on stdio or a reconnectable Unix control socket. The worker
parses every PTY output byte with Ghostty whether or not a client is
attached, so late attaches, host readbacks, and resyncs always come from a
current terminal.

## Framing

Every frame is `[u32 LE len][u8 frame_type][payload]`. `len` counts the type
byte plus the payload. The handshake is hello (parent) then welcome (worker)
carrying `SessionMetadata`; adopted connections repeat it.

## Parent to worker

| Frame | Payload |
| --- | --- |
| `FRAME_SPAWN_SESSION` | `SessionSpawnRequest` JSON, once after hello |
| `FRAME_PTY_INPUT` | Raw bytes written to the PTY as given |
| `FRAME_INPUT_OPERATION` (0x1d) | `[u64 LE key][u8 kind][u64 LE operation_id][u32 LE body_len][body]` |
| `FRAME_INPUT_CANCEL` (0x1f) | `[u64 LE key]` |
| `FRAME_RESIZE` | `ResizePayload` JSON; staged when a snapshot barrier is open |
| `FRAME_GET_MODE_FLAGS` | `WorkerProbeRequest { request_id }` |
| `FRAME_GET_SCREEN` | `WorkerProbeRequest { request_id }` |
| `FRAME_GET_SNAPSHOT` | `WorkerSnapshotRequest { request_id, cancel, complete }` |
| `FRAME_PING`, `FRAME_SET_TIMEOUT`, `FRAME_SHUTDOWN` | Health, reconnect timeout, orderly shutdown |

`WorkerInputKind`: 1 raw bytes, 2 key, 3 mouse, 4 focus, 5 resize, 6 paste.
Kinds 2 through 5 carry the client input body verbatim; paste carries
`[u8 allow_unsafe][paste bytes]`. `key` is Core-unique and is echoed in the
result; `operation_id` is the client id echoed to the client.

## Worker to parent

| Frame | Payload |
| --- | --- |
| `FRAME_PTY_OUTPUT` | Raw PTY bytes, already applied to the worker Ghostty |
| `FRAME_INPUT_RESULT` (0x1e) | `[u64 LE key][INPUT_RESULT TerminalBody]` |
| `FRAME_MODES_CHANGED` (0x20) | `MODES TerminalBody`, sent when mode bits or size change |
| `FRAME_RESIZE_APPLIED` | `ResizePayload` JSON after the PTY and Ghostty resized |
| `FRAME_MODE_FLAGS` | `ModeFlagsPayload { request_id, mode_flags, rows, cols, error_kind }` |
| `FRAME_SCREEN` | `ScreenPayload { request_id, text, error_kind }` |
| `FRAME_SNAPSHOT` | `WorkerSnapshotResult` per record-aware frame; `color_profile` on FINISH |
| `FRAME_FINAL_STATE` (0x21) | `[u32 LE json_len][WorkerFinalState JSON][raw GHOSTSNP]` |
| `FRAME_PROCESS_EXITED` | `ProcessExitedPayload` JSON, always the last frame |
| Metadata lane | title, cwd, prompt mark, bell, notification, shaping reports |

## Input operations

For each `FRAME_INPUT_OPERATION` the worker replies exactly once with
`FRAME_INPUT_RESULT`, or the link fails and the parent resolves the
operation as `outcome_unknown`.

1. The worker applies every drained PTY byte first so modes are current.
2. It decodes the body, checks its own lane (32 operations or 2 MiB pending),
   and encodes: raw bytes verbatim; key, mouse, and focus through the
   Ghostty encoders under the current modes; paste through the safety check
   (`allow_unsafe` overrides) and bracketed-paste wrapping; resize applies
   Ghostty and PTY geometry, emits `FRAME_RESIZE_APPLIED`, and results as
   `written` with zero bytes.
3. Encoded bytes join one FIFO of pending PTY writes with keyless writes
   (`FRAME_PTY_INPUT` and Ghostty query replies). Writes are nonblocking; a
   blocked PTY is retried without holding the worker loop.
4. Completion reports `written` with the accepted payload bytes and PTY bytes
   written, `partial_write` or `write_failed` on a PTY error, `cancelled`
   when `FRAME_INPUT_CANCEL` caught the unwritten remainder, and
   `session_ended` for anything still pending when the child exits.

Rejections at the worker (`rejected_protocol`, `rejected_too_large`,
`rejected_unsafe_paste`, `rejected_lane_full`) carry zero progress and the
current mode bits.

## Snapshot boundary

`FRAME_GET_SNAPSHOT` opens a PTY I/O barrier: the worker drains and applies
every byte read so far, exports record-aware GHOSTSNP frames (READY, history
pages, FINISH) inside the barrier, and holds the PTY until the parent sends
`complete` (applying any staged resize first) or `cancel`. Output read after
the barrier stays queued and follows the snapshot, so a route sees snapshot
then live in one order from one capture point. The parent completes the
barrier without waiting; the worker confirms with `barrier_released`.

## Exit

On child exit the worker resolves pending operations as `session_ended`,
sends `FRAME_FINAL_STATE` with the final screen text, mode bits, size,
colors, and the final GHOSTSNP, then `FRAME_PROCESS_EXITED`. The parent keeps
the final state for ended-session readback under the daemon retention
policy.

## Parent side

`WorkerProcessRuntime` never blocks the engine on a worker reply except the
diagnostic ping. It exposes `submit_input_operation`,
`cancel_input_operation`, `take_input_results`, `take_mode_changes`,
`latest_modes`, `begin_mode_flags_probe` / `take_mode_flags_replies`,
`begin_screen_probe` / `take_screen_replies`, `take_final_state`, the
snapshot boundary begin/poll/cancel/complete calls, and asynchronous launch
through `begin_spawn` / `poll_spawn` on a helper thread. The control lane is
a bounded queue with a dedicated writer thread; a sealed or failed lane marks
the session control plane failed and only a respawn recovers it.
