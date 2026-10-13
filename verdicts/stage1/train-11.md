# Train 11 integration review

## Composition inspection: package deltas pending

- Reviewer: Sol-C.
- Lead assignment: `msg_plugin-w_1791847277_9b68b9`.
- Exact head: `131024f3936898a3106af299e6751553c55bbc82`.
- Tree: `3934701063c7651b5ac2c62194721ad4b36801c0`.
- Base: `644f632f194f62ad7fe14e66482ee01083706e30`.
- Full-index combined diff SHA256: `d6666d60dfc8d7894f33b949c30274869b8d1c5a43b7830f55d6dee77017273d`.

The frozen order is:

| Member | Frozen head | Insertion |
| --- | --- | --- |
| #298 | `7a8592fb493450c8b046a076cf484317eb58d96e` | `2d544e361beb2fffba21330c083899209f81a89e` |
| #320 | `48ff7a038d7c466776ae114acf32fd4f2f6d22da` | `131024f3936898a3106af299e6751553c55bbc82` |

Each insertion has the preceding train state and frozen member as parents. Both unique merge bases equal the train base.
The first insertion's tree equals the #298 member tree exactly. The combined diff has 69 paths, 7163 additions, and 395 deletions.
The whitespace check passes. The members overlap in sixteen paths, including two lists and shared host, link, worker, and test files.
Every member's own added/deleted line sequence survives its insertion exactly. The later insertion removes no #298 edit.
The only declared conflict is core-real-pending. Its resolution keeps both blocks, including the R-109 re-entry comment and ST-7 cost entry.

### Complete applicable prior records and bodies

I read the complete prior #298 integration history at `415a3fd5`, verdicts/stage1/st-stall-clock.md, including current Round 7.
I read the complete applicable package import sections at `4df1d162a66cff477d5da868cbe0195ad6ea2dbf`, verdicts/st-stall-clock.md.
The current package section is Round 8. The earlier imports and modes repair retain their original source and evidence scopes.
The five original findings and later clock-post and body findings remain closed under those records.
The lead's accepted rule remains add at post, remove at consume. Historical Mac failures and reversal provenance remain unchanged.

I read the complete #320 integration history at `eadea000`, verdicts/stage1/p5-read-facts.md, including current Round 5.
I read complete package Round 4, its matching integration acknowledgment, and Round 5 at `39bbf0f9d277673587c40d6047f696ff3112aa91`.
The six numbered findings remain closed under those records. The accepted fitting choice and R320-Q4 bound-plus-test scope remain explicit.
The fixed-cap case remains deferred. The lead now calls its ceilings proposal backlog B-7, on hold; the historical A26 name supplies no amendment acceptance.

Both GitHub MCP bodies are truncated. I read the complete bodies through the authorized GitHub CLI fallback.
#298's exact body UTF-8 SHA256 is `71d88f6ca894e187d77bbca91e466b861922cc6f8fab049bba224d346391143d`.
#320's exact body UTF-8 SHA256 is `d4e137ba0c3e46df85a35b2d6b616c5e090b6942d99a565efc980a66f5a7ebbf`.
The returned member heads match the frozen heads. The bodies retain their historical gate identities, failures, local limits, and pending train gate.
No historical member gate is relabeled as execution of this train.

### Independent composition checks

I read both complete sixteen-path overlap deltas and the final shared paths that join report, deadline, close, and read behavior.
The host retains due-step budget and scheduler rules, slot reservation and takeover, report ownership, and the parked-event guard.
The ordinal loader adds its registry read at open. It does not replace the injected host clock or due-step ordering.
The new shared frame formula supplies both launch and driver bounds. The control-room formula still uses that engine bound with saturating arithmetic.
The final link enum retains the stall messages, progress marks, required cursor ordinal, and completion observation together.
The new observation is keyed Activity; it is not a mandatory slot-consuming transition in needs_room.
The host still updates client revision on receipt, but posts receipt or completion Activity only for a live route of the reporting session.
A closed route's facts status update therefore cannot recreate its retired keyed Activity through either observation arm.

