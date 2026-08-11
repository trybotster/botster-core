# Implementation report: Race-free terminal mode revision (review rework 10)

## Target repository and target_id
- Target repository: `botster-core`
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Run: `run_1786479064_760292`
- Run step: `run_step_1786490370_745480`
- PR: https://github.com/trybotster/botster-core/pull/120
- Addresses review: `review_1786490355_764752` / finding `finding_1786490355_252756`
- Also retains prior fixes for carry-forward `finding_1786488133_182843` (overflow gated-first + normal-drain FIFO)
- Implementation SHA: 997fabef2021313a533c4b075a84437b9b1715ad

## Playbooks applied
- [[implementer-playbook]], [[botster-implementer-playbook]], [[botster-core-playbook]]
- Worker atomic admit binding from `question_1786481243_140177`

## Findings addressed
| Finding | Fix |
| --- | --- |
| Write-deadline test accepts parent timeout / early screen check | Parent mode-gated wait is now `timeout + MODE_GATED_REPLY_GRACE` (1s) so a correlated worker deadline result demuxes under load; worker write fence still uses wall-clock `deadline_unix_ms` only. Strict test requires `Ok(Gated)` with `deadline` error_kind and `bytes_written == 0`, rejects parent timeout, waits past force-block window, then asserts no `echo:deadline-bytes`. Timeout-path test hold raised above grace so true parent timeout remains covered. |

## Files changed
- `crates/botster-core/src/runtime/worker_process.rs` — `MODE_GATED_REPLY_GRACE`; parent Instant wait = write timeout + grace
- `crates/botster-core-daemon/tests/daemon_integration_test.rs` — strict write-deadline test; timeout test hold beyond grace
- Implement report

## Ownership boundaries preserved
- Core Ghostty-free; daemon hosts worker
- No public API surface change (parent wait is internal runtime behavior)

## Cross-repo dependencies
- Hub ticket `ticket_1786471489_718500` remains dependent

## Deviations from plan
- None. Reply grace is a correctness hardening of the correlated RPC wait already in the approved shape.

## Tests and downstream proof
- `cargo fmt --all`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test -p botster-core-daemon --test daemon_integration_test worker_backed_mode_gated_` (12/12)
- `BOTSTER_ENV=test cargo test --workspace` (recorded at gate)

## Unverified behavior / residual risk
- Adopt mid-wait fail-closed via disconnect/timeout only
- Dual Ghostty intentional
- Parent timeout path still drops a late-arriving result after grace; worker deadline fence remains the write authority

## Missing vault guidance
- None

## Runtime-teardown
- `teardown_class_applies=false`
