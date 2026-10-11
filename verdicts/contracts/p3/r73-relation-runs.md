# Contracts #53: integration review (p3/r73-relation-runs)

Reviewer: contracts integration reviewer, session `sess-1791740105-017f-b6f22445c954bd9e60e1ae695593410e` (in place of Sol for contracts PRs).
PR: https://github.com/trybotster/botster-contracts/pull/53 (P3). Tier: HIGH (stated; correct: the shared conformance driver `botster-hub-conformance`, the fence lint, and Core transcripts).

## Round 1: head `c6db5d0010db45ecf83eb8252d1e3ed00e565b43`

VERDICT: NOT CLEAN (2 open).

### F1 (MEDIUM, open): the `written_without_op` accounting rule has no ruling

- **Evidence.** `driver.rs` `Search::without_is_unaccounted`: a placement fails when a `written_without_op` span contains the exact bytes of a `bytes` or `text` run (op not 0) that no part placed. The PR body names this as "Point for the steward (not ruled)". R-73 item 2 says only "a non-empty run that no op-carrying frame of the transcript accounts for". No ruling on main `194643cf65bb4418c522322fb94c5461b65b5757`, in `shared/steward/drafts/` or in plan pin 23bt (`stage1-plan.a730792d.md`) defines "accounts for" this way. BUILD.md architecture rule 3: the meaning of an assertion comes from a clause or a ruling, not from the driver.
- **Effect today.** None on a committed transcript. I checked the 5 Core and 1 worker transcripts with `written_without_op`: all their `bytes` and `text` frames have op 0. The rule changed only P3's local ou_1 variant (evidence `r73-ou1-relation-parts-testkit-macos.log`).
- **Two cases for the steward** (both outside today's transcripts):
  1. *False failure.* `runs()` takes every sent frame, also one that the worker refused with zero effect (`input_refused`, AM-4). So a refused `bytes` op whose bytes also occur inside a later key or focus span makes `written_without_op` fail, though that op wrote nothing.
  2. *Remaining false pass.* A key, mouse, focus or paste frame with an op that is not 0 is op-carrying, but its run is `Unknown`. So a `written_without_op` span can cover its bytes and pass. The rule cannot see this.
- **Required.** A steward ruling on the accounting rule, through the lead (I sent the QUESTION). Then the driver doc comment cites that ruling. If the ruling also covers case 1 or case 2, the driver follows it, with a test.

### F2 (LOW, open): the new lint rule is stricter than the corrected R-73 follow-up for paste bodies

- **Evidence.** `fence_lint.rs` `defs::sent_input` collects `paste_chunk` bytes with `bytes` and `text`. The lint then flags a `written_by_op` of a paste whose body holds `0d`. Under R-73 follow-up (A) a paste run is not exact. Only its `contains` is asserted, and the corrected item 2 says "any body byte except `0d` is exact". So a paste body `61 0d 62` with `contains 61` is valid by the ruling, and the lint flags it. `0d` in a `contains` literal is already flagged by `exact_inputs` (its `literals` walk reads every `hex` in `route_expect_pty`, `contains` included).
- **Required.** Choose one:
  - remove `PasteChunk` from `sent_input`, change the self-test case `chunk("1", "410d")` to expect no finding, and keep the `bytes`, `text` and raw cases; or
  - get a steward line that accepts the stricter form, and cite it in the lint comment and the PR body.

### What I checked and found correct

1. **R-73 item 2 and follow-up (A), the driver** (`route/driver.rs`).
   - The driver keeps each sent frame, typed and decoded raw, on every route. It reads no `segments`. The `pty_input` log is read as one byte stream (`bytes`).
   - `bytes` and `text` with op not 0 give an exact run. Paste and file runs (begin to commit, by route and op) and key, mouse, focus and reply runs with an op are `Unknown`: non-empty, the position, `len_var` and `contains` only. An abort drops the run. Op 0 gives no run (TS-5b). Resize gives no run.
   - Parts consume the log in order. A part takes an unused run of its op; a run whose `contains` does not match is not that part's run (follow-up item 3). The log must end with the last part.
   - The placement limit (1,000,000) ends the step as a bad step, never a pass.
   - The change after the evidence commit (`b3f9197a8ea062c3be1bacfb8a9feb40bca6778d` to `c6db5d0010db45ecf83eb8252d1e3ed00e565b43`) only rewrites the limit as a `checked_sub` budget and `furthest` as a `max`. The limit still fails at placement 1,000,001, so the evidence holds for the head.
2. **R-73 (1) and (5) refused, (B) and (C) refused.** No owner field on `Action::PtyWrite`; no testkit-only proof; the fake route endpoint and FakeCore do not change. The FakeCore numeric-op defect is recorded in `docs/fake-replacement-map.md` through `xtask/src/replacement_map.rs` (item 4).
3. **Follow-up item 2: every Core paste part has a body `contains`.** My sweep of all Core transcripts with `written_by_op` on a paste op finds 10 parts in 9 transcripts. Each has a `contains` that is a substring of the paste body, with no `0d`. This includes the raw chunk of `dp_3_data_frames_are_raw` (`82…037b7d`, op 3, `7b7d`) and the two `610a62` parts that the ruling keeps.
4. **The negative test.** `a_paste_split_by_another_writer_fails_written_by_op` uses an exact other writer inside the paste, as follow-up item 3 requires.
5. **No PASS to non-PASS.** Evidence `~/botster-sessions/shared/core-stage1/evidence/p3-r73/`: Core `6161c5c2122e43b4721497ad787814b108f22169`, macOS, 18 ids on 2 tiers, the two minimum ids with 5 real runs each. Before and after give identical result lines on all 44 runs, all failing before `route_expect_pty` or outside it (worker refusals that R-73 "Not decided here" names). The fake-tier suite passes in the gate.
6. **The new lint case for `bytes` and `text`** is needed. An exact run is compared byte for byte, and `ICRNL` would turn its `0d` into `0a` on the real tier (R-61 (c)).

### Gate on the exact head

- Log: `~/botster-sessions/gates/botster-contracts-p3-r73-relation-runs-c6db5d00-pool-20261011-105842-50586.log`, SHA256 `6e86032e1203271d2a3579afd6d0c6fb004cd5d4b8280a502d5fe0e4e551d7cd`.
- Header: head `c6db5d0010db45ecf83eb8252d1e3ed00e565b43`, base `8d79b70d1d68966911a844c73bdbc4bdcd8f85f0` (BOTSTER_CI_BASE_REF), Linux pool.
- Result (read by me): all 10 steps PASS; mutants 56 tested, 54 caught, 2 unviable; `botster-gate: exit 0 after 420 s on msa1`.
- The PR's merge of main is `194643cf65bb4418c522322fb94c5461b65b5757`. P3 merges the main tip once before the final gate; I check that merge with Q7.

### Rulings after round 1 (recorded for round 2)

- **F2.** The steward to me (2026-10-11): the stricter lint is ACCEPTED as sound ("the lint may flag a 0d in any paste body that a relation part covers ... This line closes F2"). P3's `a2aacea80473185899c299d9a94915291f975e28` narrowed the lint at the same time. I asked P3 to revert that narrowing, because F1 item (2) makes a paste body known bytes in the accounting, so a paste body `0d` would give a false pass on the real tier.
- **F1.** The steward's ruling (R-73 follow-up 2, pending; via the lead): P3's meaning is accepted. (1) An op with an observed zero-effect `input_refused` leaves `runs()`; a partial refusal is a run of `written_bytes` when exact, else unknown. (2) A paste body is known bytes (IN-8), so an unplaced paste body inside a `written_without_op` span fails; key, mouse and focus bytes are the stated limit, written in the `route_expect_pty` doc. F1 closes on a head that implements (1) and (2) with tests and cites the ruling.
