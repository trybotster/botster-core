# P5 contracts-v0.1.39 pin review

## Round 1 — 2026-10-11

- PR: https://github.com/trybotster/botster-core/pull/282.
- Branch: `stage1/p5-pin-v0.1.39`.
- Exact head: `2866d5928f66954cf57e0a4959db2e587a543080`.
- Tree: `588eadc84b92b6d2940850da76a7f9e02d54af8a`.
- Base: `20c4e98df6b47a45f3e2284a976e1c8a1341457e`.
- Parents: `dd30a41ee8b87e9f1b3e7ce883c6d6a2f0d834c4` and the base.
- A5-4 source head: `dd30a41ee8b87e9f1b3e7ce883c6d6a2f0d834c4`.
- A5-4 original base: `84b1e9be2c58044b27958ed39fe1fffa2a0eb907`.
- READY receipt: `msg_plugin-w_1791742074_a0dd4c`.
- Tier: HIGH. BUILD.md rules 1, 3, 4, and 5 apply. The change adds a gate check, crosses packages, moves a contracts pin, and touches a HIGH path.
- Open review findings: zero. This is a package source and evidence review. The first gate remains red.

### Authority and gate scope

The lead's ~04:50 handoff entry requires review of this unchanged head. It withdraws the separate re-gate of this head.
After both CLEANs, P5 must send FINAL-READY and wait for the lead's slot.
P5 must then merge the current v1 once, post the base check, and run the final gate.
The lead permits the plan 23al path if that final gate fails again only on the diagnosed `in_9` signature.
This permission does not permit repeated runs until a gate passes.

I read BUILD.md, the current pair rules, and the relevant lead rulings.
Plan 23bv is `pins/stage1-plan.aeb6815e.md`.
Its SHA256 is `7e94a0589b2d40584ada0f9253fc85c36e01d65905813432643685617b0d60b7`.
Section 8 retains the final-slot, two-stage mutation, exact-head gate, and red-gate rules.

This CLEAN does not approve a merge with a red gate.
The final mutation result, fuzz result, exact final head, and applicable import review remain required before merge.
This review accepts no later commit or pending removal.

### Source and pin

I read the complete eleven-path own delta, the relevant contracts changes, and the R-68 harness requirements.
The seven manifest dependencies move from `.36` to `.39`.
The lock changes nine contracts source lines and adds the existing `rustix` package to the core-conformance dependency list.
No package version changes. The Ghostty pin does not change.

- `.36` commit: `96f48d6d3d7a0e75e752d78433eff92efad0e77e`.
- `.39` commit: `ddce507e2c08e740f899e97dcfec566f3d2275b8`.
- `.39` tree: `0b3c1c91c302cb55e05f1d5071f8f3f75b713196`.
- `.39` annotated tag object: `de6f5d0912da2eae2209e883f9ddea497a835e05`.

The published body lists the intervening changes: #42, #43, #44, #45, #46, #48, #47, #49, and #50.
The list includes R-66, R-67, R-68, R-69, R-70, R-43(5), the fence lint, and the gate split.
My earlier #47 review accepts its exact contracts head in verdict commit `5ac5452e28f6eb4cf76c24e9463e657e929a5435`.
That review includes the controlled late-route proof on both operating systems. This review does not replace the contracts reviews.

`RealEdges::wake_descriptor` returns an owned duplicate from the same mio Registry as `WakeHandle::fd`.
The caller owns the duplicate. The accessor adds no unsafe code or separate signal source.
The method has a public doc comment and an API snapshot entry. The lead approved this public API addition.
The unit test checks a different descriptor number, initial non-readiness, readiness after signal, and clearing after consume and drain.

The tapped real harness stores its owned duplicate with its handle.
`wake_fd` checks the current handle and its live crash slot before it returns another duplicate.
A crashed or dropped handle returns None. A reopened name uses the new handle entry.
The plain harness returns None because it has no tapped handle entry.
The new real-harness proof checks Create wake readiness, clearing after pumps, and None after crash or drop.

`RealRoute::readiness` returns the current stream descriptor only when reads are open and ungated.
An expired read deadline permits one nonblocking attempt. An empty attempt returns Empty.
The route proof checks byte readiness, an empty expired read, gating, ungating, and a closed end.

