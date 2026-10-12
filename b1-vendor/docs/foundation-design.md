# Phase 0 foundation design

Owner: the Foundation lead. Date: 2026-09-30. Status: design for review (Sol). This file is internal to this repo. `docs/BUILD.md` is binding over it.

> **2026-10-01, the testing architecture changed** (orchestrator decision; BUILD.md "Phases", commit 33f3e07; candidate amendments Core A5 and HC A5). Conformance runs against the REAL implementations in-process through injected edges. The only scripted peers are **players** of the clause transcripts (the client-subject runner, the TypeScript player as the Rust player compiled to wasm, and the HP frame players). The landed FakeCore and fake route endpoint are **frozen** (no new fidelity) until their real counterparts pass; `docs/fake-replacement-map.md` maps every id to its replacement proof. Sections 7 (fakes), 7.4 (FakeHub) and 12 (queue) below are superseded where they conflict; section 12c is the current queue.

## 1. What Phase 0 delivers

1. **Contract crates.** These crates hold the types, traits, error codes and limits of each contract. They carry no product logic, with one exception: the reference `realize` in `botster-ui-contract` (UI section 16).
2. **Generated artifacts.** The JSON Schemas of HP Appendix B, the client kind schemas, and the TypeScript types for Web.
3. **Fakes.** FakeCore, FakeHub (client-facing, in Rust and in TypeScript), and the plugin-side pair FakeHub and FakePlugin. Each fake is written from the clauses only.
4. **Conformance suites.** One transcript per test id, plus seeded interleaving. Rust runners exist for every suite. A TypeScript runner exists for the Hub→client suite.
5. **The ledger.** One generated list of every test id in the current contract files. CI fails when a ledger id has no transcript, or when a transcript has an id that is not in the ledger.

**Done when:** the ledger has no missing ids, every suite passes against its fake under the CI seed set, the taint check runs in CI, and the README tells a Phase 1 lead how to depend on the crates and run the suites.

## 2. The current contract set

The ledger reads only the current files. The superseded files stay in `frozen/final6/` for the hash record, and the ledger ignores them.

| Contract | Current files | Ledger prefix |
|---|---|---|
| Core | `core-contract-v1.17.md`, Core A2, Core A3 | `core` |
| Route codec | `route-codec-v1-extract-rev36.md` | `codec` |
| Hub→client (HC) | `hub-client-contract-v1-candidate-parity45.md`, HC A1, A2, A3 | `hc` |
| Hub→plugin (HP) | `hub-plugin-contract-v1.1.joint-J4e.md`, HP A1, A2 | `hp` |
| Lua binding | `hub-plugin-lua-binding-v1.1-r9.md` | `lb` |
| UI | ui-contract 1.0, 1.1, 1.2, 1.3, 1.4 | `ui` |
| CLI spec (informative) | `cli-spec-v1-candidate34.md` | `cli` |

Superseded and ignored: codec rev26 and rev30, Lua binding r6, CLI spec candidate26.

At the time of writing, the current files hold about 2,400 distinct `conf::` ids. The exact count comes from the ledger.

**Duplicate ids.** The codec extract repeats clauses of HC section 7. One test id is one ledger row. The row records every file that names the id. The codec file is the owner of a codec id.

**Ids that an amendment withdraws** (for example `conf::a2_26_withdrawn`) still get a transcript. That transcript asserts the absence of the withdrawn behavior where the clause says so. Where no observable assertion exists, the transcript has `"kind": "withdrawn"` and the runner records it as passed with no steps. The pair asks the steward when the meaning is unclear.

## 3. Workspace layout

```
Cargo.toml                    workspace; one version for all crates
.cargo/config.toml            [net] git-fetch-with-cli = true
rust-toolchain.toml           pinned stable toolchain
contracts.toml                the current-file table of section 2 (input to the ledger)
crates/
  botster-route-codec/        codec rev36: frame types, TS-3 framing, A.4 to A.6, JSON route bodies
  botster-core-contract/      Core types, the CoreApi trait, CoreError, CoreLimits, features
  botster-hub-contract/       HC envelope, errors, kinds, describe types, limits
  botster-plugin-contract/    HP frames, Result, ErrorKind, PluginLimits, Features, the call list, the manifest
  botster-ui-contract/        UI parts, UiNode, the reference realize, vectors/ (UI-RZ-3b)
  botster-conformance/        shared: transcript model, matchers, seeds, ledger, report
  botster-core-conformance/   the Core runner over Box<dyn CoreApi>
  botster-hub-conformance/    the HC runner (codec transcripts included)
  botster-plugin-conformance/ the HP runner (frame scripts in both directions; Lua binding scripts)
  botster-ui-conformance/     the structural check of conformance/ui/ (client and hub subjects of ui ids; R-1.4)
  botster-fake-route/         one fake session-worker route endpoint, shared by FakeCore and FakeHub
  botster-fake-core/          FakeCore
  botster-fake-hub/           FakeHub, the client-facing fake (Rust)
  botster-plugin-fakes/       the plugin-side FakeHub and FakePlugin
  botster-conformance-probe/  a small binary that runs a probe script (section 6.2) for real-Core runs
xtask/                        codegen, ledger, manifest-hash check, taint check
conformance/
  ledger.json                 generated; checked in
  core/ codec/ hc/ hp/ lb/ cli/   one JSON transcript per test id, grouped by clause
schemas/
  transcript.schema.json
  plugin/                     the five artifacts of HP Appendix B, plus the A1 and A2 artifacts
  hub-client/                 envelope.schema.json, errors.json, kinds/<kind>.schema.json
ts/                           npm workspace
  packages/hub-contract/      @trybotster/hub-contract: generated types
  packages/fake-hub/          @trybotster/fake-hub: FakeHub in TypeScript
  packages/conformance/       @trybotster/hub-conformance: the TypeScript runner and the hc/ codec/ transcripts
.github/workflows/ci.yml
```

UI vectors are in `crates/botster-ui-contract/vectors/`, because UI-RZ-3b says so. The ledger accepts a UI vector as the transcript of a `ui` id.

HP Amendment 1 names `agent-definition.schema.json` and `accessory.schema.json`. HP Amendment 2 names `catalog.schema.json`. These artifacts go in `schemas/plugin/` beside the five Appendix B artifacts.

## 4. Crate boundaries

**Normal dependencies** (an arrow means "depends on"):

```
botster-ui-contract      botster-route-codec        botster-conformance (no contract type)
      ↑      ↑                ↑         ↑
      │      └─ botster-hub-contract    botster-core-contract
      │              ↑
      └──── botster-plugin-contract (also depends on botster-hub-contract: the `session` row, steward ruling R-11)

fakes:   botster-fake-route   → codec
         botster-fake-core    → core-contract, fake-route
         botster-fake-hub     → hub-contract, fake-route
         botster-plugin-fakes → plugin-contract
runners: botster-<x>-conformance → its contract crate(s), botster-conformance
```

**Test dependencies** (`dev-dependencies` only): each runner crate depends on its fake, for the self-test. `botster-hub-conformance` → `botster-fake-hub`; `botster-core-conformance` → `botster-fake-core`; `botster-plugin-conformance` → `botster-plugin-fakes`.

**Rules.**
- A contract crate never depends on a fake, a runner or `botster-conformance`.
- A fake never depends on a runner or on `botster-conformance`, in normal or test dependencies. A fake implements the runner's `TestControl` trait through a small adapter module in the runner crate's test code, so the fake does not need the trait.
- A runner depends on a fake only in its `dev-dependencies`, or as an optional dependency behind the runner's `fake` feature. That feature exports the adapter that wraps the fake in the runner's harness trait (for example `botster_core_conformance::fake::FakeCoreHarness`). The adapter lives in the runner crate, so the fake never depends on the runner.

| Crate | Depends on | Notes |
|---|---|---|
| `botster-route-codec` | serde | Both Core and the HC crate use it. The codec is owned by HC and adopted verbatim by Core, so it is its own crate. Then Core never depends on the Hub crate. |
| `botster-core-contract` | codec | `CoreApi` is the trait that FakeCore and the real `Core` implement (Core section 11). |
| `botster-hub-contract` | codec, ui-contract | Kind structs derive `JsonSchema`. |
| `botster-plugin-contract` | ui-contract, hub-contract | Frames, calls and the manifest derive `JsonSchema`. It re-exports `SessionRow` from the Hub crate, so HC and HP share one `session` row (R-11; edge approved by the Foundation lead, 2026-10-01). |
| `botster-ui-contract` | serde | The reference `realize` is pure (UI-RZ-1). |
| `botster-conformance` | serde_json, jsonschema, rand_chacha | No contract-specific type. |

