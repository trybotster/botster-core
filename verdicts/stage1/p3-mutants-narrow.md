# #267: narrow four mutation exclusions

## Round 1 — 2026-10-11

- Exact head: `b3fa5d5bb59320250b0df35eaeaf997a1502f96b`.
- Base: `2b8cf08230dc3145dfe3a436bc34feee946c9b07`.
- Tree: `10465f55678228730a02c2360e86ee0cbfff4453`.
- Risk: HIGH. This PR changes mutation acceptance and crosses worker and Core packages.
- Package artifact: local commit `752307ff148e715557c9929ff5982018e63e310d`, `verdicts/p3-worker.md`, Round 172.

Sol reads the complete three-path delta, the cached published body, the package round, the affected driver branch, and route transport. Sol also reads mio 1.2.3's kqueue readiness functions from the installed source. The Git diff check passes. The prior-art note is present. The pinned old Core tree `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c` has neither affected prior-art path.

The two new tests use existing public interfaces. They add no production branch or dependency. The package reviewer closes their package scope. Sol finds no additional boundary issue in their delta. The inherited unbounded waits retain the lead's separate follow-up scope.

### RP-MN-1 MEDIUM — the remaining regex has no exact source anchor. OPEN.

This finding agrees with package F126. The regex uses `main\.rs:\d+:\d+` for `replace \|\| with && in Driver::run$`. It can exclude a future `||` mutant in the same function.

The lead's recorded 07:25 ruling rejects function-scoped anchors because they can silently exclude future same-operator mutants. The ruling requires line:column plus the stale-match check. A future exact-count check does not replace that rule.

Required change: if the exclusion remains and its equivalence is accepted, use its exact `303:52` source anchor. Preserve the stale-match check.

### RP-MN-2 MEDIUM — the macOS equivalence argument is incomplete. OPEN.

This finding agrees with package F127. The comment and body claim that a write-closed-only event adds at most one read, which returns WouldBlock or EOF.

mio's kqueue code reports read-filter EOF and write-filter EOF separately. `Routes::event` sets readability from `readable || closed || error`. The original `||` can therefore set readability on a write-closed-only event when the `&&` mutant does not.

The argument does not rule out bytes arriving after that poll batch while the peer's write direction remains open. The extra read can return bytes. It can also return Interrupted. `read_once` retains readability after either result. A byte result emits `RouteRead` and consumes allowance. Further turns can therefore differ before the next read-filter event.

Sol claims no reproduced contract failure. The source does not support the claimed complete equivalence. A missed mutation alone supplies no equivalence proof.

Required change: provide a complete equivalence argument for the reachable UnixStream cases, or remove the exclusion and address the survivor. Do not run a reproduction campaign to answer this finding.

### Evidence limits

The cached body names the exact reviewed head and base. The selected Linux record runs on `ecc24eed6e34a40d18bc5a9deabdb130adad8a2c`. Sol reads all twenty raw outcomes and their printed outcome lists. Thirteen worker mutants and four Core mutants are caught. Two Core Default bodies are unviable. The `303:52` operator mutant is MISSED. The worker command exits 2; the Core command exits zero. The zero wrapper exit does not accept the survivor.

Selected log: `~/botster-sessions/shared/core-stage1/evidence/p3-mutants-narrow/mutants-linux-ecc24eed.log`.

The import from that evidence head to the reviewed head changes only `ci/remote/job.sh`. It changes no source in the three-path own delta. This is the first integration review of #267. Sol claims no earlier CLEAN transfer.

Sol reads the exact-head gate identity, both suite summaries, the new proof results, and all ten step results. The default suite passes 1688 tests. The slow suite passes 517 tests. Both new proof names pass. The job and wrapper exit zero after 107 seconds. The package artifact reports that the gate selects no mutant. That gate cannot prove the nineteen newly eligible mutants. Sol does not accept the package's remaining Mac execution claims independently in this rejection round. No pending removal, minimum increase, or #262 acceptance follows.

Full gate: `~/botster-sessions/gates/botster-core-stage1-p3-mutants-narrow-b3fa5d5b-pool-20261011-063650-30673.log`.

Both package findings remain open. Package artifact publication failed on DNS. Integration verdict publication must also be reported separately from the local commit.

Sol ran no test, build, gate, mutation, reversal, measurement, or executable base-check job.

VERDICT: NOT CLEAN (2 open)

### Publication receipt

DNS recovers after the local round. Package receipt `msg_plugin-w_1791726527_0c8e47` confirms publication of `752307ff148e715557c9929ff5982018e63e310d`. Sol publishes this NOT CLEAN round in `d63047dedc28842d39a2a342b5dec1cd000e866c`. Both findings remain open. No later source correction is reviewed here.

## Correction source inspection — 599cff43

- Correction: `b5d73c88d70d70f67a9405a9cc1e7ee845c3a998`.
- Imported head: `599cff43d0bdfd360f1042aa2cdcd60da74c5cc5`.
- Base: `19b24d5efa9ac8e88ac17a82211eececb2c2c00f`.
- Parents: the correction and exact base above.
- Tree: `c5da63985dbd65c9313ece0b5f9389ef5fc0c31a`.
- Owner receipts: `msg_plugin-w_1791726773_6b72bb` and `msg_plugin-w_1791726800_0c374e`.

The owner explicitly says this is not READY. The owner reports a running gate. Sol reads the complete correction delta and the corrected driver path. Sol reads mio 1.2.3's epoll and kqueue flag functions.

The correction removes the `||` operator and its exclusion. `Driver::run` has no exclusion entry. This source change addresses RP-MN-1 without adding another broad anchor.

The driver now passes only `event.is_read_closed()` as the closed argument. `Routes::event` still combines that argument with readable and error events. A kqueue write-filter EOF alone no longer sets route readability. The owner acknowledges this macOS production behavior change and will state it in the body. The correction abandons the incomplete equivalence claim. It addresses RP-MN-2 through a source change, not through proof that the old mutant was equivalent.

The inspected mio epoll code sets write-closed only for EPOLLHUP or an EPOLLERR case. EPOLLHUP also sets read-closed. EPOLLERR remains an error argument. Thus this source change preserves the Linux readiness value. Sol makes no claim about a new Mac execution record.

The import adds only the three accepted #263 paths. No own path overlaps. All four own blobs equal the correction blobs. All three imported blobs equal the exact base. The own diff equals the correction's prior-base diff byte for byte: 14412 bytes, SHA256 `5ab3b2797d172ee6f7b96981357ad95990be5721f9dc095c925aca76ab05d6c5`. The diff check passes. The independent permitted static merge tree equals the recorded tree, without conflict.

No new source finding appears in this inspection. Formal closure and CLEAN wait for final READY, the corrected body, the supplied exact-head gate, and the package artifact. The earlier NOT CLEAN history remains intact. Sol runs no execution job and requests no reproduction campaign.
