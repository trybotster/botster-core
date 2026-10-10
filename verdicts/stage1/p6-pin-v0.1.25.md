# PR #226 — Contracts pin move to v0.1.26

## Round 1 — 2026-10-10

Reviewed head: `87c42db4d2f81ffcb31a4862fab5a410f5ab32ab`.
Review and gate base: `7caf3457a04bd37f5d4e844db1900525eea7c8ec`.
Tier: HIGH. The PR moves a contracts pin and changes gate decisions and mutation exclusions.
Scope: all six changed files, the new list exception, contracts inputs, and retained gate evidence.

### R1-1 — HIGH — The list exception does not enforce a failing result

Locations: `xtask/src/lists.rs:317` and `:432`.

Plan 23u permits a re-pended id only when the pin moves, its transcript changes, and the id fails at the new tag.
Both new growth decisions check only the first two conditions.
Neither decision receives a test result.

The PR assigns the third condition to #218, which has not landed in this head.
The tree has no `xtask/src/pending.rs`.
`ci::lists_job` checks only that including ignored trials does not change the passing count.
`tests/suite/mod.rs::never_passes` returns an ignored completion without executing a pending transcript.
A passing id can therefore become pending when its transcript changes, and the gate does not test that id again.

For example, this pin changes the transcript of `conf::ou_3_progressing_reader_lossless`.
The exact-head gate passes that id in 0.717 seconds.
The new decision would permit adding it to `core-pending.txt`, although it passes at the new tag.
Such an addition would also need removal from `core-real-pending.txt` to satisfy the list membership checks.

The real-tier path has the same missing condition.
`pending_real` executes real-pending ids but reports a pass as ignored.
`ci::real_report_ran` checks only binary success and the report prefixes.
It does not reject a newly re-pended id that passes.
Thus landing #218 alone would not close the real-tier gap.

P6 confirmed both gaps in `msg_plugin-w_1791655193_0c8b11`.
The lead selected option A in `msg_plugin-w_1791655218_4ca368`:
remove the exception from #226 and make #226 a plain pin move.
The exception belongs in a separate HIGH PR after #218, with a strict real-tier check.
Re-review the removal delta and its gate evidence on the replacement head.
Status: OPEN at this head.

### Remaining review

All seven workspace dependencies name `contracts-v0.1.26`.
All nine contracts packages in the lockfile name commit `0ac061d225724f6f7fc7b2368f637ba2c9a6c2b6`.
The lockfile changes only those source fields.
The annotated old and new tags resolve to the commits stated in the PR.

Between the tags, 33 Core transcripts change.
The ledger, pending, deferred, withdrawn, replacement map, and current manifest have identical bytes.
The Core pending membership is unchanged.
The real-pending, ledger, real-only, deferred, minimum, and copied status files have unchanged bytes against the gate base.
The status test changes only its tag description.
No Core runtime code changes.

I read the changed list code, its tests, and the revised shell exclusion.
The new tests cover tag movement and changed transcript membership, but not the missing failing-result condition.
The Prior art note cites the earlier pin moves and the existing list decisions.
I compared all four merge commits with their automatic merge trees; each tree matches.
The gate base is the single merge base and is an ancestor of the reviewed head.
`git diff --check` passes.

### Evidence and limits

Gate: `botster-core-stage1-p6-pin-v0.1.25-87c42db4-pool-20261010-104949-96536.log`.
The log names this exact head and base and reports all ten steps passing.
Default: 1,486 tests passed. Slow: 381 tests passed.
Conformance: 193 passed, zero failed. Pending-real: 87, with zero passes.
Minimum counts remain testkit-passing 50/69, real-passing 29/68, and real-accepted 29/69.
Both mutation commands report 29 caught, zero missed, zero timeout, and zero unviable.
The passing gate does not close R1-1 because the final head adds no pending id.

I read the retained `8cf3fac4` reversal log and its source change.
The reversal refuses the earlier `a2_3` re-pend; the restored code accepts it.
This proof covers the first two exception conditions, not the missing third condition.
The package verdict for this exact head was pending when I wrote this round.

I changed no product code.
I ran no builds, tests, gates, mutation jobs, or reversal jobs.

VERDICT: NOT CLEAN (1 open)
