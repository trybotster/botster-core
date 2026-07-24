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

The vendored source is the trybotster Ghostty fork at commit
`76853b34274208fe7c051cfe13eb1c7ee63c469b`. The fork is needed for
`libghostty-vt` snapshot and callback work documented in
`vendor/ghostty/FORK_NOTES.md`.

Feature-enabled builds require Zig `0.15.2`. The build script checks
`BOTSTER_ZIG`, `ZIG`, a mise-managed Zig `0.15.2` install, `zig` from `PATH`,
and `mise exec -- zig`. It runs:

```sh
zig build -Demit-lib-vt -Doptimize=ReleaseFast -Dsimd=false -Dcpu=baseline -Dversion-string=1.3.2-dev
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

If Zig `0.15.2` is unavailable, the build fails before linking and names the
Zig precondition instead of surfacing a raw linker or `build.zig` error.

Restty is not used here. Restty remains a client renderer path, not the
authoritative shadow-terminal parser or snapshot owner.

## License

The workspace crate metadata points at the repository license. The vendored
Ghostty fork is MIT licensed and carries copyright attribution for Mitchell
Hashimoto and Ghostty contributors. Any source or binary distribution that
includes the vendored fork must preserve `vendor/ghostty/LICENSE` and the
fork's copyright attribution.
