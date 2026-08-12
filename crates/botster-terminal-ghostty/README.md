# botster-terminal-ghostty

`botster-terminal-ghostty` owns Botster's concrete Ghostty shadow-terminal
adapter boundary. `botster-core` keeps the backend-neutral
`TerminalScreenRuntime` seam; this crate owns Ghostty-specific source pinning,
Zig build policy, static linking, FFI, and runtime wiring.

The default crate build does not require Ghostty or Zig. Native libghostty-vt
work is behind the `libghostty-vt` feature:

```sh
git submodule update --init crates/botster-terminal-ghostty/vendor/ghostty
cargo test -p botster-terminal-ghostty --features libghostty-vt
```

The vendored source is trybotster/ghostty at commit
`5e9ba17a22ba8e40bf8de7d3e7555b8378cb1880` from
`https://github.com/trybotster/ghostty`. That pin is the production terminal
authority for Botster Core: it publishes the snapshot API
(`include/ghostty/vt/snapshot.h`), mode/palette reads, and the `write_pty`
effect callback this crate installs so session-side OSC color and device
queries can be answered before any client attaches.

Feature-enabled builds require Zig `0.16.0`, which is upstream's
`minimum_zig_version` at this pin. The build script checks `BOTSTER_ZIG`,
`ZIG`, a mise-managed Zig `0.16.0` install, `zig` from `PATH`, and
`mise exec -- zig`. It runs:

```sh
zig build -Demit-lib-vt -Doptimize=ReleaseFast -Dsimd=false -Dcpu=baseline -Dversion-string=1.3.2-dev -Demit-xcframework=false
```

The build scopes Zig's local and default global caches to Cargo's `OUT_DIR` as
`zig-local-cache` and `zig-global-cache`. This keeps both caches coherent for
each Cargo target directory and prevents builds from writing `.zig-cache` into
the shared vendored Ghostty checkout. An explicit `ZIG_GLOBAL_CACHE_DIR`
continues to override the default global cache; local-cache isolation is always
enforced by the build.

The build links the resulting static `libghostty-vt` archive. On macOS it
repackages the Zig archive with `ar` before linking and also links the C++
runtime, matching the proven CLI path.

If the feature is enabled without initialized Ghostty source, the build fails
with:

```text
botster-terminal-ghostty libghostty-vt feature requires initialized Ghostty source at crates/botster-terminal-ghostty/vendor/ghostty; run `git submodule update --init crates/botster-terminal-ghostty/vendor/ghostty`
```

If Zig `0.16.0` is unavailable, the build fails before linking and names the
Zig precondition instead of surfacing a raw linker or `build.zig` error.

Ghostty is the only production terminal authority in this workspace. This
adapter owns GHOSTSNP snapshots, complete `ModeFlags` (Kitty, mouse, cursor,
bracketed paste, alt screen, focus reporting, application cursor), palette
state, and PTY query replies. `botster-core` stays backend-neutral; hosts
compose this crate on the production daemon path.

Restty is not used here. Restty remains a client renderer path, not the
authoritative shadow-terminal parser or snapshot owner.

## Client read-only projection (TUI pin surface)

Feature `libghostty-vt` also exposes **`GhosttyClientProjection`**: a reusable,
mechanism-only client adapter for first-party TUI (and similar) consumers that
install Hub `DaemonEvent::Snapshot` history bytes and render without decoding
terminal truth in app code.

### Bytes-only install (Hub 89dae7e shape)

Hub data-plane Snapshot carries opaque history (`payload_base64` →
`decoded_bytes()`). The client install API takes **those bytes only** — no
format label, no Hub rows/cols, no `TerminalSnapshotPayload` wrapper.

```rust
// feature = "libghostty-vt"
use botster_terminal_ghostty::{GhosttyClientProjection, ScrollOp, GHOSTSNP_MAGIC};

let mut client = GhosttyClientProjection::new(size)?;
// hub_bytes == Snapshot.history.decoded_bytes()  (must start with GHOSTSNP)
client.install_ghostsnp(&hub_bytes)?;
// dimensions come from decoded Ghostty after install
let dims = client.dimensions();
client.apply_terminal_output(&live_terminal_output);
let viewport = client.project_viewport()?;
let bar = client.scrollbar()?;
client.scroll(ScrollOp::Delta(-3));
let profile = client.color_profile()?;
let modes = client.mode_flags()?;
```

Fail closed on empty, non-`GHOSTSNP` magic, corrupt body, or decode failure
(previous handle stays usable). **Never** pass `DaemonEvent::Scrollback`
payloads to `install_ghostsnp`.

### Pinned projection fields

| Type | Fields |
| --- | --- |
| `ProjectedCell` | `grapheme`, `wide` (`Narrow` / `Wide` / `SpacerTail` / `SpacerHead`), resolved `fg`/`bg` `Rgb`, `bold`, `italic`, `underline`, `inverse`, `faint`, `strikethrough` |
| `ViewportProjection` | `cols`, `rows`, row-major `cells` (`len == cols * rows`), `cursor` |
| `CursorProjection` | `visible`, `in_viewport`, `x`, `y`, `style` (`Block` / `Bar` / `Underline` / `Hollow`) |
| `ScrollbarState` | `total`, `offset`, `len` (Ghostty truth at read time) |
| `ScrollOp` | `Top`, `Bottom`, `Delta(i32)` (negative = up into history) |

### Non-goals

- No PTY ownership.
- No `write_pty` / OSC query answering (session `GhosttyTerminal` keeps that path).
- No Hub DTO ownership; no TUI/kit product code in this crate.
- Live multi-process Hub attach remains the TUI ticket after it pins this crate.

### Session vs client

| Type | Role |
| --- | --- |
| `GhosttyTerminal` | Session/shadow path: snapshots, `write_pty` OSC answers, `TerminalScreenRuntime` |
| `GhosttyClientProjection` | Client pin: bytes install → live apply → project cells/scroll/palette/cursor |

## License

The workspace crate metadata points at the repository license. Vendored upstream
Ghostty is MIT licensed and carries copyright attribution for Mitchell
Hashimoto and Ghostty contributors. Any source or binary distribution that
includes the vendored source must preserve `vendor/ghostty/LICENSE` and that
copyright attribution.
