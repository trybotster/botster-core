# Implementation report: Race-free terminal mode revision (review rework 2)

## Target repository and target_id
- Target repository: `botster-core`
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Run: `run_1786479064_760292`
- PR: https://github.com/trybotster/botster-core/pull/120
- Addresses review: `review_1786484890_817223`

## Playbooks applied
- [[implementer-playbook]], [[botster-implementer-playbook]], [[botster-core-playbook]]
- Worker atomic admit binding from `question_1786481243_140177`

## Findings addressed
| Finding | Fix |
| --- | --- |
| Unpublished PTY chunk before channel publish | Reader keeps fence critical until after channel publication; optional after-read hold stays critical |
| Deadline does not bound complete write | `write_all_blocking` takes deadline, uses `now >= deadline`, stops retries; partial write fails closed (`admitted=false`) |
| Tests miss remaining windows | `worker_backed_mode_gated_unpublished_reader_chunk_window_rejects`, `worker_backed_mode_gated_write_deadline_bounds_complete_write` |
| Probe success after barrier failure | Probe only sends success ModeFlags on barrier Ok; failures send `error_kind` and parent fail-closes |

## Files changed
- `crates/botster-core/src/runtime/local_process.rs` — critical-until-publish, deadline write, test hooks
- `crates/botster-core/src/runtime/worker_process.rs` — CLI test hooks; probe error_kind handling
- `crates/botster-core/src/contract/session_protocol.rs` — `ModeFlagsPayload.error_kind`
- `crates/botster-core-daemon/src/bin/botster-session-worker.rs` — deadline write; probe failure path; CLI args
- `crates/botster-core-daemon/src/daemon.rs` — test hook config
- Daemon integration tests for the two remaining windows

## Tests
- `cargo fmt --all`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `BOTSTER_ENV=test cargo test --workspace`
- All `worker_backed_mode_gated_*` including unpublished-chunk and write-deadline

## Residual risk
- Adopt mid-wait still covered by disconnect/timeout fail-closed, not a dedicated chaos test
- Dual Ghostty parent/worker for screen vs token remains intentional

## Ownership
- Core stays Ghostty-free; daemon hosts Ghostty worker binary

## Runtime-teardown
- `teardown_class_applies=false`
