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

#313 DP-8 implementation round 1: `d1f414362a6be86eb25d84a168782ebeb7380b23`.
PR base: #298 `4ca7bb2a4b6f5cdfb214daffd79e67af0e1cbec6`; imported v1 and gate base: `be95ef13edc6d9eccf87a8c0b2c09b9cf820963a`.
Verdict: NOT CLEAN (1 open). ST313-R1 is HIGH: the session-end flow can close an adopted route before its adoption event posts.
ST313-Q1 asks the lead for the absent-tag contract disposition. The reviewer does not invent that value.
The exact-head Linux gate passed all ten steps. Focused Mac packages passed 1114 tests and selected real ids passed 19/19.
All three removed DP-8 ids pass at seeds 0-31 on both tiers. Two epoch-fence ids remain pending under the lead's follow-up.
The verdict records the retained reversal logs and their provenance limits. The reviewer builds nothing locally.
Wait for the next revised-head doorbell.

Lead notice: the lead cannot read Botster messages because of a permission denial on the lead's side.
The lead can send messages and read verdict files. Important messages must be sent again after the lead restores the inbox.
The #313 finding and absent-tag question are already in `verdicts/st-stall-clock.md`, commit `28bd15cf`.
Continue assigned review work when artifacts arrive. Do not assume an answer to the absent-tag question.

#313 round 2: `2b08608d27b2ac15daf7c0cc1e0283c663f5518b`. Verdict: NOT CLEAN (1 open).
ST313-R1 is closed by the scheduler gates and the engine and production-driver proofs.
New ST313-R2 is HIGH: the broad session gate blocks a newly requested Stop until RouteAdopted posts.
The gate must preserve eventless stop effects under mandatory queue pressure (EV-5(c)).
The exact-head Linux gate passed all ten steps. The local selected packages and strict conformance also pass.
The absent-tag question remains pending. Send this receipt again after the lead restores inbox access.
This is the second NOT CLEAN implementation round for #313. Wait for the next revised-head doorbell.

#313 round 3 source review: `5015b0120d7e4c9ad205e8fadf34fb5e6b97070a`.
ST313-R1 and R2 are closed. No source finding remains open.
The production-driver Stop and StopAll proofs pass, and the session-end, removal and local-detach order remains correct.
The exact-head Linux gate passes all ten steps, exit 0 after 864 seconds on msa1.
Verdict: PENDING CONTRACT DISPOSITION for ST313-Q1 only. The reviewer does not assume that absent tags map to empty strings.
No further source change or execution is requested by this review. Wait for the ruling doorbell.
Send the important receipt again when the lead restores inbox access.

The lead announced restored inbox access. Earlier messages expired unread.
The reviewer sent ST313-Q1 again with head `5015b012` and the source findings' closed disposition at receipt `57f14c14`.
The final verdict still waits for the absent-tag contract answer. Wait for its doorbell.

R-104 resolves ST313-Q1 at contracts `2fa8ce2da4d05533d41ef4daef2c2a14a804221b`.
The reviewer verified the required source comment and DESIGN.md citation at `5c24322b4f49cb13d468ee3b2e17da02f6e9c101`.
The delta from `5015b012` changes comments and documentation only. All findings and questions are closed.
The final receipt waits for the completed exact-head gate. Do not poll; wait for the completion doorbell.

The reviewer builds nothing locally. The reviewer waits for a doorbell and then receives messages once.
The reviewer reviews revised design artifacts and exact PR heads when the pair sends them.
Every later implementation review checks evidence for `BOTSTER_SEEDS=0-31` on both tiers.
The reviewer checks the required Linux gate and focused Mac evidence for code that touches processes, PTYs or sockets.
