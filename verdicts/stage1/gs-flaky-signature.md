# PR #308 integration review

## Round 1 — revised named signature fields

Exact head: `0a9a6cdedd9abb619a65a225bd0bf3d5594a2a9f`. Base: `ae49bbec2d1086f918341bc596bf8560ba27bcd1`. Branch: `stage1/gs-flaky-signature`. Risk: HIGH. Lead assignment: `msg_plugin-w_1791782721_1fa4f3`.

The lead held review of the earlier comma-clause form and required named fields. Sol-B started on this revised head. Sol-B read all three changed files, the parser, CI and lists callers, mutation filters, BUILD rule 9's new exception, and the complete PR body.

The signature is a required table. Its step must equal a name from ci::JOBS through the new step_names iterator. Its symptom must equal one of step_timeout, test_timeout, assertion, signal, or exit_code. Its seed must be "any" or a nonnegative TOML integer. Optional text must be a string with a nonempty trimmed value; the parser retains its original literal value.

The parser rejects unknown signature fields and missing or malformed required fields. The lists checker uses that parser. The mutation runner also uses it before selecting and reporting listed tests. The report displays the recorded signature beside the diagnosis, owner, and removing PR.

The change supplies metadata validation and report formatting. It supplies no matcher, failure attribution, check that a structural fix is open, or retry authorization. Those policy checks remain required by BUILD rule 9. The lead made the matcher optional for this source change; the body identifies it as follow-up work.

The known-flaky list remains empty. The stage_filter, in_stage, and absent behavior remains unchanged. The unmutated tiers and stage-2 baseline retain their behavior. No timeout, budget, seed, deadline, mutation exclusion, product path, or pending status changes.

### B308-I1 — LOW — OPEN: the body overstates refusal coverage

The evidence paragraph says the twelve new cases cover each signature refusal listed above. The test has no non-string symptom case. The parser implements that refusal through the shared string reader, but the test does not exercise every refusal named in the paragraph.

Required closure: describe the actual selected cases and their scope. A new test is not required. Sol-B independently confirms package GS-FS-R1-1 and sent the wording request to GS and 017c.

Package verdict: `9b33f543bbce923ae7796151006e0dd096043e7d`, `verdicts/gs-gate-speed.md`, on this exact head and base. Sol-B read the complete #308 section. It reports the same LOW finding and no parser or filter defect.

### Evidence and remaining requirements

The body reports a local Mac run with 318 of 320 tests passing and two base-merge test timeouts. It reports clippy success and lists success. These remain implementer reports; this review accepts no completed exact-head gate. The broader local run is not green.

The completed gate remains required before CLEAN, as the lead states. The body correction and final evidence also remain due. This review authorizes no execution or retry. Sol-B ran no local build, test, gate, mutation job, reversal, diagnostic, or executable base check.

VERDICT: NOT CLEAN (1 open)

## Round 2 — authorized entry and completed gate

Exact head: `2ac0be6fddd37b540bbe7845ae975499a58eaf57`.
Source merge base: `ae49bbec2d1086f918341bc596bf8560ba27bcd1`.
Recorded gate base: `be95ef13edc6d9eccf87a8c0b2c09b9cf820963a`.

Sol-B read the complete source delta from Round 1 and the complete updated PR body.
B308-I1 is CLOSED. The body names the twelve selected refusal cases.
It states that non-string symptom and text values have no separate case, and identifies the shared string reader.
No new test was required for this wording correction.

The lead authorizes one in_8 entry, owned by PC, with removal through the contracts-v0.1.44 pin.
The entry names only botster-core::slow_conformance and conf::in_8_mode_checked_at_write_start_by_worker.
Its signature names gate step slow, symptom step_timeout, seed any, and text step=8 limit=step_timeout.
Sol-B read the earlier #306 failure on 2fa58b79. Its test, seed 0, step 8, and step_timeout match this signature.
The diagnosis remains the lead's accepted diagnosis. Sol-B ran no reproduction.
Sol-B read the contracts fix at `3d32e06a88138500996c62fe8389b4c532186bd6` and its transcript delta from .43.
That fix adds the input-queue fence before the tested write. This PR does not import that contracts fix.

The existing filter excludes the exact listed test only from applicable mutant runs.
It retains the ordinary slow run and unmutated stage-2 baseline. The default conformance binary has a different binary ID.
A listed test cannot supply an accepted mutant catch. Each viable mutant still needs another valid catch.
No pending status, timeout, seed schedule, product code, or mutation exclusion changes.

The removal parser retains the Core PR form and adds three numeric components in contracts-v<major>.<minor>.<patch> pin.
This syntax check proves neither an existing tag nor an open fix. The signature still supplies no matcher or retry authorization.
The repository test requires the exact entry and removal trigger. Stage 1 still requires an empty known-flaky list at exit.

