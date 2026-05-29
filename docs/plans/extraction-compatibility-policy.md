# Extraction Compatibility Policy Plan

Ticket: Define compatibility policy for extraction
Run: run_1780018172_144280

## Context Loaded

- Pipeline context: ticket `ticket_1780014900_475830`, run `run_1780018172_144280`, current step `botster_plan`, gate `botster_plan_gate`, prior Plan Review findings, and no open questions.
- Required playbooks: `planner-playbook`, `botster-planner-playbook`.
- Review-required notes: `plan steps need reviewable plan artifacts`, `botster packages should enforce core hub cli plugin provider boundaries`, `botster terminal forwarder terminology is deprecated`, `botster terminal egress is session backed only`, `botster lua terminal apis expose subscriptions instead of pty forwarders`, `botster runtime artifact resolution should be read only`.
- Supporting notes: `botster-architecture`, `cli-patterns`, `spa-patterns`, `project pipeline orchestration belongs in a device-level botster plugin`, `project pipelines needs an operator workbench not more primitives`, `project pipelines ui contract belongs in the plugin readme`, `botster orchestration should spawn agents with explicit target ids`, `botster orchestration prompts must bind agents to explicit worktrees`, `identity`, `goals`.
- Repo context: `README.md` is the public crate boundary document; `src/boundary.rs` owns current layer responsibility text; `tests/boundary_test.rs` owns current boundary guardrails. There is no existing compatibility policy.

## Scope

- Add a public extraction compatibility policy for `botster-core`.
- The policy vocabulary is exactly: `preserve`, `translate`, `drop`.
- Classify all seven ticket-named paths with a verdict, rationale, and enforcement path.
- Encode that there is no defer bucket. Future tickets may delete old expectations or exclude out-of-scope behavior instead of preserving accidental coupling.
- Add narrow guardrail/doc tests where practical so the policy is inspectable and fails when required policy language disappears.

## Non-Scope

- No blanket legacy migration.
- No runtime compatibility shims for legacy paths.
- No migration code for `context.json`, legacy repo-cwd hub identity, forwarders, browser plugin stores, direct snapshot helpers, hub-owned PTY relays, or product-specific UI refresh behavior.
- No product workflow policy in `botster-core`.
- No broad refactor of transport, entity, session, package, UI, or client contracts.

## Compatibility Verdicts

| Path | Verdict | Rationale | Enforcement |
| --- | --- | --- | --- |
| Transport-neutral identifiers, frames, entity frames, UI contract shapes, package manifests, capabilities, extension metadata, and narrow crypto/identity operation contracts | preserve | These are already the reusable cross-client contracts `botster-core` exists to carry. Removing them would break the extracted crate's purpose. | `tests/boundary_test.rs` should assert README/rustdoc policy names preserved core contract families. Existing boundary tests should continue asserting core owns reusable mechanisms and hub/CLI do not. |
| `context.json` migration | drop | This is a legacy migration concern, not a reusable core mechanism. It belongs to hub/CLI migration policy if still needed in the main app. | Policy/doc guardrail asserts this path appears in the excluded-path table with verdict `drop`; no `context.json` parsing or migration module should be added to `botster-core`. |
| Legacy repo-cwd hub identity | drop | Device hub identity and spawn targets must not be derived from ambient repo cwd. Core may expose identity operation contracts, but cwd-derived hub identity is old hub policy. | Policy/doc guardrail asserts this path has verdict `drop`; boundary tests should continue keeping executable startup and hub identity policy outside core. |
| Old forwarder terminology | translate | Current terminal client data-plane language is subscription-based. Historical names should be translated to terminal subscription concepts when they represent current contracts, not preserved as public core vocabulary. | Policy/doc guardrail asserts `old forwarder terminology` has verdict `translate`; implementation tests should assert the policy mentions subscription language and should not add public `PtyForwarder`, `StopForwarder`, or `create_pty_forwarder` API names to core. |
| Browser-only plugin stores | drop | Browser-only persistence is client/product behavior. Plugin-owned dynamic state should flow through namespaced entity frames and plugin/runtime storage contracts, not a browser-only store in core. | Policy/doc guardrail asserts this path has verdict `drop`; no browser-specific store module should be introduced in `botster-core`. |
| Direct snapshot helpers | translate | Snapshot/page payloads can be preserved as transport-neutral contract shapes, but direct helper calls that bypass SessionIo/ClientWorker ownership are legacy implementation mechanics. | Policy/doc guardrail asserts direct snapshot helpers have verdict `translate`; tests should distinguish allowed `TransportEgress::Snapshot` contract language from disallowed direct helper APIs such as `snapshot_and_subscribe`. |
| Hub-owned PTY relays | drop | Current terminal egress is session-backed through SessionIo and ClientWorker actors. The hub owns attach policy and cleanup, not byte delivery. | Policy/doc guardrail asserts this path has verdict `drop`; no hub-owned relay API or module should be added to `botster-core`. |
| Product-specific UI refresh behavior | drop | Product-specific refresh behavior belongs in clients, plugins, or hub policy. Core can preserve UI/entity contract shapes only. | Policy/doc guardrail asserts this path has verdict `drop`; no Project Pipelines, browser refresh, Rails, ActionCable, or product UI workflow language should enter core contracts. |

