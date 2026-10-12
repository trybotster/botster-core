# Tooling review (Phase 0)

Owner: the Foundation lead. Date: 2026-09-30. Rule: BUILD.md "Starting fresh" rule 0 (prefer established, maintained tools; hand-rolling needs a recorded reason). Versions were checked on 2026-09-30 against crates.io, the npm registry and the projects' documentation.

**Already adopted:** the `base64` crate (the `$b64` matcher decodes captured tails), the `data-encoding` crate (`BASE32_NOPAD`, the HC PR-2 pairing bundle, ruling R-39), cargo-nextest (test budgets), vitest (TypeScript tests), schemars 1.x (JSON Schema from Rust), the `jsonschema` crate (validation in Rust), json-schema-to-typescript (TypeScript from the checked-in schemas).

## Decisions

| Area | Tool (version) | Verdict | Reason |
|---|---|---|---|
| Property tests, Rust | proptest 1.11.0, test-strategy 0.4.5 (derive) | **ADOPT** | Codec round-trips, decoder robustness on arbitrary bytes, and clause invariants on stable `cargo test`. Replaces hand-picked round-trip examples. Default tier: `cases = 64`, a checked-in failure-persistence file; the slow tier raises the count. |
| Property tests, TS | fast-check 4.10.2, @fast-check/vitest 0.5.0 | **ADOPT** | The same properties for the TypeScript packages, with seeded replay. |
| Generators from JSON Schema | json-schema-faker 0.6.3; json-schema-fast-check 0.0.6 | **REJECT** | The faker does not shrink or follow fast-check's seed; the other is unmaintained since 2019. Rust generators come from the types (derive), and every generated value is validated against the checked-in schema with `jsonschema`, so the test is not its own oracle. TypeScript uses a small schema-to-`fc.Arbitrary` walker over the keywords schemars emits. **Hand-rolled, with reason:** no maintained library does this with shrinking. |
| Mutation tests, Rust | cargo-mutants 27.1.0 | **ADOPT**, at landing | `cargo mutants --in-diff` on the changed crates of a pull request, plus a weekly full run on `crates/botster-fake-*` and the codecs. It proves that the transcripts catch a broken fake, which a suite written by the fake's own author cannot prove. Surviving mutants are review findings. |
| Mutation tests, TS | StrykerJS 10.0.0 (vitest runner) | **ADOPT later** | Trigger: when `fake-hub-ts` lands. `--incremental` with a cached report in CI. |
| Fuzzing | bolero 0.13.6 | **ADOPT** | One harness per decoder (route frames, plugin frames, the manifest parser). It runs as a property test on stable in the default tier, and as a fuzzer (`cargo +nightly bolero test -T 60s`) in a landing or nightly job. |
| Fuzzing | cargo-fuzz 0.13.2; cargo-afl 0.18.2 | **REJECT** | cargo-fuzz needs nightly for every run; bolero gives the same libFuzzer targets and also runs on stable. AFL adds setup with no gain over bolero. |
| Binary framing | deku 0.20.3; binrw 0.15.2 | **REJECT** | Our frames are one type byte (route) or length, lane and type (plugin) plus a JSON or raw body. A derive would cover about ten lines. The hard rules (a per-type bound before buffering, the HP length check before allocation, the ignorable bit, the inflate cap, strict JSON) stay hand-written in either case, and the derive readers produce owned data against the zero-copy payloads. **Hand-rolled, with reason:** these rules are where the clause citations live. |
| TS generation | ts-rs 12.0.0; specta 2.0.0-rc.25; typeshare 1.0.5 | **REJECT** | Each is a second emitter next to schemars, so the TypeScript and the HP Appendix B schemas could drift. specta is still a release candidate. The schema pipeline keeps one source. |
| TS runtime validation | Ajv 8, `Ajv2020` | **ADOPT** | Validates frames in the TypeScript runner and FakeHub wrapper against the same checked-in schemas (the TypeScript mirror of `jsonschema`). |
| TS runtime validation | zod `z.fromJSONSchema` (4.6) | **REJECT** | Experimental, and a third derived form of the same schemas. |
| Protocol spec | AsyncAPI 3.1.0 (asyncapi-rust 0.5.0) | **REJECT for Phase 0** | It would give documentation only. Our raw-byte frames, the TS-3 framing, the HP lane, type and length header, the Unix-socket and WebRTC bindings have no AsyncAPI binding (only ignored `x-` extensions), and no consumer has asked for it. Revisit if a Phase 1 or 2 consumer needs generated protocol docs: a small xtask over the existing schemas is then cheaper than a derive layer. |
| Contract testing | Specmatic (AsyncAPI support is commercial) | **REJECT** | Fixed broker transports only (Kafka, JMS, WebSocket and similar); it cannot drive in-process fakes, a Unix socket or WebRTC, and it has no seeds, order constructs or clause citations. |
| Contract testing | Pact V4 (messages, plugins) | **REJECT** | Consumer-driven: the contract is what consumer tests record. Our contract is frozen markdown with clause ids that both sides are judged against. Its matchers have no cross-message binding, unordered sets or barrier quiet checks. |
| Spec authoring | TypeSpec 1.16.0 | **REJECT** | A third language between the frozen markdown and Rust; serde behavior would be modelled by hand. Rust-first keeps codec, fakes and schemas as one set of declarations. |
| Lua syntax check, Rust | full_moon 2.2.0 (`lua54`) | **ADOPT**, dev-dependency of `botster-lua-conformance` | The fixture plugins of the Lua binding transcripts are Lua source inside JSON. A parse test finds a typo in a script before a real host meets it. A pure-Rust parser needs no C build and runs no plugin code. **Rejected:** mlua (it builds and runs Lua, so it would become a second Lua host in this repo, BUILD.md "no re-implementation of the Lua host") and a hand-written check. |
| Concurrency, Phase 1 | shuttle 0.9.4, shuttle-tokio 1.0.0 | **ADOPT in Phase 1** (named in BUILD.md) | Randomized and PCT schedules for the tokio Hub and for the Core handle with its cross-thread wake handle, behind a `shuttle` feature. Not used in this repo: the fakes are deterministic and single-threaded. |
| Concurrency | loom 0.7.2 | **REJECT unless** | Only if a Phase 1 repo writes its own lock-free primitive (for example an atomic wake handle). No tokio support; exhaustive search does not scale to components. |
| Snapshots | insta 1.48.0 | **REJECT** | The generated-artifact drift test plus `git diff --exit-code` already covers generated files, and snapshots must never replace clause-derived expectations. |