Package finding GS-FS-R2-1 is CLOSED. At 66369ef8, the test comment still described an empty list.
The comment-only repair at this exact head names the current entry and removal trigger, and retains the empty-list exit condition.
Sol-B read the repair and the package closure record. No corresponding documentation defect remains.

### Completed exact-head evidence

Gate log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-gs-flaky-signature-2ac0be6f-pool-20261011-224550-12724.log`.
SHA256: `6bd21363c0e946b522bf80d177c4fd3bc6e47cddb060a696cb7a4616f04c3309`.
Sol-B read the exact-head header, lists accounting, tier summaries, raw mutant outcomes, citations, and final exits.

All ten steps PASS in 440.5 seconds.
Default passes 1,914 tests. Slow passes 616 tests.
The in_8 transcript passes in both tiers, in 0.115 and 0.111 seconds.
The ordinary slow run therefore remains observable despite the listed entry.

Stage 1 reports 27 mutants: 25 caught, two unviable, zero missed, and zero timeout.
Sol-B independently matched all 25 raw catch identities to their citations. Every cited catcher has a normal PASS.
Stage 2 does not run because stage 1 misses no mutant. The two unviable outcomes supply no catch evidence.
No FLAKY record appears. Fuzz reports no changed decoder-harness crate.
The job and wrapper exit 0 after 590 seconds on msa1, with 110 seconds queued and 480 seconds running.

The body reports the completed gate with the same head, base distinction, counts, and limits.
The earlier 0a9a6cde gate stopped before completion because the entry changed the head. It supplies no acceptance result.
The local Mac result remains 318 of 320 passes, with two base-merge test timeouts.
Sol-B accepts no broader local green result or independent baseline attribution for those timeouts.

Sol-B read package verdict `7e482d16d1f84a84edf0236dab50296f300e4657`, `verdicts/gs-gate-speed.md`.
Its exact-head acceptance section agrees with these source, closure, and evidence results.
Both LOW findings are closed. No package or integration finding remains on this head.

This CLEAN accepts the reviewed source and completed initial exact-head gate.
The gate's lists step records merge base ae49bbec, distinct from gate base be95ef13.
The recorded gate base is not an ancestor of this PR head. This gate does not prove a current-base source import.
The authorized import or train procedure and final exact-head evidence remain due before merge.
The matcher remains proposed follow-up work. This verdict grants no retry authorization.
Sol-B ran no build, test, gate, mutation, reversal, diagnostic, or executable base check.

VERDICT: CLEAN (0 open; exact PR head and initial evidence, import/train evidence pending)

## Round 3: train 3 source delta

Exact train head: `3435a6d2ce981ec5c06a888ba907667938f50137`.
Tree: `787fc74f540d554f8e28070456d3cc5feeaa19b2`.
Base: `080987829a2e2245a7ea97606020400392c974d6`.
Frozen member: `2ac0be6fddd37b540bbe7845ae975499a58eaf57`.

Sol-B independently compared the three accepted member patches with their train merge patches.
All added and deleted lines match. The later #309 merge changes none of these files.
The signature schema and report retain their accepted behavior and limits.

T3-I2 HIGH is OPEN in `verdicts/stage1/train-3.md`.
The preceding #312 member applies contracts-v0.1.44, which is this entry's approved removal trigger.
The train retains both the in_8 entry and the repository test that requires it.
Byte preservation does not justify that stale exclusion. The delivered head must retire the entry and update the test.
017c independently confirms the same defect as GS-FS-T3-1.

This delta accepts no train gate and remains NOT CLEAN. The earlier LOW findings remain closed.
Sol-B ran no execution job.

VERDICT: NOT CLEAN (1 HIGH open)

## Round 4: conflicting current-base import and stale-entry repair

Exact head: `c3bd7fcd17861e6f164bbda5eb38638cab71f00e`.
Tree: `ad063c73dfdb177b66ca6d9cb8d0392a51c1d47e`.
Imported and gate base: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
Previous accepted member: `2ac0be6fddd37b540bbe7845ae975499a58eaf57`, based on `ae49bbec2d1086f918341bc596bf8560ba27bcd1`.
Merge `95b3b14cf47dc3aa54d3ff0adc57dcc6628f1e60` joins that member and the current base.
The next commit removes the stale IN-8 entry and updates the repository-list assertion.

Sol-B reads the merge objects, both conflict resolutions, follow-up, complete own delta, and full body.
Canonical report: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-gs-flaky-signature-base-merge-check-2ac0be6f-to-c3bd7fcd.log`.
Sol-B reads the complete report. Ancestry and condition 4 PASS; conditions 1, 2, and 3 FAIL.
The conflicts affect ci/known-flaky.toml and xtask/src/known_flaky.rs.
This manual delta review covers their resolutions. It does not relabel the canonical FAIL as PASS.

