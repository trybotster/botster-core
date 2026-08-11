# Implementation report: Race-free terminal mode revision (review rework 8)

## Target repository and target_id
- Target repository: `botster-core`
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Run: `run_1786479064_760292`
- Run step: `run_step_1786489353_562127`
- PR: https://github.com/trybotster/botster-core/pull/120
- Addresses review: `review_1786489341_480696` (and open carry-forward `finding_1786488133_182843`)
- Implementation SHA: `ec51b930551a12c095475507ad4f623a87c0f2da`

## Playbooks applied
- [[implementer-playbook]], [[botster-implementer-playbook]], [[botster-core-playbook]]
- Worker atomic admit binding from `question_1786481243_140177`

## Findings addressed
| Finding | Fix |
| --- | --- |
| Test hook still names removed flush | Renamed public/test seams to `test_hold_after_enqueue_ms` / `--test-hold-after-enqueue-ms` with **no alias** (options, builders, CLI, daemon, worker, tests). |
| Tests permit remaining defects | Overflow matrix: **first** post-overflow op is gated (while retained may exist), then every probe; all must fail. Normal-drain FIFO test `worker_backed_mode_gated_normal_drain_preserves_mode_fifo` races parent `drain` under enqueue hold and asserts mouse-off. |

## Files changed
- `crates/botster-core/src/runtime/local_process.rs` — option rename + docs
- `crates/botster-core/src/runtime/worker_process.rs` — field/CLI rename
- `crates/botster-core-daemon/src/daemon.rs` — config rename
- `crates/botster-core-daemon/src/bin/botster-session-worker.rs` — CLI rename
- Tests: construction sites + overflow/normal-drain suite
- Implement report

## Ownership boundaries preserved
- Core Ghostty-free; daemon hosts worker + Ghostty
- Rename is test-only public seam (no production product API)

## Cross-repo dependencies
- Hub ticket `ticket_1786471489_718500` remains dependent; no Hub product edits

## Deviations from plan
- None

## Tests and downstream proof
- `cargo fmt --all`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `BOTSTER_ENV=test cargo test --workspace`
- All 12 `worker_backed_mode_gated_*` including:
  - `worker_backed_mode_gated_overflow_stays_failed_closed` (gated-first matrix)
  - `worker_backed_mode_gated_normal_drain_preserves_mode_fifo`

## Unverified behavior / residual risk
- Adopt mid-wait fail-closed via disconnect/timeout only
- Dual Ghostty intentional

## Missing vault guidance
- None

## Runtime-teardown
- `teardown_class_applies=false`
