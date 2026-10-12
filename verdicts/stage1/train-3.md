# Train 3: combined integration review

## Round 1: composition findings

- Reviewer: Sol-B.
- Lead assignment: `msg_plugin-w_1791818862_6bfb70`; train 3 precedes #313.
- Exact head: `3435a6d2ce981ec5c06a888ba907667938f50137`.
- Tree: `787fc74f540d554f8e28070456d3cc5feeaa19b2`.
- Base: `080987829a2e2245a7ea97606020400392c974d6`.
- Ordered members: #310 `204e91d820c2d719c00b3665e2d5ba6aa436b0d0`; #312 `54805661b93f7acc7c548d0a429d429247c54255`; #308 `2ac0be6fddd37b540bbe7845ae975499a58eaf57`; #309 `b920da760a8a643b199765d0a69f78533a9b3e45`.

Sol-B inspected the four ordered merge objects and began the combined source review.
The train diff has 21 paths, 1845 additions, and 79 deletions.
This is a partial combined review with two confirmed findings. It claims no completed train gate or complete acceptance audit.
#307 left this train because of conflicts. #313 remains separate.

### T3-I1 — HIGH — OPEN: #309 test uses the launch field removed by #305

At the train head, core-link/src/msg.rs:435 initializes LaunchSpec.stop_grace_ms in the new encoded-length test.
The final LaunchSpec type has no such field. Train base #305 removed it.
This struct initializer cannot compile against the final type.
The effective #309 patch retains an initializer from its earlier base without adapting it to the train base.

Required repair: remove the stale field from the test initializer and preserve the encoded-length assertions.
Sol-B sent the finding to Sol-C and package reviewer 017e.
017e independently confirms the same defect as RC-R100-TRAIN-1 in `msg_plugin-w_1791819015_afc29d`.
These are two review receipts for one code repair. Sol-B ran no compilation job.

### T3-I2 — HIGH — OPEN: the .44 pin does not retire the in_8 entry

Cargo.lock pins contracts-v0.1.44 at `99e5983befde9a96ca4cdb3faea0f7f9a4b930ea` through #312.
The following #308 merge adds the in_8 known-flaky entry, whose removal trigger is that exact pin.
The repository test still requires that entry.
The entry therefore excludes the repaired real-tier test from applicable mutation runs after its approved removal condition is met.

Required repair: remove the entry and update the exact repository-list assertion and comment on the delivered train head.
Preserve the signature schema, parser, and mutation verdict rules.
Sol-B initially sent this finding as MEDIUM, then classifies it HIGH for its changed mutation coverage and unmet mandatory removal condition.
017c independently confirms GS-FS-T3-1 HIGH in `msg_plugin-w_1791818934_2b888b`.
017e also confirms this composition condition in `msg_plugin-w_1791819015_afc29d`.
Sol-B sent the finding to GS and 017c. The accepted #308 scope requires removal when the .44 pin lands.

### Member delta checks completed so far

Sol-B independently compared #310's four accepted own patches with its first train merge.
Every added and deleted line matches after excluding diff headers and hunk coordinates.
Later train merges change none of its four own files.
The resolver reads the contracts package source from the delivered Cargo.lock; it does not hard-code the .43 revision.
The old revision in its fixture remains a synthetic input, not the delivered pin.
The production metadata parsing and slow-tier tests retain their reviewed source.
The exact-train-head #310 package receipt remains due before its delta CLEAN.

Sol-B also compared #308's three accepted own patches with its train merge.
Every added and deleted line matches; later #309 changes none of its own files.
That byte preservation does not close T3-I2: the earlier member now changes the required list content.
The #308 delta remains NOT CLEAN for the removal condition above.

The combined review, required member delta receipts, and one final exact-head train gate remain due after repair.
The lead owns the repair procedure and gate. This verdict authorizes no execution or retry.
Sol-B changed no product code and ran no build, test, gate, mutation, reversal, diagnostic, or executable base check.

VERDICT: NOT CLEAN (2 HIGH open; combined review incomplete)

## Round 2: reduced train 3c source acceptance

- Exact head: `b78df725177d1a931e1eddb9bb67023e7e0ae645`.
- Tree: `43cdf59980d93e0672423bedc0edf506405256be`.
- Base: `080987829a2e2245a7ea97606020400392c974d6`.
- First merge: `838e4bb01dda53c419ab824913fdc5adae7a16c1`, with #310 `204e91d820c2d719c00b3665e2d5ba6aa436b0d0`.
- Second merge: this train head, with #312 `54805661b93f7acc7c548d0a429d429247c54255`.
- Lead disposition: `msg_plugin-w_1791819182_2166e8`.

