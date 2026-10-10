# PR #222 — Real process and storage controls

## Round 1 — 2026-10-10

Reviewed head: `6b720cb67b8414314b09d946fe90380eb34fdf24`.
PR and gate base: `b41e88be535c7e1c62ddf272c4295a87b94f5685`.
Tier: HIGH. The PR changes the shared testkit and the real-tier acceptance controls.
Scope: all seven changed files, affected production callers, the approved control plan, pending-list changes, and supplied evidence.

### R1-1 — HIGH — An unread launch report does not prove that no payload runs

`payload_alive` returns `{ "alive": false }` for `Launch::Pending` at `real.rs:366`.
The tap sets Pending when `spawn_worker` succeeds.
It changes Pending only when its inbound observer reads the worker's `Launched` report.
It does not track whether the Launch request already reached the worker.

The worker starts the payload before it queues `Launched` (`worker.rs:593-607`).
The host can delay its next read while that payload runs.
During this interval, the ordinary startup row has no payload identity and the tap still reports Pending.
The control therefore reports absence for a live payload.
This can make a conformance assertion pass without proving payload absence.

The new real-process proof pumps through Running before checking liveness, so it reads the report before making this assertion.
It does not cover the unread-report interval.

Keep the result unknown after a launch can reach the worker unless an identity-checked observation establishes the result.
If the control reports absence before launch, prove that the launch cannot yet reach the worker.
Add a real-process proof with a live payload and an unread `Launched` report.
Do not infer payload absence from an unread report.

The package reviewer reported RC-A-F1. I independently confirmed the tap, control, worker, and row-write paths.
Status: OPEN.

### Other source checks

The shared `damaged` helper preserves the simulated tier's truncation behavior.
The real control reads the exact row key, damages the stored bytes, checks rejection by Core's decoder, and writes through the storage edge.
Its write preserves the tap's record of the original process identities.
The default proof checks odd, even, and empty byte sequences plus decoder rejection.

`lose_worker` uses the recorded worker identity and refuses absent or reused identities.
The tap calls the inner signal edge only for Matches; the production signal edge checks the identity again.
The worker starts in its own process group. The payload starts in a separate session and process group.
The real proof checks group separation, refusal of a mismatched start time, worker termination, and refusal after termination.
The control sends no signal to the payload's group.

The new frame observer reads successive frames and retains the first launch report across reads without changing the forwarded bytes.
The tests cover split frames, several frames in one read, malformed hello bytes, unexpected frame kinds, and connected-link behavior.
Those checks do not establish the negative liveness claim in R1-1.
Plain harnesses continue to return Unsupported for all three controls.

### Pending-list and gate evidence

The pending-real list decreases from 87 to 81 ids and adds no id.
I independently compared the id sets and found all six removed ids in the exact-head real-tier PASS output:

- `conf::a10_2_corrupted_row_is_lost_registry_corrupt`
- `conf::a2_1_read_of_created_or_lost_is_wrong_state`
- `conf::ad_1_running_adopts_running`
- `conf::ev_5_stop_kills_while_queue_full`
- `conf::lc_12_stop_all_leaves_created_exited_lost`
- `conf::tm_6_more_means_runnable_not_blocked`

These are observed passes under the reviewed harness. They do not resolve R1-1's missing timing case.
The two regrouped ids remain pending.

Gate log:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p5-real-controls-a-6b720cb6-pool-20261010-091048-97964.log`.

The header names the reviewed head and base. All ten stages pass in 440.7 seconds.
Default tests: 1,471 passed. Slow tests: 386 passed.
Testkit conformance remains 193 passes, 407 pending trials, 70 entries without transcripts, two deferred trials, and 18 withdrawn trials.
The report gives 81 pending-real ids with zero passes.
Minimum counts are testkit 50/69, real-passing 31/68, and real-accepted 31/69.
Both mutation stages report 33 mutants: 27 caught, six unviable, zero missed, and zero timeouts.
The repeated mutation stage takes 188.0 seconds and does not enable the slow feature.
The job exits zero after 636 seconds on msa1, including one queued second.

### Manual slow mutation evidence

Log:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p5-real-controls-a-6b720cb6-pool-20261010-092200-16909.log`.

The command selects the PR diff, enables the slow feature, disables exclusion configuration, and uses immediate fail-fast.
The baseline and mutant commands select the same testkit package and slow profile.
All 328 baseline tests pass.
I parsed all 52 mutant outcomes and checked their retained failure sections.
All 45 catches build successfully and end with test exit 100 and named failures.
All seven unviable mutants fail compilation with missing Default implementations and exit 101.
There are no missed mutants or timeouts. The job exits zero after 194 seconds on msa1.
This evidence satisfies the interim manual coverage scope; it does not test the missing unread-report case.

The gate base is an ancestor of the head, and `git diff --check` passes.
The remote PR head matches. The remote `v1` has since advanced to `26843c74b460688c8e7219eb48a60420b930061e`.
The replacement review must account for the current base and its gate evidence.
I sent R1-1 to the implementer and package reviewer. The package verdict artifact is pending.
I changed no product code and ran no builds, tests, gates, or mutation tests.

VERDICT: NOT CLEAN (1 HIGH open).

## Round 2 — 2026-10-10

