# Implementation report: ClientWorker terminal egress and subscription teardown

Ticket: `ticket_1786661004_845807`
Run: `run_1786669014_213203`
Step: `botster_stack_implement`
Plan: `docs/archive/plans/core-clientworker-terminal-egress-and-subscription-teardown.md`

## Target repository and target_id

- Target repository: `botster-core`
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Spawn-target name: `botster-core` (`trybotster/botster-core`)
- Independent `list_spawn_targets` resolution matches the approved plan
- Pipeline worktree: Botster-managed ticket worktree for this run
- Merge policy: `direct` (no PR)

## Repository playbook and other playbooks/notes applied

Repository charter: [[botster-core-playbook]]

Role and overlay:

- [[implementer-playbook]]
- [[botster-implementer-playbook]]
- [[botster runtime teardown lenses]]
- [[botster-core uses CI-owned Cargo commands because it has no test script]]
- [[implementation artifacts must match actual git state]]
- [[implement gate must verify committed work and pr link before review]]
- [[implementation steps must persist report artifacts for review]]
- [[pipeline vault checklists must cite exact resolvable note titles]]

Not loaded:

- [[project-pipelines-playbook]] — package/plugin paths are out of scope
- Other repository charters — this run stays inside `botster-core`

Targeted notes:

- [[botster durable terminal egress is owned by sessionio and clientworker actors]]
- [[terminal subscription lifecycle is Core owned while host session policy is Hub owned]]
- [[proposed ClientWorker owns terminal queues and terminal frames never retry]]
- [[proposed dead sink handling triggers one Core detach without a Hub round trip]]
- [[proposed ProcessExited closes terminal subscriptions but not the host session]]
- [[proposed Core publishes the transport adapter conformance harness]]
- [[conformance harnesses gate on deterministic invariants not timing]]
- [[botster core contract surface needs consumer proof]]
- [[botster core public surface needs a narrow start here path]]
- [[botster core public enums are breaking until non exhaustive is decided]]
- [[terminal adapter traits must not reuse TransportIngress or TransportEgress]]

## Botster layers changed

- `botster-core` production ClientWorker, bind/inventory/generation-aware detach, adapter pump on the existing host tick
- `botster-core-daemon` public bind, inventory, and generation-aware detach
- `botster-core-test-support` shared Fake handle plus isolated Hub-shaped bind consumer
- Living architecture and README pointers for the bound-adapter path

No Lua plugin, Hub runtime, real Unix socket, real WebRTC DataChannel, TUI, SPA, Ghostty crate change, or Project Pipelines product layer.

## Files changed

Create:

- `crates/botster-core/src/contract/terminal_subscription.rs`
- `crates/botster-core/src/engine/client_worker.rs`
- `crates/botster-core/tests/client_worker_engine_test.rs`
- `docs/architecture/client-worker-terminal-egress.md`
- `docs/reports/core-clientworker-terminal-egress-implement.md`

Edit:

- `crates/botster-core/Cargo.toml`
- `crates/botster-core/src/contract/mod.rs`
- `crates/botster-core/src/contract/terminal_adapter.rs`
- `crates/botster-core/src/engine/mod.rs`
- `crates/botster-core/src/engine/botster.rs`
- `crates/botster-core/src/engine/botster/takeover_fail_closed_tests.rs`
- `crates/botster-core/src/engine/managed_session_runtime.rs`
- `crates/botster-core/src/runtime/worker_process.rs`
- `crates/botster-core/src/lib.rs`
- `crates/botster-core-daemon/src/daemon.rs`
- `crates/botster-core-daemon/src/lib.rs`
- `crates/botster-core-daemon/tests/daemon_integration_test.rs`
- `crates/botster-core-test-support/src/terminal_adapter/core.rs`
- `crates/botster-core-test-support/src/terminal_adapter/fake.rs`
- `crates/botster-core-test-support/src/terminal_adapter/mod.rs`
- `crates/botster-core-test-support/tests/terminal_adapter_conformance_test.rs`
- `crates/botster-core-test-support/tests/consumers/hub-adapter-shaped/Cargo.toml`
- `crates/botster-core-test-support/tests/consumers/hub-adapter-shaped/src/lib.rs`
- `crates/botster-core-test-support/tests/consumers/hub-adapter-shaped/Cargo.lock`
- `docs/architecture/terminal-adapter.md`
- `docs/architecture/engine-command-surface.md`
- `docs/README.md`
- `README.md`
- `Cargo.lock`

