# Codify Entity Frame Semantics

Ticket: `ticket_1780014882_135415`
Run: `run_1780026166_924277`
Step: Plan

## Context Loaded

- Pipeline context: ticket, run, current Plan step, gate prompt, prior Plan Review findings, dependency state, checklist state, and prior gate/review events.
- Required playbooks: `planner-playbook`, `botster-planner-playbook`.
- Botster vault constraints: `botster-architecture`, `cli-patterns`, `spa-patterns`, `botster hub client state sync is entity frame only`, `botster plugin entities are canonical for plugin-owned dynamic state`, `plugin-owned dynamic state uses plugin-namespaced entity frames`, `botster plugin entity hydration has full id and scoped contracts`, `scoped entity snapshots preserve whole-family sequence gates`, `botster entity snapshots are authoritative reconnect baselines`, and `plan steps need reviewable plan artifacts`.
- Repo context: `src/entity.rs` currently contains only basic `EntityKind`, `EntityId`, and four frame variants without `snapshot_seq`, scoped snapshots, validation, or store behavior. `src/lib.rs` re-exports the current entity shell. Current tests are only boundary tests.
- Reference evidence, read-only: `/Users/jasonconigliari/Rails/trybotster/docs/plugin-entities.md`, `/Users/jasonconigliari/Rails/trybotster/docs/webrtc-protocol.md`, `/Users/jasonconigliari/Rails/trybotster/cli/tests/entity_broadcast_test.rs`, and `/Users/jasonconigliari/Rails/trybotster/cli/src/clients/tui/entity_stores/mod.rs`.
- Baseline verification: `cargo test` passes before implementation.

## Scope

Implement the reusable entity frame contract and generic entity store semantics in `botster-core`.

In scope:

- Add the fifth frame type, `entity_scoped_snapshot`.
- Add `snapshot_seq` to all entity frames and encode protocol field names with serde: `type`, `entity_type`, `snapshot_seq`, `items`, `entity`, `patch`, `id`, and `scope`.
- Add a JSON-backed `EntityStore` that stores ordered records by string id and applies all five frame kinds.
- Add an aggregate `EntityStores` keyed by entity type so plugin families can round-trip without per-plugin client code.
- Add validation helpers for built-in reserved entity families, plugin namespaced families, id-field defaults, string ids, and invalid record rejection.
- Add focused tests that map directly to the ticket acceptance criteria and required surfaces.
- Re-export the new public API from `src/lib.rs`.

Non-scope:

- Do not copy old browser or TUI store code.
- Do not move Project Pipelines record meaning into `botster-core`.
- Do not add hub policy, Lua provider registries, broadcaster scheduling, browser React selectors, or TUI renderer bindings.
- Do not retrofit old trybotster paths. They are evidence only.
- Do not add new dependencies. Existing `serde`, `serde_json`, and `thiserror` are sufficient.

## Proposed API Surface

Keep the public surface small and contract-oriented:

- `EntityKind(String)` and `EntityId(String)` remain string wrappers.
- `EntitySequence(u64)` or a plain `u64` field named `snapshot_seq`; prefer a wrapper only if it makes serde/test code clearer without ceremony.
- `EntityFrame` variants:
  - `Snapshot { entity_type, snapshot_seq, items }`
  - `ScopedSnapshot { entity_type, snapshot_seq, scope, items }`
  - `Upsert { entity_type, snapshot_seq, id, entity }`
  - `Patch { entity_type, snapshot_seq, id, patch }`
  - `Remove { entity_type, snapshot_seq, id }`
- `EntityStore`:
  - ordered `Vec<EntityId>` plus `HashMap<EntityId, serde_json::Value>`
  - `snapshot_seq` for whole-family full snapshots and deltas
  - `apply_frame`, `apply_snapshot`, `apply_scoped_snapshot`, `apply_upsert`, `apply_patch`, and `apply_remove`
  - `iter`, `get`, and `field` read helpers only if tests or existing patterns justify them
- `EntityStores`:
  - `HashMap<EntityKind, EntityStore>`
  - applies protocol frames by `entity_type`
  - gives generic access to built-in and plugin families with no plugin-specific registration code
