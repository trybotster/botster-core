# Ghostty Shadow Terminal Adapter ADR

Ticket: Audit trybotster Ghostty fork API for Rust shadow-terminal adapter

Status: accepted for future adapter implementation

## Decision

Build any Ghostty integration as a separate optional adapter crate, such as
`botster-terminal-ghostty`, that depends on `botster-core` and implements the
existing `TerminalScreenRuntime` seam. Keep `botster-core` free of Ghostty,
Zig, C build tooling, generated bindings, and terminal-backend policy.

The lowest-risk first slice is to copy the proven shape of the current
trybotster CLI `libghostty-vt` integration into the adapter crate, prove parity,
then converge the CLI onto that shared adapter. The current CLI subsystem
should coexist as production evidence until the replacement is implemented and
verified.

The future adapter should:

- vendor or otherwise pin the trybotster Ghostty fork commit in the adapter
  crate, not in `botster-core`;
- build and statically link `libghostty-vt` from that pinned fork;
- use a small handwritten FFI/sys layer for the first slice;
- hide all `unsafe` and allocator/free ownership behind a safe Rust wrapper;
- implement `TerminalScreenRuntime` over Ghostty terminal calls;
- keep snapshot bytes opaque with a format label such as
  `ghostty-terminal-snapshot-v1`.

This ADR is about Ghostty and `libghostty-vt`. Restty is not the implementation
target for this audit.

## Evidence Inspected

Botster core evidence:

- `crates/botster-core/src/engine/terminal_screen.rs`
- `crates/botster-core/src/contract/terminal_screen.rs`
- `docs/plans/terminal-screen-snapshot-boundary.md`
- `README.md`

Current Ghostty fork evidence:

- fork commit `76853b34274208fe7c051cfe13eb1c7ee63c469b`
- `build.zig`
- `build.zig.zon`
- `src/build/GhosttyLibVt.zig`
- `include/ghostty/vt/terminal.h`
- `include/ghostty/vt/formatter.h`
- `include/ghostty/vt/render.h`
- `include/ghostty/vt/screen.h`
- `src/terminal/c/terminal.zig`
- `src/terminal/c/main.zig`
- `src/terminal/snapshot.zig`
- `example/c-vt-static/build.zig`
- `example/c-vt-cmake-static/CMakeLists.txt`
- `LICENSE`

Existing CLI integration evidence:

- trybotster commit `e434d7ead77e1a03bf3cc51ebe7165fb2be0b71e`
- vendored Ghostty submodule commit
  `a3fab497315d86cf490a462f4b295b917f700902`
- `cli/build.rs`
- `cli/build_support.rs`
- `cli/src/ghostty_vt.rs`
- `cli/src/session/protocol.rs`
- `cli/src/session/mod.rs`
- `cli/src/session/connection.rs`
- `cli/vendor/ghostty/FORK_NOTES.md`

## Current Botster Seam

`TerminalScreenRuntime` already names the adapter surface:

- `write_output(&mut self, bytes)` accepts PTY output.
- `resize(&mut self, size)` changes parser dimensions.
- `capture_snapshot(&mut self)` returns `TerminalSnapshotPayload`.
- `replay_snapshot(&mut self, payload)` imports an opaque snapshot.
- `screen_state(&self)` returns `TerminalScreenState`.

`TerminalSnapshotPayload` carries raw bytes, dimensions, and an optional
host-owned format label. Core does not parse those bytes. This is the right
shape for Ghostty snapshots because Ghostty owns terminal serialization
compatibility and import invariants.

The production-facing composition point remains a host `SessionWorkerRuntime`.
A host runtime can own a `TerminalScreenEngine<GhosttyRuntime>` internally, then
return the existing session-worker snapshot, screen, and mode carriers. This
ticket intentionally changes no runtime path; it records the concrete adapter
path for the next implementation slice.

## Existing CLI Integration Findings

The current trybotster CLI integration is the primary prior art. It already
solves the risky parts this adapter would otherwise rediscover.

`cli/build.rs` builds the vendored Ghostty checkout with:

```sh
zig build -Demit-lib-vt -Doptimize=ReleaseFast -Dsimd=false -Dcpu=baseline -Dversion-string=1.3.2-dev
```

It then links a static `libghostty-vt.a`. On macOS it repacks the Zig archive
with `ar` to avoid archive-member alignment failures and links the C++ runtime.
The build also pins rerun triggers for `vendor/ghostty/build.zig`,
`build.zig.zon`, `src`, and `include`.

The adapter build passes `-Dversion-string=1.3.2-dev` because the pinned fork
commit is reachable from a downstream tag that does not match Ghostty's
`vX.Y.Z` release-tag expectation. The explicit version string preserves the
fork's `build.zig.zon` version and avoids coupling adapter builds to local git
tag discovery.

`cli/build_support.rs` requires Zig `0.15.2` and resolves candidates from
`BOTSTER_ZIG`, `ZIG`, a mise-managed Zig install, `zig` from `PATH`, and
`mise exec -- zig`. The future adapter should preserve this explicit tool
selection model, while avoiding machine-specific paths in docs and errors.

