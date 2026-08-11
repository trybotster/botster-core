# Implementation report: Ghostty-only terminal runtime authority

## Target repository and target_id
- Target repository: `botster-core` (`trybotster/botster-core`)
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Run: `run_1786471511_632427`
- Ticket: `ticket_1786471489_484901`
- Worktree: Botster-managed ticket worktree for this run
- PR: https://github.com/trybotster/botster-core/pull/119
- Commit: `6a44a61d12787ab251c29cb5d3d94f09d2d36b8e`

## Repository playbook and other playbooks/notes applied
- [[implementer-playbook]]
- [[botster-implementer-playbook]]
- [[botster-core-playbook]] (primary ownership charter)
- [[botster-terminal-ghostty-playbook]] (adapter surface in same workspace)
- Targeted notes: ghostty shadow terminal integration belongs outside botster core; session-process-owns-vt-parser-hub-rpc-snapshots; binary-page-snapshots-replace-vt-in-protocol; coredaemon must expose terminal truth used by the production hub path; pinned libghostty exposes synchronous exact mouse mode state; libghostty-vt-embedder-callback-architecture-and-constraints; synced state types are allowed while pushed event variants are forbidden; split terminal runtimes drop color probe responses before client attachment; botster core contract surface needs consumer proof; botster cli integration tests require ghostty submodule initialization; initial terminal snapshots must precede live output activation
- Not loaded: [[project-pipelines-playbook]] (out of scope); [[botster runtime teardown lenses]] (`teardown_class_applies=false`)

## Files changed
### botster-terminal-ghostty
- `crates/botster-terminal-ghostty/src/sys.rs` — modes, color data/opts, write_pty callback types
- `crates/botster-terminal-ghostty/src/lib.rs` — full ModeFlags, palette, write_pty drain, default theme, effects install
- `crates/botster-terminal-ghostty/README.md` — pin truth trybotster/ghostty@5e9ba17a

### botster-core
- `crates/botster-core/src/engine/terminal_screen.rs` — color profile + drain_pty_writes seams
- `crates/botster-core/src/engine/managed_session_runtime.rs` — PTY reply injection, color profile prepare path
- `crates/botster-core/src/engine/session_worker.rs` — fallible set_color_profile
- `crates/botster-core/src/runtime/local_process.rs` — trait signature
- Related unit tests

### botster-core-daemon
- `Cargo.toml` — non-optional Ghostty dependency; remove feature matrix
- `src/daemon.rs` — single Ghostty construction path
- `tests/daemon_integration_test.rs` — Ghostty-only expectations; Kitty/mouse input PTY proof; mode flags; OSC write_pty proof

### botster-core-test-support
- `Cargo.toml` — optional/default ghostty-terminal feature for conformance
- `src/conformance/mod.rs` — hub-shaped authority assertions
- `tests/ghostty_terminal_authority_conformance_test.rs` — required consumer-shaped proof
- Fake terminal/session worker updates

### CI / docs
- `.github/workflows/ci.yml` — drop daemon `--no-default-features` lanes
- `README.md`, `docs/architecture/core-daemon.md`, `docs/architecture/ghostty-shadow-terminal-adapter.md`
- Plan: `docs/architecture/ghostty-only-terminal-runtime-authority.md`

## Ownership boundaries preserved
- Concrete Ghostty FFI/Zig remains in `botster-terminal-ghostty`
- `botster-core` only gains neutral `TerminalScreenRuntime` seams (color profile, drain_pty_writes)
- No Hub/Web/TUI/Restty product policy moved into Core
- PlainTerminalScreenRuntime remains library/test harness only; not reachable from CoreDaemon construction

## Cross-repo dependencies or separately routed work
- Hub ticket `ticket_1786471489_718500` depends on this Core ticket via `dependency_1786471500_696870` (already registered; not recreated)
- Hub/Web/TUI/Restty cutover remains separately routed after Core merge

## Deviations from plan
- None material. Implementation followed the approved cold-turkey plan.
- Kitty/mouse input proof uses `dd|od` exact-byte harness (plan allowed implementer choice of PTY observation technique).
- Special color profile indices `0x1000/0x1001/0x1002` used for FG/BG/cursor defaults within Ghostty adapter mapping.

## Tests and downstream proof run
- `cargo fmt --all -- --check` — pass
- `cargo clippy --workspace --all-targets -- -D warnings` — pass
- `cargo test --workspace` — pass
- Production-path proofs:
  - `worker_backed_kitty_and_mouse_input_reaches_child_pty` — exact Kitty + SGR mouse bytes via `CoreDaemon::input` observed on child PTY
  - `worker_backed_mode_flags_include_kitty_and_mouse_from_ghostty_authority` — production `read_mode_flags` + GHOSTSNP
  - `worker_backed_osc_color_queries_receive_session_side_write_pty_replies` — pre-attach OSC 10/11/12 replies
- Downstream-shaped:
  - `ghostty_terminal_authority_conformance_test` (hub-facing ModeFlags / GHOSTSNP / palette shape helpers)

## Unverified behavior or residual risk
- End-to-end Hub client consumption of the new exports is intentionally deferred to Hub ticket dependency after Core merge
- Kitty keyboard protocol flag interpretation treats any non-zero `GHOSTTY_TERMINAL_DATA_KITTY_KEYBOARD_FLAGS` as enabled (matches protocol progressive enhancement)
- Default theme values are Botster-chosen session defaults for OSC query authority, not host theme policy

## Missing vault guidance discovered
- Session-side OSC color query ownership after write_pty lands (supersedes split-runtime color probe gotcha once processed)
- Production ModeFlags completeness matrix for pin `5e9ba17a…`
- Cold-turkey: `botster-core-daemon` no longer offers a no-default plain terminal lane
- Adapter README pin drift (trybotster/ghostty vs claimed upstream) — fixed in-repo; worth durable capture

## Runtime-teardown class
- `teardown_class_applies=false` per approved plan; lenses not required
