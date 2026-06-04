# Prove Core Daemon Restart Adopts Live Session Workers

Ticket: `ticket_1780532711_256483`
Run: `run_1780542137_286527`

## Context Loaded

- Pipeline context loaded with `project_pipelines_current_context`: ticket `Prove core daemon restart adopts live session workers`, run `run_1780542137_286527`, current step `botster_plan`, gate `botster_plan_gate`, Plan Review `review_1780542634_499528`, and five open findings.
- Plan Review correction applied: the first plan was authored against stale HEAD `41e0110`. The worktree was fast-forwarded to `origin/main` `3c0c1d4`, bringing in the closed dependency work, including `crates/botster-core-daemon/`, the persistent `SessionRegistry`, durable session worker contracts, and existing daemon adoption evidence tests.
- Dependencies loaded from pipeline context:
  - `ticket_1780532710_740401` closed: `Implement local session worker process runtime`.
  - `ticket_1780532711_470736` closed: `Add core daemon supervisor and persistent session registry`.
- Required playbooks loaded:
  - `planner-playbook`
  - `botster-planner-playbook`
- Required Botster overlay and vault notes loaded:
  - `botster-architecture`
  - `cli-patterns`
  - `spa-patterns`
  - `project pipeline orchestration belongs in a device-level botster plugin`
  - `project pipelines needs an operator workbench not more primitives`
  - `project pipelines ui contract belongs in the plugin readme`
  - `botster orchestration should spawn agents with explicit target ids`
  - `botster orchestration prompts must bind agents to explicit worktrees`
  - `identity`
  - `goals`
- Ticket-specific architecture constraints from loaded maps and review:
  - `stale project pipeline worktrees can miss merged dependency apis`
  - `adoption restart evidence must come from real protocol primitives not defaults`
  - `botster runtime artifact resolution should be read only`
  - `sidecar session recovery is degraded below manifests`
  - `sessionioworker is the production read path for session pty output`
- Repo context inspected after updating to `origin/main`:
  - `crates/botster-core-daemon/src/daemon.rs`: `CoreDaemon` with spawn/list/attach/detach/input/resize/drain/subscribe_output/guarded_write/health/status/adoption_scan/shutdown.
  - `crates/botster-core-daemon/src/registry.rs`: filesystem-backed `SessionRegistry`, `RegistryRecord::observe_restart_contract`, and `RegistrySessionState`.
  - `crates/botster-core-daemon/src/api.rs`: `SessionAdoptionReport` and current `SessionAdoptionState::{Adoptable, Terminal, MissingProtocolEvidence}`.
  - `crates/botster-core-daemon/tests/daemon_integration_test.rs`: existing daemon API tests, registry persistence, adoption scan, and malformed-record skip behavior.
  - `docs/architecture/core-daemon.md`: authoritative daemon/adoption documentation.
  - `crates/botster-core/src/contract/durable_session.rs`: durable worker contracts including `SessionWorkerStaleReason::{IdentityMismatch, IncompatibleProtocol, HeartbeatExpired, ProcessMissing, WorkerDied, Other}` and durable restart semantics.
  - Prior plan artifacts: `core-daemon-supervisor-persistent-session-registry.md`, `durable-session-worker-protocol-restart-contract.md`, `local-session-worker-process-runtime.md`, and the older core runtime plans.
- Project Pipelines checklist discipline:
  - Run checklist `checklist_1780542198_108691` exists after the earlier create timeout.
  - Checklist evidence should record loaded vault/project notes, no convention conflicts, planning-only verification, and no pre-implementation durable capture.

## Plan Review Resolution

This revision accepts the Plan Review findings:

- The stale-base blocker is resolved by updating the worktree to `origin/main` `3c0c1d4`.
- The implementation surface is now `crates/botster-core-daemon`, not the older in-memory `botster-core` runtime path.
- Existing adoption mechanics must be reused and extended:
  - `CoreDaemon::adoption_scan()`
  - `SessionAdoptionReport`
  - `SessionAdoptionState`
  - `RegistryRecord::observe_restart_contract(...)`
  - `RegistrySessionState`
  - durable stale reasons from `botster-core::durable_session`
- The real proof gap is narrow:
  - a live session worker process survives daemon replacement and is reachable through the restarted daemon's list/attach/drain/input/shutdown path;
  - daemon adoption scan exposes exact failure semantics for stale/dead workers, duplicate workers, incompatible protocol versions, and missed heartbeats without inventing a parallel classifier.

## Scope

In scope:

