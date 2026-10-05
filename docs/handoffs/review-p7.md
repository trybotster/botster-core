# P7 reviewer handoff

## PAUSED — user order relayed by the lead, 2026-10-04

The lead ordered PAUSE because of the user token budget.
The reviewer had no review step in progress.
The reviewer checked that the tracked tree is clean and the verdict branch is pushed.
This section supersedes the current-state and next-action statements below.
Do not resume review work until the user or lead resumes it.

- Reviewer session: `sess-1791169236-010d-73cc61588aa2c82d9260b35b4a88629f` (Sol high).
- Branch: `stage1/review-p7`.
- Verdict file: `verdicts/p7-services.md`.
- Pushed verdict head: `ac6e612aadbd50f61b8757631f62399ea3bb06d8`.
- PR #142 last review: round 5, CLEAN on `6e8d5b3445352d2653d0fc1469c31fddb899161d`.
- PR #142 open recorded findings: none. F1 through F6 are CLOSED on that exact head.
- PR 2a last review: none. No exact head or review request was received.
- PR 2a open recorded findings: none. The reviewer has not reviewed its code or design draft.
- The reviewer ran no tests or gates.

### Exact PR #142 heads reviewed

1. Round 1: `127994133869e3b6dbbcc8dedf7a22d320225ac5`.
2. Round 2: `5a68841a88a307ad9d66f60b1088333ad0312c27`.
3. Round 3: `11a55a88d8c225c89ca639ea66c7ad026b009f43`.
4. Round 4: `d9ded716cd79e2cbe9f8770c389ab13730dcfea0`.
5. Round 5: `6e8d5b3445352d2653d0fc1469c31fddb899161d`.

The verdict file preserves every earlier finding, closure, and accepted exclusion.
No review after round 5 was started or completed.

### PENDING reviews

- PR #142: the lead permits one merge-only delta review during wind-down.
  That delta must merge `origin/v1` into `6e8d5b3` after P5 PR #162 lands.
  It must exclude the wire corrections `54819a8` and `39bdc39`.
  The lead will call the review turn; remain idle until that call.
  No exact pushed merge head or evidence has been submitted.
  The round 5 CLEAN does not apply to the new merge head without delta review.
- Wire corrections: `54819a8` and `39bdc39` are on `stage1/p7-services-wire`.
  The lead excludes these commits from wind-down work.
  They belong to a post-pause follow-up PR that needs full review.
  The reviewer has not reviewed these corrections.
- PR 2a: no submitted review is waiting.
  The lifetime design draft is `~/botster-sessions/shared/core-stage1/p7-pr2-host-services-design-draft.md`.
  The implementer offered optional comments before code; the reviewer has not read or reviewed the draft.
  Initial code review remains future work after an exact pushed head is submitted.

No submitted head is currently waiting for review.

### Reported gate state

The implementer's handoff reports a red Mac full gate on `6e8d5b3`.
The reported failures are two base-v1 slow tests, which the lead assigns to P5 PR #162.
Log: `~/botster-sessions/gates/botster-core-stage1-p7-services-6e8d5b34-mac-20261004-212801-2865.log`.
The reviewer has not reviewed that full-gate log.
The implementer awaits the post-#162 merge before a new delta review and full gate.
The current PAUSE permits no further review or gate work from this session.

## Current state — round 5 CLEAN, 2026-10-04

This section supersedes earlier current-state and next-action statements below.
All earlier findings and closures remain in verdicts/p7-services.md.

- Reviewer session: `sess-1791169236-010d-73cc61588aa2c82d9260b35b4a88629f` (Sol high).
- Reviewed PR #142 head: `6e8d5b3445352d2653d0fc1469c31fddb899161d`.
- Delta base: `d9ded716cd79e2cbe9f8770c389ab13730dcfea0`.
- Pushed verdict commit: `ac6e612aadbd50f61b8757631f62399ea3bb06d8`.
- Last section: PR #142, round 5.
- Current verdict: `VERDICT: CLEAN`.
- F1, F2, F3, F4, F5, and F6 are CLOSED on this exact head.
- The `GuardianConfig::fmt` exclusion remains ACCEPTED.
- The exact `LOG_CHUNK_BYTES` subtraction exclusion is ACCEPTED under all four tuning-ruling conditions.

The log_chunks function is the one production split path.
The behavior test covers default, mutant, and minimum chunk sizes with positive bounds, contiguous offsets, complete bytes, and wire decoding.
The full-frame assertion is removed.
The guardian test also proves a complete multiple-frame replay before Status.