Reviewed head: `a6aa195660d9021995a0487db403c57534415103`.
PR and gate base: `7caf3457a04bd37f5d4e844db1900525eea7c8ec`.
Tier: HIGH. Scope: the complete PR delta, the replacement delta, affected callers, pending-list changes, and supplied evidence.
The intervening merges have the automatic merge trees. I found no added merge resolution.

### R1-1 — CLOSED

The tap now records only payload identities from observed `Launched` reports.
It no longer treats an unread report as evidence that no payload runs.
For a known payload, `payload_alive` requires a matching identity and a live process.
For an unknown payload, it lists the recorded worker's live children through the shared process crate.
It then requires the same worker identity to remain live.
That check establishes that the recorded parent existed throughout the listing; a live process keeps its pid.
The worker has one direct child, its payload. The worker and payload have separate process groups.
If the unknown payload's worker ended or its identity changed, the control returns `Bad`.
It does not infer absence from that state, because the payload can outlive its worker.

The new real-process proof reopens a running session without reading its launch report.
It proves that the control reports the live payload through the child listing.
It ends the payload and proves absence while the recorded worker remains live.
It then ends the worker and proves that the control returns `Bad`.
The exact-head Linux gate runs this proof in 1.095 seconds.
The shared process proof covers a live child, an empty listing, an unreaped zombie, and a reaped child.
Both proofs use the shared process guards and derived deadlines.
Production retains its own reaping. The PR adds no production hook.

The Linux adapter reads each process's parent and state from `/proc` and excludes zombies.
The macOS adapter uses `libproc`'s parent filter and rechecks each process's parent and state.
It clears `errno` before the child listing, so a previous error does not turn an empty listing into an error.
Both adapters retain process-table errors unless the process is gone.
I read the adapter code and the local `libproc` implementation.
I found no new source issue. Native evidence remains open below.

### R2-1 — HIGH — OPEN — The macOS adapter lacks required native mutation evidence

Plan section 8 requires a focused Mac mutation run for macOS-only code excluded from the Linux gate.
Its log must appear in READY and the PR.
This head changes `crates/botster-test-process/src/platform/macos.rs`, including `live_children` and the shared `live_of` decision.
The supplied manual Linux run lists ten macOS mutants and misses all ten because Linux does not compile that adapter.
Those results do not test the adapter's decisions.

The earlier Mac job at `c99903c4` expired at its 2,700-second deadline without running.
Its log reports a Mac disk restriction and exit 124 after 2,701 seconds on no node:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p5-real-controls-a-c99903c4-pool-20261010-095820-95517.log`.
Neither condition supplies the required native evidence.
The lead's live handoff also requires #222 to wait for the focused Mac run.
Provide a focused Mac mutation log that compiles these decisions and proves their behavior.
The evidence must match the reviewed adapter source and name its exact commit and command.
This is an acceptance-evidence finding. I found no separate macOS source defect.

### Exact-head Linux evidence

Full gate:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p5-real-controls-a-a6aa1956-pool-20261010-104732-94598.log`.

The header identifies the reviewed head and base. All ten stages pass.
Default tests: 1,488 passed, 497 skipped. Slow tests: 392 passed, 1,933 skipped.
The slow suite takes 83.139 seconds.
Testkit conformance remains 193 passes.
Pending-real remains 81 ids with zero passes.
Minimum counts are testkit 50/69, real-passing 31/68, and real-accepted 31/69.
Both mutation stages report 60 mutants: 52 caught, eight unviable, zero missed, and zero timeouts.
The full command takes 362.4 seconds. The repeated mutation command takes 191.9 seconds.
The job exits zero after 580 seconds, including one queued second.
These mutation commands do not enable the slow feature.

Manual slow mutation run:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p5-real-controls-a-a6aa1956-pool-20261010-105721-24883.log`.

The command selects the PR diff and enables `slow` with `--no-config`, `--in-place`, and `--max-fail 1:immediate`.
It uses the slow profile. All 415 baseline tests pass.
I parsed all 92 mutant outcomes and checked their retained log sections.
All 73 catches build successfully, end with test exit 100, and retain named test failures.
All nine unviable mutants retain compile errors and build exit 101.
The unread-report proof supplies six named failures in the retained mutant logs.
All ten misses are in `platform/macos.rs`; there are no timeouts.
The command exits two after 340 seconds because of those misses.
This run supports the Linux slow-tier decisions. It does not close R2-1.

The pending-real delta remains the same six removals recorded in Round 1, with no additions.
I compared the sets against the new base and found all six exact-head real-tier passes.
The tap's byte forwarding, identity checks, corruption control, worker-only signal control, and plain-harness refusals remain sound.
The head contains the gate base. `git diff --check` passes.

The package reviewer independently closes RC-A-F1 from source and the unread-report proof.
The package reviewer also identifies the missing Mac evidence as RC-A-F2 HIGH.
I read package verdict `3b915cc744efea79ee92bb35cda239c1140867ba`, `verdicts/p5-real-controls-a.md` on `stage1/review-p5`.
I assign the same HIGH severity because required native acceptance evidence is absent.
My earlier message assigned MEDIUM; this committed record replaces that preliminary severity.
I changed no product code and ran no builds, tests, gates, mutation tests, or reversal jobs.

VERDICT: NOT CLEAN (1 HIGH open).
