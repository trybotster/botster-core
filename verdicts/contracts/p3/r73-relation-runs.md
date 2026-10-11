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

## Round 2: head `d93956987db0aa8c6e46ac415c2179c2a9024b82`

VERDICT: CLEAN (0 open) on this exact head. Merge-ready from the integration review.

### F1 closed: observed refusals and paste bodies in the run accounting (R-73 follow-up 2)

- `e627f60fed43d6217fdb5d5bd19bc33885fb80ee` implements items (1) and (2) in `route/driver.rs`:
  - The driver keeps each sent frame and each `InputRefused` it reads (`Seen::Sent`, `Seen::Refused`).
  - A refusal with op not 0 changes its op's run. `written_bytes` 0 removes the run. `written_bytes` n gives a run of n bytes: an exact run is cut to n bytes when n is not more than its length, else the run is unknown with length n. No `written_bytes` gives an unknown run for `failed` and `session_ended`, else no run. The refusal applies to an open paste or file of that op first, else to the latest run of that op on that route.
  - A committed paste body is known bytes. So an unplaced paste body inside a `written_without_op` span fails, like an unplaced `bytes` or `text` run.
  - Key, mouse and focus bytes are the stated limit. The `route_expect_pty` doc in `route/mod.rs` and `docs/foundation-design.md` 5.5 state this and cite R-73 follow-up 2.
- Tests in `tests/driver.rs`: `a_zero_effect_refusal_removes_the_run_of_its_op`, `a_partial_refusal_is_a_run_of_its_written_bytes_or_of_unknown_bytes`, `a_refusal_of_an_open_paste_ends_it`, `an_unplaced_paste_body_fails_a_without_op_span_and_a_key_is_the_stated_limit`. `6581dd1070f469d0db7cdc007fc4094c970276be` adds a test for a refusal count longer than the frame (the gate's MISSED mutant on the earlier head).
- Noted edge, not a finding: a refused second `paste_begin` that reuses an open op ends that open paste. No transcript does this, and the ruling's text covers it.

### F2 closed: the lint keeps paste bodies and adds the without-op case

- The steward accepted the stricter lint. `4d6db18f31c97a353cdd2336ba20c8f541dc59c2` reverts the narrowing of `a2aacea80473185899c299d9a94915291f975e28`, so `sent_input` keeps `paste_chunk`.
- `8899bfa36a3188f2c25e8f40e91a4fd3bb3d585a` adds an R-61 case: a `written_without_op` part on a session fails when an earlier sent body of that session (any op, typed or raw, any route) holds `0d`. This is needed because F1 (2) compares paste bodies byte for byte. Self-test: `a_without_op_part_flags_a_0d_in_any_sent_body_of_its_session`.

### Other changes in the delta

- 016c's F3 (`RouteSubject::control` doc: the driver reads only `bytes` of `pty_input`) is fixed in `77b0909a7c0d81179c96ac27b36152d3ea31cb08`. 016c closed it.
- Four Core transcripts add a `contains` to a paste part (`dp_3_route_codec…` op 7 `6667`; `dp_5_abort_before_commit…` op 2 `6b657074`, two parts; `dp_5_input_done_answers_only…` op 1 `61`; `dp_5_input_done_only_for_want_result_true` op 4 `68656c6c6f`). I compared each with its sent `paste_chunk`: each `contains` equals the paste body, and none holds `0d`.

### The main merges (BUILD Q7)

- `2ea2f805777dcc3132bb5745329b6a42e2c18903` (parents `6581dd1070f469d0db7cdc007fc4094c970276be` and `5ee91959dc63a62469870096cd15dbbd0fd25682`) and `d93956987db0aa8c6e46ac415c2179c2a9024b82` (parents `2ea2f805777dcc3132bb5745329b6a42e2c18903` and `3fec741732ec515a056e1a748c5b2c291c260cea`).
- The base-merge check fails (2)/(3) on `d9395698` for the shared paths `fence_lint.rs` and `docs/fake-replacement-map.md`, so I checked the delta by hand. `git merge-tree --write-tree` of each parent pair gives the commit's own tree (`6815ded61bda459115a8816817f562ed85d66ce3`, `9324176cdec5fa8751b9e4d411181560929561d4`). So both merges are clean automatic merges with no hand edits, and the shared files keep both sides.
- The PR's diff against `3fec741732ec515a056e1a748c5b2c291c260cea` touches only its own 12 files. In `fake-replacement-map.md` and `xtask/src/replacement_map.rs` it adds only the R-73 item 4 line, so the map stays the generator's output.

### No PASS to non-PASS

- Evidence `~/botster-sessions/shared/core-stage1/evidence/p3-r73/`: `r73-before-80c5b50c-macos.log` (contracts `f303ccdb9e02c6763d31cd8f7503095084630adc`) and `r73-after-77b0909a-macos.log` (contracts `77b0909a7c0d81179c96ac27b36152d3ea31cb08`), Core `80c5b50cfd4e94efd65b06ea13c045bab91a476f`, macOS.
- The two logs differ only in timings. All 44 runs fail in both, with the same steps and reasons, and none passes.
- After `77b0909a`, the PR's own change is only the test commit `6581dd10`. The rest comes from the main merges. So the evidence holds for the head.

### Gate on the exact head

- Log: `~/botster-sessions/gates/botster-contracts-p3-r73-relation-runs-d9395698-pool-20261011-114439-56274.log`, SHA256 `38f8ad3866b63cb32fd6ec1b993a15bea34022fa258523899a9fef779f7ed4dc`.
- Header: head `d93956987db0aa8c6e46ac415c2179c2a9024b82`, base `3fec741732ec515a056e1a748c5b2c291c260cea` (BOTSTER_CI_BASE_REF), Linux pool.
- Result (read by me): all 10 steps PASS; mutants 74 tested, 71 caught, 3 unviable; `botster-gate: exit 0 after 472 s on msa1`.
- The base is still the main tip, and the PR's merge base is the same commit. So this gate is the merging gate, and no further main merge is needed.
- 016c source CLEAN on the same head (`.review-verdicts/r73-relation-runs-d9395698-source-clean.md`).