The worker retains one ordered report path. report flushes waiting output and route progress before a later report.
report_output also flushes route progress. ReadFacts returns through the existing Done path, preserving those observation fences.
ReadFacts reads model and input facts at one worker ordering point. It does not arm, disarm, or advance the host's stall clock.
The accepted shared frame bound includes the facts term and retained overflow allowance, with the fixed-cap limit still explicit.
The route keeps both queue classes/stall state and facts owner/receipt fields. Both constructors initialize those fields.
A stall drops only unstarted droppable output/modes/baseline frames. Kept input responses retain their queue class.
The close path appends queued input status updates through discard_received and retains the first route close reason.
An active input can complete after the close; the host's live-route guard remains applicable to its later completion observation.
The model, facts, receipt, ordinal, and decoder implementations retain their accepted member blobs outside shared overlap paths.
All test modules remain included. The existing stall, read-facts, route-facts, report-order, and exit-watch proofs remain present.

I check all nine conformance text sets after both insertions. All eighteen three-way formulas pass.
Real proofs use complete proof rows, not their shared identifier field. The deferred TOML equals the base.
Final counts are ledger 692, pending 361, real-pending 45, proofs seven, minimum 69, held seven, real-only 26, deferred three, and withdrawn 27.
#298 removes exactly six default pending IDs, adds those six to real-pending, and retains the separate authorized R-109 re-entry.
#320 removes exactly eighteen ST-7 default pending IDs and retains the byte-cost ID as real-pending.
Neither member changes minimum IDs. The three added ST budget holds stay pending and require full-seed held-full evidence.
The .46 pin, accepted known-flaky data, and mutation configuration remain the base's exact blobs.
The only own CI change registers ordinal_decoder and receipt_decoder in the existing harness table.
The final gate must actually run those decoders, both link decoders, and both testkit decoders for the changed crates.
The existing stable property and root-test requirements remain applicable. No new check family enters this composition.

The R-93 no-gate/no-accept-limit/default-queue premise remains applicable to route_frames_taken.
The route_reports progressed control keeps separate state. No new transcript or future pin enters this train.
R-109's seven affected real-pending IDs retain their fence debt. Passing unfenced real trials cannot close that debt.
The accepted #328 dp3 removal remains absent from real-pending. The ST-7 unsupported real cost control remains pending.
The future P6 pin and #321 composition remain separate. No DP-8 adoption implementation or A24 closure is accepted here.

### Remaining requirements

No new composition finding is identified in this inspection. Both complete same-head package deltas remain due before formal member or combined CLEAN.
The lead asked the package seats for those deltas. Sol-C owns both integration deltas, the combined review, and ONE final train receipt.
I withdrew requests for extra integration-seat rounds. Sol-B acknowledges that withdrawal in `msg_plugin-w_1791847999_811f57`.
No redundant review round from the original integration seats is required for this assigned scope.
The lead's detached warm-trial gate remains unreviewed until its completed exact-head log arrives.
The exact gate, final receipt, equal-tree fast-forward, and remote verification remain due. No completed merge follows from this inspection.
I ran no build, test, gate, pool job, mutation, reproduction, measurement, or executable merge-check job.
No formal verdict or NOT CLEAN round follows from this pending inspection.


### #298 matching package and integration source delta

I read the complete train-11 #298 package section at `fb322cf808f7578d99dd71bbbcb4952f55b4f10a`.
Sol-B supplies that reference in `msg_plugin-w_1791848213_e5a3f4` and confirms that no additional integration-seat round follows.
The package matches the complete inspection above and accepts #298 source preservation with zero open findings.
Sol-C's matching Round 1 is recorded in verdicts/stage1/st-stall-clock.md in this publication.
The #320 package delta remains due before combined source CLEAN. No train gate or final receipt is accepted yet.


## Round 1: combined source CLEAN

- Exact head: `131024f3936898a3106af299e6751553c55bbc82`.
- Tree: `3934701063c7651b5ac2c62194721ad4b36801c0`.
- Base: `644f632f194f62ad7fe14e66482ee01083706e30`.
- Complete combined inspection: `0e18f97e`, above.
- #298 package train delta: `fb322cf808f7578d99dd71bbbcb4952f55b4f10a`, read in full.
- #320 package train delta: `c0d31d32b31514187ab872f084b90e88cf51c2c9`, read in full.

