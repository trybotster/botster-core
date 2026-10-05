# Handoff: P6 package reviewer

The reviewer is PAUSED by the user order relayed through the lead on 2026-10-04.
All verdicts are committed, pushed, and sent. No review is active.
The Sol reviewer uses HIGH reasoning effort.

## Workspace and sessions

- Repository: `trybotster/botster-core`.
- Branch: `stage1/review-p6`.
- Worktree: `/Users/jasonconigliari/botster-sessions/trybotster-botster-core-stage1-review-p6`.
- Pushed verdict head: `d1a3869e68f90c0ab69a951141304776e0818eb8`.
- The local head equals `origin/stage1/review-p6`. The worktree is clean.
- The spawner changed `.gitignore` at takeover. This reviewer ran `git restore .gitignore` as the user directed.
- Never commit the spawner's `.gitignore` change.
- Lead: `sess-1790903471-008f-8b9f78eef51d48aba5a45748495fd673`.
- Current implementer: `sess-1791168867-010c-c64ca4250bee21ff7b750bdb8ab111b4` (Opus).
- Current integration reviewer: `sess-1791168757-0109-73d2ca212653045545e7480ab60be9a9` (Opus).
- Retired implementer: `sess-1791136735-00fc-4b09baad3d85f4ce4397761167aced46`. Do not message it.
- Retired integration reviewer: `sess-1791143089-0101-8ce5f4942328f5697c410ea4da89c466`. Do not message it.

## Verdict files and latest rounds

