# Implementation report: Race-free terminal mode revision (review rework 4)

## Target repository and target_id
- Target repository: `botster-core`
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Run: `run_1786479064_760292`
- Run step: `run_step_1786486473_647569`
- PR: https://github.com/trybotster/botster-core/pull/120
- Addresses review: `review_1786486464_820344`
- Implementation SHA: `2aab4856570771172a553ac120cabef14dbc5e4f`

## Playbooks applied
- [[implementer-playbook]], [[botster-implementer-playbook]], [[botster-core-playbook]]
- [[scratch cargo patch redirects measure downstream dto breakage]] for Hub consumer proof
- Worker atomic admit binding from `question_1786481243_140177`

## Findings addressed (review_1786486464)
| Finding | Fix |
| --- | --- |
| Barrier reverses older channel vs newer pending | Drain order is **channel then fence pending** (then residual). Unit test `barrier_drain_order_is_channel_then_pending` asserts opposite mode chunks stay enable→disable. |
| Pending overflow drops PTY bytes + depth skew | `push_pending` never drops; full pending fails closed via `overflow_error`. Depth increments only after successful enqueue. Unit: `pending_overflow_does_not_drop_prior_events`. |
| Full-channel tests ignore mode order/loss | Production `worker_backed_mode_gated_full_reader_channel_preserves_mode_order` asserts final mouse off after enable→disable, freshness advance, stale reject, current admit. |
| Committed report contained private absolute path | Path-patch evidence uses neutral labels (`<core-worktree>`, temp dirs) only. |

### Carry-forward still green
- Fence-owned pending + nonblocking try_flush (no full-channel deadlock)
- `bytes_written` / `partial_write` public outcome
- Partial-write and write-deadline production tests

## Files changed
- `crates/botster-core/src/runtime/local_process.rs` — FIFO drain, no-drop overflow, unit tests
- `crates/botster-core-daemon/tests/daemon_integration_test.rs` — order-preserving full-channel production test
- `docs/reports/race-free-terminal-mode-revision-implement.md` — this rework (no private paths)

## Ownership boundaries preserved
- Core remains Ghostty-free; daemon hosts `botster-session-worker` + Ghostty
- Hub consumer-only; prior scratch path-patch evidence retained without private paths

## Cross-repo dependencies
- Hub ticket `ticket_1786471489_718500` still depends on this Core work
- No Hub product code changes

## Deviations from plan
- None beyond prior explicit partial-write public fields

## Tests and downstream proof
### Workspace
- `cargo fmt --all`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `BOTSTER_ENV=test cargo test --workspace` (and targeted mode-gated suite)
- Unit: `barrier_drain_order_is_channel_then_pending`, `pending_overflow_does_not_drop_prior_events`, full-channel FIFO unit
- Production: `worker_backed_mode_gated_full_reader_channel_preserves_mode_order` plus existing race/deadline/partial suite

### Scratch Hub path-patch (prior rework; still valid for additive DTO)
Diagnostic only; primary Hub checkout not modified.

```text
Core worktree: <core-worktree> (implementation SHAs below)
Hub SHA: 90d0e1adac7a7d3c6efc815173014c68b95dbbf3
Commands:
  git -C <hub-checkout> worktree add --detach /tmp/botster-hub-path-patch-… HEAD
  append [patch."https://github.com/trybotster/botster-core"] path redirects to <core-worktree> crates
  CARGO_TARGET_DIR=/tmp/botster-hub-path-patch-target-… cargo check --workspace
    → Finished `dev` profile … exit 0
  cargo check --workspace --all-targets
    → Finished `dev` profile … exit 0
```

## Unverified behavior / residual risk
- Adopt mid-wait still fail-closed via disconnect/timeout only
- Dual Ghostty (parent screen/OSC vs worker token) remains intentional
- Overflow fail-closed stops the reader; retained events still drain FIFO before the overflow error surfaces

## Missing vault guidance discovered
- None

## Runtime-teardown
- `teardown_class_applies=false`
