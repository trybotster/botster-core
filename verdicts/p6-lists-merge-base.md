# P6 review: #294 lists merge-base fix

## Round 1 — 2026-10-11

Exact head: `11b6e2ac8a94dacad7b17876fc1b61e65ae233b5`.
Source base and parent: `5e953398925351c12e463a7b0a946b79d9841edc`.
Tree: `2a3e600a7a1829f71717990d52ff5694933c7603`.
Gate and live PR base: `f6025aed4cdf7da836e835e0040fc128b7e8f32f`.
Git independently confirms that the exact head's merge base with that gate base is the source base above.
Lead receipt `msg_plugin-w_1791761742_6ada8e` confirms package reviewer 016a and integration reviewer Sol-B, session 0182.

I read the complete two-file commit, the affected fsutil and lists command contexts, the new tests, the approved design note, and complete body.
The change implements the approved note: all five comparison files come from one merge-base commit.
Those files are core-pending.txt, core-deferred.toml, core-ledger-ids.txt, Cargo.toml, and core-real-pending.txt.
The current-tree inputs and the pure check, check_real, and pin_moved decisions remain unchanged.
The pinned-contract copy checks, minimum checks, named proof checks, and other checks with no base input remain unchanged.
The output reports both the resolved run base and merge base.

fsutil::merge_base resolves HEAD against the already resolved run base and caches the result for that run.
An unsuccessful Git query returns an error naming both commits; it does not substitute an empty ancestor or the base tip.
The explicit unrelated-history test checks that error. A missing head reference also fails.
The ancestor and merged-head test requires the fork before merging and the base itself after merging.
Thus a final head that contains the gate base compares against that base, without weakening the final shrink check.
fsutil::base and its existing callers retain their earlier meaning.

The new list tests distinguish base-side removals from actual branch additions for both pending files.
They require the correct public diagnostic decisions, including refusal of a real-pending addition that did not leave core-pending.
The deferral cases distinguish a base-side pin move from a branch's own deferral without a pin move.
The tests retain the failure of that unauthorized branch deferral. They do not assert private cache or file-reader state.
The cached-reference test observes the returned commit and uses the shared alone subprocess for environment isolation.
Git fixture commands use the existing run_to_completion helper with Deadline::cleanup and isolated Git configuration.
The Git queries under test retain the existing fsutil production-command style; no test separately reaps their children.
No Core worker or guardian fixture, signal, sleep, busy wait, or test-owned reap of a production child is added.

### Prior art and body

I read the named old CI workflow at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c` and its tracked-path inventory.
The named pending/deferred/ledger files and xtask directory are absent from that old tree.
The workflow validates the checked-out tree with formatting, lint, tests, docs, and smoke commands; it does not compare pending files with a base.
I also read the current base_merge command's Git query context at the source base.
It already resolves reviewed and new heads against their merge bases. This PR reuses that comparison form, not a new policy.
The implementation reuses the existing git_show, parsers, rule decisions, Git fixture helpers, and shared process helpers.
The body supplies a relevant prior-art disposition. This review claims no exhaustive scan of unrelated old code.

GitHub MCP truncates the body; the GitHub CLI supplies its complete text and exact head/base identities.
At 23:33:13Z the live body names the exact head and gate base above.
Raw body SHA256: `94fad25521c254c37c9d76ea4a705c11bf92f9bcb25c953af789edea88a186e4`.
The body matches the source, approved scope, and retained evidence. HIGH is correct for gate-decision code under BUILD.md rule 1.

### Focused and negative evidence

Log: `~/botster-sessions/gates/botster-core-stage1-p6-lists-merge-base-11b6e2ac-pool-20261011-162658-37804.log`.
SHA256: `e9389840ea06dae5489283a79e62103c783ec9eb497a1d8af92d36394d032d0d`.
I read the decoded command, exact-head header, all 44 named outcomes, summaries, reversal records, and exits.
All 44 selected fsutil/lists tests pass, with zero failed or ignored, in 0.16 seconds. Their command exits zero.
The focused clippy command completes and exits zero in 16.62 seconds.
The scratch reversal replaces the merge-base query with base(root), restoring the wrong tip choice.
The selected isolated test fails at fsutil.rs:422, and its outer alone wrapper reports the child failure.
The test command exits 101. The script restores fsutil.rs and prints no remaining tracked change.
The evidence job and wrapper exit zero after 30 seconds: one queued and 29 running.
The script prints command exit codes but does not enforce them. Its wrapper zero alone is not the negative proof.
The explicit passing and failing test outcomes and exit 101 supply that proof.
This is a retained negative test result, not a reversal executed by this reviewer.

### Completed gate

Log: `~/botster-sessions/gates/botster-core-stage1-p6-lists-merge-base-11b6e2ac-pool-20261011-162744-42287.log`.
SHA256: `17f4dde42884b6abfe9a09727275245d9c93d7528337370ef66e59dc6d8f723a`.
I read the exact-head/base headers, lists output, tier/stage summaries, all mutation records, named catches, and exits.
All ten stages pass in 291.4 seconds. The job and wrapper exit zero after 307 seconds: one queued and 306 running.
Default passes 1794 tests, with 460 skipped, in 16.408 seconds.
Slow passes 584 tests, with 2025 skipped, in 45.907 seconds.
The lists output names base f6025aed and merge base 5e953398, then passes with 28 real-pending identifiers.
That branch retains OU-5 and OU-12, which the later base removed. The actual gate therefore exercises the corrected base-file choice.

All six raw mutation identities are unique. The five raw catches exactly match the five named catch records.
Each named catch uses the merge_base isolated test, which passes normally in the gate.
The two merge_base replacements, two merge_base_of replacements, and deleted error-condition negation are caught.
Stage 1 reports five caught, zero missed, zero timeout, and one unviable, in 13.2 seconds.
The BaseFiles default-return mutant is unviable and supplies no catch evidence.
Stage 2 does not run because stage 1 misses none. The complete log contains no FLAKY record.
Fuzz reports no changed crate with a decoder harness. No mutation exclusion or conformance status file changes in this PR.

No package finding remains at this exact head. The source, approved design, prior-art note, retained negative proof, gate, and body are CLEAN.
The current-v1 import, applicable canonical check and delta review, final gate, and final body remain due at the assigned slot.
This verdict accepts no later head, import, pending removal, contracts pin, or PR completion beyond this reviewed head and evidence.
I changed no product code and ran no build, test, gate, mutation, reversal, measurement, diagnostic, or executable base-check job.

VERDICT: CLEAN
