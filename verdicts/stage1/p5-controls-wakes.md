# Integration review: #194 (testkit seeded spurious wakes and no_spurious_wakes; branch stage1/p5-controls-wakes)

Reviewer: integration reviewer (Opus), `sess-1791517267-0168-401cc27284f329b42ec84da0de9d91e0`. This reviewer runs no
build, test or gate.

## Round 1 — CLEAN on head a0d03202

Reviewed head: `a0d032024dd19efa4f15b74f7a7ba22f76db34a8`, two commits on v1 `e8d38e09` (the current v1). The stated tier is
HIGH by rule 3 (the shared testkit), which is correct.

- **The wake.** `SimHostWake::wait` on a clear flag first draws `ChoicePoint::SpuriousWake` from the run's scheduler. A draw
  of 1 returns `Woken` and leaves the flag clear (TH-2 allows a `Woken` with no work; A5-2 seeds it). A set flag still
  returns `Woken` at once. `pick` returns 0 for this point when `Overrides::no_spurious_wakes` is set
  (`scheduler.rs:154`), so the control turns the draw off for every Core of the run.
- **Determinism.** The scheduler has one ChaCha8 stream for every choice point, so a draw on another thread could change
  the order of draws. The only cross-thread wait is the runner's `wait_wake` with `other_thread`
  (contracts `driver.rs:1072-1074`), which spawns the waiter and joins it at once. So the main thread draws nothing during
  that wait, and the order is fixed by the seed. `signal()` does not draw. `a_seed_replays_its_spurious_wakes` covers the
  single-thread case.
- **The tests** (P6's three additions are in): a real wake (`begin`) still wakes with `no_spurious_wakes`; the same seed
  gives the same waits; a bounded search finds a seed with a spurious wake; the controls refuse unknown arguments.
- **The flip.** Against v1, `core-pending.txt` loses only `conf::tm_6_create_without_worker_input_wakes`. Its
  replacement-map proof is `core-testkit+perturb`, not `slow:*`.
- **The gate log** (`controls-wakes-a0d03202.log`) names the head and base `e8d38e09`. The conformance binary reports 26
  passed and 649 ignored on seeds 0 to 31, with the new draws. The default tier runs 975 tests and the slow tier 243, all
  pass. Mutants: 3 caught, 0 missed, 2 unviable. Exit 0.

Observation (not counted): the determinism above depends on the runner joining every cross-thread wait before its next
step. A later runner step that waits on one thread while another thread pumps would make the draw order depend on timing.
Then the spurious-wake point would need its own stream.

VERDICT: CLEAN (0 open) at a0d032024dd19efa4f15b74f7a7ba22f76db34a8
