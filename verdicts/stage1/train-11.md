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
