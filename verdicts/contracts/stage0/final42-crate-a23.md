# contracts #93 — final42 crate: Core A23 c2 (`conf::ou_7_tail_written_before_stop_reaches_the_model_before_exited`), reviewer 017f

HIGH. Base: `steward/final42` `8379a5695e2cfa3da8f5c38cd93c3d938ef0606f`, on 016b's A25 head `9075126660a24491624b1df33f6707b54d31e2bf` (#92, the steward's order). Implementer: P6. Package reviewer: 016c. The steward assigned the integration review to me.

## Round 1: CLEAN on 4e6f725f761b8ca74300658a1543f333cabd2ab9

One commit, `4e6f725f`, on `9075126`. Checked against A23 candidate 2 (`frozen/current/core-contract-v1.17-amendment-23-candidate2.md`, "The test"):
- **Step 1.** s1's program is `[ignore_sigterm, print OSC 2 "a23-ready" ST, hold]` (step 0). The print comes after `ignore_sigterm`, so the readiness title R is written only after `SIGTERM` is ignored.
- **Step 2.** `await_control program_ready {ready: true}` (step 4).
- **Step 3.** `await_title {s1, "a23-ready"}` (step 5). No launch or adoption lies between the print and the wait.
- **Step 4.** `pty_output` of OSC 2 "a23-tail" ST (step 6), 14 bytes. T differs from R.
- **Step 5.** `await_control pty_output_written {s1}` expects `{bytes: 14}` (step 7), the total of the transcript's `pty_output` bytes. No title read and no `await_title` for T follows before the `Stop`.
- **Step 6.** `setup.limits.stop_grace` is 1. `stop_grace` is in milliseconds (`duration_ms`), and `nonzero_time!` makes 1 the shortest value that `CoreLimits` accepts. `Stop` (step 9), `await_get` `Stopping` (step 10), then `advance_clock {ms: 1}` (step 11).
- **Step 7.**
  - `forbid_events` of s1's `Exited` and `Lost` is set before the `Stop` (step 8) and cleared only after `pump_until TitleChanged {s1, "a23-tail"}` (step 12). So an `Exited` before T's `TitleChanged` fails the test.
  - `pump_until` passes over R's earlier `TitleChanged`.
  - Step 14 binds the `Exited` cause, and step 15 checks `Killed`. So `HostStop` or any other cause fails. Step 16 checks the `Stop` completion `{end: {Exited: {cause: Killed}}}`.
  - A `Lost` fails under the forbid, or step 14 times out after the clear. So `Lost` is not expected.
- **Metadata.** `fake: structural` with ruling R-12, because FakeCore is frozen and has no `pty_output_written`. `requires.controls` lists `program_ready`, `pty_output` and `pty_output_written`. The README structural row is present. The proof is `core-testkit+edge`, edge `program-backend`, as for `ou_7_exit_after_tail_queued_then_close`.
- **The regenerated replacement map** has both final42 ids: 2550 ids; `a25_1` in the FakeCore table with `core-testkit`; `ou_7_tail…` structural; the totals move by these two ids.
- **The crate-symbols table** (`core-contract-v1.17-amendment-23-candidate2.toml`) resolves in the gate.
- **Disjoint from A25.** The commit changes no A25 line. The two PRs share only `conformance/replacement-map.json`, where each adds its own line.
- **Gate PASS on the exact head**, all ten steps, exit 0 after 516 s on msa1, base main `32d36828`, with "ledger ok: 2550 ids", clause-lint, crate-symbols and replacement-map clean: `~/botster-sessions/gates/botster-contracts-p6-a23-ou7-tail-on-a25-4e6f725f-pool-20261012-110146-72292.log`. This is the first head with all of final42, and it is fully green, as the steward requires.
- **Core:** the id stays in `core-real-pending.txt` until A31 (the A23 text). That is a Core list, not this PR.

No finding. CLEAN on `4e6f725f761b8ca74300658a1543f333cabd2ab9`.
