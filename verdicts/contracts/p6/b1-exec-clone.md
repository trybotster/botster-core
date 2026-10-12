# contracts #88 — p6/b1-exec-clone (backlog B-1: copy the bindings before a step only for a `pty_input` read), reviewer 017f

HIGH (BUILD.md "Risk tiers" rule 1): `botster-core-conformance/src/driver.rs` decides pass or fail for every Core run. Package reviewer: 016c.

## Round 1: CLEAN on 7b753249c695730d740a335669d5fbde4ebe1f03

One commit on main `b6c7ea9d6abd37d723ecf491c618013eb36d9da7`, in `driver.rs` (+8 −6).

- **The change keeps the behavior.**
  - `CoreDriver::exec` copies the bindings before the step only when `pty_input_read(step)` is `Some`, and `input_log_track` takes `Option<&Bindings>`.
  - In `input_log_track`, the only reader of `before` is the `changed` closure. It ran only under `if pty_input_read(step).is_some()`, which is now `if let Some(before) = before`, with the same predicate on the same step.
  - The `retain` pass and the `let` pass read only `bindings`, so they do not change.
  - `input_log_track` has one caller (`driver.rs:1268`).
- **Evidence** (`shared/core-stage1/evidence/p6-b1/`):
  - The in_9 profile's copy cost falls from 274 to 40 samples of about 3000.
  - On Linux (strict, seeds 0-31, both tiers), the testkit outcomes are identical: 822 lines, and the sorted diff is empty.
  - The real tier has the same totals (216 passed, 1 failed). Its changed lines are step timeouts or `core-real-pending` ids with a recorded race, and the steward accepted that classification.
- **Main.** Main is now `47936a983f2639259e2bc6478a6b10b0ac446bbf`. `git diff b6c7ea9d 47936a98` changes only `docs/steward-rulings.md`, a rulings-only move. The gate base `b47533be` is on that line, and `b6c7ea9d..b47533be` is also rulings-only.
- **Gate PASS on the exact head**, all ten steps, exit 0 after 726 s on linux; mutants 3 tested, 3 caught: `~/botster-sessions/gates/botster-contracts-p6-b1-exec-clone-7b753249-pool-20261011-225040-47780.log`.
- **The body** states the HIGH tier, the prior art (old Core `72b2e335` has no transcript interpreter) and the evidence.

No finding. CLEAN on `7b753249c695730d740a335669d5fbde4ebe1f03`.
