# Core #317: integration review

## Round 1

- Reviewer: Sol-C.
- Branch: `stage1/p3-known-flaky-ev2`.
- Head: `9fe97f506a1bb5cef0d935b2ba2ccf3ff9ba90ec`.
- Tree: `967b0c056186ed2aeca334898247a8dcb6cd8792`.
- Base and branch point: `838e4bb01dda53c419ab824913fdc5adae7a16c1`.
- Immediate parent: `fd13652b54ed4f1f262fec6e4dd5ed61a7cab827`.
- Reviewed delta: the complete two-file diff, 17 additions and four deletions; the known-flaky entry, test assertion, governing disposition, and gate effects.
- Package verdict: `3ec18908dccaccfed7f09276e5a34edd31f24dcb`, file `verdicts/stage1/p3-known-flaky-ev2.md`, complete Round 1.
- READY receipt: `msg_plugin-w_1791824755_dd56b6`.

I read the complete same-head package verdict. It reports zero open findings. The lead assigns Sol the package review and Sol-C the integration review. The tier is HIGH because this PR changes a CI path.

### Governing disposition and interfaces

I read complete R107-SI-2 at contracts `a10b85c363b069f8e3f98cdb6597f4451454c98b`. Its ruling text equals `28275d85`. This disposition replaces R107-SI-1's EV-2 two-fix removal condition. Contracts #89 adds await_title TB before Stop; I checked that transcript and the fix's ancestry in contracts-v0.1.45. The fenced EV-2 test no longer depends on A31. The separate OU-7 amendment, identifier, and A31 evidence remain outside this PR.

The TOML contains one entry. Its binary and test identify only the real-tier EV-2 test. The signature fixes INCONCLUSIVE, step 71, and step_timeout; only the seed varies. RC owns the pin fix, and #316 is its removal PR. I checked that #316 is an open draft whose plan removes the entry and restores the empty-list test with the .46 pin. This review accepts no implementation in that draft.

This member pins .43. Its EV-2 transcript has the same unfenced form as .44. The entry applies before .45; the planned train imports .44 separately. The retained train-4 failure at seed zero has the exact recorded signature. The entry grants no exception for another test, step, or limit. EV-1 and EV-2-retired receive no entry.

The changed xtask test parses the list and asserts exactly the one intended binary/test pair. The production parser, filters, and reports remain unchanged. Mutation stages omit the listed test only when their own build contains it. The unmutated slow tier and Stage 2 baseline still run it. An unmutated failure remains RED. BUILD permits one re-gate only when its complete known-flaky conditions hold; this verdict authorizes no specific future retry.

The PR changes no production Core path, control, transcript, timeout, terminal model, conformance status, mutation configuration, or process allowance. The .46 pin must remove this entry and restore the empty-list assertion together. Stage 1 still requires an empty known-flaky list.

### Finding history and body closure

**KF317-1 — LOW — closed.** The original body labeled the local log as the final head and described the branch point as the immediate parent. My local-head request aliases Sol's finding. The local log names fd13652b with both changed files modified. It cannot prove the exact final committed tree.

Owner receipt `msg_plugin-w_1791824930_829c6f` reports the correction. I read the complete corrected body at unchanged head 9fe97f50. It labels the pre-commit working tree, preserves the two local timeouts and isolated-run results, and identifies the exact-head pool gate separately. It calls 838e4bb0 the branch point. The package verdict independently verifies these corrections. No source or body finding remains.

### Completed exact-head evidence

Gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p3-known-flaky-ev2-9fe97f50-pool-20261012-100218-65007.log`.

SHA256: `dbaae2ff15cb744c4e1c62f6a7bf38b1ea051eae2a88e739622ccbef1eef5028`.

The header names the reviewed head and base. All ten steps pass. Default tests report 1936 passes and 431 skips. Slow tests report 621 passes and 2069 skips. The revised list test and real EV-2 test have normal PASS records. No retry or FLAKY token appears. The mutation listing finds no mutant, so no mutation run starts. Fuzz finds no changed decoder crate. Those PASS results prove no measured mutation exclusion; the unchanged filter defines its effect.

CI takes 160.0 seconds. Job and wrapper exit zero after 169 seconds on msa1. The earlier empty-list gate remains RED, and the stopped intermediate gate supplies no acceptance. The local predecessor-plus-diff record remains supporting evidence only. I use the exact-head pool gate for current-head acceptance.

Zero integration findings remain. This CLEAN accepts the exact #317 source, corrected body, and completed gate. The lead's ancestry, equal-tree merge, and push verification remain due. The new train 4b, its base delta, fresh gate, and final member receipts remain separate. Old train 03709a0c remains RED and will not be re-gated. I ran no build, test, gate, or pool job.

VERDICT: CLEAN (0 open)
