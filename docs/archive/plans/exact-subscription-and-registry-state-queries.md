# Plan: exact non-mutating subscription membership and session registry-state queries

Ticket: `ticket_1787104273_140454`
Run: `run_1787104535_852826`
Target repository: `botster-core` (`tgt_1f7bce66eb304881980f9b4a2a5ae3fe`, trybotster/botster-core)
Base: `origin/main` (302c7f7)

## Context loaded

- Repository charter: [[botster-core-playbook]]
- Role playbooks: [[planner-playbook]], [[botster-planner-playbook]]
- Targeted atomic notes:
  - [[host ShutdownSession classification must call the exact-session Core query]]
  - [[botster-core uses CI-owned Cargo commands because it has no test script]]
  - [[botster core contract surface needs consumer proof]]
  - [[botster core public enums are breaking until non exhaustive is decided]]
  - [[Core control-plane lifecycle journal advances without a terminal client or Hub terminal Drain]] (via charter routing)
  - [[terminal subscription lifecycle is Core owned while host session policy is Hub owned]] (via charter routing)
  - [[colon worktree paths break cargo dyld library paths]] (checked: this worktree path has no colon)
- Code read: `crates/botster-core-daemon/src/daemon.rs`, `crates/botster-core-daemon/src/registry.rs`, `crates/botster-core-daemon/src/api.rs`, `crates/botster-core/src/engine/client_worker.rs`, `crates/botster-core/src/engine/managed_session_runtime.rs`, `crates/botster-core/src/engine/botster.rs`, `crates/botster-core/src/contract/terminal_subscription.rs`, `crates/botster-core-daemon/tests/daemon_integration_test.rs`, `crates/botster-core-test-support/tests/*`.
- Runtime-teardown class: **not applicable.** Both queries are read-only control-plane lookups. No WebRTC/peer lifecycle, no SessionIo/ClientWorker teardown, no multi-peer ownership, no FD/CPU spin, no terminal-state divergence. [[botster runtime teardown lenses]] intentionally not loaded per its own scope rule.

## Problem

Hub ticket_1786912569_840742 needs bounded Pump phases. Today the only public
subscription inventory, `CoreDaemon::list_terminal_subscriptions`
(daemon.rs:941 → `ClientWorker::list_terminal_subscriptions`,
client_worker.rs:238), clones every live row and sorts the full vector. The
only exact-session lifecycle query, `CoreDaemon::observe_session_lifecycle`
(daemon.rs:829), mutates: it calls `observe_session`, which drains the runtime
once, reconciles lifecycle observations into the registry, and raises the
coalesced journal wake through `append_lifecycle_change`
(daemon.rs:2611 `journal_advanced = true`).

Core already has exact internal primitives:

- `ClientWorker::live_generation(session_id, subscription_id)` — one map get
  (client_worker.rs:276).
- `ManagedSessionRuntime::terminal_subscription_generation` (managed_session_runtime.rs:521).
- `DefaultBotsterEngine::terminal_subscription_generation` (botster.rs:513) and
  `WorkerBackedBotsterEngine::terminal_subscription_generation` (botster.rs:1090).
- `SessionRegistry::load(session_id)` — one file read (registry.rs:164).

Missing hops are only the `DaemonEngine` match arm and public `CoreDaemon`
methods.

## Scope

1. **Exact subscription membership query.**
   `CoreDaemon::terminal_subscription_generation(&self, session_id: &SessionId, subscription_id: &SubscriptionId) -> Option<TerminalSubscriptionGeneration>`
   - Add a `DaemonEngine::terminal_subscription_generation` match arm delegating
     to the existing engine methods (both variants already expose it).
   - `Some(generation)` is the live owner's generation; `None` is explicit
     absence. `&self`, `#[must_use]`, mirroring `list_terminal_subscriptions`
     (no `ensure_running`, identity-only).
   - No full-inventory clone, no sort, no allocation proportional to registry
     or inventory size.
   - Returning the generation (not the full record) is the smallest surface
     that satisfies the ticket's "record or generation". If Plan Review or the
     Hub consumer needs `adapter_bound`/`capabilities`, the alternative is a
     new `ClientWorker::subscription_record` map-get returning one cloned
     `TerminalSubscriptionRecord`; not planned by default.

