# Implementation report: Race-free terminal mode revision (review rework 12)

## Target repository and target_id
- Target repository: `botster-core`
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Run: `run_1786479064_760292`
- PR: https://github.com/trybotster/botster-core/pull/120
- Addresses review: `review_1786494863_541037` / finding `finding_1786494863_440502`
- Evidence artifact referenced: `artifact_1786494842_253259`
- Base tip reviewed: `52cb7bb6895c0912e55ae32e140c286811138fcd`

## Playbooks applied
- [[implementer-playbook]], [[botster-implementer-playbook]], [[botster-core-playbook]]
- Deterministic producer / condition-driven PTY readiness (no unexplained fixed sleep)

## Findings addressed
| Finding | Fix |
| --- | --- |
| `worker_backed_mode_gated_normal_drain_preserves_mode_fifo` nondeterministic: `mode_revision >= baseline+2` depended on PTY chunk timing | Replaced `sleep 0.05` producer gap with a handshake: child emits enable + `enabled`, then blocks on stdin until parent releases. Test condition-polls until worker applies mouse-on and revision advances, then releases disable. Retains ≥2 revision, mouse-off FIFO, freshness change, and mandatory stale reject. Race/normal-drain proof kept (hold_after_enqueue + normal drains during hold). |

## Root cause
Worker samples `ModeFlags` once after each applied PTY chunk. If enable and disable coalesce into one read, Ghostty ends mouse-off with **net-zero** observed flags, so `mode_revision` stays at baseline. That is the production contract; the test must establish a worker-application boundary before requiring two revisions.

## Red evidence (defective / flaky form at 52cb7bb)
1. **Stress flake:** 12× `worker_backed_mode_gated_` suite → 11 PASS / 1 FAIL. Failure:
   `expected ≥2 mode revisions … baseline mode_revision: 1 final mode_revision: 1`
2. **Forced-coalesce deterministic red:** same test with enable+disable in one `printf` → always FAIL with baseline and final revision both 1 (proves the revision-count assertion is invalid without a producer boundary).

## Green evidence (fix)
1. Exact test 10× PASS
2. Focused `worker_backed_mode_gated_` suite 8× PASS (12/12 each)
3. `cargo fmt --all -- --check` PASS
4. `cargo clippy --workspace --all-targets -- -D warnings` PASS
5. `BOTSTER_ENV=test cargo test --workspace` PASS (`daemon_integration_test` 61 passed)
6. `git diff --check` clean

## Files changed
- `crates/botster-core-daemon/tests/daemon_integration_test.rs` — deterministic producer/worker-application boundary for normal-drain FIFO proof
- `docs/reports/race-free-terminal-mode-revision-implement.md` — this report

## Ownership boundaries preserved
- Test-only change; no product API or worker mode-sampling contract change
- Core remains Ghostty-free; daemon hosts worker
- Ticket race proof not weakened: still holds enqueue, normal drain during hold, enable→disable FIFO, ≥2 distinct mode observations, stale reject

## Cross-repo dependencies
- Hub ticket `ticket_1786471489_718500` remains dependent

## Deviations from plan
- None; chose deterministic producer boundary over dropping the ≥2 revision requirement

## Unverified behavior / residual risk
- Fence-order sibling still uses sleep-separated producer (softens stale only when net-zero); out of scope for this finding
- Dual Ghostty intentional

## Missing vault guidance
- None; existing deterministic-producer / poll-for-readiness notes covered this

## Runtime-teardown
- `teardown_class_applies=false`

## Merge / close
- Not merged; ticket not closed
