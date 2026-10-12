# P6 review: group guard registration order — PR #296

## Round 1 — 2026-10-11

Exact head: `1a9ba675cd6f51afd88e657649d22ec525d32f44`.
Source and gate base: `f6025aed4cdf7da836e835e0040fc128b7e8f32f`.
Tree: `7438de354c26c381b9a75b0c1a0baca02b5e044e`.
Risk: HIGH. The shared guard is compiled into sys, Core, and worker tests. The PR changes a check allowlist.

I read the complete five-file delta, the changed guard and cleanup functions, both platform adapters, and affected guard users.
I found no source defect in this delta.

The sys spawn helper returns only after the shell writes its registration acknowledgement to a FIFO.
The shell writes that line after the anchor joins the group and acknowledges registration.
The group-kill test therefore exercises the registered group and retains its public identity and signal-9 exit assertions.
The signal-handler test retains its public exit-code-7 assertions. No existing contract assertion is removed or weakened.
The FIFO helper uses OwnedChild for mkfifo completion. The existing FIFO reader and ten-second deadline are reused.

The registration thread returns the registered group ID. The guard still reaps only its own anchor.
Production still reaps its worker. The anchor's normal cleanup reaps only its own reservation child.
An anchor exit by SIGKILL with no report selects the bounded, observation-only group wait.
That path sends no signal after the guard loses its group reservation. Other cleanup failures still fail the test.
The moved wait helper retains the cleanup policy and ten-second limit. It uses the existing platform clock wrapper.
The platform waits observe exits through pidfd or kqueue. They do not reap the production worker.
The cleanup rounds require an exit wait or a changed live set. The delta adds no sleep or busy loop.
The removed command-wait allowance names the call replaced by OwnedChild.

Core ScriptWorker, worker CLI, worker observer sessions, and the payload-reap observer retain registration before their worker bodies.
The inspected readiness records and public completion records follow that registration prefix.
The change does not add test access to private production state.

I read the two named Prior-art sources at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
Those process tests exercise production group termination without a separate test anchor.
The note rejects that ownership design and reuses the current anchor design required by BUILD.md Hygiene rule 10.
This review does not claim an exhaustive search of the old repository.

### Retained evidence

Linux log: `~/botster-sessions/gates/botster-core-stage1-p6-group-guard-order-1a9ba675-pool-20261011-164959-15401.log`.
SHA256: `1ab252c4d03fe76e960e6f3faabe04efa429e64f263877f7f960cb57c7e05117`.
I read the head/base header, selected affected-test outcomes, tier summaries, stage summaries, and final exits.
All ten stages pass in 159.1 seconds. The job exits zero after 167 seconds; the wrapper exits zero after 168 seconds.
Default passes 1796 tests, with 460 skipped. Slow passes 597 tests, with 2025 skipped.
The changed group-kill test, signal-handler test, payload-reap observer, and both CLI behavior tests pass.
The shared guard's failed-cleanup, panic-before-ready, and early-exit proofs pass in the affected targets.
The complete log contains no FLAKY record. The diff has no mutant, so the mutation stage starts no run.
That result supplies no mutation or reversal proof for this fix.

I read the complete PR body. Its raw SHA256 is `4f8f55e98de83f635f5f51b9435f404b5d560c2059f0489ae45a0fcf17df712c`.
I read the retained flake handoff and integration Round 1 at `6254f9cd3094ae9aacc02bd1f3aef24d6612aaad`.
The original failed attempt reports the guard-drop SIGKILL failure. The body identifies the missing registration order and repairs it.
The independent testkit mutation does not change the sys binary. No reproduction campaign is required.

### GG-P1 — MEDIUM — OPEN: Mac and reversal evidence is not retained

The body reports Mac passes and a drop-rule reversal on `eedbf877e63c2e239370dcceea1c478494af432a`.
It supplies no retained log path for either result. Those reports cannot establish the exact-head Mac result or the reversal failure.
The exact head also includes later clock-wrapper and FIFO-helper changes.

Supply the retained Mac and reversal logs. Supply focused Mac evidence on the exact head for changed guard paths and affected users.
Plan section 8, revision 23ai, applies because this cleanup path reads OS group membership and waits for process exits.
This finding matches integration GG-I1. An evidence-only reply can close it; a source change requires delta review.
The final-slot import, applicable base review, final exact-head gate, and final body remain due after source acceptance.