The Linux log is ~/botster-sessions/gates/botster-core-stage1-p7-services-ddc72062-linux-20261004-204542-46552.log.
The build-error extract is ~/botster-sessions/gates/botster-core-stage1-p7-services-ddc72062-mutants-unviable-reasons.txt.
The focused run passes formatting, clippy, 24 lifecycle tests, two decoder tests, and one wire test.
Mutation counts (caught/unviable/missed/timeouts): guardian.rs 66/8/0/0; link.rs 27/2/0/0; log.rs 17/0/0/0; wire.rs 6/3/0/0.
All 13 unviable mutations fail compilation with recorded type errors.
The reviewer independently counted the result lists and checked the build errors.
The tested commit ddc720624f615e7570369ebbc47bf6d5844314d8 and reviewed head share tree 74c68cb611e20c37eb11b4436fc5880f755bba61.

The focused command is not a full gate.
The reviewer ran no tests or gates.
The implementer reports a FULL HOLD from the lead.
The full gate remains pending until the lead lifts that HOLD.
Do not run a gate from this reviewer session.

This CLEAN covers PR 1 only: the guardian machine, private wire, and tests.
The host services module, real edges, binary role, and conformance proofs remain later P7 work.
The earlier lead ruling classifies PR 1 as single-package.
Later cross-package PRs still require integration review.
A later commit requires delta review before CLEAN applies to the new head.

The review branch has a clean tracked tree after the push.
Send this verdict to the implementer and the terminal CLEAN event to the lead.
Wait for the next doorbell, then call receive_messages once.
Do not poll.

## Current state — round 4, 2026-10-04

This section supersedes earlier current-state and next-action statements below.
All earlier findings and closures remain in the verdict history.

- Reviewer session: `sess-1791169236-010d-73cc61588aa2c82d9260b35b4a88629f` (Sol high).
- Reviewed PR #142 head: `d9ded716cd79e2cbe9f8770c389ab13730dcfea0`.
- Delta base: `11a55a88d8c225c89ca639ea66c7ad026b009f43`.
- Pushed verdict commit: `7ea53fac6f7e7d0b80bfc9b4c68a3fc04dafda16`.
- Last section: PR #142, round 4.
- Current verdict: `VERDICT: NOT CLEAN (2 open)`.
- F1 and F2 remain CLOSED.
- F4 is CLOSED. Spawn retains the first cause byte and transfers it to Leader.
- F5 is CLOSED. Authentication queues the complete retained ring before Status.
- The one-function `GuardianConfig::fmt` exclusion remains ACCEPTED.
- F3 remains OPEN for per-file mutation counts and inspection of the unviable builds.
- LOW F6 is OPEN. The full-frame assertion fixes a contract-free tuning value to kill a mutant.

The focused Linux log for `92f7f853b7c14f338c672ffa8abd375c88a2d832` records passing formatting, clippy, 24 lifecycle tests, and two decoder tests.
It records 128 mutations: 116 caught, 12 unviable, zero missed, and zero timeouts.
The command names all four source files, but its log gives only aggregate counts.
No copied artifacts for run 203327-10747 were available during review.
The reviewer requested outcomes.json and mutants.json, or per-file counts and the 12 build-failure reasons.
The tested commit and reviewed head share tree `a279be5ef73f19c8f01cd086e931de476f096fca`.
The command is focused validation, not a full gate.
The reviewer ran no tests or gates.

F6 requires behavior tests across valid chunk sizes through one production path, with an exclusion only if all four tuning-ruling conditions hold.
The implementer may instead identify an independent binding requirement that fixes frame fullness.
The reviewer requested additional multiple-frame reconnect coverage, but this request does not keep F5 open.
The ordered replay path and current byte, bound, offset, empty-ring, and reconnect tests prove the corrected completion condition.
F3 alone must not block a permitted gate after every other finding closes.

The review branch has a clean tracked tree after the push.
Wait for the next implementer doorbell, then call receive_messages once.
Do not poll.

## Current state — round 3, 2026-10-04

This section supersedes the earlier current-state and next-action statements below.
The earlier review history remains unchanged.

