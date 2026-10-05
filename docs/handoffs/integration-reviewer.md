# Stage 1 integration reviewer handoff

## PAUSE STATE (2026-10-04, lead's pause order, AMENDED: the near-done PRs finish first, in merge order; write the final handoff when the lead says the chain is done) — read this first

- Reviewer: Claude Opus integration reviewer. Branch stage1/integration-review, verdict head b2f7db0c (pushed; see git log for the latest). Worktree clean (spawner .gitignore restored, never commit it).
- origin/v1 at the pause: 144b0234fb632bcbb5176b17c2fe55f3239405df.
- Lead: sess-1790903471-008f-8b9f78eef51d48aba5a45748495fd673. Send the lead only QUESTION, BLOCKED, or CLEAN (head plus verdict commit).

| PR | Branch | Last reviewed head | Verdict file | Last verdict commit | Integration state |
|---|---|---|---|---|---|
| #164 | stage1/p5-audit-contract | b8b37a6d1161b83138a5e5e0e73bb68fc57e2db7 | verdicts/stage1/p5-audit-contract.md | 9edbc2762f8f4c007d30b9a67ab08c3256c1807d | CLEAN (round 9), sent to the lead; package CLEAN 89893b23. K1 to K7 closed. |
| #165 | stage1/p3-guard-macos | c03bcfb181d21cdf805b752a790359f63b9ef7b9 (round 10) | verdicts/stage1/p3-guard-macos.md | 37ed81588d524669b8104bffa2e787b4044dfcb8 | CLEAN, sent to the lead; package CLEAN ea4415fe. CORRECTION (9b9cc72): the CLEAN missed P5-F4 (unbounded readiness read_line and Parent::drop child.wait in process_guard.rs); tracked as C2 on the combined head; #165 must land only through #162. Round 11 at 71195e7209cc0cbee03bedb52eda3f2b83db2585 (cef28d18): P5-F4 closed; C3 MEDIUM (an_early_exit: unbounded read_line and child.wait) and C4 LOW (/bin/sleep loops in two self-tests) OPEN. |
| #163 | stage1/p3-audit-fixes | 40b63dceb6e3f3d7be69a1488ac77eccc121a071 (round 7) | verdicts/stage1/p3-audit-fixes.md | ca4d7d9 (plus notes up to 6b3bb0d) | 0 integration findings open. Waiting: the package CLEAN (F28 native mutation evidence; F33 execution of the newly collected slow tests in the landing gate). On the merge of the guard (#165), check G2/F39: the bounded anchor wait must be 2 x CLEANUP from one constant. |
| #162 (combined with #165) | stage1/p5-a10 | 59cda32f721f974dee8f8aeea1fef9472afe1d35 (round 4) | verdicts/stage1/p5-a10.md | e39b6b41 | C1 closed; C2 MEDIUM OPEN (P5-F4 unbounded waits, verdict commit 9b9cc72). Waiting: the C2 fix, then the P5 package CLEAN on the same head (the P5 reviewer was paused; I sent the lead BLOCKED); then send the lead CLEAN. |

