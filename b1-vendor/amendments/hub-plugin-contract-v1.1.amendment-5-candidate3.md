# Hub→plugin contract v1.1, Amendment 5 (candidate 3): the layout of one entity record

The base is the Hub→plugin contract of `frozen/current` (manifest final24): v1.1 joint J4e, with Amendments 1 to 4, which are unchanged except where A5-2 names a replacement. Other contracts are cited by contract name and clause id only.

**Supersedes candidate 2** (`da3c249e…0b1d`, commit `ccbd656`), which the Amendment reviewer rejected (verdict `bc4ff5e`). Adding `id` to the declared schema breaks `maxProperties` and composed closed-object schemas. Candidate 3 does not rewrite the schema: the client `schema` is the declaration unchanged, `id` is a core field, and a row is validated by a stated equivalence.

Candidate 2 superseded candidate 1 (`63da8f40…7466`, commit `942aadc`), which the Amendment reviewer rejected (verdict `7d753a1`):
1. The client schema projection for the flat row is defined from the one declaration (DS-2), and it works with `additionalProperties: false`.
2. The reserved `id` applies at the `entity` top level only.

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-02. Found by the hp-surfaces pair, through the Stage 0 lead (about 25 entity transcripts).

**Why.** Appendix A, "Handler kinds" (A2-5), gives the `entity_provider` result as `{records: [...], cursor?}`, but no clause fixes the layout of one record, or what the declared schema "for records" (EN-1) covers.
- `entity.publish{entity_type, type, id, entity | patch}` (EN-4, A2-7) carries the id **outside** the record body.
- WF-6 says "a row's own identifier is `id`".
- EN-1 fails a load with "custom id fields".

| # | Clause | Test |
|---|---|---|
| A5-1 | **One entity record has one shape everywhere a plugin produces it: `{id, entity}`.**<br>- `id` is the record's identifier: a string, the row's own `id` (WF-6), unique in the family.<br>- `entity` is the record body: a JSON object of the family's fields.<br>- Each entry of an `entity_provider` result's `records` is `{id, entity}`. `entity.publish` keeps its existing `{entity_type, type, id, entity | patch}`, the same pair, and a `patch` applies to `entity` (A2-7).<br>- **The declared schema "for records" (EN-1) validates `entity` only.** The identifier is fixed by the contract, so the schema never declares `id` **at the top level of `entity`**:<br>&nbsp;&nbsp;- a schema whose top-level `properties` names `id` is a **custom id field**, and it fails the load (EN-1);<br>&nbsp;&nbsp;- an `entity` whose top level carries an `id` member fails validation as `invalid_request{detail.code: entity_schema}`;<br>&nbsp;&nbsp;- **nested objects are not affected:** an object inside `entity` may declare and carry its own `id` member (for example a list of `{id, label}` items).<br>- **Clients see one flat row:** the Hub projects a record to the client kind row as `id` plus the top-level members of `entity` (client DS-1, the kind's fields).<br>- **The client schema is the one declaration, unchanged** (DS-2: no second hand-written schema). The kind descriptor's `schema` (client DS-1) is the declared `entity` schema exactly as declared. **`id` is a core field** of the kind (DS-1 `core_fields`), typed `string`, and it is not part of `schema`.<br>- **Validation equivalence:** a flat client row is valid **if and only if** its `id` is a string **and** the row without its `id` member validates against `schema`. That is exactly the validity of the record `{id, entity}`. The Hub validates by this rule, and so does any client that validates rows. So every keyword of the declaration keeps its meaning, including `maxProperties`, `additionalProperties: false` and composed closed-object schemas (`allOf`, `unevaluatedProperties`).<br>- **Generated schema:** the record shape `{id: string, entity: <family schema>}` is part of the generated Appendix B artifact for the `entity_provider` handler result and for `entity.publish`. | `conf::a5_1_provider_record_is_id_and_entity`, `conf::a5_1_schema_validates_entity_only`, `conf::a5_1_schema_declaring_id_fails_the_load`, `conf::a5_1_entity_with_id_member_is_entity_schema`, `conf::a5_1_client_row_is_id_plus_entity_fields`, `conf::a5_1_describe_schema_is_the_declaration_unchanged_and_id_is_a_core_field`, `conf::a5_1_flat_row_validates_with_additional_properties_false`, `conf::a5_1_max_properties_counts_entity_members_only`, `conf::a5_1_composed_closed_schema_validates_the_row_without_id`, `conf::a5_1_nested_id_members_are_allowed` |

## A5-2. Text replaced

1. **Appendix A, "Handler kinds", the `entity_provider` row:** `{records: [...], cursor?}` reads `{records: [{id, entity}], cursor?}` (A5-1).
2. **EN-1, "a JSON Schema for records"** reads "a JSON Schema for the record's `entity` (A5-1)", and EN-1's "custom id fields" means a top-level `id` in that schema.
2a. **Client DS-1 and DS-2, the plugin kind:** `schema` is the declaration unchanged, `id` is in `core_fields`, and rows validate by A5-1's equivalence.
3. **Appendix B:** the generated artifacts include the A5-1 record shape.

Every other clause is unchanged.
