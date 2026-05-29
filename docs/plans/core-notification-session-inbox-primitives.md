# Implement Core Notification And Session Inbox Primitives

## Context Loaded

- Pipeline context: ticket `ticket_1780075966_813073`, run `run_1780077471_707304`, step `botster_plan`, gate `botster_plan_gate`.
- Orchestrator correction received via Botster inbox: this run is main-rooted; the stale `base_run_id` and `base_ticket_id` values in current context should not make this a stacked PR. Target `main`.
- Worktree: `/Users/jasonconigliari/botster-sessions/trybotster-botster-core-project-pipelines-ticket_1780075966_813073`.
- Target: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.
- Required playbooks loaded:
  - `/Users/jasonconigliari/knowledge/notes/planner-playbook.md`
  - `/Users/jasonconigliari/knowledge/notes/botster-planner-playbook.md`
- Additional vault constraints loaded:
  - `/Users/jasonconigliari/knowledge/self/identity.md`
  - `/Users/jasonconigliari/knowledge/self/goals.md`
  - `/Users/jasonconigliari/knowledge/notes/botster-architecture.md`
  - `/Users/jasonconigliari/knowledge/notes/cli-patterns.md`
  - `/Users/jasonconigliari/knowledge/notes/spa-patterns.md`
  - `/Users/jasonconigliari/knowledge/notes/project pipeline orchestration belongs in a device-level botster plugin.md`
  - `/Users/jasonconigliari/knowledge/notes/project pipelines needs an operator workbench not more primitives.md`
  - `/Users/jasonconigliari/knowledge/notes/project pipelines ui contract belongs in the plugin readme.md`
  - `/Users/jasonconigliari/knowledge/notes/botster orchestration should spawn agents with explicit target ids.md`
  - `/Users/jasonconigliari/knowledge/notes/botster orchestration prompts must bind agents to explicit worktrees.md`
- Repo context loaded:
  - `README.md`: core owns reusable mechanisms and transport-neutral contracts; hub owns runtime policy, lifecycle, routing, recovery, extension supervision, and product workflows.
  - `crates/botster-core/src/contract/session_protocol.rs`: already has terminal OSC `NotificationPayload`; it is not a session inbox.
  - `crates/botster-core/src/contract/actor.rs`: current actor contracts include session I/O notifications and plugin notification handler refs, but no general inbox model.
  - `crates/botster-core/src/contract/session.rs`: currently only `SessionId`, `SubscriptionId`, and `RequestId`.
  - `crates/botster-core/src/contract/client.rs`: currently client identity/scope/state only.
  - `crates/botster-core/src/contract/transport.rs`: transport ingress/egress frames, no concrete doorbell or inbox transport policy.
  - Existing tests under `crates/botster-core/tests/*`: contract tests use public type instantiation and serde round trips.
- Project Pipelines checklist discipline:
  - `project_pipelines_checklist_instructions` loaded.
  - Creating the run-level vault checklist failed with Project Pipelines SQLite `database is locked`; per the loaded Project Pipelines lock incident guidance, preserve checklist evidence in this plan and gate evidence, avoid retry storms, and report the write failure.

## Scope

Build a core-only notification and session inbox contract slice in `crates/botster-core`.

In scope:

- Public typed notification payloads that represent structured messages and attention-only notifications.
- Optional notification actions as typed core data, not product-specific commands.
- Severity/source metadata.
- Expiry semantics represented by deterministic, testable timestamps or TTL/expiration fields.
- Session-scoped and client-scoped inbox addresses.
- Inbox item state needed for delivery status and drain-on-receive behavior.
- A small pure inbox model or helper that can post, notify-only, expire, drain, and report delivery status without transport or persistence.
- Public exports from `crates/botster-core/src/lib.rs` and `crates/botster-core/src/contract/mod.rs`.
- Deterministic tests proving post-message, notify-only, expiry, drain-on-receive, delivery status, and session/client scoping.
- `BoundaryJson` only for explicitly owner-classified extension payloads whose schema is not core-owned.

Non-scope:

