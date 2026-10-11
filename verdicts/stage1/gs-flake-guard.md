# Integration review: PR #256

## Round 1: retry profile and catch report

- Initial head: `a1b009a1b31412f5197d87086eb7b75ad336d8ca`.
- Base: `5ac94fabf41909a733267a62b692c1a4ee72bf62`.
- Parents: `d3f50a2830119775b0a4ccd887463e248e526f97` and the base.
- Tree: `7ad44e3d783a05f55c00414fbd0a719755ed5900`.
- READY: `msg_plugin-w_1791713746_89f8f2`.
- Package NOT CLEAN: `31823138`, #256 Round 1 in verdicts/gs-gate-speed.md.
- Package receipts: `msg_plugin-w_1791713833_e1185c` and `msg_plugin-w_1791713893_bfb8c5`.
- Risk: HIGH. This PR changes the merge gate's mutation verdict.

Sol read the complete six-file diff, surrounding stage selection and verdict paths, fixture proofs, and complete package round.
Sol also read design section 9 and BUILD.md Testing rule 9's explicit mutation-profile exception.
The initial source has two findings. The correction below closes both.

### FG-SI-1 HIGH: unknown status forms bypass the log check

This is package GS-FG-R1-1. Status: CLOSED in Round 2.
The initial `read_run` ignores every line that the narrow status expression does not match.
These lines therefore bypass both status validation and the baseline retry check:

```text
TRY 1 FAIL! [ 0.100s] w t::a
FLAKY 2/2 [ 0.100 s] w t::a
UNKNOWN_STATUS [ 0.100s] w t::a
```

A baseline outcome with summary `Success` and only those retry lines supplies no retry flag and can pass the report.
The code therefore does not meet the stated requirement to reject unknown status forms.
These examples describe unknown output forms. Sol does not claim that the current nextest version emits them.
Required correction: detect status candidates separately, reject an unknown envelope or status, and cover baseline and mutant reports.

### FG-SI-2 LOW: one retry does not eliminate every false catch

This is package GS-FG-R1-2. Status: CLOSED in Round 2.
The initial profile comment and module description state that a flake cannot count as a catch.
A flaky test can fail both attempts. That run can still count as CAUGHT.
Required correction: state that fail then pass removes a catch, while two failed attempts can still count as a catch.
The source and PR safety argument must preserve that limit.

### Interfaces and gate behavior

Only the `mutants` profile adds one retry. Other profiles retain zero retries.
The standalone stage-2 baseline explicitly passes `--retries 0`.
Cargo-mutants' own baseline can use the mutation profile. A recognized retry line makes its report fail.
This is a rejection after the run, not a claim that the baseline physically cannot retry.
The new report reads every scenario log through `outcomes.json`'s `log_path`.
It names each caught mutant's first final failed test and every FLAKY test with its mutant.
An unreadable log, unknown recognized status, missing catcher, or inconsistent MISSED result fails the report.
Each stage prints its report before it can accept its verdict. A report error propagates as a gate error.
Existing full identity, multiplicity, count, timeout, and stage-2 selection checks remain.
The unchanged candidate worker limitation remains outside this PR: mutations do not rebuild that worker for each mutant.

The fixture uses real cargo-mutants and nextest through the stage-1 command.
Four `half` mutants fail once then pass; the proof requires MISSED and named FLAKY results.
Four `triple` mutants fail persistently; the proof requires CAUGHT and the catching test name.
The existing two-stage fixture now requires the slow catching test name for all four stage-2 catches.
The parser unit proofs cover final status, duplicate summary lines, signal statuses, retries, reports, and unknown recognized statuses.
The profile proof checks all profiles, inheritance, and overrides. The baseline argument proof requires zero retries.
The report propagation proof checks both stages' failure paths.
No production crate, pin, transcript, pending file, minimum ID, timeout value, dependency, or mutation exclusion changes.
Pure gate parsing and configuration changes require no new Mac process proof.

The imported #254 paths are outside these six own paths.
Sol's independent static merge-tree calculation equals a1b009a1's tree, without a conflict.
The imported five-file delta contains the reviewed #254 source. The complete current-base diff retains both changes.
This source review grants no substitution for an exact-head gate after the import.

