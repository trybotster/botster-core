# Handoff: feature hp-calls-a (paused by the user)

Date: 2026-10-01. Branch `foundation/hp-calls-a`. Nothing is merged. The last review verdict is a REJECT of 1dac3a5; the fixes are in the last commit and are not yet reviewed.

## Goal

Transcripts for the HP host-call clauses, in `conformance/hp/`, one per ledger id, with `because` on every expectation. Brief: `~/botster-sessions/shared/foundation/brief-hp-calls-a.md`. Role file: `~/botster-sessions/shared/foundation/role-implementer.md`. Common rules: `pair-common.md` in the same folder. Reviewer label: "hp-calls-a — reviewer". Lead session: sess-1790821502-0048-460a17e7680b5e86d257dc1ca32d8690.

The clause list is the 145 ids of contract `hp` in `frozen/current/hub-plugin-contract-v1.1.joint-J4e.md` sections 9, 9.2, 9.3, 9.4, 9.5, 9.9, 9.10. List: `~/botster-sessions/shared/foundation/scratch-hp-calls-a/ids.json`.

## State

- All 145 transcripts exist and pass under seeds 0..31 (259 tests of `cargo test -p botster-plugin-conformance --test fakes`). They are out of `conformance/pending.txt`. CI was green on the last pushed sha.
- 64 transcripts are `fake: structural`, ruling R-1.3 (one is `plugin_host`, ruling R-1.4). The rest run against FakeHub or FakePlugin with scripted answers (R-1.3).
- FakeHub and FakePlugin are unchanged (0 lines of fake growth).
- Other commits on the branch:
  - `$not`, `$ne`, `$starts_with`, `$contains`, `$between`, `$array`, `$b64` matchers in `crates/botster-conformance/src/matcher.rs`, with vectors in `conformance/engine-vectors/matchers.json` and the text in `docs/foundation-design.md` 5.2 (lead-approved; TS parity comes with the TS runner, do not touch `ts/`; the `base64` crate was added).
  - 18 real-Hub controls named in `HUB_CONTROLS` (`crates/botster-plugin-conformance/src/lib.rs`; the doc comment is the table of shapes). Structural transcripts use them. FakeHub does not implement them (lead: no new fake behavior).
  - `Accessory.port_forward` (own commit 3cc7576): HP A2-40, CA-20, LB-19, regenerated schema, mapping test.
  - Templates `manifest` and `accessory` in `conformance/hp/fixtures/frames.json`, used by `admit_manifest`; test `admitted_package_files_are_complete_and_valid`.
  - Fixture `host_ops_gate.json`; tests `the_px_2_redaction_matchers_judge_samples`, `every_cited_clause_exists` (one lookup per distinct citation).
- `docs/plugin-fakes.md`, section "hp-calls-a": prior-art note, classification, findings, ablations. README real-only list is regenerated.

## Review history

- Round 1 REJECT f785c7a (5 findings), round 2 REJECT fb00052 (6), round 3 REJECT 1dac3a5 (3). Findings were: transcripts that did not observe their named behavior, forbidden results accepted, open values pinned, CA-16 reader as a host fixture, PX-2 failure shape, process-start barriers, injected completions.
- Fixed in the last commits (not yet reviewed): accessory schema; the leak checks of CF-1 and PX-2 (present value, no real secret, decoded tails); worktree identity read from the Hub's own final result (`await_operation` is a scripted no-op in `setup.controls` on the fake, plus one `operations.get`).
- No open finding is known beyond what a new review finds. Not covered by a transcript (said in the docs): http, ws and plugin_call operation kinds in `ca_16_operation_record_is_discriminated`.

## Gotchas

- The 145 transcripts come from a generator in `~/botster-sessions/shared/foundation/scratch-hp-calls-a/` (`lib.py`, `gen1.py` to `gen10.py`, `run.sh <filter>`, `ablate.py`). Later files override earlier ones by id. The JSON in `conformance/hp/` is the source of truth; if the folder is lost, edit the JSON by hand.
- A control listed in `setup.controls` (scripted) must not be in `requires.controls`; `lib.py` removes it.
- Lanes are read independently: use a barrier (a `status` control or `settle`) between a frame on one lane and a call that depends on it. `settle` adds a `{"settled": true}` item that the next `expect` would see.
- `calls_and_scripts_follow_appendix_a` checks every scripted result and every call argument against Appendix A. Use arrays for script entries, and `violates` on a call with deliberate bad arguments.
- Default-tier tests over 2 s fail CI.
- The shared git stash and `/private/tmp` are off limits; never kill by name or pattern; heavy commands go through `botsterq run --label hp-calls-a --deadline <d> -- <cmd>`.
- The ablation script (`ablate.py`) rewrites `hub.rs` and `wire.rs` temporarily; restore from `hub.rs.good` and `wire.rs.good` in the scratch folder, or `git checkout`.

## Next step

1. `cd` to the worktree, `git fetch origin && git merge origin/main`, then run `cargo test -p botster-plugin-conformance --test fakes` and the gates of CI (`cargo xtask ledger --check --base origin/main`, `taint`, `timers`, `clause-lint`, `codegen --check`, `real-only --check`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo xtask test-budget`).
2. Check CI on the head sha, then send the reviewer "READY FOR REVIEW foundation/hp-calls-a <sha>" (what changed since 1dac3a5 is in the list above).
3. Fix every REJECT finding and ask again. After ACCEPT, stay running until the lead confirms the merge.
