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

## License

The workspace crate metadata points at the repository license. Vendored upstream
Ghostty is MIT licensed and carries copyright attribution for Mitchell
Hashimoto and Ghostty contributors. Any source or binary distribution that
includes the vendored source must preserve `vendor/ghostty/LICENSE` and that
copyright attribution.