## Ownership boundaries preserved

Core owns ClientWorker queues, slow-client policy, attach phases, detach, adapter pump, inventory, and the non-blocking `close()` / `Drop` law.

Hub still owns adapter admission, Unix/WebRTC instances, route reconciliation, and host session cleanup after ProcessExited. This run does not implement those.

`TransportIngress` / `TransportEgress` names are unchanged. Bind APIs are not in `prelude`. Start-here remains spawn → attach → drain → input → shutdown.

## Cross-repo dependencies or separately routed work

- Depends on closed `ticket_1786661004_133253` (adapter contract) on the same target
- Consumed later by Hub Unix `ticket_1786661008_634435`, Hub WebRTC `ticket_1786661008_247079`, Hub drain cold-cut `ticket_1786661010_198387`, and integration `ticket_1786661010_115885`
- No new cross-repo work was implemented here

## Deviations from plan

None. Review `review_1786673745_713182` required three further product fixes that fulfill the existing plan:

- A second client attaching the same `(session_id, subscription_id)` hard-stops the first owner and assigns generation + 1.
- Snapshot phase rows are removed on unbound Snapshot delivery and on every hard-stop.
- A pending client's replacement subscription drops that client's older incremental-attach tuples.

Review `review_1786673068_686714` required five product fixes that fulfill the existing plan rather than change scope:

1. In-flight accepted writes that stay Full/WouldBlock count toward the 512-tick bound.
2. A client attaching a new subscription for the same session hard-stops the previous owner.
3. Later frames for a route that failed mid-ingest are discarded, not leaked onto drain.
4. Unbound ProcessExit stays on drain, then the inventory row is removed.
5. Inventory is published only after subscribe admission succeeds.

Typed bind errors remain `BindBeforeAttach`, `UnknownSubscription`, `StaleGeneration`, `AlreadyBound`. The committed plan's teardown-bound and attach-matrix wording was updated to match.

Review `review_1786675180_532728` required one further product fix that fulfills the existing ownership-identity lens:

- Worker-backed same-key owner replacement cancels the live IncrementalAttach owner's snapshot boundary and starts the replacement boundary on the attach path. Reconcile uses `(client_id, subscription_id)`, not subscription_id alone. `WorkerProcessRuntime` Drop cancels any outstanding snapshot before `SHUTDOWN` so a fenced worker cannot hang `child.wait()`.

Review `review_1786680402_104609` required one further product fix:

- `promote_pending_fail_closed` now discards the outgoing owner's queued input and resize before it starts a sibling. Generation detach, pre-READY failure, and teardown reconcile cannot apply the removed owner's work.

Review `review_1786679644_592711` required two further fixes:

- Every fallible pending promotion now uses `promote_pending_fail_closed`. A begin failure detaches that owner and discards its queues, then continues.
- The stale-queue test reattaches C while D's recovered boundary is still active, then completes both.

Review `review_1786678909_717793` required one further product fix:

- When a pending owner's recovery begin fails, Core now drops that client's queued input and queued resize so a later successful sibling and a fresh C reattach cannot reuse the rejected generation's work.

Review `review_1786678096_983456` required two further fixes:

- Recovery begin failure now detaches every remaining pending owner instead of dropping IncrementalAttach while those owners stay published.
- Snapshot fail-next injection is `#[cfg(test)]` and `pub(crate)` only. `CoreDaemon` no longer exposes test failure switches.

Review `review_1786677187_382484` required three further product fixes on that takeover path:

- Takeover cancels the old boundary and begins the replacement boundary before it publishes the new ClientWorker owner. Cancel or begin failure does not leave a published owner without IncrementalAttach. If recovery cannot start a pending sibling, that sibling is detached. Injected cancel and begin failures live only in crate tests.
- Takeover keeps accepted input and the latest resize for pending sibling clients. It drops only the replaced owner and the new owner's obsolete pending work.
- Takeover removes the new owner's pending tuples before that client becomes the active IncrementalAttach owner, so an obsolete B/Y boundary cannot start after B/X finishes.

