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

### Correction of the reset coverage claim — 2026-10-10

Inspected head: `b7e6817bd844808a49a938b4be52376c4117fe2b`.
Gate base: `e418bf5a8bcc7c18313e7d82775e8ae29368b366`.
P5 withdrew its claim that the retained EOF proof covers the deleted reset proof's WouldBlock guard.
I withdraw that guard-coverage claim from my earlier replacement inspection too.
The historical source entry remains above. This correction supersedes its coverage claim.

The retained EOF proof reaches Ok(0), which closes the stand-in stream.
It does not reach Err(e) and does not distinguish the guard replacement with true.
The supplied gate misses this mutant in both stages:
`crates/botster-core-testkit/src/stand_in.rs:126:27: replace match guard e.kind() == io::ErrorKind::WouldBlock with true in PeerLink::read_and_answer`.
Log: `~/botster-sessions/shared/core-stage1/gate-logs/ctl-b-full-b7e6817b.log`.
Stage 2 reports 33 mutants: 29 caught, one missed, three unviable, and zero timeout, in 1528.7 seconds.
The gate fails its mutation step and exits one. The job takes 2082 seconds; the wrapper takes 2083 seconds.
This is supplied evidence. I ran no mutation or reversal job.

I read the complete stand-in source, its retained socket proofs, and its tap callers.
The public stand-in result reports whether it answered. The tap uses that result to fix the signal-record boundary.
For the host-reset case, the host endpoint is already gone.
The mutant retains the stand-in's socket until a later step instead of dropping it at the reset.
This does not change the answered result, the existing signal boundary, or a reply to that closed host endpoint.
StandIn::drop still drops its peer sockets and restores the endpoint path.
The source supports P5's proposed equivalent-at-public-boundary classification for that case.
This is a source inference, not a new proof of every possible socket error.

I support P5's option 1 only subject to the lead's ruling and a narrowly scoped exclusion.
The reason must name this complete mutant, the public boundary, supplied miss, and ruling.
The retained EOF proof stays. No private open-link accessor or whole-function exclusion is justified.
P5 already asked the lead for the ruling. I sent this source view to both P5 agents.

The earlier request to remove private assertions remains satisfied in the source.
Its mistaken reset coverage justification is withdrawn. Mutation acceptance remains unresolved.
No replacement READY, green exact-head gate, or same-head package verdict is supplied here.
This inspection completes no round and grants no CLEAN verdict.
I changed no product code and ran no execution job.

### Inspection of the proposed reset exclusion — 2026-10-10

Inspected head: `aa1eb5c45082e668119ca0dc5dec26f90efecad6`, after `b7e6817bd844808a49a938b4be52376c4117fe2b`.
The only change is six lines in .cargo/mutants.toml: the reason and one exact-name exclusion.
Static parsing confirms that it matches the named guard -> true mutant.
It does not match guard -> false, == -> !=, or a changed column.
I ran no mutation job for this check.

I read the lead's 18:25 and 18:30 rulings in the shared core-lead handoff.
The lead chose option 1. The lead rejected a private-state proof and a code restructure.
The lead requires the reset-to-EOF observations on both operating systems and a read on every stand-in step.
The reviewers must judge the complete error reason.

I read the retained measurement source and both output logs.
The source SHA256 is `70c2aeb65edffb71c2d9961b40f9ca88d6a118517390917d51048c335ea1ef09`.
Linux 6.12.111+deb13-amd64 reports ConnectionReset, then two Ok(0) reads.
Darwin 27.0.0 reports three Ok(0) reads after the peer closes.
These are supplied measurements. I ran neither measurement.
StandIn::step calls read_and_answer for every accepted link in Role::Answer, without waiting for a readiness edge.
The host-reset public-boundary inference from the preceding entry remains valid.

The stated generic AF_UNIX error reason still needs a scope limit.
The package reviewer found another error path in Apple's published XNU source: SCM_RIGHTS externalization can return EMSGSIZE.
That error can occur with a valid read buffer and a connected peer.
This does not yet establish that the permitted harness path can reach the error.
The shared announced_args rejects adoptable protocols. impostor_args permits only a wrong token or instance.
The real controls use those same checks. Role::Withhold does not execute read_and_answer.
Those checks can limit the input to the failed-adoption handshake, before descriptor handoff.
I sent P5 source feedback to state that actual scope in the exclusion reason and PR body.
I requested no private accessor or code restructure.