**Clause citation.** Every public item in a contract crate has a rustdoc line that cites its clause ids, for example `/// Clause: HC EN-1, HC A.1.` The rustdoc never copies contract prose (BUILD.md, taint rule 5). Core section 11 asks for the clause text as rustdoc. BUILD.md overrides that sentence, so the rustdoc gives the id and the contract file, and the reader opens the frozen file.

**Facade.** Each contract crate exports a `prelude`. Everything else is private or under a feature named `unstable`.

## 5. The transcript format

A transcript is one JSON file. `schemas/transcript.schema.json` defines it. The common envelope is the same for every suite. The step vocabulary is per suite.

```json
{
  "id": "conf::en_8_pending_key_joins",
  "contract": "hc",
  "clause": "EN-8",
  "subject": "hub",
  "requires": { "features": [], "controls": ["hold_completion"] },
  "setup": { "limits": { "max_outstanding": 8 }, "fixture": "base" },
  "steps": [
    { "send": { "type": "req", "id": "1", "verb": "invoke", "kind": "session", "target": "$s1", "args": {}, "idempotency_key": "k" } },
    { "send": { "type": "req", "id": "2", "verb": "invoke", "kind": "session", "target": "$s1", "args": {}, "idempotency_key": "k" } },
    { "control": { "op": "release_completion" } },
    { "expect_set": [
        { "type": "res", "id": "1", "ok": "$r" },
        { "type": "res", "id": "2", "ok": "$r" } ] }
  ]
}
```

### 5.1 Envelope fields

| Field | Meaning |
|---|---|
| `id` | The test id. It equals the file name without `.json`, and it is in the ledger. |
| `contract`, `clause` | The ledger prefix and the clause row id. |
| `subject` | The side under test: `core`, `worker`, `hub`, `client`, `plugin_hub`, `plugin_host`, `lua_host`, `cli` or `realizer`. Section 7 maps each subject to its fake. |
| `requires.features` | Feature names that the subject must advertise. A subject without one reports the transcript as `not_applicable`, never as passed. |
| `requires.controls` | The test controls (section 6.3) that the transcript uses. |
| `setup` | Limits and a named fixture. Fixtures live in `conformance/<suite>/fixtures/`. |
| `steps` | The ordered script. |
| `seeds` | Optional. Absent means the transcript runs under every seed of the run. A list pins seeds, for a regression found under one seed. |
| `kind` | Optional: `withdrawn` (section 2). |
| `fake` | Optional: `"structural"` with a `ruling` (for example `"R-5"`). The subject's fake does not model this behavior by ruling (for example the Core 5.1A encoded bytes and the terminal model, steward ruling R-5; client, CLI and Lua-host subjects, R-1.4). Against the fake, the runner performs the structural check of section 7.1 and reports `passed` only if that check passes. Against a real implementation, the transcript runs in full. |

### 5.2 Matchers

A matcher is a JSON value that a received value must match.

- A literal matches an equal value. An object matcher matches when every key in the matcher matches. Extra keys in the received object are allowed, because EN-6 says that an unknown optional field is ignored.
- `"$name"` binds a variable at its first use. A later use must be equal.
- `{"$any": true}` matches any present value. `{"$absent": true}` matches only a missing key.
- `{"$type": "string" | "u64_decimal" | ...}` checks a type.
- `{"$one_of": [m1, m2]}` matches any one of the listed matchers.
- `{"$bytes_hex": "..."}` matches binary data.
- `{"$schema": "<schema ref>"}` validates against a generated schema.
- Predicates (steward-approved with feature hp-calls-a; they let a transcript state a relation that a contract fixes, such as "never reused" or "not an absolute path"):
  - `{"$not": m}` matches when `m` does not match. It binds no variable.
  - `{"$ne": "$var"}` matches a present value that differs from the bound variable. An unbound variable is a transcript error.
  - `{"$starts_with": "text"}` and `{"$contains": "text"}` match a string. `{"$not": {"$starts_with": "/"}}` is "not an absolute path".
  - `{"$between": [lo, hi]}` matches an integer with `lo <= v <= hi`.
  - `{"$b64": m}` decodes a base64 string and matches `m` against the decoded text (invalid UTF-8 becomes U+FFFD). `{"$b64": {"$contains": "x"}}` reads the bytes that a captured tail carries.
  - `{"$array": {"each": m, "min": n, "max": n}}` matches an array of any length in `min..=max` (both optional) whose every element matches `m`.
  - `{"$set": [m1, .., mn]}` (added with feature lb-cli-suite for exact-membership checks; lead-approved) matches an array of exactly `n` elements that can be given to the matchers one each, in any order: exact membership without an order that the contract leaves open. A variable that a matcher binds holds for the matchers that follow it. **TS parity needed** (feature ts-player); the engine vectors force it.
  - The TypeScript engine implements them with the TypeScript runner; the engine vectors in `conformance/engine-vectors/matchers.json` force it.

### 5.3 Order constructs

The contracts leave many orders open. A transcript must not assert an order that the contract does not promise.

| Step | Meaning |
|---|---|
| `expect` | The next relevant item matches. The suite defines "relevant" (for HC: frames on the named connection and stream). |
| `expect_set` | The next n relevant items match the n matchers in any order. An optional `ack` step answers each item as it arrives (see below). |
| `expect_eventually` | An item that matches arrives. The listed `ignore` matchers may arrive before it. Other items fail the step. |
| `expect_quiet_until` | No item that matches arrives before the result of a later barrier step. A barrier is a request whose answer the contract orders after the earlier frames. There is no wall-clock silence check. |
| `expect_closed` | The connection or route ends with the given reason. |
| `outcome` | The step ends a transcript whose result is `inconclusive` instead of failed when a runner execution limit expires (section 6.1). Authors never write it; the runner adds it. |

**`ack` (feature hp-events, lead-approved).** `expect_set` and `expect_eventually` take an optional `"ack": <step>`. The engine hands the step to the driver for each item as it arrives: for `expect_set`, each of the n items, before it waits for the next one; for `expect_eventually`, the matched item and every ignored item. Every `{"$item": "<JSON pointer>"}` in the template is replaced by that value of the item, and a pointer that the item lacks fails the step. The construct asserts no order among the items and binds no variable; it only answers each one. A transcript needs it where the contract leaves an order open and the subject sends the next item only after the answer to the previous one (HP: a handler `Result` before the next `Invoke`). The engine vectors of `conformance/engine-vectors/order.json` (including an `ack` that the driver refuses and a pointer that is absent) force it. **TS parity needed** (feature ts-player).

### 5.4 Sent values: valid frames, typed negative frames, raw stimuli and raw wire

A transcript sends four kinds of value. The kind decides how the runner treats it.

| Kind | Step form | Runner check before sending |
|---|---|---|
| **Valid frame** | `send` / `send_frame` / `route_send` with a JSON frame. Variables are substituted first. | The frame must validate against the generated schema. A failure is a transcript error, so a typo never reaches the subject. |
| **Typed negative frame** | `send_invalid` with a JSON value and `violates: "<schema path or rule>"`, for example a non-object payload, a forbidden `null`, a missing required field, or an unknown frame `type`. | The value must FAIL the generated schema at the named rule. If it passes, that is a transcript error. This proves that the negative test tests what it claims. |
| **Raw stimulus** | `send_raw` with `bytes_hex` (HC control stream, after binding framing), `route_send_raw` (TS-3 bytes), or `send_frame_raw{lane, type, payload_hex}` (HP, valid framing). | None. Raw bytes can hold what JSON cannot hold: invalid UTF-8 (HP WF-1), duplicate keys, a non-object payload. |
| **Raw wire** | `send_wire{bytes_hex}` (HP and the HC `local-stream` binding; `route_send_wire` for routes). `bytes_hex` holds the complete wire bytes, including the length field and everything after it. | None. The driver writes the bytes unchanged through the same channel and receiver path as ordinary frames. Several `send_wire` steps make several separate writes, and `close_channel` may follow, so a transcript can send an incomplete header, an incomplete body, a declared length larger than the bytes sent, or a declared length shorter than the bytes sent. |

**Received values** are always validated against the schema, and then matched. A subject that sends a frame that fails the schema fails the transcript, unless the step expects a protocol error from the subject.

`send_frame_raw` is a convenience: the driver adds a correct length field. `send_wire` is the only form that controls the length field.

