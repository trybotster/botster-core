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

## Round 2 — documentation and CI seed evidence correction, 2026-10-11

Head: `7e3457fc53e8c78db41fa385c697448fdc410d8e`.
Parent: `0201489c095733651e1241355af23ba77774066d`.
Source base: `e4b6cdf54a7453af182b9e3e44fc08b5f58c7b47`.
Package CLEAN: `4d362baa78f6e6c3e7be00bdeb89f4477a2d4dc0`, Round 3.
Owner receipt: `msg_plugin-w_1791735165_a2ac37`.
Package receipt: `msg_plugin-w_1791735276_0f91cc`.

I read the complete two-file correction and exact-head package verdict.
The route_fill summary and matching harness test summary now state zero while gated with no accept limit.
Only comments change. Runtime code and test assertions remain unchanged. AG-SI-1 closes.
I read every changed body paragraph. The body corrects the earlier seed claim and distinguishes source base from gate reference.

New evidence log: `~/botster-sessions/gates/botster-core-stage1-p6-accept-under-gate-0201489c-pool-20261011-090750-91245.log`.
SHA256: `44827fce457c592e41ea137c4a67957056538523e0322aa6fd46d58066afadff`.
This focused evidence job runs at the parent head. Its header uses current v1 a0adb132 as the gate reference.
The actual comparison checks out e4b6cdf5 before and 0201489c after. It prints the after tree and zero tracked changes.
I decode the retained command without executing it. Its script matches job2.sh after replacement of the embedded test with TEST_B64.
Its embedded test exactly matches the supplied a54_testkit.rs.
The script sets BOTSTER_SEEDS=0-31 and clears BOTSTER_SEED, BOTSTER_ONLY, and BOTSTER_CLAUSE.
Both executions print all effective seeds 0–31. The test asserts exact equality to that set.
Both trees use Ghostty 39a68e822e505685d1e7fa9c6abeff21125b489c.

A static parser compares all complete outcome texts from the log, without shortening their failure descriptions.
Each tree has 600 unique identifiers and no duplicate record. Their identifier sets and every outcome text match.
Each tree reports 210 PASS, 265 UNSUPPORTED_CONTROL, 123 FAIL, and two NOT_APPLICABLE outcomes.
The named ou_12 and ou_3 transcripts pass in both trees.
The before evidence test passes in 49.26 seconds. The after test passes in 48.72 seconds.
The job exits zero after 174 seconds, with one second queued and 173 seconds running. The wrapper exits zero after 175 seconds.
AG-R1-1 closes. The package reviewer also verifies the corrected table against these complete records.
I do not claim an independent complete read of all table cells.

The first full gate remains parent-head evidence. The focused comparison does not replace the required final full gate.
This CLEAN accepts source and correction evidence only. It accepts no future import, gate, or pending removal.
The lead's one-merge rule governs the final import after both source CLEANs.
I ran no execution job.

VERDICT: CLEAN (0 open) at 7e3457fc53e8c78db41fa385c697448fdc410d8e

## Round 3 — final import, gate, and merge, 2026-10-11

Head: `888a9d32880d92e49f92501671cbb987df972318`.
Base: `a0adb1321bfc3deb7d16735e9a622775ba5f1a2d`.
Tree: `0bcf6a379cb06106e1cdad282c97332588feab45`.
Package source carry: `827251b72072885b3ba52fab27d1ea1ba2a09416`, Round 4.
Owner receipt: `msg_plugin-w_1791736842_00e677`.
Package receipt: `msg_plugin-w_1791736848_48d2a6`.

Sol read the complete base-check log and the complete package Round 4.
The import has exactly two parents: the accepted source head 7e3457fc and the base above.
Both ancestry checks and all four base-check conditions pass.
The check exits zero after 20 seconds, with one second queued and 19 seconds running.
Log suffix: `888a9d32-pool-20261011-091605-25132.log`.
SHA256: `748e8e5db3f424d09ed74cb13f4994b2a6ae63197015759cf36e1a41adee257e`.

Sol independently compared both complete own diffs from Git objects. They are byte-identical, with 9986 bytes.
SHA256: `eca7e04f12bfd16693aff655b2b5ecc60c4ac6749e20773bde5d5d340df9651d`.
None of the 26 imported paths overlaps the three own paths. Every imported blob equals the base blob.
Both pending files equal the base. The static Git merge tree equals the head tree above.
The first static merge-tree call failed because the sandbox prevented a temporary file. The authorized retry succeeded.
No executable base check ran. These results carry the accepted source verdict to this exact head under Q7.

### Final exact-head gate

Log: `~/botster-sessions/gates/botster-core-stage1-p6-accept-under-gate-888a9d32-pool-20261011-091642-28196.log`.
SHA256: `b543226ec615ea432c7ee2ad8e4f8c1b9cd04614321ebafefc148067e72ee9d3`.
The Linux gate names this exact head and base. Sol read the headers, summaries, changed test outcomes, and all mutant and catcher records.
All ten steps pass in 241.0 seconds. Default passes 1725 tests, with 481 skipped. Slow passes 553 tests, with 2022 skipped.
The new gated-accept test and both route_fill tests pass. The named ad_6 real proof passes in this gate's slow tier.
The real minimum reports 46 / 68 passing and 47 / 69 accepted.
The gate reports four passing real-pending identifiers. The PR removes none of them.
The earlier before/after proof retains its original source trees and full 0–31 seed scope. This import does not enlarge that scope.

The mutation baseline passes. All 21 raw caught identities exactly match 21 unique named catcher records.
One mutant is unviable: End::readiness replaced with Default::default(). It supplies no catch evidence.
There are zero misses and zero timeouts. Stage 2 does not run because stage 1 misses no mutant.
The fuzz step reports no changed crate with a decoder harness.
Job exit zero follows 252 seconds, with zero queued and 252 running. Wrapper exit zero follows 253 seconds.

### Body and merge

Sol read the complete final GitHub body through the authorized CLI fallback.
GitHub reports this exact head, base branch v1, and state MERGED.
The body records the import, check, final gate, corrected seed proof, and historical failed formatting gate.
Package Round 4 accepts source carry only. The package reviewer has not yet supplied a final gate/body completion record.
Sol independently accepts the final gate and body described above.

Merge: `42d7056e99ed0aaf2967c5ecc7609cb3c65e970f`.
Git objects show exact parents a0adb132 and 888a9d32. The merge tree equals the accepted head tree.
Sol verifies this merge independently. AG-SI-1 and AG-R1-1 remain closed. Zero findings remain.
The active Contracts pin remains .36. No later pin, pending removal, source import, or gate is accepted.
Historical gate omissions and the separate ou_9 repair propagation retain their existing status.
Sol changed no product source and ran no execution job.

VERDICT: CLEAN (0 open) at 888a9d32880d92e49f92501671cbb987df972318
