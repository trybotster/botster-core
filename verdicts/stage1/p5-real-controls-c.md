# PR #239 — Real start controls and synchronous refusals

## Round 1 — 2026-10-10

Reviewed head: `a24c47e9613a6943a40d69183797580ac68b6187`.
Base: `cf3bcaa93faa240b418fa2248884a96581fd82c3`.
READY receipt: `msg_plugin-w_1791693885_99962b`.
Tier: HIGH under BUILD.md at `56bd0a5`, rule 3. This PR changes the shared testkit.
Authority: plan 23ag, the accepted P5 controls plan, and R-49 Q2/Q3 at contracts `b161db87`.
Scope: all six own changed paths, affected host and link interfaces, three real conformance IDs, and supplied gate evidence.
The package reviewer reports RC-C-F2 in `msg_plugin-w_1791694106_cbfc99`.
I read the complete package Round 1 NOT CLEAN artifact `3f282936375acf55637f18d4cd81fc8749edfc27`.
Its file is `verdicts/p5-real-controls-c.md`. Receipt: `msg_plugin-w_1791694385_77dc33`.

### SI-1 LOW — CLOSED: the required Prior art note was absent

The initial body has no Prior art section. `pair-common.md` requires one on every PR.
P5 adds the section without changing the head. It cites old Core at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
It rejects the old fake runtime failures for the real refusal control and reuses the existing RefusalLayer.
It rejects the old registry and protocol under plan 7.1. The current registry control uses Core's row decoder.
It gives R-49's reason for the new hold and explains reuse of Core's frame and message decoders.
I read the cited old fake failure methods, old row-read proof, and old registry source.
This PR copies no old code and needs no Stolen-From trailer.

The first added text incorrectly says that the tap keeps no byte copy.
`sendable` clones the decoder for inspection. `sent` feeds the accepted bytes into the persistent observation decoder.
I request precise wording. The published correction states that Core retains and later sends the held outbound bytes.
The tap has no buffer that later sends held bytes. Its decoder copies serve observation only.
I verify the corrected published body at the exact head. SI-1 closes.
The package reviewer's RC-C-F1 LOW tier finding also closes: the body now names HIGH and deciding rule 3.

### SI-2 / RC-C-F2 MEDIUM — OPEN at the reviewed head: a large Launch passes the hold

Locations: `edge_tap.rs:95`, `:421`, and `:429-432`; host `engine.rs:458-462`; host `admit.rs:80-103`.

The tap's outbound decoder uses DEFAULT_MAX_PAYLOAD, which is 1 MiB.
Core's HostDriver uses HostEngine::link_frame_bound for outbound encoding, including the Launch frame.
That bound follows the configured limits and is capped at `u32::MAX / 2`.
Default max_snapshot_bytes is 16 MiB, so Core's default link bound is 65 MiB.
Request validation checks argv presence and NUL bytes. It imposes no argv or environment length cap.
Seventeen 64 KiB non-NUL argv strings produce a frame above the tap's bound and below Core's bound.

The tap's lookahead decoder returns TooLarge after reading that frame's header.
`sendable` treats every decoder error as permission to send all offered bytes.
The first Launch byte therefore reaches the inner edge while the hold is armed.
`sent` then disables the decoder after its own error, so subsequent sends also pass.
R-49 Q2 condition 1 requires the hold to start at the first Launch byte and retain every later byte until release.
The decoder mismatch violates that cross-package requirement even if the worker later refuses the frame.
I independently trace the frame decoder, request validation, LaunchSpec construction, host encoder, and tap failure path.

Required change: inspect every frame that Core can send. A decoder failure must not silently release an armed hold.
Add an edge-boundary proof above 1 MiB and below Core's bound.
Require zero Launch bytes before release and all bytes unchanged and in order after release.
The package reviewer finds RC-C-F2 first. I confirm the same reported path.

### Other source checks

The real harness validates the session and reads the stored row through Core's decoder before arming a hold.
Only a Created row can arm before payload. Other steps are Unsupported; duplicate holds and missing releases are Bad.
Each data directory and tapped handle retains its own tap and refusal script.
The tap locates the instance through the accepted hello or the instance passed to connect_worker.
The host retains its complete outbound bytes and drains only the count accepted by the inner edge.
Descriptor sends return the endpoint on Blocked. They preserve the host's ownership of that endpoint.
The hold caches the host's last write interest and restores it on release.
The normal boundary proofs cover partial sends, descriptor prefixes, unheld links, and exact post-release bytes.
Their injected edge call records establish effects at HostEdges. They do not assert production queue internals.

