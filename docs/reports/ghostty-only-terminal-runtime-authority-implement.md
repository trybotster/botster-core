# Implementation report: Ghostty-only terminal runtime authority

## Target repository and target_id
- Target repository: `botster-core` (`trybotster/botster-core`)
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Run: `run_1786471511_632427`
- Ticket: `ticket_1786471489_484901`
- Worktree: Botster-managed ticket worktree for this run
- PR: https://github.com/trybotster/botster-core/pull/119
- Revision: addresses Review `review_1786474257_866024` findings

## Repository playbook and other playbooks/notes applied
- [[implementer-playbook]]
- [[botster-implementer-playbook]]
- [[botster-core-playbook]] (primary ownership charter)
- [[botster-terminal-ghostty-playbook]] (adapter surface in same workspace)
- Targeted notes: ghostty shadow terminal integration belongs outside botster core; session-process-owns-vt-parser-hub-rpc-snapshots; binary-page-snapshots-replace-vt-in-protocol; coredaemon must expose terminal truth used by the production hub path; pinned libghostty exposes synchronous exact mouse mode state; libghostty-vt-embedder-callback-architecture-and-constraints; synced state types are allowed while pushed event variants are forbidden; split terminal runtimes drop color probe responses before client attachment; botster core contract surface needs consumer proof; botster cli integration tests require ghostty submodule initialization; initial terminal snapshots must precede live output activation
- Not loaded: [[project-pipelines-playbook]] (out of scope); [[botster runtime teardown lenses]] (`teardown_class_applies=false`)

## Review findings addressed (this revision)
| Finding | Fix |
| --- | --- |
| `[ownership] Move the built-in color theme out of the Core terminal adapter` | Removed constructor theme from adapter. Daemon host composition seam `production_session_color_profile()` + `apply_color_profile` after Ghostty construct. |
| `[consumer-proof] Make the Hub-shaped authority assertion exact` | Always compare ModeFlags to screen carrier; require full 256 palette; special-color helper; negative panic tests. |
| `[cold-cut] Remove all active documentation for the deleted plain daemon lane` | README, core-daemon.md, ghostty-shadow-terminal-adapter.md, daemon capture_snapshot docs rewritten Ghostty-only. |
| `[test-gap] Verify all three OSC color replies and their values` | Production test asserts OSC 10/11/12 with exact production RGB and ordering, pre-attach. |
| `[privacy] Remove the committed machine-local target path` | Plan target path is path-neutral: resolved from target_id by Botster. |
| `[gate] Fix trailing whitespace in the committed plan` | Stripped trailing whitespace from plan file. |

## Files changed
### botster-terminal-ghostty
- `crates/botster-terminal-ghostty/src/sys.rs` — modes, color data/opts, write_pty callback types
- `crates/botster-terminal-ghostty/src/lib.rs` — full ModeFlags, palette, write_pty drain (no built-in theme policy)
- `crates/botster-terminal-ghostty/README.md` — pin truth trybotster/ghostty@5e9ba17a

### botster-core
- Neutral seams for color profile + drain_pty_writes; managed session PTY reply injection

### botster-core-daemon
- Non-optional Ghostty; single construction path
- **Production color profile seam** owned by daemon composition
- Integration proofs: Kitty/mouse input, mode flags, OSC 10/11/12 exact RGB

### botster-core-test-support
- Exact hub-shaped authority assertions + negative tests

### CI / docs
- Dropped daemon `--no-default-features` CI lanes
- Active plain-lane documentation removed from README and architecture docs
- Plan path-neutral + whitespace clean

## Ownership boundaries preserved
- Concrete Ghostty FFI/Zig remains in `botster-terminal-ghostty`
- Color **policy** (default theme) lives on the daemon host composition seam, not the adapter constructor
- `botster-core` only neutral `TerminalScreenRuntime` seams
- PlainTerminalScreenRuntime is library/test harness only; not reachable from CoreDaemon construction

## Cross-repo dependencies or separately routed work
- Hub ticket `ticket_1786471489_718500` depends on this Core ticket via `dependency_1786471500_696870`

## Deviations from plan
- None material. Color defaults supplied by daemon host seam rather than adapter constructor (review ownership correction).

## Tests and downstream proof run
- `cargo fmt --all -- --check` — pass
- `cargo clippy --workspace --all-targets -- -D warnings` — pass
- `cargo test --workspace` — pass
- Production-path proofs:
  - `worker_backed_kitty_and_mouse_input_reaches_child_pty`
  - `worker_backed_mode_flags_include_kitty_and_mouse_from_ghostty_authority`
  - `worker_backed_osc_color_queries_receive_session_side_write_pty_replies` (exact RGB for OSC 10/11/12)
- Downstream-shaped:
  - `ghostty_terminal_authority_conformance_test` including negative tests

## Unverified behavior or residual risk
- End-to-end Hub client consumption deferred to Hub dependency ticket after Core merge
- `kitty_enabled` is non-zero flag probe from Ghostty keyboard flags

## Missing vault guidance discovered
- Session-side OSC color query ownership after write_pty (policy vs mechanism split: host supplies profile, adapter owns reply mechanism)
- Production ModeFlags completeness matrix for pin `5e9ba17a…`
- Cold-turkey: `botster-core-daemon` no longer offers a no-default plain terminal lane

## Runtime-teardown class
- `teardown_class_applies=false` per approved plan
