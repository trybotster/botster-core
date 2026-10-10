# PR #228 — Real endpoint stand-ins and worker liveness

## Round 1 — 2026-10-10

Reviewed head: `a263ff04532e952687fd1c4b8c9d3a1b3037692d`.
PR base: `b4847bf51b98a2a720e60d7078b88397e2059bbf`, the #222 branch.
Gate base: `c19ad938bd87c60331bd002c0117102d8756b1ae`.
Tier: HIGH, shared testkit changes.
Scope: all 13 own-diff files, affected production edges, the controls rows, R-49, R-46, and supplied evidence.
Authority: approved plan 23y and the accepted P5 controls plan.
I read the controls and rulings at pinned contracts `0ac061d225724f6f7fc7b2368f637ba2c9a6c2b6`.
I read BUILD.md's test rules at `56bd0a5`.

### R1-1 — LOW — OPEN — New tests assert helper internals

Locations:

- `stand_in/tests.rs:91`: the test reads private `links.len()`.
- `stand_in/tests.rs:178` and `:197`: the tests read private `links[0].stream.is_none()`.
- `edge_tap/tests.rs:681` and `:841`: the tests read the private signal vector directly.

BUILD.md Testing shape rule 3 states that no test tests a helper's internals.
The socket EOF proof already checks the observable EOF after the host closes its write side.
The tap exposes the signal record through `signals_since_answer`.
Remove redundant private assertions.
Use observable socket behavior and `signals_since_answer` for the retained proofs.
If a duplicate test is removed, name the retained test that covers its clause.
I found this issue independently and sent it to P5 and the package reviewer.

### R1-2 / RC-B-F1 — HIGH — OPEN — `worker_alive` reports an unreaped ended process as alive

Location: `real.rs:437-443`.

The real control returns true whenever the recorded worker has `IdentityState::Matches`.
The controls row defines `worker_alive` as whether the worker process still runs.
Matching identity establishes which process holds a pid. It does not establish that the process runs.

`RealEdges::identity_state` delegates to `botster_core_sys::process::identity_state`.
On Linux, `process::start_time` reads field 22 from `/proc/<pid>/stat` without checking the state field.
An unreaped zombie retains its pid and start time, so this path returns Matches for that ended process.
The real control then returns `{ "alive": true }`.
The testkit control checks `!cell.ended` and returns false for an ended worker.
The tiers therefore disagree on the control's required behavior.

The cited R-46 follow-up calls for an identity-checked read like `payload_alive`.
It does not redefine liveness as identity presence.
The existing `lives` helper already combines the matching identity with `runs`, which excludes zombies.
Use that helper or an equivalent identity-and-liveness check.
Correct the method description to state the required liveness result.
Add a real-process proof that retains an ended process unreaped and expects false for the recorded identity.
Retain the live-worker result and the refusal of a reused identity.
Use the shared process guards and production ownership of reaping.

The current real proof waits until the killed worker's identity is Absent before checking false.
It skips the unreaped interval and does not cover this defect.
The package reviewer reported RC-B-F1. I independently traced the complete edge and process-identity path.

### R1-3 / RC-B-F2 — LOW — OPEN — The worker methods have the wrong attached documentation

Location: `worker.rs:691-693`.

The new `worker_alive` method follows the two existing documentation lines for `payload_alive`.
Rust now attaches those payload lines to `worker_alive`, while `payload_alive` has no attached description.
Move the payload lines back above `payload_alive`.
Keep the worker description above `worker_alive`.
The package reviewer reported RC-B-F2. I independently confirmed the changed method order and attached comments.

### Other source checks

The endpoint stand-in follows the mechanism accepted by R-49 Q4.
The control arms it from the stored row, including the recorded token and identity.
Installation occurs before reopening Core, after checking that no live tap holds that data directory.
An attempted second open while the first Core holds the directory installs nothing and leaves the arm pending.
The real-process proof checks that condition and the later reopen.
The guard restores the original endpoint on drop; failed installation also restores the renamed endpoint.
The tap declares its inner edges before its stand-ins, so the directory lock ends before endpoint restoration.
The proof checks the original endpoint inodes after the tap ends and verifies that both real workers still run.

