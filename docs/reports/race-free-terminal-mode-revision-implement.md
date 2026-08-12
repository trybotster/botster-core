# Implementation report: Race-free terminal mode revision (review rework 13)

## Target repository and target_id
- Target repository: `botster-core`
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Run: `run_1786479064_760292`
- PR: https://github.com/trybotster/botster-core/pull/120
- Addresses review: `review_1786496210_758623` / finding `finding_1786496210_169896`
- Evidence artifact referenced: `artifact_1786496190_368348`
- Prior tip: `0943a416f0c8e6ff8a51f032bf0919f7de29331e`

## Playbooks applied
- [[implementer-playbook]], [[botster-implementer-playbook]], [[botster-core-playbook]]
- Ordinary reader pressure is lossless; sticky fail-closed mode authority only after true loss

## Findings addressed
| Finding | Fix |
| --- | --- |
| Reader pressure became fatal session-wide authority overflow (`OutputFailed: pty reader buffer overflow: mode authority incomplete`) on Linux CI many-PTY load | Production capacity-full path now **waits outside the fence critical section** (lossless ordinary pressure + OS PTY backpressure). Sticky overflow retained only for true loss / test-only forced-loss injection. |

## Root cause
Private pending limit `max(reader_capacity*8, 256)` (default 512) treated capacity full as sticky mode-authority failure, stopped the reader, blocked process-exit finalization, and failed fair aggregate drain. Linux can split noisy PTY streams into more chunks than macOS, so ordinary pressure hit the private limit on Ubuntu CI.

## Contract retained
- Single fence-owned FIFO
- Sticky fail-closed mode authority after **true loss** / forced-loss injection
- Mode barriers: reader **never blocks while holding critical** (leave critical before wait)
- Forced-loss test still proves sticky reject: `test_fail_closed_when_pending_full`

## Files changed
- `crates/botster-core/src/runtime/local_process.rs` — lossless wait path + pending_cv + unit proofs
- `crates/botster-core/src/runtime/worker_process.rs` — wire test forced-loss flag
- `crates/botster-core-daemon/src/daemon.rs` — config + wiring
- `crates/botster-core-daemon/src/bin/botster-session-worker.rs` — CLI flag
- `crates/botster-core-daemon/tests/daemon_integration_test.rs` — overflow sticky uses forced-loss flag
- `crates/botster-core/tests/local_process_runtime_test.rs` — small-capacity adversarial multi-session proof
- `crates/botster-core/tests/local_session_worker_process_test.rs` — options field
- Implement report

## Tests and gates
- `cargo test -p botster-core --features local-runtime --lib pending_full_wait_is_lossless_after_drain` PASS
- `cargo test -p botster-core --features local-runtime --test local_process_runtime_test small_capacity_adversarial_chunks` PASS
- `cargo test -p botster-core-daemon --test daemon_integration_test worker_backed_mode_gated_` 12/12 PASS
- `cargo test -p botster-core-daemon --test daemon_integration_test worker_backed_mode_gated_overflow_stays_failed_closed -- --exact` PASS (forced-loss)
- `cargo fmt --all -- --check` PASS
- `cargo clippy --workspace --all-targets -- -D warnings` PASS
- `BOTSTER_ENV=test cargo test --workspace` PASS (incl. downstream_conformance 16+1 ignored)
- `git diff --check` clean

## Ownership boundaries preserved
- Core owns policy-free local reader / mode-token mechanism
- Daemon hosts worker; Ghostty authority unchanged
- No Hub product change

## Cross-repo dependencies
- Hub ticket `ticket_1786471489_718500` remains dependent

## Deviations from plan
- None

## Unverified / residual risk
- Linux CI re-run is the ultimate environment proof for many-PTY chunking
- Dual Ghostty intentional

## Runtime-teardown
- `teardown_class_applies=false`

## Merge / close
- Not merged; ticket not closed