## Runtime-teardown lenses implemented

| Lens | Implementation |
| --- | --- |
| Isolation | One subscription owns queue, adapter, barrier interest, and inventory row. Sibling subscriptions stay live. |
| Bounded teardown | 512 unsuccessful writes fail that subscription. Hard stop is ownership remove + synchronous non-blocking `close()` + drop on the same host tick. No closer thread. |
| Late-message matrix | Attach assigns generation. Bind requires the live generation. Pre-attach bind is typed. Detach and Closed are idempotent. Input/output after teardown cannot recreate the owner. |
| Production-path proof | `CoreDaemon::drain` / `DefaultBotsterEngine::drain_runtime_once` pump ClientWorker. Fake `delivered_frame_bytes` contains remaining output then `process_exit` before Closed. Inventory row is gone. Adapter is dropped. Sibling still pumps. Session stays listed. |
| Ownership identity | `(session_id, subscription_id, generation)`. Reattach is generation + 1. Stale detach does not delete N+1. A different `client_id` on the same key cancels the previous IncrementalAttach request_id and starts generation + 1. |
| Sibling / fail-closed | Successful close and write-budget failure isolate one subscription. ProcessExited closes every subscription on that session by design. Same-key takeover keeps pending sibling input and resize. Failed takeover does not publish a new owner without a tracked boundary. |

Human answer `question_1786670811_244393` is implemented as a contract bound, not a waiver: Core calls `close()` synchronously; a blocking close is an adapter defect.

## Tests and downstream proof run

Repository gates:

```bash
cargo fmt --all -- --check
BOTSTER_ENV=test cargo clippy --workspace --all-targets -- -D warnings
BOTSTER_ENV=test cargo test --workspace
BOTSTER_ENV=test cargo test --doc --workspace
```

All four passed.

Focused production-path proofs:

- `BOTSTER_ENV=test cargo test -p botster-core --test client_worker_engine_test` — includes in-flight stall budget, replacement attach, unbound ProcessExit teardown, rejected-attach inventory absence, and capacity-plus-two drain isolation
- `BOTSTER_ENV=test cargo test -p botster-core-daemon --test daemon_integration_test -- worker_bound_adapter_receives_ready_finish_without_drain_snapshots`
- `BOTSTER_ENV=test cargo test -p botster-core-daemon --test daemon_integration_test -- worker_incremental_attach_streams_ready_pages_finish_then_queued_work_and_live_output`
- `BOTSTER_ENV=test cargo test -p botster-core-daemon --test daemon_integration_test -- --exact worker_same_key_owner_replacement_cancels_the_active_boundary`
- `BOTSTER_ENV=test cargo test -p botster-core --lib takeover_fail_closed_tests` — includes failed C queues vs D recovery and a fresh C reattach
- `worker_same_key_takeover_preserves_pending_sibling_input_and_resize`
- `worker_same_key_takeover_drops_the_new_owners_obsolete_pending_subscription`
- Isolated Hub-shaped consumer via `botster-core-test-support` `terminal_adapter_conformance_test`

`drain_until_for_client` now uses `REAL_WORKER_IDLE_TIMEOUT` / `REAL_WORKER_COMPLETION_TIMEOUT` and fails with the last observed output.

## Unverified behavior or residual risk

- Real Hub Unix and WebRTC adapters are not in this repository. They must run the same close conformance proof on their later tickets.
- A production adapter that violates the non-blocking `close()` contract can still block the host tick. That is an adapter defect, not a Core closer thread.
- Process-thread-count equality is not asserted under workspace parallelism; the production-path test asserts thread count does not grow across ProcessExited close+drop.
- `WorkerProcessRuntime` Drop now sends snapshot `cancel` before `SHUTDOWN`. `child.wait()` is still unbounded if Ghostty encode never returns. That encode hang is outside this ticket.

## Missing vault guidance discovered

After merge, capture if still true:

- Bound-adapter ClientWorker push is current Core behavior, not only a proposed north-star note
- Drain remains the unbound / transitional host path until Hub cold-cut
- Subscription ownership identity is `(subscription_id, generation)`
- Attach precedes bind. `close()` and `Drop` are non-blocking state changes. Core does not spawn a closer thread

No Hub adapter implementation details should be captured from this Core slice.
