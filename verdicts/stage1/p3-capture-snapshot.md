# P3 terminal model and CaptureSnapshot integration review

## Round 1 — PR #199 — 2026-10-09

Reviewed head: `f50bf6beb486007c16e49ca13a3c5d39058da15c`.
Base and supplied gate base: `58d6663204b50ce6c42d467e4fd6715ab46145dd`.
PR: https://github.com/trybotster/botster-core/pull/199.

HIGH is correct under BUILD.md rules 3 and 5.
The change crosses the host, link, terminal binding, and worker boundaries.
It also changes the shared testkit's launch literal and the worker's input path.
This review covers the full twelve-file change and its integration effects.
The reviewer changed no product code and ran no tests, builds, mutants, or gates.

### R1-1 — HIGH — The worker discards the binding's clipboard acknowledgements

Location: `crates/botster-worker-core/src/worker/model.rs:137-227`, `Worker::after_step`.
Binding: `crates/botster-terminal-ghostty/src/events.rs`, `Drained` and `on_clipboard_write`.

The binding stores each OSC 5522 acknowledgement in `Drained::clipboard_acks`.
It keeps those entries separate from `pty_writes` and the droppable event buffer.
Its drain takes the acknowledgement entries and resets their byte count.
The binding does not write those bytes to the program.

The new worker drains the binding and reports `ClipboardWrite` to the host.
However, it queues only `drained.pty_writes`; `feed_model` separately queues a query's shadow reply.
Neither path consumes `clipboard_acks`, so the drain discards every OSC 5522 acknowledgement.
The program receives no acknowledgement even though the model accepted its clipboard write.

A13-1b requires the worker to write each acknowledgement through the admission point.
Each acknowledgement is one contiguous transaction, in write order, with no input revision or operation completion.
A13-2 also replaces EV-3 with this acknowledgement requirement.
This is an interface defect in the new model path, not a request to implement the deferred route package.
The PR body records no lead exclusion of this duty.

Required correction:

- Queue each `clipboard_acks` entry as its own admission transaction.
- Preserve acknowledgement order and the existing transaction's contiguous bytes under input pressure.
- Keep acknowledgements independent of event loss and host presence.
- Add worker proofs with expected bytes from an independent libghostty terminal.
- Check the acknowledgement path under input pressure and check that it advances no input revision or operation completion.
- Supply the completed gate on the replacement head.

The integration reviewer sent this finding directly to P3 and the package reviewer.
The package reviewer independently confirmed the source trace as F68 HIGH.
The reviewer did not execute a failing fixture.

### R1-2 / package F67 — MEDIUM — The worker inserts spaces into wide-character text

Location: `crates/botster-worker-core/src/worker/model.rs:257-268`, `Worker::read_cursor`.
Binding: `crates/botster-terminal-ghostty/src/reads.rs`, `cell_text`.

The binding returns an empty string for a wide-character spacer and a space for a cell that has no text.
The worker changes every empty cell string into a space before it builds both cursor text fields.
It therefore changes the terminal text that libghostty supplied.

For program output `a日b`, the binding's text cells start with `a`, `日`, an empty string, and `b`.
The worker inserts a space between `日` and `b` in `row_text` and `text_before_cursor`.
ST-3 and BUILD.md's libghostty ownership rule require the model's text.
The current worker read proof uses ASCII and does not exercise a wide-character spacer.

Preserve the binding's cell strings when constructing these fields.
Keep the ST-3 trailing-space trim for `row_text` and the untrimmed prefix for `text_before_cursor`.
Add a wide-character worker proof with the row and prefix derived from an independent libghostty terminal's `row_cells`.

The package reviewer found F67 and sent it to integration.
The integration reviewer confirmed the binding and worker source paths independently.
No CLEAN verdict had been issued for this head.

### Interfaces and preserved behavior

The host copies its configured `CoreLimits` into `LaunchSpec.limits`.
The link defaults an absent field to the contract defaults and tests that behavior.
The worker sets the model's clipboard limit from that field and checks its snapshot size against `max_snapshot_bytes`.
The host retains its own capture-size check, capture identity, owner, pages, and expiry.

The worker obtains snapshot bytes and the advertised format from the terminal binding.
It sends one owned page before the capture completion and reports the model revision of that snapshot.
The host accepts those pages through its existing request mapping and mints the capture identifier.
The new tests compare snapshot bytes with an independent terminal and check the exact size boundary.

The model exists before payload launch. Output received during spawn waits until after `Launched`.
The worker retains output-report coalescing and feeds the same model in both drivers.
The testkit adds no control or alternate terminal implementation.
The host passes clipboard contents, total bytes, and reason through without a second size decision.
The binding establishes `contents: None` exactly for `too_large`; the worker preserves that result for known locations.
The body leaves its unknown-location question with the lead; this review does not invent a selection letter.

The worker adds the existing terminal binding as a dependency. No dependency version, pin, or workspace setting changes.
The real-process fixture changes only its `LaunchSpec` literal.
The #198 parent-death and real-PTY proofs remain in the supplied slow run.
No new real-process helper, wait, guard, or cleanup path is added.
The package reviewer confirms that old Part B F39 concerns a different guard/reaping change and does not apply to this port.
That old duty is neither closed nor newly introduced by #199.

### Conformance accounting and supplied evidence

The pending diff removes exactly 42 ids and adds none.
The pinned replacement map assigns 31 to `core-testkit`, nine to `core-testkit+edge`, and two to `core-testkit+perturb`.
None requires a real-only proof for its removal under plan 23a.
Every removed id has a PASS record in the supplied exact-head gate.

The saved baseline is `shared/core-stage1/evidence/p3-pr-b/baseline-v1-58d66632.out` under `~/botster-sessions/`.
It reports 47 passed and 41 failed.
Its 41 failing ids equal the removed ids except `conf::am_4_capacity_returns_on_poll`, which already passes on the base.
The PR body records the lead's authorization to remove that AM-4 id with this change.
The exact-head gate reports 88 passed and zero failed.
The minimum list confirms the claimed change from 28/70 to 30/70 testkit-passing ids.
The two minimum removals are `conf::or_1_no_progress_outside_pump` and `conf::tm_3_next_deadline_is_host_armed`.
These results do not claim a new real-harness pass.

Full log: `~/botster-sessions/gates/botster-core-stage1-p3-capture-snapshot-f50bf6be-pool-20261009-135627-42103.log`.
The log names the exact head and base and runs on Linux msa1, allocation `691c70b0`.
The fetched v1 tip equals the supplied base and is an ancestor of the head.
All ten CI jobs pass. The default tier passes 1100 tests; the slow tier passes 249 tests.
Both mutation runs report 66 mutants: 58 caught, eight unviable, zero missed, and zero timeouts.
The separate mutation run uses `NEXTEST_PROFILE=slow`.
The gate exits 0 after 337 seconds.
The source diff has no whitespace error.

The passing gate does not cover R1-1 or the wide-character path in R1-2.
The package review is still being recorded; its messages confirm F67 and F68 on this exact head.
No package CLEAN is assumed.
Both findings remain open. Wait for the replacement READY head and its completed gate.

VERDICT: NOT CLEAN (2 open)
