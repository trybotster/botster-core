# ST pair reviewer handoff

Role: ST pair reviewer. Branch: `stage1/review-st`.
Lead: `sess-1790903471-008f-8b9f78eef51d48aba5a45748495fd673`.
Implementer: `sess-1791757367-0180-4b575689667bf96848e9325dba401923`.

Design review: `verdicts/st-stall-clock.md`.
Reviewed artifact hash: `2e6d6323dd5743a62962c4e9a9a1148449dcb30f1bf7659e758d56bdd1ba7a01` (revision 3).
Reviewed Core base: `5e953398925351c12e463a7b0a946b79d9841edc`.
Status: design CLEAN. ST-D1, ST-D2 and ST-D3 are closed.
The lead selected Q4 option (b). The stall-clock PR documents restoration but builds no route adoption code.
A surviving route has no stall clock until ST's later DP-8 route adoption PR lands.
Both DESIGN.md files and the stall-clock PR body must retain this adoption limit.
The reviewer read Q1-Q3's answers at contracts commit `43647054722f3133cfd30b67a909b8fc427dcb3e`.
The verdict records the required conditions. Revision 3 retains those conditions correctly.
The lead assigned a separate TM-6 host-driver PR before the stall-clock PR. The pair reviewer and Sol review both PRs.

Current implementation review: Core #295 at `dcc53d8f94d64e94a5b0028c0bd1e86369456773`.
Verdict: CLEAN. ST295-E1 is closed. The reviewer found no new logic defect.
The exact-head Linux gate passed. The focused Mac host/link checks passed 351/351.
Both removed IDs passed on the real tier with seeds 0-31.
The broader Mac run passed 230/232 and exited 100. The verdict retains its known R-66 and R-80 fence failures.
Integration reviewer Sol-B: `sess-1791760169-0182-7a5ca22a6e65a307e8f0b2a8fd1ef2d5`.

#295 merged-head delta: `113da2366286adcef4a71449c5d362c146f6754f`, base `b3cdfffbba353bf9cfc34f7d7f9b8daccc753035`.
No source finding from the diagnostics overlap. The final gate passed all ten steps, exit 0 on msa1.
The package final receipt is CLEAN on the merged head. Sol-B requested the PR body's final evidence update.

#298 first review: `0f9079c44e028644c6d1724c9dae9cd8b03d0459`, base `20b2525c60e6c3e4ee655fe1e0999c1b1b8c48a5`.
Verdict: NOT CLEAN (5 open). Two HIGH findings concern reservation deadlock and failed-resume clock state.
Two MEDIUM findings concern stale parking state and Output report order. One MEDIUM finding requires focused Mac evidence.
The Linux gate is green. The reviewer builds nothing locally.

#298 round 2: `1d044f8bb53cb7ee1e0df302d24bfaef1fd83ce6`. Verdict: NOT CLEAN (1 open).
All five first-round findings are closed. New ST298-R5 is HIGH: the close clock starts before a deferred stall event posts.
The authorized Linux re-gate passed all ten steps. Mac changed packages passed 1046/1046.
The broader Mac real run passed 240/242 and exited 100; the verdict records its R-66 and AM-2 base failures.

#298 round 3: `76b369c7a39bec37a2bdb3c31110c887f60d4edb`, source base `20b2525c60e6c3e4ee655fe1e0999c1b1b8c48a5`.
Verdict: CLEAN. ST298-R5 is closed. No finding remains open.
Core adds the close deadline when the stall event posts. Core removes it when Core consumes the resume report.
The lead approved this rule, and the host DESIGN.md states it.
The exact-head Linux gate passed all ten steps, exit 0. All four new proof tests passed.
The verdict distinguishes ST's copied reversal excerpts from independently checked original logs.
The round-2 Mac evidence covers unchanged worker and operating-system paths; its broader failures remain recorded.
This source CLEAN is not a final-slot merge receipt. Wait for the next doorbell.

DP-8 design review: revision 3, SHA-256 `84c85f149fab45954a2b251cc0c8013bfdd17c455a32f65721e50b4907fc92a6`.
Declared base: #298 head `76b369c7a39bec37a2bdb3c31110c887f60d4edb`.
Verdict: NOT CLEAN (4 open). Findings cover the complete rename, route-keyed observations, bounded messages and adoption event phases.
R1 is closed under R-98 at contracts `4fd81f1cb4063659b709ba8aa72a662e4cc2937b`.
R2 is answered by R-99 at contracts `b11db6b5476f50ba1e0b4d2a962477a0faa99188`.
The next design must distinguish a live route draining output from a route past its Closed transition.
The verdict records the required file and line citation and both tests. The reviewer accepts R3's completion barrier.
The implementer must wait for the lead's design approval before code. Wait for a revised design doorbell.

DP-8 design revision 4: reviewed SHA-256 `f9ec544e6608afdefd0d773e73d254ece86c4ca5f2c779d53643d26898a2e542`.
Verdict: NOT CLEAN (1 open). STDP8-D1 through D4 are closed at design level.
New STDP8-D5 is HIGH: retained client observations can make the host's cached client revision move backwards across routes.
The lead assigned the smaller adopting-host frame bound to a separate ST item after DP-8.
The verdict records the exact artifact, the size-proof condition and the required two-route revision proof.

DP-8 design revision 5: SHA-256 `0860f14a407f812cd38424c9bcb917977946f407c932078e9af69ab46142e7b3`.
Verdict: CLEAN. STDP8-D5 is closed; no design finding remains open.
The worker sends its current client revision for retained and fresh observations, preserving nondecreasing host revisions.
The implementation's size test must cover absent and present tags with the actual pinned serde types.
This design CLEAN does not replace the lead's approval before code. Wait for the next doorbell.
Follow-up artifact: `5bd281fd4c3a2c71265e60b63f764043e53251bfbf208e10eb00dda048e90825`.
The reviewer inspected the absent-tag encoding update. The design verdict remains CLEAN.

#298 merge delta: `4ca7bb2a4b6f5cdfb214daffd79e67af0e1cbec6`, importing v1 `ae49bbec2d1086f918341bc596bf8560ba27bcd1`.
The reviewer found no source defect in the announced conflict resolutions and added drain-test mark.
All six three-way conformance sets passed. The deferred file has two ids, not the four stated in the READY message.
The local package run passed 1086/1086. The strict default-tier run retains its one in_9 timeout.
The final receipt waits for the completed exact-head pool gate. Do not poll; wait for its doorbell.

#298 completed receipt: CLEAN at `4ca7bb2a4b6f5cdfb214daffd79e67af0e1cbec6`, base `ae49bbec2d1086f918341bc596bf8560ba27bcd1`.
The completed Linux gate passed all ten steps, exit 0 after 963 seconds on msa1.
The verdict retains the local strict timeout, earlier Mac evidence limits and copied reversal provenance limits.
The PR body still needs this head, base and completed gate before DONE. The lead controls the next merge slot.
Body follow-up: the reviewer verified the announced update at unchanged head `4ca7bb2a`.
The body update requirement is complete. The CLEAN receipt remains unchanged.

The reviewer builds nothing locally. The reviewer waits for a doorbell and then receives messages once.
The reviewer reviews revised design artifacts and exact PR heads when the pair sends them.
Every later implementation review checks evidence for `BOTSTER_SEEDS=0-31` on both tiers.
The reviewer checks the required Linux gate and focused Mac evidence for code that touches processes, PTYs or sockets.
