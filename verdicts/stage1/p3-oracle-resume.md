# P3 oracle_resume integration review

## Round 1 — PR #200 — 2026-10-09

Reviewed head: `61ab4501df234c3da74e6e2c24c1de3b12db2c8e`.
Base and supplied gate base: `a6555ebaf221042ca7b777ca2f2425e4a63dd960`.
PR: https://github.com/trybotster/botster-core/pull/200.

HIGH is correct under BUILD.md rules 3 and 5.
The change adds a shared testkit control and a public getter in the worker core.
The reviewer checked the complete nine-file change, the existing snapshot helper, simulation dispatch, capture path, and supplied evidence.
The reviewer applied orchestrate-delivery's premise check to the control's producer, comparison target, and observable result.
The reviewer changed no product code and ran no tests, builds, mutants, or gates.

### R1-1 — HIGH — The resume comparison does not observe the current session model

Location: `crates/botster-core-testkit/src/resume_controls.rs:143-154`, `oracle_resume`.
Producer: `crates/botster-core-testkit/src/worker.rs`, `WorkerEdges::take` and `WorkerEdges::ready`.

The control reads the host's actual capture pages and their revision.
It then constructs its comparison target with `replay(&size, &log.output)`.
That target is another terminal fed from the recorded program output.
It is not an observation of the worker's current terminal state.

The edge records each PTY read before returning `Input::PtyOutput` to the worker.
Recording those bytes does not establish that the worker applied them.
The getter records revision labels for finding the capture cut, but it supplies no current model state.
The control never compares the current worker state with either reconstructed terminal.

A concrete false-positive path is:

1. The worker applies a prefix and produces a valid capture at revision R.
2. The program produces a state-changing suffix.
3. The edge records that suffix, but a worker defect prevents the model from applying it.
4. The control restores the valid capture and applies the suffix to a fresh terminal.
5. The control replays the full prefix and suffix into its comparison terminal.
6. Those two terminals agree, so the control returns `equal: true` while the live worker remains at the prefix state.

The unchanged revision does not reject this case: the recorded capture revision R still has its valid cut.
The selected transcripts call this control after the suffix and accept its `equal` result.
The current negative proof changes the saved capture bytes. It does not test divergence of the live subject after a valid capture.

The pinned controls document requires comparison with the session's model for ST-6b.
The existing `snapshot_controls::oracle_resume` helper also accepts the model to compare; the new adapter supplies the wrong subject.
A replay can serve as an independent expected model, but it cannot establish the current worker state without observing that state.
The three passing transcripts therefore do not yet establish their claimed resume invariant.

Required correction:

- Compare the restored capture plus its suffix with an observation of the live session model.
- Keep the original capture bytes from Core's actual pages.
- Preserve the exact consumed-output boundary for the suffix.
- Add a negative proof with an unchanged valid capture and suffix but a divergent live subject.
- Require that proof to report `equal: false`.
- Supply the completed gate and package verdict on the replacement head.

P3 accepted this source finding and confirmed the false-positive path.
P3 supplied the lead's earlier ruling: choice (a) authorized only the read-only `model_rev` getter.
That ruling did not approve replacing the current session model with a replay.
P3 has asked the lead to approve a live-model observation interface and owns that question.
The integration reviewer did not duplicate the escalation or prescribe an unauthorized public API change.
This is source reasoning; the reviewer did not execute a defective-worker fixture.

### Other integration checks

`Worker::model_rev` returns the worker's existing contract revision and changes no production behavior.
Its new proof compares the getter with a public read result and checks output that does and does not complete a model step.
The PR records the API change. The current public-api job covers the facade, so no worker-core baseline is claimed.

The control registers through the existing registry.
`TestkitCore::poll_events` obtains capture bytes through the host's `read_page` calls before returning the completion to the caller.
The capture log belongs to the opened handle, and the worker log belongs to the process identity in the run.
The new `model_log` lookup takes `run_processes` before the process cell, matching the existing lock order.
The simulation's readiness path holds the simulation lock before the process cell.
The capture lookup releases its own lock before looking up the worker log.
No new reverse lock order was found.

The correction requires no change to the existing host capture protocol, worker launch, or process ownership rules at this head.
This PR adds no real-process fixture, unbounded process wait, mutation exclusion, dependency, contracts pin, or alternate production machine.
`oracle_restore`, `oracle_graphics`, and `oracle_resume_every_cut` remain outside this PR's assigned scope.
R1-1 concerns the correctness of the new control's comparison, not those later controls.

### Accounting and supplied evidence

The pending diff removes exactly three ids and adds none:

- `conf::st_6b_model_after_baseline_plus_output_equals_the_sessions_model`.
- `conf::st_6b_cut_inside_an_sgr_sequence_applies_the_suffix_not_prints_it`.
- `conf::st_6b_saved_cursor_tab_stops_margins_rendition_and_charsets_survive_a_cut`.

All three map to `core-testkit` at contracts commit `03891658`.
The minimum list changes from 30/70 to 31/70 through the first id.
All three have PASS records in the supplied gate, but acceptance of their removal waits for R1-1's correction.
No new real-harness progress is claimed.