## Implementation Plan

1. Put the durable policy in `README.md` under a new `## Extraction Compatibility Policy` section near the existing boundary/migration text.
2. Keep `README.md` as the production/user path because it is the crate readme and public boundary document. If implementation also adds rustdoc text, it should mirror the README rather than create a second policy source.
3. Update `tests/boundary_test.rs` with targeted doc guardrails that read `README.md` and assert:
   - the policy uses the exact decision vocabulary `preserve`, `translate`, and `drop`;
   - the policy says there is no defer bucket;
   - the policy grants delete/exclude permission for out-of-scope old expectations;
   - each of the seven named paths appears with its required verdict;
   - preserved core contract families are named so the policy is not only exclusions;
   - old forwarder terminology is translated to terminal subscriptions;
   - direct snapshot helpers are distinguished from allowed transport-neutral snapshot contracts.
4. Keep any typed Rust API optional. Add one only if doc assertions become too brittle. Do not introduce a runtime compatibility layer just to make the policy testable.

## Assumptions And Unknowns

- Assumption: a README-backed policy plus doc guardrail tests satisfies "document and enforce" for this extracted crate because the ticket is policy-focused and no runtime behavior currently exists for the legacy paths.
- Assumption: this is scaffold/documentation work for `botster-core`; the actual production path changed is public crate guidance plus tests that constrain future crate changes.
- Assumption: `botster-core` should not know about Project Pipelines, Rails, ActionCable, or browser-specific refresh flows except as exclusions.
- Unknown: whether future package splitting will move some currently preserved contract families into narrower crates. This plan should not pre-design that split.

## Affected Surfaces And Files

- `README.md`: add public policy and verdict table.
- `tests/boundary_test.rs`: add doc guardrail tests with exact assertions.
- Optional `src/lib.rs` or `src/boundary.rs`: only if the implementer decides rustdoc needs a small public policy pointer.

## Risks

- A prose-only policy could be ignored. Mitigation: tests assert the public policy text and per-path verdicts.
- A runtime shim would preserve accidental coupling. Mitigation: scope explicitly forbids migration behavior and compatibility adapters.
- Snapshot wording can be over-banned. Mitigation: plan preserves transport-neutral snapshot contracts while translating/dropping direct helper mechanics.
- Forwarder wording can be over-banned. Mitigation: public terminal client language translates to subscriptions; broker-internal historical or generic stream forwarding remains outside this crate's public policy.
- README tests can become brittle. Mitigation: assert targeted phrases and verdict/path pairs, not full paragraphs.

## Acceptance Checks And Tests

- `cargo test` passes.
- `tests/boundary_test.rs` gains ticket-specific tests, not only the pre-existing two boundary tests.
- Removing `preserve`, `translate`, `drop`, "no defer bucket", or delete/exclude permission from the public policy fails tests.
- Removing any of the seven named path verdicts from the public policy fails tests.
- Adding a public core API named like an old terminal forwarder or direct snapshot helper should be caught either by explicit tests or by review against the policy.
- Manual review confirms every changed line maps to the ticket, required boundary conventions, or guardrail tests.

## Vault Gaps Worth Capturing

No new durable vault gap is known after re-planning. The relevant durable rules already exist in vault notes: core/hub/CLI/plugin boundary enforcement, subscription terminology, session-backed terminal egress, read-only runtime artifact resolution, and reviewable plan artifacts.
