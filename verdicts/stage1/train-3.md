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