- `EntityContract` or helper functions:
  - built-in reserved family detection
  - plugin type validation for `<plugin>.<type>`
  - owner-plugin namespace match check
  - id-field default resolution
  - record id extraction and invalid-record rejection
- `EntityError` for validation/application errors that callers should be able to inspect.

Frame serde names should match the wire protocol exactly:

```json
{
  "type": "entity_snapshot",
  "entity_type": "session",
  "snapshot_seq": 42,
  "items": []
}
```

## Algorithms

### Sequence Gates

Full-family `entity_snapshot` frames are authoritative baselines:

- Drop a full snapshot only when `snapshot_seq` is lower than the store's current whole-family `snapshot_seq`.
- Accept a full snapshot when `snapshot_seq` is equal to the current whole-family sequence; this is the same-seq resync rule.
- Accept a higher-sequence full snapshot and replace the whole store.
- After applying a full snapshot, set the store's whole-family `snapshot_seq` to the frame sequence.

Deltas are strict:

- `entity_upsert`, `entity_patch`, and `entity_remove` apply only when `snapshot_seq` is greater than the store's whole-family `snapshot_seq`.
- Stale deltas with `snapshot_seq <= current snapshot_seq` are dropped.
- Applied deltas advance the whole-family `snapshot_seq` to the delta sequence.

Scoped snapshots are partial replacement frames:

- Reject empty or non-object scopes as invalid.
- Drop a scoped snapshot only when its `snapshot_seq` is lower than the whole-family `snapshot_seq`.
- Do not apply the strict delta rule to scoped snapshots. A scoped snapshot with sequence `N` must not prevent a same-sequence family delta `N` from applying afterward.
- Remove only existing rows whose top-level fields exactly match the scope, then insert valid replacement rows from `items`.
- Preserve unrelated rows in the same entity family.
- Do not treat scoped snapshots as reconnect baselines.

This matches the browser/TUI parity requirement: both clients need snapshots as reconnect baselines, deltas as ordered mutations, and scoped snapshots as filtered list hydration that cannot poison the whole-family gate.

### Shallow Patch

`entity_patch` must only merge top-level fields:

- The patch payload must be a JSON object.
- For each top-level key in `patch`, replace the existing top-level value.
- Nested objects are replaced wholesale. Do not recursively merge nested maps.
- If the target record is missing, return/apply a no-op without creating a partial record; the next snapshot/upsert reconciles it.

### Namespacing, Reserved Families, And IDs

Reserved built-in families are protocol-owned, not plugin-owned. Include at least:

- `session`
- `workspace`
- `spawn_target`
- `worktree`
- `hub`
- `connection_code`
- `template`
- `session_action`

Plugin entity types must be `<plugin>.<type>` and the prefix must match `owner_plugin` when owner validation is requested. Unreserved non-plugin entity names are invalid.

Default id fields:

- `session` -> `session_uuid`
- `workspace` -> `workspace_id`
- `spawn_target` -> `target_id`
- `worktree` -> `worktree_path`
- `hub` -> `hub_id`
- `connection_code` -> `hub_id`
- plugin families and unspecified built-ins -> `id`

Implementation should confirm whether `template` or `session_action` need a non-`id` default from the reference docs/tests before adding special cases.

Record ids are always stored as `EntityId(String)`. Invalid snapshot/scoped/upsert records with missing or non-string ids are rejected or skipped, never repaired by the client store.

## Production / Consumer Path

`botster-core` is intentionally a substrate crate, so there is no local hub runtime entry point to wire in this ticket. The production path this ticket codifies is:

1. `botster-hub`/Lua publishers emit protocol-shaped `EntityFrame`s for built-in and plugin read models.
2. Browser and TUI clients consume the same `botster-core::EntityFrame` and `botster-core::EntityStore` semantics for model state.
3. Plugin-owned families such as `project-pipelines.ticket` or `kanban.board` flow through `EntityStores` by entity type, not through per-plugin client stores.
4. UI binding/rendering layers read the resulting generic stores; this crate owns the model-state contract, not renderer-specific presentation.

Implementation should prove this path with a public integration test that imports the crate as a downstream consumer would and applies protocol-shaped frames to `EntityStores`. Actual `botster-hub`, browser, and TUI wiring remains a separate migration step because those crates are not present in this checkout.