- Reviewer session: `sess-1791169236-010d-73cc61588aa2c82d9260b35b4a88629f` (Sol high).
- Reviewed PR #142 head: `11a55a88d8c225c89ca639ea66c7ad026b009f43`.
- Delta base: `5a68841a88a307ad9d66f60b1088333ad0312c27`.
- Last verdict section: PR #142, round 3.
- Pushed verdict commit: `e478ddfec89ccc54c55a81619b6278a2fbf90baa`.
- Current verdict: `VERDICT: NOT CLEAN (3 open)`.
- F1 remains CLOSED. Stop grace still starts at `TermSent`.
- F2 is CLOSED by removal of the JSON log chunk code and its exclusion.
- The one-function `GuardianConfig::fmt` exclusion remains ACCEPTED.
- F3 remains OPEN. Require final-head mutation evidence for `guardian.rs`, `link.rs`, `log.rs`, and `wire.rs`.
- MEDIUM F4 is OPEN. The machine loses a cause byte received while `Spawning`.
- MEDIUM F5 is OPEN. The new log replay protocol has no observable completion condition.

The implementer accepted F4 and F5 and reports local corrections.
The corrections retain the first cause byte during spawn and queue the complete retained ring before `Status`.
The host would complete adoption on the ordered `Status`, including an empty ring.
The corrections have no submitted pushed head yet.
Review their complete delta and tests before closing either finding.

The focused Linux log for `c1f3b7a5f0ce1649711e1e0f8c6b6013362a32ae` records passing formatting, clippy, 22 lifecycle tests, and two decoder tests.
That commit and the reviewed head have the same tree: `3d9ad79aa6999c716640e5eaa06981147a6d3242`.
The focused command is not a full gate.
The reviewer ran no tests or gates.
F3 alone does not block a permitted gate after all other findings close.

The review branch has a clean tracked tree after the push.
The spawner's `.gitignore` change was restored and was never committed.
Wait for the implementer's doorbell, then call `receive_messages` once.
Do not poll.

Updated: 2026-10-04, at the lead's requested reviewer replacement.

## Branch, verdict, and sessions

- Repository: `trybotster/botster-core`.
- Review branch: `stage1/review-p7`.
- Review worktree: `/Users/jasonconigliari/botster-sessions/trybotster-botster-core-stage1-review-p7`.
- Verdict file: `verdicts/p7-services.md`.
- Last section: PR #142, round 2.
- Pushed verdict commit: `14117b4c598c0769ca3743567cc46fd73f9faddd`.
- Current verdict: `VERDICT: NOT CLEAN (2 open)`.
- Implementer branch: `stage1/p7-services`.
- Current implementer: `sess-1791168848-010b-36f58b104381b2f67cc32619dd70ba42` (Opus).
- The outgoing implementer session `sess-1791166872-0106-01e2db49c7fdac64d1229b8ffe0f78bc` is retired.
- Integration reviewer: `sess-1791168757-0109-73d2ca212653045545e7480ab60be9a9` (Opus).
- The original integration reviewer session ending in `0101` is retired. Never message it.
- Lead: `sess-1790903471-008f-8b9f78eef51d48aba5a45748495fd673`.

The reviewer pushed both verdict commits and sent round 2 to the current Opus implementer.
The review worktree was clean after the last push.
The spawner's `.gitignore` change was restored and was never committed.

## Exact heads reviewed

1. PR #142 round 1: `127994133869e3b6dbbcc8dedf7a22d320225ac5`.
   Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
   Verdict commit: `fb2e8ae95d0cc1ef7947df147a32927035d30135`.
   Result: NOT CLEAN, one HIGH finding.
2. PR #142 round 2: `5a68841a88a307ad9d66f60b1088333ad0312c27`.
   Delta base: the round 1 head above.
   Verdict commit: `14117b4c598c0769ca3743567cc46fd73f9faddd`.
   Result: NOT CLEAN, two open findings.

PR: https://github.com/trybotster/botster-core/pull/142.

## Findings and accepted arguments

- HIGH `P7-142-R1-F1` is CLOSED on the round 2 head.
  The first version started Stop grace before descendant enumeration and SIGTERM.
  A late census could skip SIGTERM and produce SIGKILL directly.
  The correction starts grace at the injected `TermSent` time.
  Tests prove delayed census, delayed delivery, and exit during census.
  The correction preserves descendant identities and the unreaped leader.
- The reviewer accepts grace after failed TERM when the leader remains live.
  The guardian attempts TERM, waits, and then attempts group cleanup.
  A failed TERM does not set `delivered_reason` or claim `HostStop`.
  The later real edge must report delivery and an exiting leader accurately.
- The `GuardianConfig::fmt` exclusion is ACCEPTED.
  It covers one exact mutation in one function.
  An empty successful debug result still omits the token.
  No contract fixes the remaining debug text.
  `authentication_controls_launch_and_hides_the_token` checks token omission.
