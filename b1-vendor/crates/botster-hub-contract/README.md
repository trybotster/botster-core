# botster-hub-contract

The Hub→client contract as types: the envelope, errors, handshake, `describe`, watch and event frames, the 16 Hub-owned kinds with their verbs and actions, pairing frames and the MCP tool data. Source: `frozen/current` HC parity45 with A1 to A4 (ledger prefix `hc`). It holds no Hub logic.

- `cargo xtask codegen` writes `schemas/hub-client/` from these types: `envelope.schema.json`, `pairing.schema.json`, `mcp.schema.json`, `errors.json` and `kinds/<kind>.schema.json`.
- `ts/packages/hub-contract` (`@trybotster/hub-contract`) generates the TypeScript from those schemas.
- `registry.rs` is the one table of the kinds (verbs, actions, effects, argument and result types) and of the operation kinds (A.7b).
- Design notes and prior art: `docs/foundation-design.md` section 15.

## Pending rulings

HC does not fix the JSON layout of six things. Each has one isolated type and a `// pending steward ruling H2` (or `H3`) marker. When the steward rules, only that type changes, then `cargo xtask codegen` and `npm run codegen -w @trybotster/hub-contract`.

| # | Assumption | Type | Clauses |
|---|---|---|---|
| 1 | A `watch` frame carries its sub-frame in a member `frame` (`snapshot_begin`, `snapshot_items`, `snapshot_end`, `upsert`, `patch`, `remove`, `batch_begin`, `batch_items`, `batch_end`, `watch_closed`). A change inside `batch_items` uses the same tag. | `WatchFrame`, `Change` in `watch.rs` | HC EN-2, WT-1 to WT-5 |
| 2 | `event_gap` is a server `event` frame with `"frame": "gap"`. A plain event has no `frame` member. | `EventGap`, `EventFrame` in `events.rs` | HC EN-2, EV-2, EV-4 |
| 3 | The pre-envelope frames (`route_connect`, `auth_challenge`, `pair_request`, `auth_response`, `auth_accepted`, `signal_*`) are `{"frame": "<name>", ...}`, as `route_connect` is written in TB-L2. | `AuthFrame`, `Signal` in `pairing.rs` | HC TB-L2, PR-4 to PR-7 |
| 4 | `req` has no `if_rev` member, so `update` carries `{ patch, if_rev? }` in `args`. | `UpdateArgs` in `envelope.rs` | HC EN-1, VB-4 |
| 5 | Every timestamp except `expires_at` (Unix seconds, PR-7b) is Unix milliseconds. | `Timestamp` in `wire.rs` | HC EV-1, A.8, R-11 |
| 6 | The rescan operation (`spawn_target.rescan_packages`) ends `succeeded` with `found` and `removed` as the members of `row.result`. Ruling H3. | `RescanResult` in `actions.rs`, `OPERATION_KINDS` in `registry.rs` | HC A2 A.7b, A2 part 1 |