**The structural check of R-1.4** (a transcript whose subject has no fake) validates valid frames and typed negative frames as above, and skips raw stimuli and raw wire bytes. The fake on the other side of the wire must accept every valid frame that it receives. It must refuse every invalid stimulus as its clause says (for example `Fault`, `protocol_error` or a close).

### 5.5 Step vocabulary per suite

- **Core** (`core`): `open{limits}`, `begin{op}`, `pump{now?}`, `poll_events{max}`, `get`, `list`, `terminal_state`, `read{...}`, `advance_clock{ms}`, `wait_wake`, and `attach_route` followed by route steps (`route_send`, `route_expect`). An op is the JSON form of the `Op` enum of `botster-core-contract`. `pump_until{matcher}` loops: it waits on the wake handle (or the next deadline), then pumps, then polls, until an event matches (OR-1, TM-3). It sets no pump count, because Core section 11 varies that count. The only bound is the runner execution limit of 6.1, and its expiry gives `inconclusive`, never `failed`.
- **Codec** (`worker`): route steps only, over the TS-3 framing. `route_expect_pty` names the bytes that reached the PTY: `{hex}` and `{var}` parts for bytes that a clause fixes (raw `bytes` and `text` are verbatim), and `{written_by_op, len_var?, contains?}` or `{written_without_op}` for the bytes that the session wrote for an input, by relation and never by a literal (BUILD.md, architecture rule 2). The log is one byte stream, and a relation part is a run in it that the transcript's own frames give (steward ruling R-73): `written_by_op: X` is one input of op X, sent on any route; a `bytes` or `text` frame's run is exactly its bytes, and every other run (a paste, a key, mouse or focus input, a reply, a file's path paste) is the encoder's bytes, not empty, so the part checks its position, its `len_var` and its `contains`. An `input_refused` that the driver observed changes its op's run: with zero effect the op has no run, and a partial write is a run of its `written_bytes`, or of unknown bytes when a `failed` or `session_ended` refusal has none (R-73 follow-up 2 (1)). `written_without_op` is a non-empty run of an input with no op: it may not hold the bytes of a `bytes` or `text` input or a paste's body that no part placed (R-73 follow-up 2 (2)). The stated limit: key, mouse and focus bytes are known only to the encoder (R-7), so a `written_without_op` run may cover them. A Core paste part carries a `contains` of its body, which holds no `0d` (Core IN-8 writes the payload verbatim; R-61 (c)). `{"one_of": [[..], ..]}` gives alternative orders. A frame is written in its decoded form (`{"frame": "output", ...}`), or as `{"raw_hex": "..."}` for a malformed-frame test.
- **HC** (`hub`, `client`): `connect{binding?, as, credential?}`, `send`, `expect*`, `disconnect`, `route_*` steps, and `control`.
- **HP** (`plugin_hub`, `plugin_host`, `lua_host`): `send_frame`, `expect_frame`, `close_channel`, `advance_clock`, `control`. A frame is written in its JSON form with `lane` and `type`.
- **Suite statements** (any suite): `run_suite{subject, filter?}` runs every transcript of that subject (optionally filtered by id prefix) against the transcript's own subject, and passes only if each of them gives `passed`. It proves clauses of the form "a host passes the same suite" (for example HP LH-3, TR-3). The runner reports the inner failures. It never counts as the inner ids' own transcripts.
- **CLI** (`cli`): `hub_fixture`, `run{argv, stdin?, env?}`, `expect_exit{code}`, `expect_stdout`, `expect_stderr`, and HC frames that the CLI must send, observed at the fake Hub.
- **UI** (`realizer`): the vector form of UI section 12: inputs, and the expected `RealizedTree` or the expected admission failure.
- **Client** (`client`): `client_load{document, kind_snapshots?, local_state?, viewport?, capabilities?}`, `client_act{node_id?, action}` and `client_expect{node_id?, observation}`, with the typed actions and observations of `botster_conformance::client` (schema `schemas/client-step.schema.json`). The steps are renderer-neutral: node ids from `realize`, no DOM selector. The ui ids that only a renderer proves use them, in `conformance/ui/`.

### 5.6 Rules for authors

1. One file per test id. A transcript covers the behavior that its test id names, and nothing more.
2. A transcript cites only its own clause and the clauses that its steps need. The pair writes the transcript from the clause text, never from a fake's behavior.
3. A transcript never asserts an order, a timing or a value that the contract leaves open.
4. Every valid frame in a transcript validates against the generated schema, and every typed negative frame fails it at its named rule (5.4). The runner checks this before it runs a step.
5. A transcript never asserts a count of steps, pumps, reads or frames that the contract does not fix. The runner's execution limits (6.1) are not contract bounds.
6. **Every expectation cites the clause that fixes its value.** Each `expect*` step, and each `expect_error` or `expect` field on another step, carries `"because"`: one citation `"<contract> <clause id>"` (for example `"HC EN-8"`), or an array of them when two clauses together fix the value (for example `["Codec TS-4c", "Core A3-1"]`). Every cited clause must contribute a sentence that fixes part of the value; an array is never a list of loosely related clauses. The reviewer opens that clause and checks that a sentence in it fixes the expected value. A value that no clause fixes goes to the steward (BUILD.md, Architecture facts, rule 3).
7. **No hand-written terminal bytes.** A transcript whose expected values are terminal bytes or terminal state (Core 5.1A encodings, snapshots, query answers) is `"fake": "structural", "ruling": "R-5"`, and its full run happens only against the real Core, where libghostty is the oracle. The README lists every real-only id.


## 6. Runner design

### 6.1 Shared core (`botster-conformance`)

- **Loading.** The runner crates embed their transcripts with `include_dir`. So a Phase 1 repo that pins a tag gets the transcripts of that tag, with no path setup.
- **Engine.** One interpreter runs the envelope, the matchers and the order constructs. Each suite gives a `StepDriver` trait object that executes its own step kinds.
- **Seeds.** A run has a seed set. The default tier, locally and in CI, uses seeds 0 to 31. A fake receives the seed at construction. A real implementation ignores it. A seed is an interleaving, not a rerun (BUILD.md, Testing rule 9). If the 32-seed default tier ever exceeds the 60 s budget, the lead reduces the default set and adds an in-process `sweep` job; the lead never moves fake runs to `slow`.
- **Results.** Each transcript gives `passed`, `failed{step, expected, received, seed}`, `inconclusive{step, limit}`, `not_applicable{feature}` or `unsupported_control{op}`. A run prints one line per failure and a summary. Only `passed` is a pass. Phase 0 acceptance (against the fakes) and Phase 1 acceptance (against a real implementation) need zero results other than `passed` for their clauses.
- **Execution limits.** The runner has infrastructure limits: a wall-clock limit per step against a real implementation, and an iteration limit per waiting step against a fake (so a fake bug cannot loop forever). These limits are not contract bounds. Their expiry gives `inconclusive`: it means "the run proved nothing", and the runner prints the state that it saw. A contract deadline (for example `startup`, `stop_grace` or `deadline_ms`) is asserted only through the event or frame that its clause defines.
- **Selection.** `--only <id or prefix>`, `--clause <id>`, `--seed <n>`. A failure prints the command that reproduces it.
- **cargo test.** Each runner crate has a macro, `conformance_tests!(make_subject)`, that expands to one `#[test]` per test id. A Phase 1 repo calls the macro once with its own subject factory (for Core: a `CoreHarness` factory, 6.2). Then each id is its own test in `cargo test` output.

### 6.2 Core runner

- **Operations go through `CoreApi` only,** with event-driven waits, as Core section 11 states.
- **Construction goes through a harness.** `Fn() -> Box<dyn CoreApi>` cannot express a failed `open`, two handles on one directory, a drop and a reopen with live workers, or an older worker binary. So the entry point takes a harness:

```rust
pub trait CoreHarness {
    /// Core LC-1, LC-2, 9B: one open with an explicit config; errors are results.
    fn open(&mut self, spec: &OpenSpec) -> Result<Box<dyn CoreApi>, CoreError>;
    /// A fresh data directory owned by this harness (kept across drop and reopen).
    fn data_dir(&mut self, name: &str) -> DataDirRef;
    /// The worker binaries the harness can offer: Current, and Previous (protocol T-1, AD-4).
    fn worker(&self, which: WorkerBuild) -> Option<WorkerRef>;
    /// Test controls (6.3): kill a worker or guardian, corrupt a registry row,
    /// make a registry write uncertain, hold a worker before payload launch (AD-7), and others.
    fn control(&mut self, op: &CoreControl) -> Result<(), Unsupported>;
}
pub struct OpenSpec { pub data_dir: DataDirRef, pub worker: Option<WorkerRef>, pub limits: serde_json::Value }
```

  - `OpenSpec.limits` is JSON, so a transcript can give a zero, a value out of range, or a cross-limit violation (9B), and expect `InvalidConfig`.
  - `worker: None` tests LC-1 (`MissingWorkerPath`).
  - A harness that cannot offer `WorkerBuild::Previous` reports the AD-4 skew transcripts as `unsupported_control`, which is not a pass.
  - `botster_core_conformance::from_make(make)` wraps a constructor of Section 11's form. It supports only the transcripts that use one default handle. Every other transcript gives `unsupported_control`. So the section 11 form still works, and the full suite needs the harness.
- **Core step vocabulary for handles.** `open{as, data_dir, worker, limits}` (a handle name; `expect_error` on the same step asserts a failed open), `drop{handle}`, and every operation step names its handle (`begin{handle, op}`). Three representative transcripts:

```json
{ "id": "conf::lm_1_zero_limit_is_invalid_config", "contract": "core", "clause": "9B", "subject": "core",
  "steps": [
    { "data_dir": "d" },
    { "open": { "as": "h", "data_dir": "d", "worker": "current", "limits": { "event_queue": 0 } },
      "expect_error": { "code": "InvalidConfig" } } ] }
```

```json
{ "id": "conf::lc_2_data_dir_is_exclusive", "contract": "core", "clause": "LC-2", "subject": "core",
  "steps": [
    { "data_dir": "d" },
    { "open": { "as": "a", "data_dir": "d", "worker": "current" } },
    { "open": { "as": "b", "data_dir": "d", "worker": "current" }, "expect_error": { "code": "DataDirInUse" } },
    { "list": { "handle": "a" }, "expect": [] },
    { "drop": { "handle": "a" } },
    { "open": { "as": "c", "data_dir": "d", "worker": "current" } } ] }
```

```json
{ "id": "conf::ad_1_running_adopts_running", "contract": "core", "clause": "AD-1", "subject": "core",
  "steps": [
    { "data_dir": "d" },
    { "open": { "as": "a", "data_dir": "d", "worker": "current" } },
    { "begin": { "handle": "a", "op": { "Create": { "id": "s1", "spawn": { "probe": [ { "hold": {} } ] } } } }, "bind": "$c" },
    { "pump_until": { "handle": "a", "event": { "Completed": { "op": "$c", "result": { "ok": {} } } } } },
    { "begin": { "handle": "a", "op": { "Start": { "id": "s1" } } }, "bind": "$st" },
    { "pump_until": { "handle": "a", "event": { "SessionState": { "id": "s1", "state": "Running" } } } },
    { "drop": { "handle": "a" } },
    { "open": { "as": "b", "data_dir": "d", "worker": "current" } },
    { "begin": { "handle": "b", "op": "AdoptAll" }, "bind": "$ad" },
    { "pump_until": { "handle": "b", "event": { "Completed": { "op": "$ad" } } },
      "ignore": [ { "SessionState": { "$any": true } } ] },
    { "get": { "handle": "b", "id": "s1" }, "expect": { "state": "Running" } } ] }
```

  The op and event spellings above are placeholders until `core-contract` lands; the pair writes them from the crate's serde form.
- **Probe scripts.** A session needs a program whose output and exit are known. A transcript describes it as a probe script: `print{bytes}`, `print_after_input{match, bytes}`, `exit{code}`, `signal_self{n}`, `ignore_sigterm`, `fork_child{script}`, `hold`, and similar. FakeCore interprets the script in memory (its scripted output and exits, Core section 11). Against the real Core, the runner starts `botster-conformance-probe` with the script as its argument. The same script then drives both. Services (Core section 10) use the same probe binary.
- **Clock.** FakeCore takes an injected clock. The `advance_clock` step moves it. Against the real Core, `advance_clock` sleeps only where the transcript says the step needs real time, and the step marks that with `real_time: true`.

### 6.3 Test controls

Some clauses need an event that the wire cannot cause: a crash and restart, a plugin reload, a transport loss, a hold on a completion, a full disk. HC section 6 already says that "a test that needs an oracle uses the Hub's test API, not the wire".

- Each runner crate defines a `TestControl` trait with one method per control. The list is closed and documented in the runner crate.
- Each control cites the clauses that need it.
- A fake implements every control.
- **A real implementation implements the controls by dependency injection, never by test branches in production code** (BUILD.md, Testing rule 8). The real implementation's test harness builds it with injected parts (a clock, a store, a transport, a plugin loader, a process spawner) and causes the event through those parts. A cargo feature or an environment variable that changes production behavior for tests is not allowed.
- A control is never a contract obligation for production. It exists only in test harness code.

### 6.4 HC runner

- **Subjects.** The runner drives a `HubSubject`: it opens connections over a binding adapter and it gets a `TestControl`.
- **Bindings.** A binding adapter gives the four TB-0 items: frame delimiting, peer authentication, progress, and the close signal. The transcripts run unchanged on every binding (TB-0). Phase 0 provides runner adapters and FakeHub servers for all three TB-0 bindings (steward ruling R-1.2):
  - `local-stream`: a real Unix socket (the TUI and the CLI test against it);
  - `webrtc`: with PR-0 to PR-9 pairing on test keys and TB-W2 route peers (Web tests its adapter against it);
  - `mcp`: the partial binding of HC 9.4 (the CLI `mcp-serve` tests against it).
- **Loopback is plumbing, not a binding.** An in-memory loopback connects a runner to a fake in the same process. It never appears in `welcome.features` as `binding:*`, and a loopback run never counts as a binding run (R-1.2).
- **Client-subject transcripts** (`subject: client`; for example the DS-4 fallback and the TS-8 and TS-9 close handling): the runner plays the Hub side from the transcript. The client under test is behind a `ClientSubject` trait. Phase 1 client repos implement it.

### 6.5 HP runner

- **Hub-side scripts** (`subject: plugin_hub`): the runner plays the plugin. The subject is a Hub endpoint.
- **Host-side scripts** (`subject: plugin_host`): the runner plays the Hub. The subject is a plugin host with a fixture plugin.
- **Lua binding scripts** (`subject: lua_host`): host-side scripts whose fixture plugin is Lua source in the transcript.

### 6.6 TypeScript runner

- `@trybotster/hub-conformance` reads the same `hc/` and `codec/` JSON files and has the same matcher engine and order constructs.
- It drives a `HubSubject` through the TypeScript client transport adapter boundary, the same boundary that Web uses (R-1.1). Phase 0 defines that boundary as the `HubTransport` interface in `@trybotster/hub-contract`: send a frame, receive frames, progress, and close (the TB-0 items as seen by a client). The Phase 1 client core implements its adapters against this interface. If the Client lead needs a different shape, the lead and the Client lead agree on it, and the interface changes in this repo.
- Phase 0 provides two implementations of `HubTransport` for the runner: an in-process connection to the wasm FakeHub, and a `local-stream` adapter for Node. Web adds its `webrtc` adapter in Phase 1 and runs it against the Rust FakeHub's `webrtc` server.
- A shared fixture set, `conformance/engine-vectors/`, tests the two matcher engines against each other, so the Rust and the TypeScript engines agree.

## 7. The fakes

### 7.1 Subjects and their fakes

| Subject | Phase 0 fake | Real implementation (Phase 1 or 2) |
|---|---|---|
| `core` | FakeCore | Core |
| `worker` (codec) | `botster-fake-route`, through FakeCore and through FakeHub | Core's session worker |
| `hub` | FakeHub (Rust and TypeScript) | Hub |
| `client` | none (R-1.4) | TUI, Web, CLI client core |
| `plugin_hub` | plugin-side FakeHub | Hub plugin supervisor |
| `plugin_host` | FakePlugin | the Lua host, or another host |
| `lua_host` | none (R-1.4) | the Lua host |
| `cli` | none (R-1.4) | the CLI |
| `realizer` | the reference `realize` (the vectors are the fake, UI section 16) | the Web and TUI renderers |

For a subject with no fake, Phase 0 still writes every transcript. The Phase 0 check of such a transcript is structural, over the stimulus classes of section 5.4: it parses; its valid frames pass the schemas; its typed negative frames fail the schemas at their named rules; its raw stimuli and raw wire bytes are kept as written; and the fake on the other side of the wire accepts the valid frames that it would receive and refuses the invalid ones as their clauses say (R-1.4).

**Client fault hooks (R-1.4).** FakeHub ships the `TestControl` hooks that the named semantic transcripts of HC section 12 need, so the clients can run them in Phase 1:
- drop the transport mid-route (TS-9, TS-11);
- a plugin reload between `describe` and dispatch (DS-6);
- drop a committed response (EN-7, EN-8);
- concurrent duplicate idempotency keys (EN-8);
- a resync while a frame is partly written, and a started frame that cannot finish (TS-4c, TS-9);
- an unknown verb or component (EN-6);
- internal queue loss on a watch (WT-4);
- a resync that discards notifications, so that `output_dropped` occurs (codec TS-4c, HC SF-5f);
- every seeded interleaving point of HC section 12.

### 7.2 Rules for every fake

1. **Clause only.** Every branch in a fake cites the clause that requires it, in a comment `// HC EN-8`. A behavior with no clause is not allowed. The reviewer checks this rule. An `xtask` lint checks that every cited clause id exists in the ledger.
2. **Deterministic.** A fake has no wall clock, no thread scheduling, and no OS randomness. Its only randomness is a ChaCha8 generator from the seed.
3. **Open orders are choice points.** Each place where the contract leaves an order open is one named choice point that cites its clause, for example `choose(OR_3, ...)`. The generator decides at each point. The lists in Core section 11, HC section 12 and HP section 19 define the required choice points. Each fake has a test that every listed choice point is reachable.
4. **Size.** FakeCore: 1.5k to 2k lines (Core section 11). The plugin-side FakeHub and FakePlugin: under 1.5k lines each (HP section 19). The client-facing FakeHub: small (HC section 12, guidance only).
   - **Two counts are reported** for each fake, by `cargo xtask fake-size`: (a) the fake's own crate, and (b) the total handwritten behavior that the fake runs, which adds the shared route endpoint for FakeCore and FakeHub. Both counts exclude tests, generated code, comments and blank lines. Neither count is redefined to fit a budget.
   - **A count over budget goes to the steward** through the lead, as a possible contract-size finding. The pair does not trim clause behavior to fit.
   - **Estimates before implementation** (non-binding; lines of behavior):

| Fake | Components | Estimate |
|---|---|---|
| FakeCore | registry and lifecycle 350; input and the IN rules 250; the event queue, classes and coalescing 250; admission, ops and errors 200; adoption 150; time and deadlines 100; services 250; choice points 100; route glue 100 | about 1,750 own; about 2,350 with the shared route endpoint (about 600) |
| client-facing FakeHub | envelope and requests 250; describe and kinds 250; watches and resync 250; idempotency and operations 200; sessions and routes glue 200; controls and choice points 150 | about 1,300 own; about 1,900 with the route endpoint (binding servers not counted: they are bindings, not Hub behavior) |
| plugin-side FakeHub | Hello, epochs and handshake 200; invoke, deadline and Cancel 250; scripted HostResults 150; limits and bounds 150; Shutdown, close and crash 150; choice points and shedding 100 | about 1,000 |
| FakePlugin | Ready and registration 150; the script engine for handlers 300; HostCalls 150; faults, crash and oversize 100; choice points 80 | about 800 (frame encoding is in `botster-plugin-contract`) |

   The FakeCore total with the route endpoint is above 2k in this estimate. The `fake-core` pair measures it. If the measured total is over budget, the lead refers it to the steward before the feature lands.
5. **Failure injection** uses the `TestControl` of section 6.3.
6. **Ablation proves the suite reaches the behavior** (vault: "conformance harnesses need an implementation ablation at the contract oracle"). For each clause group, the pair breaks one fake branch at a time, in a throwaway local change, and shows that the transcripts of that clause fail AT THE ASSERTION THAT CITES THE CLAUSE (not at an earlier step). READY FOR REVIEW lists each ablation and the failing step. A transcript that stays green under the ablation of its own clause is a finding. One ablation per assertion when a transcript has several. **Script-dependent clause branches:** when a fake branch applies a clause to a value that only the script supplies (for example a scripted worst-case admission size or a scripted typed outcome), no conformance transcript can assert it and stay valid against a real subject. The ablation target is then a unit test in the fake's crate whose name or doc comment cites the clause sentence and which asserts the comparison or mapping, boundary included. Wherever a relation holds for both the fake and a real subject, a transcript is still required. Step-level skips and fake-only conformance files are not allowed.
7. **No blind oracles** (vault: "a non completing shared fake adapter is a blind no progress oracle", "shared fake terminal adapter delivered frame bytes is a constant empty oracle"). A negative assertion (nothing happened, no frame, no progress) must use an observer that would change if the forbidden thing happened; the ablation of rule 6 shows it. FakeCore also injects each `CoreError` code on demand (Core section 11).

### 7.2b What the fakes do not model (steward ruling R-5)

- **No terminal model.** Screen contents, baselines, snapshots, ReadFacts values and notifications in output are script data that the transcript author keeps consistent (Core section 11: "scripted output and exits"; same reading as R-1.3).
- **No input encoder.** The Core 5.1A bytes have one observer, the PTY program, which is scripted. FakeCore implements the observable clauses of Key, Mouse and Focus (validation, the repeat rules and bound, not_ready, admission and guards, order and refusals, DP-12 focus tracking). Where a result counts encoded bytes, the session script supplies the bytes each semantic event writes.
- **The ids whose subject is the 5.1A bytes or write-time-mode encoding** carry `"fake": "structural", "ruling": "R-5"` and run in full only against the real Core.
- A shared encoding crate between the fake and the real worker is not allowed (R-5 item 3).
- The real worker should use a terminal library (the Core lead's choice in Phase 1), not hand-written encoding. The transcripts pin its bytes.

### 7.3 Shared route endpoint

`botster-fake-route` is one fake session-worker route: TS-3 framing, the baseline, `resync`, input refusals, file transfer, `output_dropped` (TS-4c) and close. It carries **scripted bytes only** (R-5, R-7): `setup.worker.script` holds the screen and history payloads, the modes, the outcome of each semantic input (the bytes it writes, or a typed not-written reason), the complete bytes of a paste and of a file path paste, and an explicit admission size for each. A `pty_output` control may carry `scripted: { modes?, notifications? }`. A real subject ignores all of it. FakeCore uses it for Core routes. FakeHub uses it for the route that a client attaches. So the codec transcripts run against one implementation through both fakes.

### 7.4 FakeHub structure and the TypeScript package

- **Sans-IO core.** The FakeHub core is a pure state machine: frames in, frames and route bytes out, with the seed and the injected clock. It has no socket, no thread and no clock of its own. It compiles to `wasm32-unknown-unknown`.
- **Binding servers** wrap the core, outside it: `local-stream`, `webrtc` and `mcp`. A binary, `botster-fake-hub serve --binding <name> --seed <n>`, runs one, so a client in any language can test against it.
- **TypeScript.** `@trybotster/fake-hub` wraps the core compiled to wasm (R-1.1). It is published from the same release tag as the Rust crate. `withSeed(n)` and every `TestControl` hook are reachable from TypeScript. The TypeScript runner drives it through `HubTransport` (section 6.6).

### 7.5 Plugin-side fakes (R-1.3)

- FakePlugin and the plugin-side FakeHub are scripted, as HP section 19 says. They hold the protocol state: handshake, registration, epochs, lanes, invocation, deadlines and `Cancel`, limits, frame bounds, `Fault` and channel close.
- They do not model Hub semantics such as storage, timers or sessions. In a Hub-side script for a semantic clause, the expected Hub frames are in the script. The Phase 0 self-test runs FakePlugin against the plugin-side FakeHub in loopback, and the FakeHub sends the scripted frames. The semantic proof comes from the real Hub in Phase 1.

## 8. Code generation

**Source of truth: the Rust declarations.** Every wire type derives `serde::Serialize`, `serde::Deserialize` and `schemars::JsonSchema` (schemars 1.x, JSON Schema 2020-12).

1. `cargo xtask codegen` writes `schemas/` from the Rust types.
2. `npm run codegen` in `ts/` generates the TypeScript types in `ts/packages/hub-contract/src/generated/` from the checked-in schemas, with `json-schema-to-typescript`. So the TypeScript comes from the same declarations, through the schemas.
3. Every generated file is checked in.
4. **The drift test.** `conf::ap_b_artifacts_match_code` and one test per client kind schema compare the schema that the code makes with the checked-in file. CI also runs both codegen steps and fails on `git diff --exit-code`.
5. **The transcripts check the schemas too,** by the stimulus classes of section 5.4: a valid frame must pass, a typed negative frame must fail at its named rule, and raw stimuli and raw wire bytes are not checked. Received frames are always validated. So a type change that breaks a transcript fails at once.

Artifacts with content that is not a type schema, such as `plugin-errors.json` (the registry of `kind` and every `detail.code`) and `lua-host-globals.json`, are generated from Rust constants in the same contract crate.

The kind schemas carry `schema_version M.m` (HC section 12, Versioning) as a schema annotation.

## 9. Test tiers and budgets (BUILD.md, "Testing: fast and lean")

**Default tier** (`cargo test`, `npm test`): in-process only. Every transcript against its fake, through the in-process loopback (section 6.4). Every unit test and drift test. No child process, no socket on disk, no daemon, no sleep, no polling, no global lock.

**Slow tier** (`cargo test --features slow` in the crates that have one, and `npm run test:slow`): the few named real-process tests, each citing its clause:
- the TB-0 transcripts over the real `local-stream`, `webrtc` and `mcp` binding servers of FakeHub (sockets and a real WebRTC peer);
- the probe binary (`botster-conformance-probe`) smoke test;
- the consumer smoke job of `release.yml`.
The slow tier runs at landing (in CI on every pull request to `main`, as its own job), not in the default tier.

**Budgets, enforced by `cargo xtask test-budget`** (it wraps `cargo nextest run`):
- Each default-tier test that runs over 2 s fails (nextest `slow-timeout = { period = "2s", terminate-after = 1 }`).
- The whole default tier must finish in under 60 s; the xtask measures it and fails over budget.
- Per-test times print in CI. The xtask writes `target/test-times.json`. CI uploads it as an artifact on `main`. On a pull request, the xtask compares with the latest `main` artifact and prints every test whose time doubled. A doubling is a review finding, not a failure.
- The xtask runs the tests in their own process group. After the run, it lists every process left in that group. A leftover process fails the run (BUILD.md, Testing rule 10). It never kills by name or pattern.
- `[profile.dev.package.*]` raises `opt-level` for the hot dependencies (serde_json, jsonschema, sha2, flate2), measured, not guessed (Testing rule 7).

**TypeScript:** vitest with `testTimeout: 2000`, a reporter that prints per-test times, and the same 60 s total check in `npm test`.

**Waiting.** A test waits on the real event: a completion, a frame, a pipe, a process exit. The only timer allowed is a marked deadline timer: `// timer: deadline — <reason>` (Testing rule 5). `cargo xtask taint` also fails on `sleep(` or `setTimeout(` in test code without that marker.

## 10. CI

One workflow, `.github/workflows/ci.yml`, with these jobs:

| Job | What it does |
|---|---|
| `manifest` | Checks the sha256 of every file in `frozen/final6/` against `MANIFEST.md`. |
| `taint` | `cargo xtask taint`: whole-word match of every token in `docs/taint-denylist.txt` over every tracked file except `frozen/` and `docs/`. BUILD.md says the check scans code and generated artifacts; `docs/` holds the process prose (BUILD.md quotes a banned name in order to explain the rule). So code, transcripts, schemas, generated TypeScript, CI files and the README are scanned. A hit fails the build. |
| `ledger` | Regenerates `conformance/ledger.json` and diffs it. Fails when a ledger id has no transcript, or when a transcript id is not in the ledger. |
| `rust` | fmt, clippy with `-D warnings`, and `cargo xtask test-budget` (the default tier with its budgets, section 9). This job runs every suite against its fake under seeds 0 to 31. |
| `slow` | The slow tier (section 9), with a deadline on the job. |
| `codegen` | Both codegen steps, then `git diff --exit-code`. |
| `ts` | Build, typecheck, and the TypeScript runner against the TypeScript FakeHub under the same seeds. |

During the build, `conformance/pending.txt` lists the ledger ids that have no transcript yet. The `ledger` job allows an id only if it is in that file, and the file may only shrink. Phase 0 is done when the file is empty, and then the file is deleted.

Local heavy runs go through `botsterq run --deadline <d>` (BUILD.md, practical rules).

## 11. How Phase 1 repos depend on this repo

**The repo is private.** Every consumer needs read access.

**Releases.** A release is a git tag `v0.x.y` on `main`. All crates and npm packages in the repo share that version. HC section 12 says that clients pin a released version, not a git rev. The tag is the release.

**Cargo.**

```toml
# Cargo.toml of a Phase 1 repo
[dependencies]
botster-core-contract = { git = "https://github.com/trybotster/botster-contracts", tag = "v0.1.0" }

[dev-dependencies]
botster-core-conformance = { git = "https://github.com/trybotster/botster-contracts", tag = "v0.1.0" }
botster-fake-core = { git = "https://github.com/trybotster/botster-contracts", tag = "v0.1.0" }
```

```toml
# .cargo/config.toml of the Phase 1 repo
[net]
git-fetch-with-cli = true
```

`git-fetch-with-cli` makes cargo use the system git and its credentials, so a private repo works on a machine that has `gh auth` or an SSH key.

**npm.** The packages publish to GitHub Packages under the scope `@trybotster`.
- **Publication.** A workflow, `.github/workflows/release.yml`, runs when a tag `v*` is pushed. It builds the wasm FakeHub core and the TypeScript packages, and publishes them with the workflow's `GITHUB_TOKEN` (`permissions: packages: write`). The wasm file is inside `@trybotster/fake-hub`, so one install gets both.
- **Consumer access.** For each consumer repo, the lead (or a repo admin) adds that repo under "Manage Actions access" of each package, with the read role. Then the consumer's own `GITHUB_TOKEN` can read the packages, and no personal token is needed in CI.
- **Consumer `.npmrc`:**

```
@trybotster:registry=https://npm.pkg.github.com
//npm.pkg.github.com/:_authToken=${NODE_AUTH_TOKEN}
```

- **Consumer CI step:** `permissions: packages: read`, and `env: NODE_AUTH_TOKEN: ${{ secrets.GITHUB_TOKEN }}` on the install step.
- **Local install:** `gh auth refresh -s read:packages`, then `export NODE_AUTH_TOKEN=$(gh auth token)`. A classic personal token with `read:packages` also works. A fine-grained token does not work for GitHub Packages npm.
- **Proof.** After it publishes, `release.yml` runs a consumer smoke job: a fresh directory with only the `.npmrc` above installs the released packages, loads the wasm, runs one transcript, and exits 0.

**Cargo in CI of another repo.** The consumer's `GITHUB_TOKEN` cannot read a different private repo. So a consumer repo has a secret, `BOTSTER_CONTRACTS_TOKEN`: a fine-grained token with read-only `contents` on `trybotster/botster-contracts` only. This token is separate from the npm credential above. The workflow step before cargo runs:

```sh
git config --global url."https://x-access-token:${BOTSTER_CONTRACTS_TOKEN}@github.com/trybotster/botster-contracts".insteadOf "https://github.com/trybotster/botster-contracts"
```

A read-only deploy key with an SSH dependency URL is the alternative for cargo.

**Running a suite against a real implementation.**

```rust
// tests/conformance.rs in the Core repo: the FULL suite (needed for acceptance).
// RealCoreHarness implements botster_core_conformance::CoreHarness (6.2):
// configured open, named data dirs, Current and Previous worker builds, controls.
botster_core_conformance::conformance_tests!(|| Box::new(RealCoreHarness::new()));

// Core section 11 faithfulness (R-1.5): the same suite against FakeCore.
// FakeCoreHarness is in the runner crate behind its `fake` feature (section 4); it wraps botster_fake_core::FakeCore.
mod fake { botster_core_conformance::conformance_tests!(|| Box::new(botster_core_conformance::fake::FakeCoreHarness::new())); }
```

`from_make` is a **partial-suite convenience** only:

```rust
// Runs only the transcripts that need one default handle. Every other
// transcript reports unsupported_control{op: "open"} (or the control it needs),
// which is not a pass. Not valid for acceptance.
botster_core_conformance::conformance_tests!(|| botster_core_conformance::from_make(|| Box::new(open_default_core())));
```

The same macro exists in each runner crate. The README gives the exact steps and the `TestControl` methods that each subject must implement.

## 12. Feature queue and staffing

A feature is a coherent group of clauses, one fake or one runner. Each feature gets a fresh pair: an implementer (`sonnet`) and a reviewer (`sol`), a branch `foundation/<feature>`, and its own worktree. One Sol ACCEPT lands a feature. The lead merges to `main` after the ACCEPT.

**Branch base.** Feature branches start from `main` and land on `main`, and release tags are cut from `main`. BUILD.md "Starting fresh" rule 1 (the orphan `v1` branch) applies to repos that contain old code. This repo was created clean, so it builds on `main` (orchestrator decision, 2026-09-30, recorded in BUILD.md rule 1). The taint check runs in CI on every merge.

Features that edit the same fake run one after another. Features in different lanes run in parallel. At most three pairs run at once (one shared Codex quota). A feature starts only when all its prerequisites have landed.

| Lane | Order | Feature | Content |
|---|---|---|---|
| — | 0 | `scaffold` | Workspace, `xtask` (ledger, manifest, taint, clause-citation lint, codegen frame), `botster-conformance` (the transcript model, the matchers, the order constructs, the seeds, the report, the macro), the transcript schema, `pending.txt`, CI. Every other feature waits for it. |
| — | 0b | `test-budget` | `cargo xtask test-budget` (nextest, the 2 s and 60 s budgets, the time report and the comparison with `main`, the leftover-process check), the `slow` tier frame and its CI job, the marked-timer check, the dev profile, the vitest settings for later TS packages (section 9). Runs beside wave 2. |
| A | 1 | `codec` | `botster-route-codec`, `botster-fake-route`, the codec transcripts. |
| A | 2 | `core-contract` | `botster-core-contract`: the types, `CoreApi`, errors, `CoreLimits`, features. |
| A | 3 | `fake-core` | FakeCore, the probe script, the Core runner, the probe binary, and the transcripts of Core sections 4 and 5. |
| A | 4–6 | `core-suite-*` | The rest of the Core transcripts in groups (sections 6 to 8; 9 to 9B; 10, A2 and A3), with the FakeCore behavior that each needs. |
| B | 1 | `hub-contract` | `botster-hub-contract`, the client kind schemas, the TypeScript generation. |
| B | 2 | `fake-hub` | The sans-IO Rust FakeHub core, the `local-stream` binding server, the HC runner, the R-1.4 fault hooks, and the transcripts of HC sections 2 to 5 and the section 12 named transcripts. No `webrtc` or `mcp` work (B7, B8). |
| B | 3–5 | `hc-suite-*` | The rest of the HC transcripts in groups (sections 6 to 8; 9 to 11 and Appendix A; A1 to A3), and the client-subject transcripts. |
| B | 6 | `fake-hub-ts` | The wasm build of the FakeHub core, `@trybotster/fake-hub`, `HubTransport`, the TypeScript runner, and the engine vectors. |
| B | 7 | `fake-hub-webrtc` | The `webrtc` binding server and runner adapter: PR-0 to PR-9 pairing on test keys, TB-W2 route peers, and the TB-0 transcripts on this binding. |
| B | 8 | `fake-hub-mcp` | The partial `mcp` binding server and runner adapter (HC 9.4 and section 10), and the TB-0 transcripts on this binding. |
| C | 1 | `plugin-contract` | `botster-plugin-contract` and every HP artifact (Appendix B, A1, A2). |
| C | 2 | `plugin-fakes` | FakePlugin, the plugin-side FakeHub, the HP runner, and the transcripts of HP sections 2 to 8. |
| C | 3–6 | `hp-suite-*` | The rest of the HP transcripts in groups (sections 9 and 24; 10 to 13; 14 to 18 and 21; A1 and A2). |
| C | 7 | `lb-suite` | The Lua binding transcripts. |
| D | 1 | `ui-contract` | `botster-ui-contract`, the reference `realize`, and the vectors. |
| D | 2 | `cli-suite` | The CLI transcripts. |

**Prerequisites** (a feature waits for every feature it lists):

| Feature | Prerequisites |
|---|---|
| A1 `codec`, D1 `ui-contract` | `scaffold` |
| A2 `core-contract` | A1 |
| B1 `hub-contract` | A1, D1 |
| C1 `plugin-contract` | D1 |
| A3 `fake-core` | A2 |
| B2 `fake-hub` | B1 (the route endpoint of A1) |
| C2 `plugin-fakes` | C1 |
| B6 `fake-hub-ts` | B2 |
| B7 `fake-hub-webrtc`, B8 `fake-hub-mcp` | B2 |
| C7 `lb-suite` | C2 |
| D2 `cli-suite` | B2 |
| each later suite group | the fake of its lane |

**Schedule.** Wave 1: `scaffold` alone. Wave 2: A1 and D1. Wave 3: A2, then B1 and C1 as slots free. After that, each lane runs its own sequence, with at most three pairs at once. The lead splits a group further when a group is too large for one review.

**What each pair receives:** BUILD.md, this file, its exact clause list (from the ledger), the messaging rule, and the stealing rule. The pair reports to the lead only QUESTION, BLOCKED or DONE. A question about clause meaning goes to the lead, and the lead asks the steward.

The `webrtc` and `mcp` binding features are inside the Phase 0 done-gate. The steward leaves this timing to the lead (R-1.2). Inside the gate, Web and the CLI start Phase 1 with their bindings ready.

## 12b. Prior art (BUILD.md rule 0; lead's pass, 2026-09-30)

Pinned sources: botster-core @ 72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c, botster-hub @ 9bc6cd7d4963ae5cccd642931619b9cac5c8daac, botster-web @ 029a9521c8f641556118b7c590214e0483472559, the vault (botster-architecture.md, botster-v1-contracts-and-conformance.md).

| Area | What exists | Verdict | Why |
|---|---|---|---|
| Route codec (`botster-route-codec`) | botster-core `crates/botster-terminal-protocol` and `-client`: the retired "scheme 2" wire (binary header with scheme, flags and length; binary bodies; u32 mode bits; binary key and mouse input; epoch envelope) | **KEEP the new crate.** Steal at most the shared `Arc<[u8]>` payload idea; copy the `PUBLIC_API_ALLOWLIST` test pattern. | Rev36 shares no bytes with the old wire: it removes the header (TS-3), uses JSON bodies (A.1), JSON ModeFlags (A.6) and JSON input (A.5). The old crate has better typed errors and zero-copy payloads; the new crate has per-type bounds, the ignorable bit, deflate caps, file chunks and output_dropped. |
| Conformance harness and xtask | botster-core `crates/botster-core-test-support` (bounded waits, unique stamps, FIFO gates, worker manifest check, terminal fakes); botster-hub `crates/botster-hub-test-support/src/isolated_hub.rs` (process census and group teardown, with sleep-poll waits and test switches) | **KEEP the new harness.** At the first real-process slow-tier driver, steal `unique.rs` (macOS clock collisions), `fixture_gate.rs` (the FIFO gate and its external-command gotcha) and the worker-manifest hash check from `real_worker.rs`, each with trailers. Port nothing from `isolated_hub.rs`. | The old terminal fakes model the screen (forbidden by R-5); the Hub harness polls with sleeps, keeps global state and has test switches (forbidden by the testing rules). The new `Deadline` gives `inconclusive` instead of a panic. |
| Generated TypeScript | Two handwritten Rust TS emitters (botster-hub `crates/botster-hub-client/src/lib.rs` `daemon_protocol_typescript()`, and `crates/botster-ui-contract/src/assets.rs` `typescript_declarations()`/`json_schema()`), which already drifted once; Web copies the files with a sha256 PROVENANCE.json | **KEEP the schema pipeline.** ADOPT the `@trybotster/hub-test-support` package shape (ESM, `exports` with `types`, npm workspaces, Node-loadable) and wasm `--target web`. KEEP `HubTransport` narrow; Web adapts its existing client. | The pipeline removes the handwritten emitters. The vault lessons apply: serde optionality must reach TS, and every wire enum has one variant inventory that the generator iterates. |

**Changes this pass makes** (feature `prior-art-fixes`, lane —): in `botster-route-codec`, zero-copy payloads instead of `HexBytes(Vec)`, deflate validation that keeps or returns the inflated buffer, a read cursor in `StreamReader` (no front drain), a typed `DecodeError` detail, and a public-API allowlist test; in `xtask test-budget`, zombies count as leftovers (attributed by PPID on Darwin), a positive control for a child that escapes with `setsid`, the TERM → grace → KILL → reap order for slow-tier drivers, and a short-path temp-root helper (macOS socket paths are limited to about 104 bytes).

## 12c. Remaining Phase 0 queue (after the 2026-10-01 decision)

| Feature | Content | Proof in Phase 0 |
|---|---|---|
| `replacement-map` (running) | conformance/replacement-map.json, the generated docs/fake-replacement-map.md, `cargo xtask replacement-map --check` | machine check plus Sol spot checks |
| `prior-art-tooling` (running) | design 12b fixes, proptest, bolero, cargo-mutants in CI | tests |
| `hp-calls-a` (running) | HP section 9 calls | the HP frame players |
| `hc-runner` | salvages foundation/fake-hub at 724892b: the HC runner, the client-subject player (the runner plays the Hub), R-1.4 faults as player scripts, the 68 HC and 6 UI hub transcripts; drops the FakeHub core | structural plus the client player |
| `ts-player` | the Rust transcript player in server role compiled to wasm (HC A5-2), the TS runner over Web's HubTransport, the engine vectors in TS | the TS runner against the player |
| `core-groups` (2–3 features) | the remaining Core transcripts (routes 5.3/5.4, reads and events 6.x, adoption 7, time 8, services 10, A2–A4) | structural (FakeCore is frozen); full runs against the testkit in Phase 1 |
| `hc-groups` (1–2 features) | the remaining HC transcripts (sections 6–11, A1–A3) | structural plus the client player |
| `hp-groups` (3–4 features) | the remaining HP transcripts (sections 9.8–9.14, 10–13, A1, A2) | the HP frame players, or structural for semantic Hub outcomes (R-1.3) |
| `lb-suite`, `cli-suite` | the Lua binding and CLI transcripts | structural |
| `phase1-readme` | the README for Phase 1 leads: how to depend on the crates, implement the harness traits (Core testkit, Hub subject), run the suites, the slow tier | review |

No new fake-fidelity work. The client-facing FakeHub, the wasm FakeHub and the fake `webrtc` and `mcp` bindings are dropped; the real bindings are proven by the real Hub in named slow-tier runs.

## 13. Decisions and open items

- **Q1 to Q4 are settled** by steward ruling R-1 (`docs/steward-rulings.md`).
- **FakeCore lives in this repo** (R-1.5). Core's CI satisfies Core section 11 with one test file that runs `botster-core-conformance` against both the real Core and `botster-fake-core` (a dev-dependency). The Core lead may move FakeCore into the Core repo later. The README states this.
- A fake that exceeds its size budget is reported to the steward as a possible contract-size finding (Core section 11, HP section 19).

## 14. UI amendments: prior art and infrastructure decisions (BUILD.md rule 0)

Owner: the ui-amendments pair. Date: 2026-10-01.

**Sources read.**
- The vault notes of the botster-v1-ui-realization map: one pass as if local changes were applied, retained user-owned state, capability and display scope, container roles, copy keys, and absent-list defaults. The implementation follows them: `realize` repeats its pass until no new draft is unset (`realize.rs`), drafts, overlays and errors are kept and not shown, and an absent `parts` or `widgets` list means all while an absent `features` or `screens` list means none.
- `botster-hub@9bc6cd7d4963` and `@9e1ba42803ee`, `crates/botster-ui-contract` (`lib.rs`, `assets.rs` vectors, `tests/ui_contract_test.rs`). The node-id scheme and its UTF-8 byte lengths came from there in the ui-contract feature (ruling R-2.1); this feature reused nothing new. The old node validation tests check the old validator, not the realized-tree rules of ui 1.4, so none is ported (BUILD.md rule 4).
- `frozen/final9/audits/client-parity-20260930.md`: C-62 (plugin surfaces rendered from UiNode trees), C-66 (embed) and C-67 (action result `navigate`). They map to UI-RZ-2, UI-EM-1 to UI-EM-15 and UI-AC-6.
- `botster-tui-kit` `renderer.rs`: no evidence for the form-section label prop (the contract text decides: UI-NV-6 `label`). `botster-web` @ 029a952 was not read; renderer behavior is written from the clause text only.

**Reused.** The UiNode vocabulary and the id function (ui-contract feature). Nothing is stolen in this feature, so no `Stolen-From` trailer.

**Custom infrastructure, and why.**
- RFC 8785 text of a value (UI-FD-2): the member sort (UTF-16 order) and string escaping are small and are fixed by the clause; the number digits come from the `ryu-js` crate, which implements the ECMAScript `Number::toString` rules. A hand-written shortest-digit formatter was rejected: it differs from Node on tie cases.
- URL validation (UI-FD-1): the `url` crate (WHATWG parser, punycode host).
- The client step vocabulary (`botster_conformance::client`) is new: no existing harness expresses renderer observations, and the lead ruled on its shape.

## 15. Hub contract: prior art and decisions (BUILD.md rule 0)

Owner: the hub-contract pair. Date: 2026-10-01.

**Sources read.**
- The vault: *generated typescript dtos must encode serde field optionality*, *terminal wire enums and TypeScript unions share one variant inventory*, *generated dto drift tests need symmetric field and type checks*, *botster HP session rows and HC session resources share one schema*.
- `botster-web@029a952`, `test-support/hub-test-support/package.json`: the package shape (ESM only, `exports` with a `types` condition, plain npm workspaces, no `workspace:` protocol).
- `botster-hub@9bc6cd7d4963`, the two handwritten TypeScript emitters (`daemon_protocol_typescript()` and the ui-contract `typescript_declarations()`): read for what they cover, not ported.
- The sibling crates of this repo (`botster-route-codec`, `botster-core-contract`, `botster-plugin-contract`): the `open set` and `NULL_ALLOWED` patterns.

**Reused.** Nothing is stolen, so no `Stolen-From` trailer. The ideas carried over from the sibling crates are the non-exhaustive reader (`lenient!`, one macro here: `open_enum!` for unit sets) and the null-stripping schema transform.

**Rejected.**
- The two handwritten emitters: replaced by the schema pipeline (schemars, then json-schema-to-typescript), because they drifted once.
- ts-rs, specta and typeshare: a second emitter next to schemars (docs/tooling.md).
- A hand-written `describe` for Hub kinds: `KINDS` in `registry.rs` is the one table (verbs, actions, effects, argument and result types), and the kind schemas carry it as `x-verbs` and `x-actions`.

**Decisions.**
- **One inventory per open set.** `open_enum!` declares the enum, `ALL`, `NAMES`, serde and the schema together. The TypeScript inventory is read from the schema, and a test checks the generated union against it (`test/drift.test.ts`). A data-carrying set (`CloseReason`, `PackageStatus`, `UpdateState`) uses `lenient!` and lists its names once.
- **Open sets are `anyOf: [enum, string]`**, so a received frame with a value from a later version still validates (HC EN-6). The TypeScript generator adds `(string & {})` to keep the names in editors.
- **Optionality is serde's.** A field with no value is absent (HC A.1). `opt_no_null` refuses `null` for such a field, and the schema transform removes `null` from every optional member, except a merge patch (VB-4), which marks its members `x-null-allowed`.
- **Wire layouts that HC leaves open** are isolated, each in one type, and marked `pending steward ruling H2`: the `frame` tag of watch sub-frames, the `event_gap` frame (`"frame": "gap"` on an `event` frame), the pairing and `route_connect` frames (`"frame": "<name>"`), the placement of `if_rev` inside `args` of `update`, and the unit of every timestamp except `expires_at` (Unix milliseconds, `Timestamp`).
- **Schema files.** `envelope.schema.json` holds the frames, the handshake, `describe`, the watch and event frames and the verb arguments. One file per kind holds the row as its root, the arguments and results of its verbs and actions in `$defs`, and the registry entry as `x-kind`, `x-schema-version`, `x-verbs` and `x-actions`. HC A2 adds no kind, only actions and fields, so there are 16 kind files.
- **Name clashes with ui-contract** are resolved in this crate (`EventPriority`, `SoftwareChannel`), because the TypeScript generator merges the definitions of the `view` kind with the ui types into one namespace.
- **`session` moved here.** `SessionRow` and its parts are defined in `botster-hub-contract`, and `botster-plugin-contract` re-exports `SessionRow`. The plugin crate keeps its own `Interaction`, `Presentation` and `SessionTypeSource`, which the HP manifest and `SessionTypeRow` use. The HP artifact `plugin-calls.schema.json` changed in wording and in openness only (the `activity`, `interaction` and `presentation` sets are non-exhaustive now).
- **Not in this crate:** the plugin-failure mapping table of section 11 (it is Hub logic over HP kinds), the pairing bundle codec and the transcripts of PR-7b (crypto, in the fake and the suite), and the `hub.toml` keys (CF-2: configuration, not wire).
