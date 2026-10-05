# P5 reviewer handoff

Status: The lead amended the pause for near-done reviews only. PR #162 waits for P3's P5-F4 fix. PR #164 waits for its merged head. All other P5 work stays paused.

## Saved verdicts

- Repository: `trybotster/botster-core`.
- Worktree: `/Users/jasonconigliari/botster-sessions/trybotster-botster-core-stage1-review-p5`.
- Branch: `stage1/review-p5`.
- Verdict file: `verdicts/p5-adoption.md`.
- Verdict branch head: `cf96240d8feef177b44b4712b2647bb2657fa2d4`, pushed to origin.
- The only worktree change is the pre-existing spawner change in `.gitignore`. Do not stage it.
- The reviewer ran no builds, tests, mutation jobs, or gates.

## Last round per PR

| PR | Last round | Exact reviewed head | Verdict |
|---|---|---|---|
| #162 | 7 | `59cda32f721f974dee8f8aeea1fef9472afe1d35` | NOT CLEAN (P5-F4 open) |
| #164 | 7 | `b8b37a6d1161b83138a5e5e0e73bb68fc57e2db7` | CLEAN for this PR's P5 audit scope |

#162's Round 5 source matches the earlier Round 3 CLEAN tree. The shared guard change was reverted and assigned to P3.
The Round 5 verdict was committed in `9c46b63`, then ordered correctly in `55cb3f2`.

#164 closes audit A1, A2, A4, A5, A7, and A9, including the reviewed storage and testkit changes.
Its package CLEAN is commit `89893b23ddc24d7c0ff6844a199e745bd52ec942`.
Integration issued terminal CLEAN on the same head, verdict commit `9edbc27`.
Commit `b91cd55d` records that integration confirmation. The lead received both terminal results.

## Exact review history

| PR | Round | Exact head | Verdict |
|---|---|---|---|
| #162 | 1 | `b5c4bffbcdf2bc1843f944eafbf315a9b9b200cb` | NOT CLEAN |
| #162 | 2 | `1a96eee7adc240a4ed90835dfc967e2efdd8e6b6` | NOT CLEAN |
| #162 | 3 | `1a96eee7adc240a4ed90835dfc967e2efdd8e6b6` | CLEAN |
| #162 | 4 | `913594cb5aa2afe858aa0ed3f0eaa5b357a373d2` | NOT CLEAN |
| #162 | 5 | `a93a13cb34d295984ca9d1f8767248b719c8a8ee` | CLEAN |
| #162 | 6 | `59cda32f721f974dee8f8aeea1fef9472afe1d35` | NOT CLEAN (2 open) |
| #162 | 7 | `59cda32f721f974dee8f8aeea1fef9472afe1d35` | NOT CLEAN (1 open) |
| #164 | 1 | `c4afe5861abe96904ad80f9507b147c2da8ea98e` | NOT CLEAN |
| #164 | 2 | `26c4c2ef58d7ac20b059487013da760bf34911eb` | NOT CLEAN |
| #164 | 3 | `737b2017df84f2d7fa3641c7e91f065b13ecc6ff` | NOT CLEAN |
| #164 | 4 | `1b1340cf80ed1f1aa3b05fba7d2e524c8b30de1a` | NOT CLEAN |
| #164 | 5 | `428c783967ca6e67d7cccb36db8336eb96aef004` | NOT CLEAN |
| #164 | 6 | `1bcbb38520440ee7e98910373926156ff886edd4` | NOT CLEAN |
| #164 | 7 | `b8b37a6d1161b83138a5e5e0e73bb68fc57e2db7` | CLEAN |

All reviewed PR bases above are `144b0234fb632bcbb5176b17c2fe55f3239405df`.

## Open findings and scope limits

No finding remains open in the accepted #162 A10-only diff or the accepted #164 package scope.
P5-F1 and P5-F2 close in #162. P5-F5 through P5-F11 close in #164.
The retained integration findings K1, K2, K3, K4, K6, and K7 are closed on #164's accepted head.

