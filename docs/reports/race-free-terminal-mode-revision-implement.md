# Implementation report: Race-free terminal mode revision (review rework 7)

## Target repository and target_id
- Target repository: `botster-core`
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Run: `run_1786479064_760292`
- Run step: `run_step_1786488790_304838`
- PR: https://github.com/trybotster/botster-core/pull/120
- Addresses review: `review_1786488779_536913`

## Playbooks applied
- [[implementer-playbook]], [[botster-implementer-playbook]], [[botster-core-playbook]]
- Worker atomic admit binding from `question_1786481243_140177`

## Findings addressed (review_1786488779)
| Finding | Fix |
| --- | --- |
| Concurrent drain can leak reader depth | `push_pending` / `take_pending` update `pressure.depth` under the **same** pending lock as queue mutation. |
| Dual-buffer path left alive after single-queue migration | Cold-removed channel `Receiver`/`Sender`, `try_flush_pending_to_channel`, and channel drain path. Reader finishes via `reader_finished` atomic. |
| Obsolete dual-buffer tests | Replaced with single-queue FIFO, concurrent depth, overflow, and finalization unit tests. |

### Carry-forward still green
- `ensure_mode_authority` after barrier apply (sticky overflow)
- Strict post-overflow probe/admit matrix
- Transfer-hold production test on single-queue enqueue window

## Files changed
- `crates/botster-core/src/runtime/local_process.rs` — pure single-queue ownership + atomic depth
- Implement report

## Ownership boundaries preserved
- Core Ghostty-free; daemon hosts worker + Ghostty

## Cross-repo dependencies
- Hub ticket `ticket_1786471489_718500` remains dependent; no Hub product edits

## Deviations from plan
- None material

## Tests and downstream proof
- `cargo fmt --all`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `BOTSTER_ENV=test cargo test --workspace`
- Unit: `concurrent_drain_does_not_leak_reader_depth`, `single_queue_fifo_order_is_preserved`, overflow/finalization
- All `worker_backed_mode_gated_*` production suite

## Unverified behavior / residual risk
- Adopt mid-wait fail-closed via disconnect/timeout only
- Dual Ghostty intentional
- Overflow still loses the unqueued current chunk; sticky fail prevents false admit

## Missing vault guidance
- None

## Runtime-teardown
- `teardown_class_applies=false`