| File | Exact reviewed implementation head | Latest verdict commit | Result |
|---|---|---|---|
| `verdicts/p6-testkit.md` | `744da574039d30d49047b6a661d6849887de53b9` | `2793fd65d609409271f922e9b0c587e2eb27134e` | CLEAN; inherited review, file unchanged in this session |
| `verdicts/p2-binding-oracle-api.md` | `a088d673d759984d9b10823d17d713b45c006ce9` | `527815d96996e972d3d45a01e20b228ae94d6226` | CLEAN |
| `verdicts/p2-ghostsnp-spec.md` | `d8b84543ab444f2cd04799eb9de6c9c7a7734d0c` | `2882a220f4e651e4d8f8b02f35a3b6bf89a5684e` | CLEAN; worker framing explicitly pending |
| `verdicts/p6-oracle.md` | `c36120e62a3252c1588bf88fa860e5325939ee19` | `ba01ce494871eb7558279b4a279dd964e8aaa8de` | CLEAN; O1–O3 closed |
| `verdicts/p6-control-registry.md` | `7e9b4ec6e7252c3ff7ec36cbff74c24fdae644fe` | `db280708bbdf9ecdef5df04d0dbef2bf07c6c3aa` | CLEAN |
| `verdicts/p6-contracts-v0.1.14.md` (PR #161, now targets v0.1.17) | `dea90ed4ee502432a70e19ec93b26c4a81558773` | `d1a3869e68f90c0ab69a951141304776e0818eb8` | CLEAN for the pin move; gate pending |

The oracle's initial round at `c57e0da22fb3ea461cd85dc756bbc0abcff5a2f7` was NOT CLEAN with three findings.
Verdict commit: `ff2ea6b27fc2978ebaed5a12b60b98b2c574900a`.
O1 required public query and hyperlink helpers.
O2 required retention checks independent of capture outcome and failure reads immediately after capture.
O3 required non-fitting cuts to remain inconclusive and oversized offers to record a mismatch.
The reviewed delta at `c36120e6` closes all three.
There are no open package findings, including LOW findings.

The lead reported PR #140 merged at `v1 e8cf150`.
The shared implementer handoff records PR #141 merged at `144b0234fb632bcbb5176b17c2fe55f3239405df` with integration CLEAN and a full green gate.
This reviewer ran no tests, mutation jobs, or gates during these reviews.

## PR #161 review history and current boundary

PR #161 remains on branch `stage1/contracts-v0.1.14`. Its current target pin is `contracts-v0.1.17`.
The filename remains `verdicts/p6-contracts-v0.1.14.md` because all rounds review the same PR.
Every earlier verdict round and finding history remains in the review branch.
No open finding remains for this pin move, including LOW findings.

| Target tag | Exact implementation head | Pushed verdict commit | Result |
|---|---|---|---|
| contracts-v0.1.14 | `5a34cf84d840c9424ef83b1bb8e37677b4524b0c` | `7b57da7af4b3f5f22604243d38708888fb54035a` | CLEAN |
| contracts-v0.1.15 | `b27fdacfad36149cc194a3e60416a03fd0e134af` | `ab235da6cf109203cae9cc5589f35dbdbd225ccd` | CLEAN |
| contracts-v0.1.16 | `257d2ed31dbb5af06551de463091e110eabcc06d` | `f61ebf82a1ca7ed451215ad79b4a1a17a743767b` | CLEAN |
| contracts-v0.1.17 | `dea90ed4ee502432a70e19ec93b26c4a81558773` | `d1a3869e68f90c0ab69a951141304776e0818eb8` | CLEAN |

The final tag resolves to `1725abf1d95aa880cb38b80a2010470e5abcb98f`, manifest final35.
The tag includes Core Amendments 14, 15, and 16, and steward rulings R-30 through R-34.
No contract crate changed across these tag moves. The pin PR changes no executable Rust statement.
The final Core ledger contains 675 IDs, up from 654: ten A14, four A15, and seven A16 IDs.
All 21 new IDs remain pending. No existing ID leaves the ledger or pending list.
The reviewer verified exact ledger bytes, copied status files, dependency tags, and lockfile sources without builds or gates.
A14 belongs to P3. A16 belongs to P7.
A15 owner comments name P5 for host and limits, P3 for admission at the bound, and P4a for the route refusal.
These CLEAN verdicts do not prove the new behavior or close other packages' findings.

The supplied gate of `5a34cf8` failed two slow tests and ran no mutation or fuzz steps.
The failing tests were `a_worker_is_not_left_when_the_cleanup_of_a_test_fails` and `the_pty_counts_output_and_delivers_input_to_the_program`.
The verdict does not close or waive A10 or A31, or establish the failures' cause.
The lead parked #161 behind the P3 guard fix, #162, #163, and #142.
The reviewer ran no tests, mutation jobs, builds, or gates during these four rounds.

## Pending reviews at pause

No submitted READY head awaits review. No review step remains in progress.
Future submissions still require review:

- PR #161 after the required v1 merge, on the new exact head, before its final gate.
- The RealCoreHarness PR after the implementer submits READY. No RealCoreHarness source review occurred in this session.

Use `verdicts/p6-real-harness.md` for the RealCoreHarness PR when it is submitted.
Keep separate verdict files for other new PRs.
The inherited scaffold at `d54ff94fdfdb9a83996083b02db7ff4486f3d242` remains unreviewed.
The implementer reported that RealCoreHarness work continues. No later READY head was supplied to this reviewer.

## Waiting work

No PR or READY head currently waits for this package reviewer.
The next work is `stage1/p6-real-harness`.
The lead identifies its inherited scaffold as `d54ff94`.
The shared implementer handoff gives the full head as `d54ff94fdfdb9a83996083b02db7ff4486f3d242`.
That scaffold has no package verdict. Do not treat its design or source as verified.
Read `shared/core-stage1/handoffs/p6-testkit.md` before its review.
The shared handoff contains outdated session UUIDs; use the current UUIDs above.

## Binding review rules

Read `shared/core-stage1/pair-common.md`, the assigned brief, BUILD.md, and plan pin `stage1-plan.bdda2359.md`.
The original handoff used `contracts-v0.1.13`. The last reviewed pin PR targets `contracts-v0.1.17`.
Confirm the lead's active integration pin and plan revision when work resumes.
Review exact heads. Every finding must close, including LOW findings.
Do not run gates. Read supplied proof and inspect source.
Libghostty owns all terminal semantics. Expected terminal values come from native APIs.
Production code must have no test branch. Faults enter through injected edges.
Do not accept tests that exist only to kill a mutant or conceal missing behavior.
Mutation exclusions must name one function and state an accurate reason.
Real-process tests own group cleanup on Drop and panic. Children block without CPU and exit when the parent is gone.

The lead ended the restriction on `harness.rs`, `program.rs`, `core.rs`, and `worker.rs` when P3 M1 merged at `01fd389`.
P3 still owns its M2a worker/program changes and agreed `open()` changes. Coordinate before overlapping edits.
Controls register from their owning module through `ControlRegistry`. Do not expand harness dispatch directly.
Live oracle dispatch waits for P3 M2b. Independent argument adapters must claim no unavailable terminal capability.

## R-30 and graphics rulings

Read R-30 in contracts `docs/steward-rulings.md` at `eb4aba0`.
The every-cut fit proof uses exact independent native encoding with the same format, version, size, and history settings.
Oracle retention must be at least the input length.
Framing comes from the format spec, never from the subject capture.
Count every per-capture field at its largest specified size.
Unknown format or state remains inconclusive.
An inconclusive within-limit cut cannot pass the ST-6b every-offset id.
The current `UnknownFraming` source remains inconclusive until P3 completes worker paging in `GHOSTSNP.md`.

The fork does not expose `Tracker.broken`. Its semantic failure flag is separate.
The lead accepted unavailable retention within an independently measured continuation limit as a mismatch under the no-injection condition.
Unknown pending length remains inconclusive. Known above-limit unavailability can support refusal classification.

The revised `oracle_graphics` ruling uses native image storage limits on both actual instances.
The model constructor and decoder set the limit to zero before input or continuation replay.
Libghostty enforces zero storage on both screens. P2 constructor tests use explicit native image lookups.
The control needs no image parser or stimulus record. The fork has no complete image count export.

## Anchor ruling for the next review

The lead assigned cleanup through public launch wrappers and a prebuilt `botster-test-anchor`.
Do not add a production Process seam or test branch.
The full ruling appears in `handoffs/p6-testkit.md`, section `Binding anchor ruling`.

1. Prebuild the anchor with `cargo xtask prebuild-worker` and include it in the sha256 manifest.
2. The wrapper must exec the verified real binary and preserve identity, exit status, and signals.
3. The wrapper starts a detached anchor through an intermediate child and reaps only the intermediate.
4. The anchor holds the inherited group and guard descriptor. It reports identity and group before the real body starts.
5. Worker launch uses `OpenConfig.worker_path`. Payload launch executes the verified probe in the payload session and group.
6. Guard EOF represents test Drop or test death. The anchor ignores TERM for itself.
7. Cleanup sends group TERM, waits the existing configured grace, verifies recorded identities and membership, and sends group KILL.
8. KILL is the final action and ends the anchor. The anchor reaps nothing.
9. Core and the worker retain exclusive reaping of production-owned children.
10. Linux may use `PR_SET_CHILD_SUBREAPER` and reap only verified descendants. macOS relies on anchors and init reaping.
11. A group move after exec must be reported. The anchor must refuse to kill an unverified group.
12. Require panic and killed-test cleanup proof on Linux and macOS. The lead requires a Mac run.
13. Require real worker PID, exit, SIGUSR1, WorkerGone, and Remove completion proof through production entry points.
14. If a wrapper changes exact argv or paths required by an id, retain the pending id and send the lead a QUESTION.

Inspect descriptor isolation, startup failure, parent death before registration, cleanup deadlines, and verified anchor reaping.
The earlier proposal to reap a real child before final KILL is superseded.
No busy loop, name-based discovery, or unverified signal is allowed.
The user ordered this reviewer to pause. Confirm the lead's current execution restrictions when work resumes.

## Messages and idle behavior

Send verdicts to the current implementer with the exact implementation head and pushed verdict commit.
Send the lead only QUESTION, BLOCKED, or terminal events.
`post_message` takes `{session_uuid, payload}` with a string payload.
Never poll. End the turn while waiting. On a doorbell, call `receive_messages` once.
Do not spawn agents. Continue on this branch and worktree under the replacement session.
