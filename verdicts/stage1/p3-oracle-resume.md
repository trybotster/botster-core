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