`cli/src/ghostty_vt.rs` uses handwritten FFI and a safe wrapper, not broad
generated bindings. The wrapper covers terminal lifecycle, VT writes, resize,
mode/data reads, callbacks, formatter output, render-state reads, allocator
freeing, and terminal-level snapshot export/import.

The session process owns the Ghostty parser. `cli/src/session/mod.rs` feeds PTY
output into the parser, services snapshot/screen/mode RPCs, and routes callback
events through atomics or bounded channels. That ownership model should carry
forward: Botster should not introduce a second shadow parser in core or the hub.

## Current Ghostty Fork API Findings

The current fork exposes `libghostty-vt` as both shared and static build
artifacts. `build.zig` installs shared `ghostty-vt` and static
`ghostty-vt-static`, and dependency consumers can request
`dep.artifact("ghostty-vt-static")`. `build.zig.zon` identifies the fork as
`1.3.2-dev` and requires Zig `0.15.2`.

The C API in `include/ghostty/vt/terminal.h` exposes the first-slice calls the
adapter needs:

- `ghostty_terminal_new`
- `ghostty_terminal_free`
- `ghostty_terminal_resize`
- `ghostty_terminal_vt_write`
- `ghostty_terminal_get`
- `ghostty_terminal_mode_get`
- `ghostty_terminal_set`
- `ghostty_terminal_snapshot_export`
- `ghostty_terminal_snapshot_import`

The same header states that effects callbacks fire synchronously during
`ghostty_terminal_vt_write`, must not re-enter `ghostty_terminal_vt_write`, and
must avoid blocking work. This matches the existing CLI callback architecture.

`include/ghostty/vt/formatter.h` provides the low-risk first screen read:
plain text, VT, or HTML output from a formatter borrowed from the terminal.
Plain text is enough for the first `TerminalScreenState.plain_text` slice.

`include/ghostty/vt/render.h` exposes higher-fidelity render-state APIs for
dirty rows, colors, cells, cursor, and row iteration. Those should be deferred
until Botster needs cell-level fidelity beyond the current
`TerminalScreenRuntime` contract.

`src/terminal/snapshot.zig` defines terminal snapshot version `1` and serializes
dimensions, primary/alternate screens, scrolling region, modes, colors,
tabstops, previous character, status display, mouse/keyboard state, PWD, and
title. `src/terminal/c/terminal.zig` wraps this as
`ghostty_terminal_snapshot_export` and `ghostty_terminal_snapshot_import`.
Export returns `GHOSTTY_INVALID_VALUE` unless the stream is idle, which preserves
the parser-boundary rule for partial UTF-8 or partial escape sequences.

`cli/vendor/ghostty/FORK_NOTES.md` states the fork exists for lossless binary
terminal snapshot export/import, C callback parity, and post-import
canonicalization. Those are exactly the adapter requirements, so the adapter
should pin the fork until that surface is upstreamed or declared stable.

## Snapshot API Reconciliation

Two snapshot API generations exist in Botster history:

- Older page/state API documented by vault notes:
  `ghostty_snapshot_page_count`, `ghostty_snapshot_page_info`,
  `ghostty_snapshot_page_read`, `ghostty_snapshot_state_export`,
  `page_load`, `state_import`, and `state_finalize`.
- Current terminal-level API observed in the current fork and CLI wrapper:
  `ghostty_terminal_snapshot_export` and
  `ghostty_terminal_snapshot_import`.

The current fork does not expose the older `ghostty_snapshot_*` names through
`include/ghostty/vt/*.h` or `src/terminal/c/*.zig`; the observable C surface is
the terminal-level export/import pair. The older protocol notes are still
valuable because they explain why Botster moved to binary Ghostty-native
snapshots, page memory portability, and restore invariants. They should be
treated as historical design rationale and gotcha inventory, not the API to use
for the next Rust adapter.

Recommendation: implement the future adapter against
`ghostty_terminal_snapshot_export` and `ghostty_terminal_snapshot_import`. Treat
the returned buffer as the bytes for `TerminalSnapshotPayload.bytes`, set
`TerminalSnapshotPayload.size` from the current terminal dimensions, and set
`TerminalSnapshotPayload.format` to `ghostty-terminal-snapshot-v1`.

The adapter must pair allocations correctly. Buffers returned by
`ghostty_terminal_snapshot_export` or `ghostty_formatter_format_alloc` must be
released with `ghostty_free` using the same allocator choice. The safe wrapper
should make that impossible to forget.

## Minimal First Slice

Map the Ghostty calls to `TerminalScreenRuntime` directly:

- `write_output`: call `ghostty_terminal_vt_write` with raw PTY bytes and
  return the preserved `TerminalOutputChunk`.
- `resize`: call `ghostty_terminal_resize` with columns and rows from
  `TerminalScreenSize`.
