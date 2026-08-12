# Implementation report: Race-free terminal mode revision (review rework 14)

## Target repository and target_id
- Target repository: `botster-core`
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Run: `run_1786479064_760292`
- PR: https://github.com/trybotster/botster-core/pull/120
- Addresses review: `review_1786496210_758623` / finding `finding_1786496210_169896`
- Scope guard: no new public production test hook / broad flag plumbing
- Evidence artifact referenced: `artifact_1786496190_368348`
- Scope-cleanup tip: `060723624a8238870ec91777c97a4e3e5dd6b5a4` (field removal complete; no remaining `test_fail_closed_when_pending_full` reads)
- Prior product tip: `41221cb0246d9aa2c536826fa1f1f846f77689a4`
- Prior tip: `0943a416f0c8e6ff8a51f032bf0919f7de29331e`

## Clean-tip re-verification (this visit)
- Worktree: clean (`git status --porcelain` empty)
- `cargo check --workspace --all-targets` PASS
- `cargo fmt --all -- --check` PASS
- `cargo clippy --workspace --all-targets -- -D warnings` PASS
- `BOTSTER_ENV=test cargo test --workspace` PASS (incl. many_pty adversarial backpressure, daemon_integration 61, mode-gated 12)
- Focused: small-capacity adversarial multi-session PASS; bounded reader backpressure PASS; ordinary pressure lossless PASS
- `git diff --check` clean
- No public `test_fail_closed_when_pending_full` flag; ReaderFence has no residual field reads

## Product fix
Ordinary fence pending capacity-full **waits outside the fence critical section**
(lossless + OS PTY backpressure). Sticky mode-authority failure is reserved for
**true loss** (reader `Failed` event) and an **internal unit-only**
`set_overflow_error` seam — not a public daemon/worker flag.

## Scope guard (this visit)
Removed the short-lived `test_fail_closed_when_pending_full` public plumbing from
LocalProcessRuntimeOptions, WorkerProcessRuntimeOptions, CoreDaemonConfig, and
worker CLI. Forced-loss proofs use the smallest internal seam:

| Proof | Seam |
| --- | --- |
| Sticky after injected overflow | unit `set_overflow_error` + drain |
| Sticky after true reader failure | unit `ReaderEvent::Failed` latch |
| Ordinary pressure is not sticky | worker integration flood + current-token admit |

## Files changed (product + scope trim)
- `crates/botster-core/src/runtime/local_process.rs`
- `crates/botster-core/src/runtime/worker_process.rs`
- `crates/botster-core-daemon/src/daemon.rs`
- `crates/botster-core-daemon/src/bin/botster-session-worker.rs`
- `crates/botster-core-daemon/tests/daemon_integration_test.rs` — `worker_backed_mode_gated_ordinary_pressure_stays_lossless`
- `crates/botster-core/tests/local_process_runtime_test.rs` — small-capacity adversarial multi-session
- Implement report

## Gates
- Unit forced-loss + lossless wait PASS
- `worker_backed_mode_gated_` 12/12 PASS
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `BOTSTER_ENV=test cargo test --workspace`
- `git diff --check`

## Merge / close
- Not merged; ticket not closed