### Prior art

Sol searched OLD CORE at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
No cargo-mutants invocation or mutation gate exists there. The matching in-place names concern unrelated session behavior and sorting.
The PR reuses nextest's retry setting and cargo-mutants' scenario log paths.
The small parser connects those existing tool outputs to the existing two-stage gate; it adds no terminal model or dependency.
Sol read locally retained nextest-runner dispatcher source: retry events precede final test completion and the max-fail check.
The real-tool fixture supplies execution evidence for the pinned gate tools; the local source read supplies no separate version claim.

### Initial retained evidence

Sol read the exact-head header, relevant suite and fixture results, all 37 mutation outcomes, catch reports, summary, and completion:
`~/botster-sessions/gates/botster-core-stage1-gs-flake-guard-a1b009a1-pool-20261011-031034-60517.log`.
The header identifies the complete initial head and current base.
All ten steps pass, total 251.1 seconds. The default tier passes 1623 tests in 16.788 seconds.
The slow tier passes 465 tests in 46.883 seconds. The new flaky fixture and existing two-stage fixture pass.
Stage 1 reports 37 mutants: 34 caught, three unviable, zero missed, and zero timeout.
Sol independently parsed all full identities and multiplicities. Each of the 34 caught identities has exactly one named catch report.
The outcome and report identity sets are equal. Stage 2 does not run because stage 1 misses none.
The complete mutation step takes 111.0 seconds. Fuzz starts no trial because no changed crate has a decoder harness.
The job and wrapper exit 0 after 260 seconds, with zero queued seconds.
This green gate covers a1b009a1 only. It does not cover the correction below.

Initial source disposition: NOT CLEAN (2 open) at a1b009a1b31412f5197d87086eb7b75ad336d8ca.

## Round 2: status candidates and bounded retry claim

- Head: `db4fd197b40788a56e57eec47c97b5f005359a80`.
- Sole parent: the initial reviewed head.
- Base: `5ac94fabf41909a733267a62b692c1a4ee72bf62`.
- Tree: `0b7de42f1dcc5b3a8f2169778d94eed528e96715`.
- Package CLEAN: `0e19fdcf95aaba2b55b8e52bf5c008a97b423feb`, #256 correction round in verdicts/gs-gate-speed.md.
- Package receipt: `msg_plugin-w_1791713998_40bb05`.
- Implementer receipt: `msg_plugin-w_1791714262_3ba095`.

Sol read the complete correction, complete package round, and updated GitHub body at this exact head.
Only the nextest comments and the new parser module change in this correction.

FG-SI-1 / GS-FG-R1-1 HIGH is CLOSED.
Candidate detection now precedes envelope parsing.
A candidate starts with an uppercase letter after indentation and has no lowercase letter before its first bracket.
Every candidate must match the known envelope and status. Otherwise, the parser returns an error.
The three reported forms, malformed time, and missing fields have explicit rejection proofs.
Report proofs cover an unknown retry form in a successful baseline and an unknown mutant status.
Mixed-case information and summaries, lowercase ordinary output, and lines without a bracket remain ignored.
An unrelated uppercase candidate can produce a conservative error. That error rejects the step; it cannot add a catch.

FG-SI-2 / GS-FG-R1-2 LOW is CLOSED.
The profile comment, module documentation, and PR body state that a flaky test can fail both attempts and count as a catch.
Fail then pass removes that test's catch. The named FLAKY result still requires a structural fix.
No other stage selection, timeout, build, or verdict behavior changes in this correction.

