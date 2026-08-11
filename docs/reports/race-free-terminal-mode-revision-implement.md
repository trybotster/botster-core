# Implementation report: Race-free terminal mode revision (review rework 11)

## Target repository and target_id
- Target repository: `botster-core`
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Run: `run_1786479064_760292`
- Run step: `run_step_1786491094_660262`
- PR: https://github.com/trybotster/botster-core/pull/120
- Addresses review: `review_1786491067_654917` / finding `finding_1786491067_719525`
- Implementation SHA: b4aaaaa098c77b6c541bece553b3401dcca123fa

## Playbooks applied
- [[implementer-playbook]], [[botster-implementer-playbook]], [[botster-core-playbook]]
- Worker atomic admit binding from `question_1786481243_140177`

## Findings addressed
| Finding | Fix |
| --- | --- |
| Normal-drain FIFO test passes on defective dual-buffer SHA df38c218 | Added `single_queue_reader_source_prohibits_dual_buffer_transfer` unit source guard that bans dual-buffer transfer symbols (`try_flush_pending_to_channel`, dual-buffer/channel-before-pending comments). **Red** on production source of `df38c218092f59377bec12457840b0a7512bd294` (hits: try_flush_pending_to_channel, pending-to-channel, dual buffers, channel-before-pending, …). **Green** on HEAD unit test + production source. Strengthened normal-drain integration: require ≥2 mode revisions, mandatory freshness change, always-stale reject. |

## Files changed
- `crates/botster-core/src/runtime/local_process.rs` — dual-buffer transfer source guard
- `crates/botster-core-daemon/tests/daemon_integration_test.rs` — stricter normal-drain FIFO assertions
- Implement report

## Ownership boundaries preserved
- Core Ghostty-free; daemon hosts worker
- Test/source-guard only; no product API change

## Cross-repo dependencies
- Hub ticket `ticket_1786471489_718500` remains dependent

## Deviations from plan
- None

## Tests and downstream proof
- Source-guard red check: defective SHA production source contains banned dual-buffer symbols
- `cargo test -p botster-core single_queue_reader_source_prohibits_dual_buffer_transfer` (green on HEAD)
- `cargo test -p botster-core-daemon --test daemon_integration_test worker_backed_mode_gated_` (12/12)
- `cargo fmt --all`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `BOTSTER_ENV=test cargo test --workspace` (recorded at gate)

## Unverified behavior / residual risk
- Adopt mid-wait fail-closed via disconnect/timeout only
- Dual Ghostty intentional
- Source guard is the deterministic red-on-historical-mutant control; integration path proves Ghostty apply order under enqueue hold

## Missing vault guidance
- None discovered this visit (review requested red historical mutant proof; recorded above)

## Runtime-teardown
- `teardown_class_applies=false`