- LOW `P7-142-R2-F2` is OPEN.
  The `Guardian::begin` chunk exclusion changes `(MAX - 128) / 4` to `(MAX / 128) / 4`.
  Its test uses only the compiled expression.
  It lacks the tuning ruling's one test across default, mutant, and minimum values.
  Remove the exclusion and kill the mutant, or satisfy all four ruling conditions.
  Name the proving test and enforced lower bound in the exclusion comment.
  Keep one production code path and clause-derived assertions.
- `P7-142-R2-F3`, mutation evidence, is OPEN.
  The old-head run reported 25 missed, 95 caught, six unviable, and no timeout.
  The delta adds guard tests and removes redundant conditions.
  No final-head mutation result proves closure yet.
  The accepted debug argument closes its one equivalent mutant.
  F2 covers the chunk exclusion.
  Final-head per-file results must cover `guardian.rs` and `wire.rs`.
  Require zero missed mutations and zero timeouts after accepted exclusions.
  Review every new equivalence argument or exclusion.

## Mutation evidence

The mutation run tested `guardian.rs` on `127994133869e3b6dbbcc8dedf7a22d320225ac5`.

Log:
`~/botster-sessions/gates/botster-core-stage1-p7-services-12799413-linux-20261004-194426-97024.log`.

Artifacts:
`~/botster-sessions/gates/artifacts-trybotster_botster_core_stage1_p7_services-e129505d-20261004194426-97024/target-mutants.out/`.

Read `missed.txt`, `outcomes.json`, diffs, and logs when assessing closure.
The outgoing reviewer inspected the missed list and source changes.
The reviewer ran no tests or gates.
The implementer reports 24 lifecycle tests, one decoder test, and focused clippy passing on the round 2 head.

## Binding inputs and rulings

Read the user's review assignment and AGENTS.md instructions.
Use these inputs:

- `~/botster-sessions/shared/core-stage1/pair-common.md`.
- `~/botster-sessions/shared/core-stage1/brief-p7-services.md`.
- `~/botster-sessions/shared/core-stage1/handoffs/core-lead.md`, sections 3 and 8 and later updates.
- Plan pin `~/botster-sessions/pins/stage1-plan.bdda2359.md`.
- `docs/stage1-clauses/p7-services.txt` on `origin/stage1/plan`.
- Contracts at `contracts-v0.1.13`, including SV-1 to SV-10, A2-5, A4-1, A6-1, service transcripts, and testkit controls.
- `~/Projects/botster-contracts/docs/BUILD.md`.

Apply these rulings:

- CLEAN applies only to an exact head with every finding closed, LOW included.
- A commit after CLEAN requires delta review.
- The reviewer runs no gates.
- The lead's gate-evidence ruling permits a gate after every other finding closes.
  F3 alone must not block that permitted gate.
  Green evidence closes a gate-only finding on the same head.
  A fix after a red result requires a new head and delta review.
- Each mutation exclusion covers one function and gives a reason and proving test.
  A mutation timeout is a finding.
- Tuning equivalence requires four conditions:
  no clause fixes the value;
  one test covers default, mutant, and minimum values;
  an enforced contract lower bound kills invalid mutants;
  one exclusion covers each function or constant and names the proving tests.
- The lead classifies PR 1 as single-package; the integration reviewer skipped this PR.
  This information came through the implementer's handoff and review request.
  Later cross-package PRs still require integration review.
- The lead requires one code path, clause-derived expected values, and no tests that exist only to kill a mutant.
  No hacks are accepted.
- Real-process tests own and kill their group on Drop and panic.
  A test guard must not reap children that production code reaps.
  Children never busy-spin and exit when their parent is gone.
  One test covers the broken-cleanup path.
- Never poll messages.
  End the turn while waiting.
  After a doorbell, call `receive_messages` once.
- `post_message` takes `{session_uuid, payload}`.
  Send verdicts directly to the current implementer.
  Send the lead only QUESTION, BLOCKED, or a terminal event.

## Scope and next action

PR 1 adds the guardian machine, private wire, lifecycle tests, decoder harness, and xtask registration.
It adds no real driver and removes no conformance id from pending.
Lane listeners, queues, epochs, readiness, entropy, and real-process proofs belong to later P7 PRs.
The PR has the required Prior art note and copies no old source.

No new head is waiting for review.
The implementer must correct F2 and submit an exact pushed head.
Review that delta and every new mutation exclusion.
Permit mutation/gate evidence under the gate-evidence ruling once non-evidence findings close.
Write a new section for each PR and round, naming the exact head and ending with a VERDICT line.
Push the verdict commit and send it to the implementer.
Send CLEAN to the lead only when every finding closes on the exact head.

The lead requested this reviewer handoff at the next idle point.
The same branch and worktree continue under a new session.