## What a tool could replace in the transcript engine

Nothing in `crates/botster-conformance` is replaced:
- **Matcher:** its schema arm already uses `jsonschema` (Ajv2020 in TypeScript). Subset matching with extra keys allowed (HC EN-6), `$name` binding across steps, `$absent`, `$one_of`, `$bytes_hex` and `u64_decimal` exist in no tool.
- **Order constructs:** `expect_set`, `expect_eventually` with `ignore`, `expect_quiet_until` with a barrier, and per-stream relevance exist in none of Specmatic, Pact or AsyncAPI tooling. They encode what order the contract does not promise.
- **Seeds and drivers:** seeded interleaving lives inside the fakes (OR-3, MD-1, IV-6, EV-4 orders); a ChaCha8 seed is all it needs.
- **Stimulus classes** (design 5.4): unique to this design; their schema checks reuse `jsonschema` and Ajv.

**Hand-rolled, with reason:** the transcript engine. No contract-testing framework supports spec-first clause transcripts with unordered expectations, barrier silence and seeded fakes.

## Where the adopted tools run

| Tool | Default tier (every push) | Landing (pull request to `main`) | Weekly |
|---|---|---|---|
| proptest, fast-check | yes, capped cases and pinned seeds | raised case counts in the slow tier | — |
| bolero | as property tests | fuzzing, 60 s per decoder target | longer fuzzing |
| cargo-mutants | — | `--in-diff`, changed crates | full run on fakes and codecs |
| StrykerJS | — | after `fake-hub-ts` lands, incremental | — |
| Ajv2020 | yes (TypeScript runner and wrapper) | — | — |
