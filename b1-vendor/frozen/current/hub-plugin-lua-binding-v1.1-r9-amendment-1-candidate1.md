# Hub→plugin Lua binding v1.1 r9, Amendment 1 (candidate 1): three host-local shapes

The base is the Lua binding r9 of `frozen/current` (manifest final25), which is unchanged except where LBA1-4 names a replacement. Other contracts are cited by contract name and clause id only.

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-02. Found by the lb-cli-suite pair, through the Stage 0 lead.

**Why.** r9 names these helpers and arguments but leaves three shapes open:
- what `json.array` and `json.empty_array` are;
- what `value` the base64 and JSON conversions return;
- whether a handler entry's descriptor fields are flat or nested.

| # | Clause | Test |
|---|---|---|
| LBA1-1 | **`json.array`, `json.empty_array` and `json.null` are constructors and values, not calls with an envelope.**<br>- `botster.json.array{ ... }` returns the **array itself**: a Lua table that encodes as a JSON array, even when it is empty. It is not an LB-3 envelope.<br>- `botster.json.empty_array` is a **value**, not a function: a read-only empty array. Writing to it is a programming error, which raises (LB-3, LM-4). A plugin that needs an array it can grow uses `json.array{}`.<br>- `botster.json.null` stays the A2-8 sentinel value.<br>- LB-8's "All host-local helpers return the LB-3 envelope" covers the helpers that convert or compute (`base64.*`, `json.encode`, `json.decode`, `crypto.*`, `ids.new`), not these three constructors and values. | `conf::lb_9_empty_array_encodes_as_array`, `conf::lba1_1_json_array_returns_the_array_not_an_envelope`, `conf::lba1_1_empty_array_is_a_read_only_value` |
| LBA1-2 | **The `value` of the base64 and JSON conversions is the bare result.**<br>- `botster.base64.encode{ bytes }`: `value` is the base64 **string**.<br>- `botster.base64.decode{ text }`: `value` is the byte **string**.<br>- `botster.json.encode{ value }`: `value` is the JSON **text**.<br>- `botster.json.decode{ text }`: `value` is the decoded Lua **value**, with `json.null` for `null` (LB-9).<br>Each is wrapped in the LB-3 envelope. The `crypto.*` helpers keep their named result tables (`{ digest_b64 }`, `{ mac_b64 }`, `{ bytes_b64 }`), as LB-8 states. | `conf::lb_8_bytes_round_trip_base64`, `conf::lba1_2_conversion_values_are_bare` |
| LBA1-3 | **A handler entry's descriptor fields are flat.** Each entry of `botster.register{ handlers }` is one table: `kind`, the optional `id` (`handler_id`), `call`, and **that kind's descriptor fields at the same level**. For example:<br>- `{ kind = "ui_action", action_id = "rename", call = fn }`;<br>- `{ kind = "mcp_resource", uri_template = "…", name = "…", mime_type = "…", call = fn }`;<br>- `{ kind = "entity_provider", entity_kind = "workspace", call = fn }`. The field `kind` already names the handler kind, so the `entity_provider` descriptor's own `kind` (A2-5) is written **`entity_kind`** in the Lua table. The host sends it as the descriptor field `kind` on the wire.<br>This matches LB-6's `{ kind = "event", id, pattern, call }` and LB-10's `{ kind = "timer", id, call }`. An unknown field or a missing required descriptor field fails the load (`invalid_request`). | `conf::lb_10_descriptors_per_kind`, `conf::lb_10_handler_kind_strings_and_payloads`, `conf::lba1_3_entity_provider_kind_is_written_entity_kind` |

## LBA1-4. Text replaced

1. **LB-8, "All host-local helpers return the LB-3 envelope"** reads with LBA1-1: the constructors and values `json.array`, `json.empty_array` and `json.null` are excepted.
2. **LB-9, `json.array{ ... }` (or `json.empty_array`)** reads with LBA1-1.
3. **LB-10, the descriptors** are written flat in the handler table (LBA1-3). The `entity_provider` descriptor's `kind` is `entity_kind` in Lua.

Every other clause is unchanged.