The tap records every Core `signal_group` call in order with its target identity.
That is Core's signalling edge at this head.
The harness's own identity-checked kill bypasses that record.
The record begins at the stand-in's first answer, before Core can reject the handshake.
It includes later calls to every target, as R-49 Q1 requires.
The controls do not infer an empty signal record from the stand-in process's identity.

The shared argument and answer helpers retain the testkit's token, instance, protocol, and script behavior.
The protocol control refuses an in-set protocol.
The stand-in sends the hello and permitted script in one write and treats a short write as setup failure.
The withheld role accepts without answering; Core's own handshake deadline decides the outcome.
Core and the production worker have no new test branch.

The new process-check allowance is valid for this source.
`StandIn::install` sets the listener non-blocking before construction returns.
The listener field is private and no later path changes that mode.
The allowed `accept` site therefore returns immediately, including WouldBlock.
Each accepted stream becomes non-blocking before a read; a failed mode change drops that stream.
The allowance names one site and does not authorize a blocking wait.

### Pending-list and gate evidence

I compared both pending-real sets: 81 becomes 71, with ten removals and no additions.
I found all ten removed ids in the exact-head real-tier PASS output:

- `conf::a10_1_wrong_instance_is_never_signalled`
- `conf::a10_1_wrong_token_is_never_signalled`
- `conf::a2_1_adopt_completes_ok_with_the_state_it_reached`
- `conf::a6_1_withheld_control_link_gives_worker_unreachable_not_worker_gone`
- `conf::a6_2_out_of_set_hello_is_lost_worker_version`
- `conf::ad_4_absent_worker_protocol_is_unknown_never_compatible`
- `conf::ad_4_compatibility_is_not_a_guarantee_of_adoption_auth_and_reachability_can_still_fail`
- `conf::ad_4_unknown_version_is_lost`
- `conf::ad_6_token_and_instance_checked`
- `conf::lc_9_worker_protocol_absent_for_an_unreachable_worker_whose_hello_was_never_read`

Core pending, the ledger, deferred files, copied status files, minimum list, and real-only map equal the PR base bytes.
The four minimum removals raise real-passing from 31/68 to 35/68.
`ad_2_lost_row_keeps_id_and_allows_retry` remains pending for the separately planned exit-notification control.
These passing results do not close R1-2's missing unreaped-process case.

Full gate:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p5-real-controls-b-a263ff04-pool-20261010-133218-79259.log`.

The header names the exact head and gate base. All ten stages pass.
Default: 1,500 passed, 497 skipped. Slow: 403 passed, 1,935 skipped.
Testkit conformance: 193 passed, zero failed.
Pending-real: 71 ids, zero passes.
Minimum counts: testkit 50/69, real-passing 35/68, real-accepted 35/69.
The real stand-in proof passes in 1.183 seconds.
Both default mutation commands report 105 mutants: 88 caught, 17 unviable, zero missed or timeouts.
The full command takes 589.1 seconds. The repeated mutation command takes 230.0 seconds.
The job and gate exit zero after 837 seconds on msa1.
The gate predates #225 and does not supply its two-stage mutation behavior.

Manual slow mutation evidence:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p5-real-controls-b-a263ff04-pool-20261010-134627-9621.log`.

The command selects the PR's own diff against `b4847bf5`.
It enables `slow`, uses `--no-config` and `--in-place`, and sets immediate fail-fast behavior.
The slow baseline selects testkit and passes all 345 tests.
I parsed all 79 records and checked the retained mutant failure sections.
All 66 catches retain named test failures and test exit 100.
All 12 unviable mutants retain compile errors and build exit 101.
There are no missed mutants or timeouts. The job and gate exit zero after 289 seconds, including one queued second.
The own diff adds no platform-only code, so this PR adds no separate native Mac evidence requirement.
Its inherited #222 evidence finding remains open.

### Review limits and next head

The PR base and gate base are ancestors of the reviewed head. The remote PR head and base match.
`git diff --check` passes.
The lead reports that #225 merged and `v1` advanced to `fc1e8160abfd8248b155383ef1d766c6377f8a11`.
The replacement must include the required base checks and exact-head gate under the current command.
#222 must merge before this stacked PR. This verdict does not close #222's native evidence finding.
The package verdict commit is pending; its reported RC-B-F1 and RC-B-F2 are confirmed above.
I changed no product code and ran no builds, tests, gates, mutation jobs, merge-check jobs, or reversal jobs.

