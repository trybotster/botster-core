# Ghostty as the only terminal runtime authority

Botster keeps exactly one terminal parser per session on the server: the
Ghostty instance inside `botster-session-worker`. The parent daemon holds no
terminal shadow for worker-backed sessions. Clients keep their own Ghostty
projection fed by the scheme 2 stream. This document states the authority
rules and where each piece of terminal state lives.

## Server side

- The worker Ghostty parses every PTY output byte, always, including when no
  client is attached. Modes, size, colors, scrollback, and query replies are
  therefore current at any attach, resync, or host readback.
- Ghostty query replies produced while parsing (OSC color probes, device
  attributes) are written back into the PTY by the worker itself, so the
  child receives them before any client exists.
- The worker encodes client input against its own modes: key events through
  the Ghostty key encoder (Kitty keyboard, application cursor, legacy),
  mouse events through the tracking mode in effect, focus reports only when
  focus reporting is on, pastes with bracketed-paste markers when mode 2004
  is on. Core never re-encodes and never guesses modes.
- Mode changes are pushed as `FRAME_MODES_CHANGED` and become one shared
  `MODES` frame per session on the client stream. There is no freshness
  token and no mode-gated input: the worker is the correctness boundary
  because it encodes and writes in one place.
- Snapshots are record-aware GHOSTSNP exports taken inside a PTY barrier so a
  route sees snapshot then live output from one capture point.
- On exit the worker sends the final screen text, mode bits, size, colors,
  and GHOSTSNP in `FRAME_FINAL_STATE`; the daemon retains that object under
  its retention policy for ended-session readback.

## Parent side

- `WorkerBackedBotsterEngine` installs `NullTerminalScreenRuntime`; it never
  replays output or snapshots into a second parser.
- Readbacks are worker probes (`FRAME_GET_SCREEN`, `FRAME_GET_MODE_FLAGS`)
  or worker captures, surfaced through the daemon's pending operations.
- `DefaultBotsterEngine` (local in-process PTY) keeps a local
  `TerminalScreenRuntime` because it has no worker. With the Ghostty
  backend, that runtime owns the same encoders and streaming snapshot
  export; with the plain backend, bound routes cannot be served a snapshot
  and attach fails explicitly.

## Client side

- The client projection (`GhosttyClientProjection`) is read-only render
  state fed by `SNAPSHOT_READY`, `SNAPSHOT_HISTORY`, `SNAPSHOT_FINISH`, and
  `OUTPUT`. It installs pages from borrowed slices; no page is copied out of
  the shared frame body.
- `MODES` frames update the client's input encoder choices (Kitty keyboard,
  bracketed paste, mouse tracking) for what the client sends; the worker
  still encodes on its own modes.
- A capture failure before `SNAPSHOT_READY` ends the route with
  `ATTACH_STATE failed`; a client must never paint live output into an
  uninitialized projection. After `SNAPSHOT_READY`, `HISTORY_UNAVAILABLE
  capture_failed` marks history incomplete, `SNAPSHOT_FINISH` still follows,
  and `abort_ghostsnp_history` keeps the READY terminal renderable.

## Pinned Ghostty

`crates/botster-terminal-ghostty/vendor/ghostty` is pinned at `eb72ec6`.
The Rust bindings in `botster-terminal-ghostty` wrap the terminal, the key,
mouse, focus, and paste encoders, the record-aware snapshot encoder and
decoder, and the color profile API. No other crate links Ghostty.

## Where terminal state lives

| State | Owner | Consumer path |
| --- | --- | --- |
| Live screen, scrollback, modes, colors | Worker Ghostty | Client stream, host probes, host captures |
| Final state of an ended session | Daemon retention (`RetainedTerminal`) | Host readback with explicit `history_unavailable` reasons |
| Client render state | Client projection | Local rendering only |
| Route egress queue and epoch | Core `ClientWorker` | Adapter writes |

No component keeps a second copy of live terminal state.