I sent this finding directly to P6. I changed no product code and ran no execution job.
This verdict accepts no later head, contracts pin, or status removal.

VERDICT: NOT CLEAN

## Round 2 — 2026-10-11: retained Mac and reversal evidence

Exact head: `1a9ba675cd6f51afd88e657649d22ec525d32f44`.
Tree: `7438de354c26c381b9a75b0c1a0baca02b5e044e`.
The head and tree are unchanged. Round 1's complete source review remains valid.
The Linux gate retains its recorded head and base scope.

Mac log: `~/botster-sessions/gates/botster-core-stage1-p6-group-guard-order-1a9ba675-pool-20261011-165612-58346.log`.
SHA256: `714a92e5737a220da34165b4c375017b42379b0ac51b4c042788bc982d68928e`.
I read the retained log, including the decoded command, exact-head output, all selected test results, reversal failure, restoration, and exits.
The job reports Darwin 27.0.0 and the exact head above.
Its base environment names `3aacc58d086e55a6b186165ccf9001e2618e2869`; this does not establish a source import of that base.
The command prebuilds the worker and runs six affected test targets with the slow feature.

All six targets pass. Each package command exits zero:

- sys slow_process: 23 passed. slow_payload: 28 passed.
- Core slow_facade_worker: 19 passed. slow_real_core: 27 passed.
- worker slow_cli: 17 passed. slow_session: 41 passed.

The results cover the changed group-kill test, signal-handler test, payload-reap observer, ScriptWorker users, CLI users, and worker observer sessions.
The shared failed-cleanup, panic-before-ready, and early-exit proofs also pass in these targets.
The retained log contains no FLAKY record.
The job exits zero after 754 seconds: 441 queued and 313 running. The wrapper exits zero after 754 seconds.
This focused job supplies Mac evidence. It is not a full gate.

The reversal changes only the new drop condition to `if false && killed`, with FIFO registration retained.
The diff statistic records one insertion and one deletion in process_guard.rs.
The selected group-kill test fails at process_guard.rs:130 with the original SIGKILL cleanup message.
It reports zero passed, one failed, and 22 filtered tests. The cargo command exits 101.
The script restores process_guard.rs. Its final status shows only the pool marker `.pool-used`.
The observed test failure and exit 101 establish the negative result. The wrapper's exit zero alone does not establish it.
This proof disables the new drop rule after registration. It does not claim that disabling registration alone causes the same failure.

I read the complete live body delta after GitHub MCP truncates the body.
It replaces the unretained eedbf877 reports with this exact-head Mac job and its reversal record.
The body matches the retained results. Raw body SHA256: `b62a514810e2aae41dffbbc376b877e332b8c246162c44f0f51e7da6539655cc`.
GitHub still names the exact head above. No source delta follows Round 1.
Integration receipt `msg_plugin-w_1791763853_ac4fd0` independently confirms these evidence records.

GG-P1 is CLOSED. No package source or evidence finding remains for this reviewed head.
The source and initial Linux gate are accepted with the required focused Mac and reversal evidence.
The final-slot import, applicable base review, final exact-head gate, and final body remain due.
This verdict accepts no later head, contracts pin, or status removal.
I changed no product code and ran no build, test, gate, mutation, reversal, diagnostic, or executable base-check job.

VERDICT: CLEAN

## Round 3 — 2026-10-11: source review after the base import

Exact head: `07f5db488e71f5da46d845ee9663a5a7e6863ec5`.
Imported and check base: `f7eeced64e652700f2f78fc063e4896832134233`.
Tree: `472925ad9b1684553cc87ef0a0e65b4792010e95`.
Merge parents: `1a9ba675cd6f51afd88e657649d22ec525d32f44` and the exact imported base above.

Canonical check log: `~/botster-sessions/gates/botster-core-stage1-p6-group-guard-order-07f5db48-pool-20261011-175929-22640.log`.
SHA256: `9a849c7ecdde43f4ad6117066d6ec105fd1d70fd023efd00b4b94d7ff8fa8b8c`.
I read the complete retained check records and decoded command. They name the exact reviewed and imported heads.
Both ancestry conditions pass. All four check conditions pass:

- The merge-tree has no conflict.
- The base changes 20 paths. None overlaps the PR's five paths.
- The own diff remains byte-identical: 13525 bytes, SHA256 `7d706be6d1560e1c0760501d74bd022772a0fda93df267df622922aa9c8a3012`.
- Both pending files retain the exact imported-base entries.

The check returns PASS and exits zero. The job exits zero after 110 seconds: one queued and 109 running.
The wrapper exits zero after 111 seconds.
The canonical PASS permits the previous source review to carry without another delta round.

I independently inspect the merge parents and Git tree entries.
All five owned entries equal the reviewed head's entries. Every other entry equals the imported base's entry.
Both core-pending.txt and core-real-pending.txt equal the imported base exactly. No unreviewed own source delta remains.
The package source review therefore carries to this exact head. No package source finding remains.

The initial gate, focused Mac evidence, and reversal retain their recorded original-head scope.
This round accepts no unfinished final gate result. The completed exact-head final gate and final body remain due.
This verdict accepts no later head, contracts pin, or status removal.
I changed no product code and ran no execution job, including no executable base check.

VERDICT: CLEAN

## Round 4 — 2026-10-11: final gate, body, and merge

Exact head: `07f5db488e71f5da46d845ee9663a5a7e6863ec5`.
Imported and gate base: `f7eeced64e652700f2f78fc063e4896832134233`.
Tree: `472925ad9b1684553cc87ef0a0e65b4792010e95`.
The unchanged head retains Round 3's verified source carryover and canonical PASS.

Final log: `~/botster-sessions/gates/botster-core-stage1-p6-group-guard-order-07f5db48-pool-20261011-180130-28242.log`.
SHA256: `85648fdc79335bcd4c5750ecd40295a52df5d03b49f0662726298fb959e5a15f`.
I read the exact-head/base header, lists records, selected affected-test outcomes, tier/stage summaries, and final exits.
All ten stages pass in 195.3 seconds. The job exits zero after 249 seconds: 31 queued and 218 running.
The wrapper exits zero after 249 seconds.
Default passes 1843 tests, with 435 skipped, in 13.857 seconds.
Slow passes 606 tests, with 2040 skipped, in 42.373 seconds.

The changed group-kill test and signal-handler test pass. The payload-reap observer passes.
The guard's failed-cleanup, panic-before-ready, and early-exit proofs pass in the affected sys, Core, and worker targets.
The complete log contains no FLAKY record or failed nextest result.
The mutation stage lists no mutant of this test-only diff and starts no run. It supplies no additional reversal proof.
Fuzz reports no changed crate with a decoder harness.
The lists command names the exact imported base as both run base and merge base. It passes with 44 real-pending identifiers.
The inherited conformance summaries pass. The held-full command reports two budget identifiers at 32 seeds, one trial per seed.
This PR changes no pending file or mutation exclusion. These inherited outcomes authorize no additional status removal.

I read the complete live final-body delta after GitHub MCP truncates the body.
It records the exact import, canonical PASS, final gate, and package/integration source receipts.
Its records agree with the completed evidence. Raw body SHA256: `aac2aebae733ff0444075f63b0ff85026836e4311e5a6431d69a6c312c5fdde0`.
The focused Mac and reversal evidence retain their recorded original-head scope through the verified source carryover.
I also read Sol-B's complete record at `7170bb52858a9d2557a2cf0aa502c10a90092f8b`.
That record closes GG-I1 and permits source carryover. Its final-gate claim remains pending; it does not supply this round's gate evidence.

GitHub reports #296 merged at 2026-10-12 01:06:28Z, merge `1b984f06f11378e1a424db75dfe99b1e2d15c567`.
I independently inspect the merge object. Its parents are the exact gate base and reviewed head above.
Its tree exactly equals the reviewed and gated tree. The merge adds no source delta.

No package finding or required acceptance record remains for this exact head.
The source, canonical check, required Mac and reversal evidence, final gate, final body, and exact-tree merge are reviewed.
This verdict accepts no later head, contracts pin, or status removal.
I changed no product code and ran no build, test, gate, mutation, reversal, diagnostic, or executable base-check job.

VERDICT: CLEAN