VERDICT: NOT CLEAN (1 HIGH, 2 LOW open)

### Package verdict receipt — 2026-10-10

I read package verdict `8d5f0f38f3a25076e2e349caf4017824468e51c9`, `verdicts/p5-real-controls-b.md` on `stage1/review-p5`.
It reports NOT CLEAN on the same head with the same three findings.
RC-B-F1 matches R1-2; RC-B-F2 matches R1-3; RC-B-F3 matches R1-1.
Both reviewers accept the non-blocking process-check allowance and retain #222's separate native evidence finding.
This receipt changes no review scope or conclusion. Round 1 remains NOT CLEAN.

### Replacement source inspection — 2026-10-10 — Gate pending

Reported replacement head: `7251ba42d7023585cadeac00e9f8a9f18fa3c62f`.
Own PR base: #222 `deb34002eae1df16199b753c510ebfc44475b8a9`.
The replacement contains v1 `fc1e8160abfd8248b155383ef1d766c6377f8a11`.
Its correction commit follows base import `74347be6385d1a0e1c14ccee7aa5bccb218cb926`.
The lead permits source review before the replacement gate completes.
This entry records that source inspection. It gives no new CLEAN or completed round.

All three source corrections are present:

- R1-1: private links, stream, and signal-vector assertions are removed.
  The new signal proof reads `signals_since_answer` and distinguishes the harness kill from Core's signal.
  The retained `the_stand_in_closes_its_end_when_the_host_closes` checks socket EOF.
  It covers the common closure arm of the deleted reset test.
- R1-2: `worker_alive` calls `lives`, which requires matching identity and live group membership.
  Observation errors become `ControlError::Bad`; a missing recorded worker returns false.
  The new real-tap proof checks a live identity, a reused identity, and a matching unreaped ended identity.
  Its shared `OwnedChild` guards own the test children and observe exit within a deadline without reaping early.
  Their drops clean up those children on every path. Core retains its own worker reaper.
  The control-level live, ended, and invalid-session proofs remain.
- R1-3: the payload documentation is back above `payload_alive`.

The correction changes four files. The other nine own files equal Round 1 bytes.
The automatic base-import tree and actual import tree both equal `f4061e77a249a0cb205efb6cae71bf323d2370a9`.
The own base and stated v1 tip are ancestors of the replacement.
I found no new source issue in this correction.

P5 supplies no exact-head gate for this replacement yet.
P5 reports that #222's Linux gate failed on #225's stage-2 selection defect.
The separate #225 addendum records the independently confirmed defect.
The next completed review needs the corrected gate, its exact head and base, and the supplied merge evidence.
#222 still must merge first and still needs native Mac evidence.
I sent this source result to P5 and its package reviewer.
I ran no build, test, gate, mutation job, or reversal job.

### Further base imports — 2026-10-10 — Gate pending

Reported head: `5586e0eebed19df81247c12c82642fab077a2d7b`.
Own base: #222 `996a17f0afab8f6509d32623cfeaf00619fd2a01`.
This head merges #222's updated bases and its native Mac correction into the source-checked `7251ba42`.
There is no additional own correction.
The sole changed byte line in a #228 own file imports #218's `pub mod pending` into testkit `lib.rs`.

The automatic `7251ba42` plus `7351d67e` tree equals actual `3aab17d0`, tree `cb503af88673652fc28af1755c2f09ddfbdee5fe`.
The automatic `3aab17d0` plus `996a17f0` tree equals actual `5586e0ee`, tree `ce95fd91facec1a91ca0292a00a9ba2fe0e48a69`.
The inherited Mac correction is reviewed in `verdicts/stage1/p5-real-controls-a.md`, Round 3.
The required native Mac evidence now closes there. Its final full gate and package verdict remain pending.

The three #228 source corrections remain present. `git diff --check` passes.
No exact-head gate is supplied for this head. #222 must still merge before #228.
This entry gives no new CLEAN or completed round.
I ran no build, test, gate, mutation job, merge-check job, or reversal job.
The tree comparison uses Git source objects only.