The lead abandons train 3 and train 3b. The reduced train contains #310 and #312 only.
T3-I1 and T3-I2 remain recorded against the abandoned head. Neither finding applies to the reduced composition.
This is not a repair or CLEAN for #308 or #309. #307 and #313 also remain outside this train.

The required behavior is a .44 contracts pin whose gate checks preserve their decisions without Cargo execution in default tests.
The contracts repository owns the pinned schemas and transcripts. Core owns pin selection, list checks, and test-tier selection.
The testkit keeps print-history serving outside this pin. The train changes no terminal model or product API.

Sol-B read the complete nine-file train delta and the affected helper and probe contexts.
The diff has 278 additions and fifty deletions.
Sol-B independently compared each accepted member patch with its ordered train merge patch.
Every added and deleted line matches after excluding diff headers and hunk coordinates.
The four #310 file blobs and five #312 file blobs each equal their accepted member versions.

### Combined interface and lifecycle

The .44 pin keeps the contracts repository and package name that the #310 resolver reads.
The resolver reads the delivered Cargo.lock source revision. It does not select the fixture's .43 revision.
The seven direct dependencies and nine contracts lock records all use .44 at `99e5983befde9a96ca4cdb3faea0f7f9a4b930ea`.
Sol-B independently verifies that this commit is the .44 tag's commit.
The pure resolver retains its single-match requirement. The wrapper starts no process.
The default job-common test retains its nonempty-copy assertion and byte comparison.
The production lists command still uses Cargo metadata. The slow resolver comparison remains selected and is due in the final gate.

The pure packages_of fixture and moved real metadata test retain their reviewed source and tier selection.
The train adds no mutation exclusion or timing allowance for these paths.
The known-flaky list remains the accepted base's empty list. The reduced train imports no signature entry or retry policy.

The pinned probe's Printed report remains opt-in through BOTSTER_PROBE_PRINTS.
The testkit environment sets only BOTSTER_PROBE_CONTROL. Its added report arm supplies compilation compatibility without serving print history.
The separate P3 serving change remains due. No new print-history oracle acceptance follows from this train.
The route-log test finds the sole route_expect_pty step by its key and retains the earlier replacement and truncation behavior.
Its inserted program_ready fence precedes route_send. The existing await_quiet remains.

### Sets and accepted evidence scope

Sol-B independently checked all nine conformance text-set formulas against the member base, member head, train base, and train head.
Every formula matches. Core-pending remains 406. Core-real-pending becomes 34 through the nine accepted #312 removals.
No identifier returns and no additional removal appears.
For all nine removals, Sol-B independently verifies normal real PASS in the accepted #312 gate and absence from the .44 fence exceptions.
This check reuses initial member evidence. It does not establish a train gate or a new full-seed real run.
The shared CI job, mutation exclusions, process allowlist, and known-flaky file retain the accepted base versions.

Sol-B read these complete source records:

- #310 package delta: `28212287cb8cae87306e46f2ae58d235b113e640`, `verdicts/gs-gate-speed.md`, train 3c section.
- #312 package delta: `8afd00632950c0e2607e5e2dbc22c76f29ed48f4`, `verdicts/rc-real-controls.md`, train 3c section.
- #312 integration delta: `ce37c0839d112d0a2dc8c25afde8cf0991abd36d`, `verdicts/stage1/rc-pin-44.md`, Round 4.
- #312 initial package: `779edadce8641e4cbcd8d2cb0588c06b1c0dc697`, its complete #312 section.
- #312 initial integration: `44742efea6f308f2d621437a69cd664632c13cb8`, both rounds.

The member source records agree with the independent composition checks and report zero findings.
The three earlier #312 body findings remain closed with their original scope.
The #310 initial gate and body acceptance remain in `verdicts/stage1/gs-no-cargo-default-tier.md`.
Its exact-train-head integration delta is Round 2 there in this commit.

### Disposition

No source composition finding remains on this reduced exact train head.
This CLEAN covers combined source only. It accepts no running or future train gate.
The one exact-head train gate, final member receipts, final evidence bodies, and lead merge/tree/push verification remain due.
The final gate must exercise the .44 resolver comparison and retain the moved metadata mutation catch.
Sol-B ran no build, test, gate, mutation, reversal, diagnostic, or executable base check.

VERDICT: CLEAN (0 open; reduced combined source only, final train evidence pending)
