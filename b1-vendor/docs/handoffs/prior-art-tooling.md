# Handoff: prior-art-tooling (paused 2026-10-01)

## Goal and scope
Feature `prior-art-tooling`, branch `foundation/prior-art-tooling`, worktree `~/botster-sessions/trybotster-botster-contracts-foundation-prior-art-tooling`.
Brief: `~/botster-sessions/shared/foundation/brief-prior-art-tooling.md`. Role file: `~/botster-sessions/shared/foundation/role-implementer.md`.
Labels: "prior-art-tooling — implementer" (you) and "prior-art-tooling — reviewer" (Sol). The lead is session `sess-1790821502-0048-460a17e7680b5e86d257dc1ca32d8690`.
Scopes: A (route codec fixes), B (xtask test-budget census, driver, temp root), C (proptest, bolero, cargo-mutants, README).
Notes of record: `docs/prior-art-tooling.md` (Prior art), `README.md` "Building", `docs/tooling.md`.

## State
- Last pushed commit: the one that carries this file (see the reply to the lead). The commit before it is `62837a8`.
- The branch has `origin/main` merged (last merge `e0355e7`; main moved again after that, so merge `origin/main` first).
- All of scopes A, B and C are implemented. Local gates passed at `62837a8` content: fmt, clippy `-D warnings`, taint, timers, ledger, codegen, manifest, `cargo xtask test-budget` (662 tests, 3.3 s), `--slow` (8 tests). clause-lint passed after the last fix.
- CI on `62837a8` was running when work stopped. Check: `gh run list --branch foundation/prior-art-tooling`. The `fuzz` and `mutants` jobs run only on a pull request, so a push shows them as skipped; I ran their commands locally.

## What was built
- A: `Payload(Bytes)` for data-frame payloads (`HexBytes(Vec)` stays for the JSON form); `decode_shared`; `Inflated` cache on `ToClient::OutputDeflate` (`ToClient::inflated()`); `BytesMut` readers; `DecodeDetail`; `encode_shared` (steal commit ef21fdb with trailers); `tests/public_api.rs` + `public_api.txt`.
- B: crate `crates/botster-test-support` (`census`, `process::Driver`, `tempdir::TempRoot`), real-process controls in `tests/slow.rs` (slow tier), used by `xtask/src/test_budget.rs`.
- C: `crates/botster-route-codec/src/props.rs` (24 proptest properties, bolero harness `props::bolero_route_frame_decoder`), `proptest-regressions/props.txt`, `[profile.fuzz]`, CI jobs `fuzz` and `mutants` in `.github/workflows/ci.yml`, `.github/workflows/mutants-weekly.yml`, `.cargo/mutants.toml`.
- Mechanical call-site edits in `botster-fake-route` (lead approved): `Payload::from`, `.to_vec()`, `ToClient::output_deflate`, and a free fn `deflate_negotiated` (so the mutants entry covers only the seeded coin). Plus one test, `output_deflate_only_on_a_route_that_negotiated_it`.
- xtask test change: `codegen_check_fails_when_the_generated_file_differs` runs the binary twice (three runs hit 2.003 s on CI).

## Review history
- REJECT eaf7e62 (5 findings): all fixed in 7e3e878.
- REJECT 7e3e878 (1 finding): the equivalent-mutant entry matched two mutants. Fixed after that sha: `deflate_negotiated` is its own function; CI step "the equivalent-mutant entries are exact" checks the split; the comment says "across the seeds both branches are taken"; the new fake test kills the `deflate_negotiated -> true` mutant. Landing mutants on the fix: fakes step 7 of 7 caught.
- No REJECT is open. The fix after 7e3e878 (commits acfc8e1 to 62837a8 and this handoff) has NOT been sent to the reviewer.

## Left
1. Merge `origin/main`, rerun the gates (below), confirm CI green on the exact sha.
2. Send the reviewer "READY FOR REVIEW foundation/prior-art-tooling <sha>" (post_message, `agent_label` = "prior-art-tooling — reviewer", content in `payload`). Say what changed since 7e3e878: finding 1 of the last REJECT (the entries are exact; control step; new fake test; comment fix).
3. Fix any REJECT; wait for ACCEPT; stay running until the lead confirms the merge. Send the lead only QUESTION or BLOCKED.

## Gates (run from the worktree)
`cargo fmt --all --check; cargo clippy --workspace --all-targets -- -D warnings; cargo xtask taint; cargo xtask timers; cargo xtask clause-lint; cargo xtask manifest; cargo xtask ledger --check; cargo xtask codegen --check; cargo xtask real-only --check; botsterq run --label prior-art-tooling --deadline 20m -- cargo xtask test-budget; botsterq run ... -- cargo xtask test-budget --slow --deadline 5m`.
Mutants on the diff (what CI does): `git diff origin/main...HEAD > <scratch>/d.diff`; package step `cargo mutants --in-diff d.diff --no-shuffle -j 4 --exclude 'crates/botster-fake-*/**' --exclude 'crates/botster-plugin-fakes/**'`; fakes step `cargo mutants --in-diff d.diff --no-shuffle --test-workspace=true -j 2 --file 'crates/botster-fake-*/**/*.rs'`. Scratch dir: `~/botster-sessions/shared/foundation/scratch-prior-art-tooling/` (mutants logs; not in the repo).
Fuzz: `cargo +nightly bolero test -p botster-route-codec props::bolero_route_frame_decoder -T 20s` (nightly toolchain and cargo-bolero 0.13.5 are installed locally; cargo-mutants 27.1.0 too).

## Gotchas
- Never kill by name or pattern (lead rule). The Driver and xtask kill only a process group they started.
- A proptest failure writes a seed into `crates/botster-route-codec/proptest-regressions/props.txt`. After an ablation run, `git checkout` that file; commit it only for a real bug.
- bolero creates `crates/botster-route-codec/src/__fuzz__/`; the corpus is gitignored, delete the directory after a local fuzz run.
- `cargo mutants` per-mutant builds are slow with `--test-workspace=true`; use it only for fakes.
- clause-lint checks citations in fake crates: cite a ledger clause (TS-1, TS-3, BP-2 ...), not `A.4`.
- 168 mutants survive in code this diff does not change (base64, hex, `file_component`, `parse_op`, `OutputDropped::fit`, ...). They are for the weekly workflow, not this review. Do not add `exclude_re` entries for them.
- The `mutants.toml` entry for `FakeWorker::send_output` (seeded deflate coin) is the only accepted equivalent mutant; the reviewer accepted its reason.
- Reviewer messages arrive via `receive_messages`; send with `post_message` (payload field).
