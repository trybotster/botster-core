# Implementation report: Race-free terminal mode revision (review rework 3)

## Target repository and target_id
- Target repository: `botster-core`
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Run: `run_1786479064_760292`
- Run step: `run_step_1786485506_892406`
- PR: https://github.com/trybotster/botster-core/pull/120
- Addresses review: `review_1786485495_462845`

## Playbooks applied
- [[implementer-playbook]], [[botster-implementer-playbook]], [[botster-core-playbook]]
- [[scratch cargo patch redirects measure downstream dto breakage]] for Hub consumer proof
- Worker atomic admit binding from `question_1786481243_140177`

## Findings addressed (review_1786485495)
| Finding | Fix |
| --- | --- |
| Full reader channel deadlocks admission fence | Reader critical section only captures into fence-owned `pending`; channel send is non-blocking `try_flush` after `leave_critical`. Barrier drains `fence.pending` first. |
| Partial PTY write returns incorrect rejected outcome | `PtyWriteFailure.bytes_written` + public `ModeGatedPtyInputResult.bytes_written`; partial maps to `admitted=false`, `error_kind=partial_write:…`, nonzero `bytes_written` (not a clean reject). |
| Tests miss full-channel / partial-write | Unit: fence pending under full channel; partial write deadline. Production: `worker_backed_mode_gated_full_reader_channel_does_not_deadlock`, `worker_backed_mode_gated_partial_write_reports_bytes_written`. |
| Rework report removed Hub consumer proof | Scratch Hub path-patch re-run recorded below with SHAs and exit evidence. |

## Files changed
- `crates/botster-core/src/runtime/local_process.rs` — fence pending queue, try_flush, PtyWriteFailure, write max-chunk hooks, unit tests
- `crates/botster-core/src/runtime/worker_process.rs` — CLI hooks for max-chunk / capacity builders
- `crates/botster-core/src/contract/session_protocol.rs` — `ModeGatedPtyInputResult.bytes_written` contract docs
- `crates/botster-core-daemon/src/bin/botster-session-worker.rs` — partial vs clean reject mapping
- `crates/botster-core-daemon/src/daemon.rs` — test hooks + `pty_reader_chunk_capacity` config
- `crates/botster-core-daemon/tests/daemon_integration_test.rs` — full-channel + partial-write production tests
- Protocol/runtime construction tests for new fields

## Ownership boundaries preserved
- Core remains Ghostty-free; daemon hosts `botster-session-worker` + Ghostty
- Public contract change is additive (`bytes_written` with serde default)
- Hub remains consumer-only; measured via scratch path-patch, not committed Hub edits

## Cross-repo dependencies
- Hub ticket `ticket_1786471489_718500` still depends on this Core work
- No Hub product code changes in this implement visit

## Deviations from plan
- Partial delivery is explicit public outcome (`bytes_written` + `partial_write`) rather than inventing a third `admitted` enum value
- Parent drain remains optimization-only; correctness stays on worker atomic barrier

## Tests and downstream proof
### Workspace
- `cargo fmt --all`
- `cargo clippy --workspace --all-targets -- -D warnings` (pass)
- `BOTSTER_ENV=test cargo test --workspace` (run for this rework)
- All `worker_backed_mode_gated_*` including:
  - `worker_backed_mode_gated_full_reader_channel_does_not_deadlock`
  - `worker_backed_mode_gated_partial_write_reports_bytes_written`
  - prior unpublished-chunk and write-deadline windows

### Scratch Hub path-patch (durable consumer proof)
Diagnostic only; primary Hub checkout not modified. Scratch worktree of Hub + temporary `[patch."https://github.com/trybotster/botster-core"]` redirect to this Core worktree.

```text
Core path: /Users/jasonconigliari/botster-sessions/trybotster-botster-core-project-pipelines-ticket_1786478568_882200
Implementation SHA: a9d6d23feb40684d6cdddb13170d96522333374c
Core HEAD at probe start: 8e87c133653051e58b535e5a44a62f84591962ac
  (path-patch compiled the live dirty worktree committed as a9d6d23)
Hub SHA: 90d0e1adac7a7d3c6efc815173014c68b95dbbf3
Commands:
  git -C ~/Projects/botster-hub worktree add --detach /tmp/botster-hub-path-patch-… HEAD
  append [patch."https://github.com/trybotster/botster-core"] path redirects
  CARGO_TARGET_DIR=/tmp/botster-hub-path-patch-target-… cargo check --workspace
    → Finished `dev` profile … exit 0
  CARGO_TARGET_DIR=… cargo check --workspace --all-targets
    → Finished `dev` profile … exit 0
```

Result: Hub production + all-targets compile cleanly against this Core public surface (`ModeGatedPtyInputResult.bytes_written` additive default).

## Unverified behavior / residual risk
- Adopt mid-wait still covered by disconnect/timeout fail-closed, not a dedicated chaos test
- Dual Ghostty (parent screen/OSC vs worker token) remains intentional
- Partial write leaves un-undoable prefix bytes in the PTY; callers must honor `bytes_written`

## Missing vault guidance discovered
- None new; partial-write public outcome follows existing contract-proof norms

## Runtime-teardown
- `teardown_class_applies=false`
