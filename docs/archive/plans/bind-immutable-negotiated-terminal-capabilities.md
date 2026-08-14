# Core: bind immutable negotiated terminal capabilities on adapter subscriptions

Ticket: `ticket_1786682902_405026`
Run: `run_1786682907_564108`
Target: `botster-core` / `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
Pipeline: `botster_stack_delivery` / `botster_stack_plan`
Base: pipeline worktree on `botster-core` at `ce7474a`
Required by: Hub Unix adapter ticket `ticket_1786661008_634435`
Human answers:
- `question_1786682822_139812` chose option 1B
- `question_1786683571_824777`: a required Rust argument satisfies omission. An empty set is valid. Do not add a runtime missing-set bind error.

Revision 2 addresses Plan Review `review_1786684058_934252`:
- `finding_1786684058_740060` (product): empty sets stay valid. Remove `UnsupportedCapabilities` from Core bind.
- `finding_1786684059_547807` (process): load [[project-pipelines-playbook]] as workflow-only context.
- `finding_1786684058_290943` (process): reuse `artifact_1786683335_768743` and `checklist_1786683323_490735`. Do not recreate them.

This ticket is **not** runtime-teardown class. It extends bind and inventory on the shipped ClientWorker owner. It does not change teardown, peer lifecycle, or late-message admission. Do not load [[botster runtime teardown lenses]].

## Target repository and target_id

- Target repository: `botster-core`
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Spawn-target name: `botster-core`
- Repository path from `list_spawn_targets`: the admitted `botster-core` spawn target
- Repository playbook: [[botster-core-playbook]]
- Resolved from ticket `target_id` through `list_spawn_targets`. Not inferred from the ambient session directory.

## Repository playbook loaded

[[botster-core-playbook]]

Core owns reusable policy-free runtime mechanisms and the terminal protocol contract. Core does not own host admission, grants, or session policy.

## Other role and surface playbooks and atomic notes loaded

Role and overlay:

- [[planner-playbook]]
- [[botster-planner-playbook]]
- [[botster-architecture]]
- [[cli-patterns]]
- [[spa-patterns]]
- [[botster-runtime-reviewer-playbook]]
- [[project-pipelines-playbook]] — workflow-only. This ticket records pipeline artifacts, gates, and checklists. It does not change Project Pipelines plugin source.
- [[project pipeline orchestration belongs in a device-level botster plugin]]
- [[project pipelines needs an operator workbench not more primitives]]
- [[project pipelines ui contract belongs in the plugin readme]]
- [[plan review must verify a plan artifact exists before trusting gate summaries]]
- [[project pipelines mcp create calls can time out after committing]]
- [[botster orchestration should spawn agents with explicit target ids]]
- [[botster orchestration prompts must bind agents to explicit worktrees]]
- [[plan steps need reviewable plan artifacts]]
- [[plan agents must author vault context as wikilinks not home paths]]
- [[pipeline vault checklists must cite exact resolvable note titles]]
- [[vault example paths are not repository placement conventions]]
- [[botster core historical plan docs should not sit beside living architecture]]
- [[botster-core uses CI-owned Cargo commands because it has no test script]]

Targeted atomic notes:

- [[Core ClientWorker bind requires a live attach generation]]
- [[Core terminal subscription ownership is session, subscription, and generation]]
- [[terminal subscription lifecycle is Core owned while host session policy is Hub owned]]
- [[Core reports terminal mechanism capabilities and Hub admits their use]]
- [[Core terminal protocol separates Hub-safe envelopes from client semantic bodies]]
- [[botster terminal v1 starts at protocol 1 and conformance revision 1]]
- [[public protocol versions host control and Core terminal planes independently]]
- [[Core owns the incremental attach phase machine]]
- [[botster core contract surface needs consumer proof]]
- [[botster core ui and capability contracts must avoid product gravity]]
- [[botster core public enums are breaking until non exhaustive is decided]]
- [[public dto field additions are source breaking without non exhaustive]]
- [[additive daemon capabilities do not raise the default client requirement]]
- [[ready then history is advertised as optional daemon support]]
- [[proposed Hub admission binds adapters with negotiated subscription capabilities]]
- [[Hub embeds CoreDaemon behind one client admission point]]

Not loaded, with reason:

- [[botster runtime teardown lenses]] — this ticket is not teardown class.
- Hub, Web, TUI, TUI Kit, Ghostty, and Workspaces charters — this run targets `botster-core` only.

## Context loaded

- Ticket text and Hub parent finding `finding_1786682759_137158`.
- Human answer 1B: Core owns terminal capability tokens. Hub may compute the intersection. Core must receive the resulting tokens because Core produces the stream. Core must not store host grants.
- Human answer `question_1786683571_824777`: omission is unrepresentable at the Core bind call. A required argument is enough. An empty set is valid when negotiation produces no optional capabilities. Admission constructs the value before bind. Malformed or unsupported values belong at the external negotiation or type-construction boundary.
- Shipped Core bind at `ce7474a` accepts client, session, subscription, generation, and adapter only.
- `list_terminal_subscriptions` reports identity, generation, and `adapter_bound` only.
- `botster-terminal-protocol` already owns feature tokens `terminal_streaming`, `resize`, and `snapshot_delivery=ready_then_history`.
- `ready_then_history` is optional advertised support. It is not in the default client requirement.
- ClientWorker encodes bound-route frames in `encode_terminal_frame` after bind.
- Living docs live under `docs/architecture/`. This plan stays under `docs/archive/plans/`.
- Repo gates remain the vault-owned Cargo commands plus `script/terminal-protocol-node-smoke.sh`.
- `.gitignore` is present and non-empty. The worktree path has no `:`.
- Existing Plan artifact: `artifact_1786683335_768743`.
- Existing ticket vault checklist: `checklist_1786683323_490735`. This visit does not create another checklist.

## Scope

1. Add one Hub-safe opaque capability-set type to `botster-terminal-protocol`. Empty sets are valid.
2. Require that type on `bind_terminal_adapter` together with the adapter and live generation. Do not use `Option`.
3. Persist the set on the live `SubscriptionOwner`. The set is immutable after bind.
4. Expose the bound set from `list_terminal_subscriptions`, including an empty bound set.
5. Use the bound set when ClientWorker encodes optional snapshot delivery.
6. Keep host grants and host session policy out of Core.
7. Update daemon bind, engine facades, in-repo tests, the published adapter harness callers, and the Hub-shaped consumer.
8. Update living architecture docs that describe bind and inventory.
9. Merge directly into `main`. Do not create a pull request.

## Non-scope

- Hub Unix or WebRTC adapter implementation.
- Host grant objects, BootstrapGrant, or Unix admission policy.
- Changing `TerminalCompatibilityRequirement::current()`.
- A second snapshot delivery mode or a new attach phase machine.
- Removing the unbound `TransportEgress` drain path.
- Teardown, detach, `Closed`, generation, or queue-budget changes.
- Plugin `CapabilitySet` or UI `UiCapabilitySet`.
- `EngineCommand` bind variant.
- New `BindTerminalAdapterError` variants.
- Project Pipelines plugin source.

## Repository ownership boundaries and cross-repo dependencies

Core owns:

- Terminal feature tokens and the opaque capability-set type.
- Bind API, live subscription state, and control-plane inventory.
- Stream encoding for bound adapters.

Hub owns:

- Admission, host grants, and the intersection of Core-reported facts with those grants.
- Construction of the negotiated set before bind.

Hub must not store the negotiated terminal set only on a route record. Hub must not decode Snapshot bodies to learn tokens.

Cross-repo:

- Hub ticket `ticket_1786661008_634435` already depends on this ticket (`dependency_1786682905_577001`).
- This ticket has no outbound repository dependency.
- Do not edit `botster-hub` in this run.

## Product decision ledger

Defaults:

- Bind takes `TerminalCapabilitySet` by value. Omission does not compile. There is no `Option` and no `MissingCapabilities` error.
- An empty set is a valid negotiated result. `from_tokens([])` and `empty()` succeed.
- Unknown or malformed tokens fail at `TerminalCapabilitySet` construction, not at Core bind.
- Core bind does not add `UnsupportedCapabilities`. Existing bind errors stay `BindBeforeAttach`, `UnknownSubscription`, `StaleGeneration`, and `AlreadyBound`.
- A later bind on the same live generation returns `AlreadyBound`. Core does not replace the adapter or the set.
- Unbound inventory rows report `capabilities: None`. Bound rows report `Some(stored set)`, including `Some` empty.
- Empty set means no optional capabilities. Baseline live `TerminalOutput` and `Scrollback` still encode.
- Encode Snapshot only when the stored set contains `snapshot_delivery=ready_then_history`.
- Always encode `ProcessExit` and `AttachState` except `Detached`.
- `resize` and `terminal_streaming` may appear in the set. They do not change this ticket's encode gates. They remain protocol tokens for Hub negotiation.
- Skipped Snapshot frames are intentional omissions. They are not lost-snapshot failures and they do not return on drain after bind.

Non-goals:

- Host policy in Core.
- Dual production paths.
- Raising the default client requirement.
- A public bind-error enum break.

Follow-up-ok:

- Hub Unix ticket computes the intersection and passes a possibly empty set after this ticket closes.

Ask-human threshold:

- None remaining. Answers 1B and `question_1786683571_824777` lock the product fork.

## Assumptions and unknowns

Assumptions:

- Option 1B remains the ownership decision.
- "Bind without a capability set is a typed error once the API ships" means a required Rust argument.
- "AlreadyBound or a typed mismatch" is satisfied by `AlreadyBound` on any second bind of the live generation.
- Core-reported mechanism facts are `TerminalCompatibility::current().features`.
- The Hub-shaped consumer is the required downstream-shaped proof.
- Adding a public bind argument, an inventory field, and a protocol type is an accepted `0.1.0` source break. Adding a bind-error variant is not.
- Session-type eligibility parent pins do not apply.

Unknowns:

- Exact Hub grant-token list for Unix is Hub-owned. Core must accept any constructed set, including empty.

## Affected surfaces and files

Protocol contract:

- `crates/botster-terminal-protocol/src/lib.rs`
- `crates/botster-terminal-protocol/src/compatibility.rs` or a sibling `capabilities.rs`
- `crates/botster-terminal-protocol/tests/public_api.rs`
- `crates/botster-terminal-protocol/tests/compatibility.rs`
- `crates/botster-terminal-protocol/tests/hub_shaped.rs`
- Generated TypeScript and node smoke only if existing drift tests require a new export. Do not add a browser inventory DTO.

Core bind and inventory:

- `crates/botster-core/src/contract/terminal_subscription.rs`
- `crates/botster-core/src/engine/client_worker.rs`
- `crates/botster-core/src/engine/managed_session_runtime.rs`
- `crates/botster-core/src/engine/botster.rs`
- `crates/botster-core/src/lib.rs` re-exports
- `crates/botster-core-daemon/src/daemon.rs`
- `crates/botster-core-daemon/src/lib.rs` if the new type is re-exported

Tests and consumer proof:

- `crates/botster-core/tests/client_worker_engine_test.rs`
- `crates/botster-core-daemon/tests/daemon_integration_test.rs`
- `crates/botster-core-test-support/tests/consumers/hub-adapter-shaped/src/lib.rs`

Living docs:

- `docs/architecture/client-worker-terminal-egress.md`
- `docs/architecture/terminal-protocol.md`
- `docs/architecture/engine-command-surface.md`
- `docs/architecture/terminal-adapter.md` if it still describes bind inputs

## Implementation plan

### 1. Protocol type

Add `TerminalCapabilitySet` to `botster-terminal-protocol`.

- Store unique tokens in a private ordered set.
- Construct with `from_tokens` and `empty()`.
- Accept an empty token list.
- Reject unknown tokens against the advertised feature inventory at construction.
- Expose `contains`, `is_empty`, ordered token iteration, and equality.
- Do not store host grants, protocol version, or conformance revision.
- Do not reuse plugin `CapabilitySet`.
- Add the new public names to `PUBLIC_API_ALLOWLIST`.
- Prove `TerminalCompatibilityRequirement::current()` still omits `snapshot_delivery=ready_then_history`.

Hub can build this type from token strings, including an empty intersection.

### 2. Bind API

Change every `bind_terminal_adapter` to:

```text
bind_terminal_adapter(
    client_id,
    session_id,
    subscription_id,
    generation,
    capabilities,
    adapter,
)
```

Keep existing generation checks. Then:

1. If the live owner already has an adapter, close the presented adapter and return `AlreadyBound`.
2. Store the adapter and the set together. Store an empty set as empty. Do not coerce it to `None`.
3. Do not inspect token contents at bind.
4. Do not provide a setter.

Do not add `UnsupportedCapabilities` or `MissingCapabilities`.

### 3. Inventory

Add `capabilities: Option<TerminalCapabilitySet>` to `TerminalSubscriptionRecord`.

- Unbound rows: `adapter_bound = false`, `capabilities = None`.
- Bound rows: `adapter_bound = true`, `capabilities = Some(stored set)`.
- A bound empty set round-trips as `Some` empty, not `None`.
- Do not add phase, snapshot, or queue fields.

This field addition is an accepted `0.1.0` struct-literal break. Do not mark the record `non_exhaustive` in this ticket.

### 4. Stream production

In `ingest_bound_terminal_frames` / `encode_terminal_frame`, read the owner's stored set:

- Snapshot → encode only with `snapshot_delivery=ready_then_history`.
- TerminalOutput and Scrollback → encode for every bound adapter, including an empty set.
- ProcessExit and AttachState → encode as today.
- Clear unused snapshot-phase notes when a Snapshot is skipped.
- Do not fail the subscription for a skipped unauthorized Snapshot.
- Do not change worker attach phases.

Empty-set stream semantics: live output and process-exit still flow. Incremental Snapshot tags do not.

### 5. Consumer proof

Extend the Hub-shaped consumer so it:

1. Builds an opaque `TerminalCapabilitySet` from protocol tokens, including an empty set.
2. Binds through the public Core API.
3. Reads the same tokens from `list_terminal_subscriptions`.
4. Observes opaque live frames without decoding GHOSTSNP.

Required proofs:

- Empty-set bind succeeds.
- Empty-set inventory reports `adapter_bound=true` and an empty token list.
- Empty-set stream emits live `terminal_output` and does not emit snapshot event tags.
- Optional-token bind that includes `snapshot_delivery=ready_then_history` still produces the current incremental snapshot plus live output.
- The published adapter harness still passes.

### 6. Docs

Update living architecture. Do not write a second plan under `docs/plans/`.

## Risks

- Source break: Hub and in-repo bind call sites will not compile until they pass a set. That is the point of this dependency.
- Empty-set over-gating: do not treat empty as "no stream". Baseline live output must still encode.
- Snapshot skip must not trip lost-snapshot teardown.
- Product gravity: do not store grants or policy objects on the owner.
- Construction-boundary drift: unknown-token rejection belongs on `from_tokens`, not on bind.
- Accidental default-requirement bump: protocol tests must keep `ready_then_history` optional.

## Acceptance checks and tests

Repository gates (vault-authorized):

```bash
cargo fmt --all -- --check
BOTSTER_ENV=test cargo clippy --workspace --all-targets -- -D warnings
BOTSTER_ENV=test cargo test --workspace
BOTSTER_ENV=test cargo test --doc --workspace
script/terminal-protocol-node-smoke.sh
```

Focused proofs:

- Bind without a `TerminalCapabilitySet` argument does not compile.
- Empty `TerminalCapabilitySet` constructs and binds.
- Unknown tokens fail at construction, not at `bind_terminal_adapter`.
- `BindTerminalAdapterError` gains no new variant.
- Bind before attach, stale generation, and unknown subscription stay typed errors.
- Second bind of the live generation returns `AlreadyBound` even when the new set differs, including empty versus non-empty.
- Unbound inventory reports identity, generation, `adapter_bound=false`, and `capabilities=None`.
- Bound empty inventory reports the same empty set. It does not report READY, PAGE, FINISH, snapshot bytes, or queue state.
- Hub-shaped consumer binds an opaque empty set and a non-empty set, and observes the same tokens without decoding Snapshot bodies.
- Empty-set bind produces live output and does not emit snapshot event tags.
- Ready-then-history bind still produces incremental snapshot plus live output.
- `TerminalCompatibilityRequirement::current()` still requires only `terminal_streaming` and `resize`.
- Published adapter harness still passes.
- Daemon bind tests pass through `CoreDaemon::bind_terminal_adapter`.

Downstream proof required by [[botster core contract surface needs consumer proof]]:

- The isolated Hub-shaped consumer is the in-repo proof.
- Live Hub Unix admission stays on `ticket_1786661008_634435` after this ticket closes.

Production entry points:

- `CoreDaemon::bind_terminal_adapter`
- `DefaultBotsterEngine::bind_terminal_adapter`
- ClientWorker `ingest_bound_terminal_frames` plus host `drain` / `drain_runtime_once`

This ticket is not scaffold-only.

Merge policy: merge directly into `main`. Do not create a PR.

## Vault gaps worth capturing

After implement, capture one convention:

- Core bind stores one immutable negotiated terminal capability set, including empty, on the live subscription and exposes that set from inventory.

Then link it from [[Core ClientWorker bind requires a live attach generation]] and [[Core reports terminal mechanism capabilities and Hub admits their use]].

Keep [[proposed Hub admission binds adapters with negotiated subscription capabilities]] proposed until the Hub Unix ticket ships the admission side.

## Worktree hygiene

- Tracked `.gitignore` has content. Do not restore or truncate it.
- Worktree path has no `:`. Do not set `CARGO_TARGET_DIR`.
- Agents must use the vault-owned Cargo commands. Do not invent a test wrapper.

## Session-type eligibility parent

Not applicable. This ticket is not a consumer of Hub session-type eligibility work.

## Vault checklist

This Plan visit reuses ticket checklist `checklist_1786683323_490735`. A second vault checklist would be a duplicate. Skip reason: the ticket already has one Plan vault checklist from the first Plan visit.
