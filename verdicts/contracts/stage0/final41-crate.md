# contracts #90 — stage0/final41-crate (Core A22: `RouteAdopted.route_tag` and `.owner` are `Option<String>`; R-104), reviewer 017f

HIGH: the PR changes the frozen-crate types of `botster-core-contract`, the generated Core event schema, and adds one Core transcript. Base: `steward/final41` `b189abe7c3a5121cd36914b506b423f32fd45465` (manifest final41 = final40 + Core Amendment 22, staging). Implementer: 016b. Package reviewer: 016c.

## Round 1: no finding of mine on 94dbbab7bd7a08ca63b2121ede9d7e5d3fb49ee4

Checked against the A22 candidate 1 text (`frozen/current/core-contract-v1.17-amendment-22-candidate1.md`):
- **The crate** (`event.rs`): `RouteAdopted.route_tag` and `.owner` are `Option<String>`, each with `#[serde(default, skip_serializing_if = "Option::is_none")]`. This is the form of `RouteClosed.route_tag`, as A22 requires. `client` stays required.
- **The schema** (`schemas/core/event.schema.json`): both fields change to `type: ["string","null"]` and leave `required`. `RouteClosed.route_tag` has the same generated form, and the gate's codegen step (`--check`) passes. The serde form never writes `null`, so the schema is wider than the wire only by `null`, as for `RouteClosed`.
- **The TypeScript.** A22 says the generated TypeScript follows by codegen. The repository's only TS package is `hub-contract`. No TS file names `RouteAdopted` or `route_tag`, so no TS change is due.
- **The tests** (`tests.rs`): `route_adopted_tells_an_absent_tag_from_an_empty_one` covers `(t3,o3)`, `("","")`, `(None,None)` and `(None,Some(""))` in the round-trip, the deserialize and the schema validation. The old round-trip case uses `Some("t")`/`Some("o")`.
- **The transcript** `conf::a22_1_route_adopted_tells_an_absent_tag_from_an_empty_one` (`fake: structural`, ruling R-12) matches A22's test step by step:
  - s4-s21: r1 with no tag options, r2 with `route_tag: ""` and `owner: ""`, r3 with `t3`/`o3`, each to `live`;
  - s22-s24: `drop h`, `open h2`, `AdoptAll`;
  - s25: `RouteAdopted` for c1 with `route_tag` and `owner` both `{"$absent": true}`, for c2 with `""`/`""`, for c3 with `t3`/`o3`;
  - s26-s31: a `Detach` of each adopted route gives `RouteClosed` with `route_tag` absent, `""` and `"t3"`.
- **The other DP-8 transcripts.** Every `RouteAdopted` expectation in `conformance/core/` names a tag that its `attach_route` gave (a3_1, dp_12, three dp_8 ids, ev_8). So no existing expectation changes, as A22 says.
- **The maps.** The README structural row and the `core-testkit` proof in `replacement-map.json` are present. The counts in `fake-replacement-map.md` move by one id (2547 to 2548), and the gate's ledger step gives "2548 ids".
- **Gate PASS on the exact head**, all ten steps, exit 0 after 421 s on gaming, base main `29b8f6e7a4884ca76aeee549c77b683c1bcd222a` (the merge base of `steward/final41` and main): `~/botster-sessions/gates/botster-contracts-stage0-final41-crate-94dbbab7-pool-20261012-083808-23047.log`.

016c's round 1 (REJECT on `94dbbab7`): one LOW, the FakeCore adoption gap note that A22 requires in `docs/fake-replacement-map.md`. That is 016c's package scope, so I do not re-review it.

## Final: CLEAN on 76a8d7e7f9b8d4c0415f14cb4432ee02caadce24

- **The delta from `94dbbab7`.** One commit, `76a8d7e7` (016c's LOW). It adds one line to `docs/fake-replacement-map.md` and the same line to `render()` in `xtask/src/replacement_map.rs`, so the generated map and its `--check` agree. No crate, schema, transcript or map row changes, so my round-1 facts hold.
- **The gap note is true.** FakeCore's `adopt_all` (`crates/botster-fake-core/src/pump.rs:262`) clears `s.routes` and posts no route event, and no file of `botster-fake-core` names `RouteAdopted`. So `fake: structural` with the `core-testkit` proof is the case that A22 names.
- **Base and main.** `steward/final41` is `b189abe7c3a5121cd36914b506b423f32fd45465`, the parent of the PR's first commit. Main is `29b8f6e7a4884ca76aeee549c77b683c1bcd222a`, the gate base.
- **Gate PASS on the exact head**, all ten steps, exit 0 after 176 s on gaming, base main `29b8f6e7`, "ledger ok: 2548 ids": `~/botster-sessions/gates/botster-contracts-stage0-final41-crate-76a8d7e7-pool-20261012-084825-76122.log`.

I have no open finding, and 016c's LOW is closed in this head (016c's scope). CLEAN on `76a8d7e7f9b8d4c0415f14cb4432ee02caadce24`.