2. **Exact non-mutating registry-state query.**
   `CoreDaemon::session_registry_state(&self, session_id: &SessionId) -> Result<SessionRegistryStateLookup, CoreDaemonError>`
   - `ensure_running()?` first (shutdown returns `Err`, mirroring
     `observe_session_lifecycle`). Note `ensure_running` must be callable from
     `&self`; it only reads `self.running` today.
   - Then exactly one `self.registry.load(session_id)?`:
     - `Some(record)` → `Found(record.state)` — one of `Running`, `Stopping`,
       `Exited`, `Stale` (`RegistrySessionState`, registry.rs:16).
     - `None` and `self.engine.session(session_id).is_none()` → `Absent`.
     - `None` but engine has the session → `Err(CoreDaemonError::UnknownSession)`,
       mirroring `observe_session_lifecycle` (daemon.rs:844-845) so the two
       exact-session queries can never disagree about absence.
   - **No** `observe_session`, no `drain_runtime_once`, no
     `reconcile_lifecycle_observations`, no registry save, no journal append,
     no wake. The method takes `&self`, so non-mutation of daemon state is
     compiler-enforced in addition to the required negative tests.
   - New public type in `api.rs`, inside the lifecycle-types section (before
     `AttachedSession`) so `lifecycle_api_types_are_control_plane_only` scans it:

     ```rust
     #[non_exhaustive]
     pub enum SessionRegistryStateLookup {
         Found(RegistrySessionState),
         Absent,
     }
     ```

     `#[non_exhaustive]` per [[botster core public enums are breaking until non
     exhaustive is decided]] and the `SessionLifecycleLookup` precedent
     (api.rs:335). `RegistrySessionState` itself is reused unchanged.
   - Export the new type from `lib.rs` alongside the other api types.

3. **Architecture tests.**
   - Extend `lifecycle_api_types_are_control_plane_only`
     (daemon_integration_test.rs:149) to assert
     `pub enum SessionRegistryStateLookup` is present in the scanned
     control-plane section (the existing forbidden-string scan then covers it).
   - Add a source-shape test (repo idiom: `include_str!` + section split, as in
     the lifecycle test and client_worker_engine_test.rs:675) over the two new
     `CoreDaemon` methods asserting their section contains none of
     `list_terminal_subscriptions`, `load_all`, `sort`, `observe_session`,
     `append_lifecycle`, `TransportEgress`, `TerminalSnapshotPayload`,
     `client_egress`.

4. **Work-bound and behavior tests** (daemon.rs `cfg(test)` mod and/or
   daemon_integration_test.rs):
   - Subscription query: attach → `Some(generation)` equal to the inventory
     row's generation; detach → `None`; unknown session or subscription →
     `None`; generation increments across re-attach are visible.
   - Registry-state query work-bound proof: reuse the
     `exact_session_lookup_does_not_scan_a_large_registry` pattern
     (daemon.rs:3741): 257 dummy registry rows, query one, assert
     `registry.test_load_all_calls() == 0`, `registry_load_all_calls == 0`,
     `observe_index_scans == 0`, `baseline_index_scans == 0`, plus the existing
     ablation that a direct `load_all` increments the counter.
   - Non-mutating negative test: spawn one session; clear the spawn wake with
     `take_journal_advanced_wake()`; record the lifecycle cursor; wait for the
     child process to actually exit using a bounded OS-level check (no sleep as
     oracle, per repo test discipline; reuse the existing finite-producer
     fixture pattern); call `session_registry_state` → still
     `Found(Running)` (parked exit NOT reconciled); assert
     `take_journal_advanced_wake() == false` and `lifecycle_changes_page` from
     the recorded cursor returns no new changes; positive control: one
     `observe_session_lifecycle` call then reconciles to `Exited`, sets the
     wake, and appends a journal change. The positive control proves the exit
     was genuinely pending, so the negative half is meaningful.
   - Absence and shutdown behavior: absent id → `Absent`; after daemon
     shutdown → `Err`.

5. **Downstream-shaped consumer proof** (charter requirement: crate-local
   tests alone are insufficient for public contract changes):
   - Extend `crates/botster-core-test-support/tests/consumers/hub-lifecycle-shaped`
     to call `session_registry_state` in a close-suppression-shaped check that
     replaces a full `list()` read, and assert the non-mutating contract
     (wake stays clear).
   - Extend `crates/botster-core-test-support/tests/consumers/hub-adapter-shaped`
     to check exact membership through
     `terminal_subscription_generation` where it currently scans
     `list_terminal_subscriptions` for one route (keep existing list-based
     assertions that genuinely need inventory).
   - These isolated crates run through their wrapper tests
     (`lifecycle_journal_consumer_test.rs` etc.), which the workspace test run
     starts; per [[botster-core-playbook]], confirm the wrapper tests execute
     (a workspace filter would not run the nonmember crates directly).

6. **Docs.** Rustdoc on both methods states the contract: exact, work-bound,
   control-plane only, non-mutating (registry query), and that
   `observe_session_lifecycle` remains the reconciling query for hosts that
   want lifecycle progress. Any doctest examples must pass
   `cargo test --doc --workspace`.

## Non-scope