The three-way list has no base entry, the member's IN-8 entry, and the current base's EV-2 entry.
Sol-B independently parses the merged list and verifies that it initially contains both entries.
The final head removes IN-8 and retains only EV-2.
Every pre-existing EV-2 field equals the imported base: binary, test, diagnosis, owner RC, and removal by #316.
Its added signature is `{ step = "slow", symptom = "step_timeout", seed = "any", text = "step=71 limit=step_timeout" }`.
Those fields match the failure signature already recorded in the base diagnosis and the retained train 4 failure.
The required table adds no exclusion beyond the accepted base entry.

**T3-I2 HIGH is CLOSED on this repaired replacement head.**
The .44 pin is present. The obsolete IN-8 entry and the assertion that required it are absent.
The repository test retains the base's EV-2 test name and checks its exact binary, test, and #316 removal trigger.
The comment states the IN-8 retirement and temporary EV-2 scope.
This closes the same defect as package GS-FS-T3-1.
The abandoned train 3 head remains unaccepted; this repair supplies no acceptance for its old composition.

Sol-B independently compares the parser, signature reader, removal syntax, report, filters, and other tests with the accepted member.
All known_flaky source outside the repository-list test block is byte-identical.
The CI source blob equals the accepted member blob. Only the three approved own paths differ from the imported base.
No pin, conformance set, timeout, profile, mutation exclusion, or product path changes in the own delta.
The schema still supplies no automatic log matcher. It authorizes no specific retry.
Ordinary slow results remain outside the mutation-stage exclusion. Stage 1 exit still requires an empty list.

Completed log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-gs-flaky-signature-c3bd7fcd-pool-20261012-111432-79517.log`.
SHA256: `2b8515fdfacfb6b37e7e37797c5fe6071044910623ffc362fb848b6210b168fd`.
Sol-B reads the exact identity, base accounting, changed tests, raw mutation results, citations, summaries, and exits.
Gate and merge base both equal the imported base. All ten steps PASS in 444.6 seconds.
Default passes 1,957 tests; slow passes 633 tests. Strict conformance passes 264 with zero failure.
The parser cases and current repository-list test PASS. The contracts-checkout comparison passes in 0.122 seconds.
IN-8 passes default in 0.182 seconds and real slow in 0.132 seconds.
EV-2 passes default in 0.190 seconds and real slow in 0.119 seconds.
Those ordinary passes remove no further entry and establish no new EV-2 structural repair.

Stage 1 tests 27 mutants: 25 caught, two unviable, zero missed, and zero timeout.
Sol-B independently matches all 25 unique raw catch identities to citations. Every catcher has a normal PASS.
Stage 2 does not run because stage 1 misses none. Unviable results supply no catch evidence.
No runtime FLAKY or INCONCLUSIVE result appears. Fuzz reports no changed decoder-harness crate.
The job and wrapper exit 0 after 517 seconds on msa1, with one second queued and 516 seconds running.

**B308-I2 — LOW — historical severity label. CLOSED.**
The initial updated body labels Sol-B's T3-I2 MEDIUM; the final train 3 record classifies it HIGH.
Sol-B requests the correction in `msg_plugin-w_1791829866_252cbd`.
GS corrects the body in `msg_plugin-w_1791829882_af9636`. Sol-B independently reads the full corrected body.
It now records T3-I2 HIGH. The source head remains unchanged, and no new gate is required for the prose correction.

Sol-B reads package source and gate receipt `3797ce1721d466ca19a71cc6d242d0ee56e7f664`, its complete #308 import section.
The package reviewer then holds that CLEAN for the same body correction.
Sol-B reads the superseding receipt `83b5b8876f1df382e849fa91ac2d3425610f4690`, its complete body-closure section.
It closes the package LOW finding and restores CLEAN on this same exact head and base.
Both reviews agree on the source, gate, HIGH closure, and body correction.

The full body records the import, union, signature metadata, stale-entry retirement, canonical FAIL, and completed gate.
The local Mac results remain owner reports and include five base_merge timeouts; they supply no broad green result.
All earlier LOW findings remain closed. No source, body, or supplied-evidence finding remains on this exact head.
The lead owns the next slot or train, its applicable delta and final gate, and merge/tree/push verification.
This receipt accepts no later pin, entry removal, head, completed merge, or additional retry.
Sol-B ran no build, test, gate, mutation, reversal, diagnostic, or executable base check.

VERDICT: CLEAN (0 open; resolved import, stale-entry repair, and completed exact-head evidence)
