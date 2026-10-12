# lb-cli-suite: the Lua binding and CLI conformance runners

Owner: the lb-cli-suite pair. Date: 2026-10-02. Contracts: Lua binding r9 with Amendment 1 (ledger prefix `lb`, plus `lh_1`/`lh_4` of `hp`), CLI spec candidate 34 (prefix `cli`, informative). Steward rulings R-1.4, R-12.
Two READYs on one branch: part 1 is the Lua binding runner (this section), part 2 is the CLI runner.

## Part 1: the Lua binding

### Prior art (BUILD.md rule 0)

- **Vault.** `botster phase zero fake proofs do not establish real Hub semantic conformance` (for LB and CLI ids Phase 0 is transcripts plus structural validation), `botster core conformance suites receive a configured CoreHarness` (the harness-trait shape), `conformance harnesses need an implementation ablation at the contract oracle`, `botster conformance runner limits yield inconclusive outcomes`.
- **botster-hub@9bc6cd7d4963ae5cccd642931619b9cac5c8daac** (read only): `docs/lua-plugin-abi.md`, `src/lua_runtime.rs`, `crates/botster-plugin-test-kit/fixtures/kit-fixture*/plugin.lua`. REUSED as a idea only: the test kit's fixture plugins show how a plugin is written (`botster.register{ tools = {{ name, description, input_schema, handler, call }} }`) and that a bad-global fixture needs a probe. Nothing is stolen, so no `Stolen-From` trailer. REJECTED as a source of behavior: the old ABI is a different protocol (kinds `command`, `hook`, `surface_route`, an in-process thread host, no frames), and the old kit runs Lua specs against a real daemon.
- **frozen/final6/audits/plugin-parity-20260930.md.** The shipped plugins' call patterns fed the choice of calls in the scripts (sessions, store, timers). It fixed no expected value.
- **In this repo (REUSED).** The HP runner (`botster-plugin-conformance`: `HpDriver`, `HpSubject`, `HpSchemas`, the frame templates and the engine), the transcript engine, `testgen`. Only one runner function changed: `run_test_with` takes the template directory separately, so the Lua suite shares the HP templates.
- **Libraries.** `full_moon` (dev-dependency, `lua54`): a pure-Rust Lua parser that checks the syntax of every script. Rejected: `mlua` (it would run Lua, which makes it a second Lua host in this repo).

### What landed

- `botster-lua-conformance`: `LuaHostSubject` (an `HpSubject` that also loads a package), `LuaPackage`, `LuaAdapter`, `lua_host_conformance_tests!`, `StructuralLuaHost` (the Phase 0 subject, ruling R-1.4: the Lua host has no fake).
- `conformance/lb/`: 53 transcripts (44 `lb` ids, the 4 `lba1` ids of Amendment 1, the 3 `lba2` ids of Amendment 2, and `lh_1_local_helpers_send_nothing`, `lh_4_globals_equal_published_set`, contract `hp`, handed to this feature by the lead), plus `lb_21` in `conformance/hp` (54 ids).
- `docs/lua-host-controls.md`: the harness obligations: the package (`setup.package`), `settle`, `admit_package`, `package_session_types`, `load_entrypoint`, `readable_globals`, and the rule that a refusal's place (local or forwarded) is the host's (R-27).

### What a transcript is