The frozen order, parents, common bases, combined diff hash, and complete preservation checks remain unchanged from the inspection.
The #298 matching integration delta is Round 1 at `4353366a`, verdicts/stage1/st-stall-clock.md.
The #320 matching integration delta is Round 1 in verdicts/stage1/p5-read-facts.md in this publication.
Both complete same-head package deltas match those integration deltas and the combined inspection.
Neither package has an open finding, including LOW. Both original finding histories and all accepted closure limits remain preserved.

All member edits survive the sixteen-path overlap. No unresolved source or list conflict remains.
The shared ordered report path preserves progress, output, receipt, completion, and read-reply behavior.
The host's ordinal lifecycle retains the injected stall clock, mandatory reservations, takeover, and add-at-post/remove-at-consume rule.
The classed route queue retains its input responses; close paths preserve facts status updates and live-route Activity guards.
The shared frame formula retains its payload floor and accepted facts term, including the explicit fixed-cap limit.
No independent new behavior or test hook follows from the composition.
All eighteen text-set formulas pass. Real proofs use complete rows. All current-base removals remain removed.
Counts remain ledger 692, pending 361, real-pending 45, proofs seven, minimum 69, held seven, real-only 26, deferred three, and withdrawn 27.
The eighteen ST-7 default removals, six ST default removals, separate R-109 re-entry, and three pending ST budget holds retain their exact scopes.
No pending-real trial can replace the required fenced transcript or unsupported-control disposition.

The combined source is CLEAN with zero open findings, including LOW. No exact train gate is supplied or accepted in this source round.
The merging gate must retain full-seed evidence for all six budget-held IDs and actual selected decoder fuzz execution.
Both new decoder harnesses, both link harnesses, and both testkit harnesses are selected for the changed crates.
Sol-C owns the ONE final whole-train receipt after reviewing that completed exact-head gate.
The lead must then verify the equal-tree fast-forward and remote push. No completed merge is claimed here.
No future P6 pin, #321 composition, DP-8 implementation, B-7 ceilings amendment, or A24 closure is accepted.
I ran no build, test, gate, pool job, mutation, measurement, reproduction, or executable merge-check job.

VERDICT: CLEAN (0 open, including LOW; complete combined source only)


### Matching #320 package acknowledgment

Package message `msg_plugin-w_1791848556_a8157b` confirms the complete matching source receipt at `e1ff831c0282fefd60b24162706ae924a1a9ff09`.
I read that complete acknowledgment in verdicts/p5-read-facts.md.
The package reviewer read complete #320 and combined verdicts at `db371fd0efa05dfec92684fedaa2a11c81dd07b4` and reports remote publication verified.
The acknowledgment matches the exact train head, tree, base, frozen order, both member records, all closures, and source-only limits.
No source change, new review round, or execution evidence follows. The exact gate, ONE final receipt, and lead merge checks remain due.


## Round 2: FINAL whole-train receipt

