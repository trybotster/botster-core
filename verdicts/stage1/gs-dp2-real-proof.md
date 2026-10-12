# PR #304 integration review

## Round 1

Exact head: `37e5ee32d326f588676b6fc727ad52c86022de61`. Source and gate base: `fcc09461cb0c0de4ef067c160f24d205f0f65c30`. Branch: `stage1/gs-dp2-real-proof`. Risk: HIGH. Lead assignment: `msg_plugin-w_1791778065_5a7671`.

Sol-B read the approved option-A design and section 2a's clause map before source review. Sol-B read all four changed files, affected proof and helper context, the complete PR body, and retained evidence.

The change matches the lead's approved scope. Three proof lines name existing slow tests for `conf::dp_2_stream_handoff_transfers_ownership_and_closes_host_copy`. The change removes only that ID from core-pending. It retains the real-only class, real-pending file, minimum list, contracts pin, and gate decisions. Two doc comments add the exact ID. One test adds a read-to-EOF assertion.

The facade proof receives one descriptor with the expected route identity through the real control socket. The worker side reads every client byte to EOF. The client then reads the worker's exact first line. The new assertion requires no remaining byte after stop.

Sol-B read first_line, Bounded::line, and Bounded::to_eof. The same Bounded reader retains bytes read past the newline. The new assertion therefore checks both buffered tail bytes and later socket bytes. The existing cleanup deadline bounds the EOF read. A read error or deadline fails the assertion.

The unchanged stop helper joins the worker-side thread and ends Core before the EOF read. The new assertion changes no process ownership or reap behavior.

The R-65 proof closes the worker's received copy before acknowledgement. A client write succeeds while the host still holds its copy. A later capture's Completed event establishes that the host processed the preceding RouteReceived. The client then observes the close and BrokenPipe. Linux and macOS use their existing close assertions.

The worker-bin proof uses the real worker binary and a handed-off descriptor. The route carries attached, baseline, live, client input, and payload output. These existing tests compose the approved DP-2 proof across the package boundary.

The composition has a stated limit: no named test combines real Core and the real worker binary in one test. The lead approved option A with this limit. The transcript's own real-tier proof remains follow-up work after its fence pin. This verdict approves no transcript fence bypass.

Each named proof cites its ID and clause. The worker-bin form has the retained build-root test. The facade binary's build-root coverage gap remains in the separate coverage item. This PR does not close that item.

### Evidence

Linux gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-gs-dp2-real-proof-37e5ee32-pool-20261011-205649-66612.log`. The header names the exact head and base. The lists step names the same merge base. All ten steps PASS in 366.6 seconds. Default: 1,911 passed. Slow: 613 passed. The three exact proof tests PASS, and real-proofs reports each proof in the slow step. The suite credits dp_2 by those named proofs. The diff lists no mutant. No FLAKY result appears. The job and wrapper exit 0 after 454 seconds, with one second queued.

Mac job: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-gs-dp2-real-proof-37e5ee32-pool-20261011-210444-97216.log`. Sol-B read the complete command and output. The job prebuilds the candidate, then runs the two facade proofs and the worker-bin proof. All three PASS on Darwin 27.0.0 in slot-2. Both test commands exit 0. The job and wrapper exit 0 after 106 seconds. The short TMPDIR changes no product rule.

Reversal: `/Users/jasonconigliari/botster-sessions/shared/core-stage1/evidence/gs-dp2-proof/reversal-window-write.log`. The scratch change writes `!` through the host's copy when handling RouteReceived, before that copy drops. The original output shows the new tail assertion fails with `Ok([33])` instead of `Ok([])`. The first-line assertion had passed. The R-65 close assertion also fails. After restoration, both facade tests PASS. The scratch change is not in the reviewed source.

The reversal has a provenance limit. No original clean record exists. GS added an after-run note naming `env -u RUSTUP_TOOLCHAIN cargo clean --target-dir $HOME/botster-sessions/targets/gs`. GS reports `Removed 13716 files, 3.6GiB total` before any build in this worktree. GS reports no intervening build from another worktree.

Sol-B retains those cleanup facts as GS's owner statement. Sol-B does not claim independent cleanup or artifact-provenance verification. The original reversal output establishes that the new tail assertion executed. The helper preserves tail bytes. Independent pool jobs validate the positive proof paths. The approved reversal requirement does not require a contemporaneous clean transcript. Sol-B accepts the supplied reversal with this explicit limit and finds no contrary evidence.

### Package receipt and scope

