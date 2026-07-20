# Libghostty Mouse-Tracking Mode Feasibility

Ticket: `ticket_1784564069_340152`

## Finding

Classification **(a): feasible with no Ghostty-repository change**.

The pinned Ghostty revision
`76853b34274208fe7c051cfe13eb1c7ee63c469b`
(`botster-vt-2026.03.31.2`) already exposes authoritative synchronous reads:

- `ghostty_terminal_mode_get` accepts a packed mode and returns its current
  value (`vendor/ghostty/include/ghostty/vt/terminal.h:1033-1049`).
- Its implementation resolves the packed mode and reads the terminal's durable
  `t.modes` state (`vendor/ghostty/src/terminal/c/terminal.zig:568-577`).
- The public constants include DEC modes 1000, 1002, 1003, and 1006
  (`vendor/ghostty/include/ghostty/vt/modes.h:74-79`).
- The symbol is exported from libghostty-vt
  (`vendor/ghostty/src/terminal/c/main.zig:132-143`).
- Ghostty also exposes an aggregate tracking bool
  (`vendor/ghostty/include/ghostty/vt/terminal.h:768-776`), but that bool omits
  the exact tracking mode and the independent SGR encoding bit.

No Ghostty fork or parser change is required. The Botster gap was only its
narrow handwritten FFI surface. The adapter now declares the existing packed
mode ABI, constants, and query function
(`crates/botster-terminal-ghostty/src/sys.rs:14-35,121-127`).

## Runtime Proof

The feature-gated unit proof sends the same PTY-shaped DECSET/DECRST bytes used
in production through `TerminalScreenRuntime::write_output`, which delegates to
`ghostty_terminal_vt_write`. It then synchronously reads the resulting state
from the same `GhosttyTerminal` handle through
`ghostty_terminal_mode_get`
(`crates/botster-terminal-ghostty/src/lib.rs:358-393,553-575`).

The native test passed with the initialized pinned submodule and mise Zig
0.15.2:

```text
cargo test -p botster-terminal-ghostty --features libghostty-vt mouse_mode
running 1 test
test native::tests::mouse_mode_tracks_decset_and_decrst_on_the_native_terminal ... ok
test result: ok. 1 passed; 0 failed
```

The proof covers default/reset `0`, the four individual modes, combined
1000+1006, and full reset:

| Ghostty mode | Existing Botster bit | Meaning |
| --- | ---: | --- |
| 1000 | 1 | normal tracking |
| 1003 | 2 | any-event tracking |
| 1002 | 4 | button-event tracking |
| 1006 | 8 | SGR encoding |

This intentionally preserves the production encoding at
`trybotster@70c002f397007c8b0f3ebfe6b33a503dc7a283f6`
(`cli/src/ghostty_vt.rs:992-1007` and
`cli/src/session/protocol.rs:153-163`). Bit 8 alone selects an encoding; it
does not enable tracking.

The private test helper is fallible. Unlike trybotster's existing primitive at
`cli/src/ghostty_vt.rs:939-944`, it returns an error when
`ghostty_terminal_mode_get` fails instead of turning failure into an
authoritative-looking false value. The helper is `#[cfg(test)]` and is neither
public nor `pub(crate)`; this spike does not ship a safe mode-reporting API.

## Minimal Downstream Shape

The existing backend-neutral seam has no mode probe
(`crates/botster-core/src/engine/terminal_screen.rs:8-29`), while the existing
contract already carries `ModeFlags.mouse_mode: u8`
(`crates/botster-core/src/contract/session_protocol.rs:112-128`).
`TerminalScreenState::new` currently installs `ModeFlags::default`
(`crates/botster-core/src/contract/terminal_screen.rs:140-150`), and the
managed runtime still rejects the existing request
(`crates/botster-core/src/engine/managed_session_runtime.rs:916`).

The next core ticket should add one required, backend-neutral
`TerminalScreenRuntime::mode_flags() -> ModeFlags` method, including its boxed
forwarder. The plain backend and fakes should implement it explicitly. The
Ghostty backend should query the authoritative state and populate the existing
`ModeFlags`; it should not add another enum, bool, wire field, or versioned
flags type. `ManagedSessionRuntime` should use that method to answer the
existing `GetModeFlags`/`ModeFlagsReady` probe.

Assumptions:

- The synchronous query is made after `ghostty_terminal_vt_write` returns, not
  reentrantly from a Ghostty callback.
- The existing trybotster `u8` mapping remains the product contract.
- X10 tracking (DEC mode 9) is deliberately outside that `u8`. Ghostty's
  aggregate `GHOSTTY_TERMINAL_DATA_MOUSE_TRACKING` value includes X10, so it is
  not an equivalent substitute for the exact Botster bitmask.
- A future Ghostty revision change requires rechecking this ABI evidence.
- Downstream work must decide explicitly whether it populates all current
  `ModeFlags` fields together or lands honest mouse reporting first.

## Ordered Next-Ticket DAG

1. **Core + terminal adapter:** add the required runtime probe, populate it
   from Ghostty, explicitly implement plain/fake behavior, and serve the
   existing session request/response.
2. **Hub + hub-client:** expose a correlated probe over that request/response.
   Do not add a pushed `ModeChanged` event.
3. **Kit:** hydrate a client-owned `u8` attachment shadow and replace the
   current bool derived from a schema-invalid `terminal_view.mouse_mode` prop.
   The closed `terminal_view` contract stays unchanged.
4. **TUI:** use the hydrated shadow to decide chrome versus focused-child mouse
   ownership. Require SGR bit 8 plus at least one tracking bit, while preserving
   the kit's complete forwarded mouse-event stream.

Each ticket depends on the previous one. No stage should add a TUI-side
DECSET/DECRST parser, a pushed terminal-mode event stream, or a
`terminal_view` prop.

This boundary follows
[[ghostty shadow terminal integration belongs outside botster core]],
[[terminal view prop contract is closed in botster core]], and
[[synced state types are allowed while pushed event variants are forbidden]].
