# Implementation report: Race-free terminal mode revision for mode-dependent input

## Target repository and target_id
- Target repository: `botster-core` (`trybotster/botster-core`)
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Run: `run_1786479064_760292`
- Ticket: `ticket_1786478568_882200`
- Plan: Plan visit 6 (`docs/archive/plans/race-free-terminal-mode-revision.md`)
- Base SHA: `747be95b8922130d3e2c3f6844e3dbe1deeb2faa`
- PR: https://github.com/trybotster/botster-core/pull/120
- Review findings addressed: `review_1786483831_205485` open findings

## Repository playbook and other playbooks/notes applied
- [[implementer-playbook]], [[botster-implementer-playbook]], [[botster-core-playbook]]
- [[ghostty shadow terminal integration belongs outside botster core]]
- Human binding `question_1786481243_140177`
- Not loaded: [[project-pipelines-playbook]]; teardown lenses (`teardown_class_applies=false`)

## Review findings addressed (this rework)
| Finding | Fix |
| --- | --- |
| Worker PTY read-to-write race | `LocalProcessRuntime::with_pty_io_barrier` pauses nonblocking reader, drains channel + residual OS PTY buffer, then writes under exclusive ownership |
| Timeout/disconnect late write | `deadline_unix_ms` on gated request; worker refuses write after deadline; timeout test waits past hold and asserts zero PTY bytes |
| Race matrix incomplete | Separate race (a)/(b), post-final-drain hold, interleaved demux, timeout late-write proofs |
| Uncorrelated probe / silent parent fallback | `ModeFlagsPayload.request_id` match; worker-backed probe fails closed (no parent token substitute) |
| Process-global hold env | Per-request `test_hold_ms` + `CoreDaemonConfig::with_test_mode_gated_hold_ms` |
| Plan trailing whitespace | Stripped |
| README duplicate daemon row | Merged runtime ownership table |

## Files changed (rework)
- `crates/botster-core/src/runtime/local_process.rs` — reader fence, residual drain, nonblocking master + blocking write retry, `PtyIoBarrier`
- `crates/botster-core/src/runtime/worker_process.rs` — deadline + test hold on request; correlated probe wait
- `crates/botster-core/src/contract/session_protocol.rs` — `deadline_unix_ms`, `test_hold_ms`, probe `request_id`
- `crates/botster-core/src/engine/botster.rs` — no silent parent mode fallback
- `crates/botster-core-daemon/src/bin/botster-session-worker.rs` — barrier admit + deadline
- `crates/botster-core-daemon/src/daemon.rs` — test hold config
- Daemon integration tests, protocol tests, README, plan whitespace

## Ownership boundaries preserved
- Core stays Ghostty-free; daemon hosts Ghostty worker binary
- Worker atomic admit remains correctness boundary

## Cross-repo
- Hub product still separate; prior path-patch compile policy retained
- Additive request fields remain wire-compatible for coordinated 0.1.0 consumers

## Deviations from plan
- Same as prior report for `CoreDaemonError::Engine` mapping
- Residual PTY reader under barrier supplements paused background reader (required for true pre-write drain)

## Tests and downstream proof
- `cargo fmt --all`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `BOTSTER_ENV=test cargo test --workspace`
- Mode-gated: admit/stale, race-a, race-b, post-final-drain hold, interleaved, timeout-without-late-write, packaging

## Unverified / residual risk
- Adopt mid-wait remains fail-closed via disconnect/timeout paths (no separate chaos test)
- Dual Ghostty parent/worker still exists for screen vs token
- Mismatched `request_id` handling covered in parent demux logic; no separate injectable-stale-result integration harness

## Missing vault guidance
- Nonblocking PTY master requires blocking write retries
- Admission barriers need residual OS-buffer drain when the background reader is paused

## Runtime-teardown class
- `teardown_class_applies=false`
