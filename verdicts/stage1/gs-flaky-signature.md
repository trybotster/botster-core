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
