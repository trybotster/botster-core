# Minimum Core 71 integration review

## Source inspection: package verdict pending

PR: #332. Tier: HIGH. Reviewer: Sol-C.
Lead assignment: `msg_plugin-w_1791851961_abf755`. Delivery train: 13, outside frozen train 12.
Exact head: `6875eeb063f8d9cc7ca568f499bb6a0906e7288c`.
Tree: `1153978a085efe93f3213296c5df06b94348ea6c`.
Base: `131024f3936898a3106af299e6751553c55bbc82`.
Full-index diff SHA256: `e09fbc78d1b6ebe462233041b14f516e1300c0ca9f93edce86e3b955c1b2fbc7`.

The exact diff changes only conformance/minimum-core.txt. The whitespace check passes.
The 69 existing complete rows remain unchanged. Two rows are added, giving 71 unique IDs:

| ID | Clause | Owner |
| --- | --- | --- |
| conf::ev_1_metadata_reaches_host | EV-1 | p3-worker |
| conf::lc_9_update_metadata_is_durable | LC-9 | p1-lifecycle |

Both IDs are active ledger entries. Neither is pending, real-pending, real-only, held, deferred, or withdrawn.
Every other conformance blob, source, contracts pin, exclusion, and budget equals base.
The minimum-Hub pin 26c2c795 explicitly requires both additions for labels in story 1.
The lead handoff records that scope and assigns this HIGH minimum-list PR.
The header records the count and approved addition. Commit 6875eeb0 changes only the historical document reference.
The referenced file exists at 596082d4 on stage1/plan. It retains the earlier minimum rationale and count scope.
The new IDs' rationale is the minimum-Hub pin, as the PR body explicitly states.
The design-document prose-wrap rule does not require a design-topic split for this list-header edit.

GitHub MCP truncates the body. I read the complete body through the authorized CLI fallback.
Returned head matches this exact head. Body UTF-8 SHA256 is
`40f3a1b3d7516f7b1293a42fa3068d23823619570ad180c314b0de67bcc18092`.
It includes HIGH tier, prior art, train 13, historical gate scope, uncommitted local-list scope, and the reference fix.

The complete train-11 gate was audited in my train-11 final receipt on exact base 131024f3.
Its SHA256 is `fd60a82b322478830a6f87d098ea096d04fb019c8b44925443dd6117a8497e94`.
I check the four ordinary PASS records cited here: EV-1 testkit 0.141 s and real 0.129 s;
LC-9 testkit 0.077 s and real 0.131 s. They remain evidence on the exact base, not execution of this member.
Both new minimum IDs were already must-pass on both tiers before the list changed.

Local record: shared/core-stage1/evidence/p3-min71/check-131024f3.log.
Verified SHA256: `0a0e45f45a6964eeca619164fafb505d6b1cff75fcd9d90585dc9e046673835e`.
I read its producer script, header, focused results, full-run failure summary, lists result, and taint result.
The header names base 131024f3 with an uncommitted minimum-list edit.
The script sets strict mode and seeds zero through 31, with both concurrency caps four.
The focused local runs report two passes on each tier. Lists reports minimum 71 and exit zero; taint reports PASS.
The full local strict run reports 545 passes and 34 timeouts, exit 100. It supplies no passing minimum report.
All local Mac timeouts remain unclassified. Reported load does not establish their cause.
The producer records target removal and a later clean marker but no numeric clean exit or independent writer history.
No local result is accepted as strict, slow, timing, or final pool evidence.
The separate post-header lists success remains an owner report without a supplied exact-head log.

No source issue is found in this inspection. A formal CLEAN verdict awaits the complete same-head package verdict.
The train-13 composition, exact merging gate, assigned final receipt, and lead merge verification remain due.
I ran no build, test, gate, mutation, reversal, measurement, pool job, or executable merge-check job.
