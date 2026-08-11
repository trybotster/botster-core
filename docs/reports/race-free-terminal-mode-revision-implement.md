# Implementation report: Race-free terminal mode revision (review rework 6)

## Target repository and target_id
- Target repository: `botster-core`
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Run: `run_1786479064_760292`
- Run step: `run_step_1786488144_829112`
- PR: https://github.com/trybotster/botster-core/pull/120
- Addresses review: `review_1786488132_330624`
- Implementation SHA: `81338d1f5a72c67e0e64964210b9c51f863a9741`

## Playbooks applied
- [[implementer-playbook]], [[botster-implementer-playbook]], [[botster-core-playbook]]
- Worker atomic admit binding from `question_1786481243_140177`

## Findings addressed (review_1786488132)
| Finding | Fix |
| --- | --- |
| Retained output hides sticky authority on first gated op | `PtyIoBarrier::ensure_mode_authority()`; worker `apply_barrier_outputs` applies retained drain then **always** calls ensure before returning Ok. Probe/admit cannot succeed after sticky overflow. |
| Normal drain can reverse events during pending transfer | Reader uses a **single fence-owned FIFO** for ownership (no production pending→channel transfer). Concurrent normal drains cannot reverse mid-transfer. |
| Tests permit remaining defects | Overflow test requires **every** post-overflow probe (incl. first) and gated call to fail. Transfer-hold production test retained. |

## Files changed
- `crates/botster-core/src/runtime/local_process.rs` — single-queue ownership; `ensure_mode_authority`
- `crates/botster-core-daemon/src/bin/botster-session-worker.rs` — apply retained then ensure
- `crates/botster-core-daemon/tests/daemon_integration_test.rs` — strict overflow matrix
- Implement report

## Ownership boundaries preserved
- Core Ghostty-free; daemon hosts worker + Ghostty

## Cross-repo dependencies
- Hub ticket `ticket_1786471489_718500` remains dependent; no Hub product edits

## Deviations from plan
- None material; single queue is the durable FIFO boundary for mode authority

## Tests and downstream proof
- `cargo fmt --all`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `BOTSTER_ENV=test cargo test --workspace`
- All 12 `worker_backed_mode_gated_*` including strict overflow and transfer-hold

## Unverified behavior / residual risk
- Adopt mid-wait fail-closed via disconnect/timeout only
- Dual Ghostty intentional
- Unqueued overflow chunk is still lost at overflow; sticky fail prevents false admit

## Missing vault guidance
- None

## Runtime-teardown
- `teardown_class_applies=false`