- Extend `botster-core-daemon` restart/adoption behavior using the existing daemon crate.
- Add an end-to-end daemon restart test that starts a worker-backed local session, stops or replaces the `CoreDaemon` without killing the worker where the current worker runtime supports that, constructs a fresh `CoreDaemon` over the same `data_dir`, adopts the still-live worker, then proves list/attach/drain/input/shutdown through the restarted daemon.
- If the current local worker runtime cannot yet detach daemon ownership from worker process ownership, make that gap explicit in the implementation plan and add the smallest daemon/worker handle seam needed to keep the worker alive across daemon drop. Do not simulate adoption solely with registry rows.
- Extend existing daemon adoption state/failure reporting instead of adding a parallel adoption system. Prefer adding variants or structured fields to `SessionAdoptionState` / `SessionAdoptionReport` and mapping to existing `SessionWorkerStaleReason` values where possible.
- Add targeted tests for new failure semantics:
  - dead worker with live registry record;
  - duplicate workers for one session id;
  - incompatible protocol version;
  - missed heartbeat / ping-pong failure.
- Preserve existing covered behavior and cite it rather than reimplementing it:
  - registry durability and `MissingProtocolEvidence`/`Adoptable`/`Terminal` scan behavior;
  - malformed registry records do not block good records;
  - daemon spawn/list/attach/drain/input/resize/shutdown through the current API.
- Update `docs/architecture/core-daemon.md` with restart limitations, operator expectations, failure semantics, and the explicit no-botster-hub requirement.
- Keep registry/adoption discovery read-only until an explicit cleanup/mark operation runs. If implementation adds cleanup/marking for stale/dead records, test that mutation is explicit and deterministic.
- Keep tests and docs free of private paths, prompts, usernames, credentials, or terminal transcripts.

Non-scope:

- No changes to botster-hub, Rails, ActionCable, WebRTC, TUI, React SPA, Lua plugins, MCP, providers, cloud, auth, target admission, marketplace, or Project Pipelines product behavior.
- No new adoption classifier in `botster-core`; reusable protocol/stale-reason contracts already exist.
- No broad rewrite of `DefaultBotsterEngine`, `ManagedSessionRuntime`, `LocalProcessRuntime`, `SessionWorkerEngine`, or `MultiplexerEngine`.
- No second adoption document; `docs/architecture/core-daemon.md` is the authoritative docs target.
- No CLI-output parsing as proof. Tests should drive the typed daemon API first. CLI smoke can remain supporting evidence only.
- No speculative recovery policy beyond this ticket's restart-adoption proof and failure semantics.

Botster layers touched:

- Rust `botster-core-daemon` API/daemon/registry/tests: primary.
- Rust `botster-core` durable session contract: only if daemon needs a narrow exported stale/failure shape that is not already usable.
- Docs: `docs/architecture/core-daemon.md` and this plan.
- No plugin, Lua core, TUI, React SPA, Rails relay, MCP, or Project Pipelines runtime layer changes.

Worktree/target assumptions:

