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
