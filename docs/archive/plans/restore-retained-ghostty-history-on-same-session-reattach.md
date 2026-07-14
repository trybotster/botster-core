# Restore Retained Ghostty History On Same-Session Reattach

Ticket: `ticket_1784057849_665844`

Run: `run_1784057878_843792`

## Outcome

Implementation found no missing Core producer behavior at the exact reported coordinate, Core `db69456` including `1ef5418`. The shipped Core change is therefore the missing regression lock only: a worker-backed real-Ghostty test proves retained semantic history on explicit detach followed by reattach to the same running session with a fresh client and fresh subscription identity.

Durable product clarification `question_1784059027_949092` supersedes the plan's conditional producer-fix steps: every supported Attach/reconnect uses a fresh `SubscriptionId`; same-ID reconnect is not a Core compatibility contract. Hub socket-loss stale-route cleanup remains owned by `ticket_1784052230_812754`, and TUI subscription rotation remains owned by `ticket_1783965015_654184`.

The regression is ablation-sensitive at the existing targeted initial-Snapshot delivery site. No Core production, protocol, Hub, TUI, or client workaround was added.

## Context Loaded

- Project Pipelines context: active Plan step `botster_plan`, gate `botster_plan_gate`, target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`; no prior artifacts, findings, reviews, dependencies, questions, or answers.
- Plan Review `review_1784058392_509955` returned four findings: pin fresh reconnect identities, target `subscription_multiplexer.rs`, use the revision-10-to-12 differential, and distinguish the initial visible-screen snapshot from configured scrollback-window replay. This revision addresses all four.
- Role and repository planning authority: [[identity]], [[goals]], [[planner-playbook]], [[botster-planner-playbook]], [[botster-architecture]], [[cli-patterns]], and [[spa-patterns]].
- Relevant terminal architecture: [[botster data plane bypasses the hub through session and client actors]], [[botster initial terminal scrollback is delivered by sessionio directly to clientworker]], [[initial terminal history replay targets one active subscription]], [[initial terminal snapshots must precede live output activation]], [[terminal subscribe readiness gates on sessionio initial snapshot delivery]], [[coredaemon attached follows initial snapshots before live terminal output]], [[opaque terminal snapshot bytes do not prove renderable history]], [[late subscriber snapshots must not resize the shared session pty]], and [[ghostty shadow terminal integration belongs outside botster core]].
- Workflow constraints: [[project pipeline orchestration belongs in a device-level botster plugin]], [[botster orchestration should spawn agents with explicit target ids]], [[botster orchestration prompts must bind agents to explicit worktrees]], [[plan agents must author vault context as wikilinks not home paths]], and [[project pipelines checklist worker timeouts require artifact evidence fallback]].
- Repository history and code inspected: current `HEAD` `db69456`, readiness producer commit `1ef5418`, its parent diff, `CoreDaemon::{attach,detach,drain}`, `WorkerBackedBotsterEngine::{attach_client,detach_client,drain_runtime_once}`, `ManagedSessionRuntime`, `SessionRuntimeWorkerAdapter`, `SubscriptionMultiplexer`, `ClientStreamHarness`, the session-worker process, Ghostty adapter/runtime tests, current daemon integration tests, architecture docs, and the archived attach-output/default-Ghostty plans.
- Revision differential: the ticket's production labels “revision 10” and “revision 12” are not encoded in this repository (`rg` found no revision constants or fixture labels), so their exact commit mapping must be recovered from the producing build metadata if available. Local history still bounds the prime candidate: `1ef5418` (“Emit attach readiness after initial snapshot”) is the final Core change before this run and directly changed `ClientStreamHarness` plus `SubscriptionMultiplexer` attach/readiness routing. Implementation starts with the revision mapping/range diff and the already-inspected `1ef5418^..1ef5418` diff; it does not assume that commit is guilty without the failing production-shaped test.
- Current runtime path: default `botster-core-daemon` enables `ghostty-terminal`; a configured `botster-session-worker` owns the PTY and emits raw PTY output, while the parent `ManagedSessionRuntime` feeds that output into the session's Ghostty-backed `TerminalScreenRuntime`. `SubscribeSession` requests a subscription-scoped initial snapshot; `InitialSnapshotReady` projects it before `Attached`; `CoreDaemon::attach` retains this output for `drain`.
- Existing proof gap: daemon tests cover a second late subscriber, future output after parent consumer detach/reattach, synthetic ordering, and direct Ghostty replay. They do not detach an established daemon client/subscription and then reattach the same running session while asserting the retained payload contains the already-visible history.
- Botster layer: Rust Core daemon, worker-backed managed session runtime, and concrete Ghostty terminal adapter. No plugin, Lua core, Hub, TUI, SPA, Rails relay, or MCP change is planned.
- Worktree assumption: implementation and review operate only in the assigned pipeline worktree for the explicit target above.

## Scope

1. Add a failing production-shaped regression in the daemon integration harness before changing producer code:
   - start the real `botster-session-worker` through default-feature `CoreDaemon`;
   - attach one client and wait for readiness;
   - produce unique prior and later-visible markers and prove both reached authoritative terminal state;
   - detach/disconnect that consumer through the public daemon path;
   - detach with the original `(ClientId, SubscriptionId)`, then reattach the same still-running `SessionId` with a fresh `ClientId` and fresh `SubscriptionId`, matching the production Hub/TUI reconnect topology;
   - capture the subscription-scoped initial `Snapshot` emitted before `Attached`, replay/decode it through the real Ghostty adapter, and require both visible-screen markers exactly once and in order;
   - require `Attaching < retained history < Attached < subsequent live marker` for the reattached subscription.
2. Diagnose the producer boundary with differential assertions, then make the smallest correction at the first point retained state is replaced, cleared, or captured from a fresh backend:
   - compare pre-detach `read_screen`/snapshot truth with the reattach-time `InitialSnapshotReady` payload;
   - confirm detach does not replace the session's `SessionRuntimeWorkerAdapter` or Ghostty terminal;
   - compare the payload produced by `ManagedSessionRuntime::request_initial_snapshot` with the payload delivered by `SubscriptionMultiplexer::route_initial_snapshot_ready` to the fresh reattach route;
   - confirm `remove_subscriber`/`add_subscriber` route lifecycle does not replace terminal state or deliver against the purged original identity;
   - confirm the reattach request is routed to that retained session-owned backend and not to a new/default terminal instance;
   - preserve targeted per-subscription replay and the existing initial-snapshot barrier.
3. Keep no-history behavior honest:
   - the plain/no-default-features path remains `Attaching -> Attached -> live` with no fabricated `Snapshot`/`Scrollback`;
   - an idle Ghostty backend may still emit its authoritative opaque blank snapshot, but that payload must not count as visible history and must not be converted into fabricated scrollback.
4. Add an explicit ablation of the fixed retained reattach delivery site. The regression must fail on marker fidelity/order when that one delivery is disabled, while the unmodified implementation passes.
5. Update architecture documentation only if the fix changes or clarifies the producer ownership contract. Do not document test-only mechanics as a new public abstraction.

## Non-Scope

- No Hub policy, Hub cache, client/TUI/browser workaround, last-known-state fabrication, or downstream adoption work.
- No legacy branch, dual protocol, versioned event, compatibility shim, or new daemon DTO.
- No synthetic session-wide scrollback cache or replay broadcast; history remains session-owned and replay remains client/subscription-targeted.
- No Ghostty dependency movement into `botster-core`; concrete backend mechanics remain in `botster-terminal-ghostty` and host-profile wiring remains in `botster-core-daemon`.
- No resize, backpressure, restart/adoption, terminal metadata, or adjacent cleanup unless the regression proves a touched line is necessary for retained same-session history.
- No scrolled-off/configured scrollback-window expansion. The initial reattach path is a visible-screen `TerminalScreenEngine::capture_snapshot`; configured scrollback retention is already covered by `worker_backed_daemon_default_ghostty_path_replays_configured_scrollback_window`.
- No broad refactor or optional configurability.

## Assumptions And Unknowns

- Assumption: “same-session reattach” means the same running `SessionId` after a real public detach/disconnect, not daemon restart and not a second concurrently attached client.
- Assumption: production reconnect uses a fresh `ClientId` and fresh `SubscriptionId` after detaching the original pair. This primary regression must not reuse either identity. Same-id reattach is not a substitute and is added only if a real caller is shown to support it.
- Assumption: history success is semantic. Non-empty byte length, a `Snapshot` variant, or an all-NUL/opaque blank envelope is insufficient; replayed Ghostty text must contain both unique markers exactly once and in order.
- Assumption: the ticket's no-history sequence forbids fabricated semantic history. This is compatible with [[opaque terminal snapshot bytes do not prove renderable history]]: a backend-authoritative blank Ghostty snapshot may appear before `Attached`, but cannot satisfy a history assertion. The no-default-features plain path must retain the exact no-snapshot sequence.
- Unknown: whether detach replaces/clears terminal state, whether reattach captures from the wrong adapter, whether fresh-route subscription bookkeeping loses/mismatches the produced snapshot, or whether Ghostty export/replay loses retained visible-screen state after the first attach cycle. The revision range, failing end-to-end test, and produced-versus-routed payload differential should select one surgical fix; the implementer must not patch several layers defensively.
- Unknown: exact commits represented by external production labels revision 10 and revision 12. Resolve them from available build metadata before coding when possible; if the mapping is unavailable, record that limitation and use the local candidate range ending at `1ef5418` plus the failing regression rather than guessing.
- Unknown: whether the production regression requires changes in core coordination or the Ghostty adapter. If authoritative pre-detach snapshot replay already contains both markers but reattach egress does not, fix core routing/capture. If the authoritative Ghostty export itself loses them across detach, fix the adapter while preserving the neutral core seam.
- No human question is currently blocking: the ticket explicitly rules out alternative ownership and compatibility strategies, and the semantic-history interpretation reconciles its no-history clause with the newer vault note about opaque blank Ghostty snapshots.

## Affected Surfaces And Files

Expected:

- `crates/botster-core-daemon/tests/daemon_integration_test.rs`
  - Add the worker-backed default-Ghostty disconnect/reattach regression, semantic snapshot replay helper assertions, ordering checks, exactly-once checks, no-history guard, and a clearly documented ablation seam/test command.
- One smallest producer fix selected by the failing differential, most likely in one of:
  - `crates/botster-core/src/engine/managed_session_runtime.rs` if reattach captures from fresh/non-retained parent terminal state;
  - `crates/botster-core/src/engine/subscription_multiplexer.rs` if `remove_subscriber`, `add_subscriber`, or `route_initial_snapshot_ready` loses or mismatches the freshly identified reattach route;
  - `crates/botster-core/src/contract/client_stream.rs` if the `1ef5418` readiness change routes the correct snapshot against stale client/subscription state;
  - `crates/botster-core/src/engine/botster.rs` only if facade detach/attach ordering is implicated by the differential;
  - `crates/botster-terminal-ghostty/src/lib.rs` if retained Ghostty state exists but export/replay after detach produces a blank payload.
- `crates/botster-core/tests/managed_session_runtime_test.rs` or `crates/botster-terminal-ghostty/tests/managed_session_runtime_test.rs`
  - Add the narrow unit/integration regression at the actual corrected boundary, paired with the production daemon proof.

Conditional:

- `docs/architecture/core-daemon.md` or `docs/architecture/ghostty-shadow-terminal-adapter.md` only if implementation clarifies a durable ownership/lifecycle rule not already documented.

Not expected:

- `crates/botster-core/src/bin/botster-session-worker.rs` and `crates/botster-core/src/runtime/worker_process.rs`: the worker currently owns PTY lifetime/bytes while the parent session adapter owns Ghostty terminal truth. Touch only if the failing differential disproves that current production contract.
- Protocol contract enums, Hub/client repositories, or feature manifests.

## Implementation Sequence

0. Resolve the production revision-10/revision-12 build commits from available metadata, then run `git log --oneline` and a path-restricted `git diff` across that range. Record candidate regressors, explicitly including or excluding `1ef5418` and its changes to client-stream/subscription readiness routing.
1. Add the exact fresh-identity detach/reattach daemon regression and demonstrate it fails because the decoded initial visible-screen snapshot lacks the prior markers, not because of timing or missing build prerequisites.
2. Add one produced-versus-routed differential: compare the `InitialSnapshotReady.snapshot` created by `ManagedSessionRuntime::request_initial_snapshot` with what `SubscriptionMultiplexer::route_initial_snapshot_ready` delivers after original-route removal and fresh-route addition. This separates blank capture from routing loss.
3. Correct only the owner selected by the revision diff and payload differential. Preserve session ownership, targeted replay, `Attaching`/`Attached` ordering, live-output barrier, and current geometry.
4. Add a narrow boundary test for the corrected owner and keep the production daemon test as the wiring proof.
5. Run the ablation by temporarily disabling/removing the corrected retained-payload delivery and record the expected targeted test failure; restore the implementation and rerun green.
6. Run package/workspace verification and update docs only if the durable contract changed.

## Risks

- False positive: asserting only event presence or payload length would accept the reported all-NUL/opaque blank regression. Mitigation: replay through Ghostty and assert visible markers exactly once and ordered.
- Timing flake: PTY/worker output is asynchronous. Mitigation: use existing bounded `drain_until`/`read_screen_until` helpers, unique markers, and prove both markers were authoritative before detach.
- Testing the wrong topology: a second subscriber does not exercise disconnect/reattach. Mitigation: call public `CoreDaemon::detach`, verify the worker/session remains live, then reattach that `SessionId`.
- Identity false positive: reusing the original client/subscription can exercise idempotent or stale-route behavior unlike production reconnect. Mitigation: require both identities to be fresh and assert no reattach delivery targets the detached pair.
- Snapshot/live race: later output could overtake bootstrap history. Mitigation: assert the full subscription-scoped event order through `CoreDaemon::drain`.
- Duplicate history: replay plus buffered live output can render markers twice. Mitigation: exact occurrence counts for each retained marker and one distinct post-`Attached` live marker.
- Shared-state damage: reattach capture could resize or replace the session backend. Mitigation: preserve current session geometry and add an assertion if the diagnosed site can mutate it.
- Overfitting to Ghostty internals: decoding in acceptance tests could leak backend policy into core. Mitigation: keep production egress opaque and use Ghostty replay only in the default-host test helper.
- Build-environment failure: default daemon tests require initialized Ghostty source and Zig `0.15.2`. Mitigation: distinguish prerequisite failure from test failure and run plain-backend checks separately.
- Broad defensive edits could hide the root cause. Mitigation: require the differential and ablation to identify one load-bearing producer change.

## Acceptance Checks And Tests

Focused production proof (exact final test name may follow local naming):

```sh
git submodule update --init crates/botster-terminal-ghostty/vendor/ghostty
BOTSTER_ENV=test cargo test -p botster-core-daemon --test daemon_integration_test worker_backed_ghostty_same_session_reattach_restores_retained_history -- --nocapture
```

The focused test must prove all of the following:

- a real worker-backed default-Ghostty session remains running across detach;
- two unique, nearby pre-detach visible-screen markers exist in authoritative terminal state without being scrolled off;
- detach uses the original identities and reattach uses a fresh `ClientId` plus fresh `SubscriptionId` for the same running session;
- the reattach initial `Snapshot` decodes to both markers exactly once and in order;
- all-NUL/non-renderable payloads fail the semantic-history assertion;
- reattach egress orders `Attaching`, retained history, `Attached`, then a distinct later live marker;
- no original client/subscription receives reattach-only delivery;
- no-history/plain fallback emits no fabricated history and reaches `Attached` before live output.

Ablation proof:

- Temporarily remove/disable only the corrected retained reattach payload delivery.
- Run the focused production test and record a failure specifically at decoded marker fidelity/order.
- Restore the fix and rerun the identical command green. Do not commit the ablation.

Regression and quality gates:

```sh
BOTSTER_ENV=test cargo test -p botster-core-daemon --test daemon_integration_test
BOTSTER_ENV=test cargo test -p botster-core-daemon --no-default-features
BOTSTER_ENV=test cargo test -p botster-terminal-ghostty --features libghostty-vt
BOTSTER_ENV=test cargo test -p botster-core
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --doc --workspace
```

- This extracted core repository has no `cli/test.sh`; README and CI use raw Cargo. `BOTSTER_ENV=test` remains explicit on runtime tests.
- Planning baseline attempted the existing default-Ghostty late-subscriber test; it failed before compilation because the vendored Ghostty submodule was uninitialized. That is a prerequisite gap, not behavioral evidence.
- If docs change, also run `cargo doc --workspace --no-deps`.
- Scan committed artifacts for local paths/PII and confirm every changed line traces to retained reattach fidelity, required verification, or documentation made necessary by the fix.

## Pipeline Gates And Artifacts

- Plan artifact: this file plus structured `botster_plan_gate` evidence.
- Implement evidence must include: failing-before regression output, diagnosed loss boundary, changed production entry point, green focused test, ablation failure and restoration pass, package/workspace checks, and exact attribution for any unrelated failure.
- Implement evidence must also record the revision-10/revision-12 commit mapping and path-restricted diff, or explicitly state why external build metadata could not map those labels and show the local candidate range used instead.
- Review must check correctness, same-session topology, semantic payload fidelity, ordering, duplicate delivery, no-history behavior, architecture boundary, hidden compatibility paths, dead/unwired helpers, and absence of Hub/client workarounds.
- Verify must rerun the default Ghostty path with prerequisites, the plain no-default-features path, and the ablation or inspect recorded reproducible ablation evidence.

## Vault Gaps Worth Capturing

- Capture after implementation if confirmed: a focused note naming the exact mechanism by which same-session detach/reattach selected fresh Ghostty state or lost retained pages. Current vault notes cover ordering and opaque-payload semantics, but not this concrete lifecycle failure.
- Capture if the test helper becomes reusable: production Ghostty history tests must replay opaque snapshots and assert renderable marker fidelity; non-empty payload length is not evidence.
- Do not capture the uninitialized-submodule planning failure as new knowledge; [[botster cli integration tests require ghostty submodule initialization]] and repository README already cover the prerequisite shape.

## Checklist Evidence

- Vault context: loaded the notes listed above; they constrain ownership to Core/session actors, preserve the concrete Ghostty boundary outside core, require targeted replay and snapshot-before-live ordering, and reject byte-length-only history proof.
- Convention review: no conflict. The plan is a surgical Core producer fix using existing engine/runtime/adapter seams, adds no abstractions or compatibility branches, and keeps Hub/client policy out of scope.
- Verification evidence: inspected current pipeline context, git history, production entry points, tests, docs, feature wiring, and submodule status. The attempted focused daemon baseline failed only because `vendor/ghostty` is uninitialized; no behavioral test result is claimed.
- Durable capture: none during Plan. The exact root cause is still unknown; capture is deferred until implementation proves the mechanism.
- Project Pipelines checklist: `checklist_1784057922_800206`. Initial creation timed out to the caller but persisted asynchronously; this artifact and gate evidence retain the same evidence as fallback.