- Implementers work in the assigned Project Pipelines worktree for target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`, now fast-forwarded to `origin/main` `3c0c1d4`.
- This run is main-rooted. Do not stack from dependency branches.

Pipeline gates/artifacts:

- This file is the revised Plan artifact.
- Gate evidence should cite this file, the worktree update, and checklist `checklist_1780542198_108691`.

## Assumptions And Unknowns

Assumptions:

- `botster-core-daemon` is the production core daemon layer for this ticket.
- Current registry records already capture non-PII metadata and restart-contract evidence; this ticket should extend that record/reporting path, not replace it.
- `MissingProtocolEvidence` behavior is already covered and should remain fail-closed.
- Existing `SessionWorkerStaleReason` variants are the preferred vocabulary for incompatible protocol, heartbeat expiry, process missing, and worker death.
- Duplicate-worker semantics need to be represented in the daemon adoption surface. If no existing stale reason fits, add the smallest new variant or report field needed.
- The ticket's "cleaned or marked deterministically" can be satisfied by explicit mark-to-stale/dead semantics if removal would lose useful operator evidence.
- `docs/architecture/core-daemon.md` is the only docs target unless implementation discovers a separate existing document directly owning worker-process protocol detail.

Unknowns for implementation:

- Whether the current local session worker process runtime can genuinely outlive `CoreDaemon` drop, or whether daemon drop currently tears down `DefaultBotsterEngine` and the worker process. The implementer must inspect this before coding tests.
- Exact adoption method name. Current `adoption_scan` only classifies registry records; live-worker adoption may need a new method such as `adopt`, `adopt_session`, or an extended scan result that can rehydrate engine state.
- Exact duplicate-worker representation. A registry may currently have one record per session id, so duplicate detection may need to model duplicate worker endpoints/recovery identities rather than duplicate JSON files.
- Whether missed heartbeat should be terminal stale or degraded/unhealthy. Use the existing durable contract distinction if possible: missed heartbeat can be `Unhealthy::MissedHeartbeat` while expired heartbeat can be stale.
- Whether stale/dead cleanup should remove records or mark them. Prefer marking unless existing registry conventions already use removal for terminal records.

No human question is blocking this revised plan. The Plan Review findings identify the needed correction clearly.

## Affected Surfaces / Files

Expected:

- `crates/botster-core-daemon/src/api.rs`
  - Extend `SessionAdoptionState` / `SessionAdoptionReport` with exact failure semantics, reusing `SessionWorkerStaleReason` where appropriate.
- `crates/botster-core-daemon/src/daemon.rs`
  - Extend `adoption_scan` and add any live adoption/rehydration method needed for restarted daemon list/attach/drain/input/shutdown.
- `crates/botster-core-daemon/src/registry.rs`
  - Add narrow registry fields or mark helpers only if needed for dead/stale/heartbeat/duplicate semantics.
- `crates/botster-core-daemon/tests/daemon_integration_test.rs`
  - Add the end-to-end live-worker-survives-restart-and-reattaches test and targeted failure-semantics tests.
- `docs/architecture/core-daemon.md`
  - Update restart/adoption limitations, operator expectations, failure semantics, and no-botster-hub guarantee.
- `docs/plans/core-daemon-restart-adopts-live-session-workers.md`
  - This revised plan artifact.

Possible but keep narrow:

- `crates/botster-core/src/contract/durable_session.rs`
  - Only if a required stale reason such as duplicate worker is missing from the existing contract.
- `crates/botster-core-daemon/src/main.rs` or `tests/cli_smoke_test.rs`
  - Only if docs/API changes require CLI smoke updates. Do not make CLI smoke the primary proof.
- `crates/botster-core/src/runtime/worker_process.rs`
  - Only if the worker process ownership seam must change so daemon replacement can leave workers alive.
- `crates/botster-core/src/bin/botster-session-worker.rs`
  - Only for protocol evidence or heartbeat behavior required by adoption tests.

Not expected:

- New workspace dependencies.
- New daemon/adoption crate.
- New adoption docs outside `docs/architecture/core-daemon.md`.
- Broad changes to `botster-core` engine internals.

## Implementation Shape

Suggested minimal sequence:

1. Inspect the current daemon/worker ownership path and prove whether dropping the first `CoreDaemon` kills the worker. If it does, add the smallest explicit "daemon replacement without worker shutdown" seam required by this ticket.
2. Extend the existing adoption API, not a new classifier:
   - keep `Adoptable`, `Terminal`, and `MissingProtocolEvidence`;
   - add typed states or fields for dead/stale worker, duplicate worker, incompatible protocol, and heartbeat failure;
   - map protocol/health failures to `SessionWorkerStaleReason` or a new minimal variant.
3. Add a live adoption method if needed:
   - load registry record from `SessionRegistry`;
   - verify real restart-contract evidence already recorded or actively probed;
   - rehydrate the restarted daemon's engine/session route to the still-live worker;
   - preserve `list`, `attach`, `drain`, `input`, and `shutdown` through `CoreDaemon`.
4. Keep discovery and scan side-effect free. Add a separate explicit mark/cleanup path if stale/dead registry records must be updated.
5. Update `docs/architecture/core-daemon.md` to document:
   - adoption requires HELLO/WELCOME, ping/pong, and `SessionMetadata.recovery_identity`;
   - what each failure state means;
   - whether stale/dead records are marked or removed;
   - duplicate-worker behavior;
   - operator expectations and limitations;
   - adoption does not require botster-hub.

Runtime path proof:

- The primary test must construct two daemon instances over the same `data_dir`.
- The first daemon must start a real worker-backed session.
- The first daemon must be stopped/replaced without intentional session shutdown.
- The second daemon must adopt the still-live worker and prove the public daemon path:
  - `list`;
  - `attach` / `subscribe_output`;
  - `drain`;
  - `input`;
  - `shutdown`.
- Registry-only adoption scan is not enough, because existing tests already cover that.

## Risks

- Current worker ownership may still be tied to `DefaultBotsterEngine` drop. If so, the implementer must fix the ownership seam rather than weakening the test into a registry-only proof.
- Adding new adoption types beside existing `SessionAdoptionState` would create a parallel system. Extend existing types.
- Overloading `MissingProtocolEvidence` for dead worker, incompatible protocol, duplicate worker, or missed heartbeat would fail the ticket's exact failure-semantics requirement.
- Duplicate-worker fixtures can become artificial if the registry cannot represent duplicate endpoints. The test should model the real daemon/registry endpoint shape after inspection.
- Scan-time cleanup can hide bugs. Keep scan read-only and make marking/removal explicit.
- Real process restart tests can leak workers on failure. Use synthetic commands, temp dirs, and teardown guards that explicitly shut down or kill worker processes.
- PII can leak through registry/process metadata or docs examples. Use generic ids, command labels, and temp dirs only.
- CLI smoke can give false confidence. Typed daemon API tests are the required proof.

## Acceptance Checks / Tests

Existing tests to preserve and cite:

- `daemon_spawns_lists_attaches_drains_inputs_resizes_and_shuts_down`
- `registry_records_are_durable_enough_for_adoption_scan`
- `registry_load_all_skips_malformed_records_without_blocking_good_records`
- `session_worker_protocol_contract_test` coverage for durable protocol compatibility, heartbeat/health, restart semantics, and stale reasons.

Required new/updated targeted tests:

1. `daemon_restart_adopts_live_worker_and_reattaches`
   - Spawn a worker-backed session through `CoreDaemon`.
   - Replace the daemon without shutting down the worker.
   - Build a fresh daemon over the same `data_dir`.
   - Adopt the live worker.
   - Assert `list`, `attach`, `drain`, `input`, and `shutdown` all work through the restarted daemon.

2. `adoption_scan_reports_dead_worker_with_live_registry_record`
   - Registry record says running/adoptable evidence exists, but worker process is gone or endpoint is unreachable.
   - Assert a distinct dead/process-missing/worker-died state and deterministic mark/cleanup behavior if invoked.

3. `adoption_scan_reports_incompatible_protocol_version`
   - Registry or probe shows incompatible protocol version.
   - Assert distinct incompatible-protocol state and no attachable adopted session.

4. `adoption_scan_reports_missed_or_expired_heartbeat`
   - Simulate heartbeat/ping-pong evidence outside the accepted budget.
   - Assert exact missed/degraded versus expired/stale semantics documented in `core-daemon.md`.

5. `adoption_scan_reports_duplicate_worker_for_session`
   - Two live worker candidates claim one session id or recovery identity.
   - Assert distinct duplicate outcome or the documented deterministic resolution rule.

6. `adoption_scan_is_read_only_until_explicit_mark_or_cleanup`
   - Run scan/classification and assert registry files are unchanged.
   - Run explicit mark/cleanup and assert deterministic state update/removal.

7. `core_daemon_docs_cover_restart_failure_semantics`
   - Review/source assertion that `docs/architecture/core-daemon.md` documents live adoption, stale/dead marking, duplicate behavior, incompatible protocol, heartbeat handling, limitations, operator expectations, and no botster-hub requirement.

Suggested verification commands:

- `cargo fmt`
- `cargo test -p botster-core-daemon daemon_restart`
- `cargo test -p botster-core-daemon adoption`
- `cargo test -p botster-core-daemon`
- `cargo test -p botster-core session_worker_protocol`
- `cargo test -p botster-core local_session_worker_process`
- `cargo test -p botster-core`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`

