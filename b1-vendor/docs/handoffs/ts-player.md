# Handoff: ts-player (paused 2026-10-02)

## Goal and clause list

- Pin (the plan): `~/botster-sessions/pins/foundation-ts-player.89dc22bb.md`. Design 12c; HC Amendment 5 (`frozen/current/hub-client-contract-v1-amendment-5-candidate2.md`): A5-2 (one Rust transcript player, compiled to wasm), A5-3 (shared transcripts and client-only transcripts), A5-4 (producer variation and delivery variation); R-1.4 (the fault list, `docs/steward-rulings.md`).
- Scope in order: (1) the script format for scripted server answers; (2) the Rust server-role player with the fault scripts moved from hc-runner; (3) the player compiled to wasm with a TS package; (4) the TS runner over `HubTransport`, with TS parity for every engine addition; (5) CI.
- The feature lands no new ledger ids. The `a5_*` ids stay pending for hc-groups (they need a statement-step vocabulary). This feature's tests are their proofs.
- The decisions and the full plan are in `docs/ts-player-notes.md` on the branch. That file is the design record: it holds the plan, the prior art and the READY-1 ablation table. The format is also in design section 5.7 (`docs/foundation-design.md`).

## Branch and head

- Branch `foundation/ts-player`, pushed. Head **95d9dbb** (a WIP commit). Worktree: `~/botster-sessions/trybotster-botster-contracts-foundation-ts-player`.
- **31086f9** is the last clean, READY-1-candidate head: the READY-1 work merged with origin/main at 8a8cbc8 (main then had lb-cli-suite, including `$set`).
  - On 31086f9: focused tests are green (`cargo test -p botster-conformance -p botster-hub-conformance`) and clippy is clean on both crates.
  - **No botster-gate has run yet.** No READY was sent.
- **95d9dbb** = 31086f9 plus the READY-2 and READY-3 DRAFTS. This code was **never compiled or tested** (details below).

## Done (READY 1 content, on 31086f9)

1. **The format** (`crates/botster-conformance/src/serve.rs`, design 5.7):
   - Optional `serve` on the steps `expect`, `expect_set`, `expect_eventually`, `expect_quiet_until`, `expect_closed` and `route_expect`. A `control` step's `expect` is not a served step.
   - Forms: `[actions]`, or `{"alternatives": [[actions]..], "choice"?}`.
   - Tagged actions: `frame`, `raw_hex`, `wire_hex`, `cut`, `route_frame`.
   - `serve::check` runs the step through the engine's own `run_step` over what the client sees, so the same run validates the answer and binds the variables. Every served item must be used.
   - `parse_all` refuses a shared `choice` name with unequal counts; `combinations` lists every combination.
   - `Choices` picks the alternatives by seed, with optional forced indices. `interleave` is the A5-4 permute-only delivery.
   - The engine (`order.rs`) accepts `serve` on the order steps and ignores it.
   - Vectors: `conformance/engine-vectors/serve.json` (Rust only), with a test that every flipped case is reported.
2. **The player** (`crates/botster-hub-conformance/src/hc/server_player.rs`):
   - `ServerPlayer` is sans-IO. It walks its own copy of the transcript and holds no Hub logic.
   - `PlayerSubject` implements `HubSubject`, so the existing `HubDriver` plays the client in the tests.
   - It refuses a transcript that is not servable, with the reason: no serve, or a partial serve; an observing control; `uid`, `credential` or `binding`; `start: false`; `ack`; a subject other than `hub`.
   - `ACCEPTED_CONTROLS`: `hold_completion`, `release_completion`, `shutdown_now`, `reload_plugin` and `watch_queue_loss` are no-ops whose consequences the serves script. `drop_response` cuts the connection of each of the next N requests.
   - TS-4c rule: after a `wire_hex` that leaves a frame unfinished, only `wire_hex` or `cut` may follow.
   - Delivery (A5-4): an `expect_set` serve interleaves across streams, and each stream keeps its order. All `res` frames are one stream, because response order is a producer variation (EN-3). Each watch (`watch_id`) is a stream. Each event `stream_id` is a stream. `welcome` and `close` stay fixed. Reads return a seeded byte count.
   - Variables in served frames come from the player's bindings. `ended()` and `connect()` first catch the walk up.