A host-side script (design 6.5): the runner plays the Hub with HP frames; the plugin is Lua source in `setup.package`. A tool handler runs the calls under test and returns what the Lua code saw, so the `Result` frame is the observation. Every `HostCall` the script expects has the Appendix A arguments, and every scripted answer has the Appendix A result (test `calls_and_scripted_answers_follow_appendix_a`). Scripted answers (the Hub's) come from the clause of the id; the host's own work (the envelope, the null sentinel, `require`, arguments passed unchanged) is what is asserted.

### Phase 0 checks (all in `cargo test -p botster-lua-conformance`)

1. Each transcript runs the structural check against `StructuralLuaHost` (every valid frame validates against the HP schemas).
2. Every Lua source parses (full_moon); every JSON package file parses; every manifest validates against `plugin-manifest.schema.json`.
3. Every expectation has a well-formed `because`, and every cited clause exists in the file its prefix names.
4. `lb_1` probes exactly `botster_names()` of the contract crate. `lh_4` has the harness report the complete readable global set (control `readable_globals`), binds `Ready.host.globals`, passes both to the plugin, and the plugin compares them in both directions and looks up every published name for real (`_ENV[name]`); no name list, no table-storage assumption and no count bound is copied into the transcript.
5. Orders the contract leaves open are matched as sets: the new matcher `$set` (with engine vectors; the TypeScript engine needs the same, feature ts-player) matches an array against matchers, each once, in any order. The step `answer_host_call` of the HP driver is new, with tests in botster-plugin-conformance.
6. Runner self-test (`tests/driver.rs`): the package reaches the subject as written; `expect_quiet_until` passes when the subject sends no `HostCall` and FAILS at its step when it does (FakePlugin, a frame-level stand-in that runs no Lua). This is the observer that shows the negative assertions of the suite are not blind (design 7.2 rule 7). There is no fake in this feature, so there is no fake branch to ablate (rule 6).

### Ids left pending, with the gap

- `conf::tr_2_hostile_plugin_contained` (HP TR-2): proof is `slow:hostile-plugin-sandbox` in the replacement map, a real sandbox run. A structural Lua check cannot replace it (lead and reviewer, 2026-10-02). Stays pending.
- Moved, not pending: `conf::lb_21_route_ended_is_client_only` (LB-21). Steward ruling R-23: the refusal is the Hub's, at registration (HP EV-2, EV-9, CP-4). It is a `plugin_hub` transcript in `conformance/hp` (structural, R-1.3): a `Ready` that subscribes to `hub/route.ended` fails the load (`load_failed`, nothing published) and no copy is ever delivered. No clause names the failure kind, so it asserts only the failed load. Its replacement-map proof is `hub-inproc`.
- Amendment 2 (final28): the three `lba2_1_*` ids landed (the Lua shape of tool, prompt and resource entries). The other scripts register tools as flat `mcp_tool` handler entries (LBA1-3), so they do not depend on the entry shape of Amendment 2.

### Assumptions the reviewer should check

1. **Where a refusal is decided** (R-27): LB-2, LB-15 (`web.set_origin{}`), LB-11 (an invalid method) and LB-16b (an unknown spawn key) are asserted by the plugin's outcome only (`ok = false`, `kind = invalid_request`, `effect = none`); the step `answer_host_call` makes the transcript accept a host that decides locally and a host that forwards the call. No `HostCall` is asserted or forbidden for them.
2. **Tools are handler entries.** Every script registers its tool as `{ kind = "mcp_tool", id = "h_t", name, description, input_schema, call }` in `handlers` (the flat form of LBA1-3, descriptor of A2-5), so no transcript depends on a separate `tools` entry shape (the lead held the first version for that reason). Scripts that register no tool pass `tools = {}`, an argument LB-10 names. The Ready assertions name handler ids that the plugin chose.
3. **`lb_12_observation_event_grants`** asserts only what the host declares: `Ready.events.subscribed` and `Ready.grants` for the hub/session.* and terminal/* patterns, as exact sets without an order (the new matcher `$set`). The grant check itself is the Hub's.
4. **Scripted Hub answers** carry error shapes from `plugin-errors.json` (`wrong_state` reasons, `revision_conflict`, `unknown_handler`, `handler_id_required`). The timer and watch `handler_id` rules of A2-53 and the mandatory `fs.watch` handler id are refusals whose place is not fixed (R-27): their omitted-key cases use `answer_host_call`, accept a local refusal and a forwarded one, and keep `handler_id` ABSENT in any forwarded call so a synthesized default fails; the envelope is asserted on both paths (`handler_id_required` for the timer case). A call that carries `handler_id` as written is forwarded and its arguments are asserted.