Primary source read: [Linux v6.12 net/unix/af_unix.c](https://github.com/torvalds/linux/blob/v6.12/net/unix/af_unix.c), unix_stream_read_generic.
Its signal-interruption branch follows the nonblocking EAGAIN return.
Primary source read: [Apple's published bsd/kern/uipc_socket.c](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/uipc_socket.c), soreceive and soreceive_ctl.
I also read [unp_externalize at the package reviewer's pinned source](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/bsd/kern/uipc_usrreq.c#L2383).
It returns EMSGSIZE when the receiving process has insufficient descriptor slots.
The generic socket read supports more error paths than the reason currently states.
I do not infer that every generic error is reachable through these controls.

No replacement READY, completed green gate, or same-head package verdict is supplied for this inspection.
This entry completes no round and grants no CLEAN verdict.
I changed no product code and ran no execution job.

### Scope clarification for the reset exclusion — 2026-10-10

The PR body at `aa1eb5c45082e668119ca0dc5dec26f90efecad6` now states the permitted harness inputs explicitly.
No compatible stand-in can be armed through these controls.
impostor_args permits only a wrong token or instance. impostor_hello constructs the corresponding failed proof.
announced_args rejects protocols in `{T, T-1}`. Role::Withhold never executes read_and_answer.
The real controls call these shared argument checks.
adopt_hello closes the failed proof or unsupported protocol before the adopted link can become usable.
The host admits no route attach during adoption. The sole descriptor send serves an AttachRoute mark.
Thus Core sends no SCM_RIGHTS message to these stand-ins through the declared control path.
The macOS descriptor-externalization error does not challenge equivalence within that path.

I read the shared control checks, impostor_hello, adopt_hello, attach admission, flush_handoffs, and the descriptor mark and send.
I accept the exact-name exclusion reason within this declared harness scope.
This acceptance does not establish a generic AF_UNIX error guarantee.
The earlier request to state the actual scope is satisfied by the revised PR body.
The package reviewer agrees with this scoped inference and will retain its limit in the final verdict.
The retained reset measurements and drop behavior support the unchanged public result after peer closure.

P5 reports a planned TOML comment clarification at `004fbd2a0c48a63f7d345ee22245fba8e6640622`.
P5 reports that its parent merges #234 into #228. That replacement is not yet published for review.
P5 will supply its own gate and READY. The aa1eb5c4 gate does not cover that new head.
No completed round or CLEAN verdict follows from this source-only scope clarification.
I changed no product code and ran no execution job.

### Replacement source and gate inspection — 2026-10-10 — Package verdict pending

READY head: `004fbd2a0c48a63f7d345ee22245fba8e6640622`.
PR and gate base: `0a564513e50930ce25f2e4a175f45cffbef7119d`.
Previously inspected exclusion: `aa1eb5c45082e668119ca0dc5dec26f90efecad6`.
The replacement merges #234 and changes only the exclusion comment after that merge.
This inspection completes no round before the same-head package verdict arrives.

The three Round 1 source corrections remain present.
R1-1 / RC-B-F3 removes the private link, stream, and signal-vector assertions.
The retained EOF proof observes closure through the socket. The signal proofs use `signals_since_answer`.
The mistaken claim that EOF proves the reset guard remains withdrawn.
The lead-approved exact-name exclusion settles that separate mutation question within the declared harness scope.

R1-2 / RC-B-F1 makes `worker_alive` require both matching identity and a running process.
`lives` checks AD-6 identity; `runs` requires a live member of the process's own group and excludes zombies.
Observation errors become Bad. A missing recorded worker returns false.
The real proof retains live, reused, and matching unreaped-ended process cases.
Shared `OwnedChild` guards own those test children. They observe exit within the cleanup deadline without reaping before the assertion.
Core retains ownership of its worker reaper.
The exact-head gate passes that proof and the live, ended, and unknown-session control proof.

R1-3 / RC-B-F2 keeps each worker and payload description above its corresponding method.
The PR body now names base `0a564513` and describes matching identity plus a running process for `worker_alive`.
I read those description corrections and confirmed the unchanged PR head and base.

The final exclusion comment now states the permitted stand-in path.
It names the failed handshake checks and the absence of descriptor sends to those stand-ins.
The prior source inspection and retained Linux and Mac reset measurements support that scoped reason.
The reason establishes no generic AF_UNIX error guarantee.
Static TOML parsing shows that the complete mutation configuration equals `aa1eb5c4` after the comment change and #234 import.
The own delta over v1 adds exactly one exclusion pattern.
It matches the named guard-to-true mutant. It does not match guard-to-false, equality-to-inequality, or a changed column.
The EOF proof remains. Stand-ins still read each accepted answer link on every step without a readiness-edge wait.
The nonblocking accept allowance retains its prior reviewed scope.

### Source merges and coverage

All eight source merges after the previously checked `5586e0ee` match their automatic trees:

- `2c998bcc`: `472aa8ce4daf47b9ec48fd69ab530945ae280ad0`.
- `ff792fd1`: `d21a8d3bcc911964dff535e9ff0f46bf5ce38d70`.
- `273fb9ac`: `7ed1ac3ba7086dcacafabfb4dbe8de87140a1fe1`.
- `2cc01d21`: `a20be489b87720d88ecd2a7e1054ef47e2e83b1b`.
- `00365c57`: `8026148c73489e7e79a6fc49ca6529e1d1222a7a`.
- `01643ca3`: `8026148c73489e7e79a6fc49ca6529e1d1222a7a`.
- `b7e6817b`: `a69761cdae7f1efebc6e0b1a6f881ea7738661c1`.
- `54a8e526`: `bc2f8033a1cf4ff039b4e71d039b48784b85070e`.

All testkit and real-pending bytes equal the inspected `b7e6817b` bytes.
The only testkit change from `5586e0ee` to that head imports the reviewed status change.
The replacement retains #222's final accepted source and evidence, the accepted gate changes, and #234.
#222 merged first at `b028a269`. Its separate native Mac finding is closed in the #222 Round 3 verdict.
The #228 own diff adds no platform-only code. This replacement adds no separate native Mac gate requirement.

The same ten IDs recorded in Round 1 leave real-pending, with no additions: 81 becomes 71.
I found every removed ID in the exact-head real conformance PASS output.
The four minimum removals raise real-passing from 31/68 to 35/68 and real-accepted from 31/69 to 35/69.
Core pending, minimum lists, held IDs, real-only maps, and other coverage lists have no own change.
`ad_2_lost_row_keeps_id_and_allows_retry` remains pending for its separately planned exit notification.
`git diff --check` passes. The PR head and base match the supplied gate.

### Supplied exact-head gate

Log: `~/botster-sessions/gates/botster-core-stage1-p5-real-controls-b-004fbd2a-pool-20261010-190048-7409.log`.
Its header names the exact head and base. The command is plain `cargo xtask ci` through the pool.
All ten jobs pass. Total CI time is 2012.6 seconds.
Default: 1536 passing tests, 12.682 seconds. Slow: 408 passing tests, 117.356 seconds.
Both testkit reports show 193 passed and zero failed. Strict pending runs 386/407, with 21 real-only IDs and two held IDs.
The reports list 70 IDs without transcripts, two deferred IDs, and 18 withdrawn IDs.
Minimum counts are testkit 50/69, real 35/68, and real-accepted 35/69.
Real-pending is 71, with zero passing IDs in that list.

Stage 1 has 76 mutants: 35 caught, 32 missed, nine unviable, zero timeout, in 191.0 seconds.
The unmutated stage-2 baseline passes in 100.7 seconds, with 408 tests passing.
Stage 2 has 32 mutants: 29 caught, three unviable, zero missed, zero timeout, in 1484.0 seconds.
Static parsing of every printed outcome confirms that the stage-2 exact-name multiset equals all 32 stage-1 misses.
There are no duplicate stage-2 names. Total: 76 mutants, 64 caught and 12 unviable, with zero missed or timeout.
The mutation job takes 1778.3 seconds.
The job exits zero after 3347 seconds, including 1319 queued seconds. The wrapper takes 3348 seconds.
Queue time is not a source blocker.

I found no open source finding in this replacement. The exact-head package verdict remains pending.
I changed no product code and ran no build, test, gate, mutation job, measurement, merge-check job, or reversal job.
The tree comparisons use Git source objects only. All execution results above come from supplied evidence.

## Round 2 — 2026-10-10

Reviewed head: `004fbd2a0c48a63f7d345ee22245fba8e6640622`.
Reviewed base: `0a564513e50930ce25f2e4a175f45cffbef7119d`.
The replacement inspection above records the source corrections, scoped exclusion, eight source merges, coverage, and complete supplied gate.

I read package Round 2 at verdict commit `d98c6a04ffe7209575b32b2607b8c9f1b7da7ec9`, `verdicts/p5-real-controls-b.md`.
It reports CLEAN on this exact head and base. Both reviewers agree on all three closures and the exclusion's scope.
Plan 23af is accepted at `4f6516bd638bcf835dae5d7496e6ed9dc660bf5a`; R-54 changes no requirement for this package.
Its published pin matches SHA256 `8e463cfda8bffa98cbfd321995b13c57e4eb752ef9fc9ccff366ca9c07f40bc6`.

R1-1 / RC-B-F3 LOW is CLOSED: the proofs use observable socket behavior and the public signal record.
The earlier EOF-guard coverage claim remains withdrawn. The scoped exact-name exclusion resolves the surviving reset-guard mutant.
R1-2 / RC-B-F1 HIGH is CLOSED: real worker liveness requires identity and a running process, with the unreaped-ended proof passing.
R1-3 / RC-B-F2 LOW is CLOSED: worker and payload descriptions attach to the correct methods.
No finding remains open, including LOW. I found no new cross-package issue.

#222 merged first. Its native Mac evidence and both reviews are complete.
The replacement retains #234 and all reviewed gate changes.
All ten supplied gate jobs pass. All ten real-pending removals have exact-head real-tier PASS results.
Both mutation stages pass with exact stage-2 selection: 64 caught, 12 unviable, zero missed, and zero timeout.
This verdict accepts the exclusion only within the declared failed-handshake stand-in path.
It establishes no generic AF_UNIX error guarantee.

I changed no product code and ran no build, test, gate, mutation job, measurement, merge-check job, or reversal job.

VERDICT: CLEAN (0 open).