The real positive proof checks a durable Starting row and recorded identity before release.
It checks payload_alive false through the existing identity-and-liveness path, rather than inferring that result from the hold.
After release, it requires Completed Running and payload_alive true.
The existing real guards retain process-group ownership and production ownership of reaping.
No production crate, contract pin, dependency, native configuration, or mutation exclusion changes.

The real fail_next control reuses the existing RefusalLayer before Core's API call.
The layer returns the scripted synchronous refusal without calling Core. Other methods pass through.
The shared script validation retains sync-column checks, occurrence validation, and one-time consumption.
The two refusal transcripts check unsupported codes and the absence of an op, event, slot, or state change.
Plain Core handles retain Unsupported controls. Only tapped handles advertise the four added controls.

### Merge and conformance accounting

Both own source merges match their automatic Git trees. Neither merge has a conflict.
Merge `5c4b7a70` has tree `de8f157b50d7833a6ee0e6facf98b7ea491b82db`.
Merge `a24c47e9` has tree `4817880cb0efc392f3f2014a77c7e7e3731a61a6`.
The accepted #228 and #227 imports remain intact. The six-file own delta has 585 insertions and 43 deletions.
The source diff check passes. No executable merge-check job runs.

Only three IDs leave core-real-pending, which changes from 83 to 80:

- `conf::ad_7_payload_launches_after_durable_identity`;
- `conf::a5_3_only_sync_column_codes_scripted`;
- `conf::a5_3_sync_refusal_has_no_op_event_slot_or_state`.

I read all three pinned transcripts. All three pass on the exact-head real tier.
They do not overlap #227's twelve pending additions. Core-pending remains 465: 395 transcripts and 70 IDs without transcripts.
The large-frame defect remains outside these currently passing transcript inputs.

### Supplied exact-head gate

Log: `~/botster-sessions/gates/botster-core-stage1-p5-real-controls-c-a24c47e9-pool-20261010-211218-74506.log`.
I read its header, results, conformance records, and all individual mutation outcomes through static parsing.
All ten CI steps pass in 1202.5 seconds. The job and wrapper each exit zero after 1913 seconds.
The job reports 701 queued seconds and 1212 running seconds.
Default tests: 1591 passed in 10.597 seconds; 485 skipped.
Slow tests: 412 passed in 97.948 seconds; 1980 skipped.
Both testkit reports show 205 passed and zero failed.
Strict pending runs 374/395, with 21 real-only IDs and two held IDs. Testkit minimum is 54/69.
Real minimum is 36/68; real-accepted is 36/69. Real-pending is 80, with zero passing IDs.

Stage 1 records 57 mutants: 36 caught, 19 missed, two unviable, and zero timeouts, in 164.8 seconds.
Stage 2's unmutated baseline passes 412 tests in 112.239 seconds; its CI baseline step takes 124.7 seconds.
Stage 2 records 19 mutants: 16 caught, three unviable, zero missed, and zero timeouts, in 725.9 seconds.
The complete stage-2 name multiset equals the complete stage-1 miss multiset. Neither stage repeats a mutant identity.
The final verdict is 57 mutants: 52 caught, including 16 in stage 2, five unviable, zero missed, and zero timeouts.
The mutation CI step takes 1017.5 seconds.
No new native-only source requires an additional Mac proof within this PR's own scope.

One MEDIUM finding remains open at the reviewed head. Both LOW body findings close.
I run no build, test, gate, mutation, measurement, reversal, or executable merge-check job.

VERDICT: NOT CLEAN (1 open)

## Correction source inspection — 2026-10-10 — Replacement READY pending

Inspected local correction: `c6f6c9b120899cd81433b3a65a55cb28849a54e3`.
Parent: `a24c47e9613a6943a40d69183797580ac68b6187`.
Receipt: `msg_plugin-w_1791694286_4f7741`; local path confirmation: `msg_plugin-w_1791694317_7ec254`.
I read the complete two-file correction: 62 insertions and 21 deletions. Its diff check passes.

The outbound decoder now uses OUTBOUND_BOUND = u32::MAX, above Core's capped encoder range.
Core can send no frame that exceeds this observation bound.
A decoder refusal now blocks the armed link, and `sent` retains the decoder's failed state.
The old proof that passed junk bytes after a decoder failure is removed because that behavior would violate the hold.