- Exact head: `131024f3936898a3106af299e6751553c55bbc82`.
- Tree: `3934701063c7651b5ac2c62194721ad4b36801c0`.
- Gate base: `644f632f194f62ad7fe14e66482ee01083706e30`.
- Complete combined source: `db371fd0efa05dfec92684fedaa2a11c81dd07b4`, Round 1 above.
- Lead gate request: `msg_plugin-w_1791850372_14f289`.
- Gate log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-detached-131024f3-131024f3-pool-20261012-164112-3401.log`.
- Verified SHA256: `fd60a82b322478830a6f87d098ea096d04fb019c8b44925443dd6117a8497e94`.

The frozen order remains #298 7a8592fb, then #320 48ff7a03, with full identities and insertions recorded above.
The detached gate header names the reviewed exact head and base. The tree remains the reviewed combined tree.
The job uses target volume botster-core-target-v1-5a6df720 under the lead's warm trial.
The log records the exact-head snapshot command, base reference, clean pinned submodules, and dependency fetch.
It supplies no independent target-writer history. I accept the completed exact-head gate within the lead's stated trial scope.
Both parallelism caps are four. The job runs on msa1. Its stated Mac-only compilation limits remain applicable.

All ten steps pass. Default records 2245 passes and 388 skips. Slow records 711 passes and 2082 skips.
Every ordinal in both tiers appears exactly once. Stage 2's unmutated baseline records all 723 normal passes before mutation trials.
Conformance reports 305 passes and zero failures. The strict run executes 274 of 291 pending transcript IDs, with 17 real-only and seven held.
The minimum report counts 69 of 69, including one budget-held ID. Every named real-process minimum proof has normal PASS evidence.
The real report states real-passing 67/68 and real-accepted 68/69. These reported counts supply no new pending removal or fence closure.
The ignored real report retains 45 pending IDs and reports 32 passes. The R-109 debt and unsupported ST-7 real cost control remain explicit.

All 192 unique held-full trials pass: six budget IDs at every seed from zero through 31.
The recorded maximum durations are:

| Budget ID | Maximum seconds |
| --- | ---: |
| dp_5_full_input_queue_stops_reading_the_route_and_never_drops | 0.358 |
| dp_5_unstarted_terminal_query_frame_retires_when_the_query_resolves | 1.274 |
| ev_8_every_query_stays_in_output | 1.280 |
| ou_2_route_state_machine | 0.112 |
| ou_2b_failed_reasons_are_reported_to_the_host_and_route_ended | 0.112 |
| ou_3_stalled_reader_resyncs | 0.114 |

Every trial remains within two seconds. The three ST budget holds stay pending; this gate does not remove their accepted temporary status.
All six #298 default removals and all eighteen #320 default removals have normal PASS records.
All seventeen required ST-7 real forms pass normally. The remaining byte-cost ID retains its unsupported real-control disposition.
The clock-post, modes-drop, overflow-gap, protected-pair, exact-room, closed-route Activity, and large-tail reply closure tests pass normally.
No source finding or earlier closure reopens through this execution evidence.

I match all 483 unique raw mutation outcomes. Final outcomes are 428 caught and 55 unviable, with no miss or timeout.
All 428 unique catches match citations and normal PASS records. Stage 1 catches 425 and leaves five misses for Stage 2.
After its unmutated baseline passes, Stage 2 catches three and reports two unviable.
The three Stage 2 catchers have normal PASS records, including the real route control and route_reports paths.
Unviable outcomes supply no catch credit. No FLAKY or RETRY trial appears in the gate's runtime record.
The only INCONCLUSIVE text describes the existing R-111 exclusion, which retains its accepted scope and supplies no catch credit.
No known-flaky entry, retry authority, or new exclusion enters this train.

The fuzz step actually completes all six selected harnesses. Each is configured for 60 seconds and reports 61 seconds:

| Harness | Runs |
| --- | ---: |
| ordinal_decoder | 37316226 |
| link_decoder | 5613636 |
| msg_decoder | 3341661 |
| frame_ends_decoder | 2623102 |
| sock_diag_decoder | 4212048 |
| receipt_decoder | 4907505 |

The existing CI uses the pinned nightly and fails on command failure. No selected decoder is skipped.
CI takes 1773.6 seconds. The job and wrapper exit zero after 1874 seconds, with 77 seconds queued and 1797 seconds running.
The earlier member gates and local producer reports retain their original heads and limits.

This is the ONE final whole-train receipt under BUILD Green. All applicable source and package findings remain closed, including LOW.
The unchanged frozen members need no redundant final package receipts. No #331 code or future P6 pin enters this composition.
The lead must verify the equal-tree fast-forward and remote push. This receipt does not claim a completed merge.
The R-109 fence debt, DP-8 adoption limit, B-7 ceilings hold, Mac evidence limits, F28 duty, and A24 gap remain outside this acceptance.
I ran no build, test, gate, pool job, mutation, reproduction, measurement, or executable merge-check job.

VERDICT: FINAL CLEAN (0 open, including LOW; complete combined source and exact-head train gate)
