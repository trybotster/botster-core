# Integration review: #197 (the worker reports `Observed{Output}` with `model_rev`; branch stage1/p3-output-activity)

Reviewer: integration reviewer (Opus), `sess-1791517267-0168-401cc27284f329b42ec84da0de9d91e0`. This reviewer runs no
build, test or gate.

## Round 1 — CLEAN on head 3c243a04

Reviewed head: `3c243a044e762cbdf5a93d4858ea9298b16e09be`, five commits on v1 `1dd1657a`, with the merge `2b47fc7d` of v1
`c06f5b98` (the current v1; the merge has the tree of `git merge-tree --write-tree` of its parents). 5 files, +150 -11.
P3's gate log `gates/botster-core-stage1-p3-output-activity-3c243a04-pool-20261009-114214-21195.log` names this head and
base `c06f5b98`. Results: 998 default and 243 slow tests passed; conformance passed 33, failed 0; the mutants job had 13
mutants (12 caught, 0 missed, 0 timeout, 1 unviable); exit 0. This reviewer read its header and summaries.

The stated tier HIGH is correct. `botster-worker-core/src/worker.rs` is on #184's list (`ci/high-tier-paths.txt:44` at
`57380b2b`), and the worker's reports are in the PTY and process-control area (rule 5).

- **The worker.** `Input::PtyOutput` now calls `on_output`. A read that is not empty advances `model_rev` (wrapping).
  When the link is `Ready`, the read sends `Observed{Output{model_rev}}`, or it marks `output_unsent` while the last
  `Output` report is not written (`written_total < output_sent_to`).
  - The waiting report is sent at `LinkWritten`, when the last one is written, or before any other report (`report`), so
    the reports keep the order of the events.
  - `Launched` carries the same `model_rev`.
  - Before `Ready`, a read advances `model_rev` and sends nothing. The state in `Launched` carries the revision.
- **Cross-package: the host.** `botster-core-host/src/inbound.rs:423` already maps `Observation::Output` (no change
  here). The testkit and the conformance run see the new reports, and the gate passes with them.
- **The pending list.** The change only removes `conf::tm_3_due_deadline_fires_in_pump` (minimum) and
  `conf::tm_3_wake_handle_ignores_deadlines`. The replacement map gives both `core-testkit+edge` (clock), not `slow:*`.
  So by plan 23a, a TestkitHarness pass is enough. The conformance count rises to 33 passed.
- **The test helpers (botster-worker tests).** `session.rs` `Link::report` skips `Output` observations through the
  existing `frame()`. `driver_edges.rs` adds a reader of the link, so the staged close of `Remove` is not held by
  unread bytes.

Observations (not counted; package scope):
- `report` sends a waiting `Output` before another report even when the last `Output` is not yet written. So two `Output`
  reports can wait for a short time: one for each other report. The bound "at most one" holds only between other
  reports. The doc of `output_sent_to` can say so.
- The `driver_edges.rs` reader is a detached thread with a blocking `read` on a clone of `h.peer`. `set_nonblocking(false)`
  on the clone also makes `h.peer` blocking, because they share one open file description. The test only writes to
  `h.peer` after that, so the change does no harm here. The thread ends at end of file or with the test process.

VERDICT: CLEAN (0 open) at 3c243a044e762cbdf5a93d4858ea9298b16e09be

## Round 2 — CLEAN on head cdf0f4d2 (delta: one doc comment)

Reviewed head: `cdf0f4d20e39069dd75747d523cdb36ca6b29c06`, one commit on `3c243a04`. It changes only the doc comment of
`output_sent_to` (`worker.rs`, +3 -2): reads alone queue at most one `Output` report, and the bound is one plus one for each
other report. This matches the code and closes observation (1) of round 1. No code changes. P3's gate on this head was
running when this verdict was written. By the Merge bullet, the merge needs that gate green on this exact head over the
current v1 tip.

VERDICT: CLEAN (0 open) at cdf0f4d20e39069dd75747d523cdb36ca6b29c06