The new proof builds a Launch with seventeen 64 KiB strings.
It requires a payload above DEFAULT_MAX_PAYLOAD and below HostEngine::link_frame_bound with default limits.
It checks that a preceding frame passes, no Launch byte passes before release, and every later byte passes unchanged after release.
Its assertions inspect injected edge effects. They do not add a production test hook.
The worker's initial decoder remains 1 MiB until Launch. This edge proof makes no claim that the large argv starts a payload.
The proof establishes the R-49 first-byte hold and byte-preservation obligations independently of worker acceptance.

SI-2 / RC-C-F2 closes its reported source path at this correction.
The correction is not yet published as replacement READY. Its exact-head gate and package Round 2 artifact remain pending.
This source inspection grants no CLEAN verdict and completes no replacement round.
I run no reviewer execution job.

## Round 2 READY inspection — 2026-10-10 — Package artifact pending

READY head: `a96679e9b2df48b3d3918b6149d16060846a430f`.
Base: `08ee8f2c6e343ff47966f12fcb80fe7cd35d0a83`.
Receipt: `msg_plugin-w_1791697326_e8a951`.
Parents: inspected correction `c6f6c9b120899cd81433b3a65a55cb28849a54e3` and the new v1 base.
The base contains accepted #230 at `cc834158bfde2e87105bbd2c9620f51815d0c6aa`.

I compare the merge through Git objects. It has no conflict and matches automatic tree `d38e7b9c4763e41163c5bff0c9faba3d54856b78`.
Its complete parent delta consists of the twelve accepted #230 paths. None changes this PR's six own paths.
The own delta against the new base has 626 insertions and 43 deletions. Its source diff check passes.
The inspected large-frame correction and all prior source checks remain intact.
I verify the published full head, HIGH tier and rule 3, Prior art wording, Round 1 correction description, and replacement proof section.

SI-2 / RC-C-F2 closes in source at this exact READY head.
The tap's outbound decoder covers every frame Core's capped encoder can send.
A decoder failure holds an armed link, and the persistent decoder retains its failure.
The boundary proof requires no Launch byte before release and exact ordered bytes afterward for a frame above 1 MiB.
Core retains and sends those bytes; the tap's decoder copies serve observation only.
The proof does not claim that the large argv starts a payload through the worker's initial 1 MiB decoder.
The real small-frame proof separately checks durable identity and identity-checked payload absence before release, then Running and payload presence afterward.
The real synchronous-refusal wrapper still acts before Core. The shared argument checks retain parity with the testkit tier.
Both LOW body findings remain closed.

Exact-head gate: `~/botster-sessions/gates/botster-core-stage1-p5-real-controls-c-a96679e9-pool-20261010-221334-86497.log`.
I read the exact head and base, result summaries, conformance records, correction proof records, and all 76 individual mutation outcomes.
All ten CI steps pass in 1370.0 seconds on msa1.
The job and wrapper each exit zero after 1678 seconds. The job reports 295 queued seconds and 1383 running seconds.
Default: 1603 passed, 484 skipped, in 16.574 seconds.
Slow: 412 passed, 1991 skipped, in 116.999 seconds.
Both testkit conformance reports show 206 passed and zero failed.
Strict pending runs 373/394, with 21 real-only IDs and two held IDs. Testkit minimum is 55/69.
Real minimum is 36/68; real-accepted is 37/69. Real-pending is 80, with zero passing IDs.
All three own removed IDs pass on the real tier and in the unmutated slow baseline.
The large-frame boundary proof passes in 0.048 seconds. The real start-control proof passes in 1.372 seconds.
The imported DP-3 ID remains testkit-proven under A20-1; it adds accepted coverage and no real allocator claim.

Stage 1 reports 57 mutants: 36 caught, 19 missed, two unviable, and zero timeouts, in 262.9 seconds.
The stage-2 unmutated baseline passes 412 tests in 150.138 seconds; its job takes 163.3 seconds.
Stage 2 reports 19 mutants: 16 caught, three unviable, zero missed, and zero timeouts, in 706.5 seconds.
The complete stage-2 name multiset equals the complete stage-1 miss multiset. Neither stage repeats a mutant identity.
The final result is 57 mutants: 52 caught, including 16 in stage 2, five unviable, zero missed, and zero timeouts.
The complete mutation step takes 1135.5 seconds. The gate has no failed, timed-out, or signal-ended test result.
No own mutation exclusion or native configuration changes.

All reported findings have verified source or documentation corrections, and the exact-head gate is verified.
The formal package Round 2 artifact remains pending. This inspection completes no replacement round and grants no CLEAN verdict.
I run no reviewer execution job.
