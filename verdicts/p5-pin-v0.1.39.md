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