P5-F3 and P5-F4 transferred to P3's shared guard work. They were not waived or fixed by #162's revert.
The #162+#165 merge review found the P5-F3 precondition absent. P5-F4 still has its exact unbounded readiness read and parent wait.
C1 and P5-F12 are closed. Integration corrected its #165 CLEAN and opened C2 for P5-F4, verdict commit `9b9cc72`.
The lead assigned P5-F4 to P3 in #165. P5 must review the next combined head after P3 closes the finding.
Do not infer closure from an isolated passing run of a known flaky guard test.

The review does not close issues #155 or #157, the P1 part of #156, or P5 deliverable 2.
The non-Created live-worker adoption placeholder remains deliverable 2. Its tests do not certify it as AD-1 recovery behavior.

The lead's final K4 ruling governs storage: Core creates only the final data_dir component.
The parent must exist and be openable for its required sync. Every open syncs data_dir and its immediate parent.
Core syncs no higher ancestor; the host provides those ancestors and owns their durability.
The previous full-ancestor and writable-ancestor rules are superseded.

## PENDING reviews owed

1. **#162+#165 revised merge delta.** The reviewer checked combined head `59cda32f721f974dee8f8aeea1fef9472afe1d35`.
   It differs from #165 head `c03bcfb181d21cdf805b752a790359f63b9ef7b9` only in the A10 test and its common module.
   The C1 fix derives the outer deadline from `2 * CLEANUP` and carries the guard panic to the main test.
   P5-F4 remains open: direct readiness read at process_guard.rs:280-282 and Parent::drop wait at :237 before EOF at :290.
   P3 fixes the shared file in #165. P5 merges its next CLEAN head and sends that exact head for both delta reviews.
   The lead permits one combined gate after both exact-head source reviews report CLEAN. The reviewer runs no gates.
   Round 6 verdict commit: `69ce4d669aa274a7e021bff7395ee4c573e66d3b`.
   Round 7 verdict commit: `cf96240d8feef177b44b4712b2647bb2657fa2d4`.
   P3 then submitted `71195e7209cc0cbee03bedb52eda3f2b83db2585`. The reviewer read its source delta and raw Mac log.
   Its readiness uses first_line, and cleanup::Owned observes exit with WNOWAIT and a marked deadline before reaping the test-owned parent.
   The log reports 168 passed, then 15 selected tests passed with 9 skipped; exit 0. No combined-head gate is established.
   These source changes cover the two reported waits. F4 can close after the reviewer verifies them in the next exact #162 combined head.
   Integration then opened C3 MEDIUM for the unbounded read and wait in an_early_exit_keeps_the_group_owned_until_cleanup.
   It also opened C4 LOW for sleep loops in two self-tests. Its verdict commit is `cef28d1`.
   Neither dependency finding is waived. Require their closure before accepting the next combined head.
2. **#164 v1-merge delta.** Review the exact head after #164 merges origin/v1 and its required dependencies.
   Compare it with accepted `b8b37a6d`. Check conflict resolution and all changed package boundaries.
   Obtain integration review on that exact head. Record a new verdict round.

The current #162 combined head is reviewed and NOT CLEAN. Its next head and #164's merge head remain unknown.
Full landing checks and mutation results remain the implementer's responsibility. The reviewer runs no gates.

## Session routing and resume rule

- Lead: `sess-1790903471-008f-8b9f78eef51d48aba5a45748495fd673`.
- Implementer: `sess-1791169804-0110-5d99de2d47b2ff27cfaac8fba5d16d3e`.
- Integration reviewer: `sess-1791168757-0109-73d2ca212653045545e7480ab60be9a9`.

The lead permits only the near-done #162 and #164 merge reviews. All other P5 work stays paused. Do not poll the inbox.
After a doorbell, call receive_messages once. Send verdicts to the implementer.
Send the lead only QUESTION, BLOCKED, or terminal CLEAN with the exact head and verdict commit.
The pause order additionally requires the PAUSED acknowledgment.