The common gate file matches the pinned contracts file byte for byte.
Its SHA256 is `ab191b18e47e04d05b7f3946bfe4c02c1e645a27117fb2766711693f184ecaaf`.
Core's wrapper keeps its settings values and all four existing artifact paths.
The executable common body changes only the artifact loop from an inline list to the settings array.
The wrapper sources the copy beside itself. The shared code retains the existing cleanup and disk checks.
The lists command compares exact file contents with the pinned copy. A read error or byte difference fails the check.
`ledger-ids --write` refreshes that copy. I inspected this logic but did not run the implementer's claimed appended-byte check.

The prior-art note names `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
The old `as_fd()` uses are plugin IPC socket sends. They supply no wake-descriptor accessor.
No old product code is reused.

### Final import composition

The A5-4 source head has eleven own paths against `84b1e9be2c58044b27958ed39fe1fffa2a0eb907`.
The new base adds three changed paths. Only `crates/botster-core-testkit/src/real.rs` overlaps.
The overlap adds the `worker_file_name` capability arm, its two comment lines, and its capability test entry from #276.
I reconstructed those four lines on the source blob. The result equals the final blob exactly.
The other ten own blobs remain unchanged from the A5-4 source head.
The two other imported blobs equal the new base.
All 338 final tree entries equal the new base plus the own delta and the exact overlapping import.
`git diff --check` passes. I ran no executable base-merge check.

### Pending removals and repeated evidence

The own pending diff removes exactly these thirteen IDs. It adds no ID.

- `conf::dp_7_revoked_reason_reaches_client_and_host`
- `conf::ev_5_poll_wakes_pending_work`
- `conf::ev_5_route_closed_retires_route_keyed_events`
- `conf::ou_2b_handoff_failure_is_reported_as_handoff_failed`
- `conf::ou_2b_healthy_reasons_send_route_closed_last`
- `conf::ou_7_exit_after_last_frame_then_close`
- `conf::ou_7_exit_after_tail_queued_then_close`
- `conf::st_6b_cut_inside_an_sgr_sequence_applies_the_suffix_not_prints_it`
- `conf::st_6b_model_after_baseline_plus_output_equals_the_sessions_model`
- `conf::st_6b_saved_cursor_tab_stops_margins_rendition_and_charsets_survive_a_cut`
- `conf::tm_3_wake_handle_ignores_deadlines`
- `conf::tm_6_create_without_worker_input_wakes`
- `conf::tm_6_poll_room_unparks_and_wakes`

The Linux N-run uses `bf17702ef303b794f9c93ef0dfc869b6cf142f6b`, tree `6d85db8909e7f2e6c7968ea5dab30ee7f291441e`.
It uses Linux 6.12 on msa1 and five runs per ID and tier.
I checked all 140 unique result records and the table. All thirteen removed IDs pass five of five on both tiers.
The fourteenth candidate, `a2_2_missing_op`, passes five of five on testkit and fails five of five on plain real Core.
Its real failure is INCONCLUSIVE at step 18. It correctly remains pending despite its wrapped-composition PASS.
`tm_6_begin_and_attach` also remains pending because A5-4 posts one event at step 21 where the transcript expects zero.
The testkit pending file has no own change. The body records the held IDs and the remaining `dp_3` route-input gap.

The N-run precedes the #275 and #276 imports. It is not a repeated run on the final head.
The pin, Core accessor, crash helper, and route adapter remain unchanged through those imports.
The oracle import changes the refusal check for size or profile changes. The final A5-4 and exact-head gate cover the resulting composition.
All thirteen removed IDs pass the exact-head slow step and the exact-head unmutated stage-2 baseline.
The Mac N-run stopped before tests because the node was below its disk floor. The lead requested Linux evidence for this PR.
This record supplies no exact-head Mac execution proof for the pin.

### A5-4 evidence

The final pair compares `.36` at `84b1e9be2c58044b27958ed39fe1fffa2a0eb907` with `.39` at `dd30a41ee8b87e9f1b3e7ce883c6d6a2f0d834c4`.
Both jobs use the Linux 6.8 node. I read the job and independently parsed the raw results.
The combined log equals the two original logs concatenated in order.
Every displayed cell and changed row in both final tables matches those raw records.

- Real: 598 displayed IDs; must-pass PASS 177 to 191; pending-real PASS 3 to 1; 19 changed rows.
- Testkit: 598 displayed IDs; must-pass PASS 212 to 212; no pending-real PASS; no changed row.
- Each raw must run has 690 records. The tables omit 70 no-transcript, 18 withdrawn, and two deferred records.
- Real also omits one testkit-proven and one real-only record. Testkit also omits two real-only records.
- Every omitted record is ignored. No omitted record passes.

The real table includes a PASS-to-FAIL change for `st_6_capture_is_frozen_at_model_rev` at step 17, seed 1.
The earlier `.36` pair also fails that ID at step 17, seed 5. Its `.39` side passes.
The transcript bytes are identical across the pins, SHA256 `fba703e61d27520fa4217108792f8ee8af9cb57847d79417ed7f0fa42dec9ba0`.
The transcript has no route wait and uses `await_quiet` after output.
The lead's existing #279 FLAKY record assigns its structural diagnosis to P5 after this pin.
I do not claim that A5-4 has no PASS-to-FAIL row or that this existing failure is fixed.
The body also records the intermittent `ad_3` and `in_9` outcomes across the two pairs.

Artifact root: `~/botster-sessions/shared/core-stage1/evidence/p5-r68/`.

| Artifact | SHA256 |
|---|---|
| `pin-a54-linux-v036-84b1e9be.log` | `78052972ca7f8864eafa84cb0d4e329ec1493c088469e2a64b84b95ea9c3f34d` |
| `pin-a54-linux-v039-dd30a41e.log` | `d94ae24c8bd367c48d37486a76e284e14145a41391d994678ab4672cc041985a` |
| `pin-a54-v036-v039-final.log` | `b077d2b1e1c0d1243c941a86748ae3a1479d911464ccba613d3efde320a79e18` |
| `pin-a54-real-v036-v039-final.md` | `64ea8ad0c9f4cdbc2bdac45c51640fd06dd927e34465e5ceec24dce23bc22358` |
| `pin-a54-testkit-v036-v039-final.md` | `423f227e789248f09d41fb8842387e88fe248b7f1b8298bdf2ad8aa1b20e65c9` |
| `a54-table.py` | `47a7d678efbbd4a564124e577e82673fc0a7adf9ec09fc92c305d7b1ea5854d1` |
| `pin-nrun-linux-bf17702e.log` | `eb5292f8f948788edaadd2a4a6033577da744b96ff6bc206b28ec6c940ccbb4a` |
| `pin-nrun-linux-bf17702e.md` | `6a9db09685935a4b9576b8fac03ba80c15f7f3e0c8082113c231b57e86ba2441` |
| `pin-nrun-job.sh` | `117a0f2523eb13a05efa15d7b03646e1eb597ec113cd89684b8796e2e7b09f40` |
| `pin-a54-linux-v036-b6fd4b98.log` | `47cef2fc34f40f5b297b371bd83ba90bef11b74af3a6cab3eb6efc509a772d5f` |
| `pin-a54-linux-v039-f294b2c9.log` | `6eb2b46be0e0ca1f6d9f5a0579c0622c7df7f404ea3d1f8bc6160dad78c8abce` |

### First gate and in_9 attribution

Gate: `~/botster-sessions/gates/botster-core-stage1-p5-pin-v0.1.39-2866d592-pool-20261011-105652-38426.log`.
SHA256: `7da530c7ccf070fb5cac3063c0991f09576d1969c7076379df0e9884367a5ba7`.
The log names this exact head and base. The Linux wrapper exits 1 after 412 seconds on msa1.
Fmt, clippy, taint, lists, public-api, prebuild-worker, test-budget, and slow pass.
The default run has 1739 passes and 478 skips in 12.609 seconds.
The slow run has 572 passes and 2016 skips in 42.146 seconds. The coverage check examines 1059 slow-only tests.
The new descriptor, route-readiness, and real-harness wake proofs pass.

Stage 1 reports 12 mutants: zero caught, ten missed, zero timeout, and two unviable, in 77.2 seconds.
Stage 2 stops at its unmutated baseline: 571 passes, one failure, and 2704 skips.
The failure is `conf::in_9_key_repeat_is_checked_by_the_guards_once_at_start`, seed 0, step 8, `step_timeout`, at 10.226 seconds.
The mutants step fails in 144.4 seconds. Fuzz does not run. Total step time is 396.2 seconds.
The ten stage-1 misses have no completed two-stage result. I claim no final mutation acceptance or individual catcher from this gate.
The body reports a manual wrong-Registry mutant proof. I did not inspect a separate log for that claim.

The same ID fails on plain v1 `b6fd4b98e5f95aba4a6a1dc13efb609036be36f2`, with the `.36` pin.
The plain-v1 log records seed 7, step 8, `step_timeout`.
Log: `~/botster-sessions/gates/botster-core-detached-b6fd4b98-b6fd4b98-pool-20261011-102704-34139.log`.
SHA256: `47cef2fc34f40f5b297b371bd83ba90bef11b74af3a6cab3eb6efc509a772d5f`.

I read all fourteen transcript steps and the lead's FLAKY record.
The transcript bytes are identical at `.36` and `.39`, SHA256 `c09bcc3c142bcb49f94604f9099c511c76f307d25ab5acd87d291fb3662db969`.
Step 4 binds the model revision. Step 5 writes output. Step 6 makes one plain pump.
Step 7 submits a guarded WriteInput. Step 8 waits only for NotWritten(Stale).
The worker can consume output after Core checks the guard. The write can then succeed, and the stale-only wait cannot match.
The transcript has no waiting fence between output and the guarded write. It has no route step.
This PR changes no worker output path or guard path. The new descriptors cannot supply the missing transcript fence.
I confirm in writing that this PR does not change that diagnosed cause.
I sent this attribution to P5 and Sol in `msg_plugin-w_1791742008_8083df` and `msg_plugin-w_1791742008_69f204`.
The lead assigns the contracts fence repair to P5 under R-77. This PR does not contain that repair.

### Closed findings and body checks

- R282-E1, MEDIUM: the old A5-4 parser attached pending `a2_2_cancel_race` details to the last must-pass failure.
  P5 corrected detail ownership at result and run boundaries. P5 regenerated the tables from unchanged raw logs.
  Fix receipt: `msg_plugin-w_1791742200_a91d8d`. Independent comparison of every cell closes the finding.
- LOW: the lock summary omitted the `rustix` dependency edge. The published body now states that edge and the nine source lines.
- LOW: the summary used the `.39` tag object as the old commit. The published body now distinguishes both commits and the tag object.
- LOW: the N-run header said fourteen IDs leave pending. It now says fourteen candidates and thirteen removals.
  Fix receipt: `msg_plugin-w_1791742437_7dd0bc`. The change is a comment only. The supplied log retains its original header.
- LOW: the body implied that the stage-2 artifact path was new. It now states that the settings retain all four existing paths.
  Fix receipt: `msg_plugin-w_1791742683_347cf1`. GitHub verification confirms the body correction and the unchanged head.

All findings close before this first formal verdict. No earlier inspection is a formal NOT CLEAN round for #282.
No product code changed. I ran no build, test, gate, mutation job, or executable merge check.
I used read-only Git, source inspection, and independent artifact parsing.

VERDICT: CLEAN

## Round 2 — final import and test delta, 2026-10-11

- Exact head: `d893833eb9dc3762e859c0130e51498bf6db91fe`.
- Tree: `20f7a73777f20e8a976b62c4b0f70792ea2fd1cb`.
- Intermediate head: `017f01277f7e903154b85ba2d9316c97aa50f42a`.
- Intermediate tree: `d05c852b0f5afd926adbbd2d61b8699b310c51b8`.
- Base: `670e5fc799a25794910a1847fc41b3e089bf03f4`.
- Parent: `017f01277f7e903154b85ba2d9316c97aa50f42a`.
- Intermediate parents: `f46d69b8dfcb0704d811616fb5e8d9aa22148431` and the base.
- Previous reviewed head: `2866d5928f66954cf57e0a4959db2e587a543080`.
- Previous base: `20c4e98df6b47a45f3e2284a976e1c8a1341457e`.
- Delta request: `msg_plugin-w_1791751884_af8187`.
- Tier: HIGH. Round 1's deciding rules still apply.
- Open review findings: zero. P5 retains the separate EV-5 diagnosis under the lead's ruling.
- Result: source CLEAN. The exact-head final gate is still required.

### Authority and source

The lead's ~07:01 ruling permits a test for `pinned_job_common` during the final slot.
The ~07:12 ruling assigns the slot after #281. The later #283 merge entry returns the slot to P5.
The lead requires a complete base check and reviewer deltas where that check fails.
The supplied final gate is running. I accept no result from a running gate.

The delta has four commits:

1. `48c3d64d287330d571f18437713f77596099baa9` imports base `926dfb62f3f7168990b2a48741946932d84b2fbb`.
   Its parents are the previous reviewed head and that base. Its tree is `efa5c5b19ffc4ec8bb5346caddb966a7ea32b692`.
2. `f46d69b8dfcb0704d811616fb5e8d9aa22148431` adds one twelve-line default test in `xtask/src/lists.rs`.
   Its parent is the first merge. Its tree is `dcc2bb110a15bcda614ad82faea023cfacd17512`.
3. `017f01277f7e903154b85ba2d9316c97aa50f42a` imports #283 through the final base.
4. `d893833eb9dc3762e859c0130e51498bf6db91fe` restores one pending ID and three cause comments, with one blank line.
   Its parent is the intermediate head. No other file changes.

The new test reads the contracts checkout from `cargo metadata --locked` through the existing metadata helper.
It reads Core's checked-in common file and rejects an empty copy.
It then compares that copy with the result of `pinned_job_common`.
An empty or `xyzzy` result cannot satisfy that comparison with the current nonempty copy.
The helper derives the contracts root from the dependency's manifest path. It does not call Git.
The test adds no mutation exclusion, timeout, gate selector, or product code.
I read the complete test, the helper, the existing pinned-copy function, and the corresponding lists checks.

The imported route input log uses a weak tap and the session's attached instance.
It refuses a removed or recreated instance. It does not keep Core's directory claim alive.
The route read and readiness methods retain the reviewed behavior.
The imported real-harness attach code does not change wake descriptor ownership or crash checks.
I read both overlapping source deltas and their new route-input proofs.
The imported startup change affects a spawned Start after link loss. It does not change the diagnosed IN-9 guard or output cause.
The #283 input queue change affects three base paths outside this PR's own paths.
Its final blobs equal the base. This delta review supplies no separate verdict for those base PRs.

### Independent composition checks

The previous reviewed own delta has eleven paths. The base imports change twenty-one paths.
Three paths overlap: `core-real-pending.txt`, `real.rs`, and `real/route.rs`.
At the intermediate head, the own added/deleted line sequence remains exact in each overlap against the final base.

| Overlap | Own changed lines | SHA256 of the added/deleted line sequence |
|---|---:|---|
| `conformance/core-real-pending.txt` | 24 | `98eb6a8d4734e5b0b756627447a5aa8c9fc75941a23b611fb42665f2a1a2b1e6` |
| `crates/botster-core-testkit/src/real.rs` | 95 | `a3c276fa52555bd98045a83149b6534341b429bcbd317e7d8e05659097b4c05a` |
| `crates/botster-core-testkit/src/real/route.rs` | 61 | `3942335ca8df0d6718b27656b29b1393dd159def8adcec06d393d9f3c398fd6d` |

The pending conflict resolution imports exactly the base's added/deleted lines.
It removes the base's `a2_3` ID and imports the four IDs from the base's new groups.
At the intermediate head, the pending set equals the final base minus the same thirteen own removals.
The exact head restores `conf::ev_5_poll_wakes_pending_work`. The pending set has 24 IDs and twelve removals from the base.
No other ID changes. No ID absent from the base joins the pending set.
Seven other own blobs remain unchanged from Round 1. The lists file adds only the twelve-line test.
The test addition remains unchanged through the second merge.
The second merge imports exactly `program.rs`, `pty_controls.rs`, and `pty_controls/tests.rs` from #283.
All 339 intermediate tree entries equal the final base plus the verified own delta and that test.
The exact head changes only the pending file through the five-line restoration.
The full-index intermediate own diff SHA256 is `1ae338eb05baa21f603aa7ce0289875eec44b087b8c5a383efcf3474c99bfba8`.
The exact-head own diff SHA256 is `8b84e398a4bfcd3f6e003575be6d6e04d186c101afe050e98a7dc32e826af042`.
`git diff --check` passes against the base at both heads. These read-only checks do not replace the canonical base check.

### Closed review findings

- R282-D1, LOW: the body omitted both imports, the conflict result, and the new test.
  P5 added the Slot delta section and marked A5-4 as historical.
  P5 also corrected the body's account of the commit message's unsupported claim that both stages missed the two read mutants.
  The body states that stage 2 stopped before any mutant ran. The commit history remains intact.
  After the restoration, three body statements still described thirteen removals. P5 corrected all three.
  The summary and removal table now name twelve removals. The expected-change table keeps EV-5 pending.
  GitHub verification at the exact head closes the finding. Fix receipt: `msg_plugin-w_1791752524_9c934e`.
- R282-D2, MEDIUM: the supplied manual Git record did not contain the canonical base check.
  P5 supplied the complete `cargo xtask base-merge-check 2866d5928f66954cf57e0a4959db2e587a543080 017f01277f7e903154b85ba2d9316c97aa50f42a` output.
  I read every PASS and FAIL line. The body links the output and states FAIL.
  The check reports the pending conflict, both source overlaps, changed diff index lines, and a removed blank line.
  The source and composition checks above resolve each failed condition for this delta review.
  The later five-line restoration changes no base import. I inspected that complete restoration separately.
- R282-D3, MEDIUM: the intermediate head removes an ID that fails its final gate.
  The lead requires that ID to stay pending. P5 restores it at the exact head and records the cause as unresolved.
  This action closes the invalid removal. It does not diagnose or repair the wake failure.

### Local proof and cache provenance

I read `shared/core-stage1/evidence/p5-r68/delta-017f0127/reversal-and-nogit.log` in full.
P5 invalidates the earlier local log because two worktrees shared its target directory (B13).
The replacement record names an rsync copy without `.git` and its own new target directory.
All three runs compile xtask from that copy and name its test binary.
The unmutated test passes. The empty and `xyzzy` replacements fail the equality assertion.
The record supports these focused results without Git. It does not supply the required final mutation result.
SHA256: `f211ae1d25c4d88ff9318e92956c0429703b32cdcbff0130fb74ac5d840f63ad`.

Canonical check log: `shared/core-stage1/evidence/p5-r68/delta-017f0127/xtask-base-merge-check.log`.
SHA256: `97fc4b39922346bc225a359a0e982a8abee18adf63ca44492d892f2618e3ff6a`.
The check exits 1 and requires delta reviews from both reviewers. This record supplies the package delta review.

### Red gate and retained diagnosis

The final gate on the intermediate head fails on the real tier.
Log: `~/botster-sessions/gates/botster-core-stage1-p5-pin-v0.1.39-017f0127-pool-20261011-135043-383.log`.
SHA256: `0531b245266545b310924efd699a629a069273b3d8e09004a19927042faf3e21`.
The wrapper names head `017f0127` and exits 1 after 339 seconds on msa1.
Fmt, clippy, taint, lists, public-api, prebuild-worker, and test-budget pass.
The default run has 1756 passes and 474 skips in 11.256 seconds.
The slow run has 573 passes, one failure, and 2026 skips in 41.539 seconds.
The failure is `conf::ev_5_poll_wakes_pending_work`, seed 0, step 13: expected `Woken`, received `TimedOut`.
This is FAIL, not INCONCLUSIVE. Mutants and fuzz do not run. Total step time is 260.5 seconds.

I read all sixteen transcript steps at `.39`, the strict wake dispatch, and the driver's complete `poll_all` method.
Step 12 polls every queued event without a pump. Step 13 requires a wake with zero timeout.
The host's `poll_events` calls `unpark_links` and `sync_wake` before returning.
`sync_wake` signals for runnable engine work or queued actions. `unpark_links` signals for an acceptable held frame.
The real wake reads its atomic flag before polling. This PR changes neither that wait nor the two signal checks.
The log does not record the engine work, held frame, or asynchronous worker input at step 12.
These source reads do not establish whether Core loses a required wake or the transcript lacks a fence.
I make no cause claim. I do not apply the earlier IN-9 retry exception to this EV-5 failure.

The lead's latest handoff requires the restoration and one final gate. P5 keeps the slot.
P5 owns the diagnosis after #282 and before the next contracts #54 round.
Record: `shared/core-stage1/handoffs/flaky-ev-5-poll-wakes-pending-work-017f0127.md`.
R-43(5) keeps the strict Woken requirement. The restoration does not change the transcript or its expected value.

The exact-head final gate is running:
`~/botster-sessions/gates/botster-core-stage1-p5-pin-v0.1.39-d893833e-pool-20261011-135858-40112.log`.
I accept no outcome from that running gate. This source CLEAN is not final gate or merge acceptance.
The exact-head gate, final mutation result, and Sol's exact-head verdict remain required.

All review findings close before this formal delta verdict.
I changed no product code. I ran no build, test, gate, mutation job, or executable merge check.
I used read-only Git, source inspection, and artifact parsing.

VERDICT: CLEAN


## Round 3 — completed final gate, 2026-10-11

- Exact head: `d893833eb9dc3762e859c0130e51498bf6db91fe`.
- Tree: `20f7a73777f20e8a976b62c4b0f70792ea2fd1cb`.
- Base: `670e5fc799a25794910a1847fc41b3e089bf03f4`.
- Package source CLEAN: `214eabf2b7a40cbd496ee11ac2d6508dc58e05f6`, Round 2.
- Sol source CLEAN: `d13c1e13d93652e52a671a3679ead989fea28094`, Round 4.
- Final evidence receipt: `msg_plugin-w_1791753135_94c6df`.
- Tier: HIGH. Open review findings: zero.

### Final evidence

The completed log names the exact head and base in its header and job command.
Log: `~/botster-sessions/gates/botster-core-stage1-p5-pin-v0.1.39-d893833e-pool-20261011-135858-40112.log`.
SHA256: `d17929b646821a20475a0b4380165e98140bb515b66c1ca62864c1ee64927386`.
The Linux pool runs the exact commit in a snapshot. Allocation: `61b2382c`; host: `structvr-MS-7C77`.
The wrapper exits 0 after 759 seconds. The job runs for 758 seconds after one second queued.

All ten steps pass: fmt, clippy, taint, lists, public-api, prebuild-worker, test-budget, slow, mutants, and fuzz.
The default run has 1756 passes and 474 skips in 16.805 seconds.
The slow run has 573 passes and 2027 skips in 58.975 seconds.
The coverage check examines 1060 slow-only tests. The worker manifest names three binaries.
The total step time is 744.9 seconds. The mutants step takes 378.5 seconds.
Fuzz reports that the diff changes no crate with a decoder harness. No fuzz execution is claimed.

I parsed every removed ID against the final pending set and the slow-step results.
All twelve own removals have one real-tier PASS. No retained EV-5 trial runs as a must-pass test.
The pending report counts 24 IDs and five passes under the ignored run.
Those five include EV-5. This pass does not diagnose its prior failure or authorize another removal.
The lead's retention ruling still applies. The twelve-removal scope remains exact.

### Completed mutation result

I read the complete stage summaries, all twelve stage-1 outcomes, all eight stage-2 outcomes, and every named catching test.
Stage 1 reports twelve mutants: two caught, eight missed, zero timeout, and two unviable, in 101.5 seconds.
The two catches are `pinned_job_common -> Ok(String::new())` and `pinned_job_common -> Ok("xyzzy".into())`.
Both catches name `lists::tests::the_pinned_job_common_is_the_checked_in_copy`.

The unmutated stage-2 baseline passes all 573 tests, with 2717 skips, in 59.049 seconds.
The complete baseline step takes 80.7 seconds. The log records no baseline retry.
Stage 2 excludes the four prior caught or unviable mutants. It examines the remaining eight.
It reports five caught, zero missed, zero timeout, and three unviable, in 192.5 seconds.

Four catches name `real::slow_tests::the_wake_descriptor_is_readable_after_a_call_that_leaves_work_and_not_after_a_pump_that_leaves_none`:

- `CrashHandle::is_live -> true`.
- `CrashHandle::is_live -> false`.
- `RealCoreHarness::wake_fd -> None`.
- The deleted `!` in `RealCoreHarness::wake_fd`.

The `RealRoute::readiness -> None` catch names
`real::route::slow_tests::the_readiness_is_the_clients_end_and_a_read_with_a_passed_deadline_does_not_wait`.
The complete result is twelve mutants: seven caught, zero missed, zero timeout, and five unviable.
This result completes the mutation requirement left open in Round 2.

### Final body and scope

GitHub still names the exact head and base branch `v1`.
I read the complete published body. Its final-gate paragraph matches the completed log and mutation result.
The body retains both red gates, the incomplete first mutation result, and the historical A5-4 limit.
The body retains all twelve removals and the EV-5 cause record.
The separate EV-5 diagnosis, ST-6 diagnosis, and IN-9 fence work remain open under their existing ownership.
The final gate does not close those separate tasks.

The completed exact-head gate and final body now satisfy the package's remaining review conditions.
Sol's completed-gate review remains separate. The lead controls the merge.
I changed no product code. I ran no build, test, gate, mutation job, or executable merge check.
I inspected the supplied log and body and used read-only artifact parsing.

VERDICT: CLEAN