PENDING reviews owed:
1. #165: DONE, CLEAN at c03bcfb1 (37ed815), sent to the lead.
2. The combined head: round 4 at 59cda32, C1 closed; CLEAN after both package CLEANs on the same head. (Original note: #162 merged with the CLEAN #165 head (lead ruling: P5 merges it; both reviewers do delta reviews; one Mac gate). It spans two packages, so an integration verdict is required. Check that A10's slow_real_core test and the guard modules merge cleanly, and G2 (outer wait 2 x CLEANUP) in any merged guard Drop.
3. A #164 delta, if a new commit appears after b8b37a6 (none at the pause).
4. The fork binding PR (named by the lead; not received yet).
5. #163's merge delta after the guard lands (G2/F39, collection still at test(/(^|::)slow_/)), then the CLEAN after its package CLEAN and green gate.
6. The lead's pause status docs PR (the lead will ring).

Lessons from this session (for the next reviewer):
- nextest names a test by its full module path; a slow filter must match at a module boundary: test(/(^|::)slow_/).
- Check process ownership and every error branch of new test helpers, not only the assertions.
- A merge-tree trial (git merge-tree --write-tree) is a cheap check of open-PR overlap.


Updated: 2026-10-04, at the idle point requested by the lead.
The lead relays the user's decision to move this role to Claude Opus.
No review is in progress. All verdict commits are pushed.

## Review branch and identity

- Repository: trybotster/botster-core, base v1.
- Worktree: /Users/jasonconigliari/botster-sessions/trybotster-botster-core-stage1-integration-review.
- Branch: stage1/integration-review.
- (History, Sol handoff) verdict head then: aa1d354ac7dd709b8357acb60419a1d030b93837. The current head is in PAUSE STATE above.
- Current origin/v1: 144b0234fb632bcbb5176b17c2fe55f3239405df.
- Retiring session: sess-1791143089-0101-8ce5f4942328f5697c410ea4da89c466.
- Lead: sess-1790903471-008f-8b9f78eef51d48aba5a45748495fd673.
- A pre-existing spawner change to .gitignore remains unstaged. No review changed or committed it.

## Verdict files and completed reviews

### PR #136 — P3 M1

- Verdict file: verdicts/stage1/p3-m1-v1.md. It retains all 49 rounds, findings, and closure evidence.
- Final reviewed head: da2b0494bbda711e5a67cb180ddf05c607784635.
- Final integration verdict: CLEAN, zero open, commit c9a1177bf3797b57dba0449b25e02a68becfb9f0.
- Final package verdict: 22124c35f2bb2d785f92182a90c6cfe9149d6fb4, verdicts/p3-worker.md, round 73.
- All integration findings I1-I6 and package findings F1-F24 are CLOSED, including LOW findings.
- PR #136 merged into v1 at 01fd38968b9e7605becc7e2b5088628aff52865a.
- The final review required an exact-head landing gate before merge. This reviewer did not run that gate.
- Review scope covered host/worker interfaces, control framing, injected edges, TestkitCore wiring, candidate/refusal plumbing, BUILD.md, accounting, and merge order.
- The real and testkit drivers use the same machines. M2a, M2b, real-harness dispatch, and worker paging remain separate work.
- The verdict retains all 167 original mutation dispositions and later focused Linux and native Mac evidence.
- No timeout was accepted as caught. Exact main, READ_CHUNK, OS adapter, and diagnostic formatter exclusions have recorded reasons and concrete proof.
- The b77038ac landing gate had one formatter survivor. The final head adds one formatter exclusion with prior exact-mutant slow evidence.
- The earlier default-test failure was corrected to open:{worker:null}; it retains real failure propagation through the working harness under LC-1.

### PR #141 — P6 control registry

- Verdict file: verdicts/stage1/p6-control-registry.md. It retains both review rounds.
- Reviewed head: 7e9b4ec6e7252c3ff7ec36cbff74c24fdae644fe.
- Final integration verdict: CLEAN, zero open, commit aa1d354ac7dd709b8357acb60419a1d030b93837.
- Package verdict: db280708bbdf9ecdef5df04d0dbef2bf07c6c3aa, verdicts/p6-control-registry.md.
- PR #141 merged into v1 at 144b0234fb632bcbb5176b17c2fe55f3239405df.
- The lead explicitly assigned this one-crate PR to integration review.
- Assigned checks: existing control names/arguments/results/errors; module registration without harness.rs dispatch edits; unchanged RefusalScript rules.
- The registered set remains only fail_next. Discovery and dispatch use the same registry.
- The moved parser retains every result and error. RefusalHandle cloning retains the shared script consumed by RefusalLayer.
- The pending package acceptance item in round 1 is CLOSED in round 2.
- The supplied focused Linux log reports 198 unit tests, one doc test, Clippy, and formatting passed.
- This reviewer required the full exact-head gate and supplied no full-gate result.

### PR #142 — P7 stacked PR 1

- Exact requested head: 127994133869e3b6dbbcc8dedf7a22d320225ac5.
- The lead explicitly instructed this reviewer to SKIP it. No integration verdict file exists.
- Scope: new botster-guardian-core, its design note, and one xtask fuzz registration line.
- The P7 package reviewer owns its review. Later P7 PRs touching host, worker, or testkit require integration review.
- P7 received the skip instruction. No review is pending here.

### PR #163 — P3 audit fixes (Opus reviewer)

- Verdict file: verdicts/stage1/p3-audit-fixes.md.
- Round 1 at head 0403470b8a08bdcbb6d89aab2a98ca3242c8e37b: NOT CLEAN (6 open), verdict commit d6a8204.
- Round 2 at head 82b4269bcb0d1412b94f38c818df5169617570c6, verdict commit 149a3ac: I1 to I5 CLOSED (I2 under the lead's option (b) ruling), I6 WITHDRAWN.
- Round 3 at head 800606ac2a99c42a7e25886a5aa1ccc2b88070d1, verdict commit ca088f6: 0 integration findings open.
- Round 4 at head ccba0504b8f0e18d274132e0b88b80b2e364becd: 0 integration findings open.
- Round 5 at head 1a13fd1 (xtask F33 filter), verdict commit 12bfc05: I7 MEDIUM (lib slow_tests not collected).
- Round 6 at head 0110caa0fd0396759c5dc67215a77753a0e734c7, verdict commit 5371aa7 closed I7 in source, but correction fa34b7d REOPENED I7: test(/^slow_/) misses the nested storage::slow_tests and real::slow_tests. Required: a boundary match such as (^|::)slow_, plus nextest list evidence.
- Round 7 at head 40b63dceb6e3f3d7be69a1488ac77eccc121a071, verdict commit ca4d7d9: I7 CLOSED for collection (Mac nextest list log checked). 0 integration findings open. CLEAN waits for the package CLEAN and a green gate on the final head (after the guard PR merges).
- Waiting: the P3 package verdict on ccba050. When it is CLEAN, write round 5 with CLEAN and send the lead CLEAN with the head and the verdict commit.
- P3 implementer (Opus): sess-1791168825-010a-e873347e8fdb315566b5545e5c2d418b.

### PR #162 — P5 A10, with a shared guard commit

- Verdict file: verdicts/stage1/p5-a10.md. Round 1 at head 913594cb5aa2afe858aa0ed3f0eaa5b357a373d2: NOT CLEAN (2 open), commits 559350a and 66ea0dd.
- J1 MEDIUM: the "kill only a quiet group" precondition in process_guard.rs. The anchor does one killpg and dies; a macOS fork escapee stays in the group. Required: the guard ends every member on every exit path, plus a deterministic test.
- J2 LOW: merge order with #163 and the guard PR.
- Round 2 at head a93a13cb34d295984ca9d1f8767248b719c8a8ee (reverts 913594c; tree = 1a96eee; single crate), verdict commit 0b46476: J1 MOVED to the guard PR, J2 CLOSED. No integration verdict is needed unless the merge of v1 brings in shared code.
- Waiting: P3's stage1/p3-guard-macos (J1 requirement, plus P5-F3 and P5-F4). Design accepted in outline (rounds of list, kill and await exit, with the anchor staying a member); I asked them to close the per-pid kill reuse window (Linux pidfd) or record it (macOS). Branch head c5ba07e is not yet submitted. PR #163 also waits for F33 (the gate collects only slow* binaries); review any xtask change.
- The lead moved the guard fix to P3's stage1/p3-guard-macos (integration review required). The merge order is the guard PR, then #162, #163, #142, #161. J1 follows the guard change.
- P5 implementer: sess-1791169804-0110-5d99de2d47b2ff27cfaac8fba5d16d3e. P5 reviewer: sess-1791169805-0111-87b8604455fe78bb06902df4c4e64bc3.
- Steward draft Core A15: CoreLimits.max_key_text_bytes. The I2 follow-up of #163 waits for A15 final.

### PR #164 — P5 audit fixes in the host (A1, A2, A4, A5, A7, A9)

- Verdict file: verdicts/stage1/p5-audit-contract.md. Round 1 at head c4afe5861abe96904ad80f9507b147c2da8ea98e: NOT CLEAN (1 open), commit 3a1aa39.
- K1 MEDIUM: the testkit WorkerSpawner gives each handle a new process table, so after a reopen identity_state says Absent for a live in-Sim worker. Required: one table per Workers, exits per handle, a reopen-then-Remove test, or a lead ruling.
- A trial merge with #163 ccba050 is clean (tree 9059e5f).
- Round 2 at head e6d9f487a5c815e8d3abfd922b46d722af5db5b9, verdict commit e6078c1: K1 CLOSED; K2 LOW (Prior art must name the hand-rolled dirfd atomic write and data-encoding) and K3 LOW (errno 0 in real.rs read_rows) OPEN.
- Round 3 at 26c4c2e (8bed899): K2 and K3 CLOSED. Round 4 at 737b2017df84f2d7fa3641c7e91f065b13ecc6ff: package scope only, 0 integration findings open. Round 5 at 1b1340c (2aefb09): K4 LOW. Round 6 at head 428c783967ca6e67d7cccb36db8336eb96aef004 (87ed24c, then 1 more commit): K4 CLOSED; K5 LOW (now moot). LEAD RULING: Core creates only the last component of data_dir (a missing parent fails), and fsyncs data_dir and its direct parent on every open; no other ancestor. Review the next head against it.
- Round 7 at 1bcbb38 (b95fb90): K4 closed; K6, K7 LOW. Round 8 at b8b37a6: K6, K7 CLOSED. Round 9: CLEAN at b8b37a6d1161b83138a5e5e0e73bb68fc57e2db7, verdict commit 9edbc2762f8f4c007d30b9a67ab08c3256c1807d (package CLEAN 89893b23). CLEAN SENT TO THE LEAD. A new commit needs a delta review.
- Lead: combined gate. #165 gets CLEAN at an exact head, P5 merges it into #162, then delta CLEANs from both reviewers (integration too, because it spans two packages), then one Mac gate.

### stage1/p3-guard-macos — the shared group guard (P3, lead-assigned)

- Verdict file: verdicts/stage1/p3-guard-macos.md. Round 1 at branch head e0514aac36ba865fb4c59d4e631ae63a29a169ff (no PR yet), verdict commit 4325c3f: NOT CLEAN (2 open).
- J1 is met. G1 MEDIUM: an expired cleanup is silent (the anchor status is ignored). G2 LOW: after the #163 merge, the outer 10 s Drop wait equals the anchor's 10 s CLEANUP.
- Round 2 at branch head 81ccd175806a71a11b18b79694177207256e9f33 (reserve design: an unreaped /usr/bin/true pins the pgid; the anchor leaves and sends killpg only; accepted in principle), verdict commit fbc7973: NOT CLEAN (4 open). G3 MEDIUM: kill-list-await order makes an escaped child wait out the deadline; use list-kill-await with a model test. G4 MEDIUM: a failed wait setup loops hot, and a listing error reads as empty. G1 and G2 carry over.
- PR #165 opened. Round 3 at head 36702023b8a36876ad226ac61adb63723511f28e, verdict commits 2722ce0 and 6b3bb0d: G1, G3 and G4 CLOSED; G2 moved to #163's merge delta (2 x CLEANUP); G5 LOW (macOS ESRCH-but-listed spin), CLOSED in round 4.
- Round 6 at f5f9d0f6 (113f3f8): module split and harness order. Round 7 at 08fef89d (ffcc8f5): Bounded<Driver> waits 2 x CLEANUP (the G2 shape). Round 8 at head 4c06846d039b9209cbd2d8d57144bbad807b6d98: 0 integration findings open (test ownership checked). Waiting: the P3 package CLEAN on 4c06846d; then send the lead CLEAN (this starts the combined gate).
- Round 4 at acd05de6 (bb23fd6): G6 LOW opened. Round 5 at head 97b8e94767e862b7349d8542306f91c9bed45bb0: G6 CLOSED, 0 integration findings open. Waiting: the P3 package CLEAN on 97b8e947; then send the lead CLEAN (the combined-gate plan needs it). The package reviewer's round 81 (6f2693e) has F35 to F38 open on #165; F39 (the outer wait) is assigned to #163's merge delta.
- Merge order (lead): guard PR, then #162, #163, #142, #161. Trial merges with #163 and #164 are clean.

## Open findings and waiting work

There are no open integration findings and no active review request.
No PR currently waits for this reviewer.
P6 supplied a RealCoreHarness design premise but has not supplied a PR or exact head.
This reviewer did not accept that design premise as a verdict.
P3 M2a/M2b and later P7 cross-package PRs may arrive after this handoff.

P6's proposed guard uses a prebuilt botster-test-anchor and public launch wrappers.
An intermediate process starts an anchor grandchild and exits. The wrapper reaps only that intermediate, then execs the verified binary.
P6 proposes the same shape for payload wrappers, a TERM-ignoring anchor, configured stop grace, and identity checks before group KILL.
P6 proposes a Linux subreaper that reaps only recorded detached anchors.
Review the actual source and evidence when a PR arrives. These are implementer claims, not accepted proof.
This reviewer sent P6 the required checks:

- Exec must preserve the Core-observed worker PID, exit, and signal path.
- The guard must own cleanup through panic and parent death.
- The anchor must remain in the verified group.
- Test reaping must not consume a production-owned child.
- Review anchor descriptor isolation, cleanup deadlines, recorded identity checks, and the Linux subreaper path.
- Require focused evidence for broken cleanup, parent death, and normal production reaping.

## Review conventions and rules

Read pair-common.md, brief-integration-reviewer.md, core-lead.md sections 3 and 8, the plan pin, binding BUILD.md, and v1 status/handoffs.
The current plan pin is ~/botster-sessions/pins/stage1-plan.bdda2359.md unless the lead supplies a later pin.
Use simplified technical English. Keep one verdict file per PR and append ordered rounds.
Preserve all findings and closure evidence. Each round names an exact head and reviewed delta.
CLEAN requires every finding, including LOW, closed and a package verdict on the same head.
Review only cross-package or stage-completing PRs unless the lead explicitly assigns another scope.
Do not duplicate package review unless a cross-package effect requires it.
Check the current v1 merge base. Preserve merged PRs, pins, and conformance accounting.
An id leaves pending only with the required real harness proof. Do not infer conformance completion from helpers or unit tests.
Run no gates, builds, or heavy tests. This reviewer ran none.
Read implementer logs and source. Static parsing and git reads were used for review.
Each surviving mutant needs a test kill, one-function exclusion with reason and named slow proof where applicable, or written equivalence.
No broad file/crate exclusion is acceptable. A timeout is a finding.
Native cfg code needs focused native mutation evidence. The lead requires that log in READY and the PR.
Real-process guards kill groups on Drop, panic, and parent death without reaping production-owned children.
Do not add test branches or alternate machines to production. Respect libghostty ownership of terminal semantics.
Apply orchestrate-delivery to cross-boundary work and when a new finding changes an architectural premise. Do not delegate without authorization.
Use receive_messages once per doorbell. Never poll. End the turn while waiting.
post_message takes {session_uuid, payload}.
Send verdicts to the implementer. Send the lead only QUESTION, BLOCKED, or CLEAN with the exact head and verdict commit.
The lead explicitly requests HANDOFF READY for this transfer.
Commit and push each verdict round. Never force-push. Leave unrelated .gitignore edits unstaged.
Shared git metadata is outside this worktree's writable root; git mutations required approved sandbox escalation.

## Sessions for continuation

- P3 implementer: sess-1791142478-00ff-2e34fc638a8fdb437255a2fa665f81b6.
- P3 reviewer: sess-1791169299-010e-6e279534401cc89d86c447f2fa43f4bc (replaced …0100, retired).
- P6 implementer: sess-1791136735-00fc-4b09baad3d85f4ce4397761167aced46.
- P6 reviewer: sess-1791169299-010f-cc0146e26529bfbb3589ccc55e082532 (replaced …00fd, retired).
- P7 implementer: sess-1791166872-0106-01e2db49c7fdac64d1229b8ffe0f78bc.
- P7 reviewer: sess-1791169236-010d-73cc61588aa2c82d9260b35b4a88629f (replaced …0107, retired).

The retiring reviewer stops after sending HANDOFF READY. The replacement owns later review requests.