The implementer names a failed exact-head gate:
`~/botster-sessions/gates/botster-core-stage1-gs-flake-guard-db4fd197-pool-20261011-031855-22867.log`.
Sol read its exact-head/base header, EV-5 failure trace, summary, and completion.
The slow step fails; mutants and fuzz do not run. Total time is 257.7 seconds.
The job and wrapper exit 1 after 272 seconds; the job queues one second and runs 271 seconds.
The slow test fails at seed 0, step 21: `PendingLimit`, category `capacity`, detail `pending_ops is 4`.
The implementer reports a second gate running on this same head.
The package reviewer confirms no retained plain-v1 comparison, cause confirmation, or written lead approval exists for this occurrence.
Receipts: `msg_plugin-w_1791714310_b24423` and `msg_plugin-w_1791714356_8ea3ff`.
The reviewer cannot infer the cause from `PendingLimit` alone.
The latter receipt reports that the implementer now treats the second gate as diagnostic only, without merge-evidence status.
The earlier #246 exception does not automatically authorize this new re-gate.
Plan 23al requires the written lead approval, cause evidence, and removal owner before accepting this exception.
No complete green gate on db4fd197 is accepted in this review. The exact-head gate and any exception evidence remain open acceptance work.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN (0 open) at db4fd197b40788a56e57eec47c97b5f005359a80.
This verdict covers source findings. It does not accept the failed gate or authorize a merge.


### Lead receipt after source verdict

Receipt: `msg_plugin-w_1791714428_1c2552`.
The lead does not seek a same-head 23al exception for db4fd197.
The lead proposes a v1 import after #255, a base-merge check, and one full gate on that new head.
The lead states that a v1-only merge does not fix EV-5 and cannot supply a rerun channel without cause attribution.
The lead names the prior PC plain-v1 trace and the qualified #249/#246 classification as comparison evidence.
The lead requires written GS reviewer confirmation that this PR does not alter that cause.
The lead names P3's combined #240/#243 with the .33 R-63 fence as the removal owner and PR.
Without that reviewer confirmation, the new head waits for the EV-5 removal.
The lead will submit plan 23an to specify attribution after a red gate and v1 import.
Sol has not reviewed that future plan revision, future import, or future gate.
The current source verdict stays CLEAN. Gate acceptance remains pending.


## Source carry-over: v1 import after #255

- Head: `0e222c592d17995380ec6f20449f91d71207d2fc`.
- Parents: reviewed `db4fd197b40788a56e57eec47c97b5f005359a80` and current base `c8907b74bd247002784843e1a12f6487b08c579f`.
- Tree: `e39035ef298ac20f642a7f27f4f75c64081da2c7`.
- Receipt: `msg_plugin-w_1791714694_4501cd`.

Sol read the complete retained base-merge-check:
`~/botster-sessions/gates/botster-core-gs-flake-guard-base-merge-check-db4fd197-0e222c59.log`.
Both ancestry checks and all four conditions pass.
The four imported paths are disjoint from the six own paths. Both pending files retain the base's bytes.
The script reports 46885 own-diff bytes and SHA-256 `c74d91f086dfc4fe14fda2c53f6fe68391565b31fd296ac0b1e6a38bdacf6ac2`.
Sol independently compares raw binary diff outputs: both have 46501 bytes and SHA-256 `1eb4cf9ed0766a6e957dc6af5844f7ce1184de7b8c8e54cdc498174952f99d9a`.
Those raw outputs are byte-identical. Their format differs from the script's normalized comparison; Sol does not substitute either hash for the other.
Sol's independent merge-tree calculation equals the imported tree exactly, without a conflict.
The source CLEAN carries over under the approved base-import rule. No new source delta round is required.

Sol read the lead handoff's written #256 attribution and conditional approval, recorded at approximately 03:50 PT.
It names the prior PC plain-v1 traces, the qualified #249/#246 classification, the GS reviewer's unchanged-cause line, and P3 combined #240/#243 with .33.
The GS line preserves its limits: no independent reproduction of this occurrence, no equivalence from error text alone, and possible scheduling changes from fixture tests.
The lead approves one new-head gate. Another red result requires waiting for the cause removal.
The historical `PendingLimit` versus `UnknownRoute` distinction remains explicit.
This import does not remove EV-5's cause. The approved 23an unchanged-cause case applies.
Sol has not received or reviewed a completed full gate on 0e222c59. This record grants no gate acceptance or merge approval.
Sol ran no execution job.

VERDICT: CLEAN (0 open) carries to 0e222c592d17995380ec6f20449f91d71207d2fc; exact-head gate acceptance remains pending.