Package CLEAN: `3f29eabc7a49f86cbaf4dd04135935d56fc909ab`, `verdicts/gs-gate-speed.md`, on this exact head and base. Sol-B read the complete #304 section. It agrees on the proof mapping, EOF helper, completed evidence, and provenance limit. No package or integration finding remains.

This CLEAN covers this exact source head and its supplied initial evidence. The final-slot flow, applicable current-base review, exact-head final gate, and final body remain due. This verdict accepts no later head, contracts pin, or status removal.

Sol-B ran no local build, test, gate, mutation job, reversal, diagnostic, or executable base check.

VERDICT: CLEAN (0 open)

## Train 1 — source delta

Exact train head: `be95ef13edc6d9eccf87a8c0b2c09b9cf820963a`. Tree: `456fdd44172ee85eb58ad443ec1e3dbf9f1dd2bc`. Current v1 base: `ae49bbec2d1086f918341bc596bf8560ba27bcd1`. Frozen #304 member: `37e5ee32d326f588676b6fc727ad52c86022de61`. Lead assignment: `msg_plugin-w_1791781317_0b4065`.

Sol-B inspected the train's merge ancestry and #304's complete delta. The train first merges #301 at `403bbf689cc9f745a9552cc518374a6a8f2cdb32` into current v1, then merges #304. The #304 own patch remains equal after excluding blob-index and hunk-location metadata. Its proof map, facade proof source, and worker proof source are byte-identical to the reviewed member.

Sol-B independently checked the pending ID sets. The train set equals current v1 minus `conf::a2_1_attach_state_rules` from #301 and `conf::dp_2_stream_handoff_transfers_ownership_and_closes_host_copy` from #304. The base's earlier tm_6 removal remains. The #301 comments retain their IDs. No unrelated ID disappears. The held and real-pending files equal current v1. The three DP-2 proof entries retain their reviewed mapping and order.

Package delta CLEAN: `7b98dfd7e7b5009641af8fee4edba3ebfbea1af4`, `verdicts/gs-gate-speed.md`, on this exact train head. Sol-B read the complete #304 delta section. No package or integration delta finding remains. The original composed-proof and local-reversal provenance limits remain.

The separate combined train review is in `verdicts/stage1/train-1.md`. This delta CLEAN accepts no train gate result. The final gate must run on the frozen train head after all required source reviews. Final evidence receipts and the base-tree verification remain due. Sol-B ran no execution job.

VERDICT: CLEAN (0 open; source delta only, train evidence pending)

## Train 1 — final evidence acceptance

Exact train head remains `be95ef13edc6d9eccf87a8c0b2c09b9cf820963a`. Tree remains `456fdd44172ee85eb58ad443ec1e3dbf9f1dd2bc`. Gate base: `ae49bbec2d1086f918341bc596bf8560ba27bcd1`.

Completed log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-train-1-be95ef13-pool-20261011-221548-42911.log`. SHA256: `1e84c3eb42a01cf5bf59dd1cbf88f440a3d6788fd40f11ae8eae42a438842705`. Sol-B read the completed identity, lists, tiers, proof records, mutation records, and final exits.

All ten steps PASS in 520.5 seconds. Default: 1,920 passed. Slow: 617 passed. Each exact DP-2 test passes in the slow tier, and real-proofs credits all three. The lists step names the same base and merge base and reports 407 pending IDs. The accepted two-member removals remain.

The train tests 29 mutants from #301: 21 caught, 8 unviable, 0 missed, 0 timeout. Sol-B independently matched all raw catches to their named citations. Every catcher has a normal PASS. Stage 2 does not run because stage 1 misses none. No FLAKY result appears. These results claim no new mutation proof for #304's test-only change.

The job and wrapper exit 0 after 850 seconds on MS-A1, with 232 seconds queued and 618 running. The combined source review remains valid on this unchanged train tree.

Package final CLEAN: `7163379f0a518d142425d4ae60fe8e9e2c617bd3`, `verdicts/gs-gate-speed.md`. Sol-B read the complete #304 final section. Sol-B also read the complete updated #304 body. Its train section names the frozen member, exact train head/tree/base, source receipts, completed log, and gate results. Original proof and provenance limits remain.

This completes #304's final integration evidence receipt on the frozen train head. The combined acceptance is recorded in train-1.md. The lead must update the base under the train rule, verify the gated tree, and verify the push. This receipt claims no completed merge. Sol-B ran no execution job.

VERDICT: CLEAN (0 open; #304 final train evidence accepted)
