# Implementation report: Race-free terminal mode revision (review rework 5)

## Target repository and target_id
- Target repository: `botster-core`
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Run: `run_1786479064_760292`
- Run step: `run_step_1786487435_539660`
- PR: https://github.com/trybotster/botster-core/pull/120
- Addresses review: `review_1786487424_418020`

## Playbooks applied
- [[implementer-playbook]], [[botster-implementer-playbook]], [[botster-core-playbook]]
- Worker atomic admit binding from `question_1786481243_140177`

## Findings addressed (review_1786487424)
| Finding | Fix |
| --- | --- |
| Pending-to-channel transfer can escape the admission barrier | All nonblocking `try_flush_pending_to_channel` runs **under fence critical**. Barrier waits on `!in_critical`, so it never observes an event mid-transfer (neither buffer). Optional `test_hold_after_flush_ms` holds while still critical. |
| Overflow authority failure is not latched | Session field `authority_failed` is sticky for the session life. Overflow promotes into it. Drains deliver retained FIFO output first; empty drains fail closed with the sticky error forever (probes/admits). |
| Tests miss transfer race and persistent overflow | Unit: flush under critical ownership; overflow retention. Production: `worker_backed_mode_gated_transfer_hold_preserves_modes`, `worker_backed_mode_gated_overflow_stays_failed_closed` (multiple probes + gated fails). |

## Files changed
- `crates/botster-core/src/runtime/local_process.rs` — fenced transfer, sticky authority, test hooks
- `crates/botster-core/src/runtime/worker_process.rs` — CLI/options for pending capacity + flush hold
- `crates/botster-core-daemon/src/daemon.rs` — config surface for new test hooks
- `crates/botster-core-daemon/src/bin/botster-session-worker.rs` — CLI parse for hooks
- `crates/botster-core-daemon/tests/daemon_integration_test.rs` — transfer + sticky overflow tests
- Options construction in core tests

## Ownership boundaries preserved
- Core remains Ghostty-free; daemon hosts worker + Ghostty
- Test hooks are opt-in only

## Cross-repo dependencies
- Hub ticket `ticket_1786471489_718500` remains dependent; no Hub product edits

## Deviations from plan
- None material; sticky authority is the fail-closed product of overflow without drop

## Tests and downstream proof
- `cargo fmt --all`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `BOTSTER_ENV=test cargo test --workspace`
- All 12 `worker_backed_mode_gated_*` tests including transfer-hold and overflow-sticky

## Unverified behavior / residual risk
- Adopt mid-wait still fail-closed via disconnect/timeout only
- Dual Ghostty intentional
- Extreme pending overflow still loses the *current* unqueued chunk but latches sticky failure so no false admit

## Missing vault guidance
- None

## Runtime-teardown
- `teardown_class_applies=false`