3. **Serves on 9 shared transcripts** (values come from the base fixture and the HC section 11 error table): `en_1_request_shape`, `en_3_one_response_while_writable`, `en_3_shutdown_is_effect_unknown`, `en_5_frame_bound`, `en_6_unknown_verb_unsupported`, `en_8_completed_key_returns_first`, `en_8_pending_key_joins`, `hs_1_hello_first`, `hs_2_selects_highest_common`. `en_1`, `en_3_one_response` and `en_8_pending` carry response-order alternatives.
4. **Tests:**
   - `crates/botster-hub-conformance/tests/server_player.rs` (30 tests):
     - every served transcript runs under every combination and seeds 0 to 31, and passes only when the runner passes AND the verdict is `Done`;
     - the negatives of A5-2;
     - the refusals;
     - a choice held across steps;
     - interleave and chunking;
     - route fixtures: the TS-4c resync after a completed partial write, the negative of a resync served too early, and a frame that cannot finish followed by TS-9 `route.ended` `write_failed`;
     - `watch_queue_loss`;
     - route sends and raw answers;
     - walk errors;
     - the `PlayerSubject` controls.
   - `crates/botster-conformance/tests/serve.rs`: choices and interleave.
5. **Docs:**
   - design 5.7;
   - a "Server-role player" section in `docs/hub-subject-controls.md`;
   - `docs/ts-player-notes.md` (plan, prior art, READY-1 ablations: all 13 ablations fail their test).
   - `.cargo/mutants.toml` gains 2 equivalent-mutant entries with their reasons: `ServeDriver` `has_feature`/`has_control`, and `hc_schemas`.
6. **Mutants:**
   - The first local run on 87d9bd8 had 25 misses.
   - Tests closed every miss, except the 2 equivalent entries above.
   - A targeted re-run on the previously missed functions was cancelled by the pause after 58 mutants: 0 missed so far. **The gate's full mutants job has not run.**

## Left

- **READY 1:** on 31086f9, or on a re-merge with main if main gained code:
  - run `~/botster-sessions/shared/tools/botster-gate <worktree>`;
  - close any missed mutant;
  - send the reviewer READY 1 with the sha, the gate log, the Prior art note and the ablation table, all taken from `docs/ts-player-notes.md`.
- **READY 2 (drafted in 95d9dbb, never built):**
  - `crates/botster-hc-player-wasm`: wasm-bindgen glue.
    - Exports: `Player`, `servable`, `combinations` and `runClientOnly`, with a JS `ClientSubject`.
    - Lints: `unsafe_code = "deny"` instead of the workspace `forbid`, because the macro emits `unsafe`. This needs checking.
  - Workspace: `wasm-bindgen = "=0.2.129"` in `Cargo.toml`.
  - `ts/packages/hc-player`:
    - `scripts/build-wasm.mjs` runs cargo for the wasm32 target, then `wasm-bindgen --target web`, into `wasm/` (gitignored).
    - `src/index.ts` holds `init(source)` (explicit), `ServerPlayer`, `PlayerTransport` (the in-process `HubTransport`, binding `in-memory`, `ready()`, `sendWire`), `PlayerHarness` and `runClientOnly`.
    - Tests: vitest (`player.test.ts`, and `client-only.test.ts` with `toy.ts`, a port of the Rust toy with its defects), a plain-Node consumer test, and `test-vite/build.mjs` (a Vite 8 build with no plugin, which must emit and reference the `.wasm`).
  - CI:
    - `rust-toolchain.toml` gains `targets = ["wasm32-unknown-unknown"]`;
    - `ci/remote/Dockerfile` gains `-t wasm32-unknown-unknown` and `wasm-bindgen-cli@0.2.129`;
    - the xtask `ts_job` checks `wasm-bindgen` (SKIPPED locally, fail on CI) and runs `test:consumer` for hc-player;
    - `ts/package.json` pins the dev dependency `vite 8.3.2`;
    - `docs/tooling.md` gains rows for wasm-bindgen and Vite.
  - `Cargo.lock` in the WIP comes from a patch made before the merge, so regenerate it.
  - `ts/package-lock.json` was NOT updated: `npm install` is needed for the 2 new workspaces and for vite.