The reviewer compared `evidence/p3-pr-b/probe-pr-b.out` and `evidence/p3-pr-c/probe-pr-c.out` under `shared/core-stage1`.
The saved probes report 132 and 135 passes respectively.
Exactly the three removed ids are new passes, and no prior pass is lost.
The previous probe reports `UNSUPPORTED_CONTROL` for `oracle_resume` on all three ids.
The exact-head gate reports 91 selected ids passed and zero failed.

Full log: `~/botster-sessions/gates/botster-core-stage1-p3-oracle-resume-61ab4501-pool-20261009-145718-80866.log`.
The log names the exact head and base and runs on Linux msa1, allocation `0a32d96c`.
The fetched v1 tip equals the supplied base and is an ancestor of the head.
All ten CI jobs pass. The default tier passes 1112 tests; the slow tier passes 249 tests.
Both mutation runs report 35 mutants: 23 caught, twelve unviable, zero missed, and zero timeouts.
The separate mutation run uses `NEXTEST_PROFILE=slow`. The gate exits 0 after 279 seconds.
The control tests and getter proof have PASS records. The source diff has no whitespace error.
These results do not prove that the control detects a divergent live subject.

The package review is pending at the time of this verdict. No package CLEAN is assumed.
One integration finding remains open. Wait for the replacement READY and its completed evidence.

VERDICT: NOT CLEAN (1 open)

## Round 2 — PR #200 — 2026-10-09

Reviewed head: `63b3b280fb1b21625f18297a15bff41d66a3f65e`.
Base and supplied gate base: `a6555ebaf221042ca7b777ca2f2425e4a63dd960`.
Previous reviewed head: `61ab4501df234c3da74e6e2c24c1de3b12db2c8e`.
HIGH remains correct under rules 3 and 5.
The reviewer read the complete six-file correction, its callers, tests, updated PR description, and supplied gate.
The unchanged source and pending-map assessment retain the round 1 review.
The reviewer changed no product code and ran no tests, builds, mutants, or gates.

### R1-1 / F70 — CLOSED

The lead approved the live-model observation interface in ruling (1), message `msg_plugin-w_1791583451_c908fe`.
P3 records that ruling in its shared handoff. The ruling keeps replay only to find the suffix.
The PR description names both public getters and their scopes.

`Worker::model_snapshot` reads the current `model.term.snapshot()` through `&self`.
It uses the same encoder as `CaptureSnapshot`, without the capture size bound.
It returns `None` before the model exists. It changes no worker state or protocol action.
The getter test compares the result with an independent terminal and the actual capture page.

The testkit stores the same worker machine in the simulation and process cell through `SharedWorker`.
The wrapper delegates each machine call to that worker. It creates no second worker or comparison model.
Each machine lock ends before the simulation performs an edge action.
Snapshot lookup clones the shared worker and releases the process and cell locks before it locks the machine.
The readiness path also releases the machine lock before it records the revision in the cell.
No new reverse lock order was found.

`oracle_resume` retains Core's actual capture pages from `TestkitCore::poll_events`.
It retains the revision cut and computes the exact consumed suffix with the existing step rule.
It restores the capture, applies that suffix, and compares the resulting snapshot with the live worker snapshot.
An unknown capture, unknown revision, or absent model returns an error instead of an equality verdict.

Two new negative tests exercise the former false positive:

- `a_live_model_that_diverged_is_not_equal` keeps the capture and logged suffix unchanged, then changes the live model.
  The control returns `equal: false` after previously returning true.
- `a_worker_that_stopped_stepping_is_not_equal` appends output to the edge log without applying it to the live model.
  The control returns `equal: false` with the original valid capture.

Both tests have PASS records in the exact-head gate.
The positive partial-sequence and distinct-cut tests remain selected and pass.
P3 reports that the new negative tests fail against the prior control.
The reviewer establishes closure from the source, test bodies, and passing replacement gate; the reviewer did not execute that baseline experiment.

### Accounting and evidence

The pending list does not change in this correction.
All three ST-6b ids listed in round 1 pass in the new gate and retain their `core-testkit` replacement-map assignments.
R1-1 no longer blocks those removals. The change raises the testkit minimum from 30/70 to 31/70 against this base.
It establishes no additional real-harness progress.
The other oracle controls remain outside this PR's scope.

The fetched branch and PR description name the reviewed head. The head contains the stated base.
`git diff --check` reports no error.
The correction changes no dependency, contracts pin, transcript, mutation exclusion, or real-process fixture.

Full gate:
`~/botster-sessions/gates/botster-core-stage1-p3-oracle-resume-63b3b280-pool-20261009-151955-30889.log`.
It names the exact head and base. It ran on Linux node `msa1`, allocation `9438fb70`.
All ten CI checks passed. The default tier passed 1115 tests. The slow tier passed 249 tests.
Conformance reports 91 passed and zero failed. The three removed ids have individual PASS records.
Both mutation runs report 49 mutants: 36 caught, 13 unviable, zero missed, and zero timeouts.
The separate mutation run uses `NEXTEST_PROFILE=slow`.
The gate exited zero after 332 seconds.

The reviewer read package round 130 at `ccbe1c8c623cf7734b1c9360447f38d89fc0aa01`, file `verdicts/p3-worker.md`.
That verdict is CLEAN on this exact head and independently closes F70.
No integration or package finding remains for this PR.

VERDICT: CLEAN (0 open) at 63b3b280fb1b21625f18297a15bff41d66a3f65e