## Browser / TUI Parity

The canonical contract must serve both clients:

- Browser stores rely on serde-compatible wire names, generic plugin family keys, shallow patch semantics, and scoped snapshots that trigger filtered list hydration without full-family replacement.
- TUI stores rely on the same ordering, id extraction, sequence gates, same-seq snapshot resync, and plugin family fallback to `id`.
- Neither client should need per-plugin code for a new `<plugin>.<type>` family. The only plugin-specific state is the record schema carried in JSON.

## Affected Surfaces / Files

- `src/entity.rs`: primary implementation.
- `src/lib.rs`: public exports.
- `tests/entity_test.rs`: downstream-style public API tests and acceptance matrix.
- `README.md`: only update if the final API needs a short contract mention; avoid broad documentation churn.
- `docs/plans/codify-entity-frame-semantics.md`: this plan artifact.

## Assumptions And Unknowns

Assumptions:

- The current `EntityFrame` shell is early extraction scaffolding, so replacing it with the protocol-correct shape is acceptable.
- `botster-core` owns reusable mechanisms and contracts only; hub/plugin policy stays out.
- Invalid records should not be repaired by the client store.
- The old trybotster implementation is evidence for semantics, not source material to copy.

Unknowns to resolve during implementation:

- Whether `template` and `session_action` need built-in id defaults other than `id`.
- Whether an `EntitySequence` wrapper improves API clarity enough to justify the extra type.
- Whether `apply_frame` should return a compact enum such as `Applied`, `DroppedStale`, `RejectedInvalid`, or just `Result<bool, EntityError>`. Prefer the smallest inspectable result that tests can assert without overdesigning.

## Risks

- Treating scoped snapshots like full baselines would block valid same-seq deltas and break filtered hydration.
- Accepting plugin records without string ids would hide producer bugs and force renderer-specific repair logic.
- Overfitting to the old TUI module would leak client presentation behavior into core.
- Keeping old `EntityFrame` variant field names would make the crate diverge from the actual wire protocol.
- Weak tests could pass code shape without proving client behavior.

## Acceptance To Test Matrix

Add named tests covering each criterion:

| Requirement | Test name |
| --- | --- |
| `entity_snapshot` serde wire shape | `entity_frame_snapshot_round_trips_wire_shape` |
| `entity_scoped_snapshot` serde wire shape | `entity_frame_scoped_snapshot_round_trips_wire_shape` |
| `entity_upsert`, `entity_patch`, `entity_remove` serde wire shape | `entity_delta_frames_round_trip_wire_shape` |
| stale deltas drop after newer snapshots | `stale_delta_after_newer_snapshot_is_dropped` |
| same-seq snapshots resync | `same_sequence_snapshot_replaces_store_as_resync` |
| scoped snapshots preserve unrelated rows | `scoped_snapshot_replaces_only_matching_scope` |
| scoped snapshots do not poison whole-family sequence gate | `scoped_snapshot_allows_same_sequence_delta` |
| nested patch replaces instead of deep-merging | `patch_replaces_nested_values_without_deep_merge` |
| plugin entity types round-trip without per-plugin client code | `plugin_entity_family_round_trips_through_generic_stores` |
| plugin namespacing | `plugin_entity_type_requires_owner_namespace` |
| built-in reserved families | `reserved_builtin_families_do_not_require_plugin_namespace` |
| string ids | `plugin_records_require_non_empty_string_ids` |
| id-field defaults | `builtin_id_field_defaults_extract_expected_ids` |
| invalid record rejection | `invalid_snapshot_and_upsert_records_are_rejected` |
| public consumer path | `public_api_applies_protocol_frames_like_downstream_client` |

Required verification commands:

- `cargo fmt`
- `cargo test`

## Vault Gaps

No new durable vault gap is known at plan time. Existing notes already cover entity-only state sync, plugin namespaced frames, scoped hydration, scoped sequence gates, reconnect baselines, browser/TUI parity, and plan artifact discipline.

Capture a new vault note only if implementation discovers an id-field default or cross-client semantic that is not already documented in vault or repo references.