If docs/rustdoc exports change:

- `cargo test --doc --workspace`
- `cargo doc -p botster-core-daemon --no-deps`

## Vault Checklist Evidence

- Notes read: `planner-playbook`, `botster-planner-playbook`, `identity`, `goals`, `botster-architecture`, `cli-patterns`, `spa-patterns`, Project Pipelines orchestration/workbench/UI-contract notes, explicit target/worktree orchestration notes, stale-worktree and adoption-evidence notes from the Botster maps, and the repo plans/docs listed in Context Loaded.
- Convention conflicts: none after this revision. The plan now follows the core-vs-daemon boundary and avoids a duplicate adoption system.
- Verification evidence in Plan step:
  - `git fetch origin`
  - `git merge origin/main` fast-forwarded `41e0110..3c0c1d4`
  - repo inspection only; no implementation tests run.
- Durable knowledge captured: not before implementation.

## Vault Gaps Worth Capturing

Capture after implementation if the final behavior settles any of these durable rules:

- The exact daemon adoption state vocabulary for dead workers, duplicate workers, incompatible protocol, and heartbeat failure.
- The rule for daemon replacement without worker shutdown if it requires a new ownership seam.
- Whether stale/dead registry records are marked or removed, and why.
- The duplicate-worker resolution rule if implementation chooses deterministic resolution instead of rejection.