- **READY 3 (drafted in 95d9dbb, never run):** `ts/packages/hub-conformance`:
  - `matcher.ts`: a port of matcher.rs that uses own-property lookups and strict base64. **It lacks `$set`**, which landed on main with lb-cli-suite: port it from `crates/botster-conformance/src/matcher.rs` `assign_set`, and check that `matchers.json` now has `$set` cases.
  - `order.ts`: async; `ack` and `$item` included.
  - `stimulus.ts`;
  - `schemas.ts`: Ajv through `@trybotster/hub-contract` `validateWith`, with the Rust reference names;
  - `runner.ts`: `HubDriver`, `HubHarness` with `idle()`, and `runTranscript`;
  - tests: `vectors.test.ts`, `props.test.ts` (fast-check) and `runner.test.ts` (every served transcript × every combination × seeds 0 to 31 against the wasm player, plus negatives).
- **Open decision:** StrykerJS. `docs/tooling.md` says StrykerJS applies "after `fake-hub-ts` lands, incremental", and ts-player is that successor. If unsure, ask the lead.

## Last review verdict and open findings

- Reviewer: `ts-player — reviewer`, session sess-1790955508-00ca-975fecd56eaac180be3e83864017db16. The reviewer reviewed the plan only (on 0861e02). It reviewed no READY batch and issued no CLEAN.
- Open findings. Both are addressed in code and docs on 31086f9, but **closure has not been reviewed**:
  1. HIGH, TS-4c: finish every started frame before a resync. For a frame that cannot finish: `cut`, plus a separate TS-9 `write_failed` report. Add a negative proof for a resync before completion.
     - Done: the player rule, the notes' fault table, and the tests `a_resync_follows_the_finished_started_frame` and `a_started_frame_that_cannot_finish_ends_the_route`.
  2. Shared choice names: reject unequal counts, with no modulo mapping. Validate every complete combination with bindings carried across steps.
     - Done: `parse_all`, the `serve.json` cases, `a_choice_holds_across_steps` and `a_choice_with_unequal_counts_is_refused`, and `play_every_way` over `combinations`.
- The reviewer accepts: the 3-batch split; binding servers (`a5_2_player_serves_real_bindings`) as a documented obligation outside this pin; `answer_host_call` outside scope only while TS runs no HP transcript; the required ids kept pending until their proofs exist. Review resumes on an exact READY sha with a green botster-gate log.

## Steward rulings and amendments in play

HC A5 candidate 2 (current) and R-1.4. No question is waiting. No amendment is needed.

## Gotchas

- `botster-gate` refuses uncommitted tracked changes (exit 65). Set work aside as a WIP commit or a patch; never `git stash`.
- cargo-mutants runs the tests of each package only. Code in `botster-conformance` (`serve.rs`) needs tests in that crate: `tests/serve.rs` and the engine vectors.
- In wasm-bindgen, `JsError::new` panics on native targets. Native tests of the wasm crate can cover only the success paths. Expect mutants of the glue's error paths: give them tests in the TS suite and record equivalence entries with their reasons, or settle the question with the reviewer.
- Node's `fetch` has no `file:` URLs. Node tests pass the wasm bytes to `init`; only the Vite build proves the URL path.
- The route driver validates every `route_expect` frame against `to_client`. An undecodable or raw route answer needs the engine `expect` with `on: r1` and `allow_invalid`.
- The client sees a `raw_hex` with an ignorable type byte (0x80 and above) as nothing.
- Every served item must be used by its step: a `close` plus `cut` belongs in two steps, `expect` then `expect_closed`.

## Exact next step for a fresh implementer

1. In the worktree, `git checkout foundation/ts-player`; HEAD is 95d9dbb.
2. Do READY 1 from the clean state. Option 1: branch the READY-1 gate at 31086f9 by setting the WIP aside (`git revert --no-commit 95d9dbb`, committed on a side branch, or a patch). Option 2 (simpler): gate 95d9dbb once READY 2 builds. **Recommended:** create a local branch at 31086f9, merge origin/main, push it as the READY-1 head on `foundation/ts-player` (a fast-forward is impossible, so ask the lead about the history), run botster-gate, and send READY 1. If the history question blocks, finish READY 2 on top of 95d9dbb and send READY 1 and READY 2 together, with the reviewer's agreement.
3. READY 2:
   - `cargo install wasm-bindgen-cli --version 0.2.129 --locked`;
   - `cd ts && npm install`;
   - `npm run build -w @trybotster/hc-player`;
   - fix the compile errors in the wasm crate and the TS code;
   - `npm test -w @trybotster/hc-player`;
   - `npm run test:consumer -w @trybotster/hc-player`;
   - run botster-gate.
4. READY 3: add `$set` to `matcher.ts`, then `npm test -w @trybotster/hub-conformance` and botster-gate.
