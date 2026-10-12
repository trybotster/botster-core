# Hub→plugin contract v1.1, Amendment 5 (candidate 1): the layout of one entity record

The base is the Hub→plugin contract of `frozen/current` (manifest final24): v1.1 joint J4e, with Amendments 1 to 4, which are unchanged except where A5-2 names a replacement. Other contracts are cited by contract name and clause id only.

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-02. Found by the hp-surfaces pair, through the Stage 0 lead (about 25 entity transcripts).

**Why.** Appendix A, "Handler kinds" (A2-5), gives the `entity_provider` result as `{records: [...], cursor?}`, but no clause fixes the layout of one record, or what the declared schema "for records" (EN-1) covers.
- `entity.publish{entity_type, type, id, entity | patch}` (EN-4, A2-7) carries the id **outside** the record body.
- WF-6 says "a row's own identifier is `id`".
- EN-1 fails a load with "custom id fields".

| # | Clause | Test |
|---|---|---|
| A5-1 | **One entity record has one shape everywhere a plugin produces it: `{id, entity}`.**<br>- `id` is the record's identifier: a string, the row's own `id` (WF-6), unique in the family.<br>- `entity` is the record body: a JSON object of the family's fields.<br>- Each entry of an `entity_provider` result's `records` is `{id, entity}`. `entity.publish` keeps its existing `{entity_type, type, id, entity | patch}`, the same pair, and a `patch` applies to `entity` (A2-7).<br>- **The declared schema "for records" (EN-1) validates `entity` only.** It never declares `id`, because the identifier is fixed by the contract. A schema that declares a property named `id` is a **custom id field**, and it fails the load (EN-1). An `entity` that carries an `id` member fails validation as `invalid_request{detail.code: entity_schema}`.<br>- **Clients see one flat row:** the Hub projects a record to the client kind row as `id` plus the members of `entity` (client DS-1, the kind's fields).<br>- **Generated schema:** the record shape `{id: string, entity: <family schema>}` is part of the generated Appendix B artifact for the `entity_provider` handler result and for `entity.publish`. | `conf::a5_1_provider_record_is_id_and_entity`, `conf::a5_1_schema_validates_entity_only`, `conf::a5_1_schema_declaring_id_fails_the_load`, `conf::a5_1_entity_with_id_member_is_entity_schema`, `conf::a5_1_client_row_is_id_plus_entity_fields` |

## A5-2. Text replaced

1. **Appendix A, "Handler kinds", the `entity_provider` row:** `{records: [...], cursor?}` reads `{records: [{id, entity}], cursor?}` (A5-1).
2. **EN-1, "a JSON Schema for records"** reads "a JSON Schema for the record's `entity` (A5-1)".
3. **Appendix B:** the generated artifacts include the A5-1 record shape.

Every other clause is unchanged.