- No change to `observe_session_lifecycle`, `list_terminal_subscriptions`,
  `observe_lifecycle_slice`, `lifecycle_baseline_page`,
  `lifecycle_changes_page`, `take_journal_advanced_wake`, journal semantics,
  or terminal subscription lifecycle.
- No terminal bytes, phases, snapshots, or attach state on either new surface.
- No Hub scheduling work; Hub ticket_1786912569_840742 consumes these queries
  after merge via a Core git pin bump in botster-hub.
- No new wrapper script; gates stay the CI-owned Cargo commands.
- No `#[non_exhaustive]` retrofits to existing enums.

## Ownership boundaries and cross-repo dependencies

- Core owns both queries as policy-free mechanisms; Hub owns when and why to
  call them (close suppression, Pump scheduling). No host policy enters Core.
- Consumer: Hub ticket_1786912569_840742 (bounded fair owner-loop background
  scheduling) will bump its Core pin to a merged Core main revision containing
  these queries. That dependency is registered on the Hub ticket; this run has
  no upstream prerequisites and registers no new dependencies.
- Downstream-shaped proof lives in botster-core-test-support consumer crates,
  inside this repository.

## Assumptions and unknowns

- Assumption: the Hub Pump/close-suppression consumer needs membership +
  generation identity, not `adapter_bound`/`capabilities`; the ticket's
  "record or generation" wording permits the generation-only shape. The
  record-returning alternative is documented above if review disagrees.
- Assumption: `Err(UnknownSession)` for the registry-missing-but-engine-live
  anomaly is correct because it mirrors `observe_session_lifecycle`; a
  registry row is written synchronously during spawn, so the state is
  abnormal by construction.
- Unknown (implementer choice): the exact bounded mechanism to await real OS
  process exit in the negative test without draining; reuse the repository's
  existing finite-producer patterns, never a bare sleep oracle.
- Verified: `ensure_running(&self)` (daemon.rs:1758) is `&self`-compatible, so
  both new queries can take `&self`.

## Affected surfaces/files

- `crates/botster-core-daemon/src/daemon.rs` — two public methods,
  `DaemonEngine::terminal_subscription_generation` arm, exact-session tests.
- `crates/botster-core-daemon/src/api.rs` — `SessionRegistryStateLookup`.
- `crates/botster-core-daemon/src/lib.rs` — export.
- `crates/botster-core-daemon/tests/daemon_integration_test.rs` — architecture
  test extension, source-shape test, behavior tests.
- `crates/botster-core-test-support/tests/consumers/hub-lifecycle-shaped/src/lib.rs`
  and `.../hub-adapter-shaped/src/lib.rs` — consumer-shaped proof (and their
  wrapper tests' source assertions if they enumerate required calls).
- No changes required in `crates/botster-core` (all engine primitives exist);
  if a record-returning query is chosen instead, add one map-get to
  `client_worker.rs` and thread it through the existing layers.

## Risks

- **Negative-test flake**: waiting for real process exit must be bounded and
  deterministic; mitigated by reusing existing finite-producer fixtures and a
  positive observe control in the same test.
- **Public surface creep**: one new `#[non_exhaustive]` enum, one `Option`
  return; no new state vocabulary (reuses `RegistrySessionState`).
- **Consumer-proof gap**: forgetting that consumer crates are nonmembers;
  run their wrapper tests and confirm they executed.
- **Semantic drift between the two exact queries**: prevented by mirroring the
  `Found/Absent/UnknownSession` classification and asserting it in tests.
- **Wake-bit test ordering**: spawn sets the wake; tests must clear it before
  asserting the query leaves it clear.

## Acceptance checks/tests

- Exact subscription query returns the live generation for a present
  subscription and `None` for absent, with the work-bound source-shape proof
  (no `list_terminal_subscriptions`, no sort, no `load_all` in the query path).
- Registry-state query: large-registry counter test proves no collection scan;
  negative test proves no journal advancement, no coalesced wake, and no
  parked-exit reconciliation from the query alone, with a mutating positive
  control.
- `lifecycle_api_types_are_control_plane_only` passes with the new type
  included; the new source-shape test keeps terminal bodies off both surfaces.
- Consumer-shaped tests in both hub-shaped consumer crates compile and pass
  against the new queries.
- Gates (from [[botster-core uses CI-owned Cargo commands because it has no
  test script]]):
  - `BOTSTER_ENV=test cargo test --workspace`
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --doc --workspace`
- Delivery: direct merge to main, no PR, per ticket policy.

## Vault gaps

- Capture candidate after implementation: "CoreDaemon exact-session queries
  take &self and prove work bounds with registry load_all counters" — extends
  the existing exact-session note with the non-mutating variant.
- Capture candidate: hub-shaped consumer crates are the standing proof vehicle
  for new public CoreDaemon queries (pattern now used three times).
