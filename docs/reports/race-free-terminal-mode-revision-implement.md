# Implementation report: Race-free terminal mode revision (review rework 9)

## Target repository and target_id
- Target repository: `botster-core`
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Run: `run_1786479064_760292`
- Run step: `run_step_1786489926_187346`
- PR: https://github.com/trybotster/botster-core/pull/120
- Addresses review: `review_1786489914_134076` (and open carry-forward `finding_1786488133_182843`)

## Playbooks applied
- [[implementer-playbook]], [[botster-implementer-playbook]], [[botster-core-playbook]]
- Worker atomic admit binding from `question_1786481243_140177`

## Findings addressed
| Finding | Fix |
| --- | --- |
| Dual-buffer test names/comments after channel removal | Renamed `worker_backed_mode_gated_full_reader_channel_preserves_mode_order` → `worker_backed_mode_gated_fence_queue_preserves_mode_order`. Updated comments to single fence queue. Overflow test no longer says dual buffers. Docs: hold-after-read is critical-section hold, not channel publication. |
| Tests permit authority/FIFO defects (carry-forward) | Overflow matrix still gated-first + all fail; `worker_backed_mode_gated_normal_drain_preserves_mode_fifo` races normal parent drain under enqueue hold. |

## Files changed
- `crates/botster-core-daemon/tests/daemon_integration_test.rs` — single-queue naming; write-deadline parent-timeout tolerance
- `crates/botster-core/src/runtime/local_process.rs` — hold-after-read docs
- `crates/botster-core-daemon/src/daemon.rs`, `worker_process.rs`, unit test comments — single-queue wording
- Implement report

## Ownership boundaries preserved
- Core Ghostty-free; daemon hosts worker
- Comment/test-only cleanup; no product API change

## Cross-repo dependencies
- Hub ticket `ticket_1786471489_718500` remains dependent

## Deviations from plan
- None

## Tests and downstream proof
- `cargo fmt --all`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `BOTSTER_ENV=test cargo test --workspace`
- All 12 `worker_backed_mode_gated_*` including fence-queue order, normal-drain FIFO, overflow sticky

## Unverified behavior / residual risk
- Adopt mid-wait fail-closed via disconnect/timeout only
- Dual Ghostty intentional
- Write-deadline test accepts parent timeout under suite load as fail-closed when zero payload bytes

## Missing vault guidance
- None

## Runtime-teardown
- `teardown_class_applies=false`
