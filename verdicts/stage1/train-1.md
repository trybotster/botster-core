# Train 1 combined integration review

## Source review before the final gate

Exact head: `be95ef13edc6d9eccf87a8c0b2c09b9cf820963a`. Tree: `456fdd44172ee85eb58ad443ec1e3dbf9f1dd2bc`. Base: `ae49bbec2d1086f918341bc596bf8560ba27bcd1`. Lead assignment: `msg_plugin-w_1791781317_0b4065`.

Sol-B read the approved train rule in contracts PR #86 and the lead's current train record. Sol-B applied orchestrate-delivery to this combined review. This artifact reuses the existing train plan and does not create another delivery plan.

### Review contract

The train contains two frozen HIGH members: #301 at `403bbf689cc9f745a9552cc518374a6a8f2cdb32`, then #304 at `37e5ee32d326f588676b6fc727ad52c86022de61`. Both change core-pending.txt. #300 and #298 are outside this train.

#301 adds the testkit's accept-time hold_start control. P5 owns its testkit edge, control validation, release wake, and quiet observation. Sol and 0167 own its member review. #304 names the composed DP-2 real-process proof and checks the remaining route bytes to EOF. GS owns those proof and list changes. Sol-B and 017c own its member review.

The combined review must preserve the shared machines, control-link byte order, descriptor ownership, release wake, mandatory accounting, and exact pending removals. The real entry points are the testkit harness controls and the named real-process tests. The final train gate must validate their delivered combination on this exact head.

The lead owns the train, frozen heads, final gate, and base update. Sol-B owns the combined source review. Required member delta reviews and final evidence receipts remain separate. This source review authorizes no local execution or base update.

### Combined source and accounting

Sol-B read the complete twelve-file diff against current v1 and relevant callers. The train's first merge has parents current v1 and #301. Its final merge has parents `f3b80d034acf7872fa2cc4009002a2460a5d5329` and #304. Both members remain reachable. No member conflict resolution appears in this train.

Sol-B independently compared every non-list own patch with its reviewed member patch. All match after excluding blob-index and hunk-location metadata. The three #304 proof and source files are also byte-identical to its reviewed head. The combined diff contains no additional source path beyond the two members.

#301's hold remains at SimEdges::accept_link. The spawner creates the worker's control link and records its real simulated identity. Per-host Faults stores the hold. Acceptance skips that link and preserves other links' order. release_start removes the hold and signals the host wake. The existing pending link retains its bytes.

The quiet control excludes only the held workers' link reports. Worker ready work, process exits, other connection reports, and other worker links remain observable. Its new scenario observes quiet while the hello waits, then nonquiet after release and before consumption. Start stays incomplete and Starting until release. The separate host test preserves hold scope when instance IDs match.

#301 does not change the descriptor handoff method, production host or worker machine, or the real harness. Its new hold map is separate from the existing handoff hold and failure fields. The pinned attach-state transcript uses hold_start before Start, attaches while Starting, then releases and observes Running. The added control supplies that held state without changing the attach rule.

#304's two facade proofs use real Core and a stand-in worker on a real control socket. Its worker-bin proof uses the real worker. Their setup does not call the new testkit hold_start control or quiet exclusion. The composed DP-2 proof therefore retains its descriptor-receipt, host-copy-close, byte-identity, and EOF checks. #304 changes no shared helper or machine that implements #301's hold.

The shared pending file contains exactly the two own removals: a2_1_attach_state_rules and dp_2_stream_handoff_transfers_ownership_and_closes_host_copy. Sol-B independently computed the union of current-base changes and both members' changes. The result equals the train set. The comments preserve all remaining IDs. The held and real-pending files equal current v1. No contracts pin or other status change belongs to the combined diff.

The train therefore credits attach-state through #301's harness path and DP-2 through #304's three named real proofs. Neither member supplies acceptance evidence for the other's removal. The final gate must check both paths and the complete list accounting.

### Member receipts and retained limits

Sol-B read #301's original integration CLEAN `500639cb61565a1a42adadf7e46938728fdb6d81`, `verdicts/stage1/p5-hold-start.md`. Sol-B also read package CLEAN `9f100371edc7b754d0ae6d466a32db909b36be96`, `verdicts/p5-hold-start.md`. The complete own patches remain preserved in this train.

R-97's separate worker-startup clock fix remains outside #301. The lead and steward permitted that control PR before the clock fix. The identity hold and guardian_exit remain separate work. The combined review approves no workaround or additional pending removal for them.

#304's original CLEAN is `24dc5fc8188f8c055073d821bdee04e5874867d9`. Its exact-train-head package delta CLEAN is `7b98dfd7e7b5009641af8fee4edba3ebfbea1af4`. Sol-B read both. The composed proof still does not combine real Core and the real worker in one test. Its local reversal cleanup remains an after-run owner statement. The original Mac and gate evidence retain their original-head scope.

No combined source finding remains. This review does not replace either member's required delta review or final evidence receipt. The final train gate, all member final receipts, exact base-tree verification, and verified push remain due under the train rule.

Sol-B ran no local build, test, gate, mutation job, reversal, diagnostic, or executable base check. Static Git reads and ID-set comparisons supply the source checks above.

VERDICT: CLEAN (0 open; combined source only, train evidence pending)
