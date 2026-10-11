# Integration review: PR #274 — route_accept under route_gate

## Round 1 — 2026-10-11

Head: `0201489c095733651e1241355af23ba77774066d`.
Parent evidence head: `734a76e32cf5acdb7c95fe0b0fbac53fbe4b900c`.
Actual source base: `e4b6cdf54a7453af182b9e3e44fc08b5f58c7b47`.
Tree: `6b2f8d604b43a6a54497a350b09ace8bb0d31387`.
Gate base reference: `d582853da895688710ab97f8f1a0858599e8ba18`.
Package verdict: `695bfad8ed845340ede2ee9d126565056d0ca1f4`, Round 1, NOT CLEAN.
Owner READY: `msg_plugin-w_1791734534_2ef260`.
Package receipt: `msg_plugin-w_1791734809_bdf3a7`.

I read the complete package verdict, PR body, and two-file own delta.
I verify the source merge base independently. The READY's d582853d reference names the gate base, not the source merge base.
The head does not import #269. The lead's one-merge rule governs the later import after both source CLEANs.
No later import or final gate is accepted by this round.

The change uses the injected stream edge. It adds no production test branch or OS operation.
SideControl::limit supplies the same byte allowance to write, readiness, and room.
Queue capacity still limits the allowance, and a write consumes only its accepted bytes.
The failure, reset, close, and empty-write paths retain their order.
Gate release clears the allowance. A new gate after release takes no bytes until another allowance arrives.
I read the route client control contexts and the pinned route_gate, route_accept, and route_fill rows at Contracts .36.
The body records the lead's ruling that route_accept supplies a partial opening under route_gate.
The new test observes public room, readiness, write counts, and WouldBlock results.
The changed route_fill test retains its earlier assertions and checks an allowance under a gate.
The own delta changes no pin, dependency, pending list, minimum list, production crate, or mutation exclusion.

### AG-SI-1 — LOW — the route_fill summary omits the accept allowance

Location: `crates/botster-core-testkit/src/route_client.rs:140`.
The route_fill summary still states that C is zero while gated.
The implementation obtains C from worker_end().room(). The changed room rule permits the remaining accept allowance under the gate.
The changed route_fill test requires room four under that condition.
Change the summary to state zero while gated with no accept limit, consistent with net.rs and the changed test.
This finding requires no behavior change.
I sent this finding to the implementer and package reviewer in receipts `msg_plugin-w_1791734894_a24554` and `msg_plugin-w_1791734894_711aa0`.

### AG-R1-1 — MEDIUM — package evidence condition remains open

I retain the package reviewer's finding as an acceptance condition. I add no duplicate source finding.
The first evidence script invokes cargo test directly and sets no BOTSTER_SEEDS value.
The body and table claim the full CI seed set. The pinned seed source defaults to 0–7 when that value is absent.
The required CI set is 0–31. The package reviewer requests an explicit focused comparison with all selection overrides cleared.
I read the supplied scripts and current evidence test. The second script sets 0–31 and the test asserts the effective seeds.
I have not received or accepted the second job's completed log, corrected table, or corrected body.
I make no independent claim about all 600 first-job outcome texts or its reversal result.
The package verdict retains those first-job facts within its stated seed limit.

### Exact-head gate

Log: `~/botster-sessions/gates/botster-core-stage1-p6-accept-under-gate-0201489c-pool-20261011-085505-41999.log`.
SHA256: `099ad905f15640d05601de84bbb8197000d370205acc4bcf84e7d5d3a58a3f00`.
I read the head and base headers, stage summaries, both changed test outcomes, and all 22 raw mutant identities.
I match all 21 catch records to the raw caught identities.
All ten stages pass in 387.9 seconds. Default passes 1701 tests; slow passes 548 tests.
Stage 1 catches 21 mutants and reports one unviable mutant, with zero misses and timeouts. Stage 2 does not run.
The unviable identity replaces End::readiness with Default::default(). It supplies no catch evidence.
The job and wrapper exit zero after 399 seconds, with two seconds queued and 397 seconds running.
The real minimum remains 46 / 68 passing and 47 / 69 accepted. This review authorizes no pending removal.
The green gate does not close the full-CI before/after evidence condition.

I ran no build, test, gate, mutation, reversal, measurement, or executable base check.

VERDICT: NOT CLEAN (2 open: AG-SI-1 and package AG-R1-1) at 0201489c095733651e1241355af23ba77774066d