- `capture_snapshot`: call `ghostty_terminal_snapshot_export`, copy the returned
  bytes into `TerminalSnapshotPayload`, free the Ghostty allocation, and label
  the payload `ghostty-terminal-snapshot-v1`.
- `replay_snapshot`: call `ghostty_terminal_snapshot_import` with
  `TerminalSnapshotPayload.bytes`.
- `screen_state`: use `ghostty_formatter_terminal_new` plus plain
  `ghostty_formatter_format_alloc` for `plain_text`; read mode/title/cwd/color
  fields only through the proven callback or `ghostty_terminal_get` patterns.

Defer render-state row/cell iteration until a later fidelity slice. The first
slice should prove construction, write, resize, opaque snapshot round trip, and
plain screen reads before replacing any production CLI behavior.

## Build And Dependency Strategy

Recommended crate shape:

- `botster-core`: no Ghostty dependency, no Zig dependency, no build script.
- `botster-terminal-ghostty`: optional adapter crate that owns the Ghostty fork
  pin, Zig build, static link, FFI, and safe wrapper.
- CLI integration: keep the existing subsystem until the adapter proves parity,
  then migrate the CLI to the adapter and remove duplication in one deliberate
  step.

Build constraints to preserve:

- initialize the vendored Ghostty checkout before running integration builds;
- require Zig `0.15.2` until the fork moves;
- include `-Demit-lib-vt`, `-Doptimize=ReleaseFast`, `-Dsimd=false`, and
  `-Dcpu=baseline`;
- pass `-Dversion-string=1.3.2-dev` for the pinned fork commit so Ghostty's
  release-tag validation does not depend on local git tag discovery;
- account for mise tool selection as well as `PATH`;
- keep static-vs-shared distribution policy inside the adapter crate;
- document platform-specific link handling such as macOS archive repacking if
  the adapter repeats the current CLI approach.

## Callback Strategy

The adapter should mirror the existing CLI callback discipline:

- callbacks fire synchronously during `ghostty_terminal_vt_write`;
- callbacks must not block, perform I/O, or re-enter the same terminal;
- simple notifications use atomics;
- data-carrying events use bounded non-blocking channels;
- title data must be treated carefully because callback payloads can split
  multi-byte UTF-8 across write boundaries;
- Kitty keyboard protocol changes should remain distinct from ordinary DEC mode
  flags.

Title, cwd, mode flags, and color information belong in `TerminalScreenState`
or host session state reads. They should not become pushed terminal-mode events
inside `botster-core`.

## License And Attribution

The Ghostty fork is MIT licensed and carries copyright attribution for Mitchell
Hashimoto and Ghostty contributors. The adapter crate can use and distribute the
fork under that license, but any vendored source, source distribution, binary
distribution, or generated package must preserve the copyright and MIT license
text required by `LICENSE`.

Because the adapter would pin and build a fork, the crate README should document
the fork commit, why the fork is needed, and what local patches are relied on.

## Risks

- The fork-local API is not yet an upstream-stable Rust dependency surface.
- `GhosttyTerminalOptions` has a TODO about ABI compatibility padding; pin the
  fork and keep FFI structs minimal.
- Snapshot export can fail at parser boundaries; callers must decide whether to
  retry, skip, or report a recoverable error.
- Import resets the VT stream parser state; host wrapper state around the
  terminal must be rebuilt or kept independent.
- Zig version drift or mise resolution can silently select the wrong compiler.
- Omitting `-Dcpu=baseline` can create release binaries that crash on older
  x86_64 hardware.
- Synchronous callbacks can stall terminal IO if they block.
- Stale older docs may still mention `ghostty_snapshot_*`; adapter code should
  follow the current terminal-level C API unless the fork reintroduces a public
  page/state surface.
- Static link distribution and platform linker behavior remain adapter-crate
  responsibilities.

## Assumptions

- The future adapter is allowed to pin the trybotster Ghostty fork until the
  required snapshot and callback APIs are upstream-stable.
- The first adapter does not need a broad generated C binding surface.
- `TerminalSnapshotPayload.format` is sufficient for a host-owned snapshot
  format label; core does not need a Ghostty enum.
- Plain formatter output is acceptable for the first `screen_state` slice.
- Cell-level render fidelity can wait until Botster has a concrete consumer that
  needs it.

## Verification

This ticket is intentionally documentation-only. Production runtime behavior is
unchanged. The production path that a future implementation will change is a
host `SessionWorkerRuntime` that owns a `TerminalScreenRuntime` implementation
and returns the existing Botster session-worker carriers.

Verification performed for this ADR:

- inspected the current `botster-core` terminal screen contract and engine;
- inspected the current Ghostty fork C headers, Zig build files, snapshot
  implementation, and license;
- inspected the existing trybotster CLI build, FFI wrapper, session protocol,
  parser ownership, and callback wiring;
- reconciled older Botster page/state snapshot notes against the current
  terminal-level C API;
- kept the recommendation to documentation and future adapter shape only.
