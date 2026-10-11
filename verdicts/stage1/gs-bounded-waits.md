# PR #270 — bounded worker waits

## Round 1 — source review on the new base, 2026-10-11

Head: `b038bac80c7939343433ec2e843c393f694470ce`.
Base: `1da39f7c9bb99b8cf8c8fd85355e5e48fd7b4c21`.
Parents: `7e568e20bbac33a99f01fbf3b2cdd3acb6d5b804` and the base.
Tree: `737c4e287939b33c78499e3964fafe594fcc0ad4`.
Risk: HIGH. The lead authorizes the shared helper export and exception-list reduction.

I read the complete four-file own delta, caller contexts, cleanup owners, shared helper, and live body.
I read the package's three review sections, including its source CLEAN at `77d7aa35eb64fd4f5e8bcea52d60bdbc037ec4d4`.
The earlier package CLEAN is `8f81912136d0cf85fbbc62d958ea510212b30f80`.

The change exposes `reap_within` and exports it from the crate root. Its signature and body stay unchanged.
The helper observes the exact child before its nonblocking reap. A deadline returns `None` and leaves ownership with the caller.
The worker helper adds the step, PID, and failure report. Three call sites use the existing cleanup bound and preserve their exit assertions.
The observer-parent proof uses child-only `OwnedChild`. Its drop kills and reaps only that child within the existing bound.
The change removes the custom observer owner. It removes five allow entries and adds none.
The remaining duplicate-name drop entry covers the existing worker owner. This review accepts no additional raw wait.
Existing `end_child_worker` cleanup stays unchanged. This PR does not claim that every existing worker cleanup wait becomes bounded.

The new proof checks the public error after a marked 100 ms deadline. It then stops and removes the same worker.
The proof uses the existing Blocker. It adds no sleep, polling loop, production branch, or cleanup timeout value.
The shared platform implementation stays byte-identical to the base. Existing callers keep their behavior.
The Prior art note identifies the reused wait mechanism and explains the worker's separate SIGTERM ownership.

### Base transfer

I read the complete canonical check at `gates/botster-core-stage1-gs-bounded-waits-b038bac8-base-merge-check.log`.
Conditions 1 and 4 pass. Conditions 2 and 3 fail on the crate-root export. The failed check remains failed.
I independently compare both complete own diffs. They match after removal of index lines and hunk positions.
The helper, worker fixture, and allow-file blobs match the earlier reviewed head exactly.
The sole shifted export retains the base's `alone` module and adds the same `reap_within` export.
The independent static merge produces this head's exact tree without a conflict.
Cargo.lock, both pending files, mutation configuration, and the platform implementation equal the new base.

The base is the #250 merge. Its parents are `c942593697e856017572977af0405b17de0124e2` and `2603483b518cab83d505ac6a0688b52afaccbf60`.
Its tree `309bfb121fda55dfa4864d5552e45a0c9a53debe` equals the accepted #250 tree.

### Supplied evidence and limits

I read both focused evidence records and decode their scripts without executing them.
Both records identify `7e568e20bbac33a99f01fbf3b2cdd3acb6d5b804` and base `c942593697e856017572977af0405b17de0124e2`.
The Mac record passes 53 process tests and 137 worker tests. Both fixtures run the proof and changed call sites.
The proof passes in 0.152 and 0.154 seconds. Replacing the wait call makes both proofs time out in 2.028 and 2.035 seconds.
The Linux record passes 53 process tests and 140 worker tests. Both reverted proofs time out in 2.002 seconds.
Each script captures the test command's status and restores the changed file. Each record reports zero changed files after restoration.
The Mac evidence covers the unchanged wait code and adapters at the new head. Package GS-BW-1 remains closed.

Mac record: `~/botster-sessions/gates/botster-core-stage1-gs-bounded-waits-7e568e20-pool-20261011-075344-20569.log`.
SHA256: `4ea78630143a5e691feee9f46777c385e9364ebf5e99dd06be623e74a031ad76`.
Linux record: `~/botster-sessions/gates/botster-core-stage1-gs-bounded-waits-7e568e20-pool-20261011-075447-24981.log`.
SHA256: `0f81e9b8bdf4b8d45a348908538489609de5bf8f860e8ddd7308360cb039a595`.

I read the earlier full gate's header and final summaries. All ten steps report PASS in 112.6 seconds.
That gate reports no mutant for the diff and 110 allowed process sites. It is not the final gate for this new head.
Earlier gate: `~/botster-sessions/gates/botster-core-stage1-gs-bounded-waits-7e568e20-pool-20261011-075038-10685.log`.
Earlier failed gates stay failed. No pending identifier leaves a list through this review.

The lead orders source CLEAN before the final exact-head gate. This verdict completes source review with zero open findings.
The final exact-head gate and the lead's merge decision remain pending. This verdict does not accept that future gate.
I ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN (0 open)