- PTY doorbells, WebRTC/DataChannel/socket/TUI/ActionCable transport wiring.
- Hub auth, capability checks, admission policy, client authorization, or session lifecycle decisions.
- Persistence policy, database schema, retention jobs, durable queues, or Project Pipelines plugin storage.
- UI presentation, browser/TUI components, notification badges, or operator workbench behavior.
- Product-specific message types such as Project Pipelines questions, findings, gate prompts, or MCP messages.
- Botster MCP implementation details or any dependency on Project Pipelines internals.
- Copying old `trybotster` paths into this repo. Old paths are evidence only and are not required to exist.

## Botster Layers Touched

- Core contract layer: yes, primary surface.
- Hub/session/client worker layers: not implemented here; they are downstream consumers of these core contracts.
- Transport adapters: not implemented here; doorbells and delivery mechanics remain host-owned.
- Plugin, MCP, TUI, React SPA, Rails relay: not changed.

## Surface Inventory

Expected affected files:

- `crates/botster-core/src/contract/notification.rs` or `crates/botster-core/src/contract/inbox.rs`: new core-owned types and pure inbox behavior.
- `crates/botster-core/src/contract/mod.rs`: export the new module and public types.
- `crates/botster-core/src/lib.rs`: re-export the public core types.
- `crates/botster-core/tests/notification_inbox_test.rs`: acceptance tests for the new primitive.
- This plan file.

Possible but avoid unless needed:

- `crates/botster-core/src/contract/session.rs`: only if the implementer chooses to place session/client inbox address types beside existing identifier types.
- `crates/botster-core/src/contract/client.rs`: only if client-owned address helpers are clearer there.
- `crates/botster-core/src/contract/boundary.rs`: only if a new `BoundaryJson` owner/reason classification must be documented in tests.
- `README.md`: only if the new public surface changes the ownership-boundary table materially.

Not expected:

- `Cargo.toml`: existing `serde` and `serde_json` should be enough.
- `crates/botster-core/src/contract/transport.rs`: no doorbell transport policy in core.
- Runtime, plugin, MCP, Rails, React, TUI, or old trybotster files.

## Proposed Contract Shape

Prefer a new focused module, `contract::notification`, to avoid overloading terminal OSC `NotificationPayload` from `session_protocol`.

Core types should cover:

- `NotificationId` and optional `NotificationThreadId` or `reply_to` using existing `RequestId` only if the semantics match; do not force request correlation onto inbox identity if it obscures meaning.
- `NotificationTarget` with typed variants for `Session(SessionId)` and `Client(ClientId)`.
- `NotificationKind` or equivalent for `Message` and `NotifyOnly`.
- `NotificationSeverity` with a small stable vocabulary such as `Info`, `Success`, `Warning`, `Error`, and `Attention`.
- `NotificationSource` as typed metadata with a stable source label and optional owner/plugin key if needed; avoid product-specific enum variants.
- `NotificationAction` with typed action id/label/hint and optional owner-classified extension payload.
- `NotificationContent` or `NotificationPayload` for title/body plus structured extension fields. Stable core fields should be typed; only extension-owned payloads may use `BoundaryJson`.
- `NotificationExpiry` or `expires_at` field and a deterministic clock input for tests.
- `NotificationDeliveryStatus`, with statuses sufficient to prove queued, delivered/drained, expired, and dropped or acknowledged states.
- `SessionInbox` or a pure `NotificationInbox` model keyed by `NotificationTarget`, with methods equivalent to `post`, `notify_only`, `receive/drain`, `expire`, and `status`.

The pure inbox model is acceptable in core because the ticket asks for drain behavior and delivery status primitives. It must remain in-memory and policy-free: no background tasks, persistence, auth, transport, or UI decisions.

## BoundaryJson Classification

Use `BoundaryJson` only when the payload schema is explicitly outside core ownership. Acceptable examples:

- Extension-owned fields on `NotificationAction`.
- Extension-owned structured body data for plugin/provider-specific rendering.

Do not use `BoundaryJson` for stable fields that the ticket explicitly asks core to own: kind, target, severity, source label, actions list shape, expiry, delivery status, and drain behavior.

If a new `BoundaryJson` use is added, add a test-side inventory with owner and reason metadata, following the existing `actor_contract_test.rs` pattern.

## Assumptions And Unknowns

Assumptions:

- This ticket is intentionally scaffold/core-primitive work. The production user path changed by this ticket is downstream compile-time consumption of exported core types and deterministic inbox behavior, not an immediately wired hub doorbell.
- No PII fields are required. Tests should use synthetic ids and generic content.
- Time can be represented as deterministic integer timestamps or `std::time::Duration`/seconds rather than adding a time crate.
- `NotifyOnly` means an attention event that can have title/body/action metadata and status, but should not create a generic message body for inbox readers beyond the notification envelope.
- `PostMessage` means durable-until-drained-or-expired inbox item within the pure model, not persisted storage.

Unknowns for implementation:

- Whether to name the module `notification` or `inbox`. Prefer `notification` if it contains both payload and inbox types; prefer `inbox` only if the public vocabulary centers on the inbox model.
- Whether `NotificationId` should be a plain string newtype or reuse `RequestId`. A distinct newtype is clearer unless existing request correlation is intentionally reused.
- Whether `NotificationAction` should borrow `UiActionId` from the UI contract. Prefer a small notification-specific action id if importing UI vocabulary would imply presentation ownership.

No human question is required before implementation because the ticket is explicit that core owns primitives while hub/hosts own delivery, auth, persistence, and UI.

## Risks

- Overreaching into transport doorbells would violate the core/hub/client boundary.
- Treating Project Pipelines notifications as the model would leak product workflow policy into core.
- Using raw `serde_json::Value` broadly would violate the `BoundaryJson` owner-classification convention and undercut typed payload acceptance.
- A pure inbox helper can accidentally become persistence or lifecycle policy if it owns background cleanup or auth decisions.
- Reusing terminal OSC `NotificationPayload` for inbox messages would conflate PTY-emitted notifications with client/session inbox primitives.
- Missing exports would make tests pass locally while downstream crates cannot consume the new primitive.

## Acceptance Checks And Tests

Run:

- `cargo fmt`
- `cargo test -p botster-core notification_inbox`
- `cargo test -p botster-core`

Add named tests that map directly to ticket acceptance:

1. `post_message_queues_structured_session_message`
   - Posts a message to `NotificationTarget::Session`.
   - Asserts typed title/body/source/severity/actions survive a serde round trip.
   - Asserts delivery status starts as queued or pending.

2. `notify_only_records_attention_without_generic_message_body`
   - Creates a notify-only item.
   - Asserts kind is `NotifyOnly`, target is typed, actions are optional, and no product-specific message type is required.

3. `expired_items_are_not_drained_as_deliverable`
   - Inserts one expired item and one live item with deterministic timestamps.
   - Drains at a fixed timestamp.
   - Asserts the expired item is marked expired and only the live item is delivered.

4. `receive_drains_target_inbox_once`
   - Posts multiple items for one target.
   - Drains that target and asserts items are returned in deterministic order.
   - Drains the same target again and asserts no duplicate delivery.

5. `delivery_status_tracks_post_deliver_expire`
   - Verifies status transitions for queued/pending, delivered/drained, and expired paths without a transport adapter.

6. `session_and_client_scopes_are_isolated`
   - Posts items to a session target and a client target.
   - Drains each target independently.
   - Asserts no cross-target leakage.

7. `boundary_json_is_limited_to_extension_payloads`
   - Asserts core-owned fields are typed.
   - If extension payload support exists, asserts it uses `BoundaryJson` and is classified by owner/reason in the test.

Runtime path evidence to preserve:

- Document in the implementer artifact that this is scaffold/core-primitive work by design.
- Prove the changed path through exported public core types plus tests that instantiate the pure inbox behavior through the public API.
- Do not claim hub/client delivery changed until a later host-wiring ticket actually consumes the primitive.

## Pipeline Gates And Artifacts

- Plan gate artifact: this file plus gate evidence summarizing context, scope, assumptions, affected surfaces, risks, acceptance checks, and vault gaps.
- Checklist evidence: Project Pipelines checklist write failed due SQLite lock; this plan records notes read, no convention conflict, expected verification commands, and no durable vault capture needed yet.
- Advancement target: `botster_plan_review`.

## Vault Gaps Worth Capturing

No durable vault capture is required before implementation. Existing vault notes already constrain the relevant boundaries:

- core versus hub/product policy
- session/client data-plane ownership
- `BoundaryJson` owner/reason classification
- Project Pipelines as plugin-owned workflow policy
- plan artifacts as reviewable repo documents

Capture later only if implementation discovers a reusable rule about the difference between terminal OSC notifications and session inbox notifications that is not already covered in the Botster architecture notes.
