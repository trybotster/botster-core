# Contracts #66 (`rc/r86-route-progress`): integration review (R-86, R-81, R-89; RC)

Reviewer: contracts integration reviewer, session `sess-1791740105-017f-b6f22445c954bd9e60e1ae695593410e` (in place of Sol for contracts PRs).
Implementer: RC (`sess-1791701696-017d-d5022b29376b8c0aaeecc0885355c8ba`). Package reviewer: 016c. Ruling: R-86 (pending; it governs now). Tier: HIGH (steward).

## R-86 checks for the READY head (steward, 2026-10-11)

### The mechanism

- `RouteClient::progress()` moves the client bytes that wait on each live route into the stream, as room appears.
- On the testkit, `progress()` also runs the workers' ready route work.
- The driver calls `progress()` for every live route in every wait loop.

### The limits

- `progress()` moves only bytes that the transcript already sent, and only as far as the stream takes them.
- It reads no Core event, and it polls nothing.
- It reads no route frames for the transcript.

### Evidence required

- The A5-4 table on both Core tiers, with no PASS to non-PASS.
- `dp_5_full_input_queue` passes in R-81's form.
- A driver self-test that fails when `progress()` is removed.

## R-89 checks for #66 (steward FYI, pending; it governs now)

- A step deadline bounds every driver wait.
- `next_event` checks `deadline.expired()` after any queued event, and before the `Pending` return for `more` or `posted`. When the deadline has expired, it returns `TimedOut` (INCONCLUSIVE `step_timeout`).
- Self-tests are required with a Core or a client that always answers `more`. One covers `pump_until`, and one covers the expect-all/collect loop. Each must end INCONCLUSIVE `step_timeout`, and each must be red without the check.
- Owner: RC, in #66.

## Round 1: NOT CLEAN (2 LOW) on 6c22bc91e796bed9a44362bcd48f3e2edebc7e94

The branch is based on main `7c4004a0ba6c9734fbe8d2072f68addde2d58596`. Own commits: `c06d67a` (R-81), `13eae88` (R-86), `c075b04` (unit tests) and `6c22bc9` (R-89). Merges `5dd6715` and `5305007` are automatic: the recomputed trees `65d8d66c` and `316dff6c` equal the commit trees.

Gate: `~/botster-sessions/gates/botster-contracts-rc-r86-route-progress-6c22bc91-pool-20261011-151341-34266.log` (sha256 `2771a7f54afb24c6c168b5f112dae09b21459de11104a33319cd649c0bc9a30a`). The header names head `6c22bc91...` and base `52f1b07f`. msa1. Ten steps PASS. Mutants: 15 tested, 13 caught, 2 unviable, 0 missed. Exit 0.

### R-86 checks: hold

- `RouteClient::progress()` is a required method with no default. Its doc states the three limits: only bytes already written, as far as the stream takes them; no stream read and no Core event; no poll.
- `FakeEnd::progress` is `false`, because the fake's `write` hands over every byte at once.
- Every wait loop pumps through `CoreDriver::pump`, which calls `progress(share)` first, and a moved byte sets `more`. I checked every `pump` call site in `driver.rs`: `next_event`, `real_wait`, the tap wait, `next_refusal_read`, `route_expect_refusals`, `await_title`, `await_pty`, `pump_idle`, `drain`, `collect_events`, `await_quiet`, `await_control` and `await_get`.
- `route_pass` (R-68) calls `progress` before its own pump, and it counts a moved byte as progress. The route driver checks the deadline after each read (`route/driver.rs` lines 323 and 438).
- The `pump` step uses `core_pump`, which gives Core's own report. So no step result changes, except through bytes that a real client would deliver.
- Self-tests:
  - a wait loop and a route wait, each with a 16-byte stream room and a 4096-byte send;
  - `pump` step `{more: false, events_posted: 0}` while client bytes wait;
  - lib unit tests of `progress(share)`: every end is asked, and `|` not `^`.
  - The body states red on revert for each.

### R-89 checks: hold

- `next_event` takes a queued event first. Before the `Pending` return for `more` or posted events, it checks `deadline.expired()` and returns `TimedOut`.
- Every other Core-driver loop checks the deadline on each pass, or is bounded by `poll_limit`, or by its `stuck` rule.
- The self-test covers an always-`more` Core and an always-progressing client, each through `pump_until` and `pump_collect`, with a 200 ms step deadline and a 10 s watchdog. All four cases give INCONCLUSIVE `step_timeout`. The body states red on revert (watchdog).

### R-81: holds

- `dp_5_full_input_queue_stops_reading_the_route_and_never_drops` sends ten 512 KiB frames with distinct digits (`let repeat`, no literal).
- A `program_ready` fence (index 3) comes before the input.
- Index 41: `input_blocked` `true`. Index 43: an empty PTY read while the PTY is blocked. Index 45: `await_pty` of 5,242,880 bytes, which equals 10 × 524,288. That fences the full read at index 47.
- The two exception lines of the id leave the file. The gate's corpus test of the lint passes.
- No `0d` byte is in an input-log assertion (R-85).

### Findings

- **F1 LOW (evidence: an unexplained real-tier INCONCLUSIVE).** In 12 real-tier runs of `dp_5` in the R-81 form, one is INCONCLUSIVE `step_timeout` at index 42 (`await_quiet`, seed 2; `...-dp5-real-with284-mac-20261011-140001.log`, 13.95 s, against about 10 s for a pass). The body says that it gets a rule-9 record and a diagnosis "if it recurs in the pool runs". But no pool run executed `dp_5`: the A5-4 base lacks #285, so `dp_5` is `unsupported_control` there. So the condition was never tested. The real-tier logs also do not state their seed set. Required:
  - a rule-9 record of the index-42 event, from the retained output and the source: what `await_quiet` waited for, with 5 MiB queued and the PTY blocked;
  - one real-tier run of `dp_5` on the Linux pool, on a base with #285, with the seed set stated.
  - If the diagnosis finds a cause in the driver (R-86 or R-89), it is fixed in this PR.
- **F2 LOW (the trait contract and the Core implementations differ).** The trait doc says that `progress()` is "True when it moved a byte or ran work". The body says that the Core implementations (`TestkitRoute::progress`: flush, `run_ready`, flush; `RealRoute::progress`) are "true only when a byte moved". The evidence ran with those implementations. On the testkit, a `run_ready` pass that runs ready route work but moves no client byte then gives `false`, and a wait can take the Core as idle while work ran. That breaks the trait's contract. Choose one:
  - (a) state in the body that the Core implementations return true also when `run_ready` ran work, and confirm that the evidence ran with that form;
  - (b) narrow the trait doc to "moved a byte", and show from the source why `run_ready` work without a moved byte never needs another pass.

### Notes (no finding)

- The A5-4 real-tier sides ran on different nodes (before on `linux`, after on msa1). The two ids that changed (`am_2_no_interleave_across_paste_markers` ok → FAIL, `in_9_text_is_one_transaction_and_utf8` FAIL → ok) are both listed unfenced R-56 reads, and a one-byte send moves no client byte. I accept this explanation.
- The A5-4 port has R-86 and R-81, not R-89. R-89 can turn a wait that outran its step deadline and then passed into INCONCLUSIVE. Core's `.42` pin PR gate, which runs every live id with R-89, is the check for that.

### 016c's round 1 finding on the same head (package scope; recorded, not repeated)

- **MEDIUM (R-89).** `next_event` checks the deadline only on its `more` or posted path. `idle_wait` returns `Pending` with no deadline check in two paths:
  - after `clock.jump_to(at)` (lines 666-667);
  - on `Wake::Woken` (line 680).

  So a Core that keeps giving future deadlines under a jumping clock, or a wake handle that keeps giving spurious `Woken` with no work, keeps `pump_until` and `pump_collect` alive past the step deadline. 016c's two exact-function proofs return `Pending` for an already-expired deadline.
- **My round 1 missed this.** I checked the deadline in each loop and in `next_event`'s `more` path, but not in `idle_wait`'s `Pending` returns.
- On the next head I check two things:
  - every `Pending` return of `next_event` and `idle_wait` is behind an expiry check (or the check is at the start of `next_event`, after the queued-event pop);
  - regression cases (a repeating future deadline under a jumping clock, and a spurious `Woken`) end INCONCLUSIVE `step_timeout`, and are red on revert.

## Round 2: no open finding on 2efda094b1fa2eeed5f7cb8ba4ac3a0eb952ae13 (final CLEAN after the main merge and its gate)

Delta against `6c22bc91`: `5a49fc4` (F2) and `2efda09` (016c's MEDIUM). It changes `driver.rs`, `harness.rs` and `driver_steps.rs`.

- **016c's MEDIUM closed.** One helper, `pending(deadline)`, gives `TimedOut` once the deadline has passed. Every `Pending` of `next_event` and `idle_wait` goes through it: the `more`/posted path, the clock jump (line 674), and `Wake::Woken`. The last `Pending` arm of `idle_wait` comes after its `deadline.expired()` arm. A queued event is still taken first.
  - The self-test has four sources (`core_more`, `busy_client`, `deadline_ahead`, `always_woken`), each through `pump_until` and `pump_collect`.
  - The test `Harness` has an injected clock with injected progress, so `clock.jumps` is true (driver line 550). So `deadline_ahead` takes the jump path on every pass: each `next_deadline` is one hour after the one before.
  - The body states red on revert for each of the three paths, at the watchdog.
- **F2 closed (option b).** The trait doc and the two driver comments now say "true when it moved a byte". The reason is in the doc. I checked it against Core v1 `crates/botster-core-testkit/src/worker.rs` `TestkitCore::pump`: `workers.run(now)`, then `driver.pump(now)`, then `more = true` and a wake signal while `workers.has_ready()`. The driver calls that pump right after `progress()` in the same pass, so ready work that moved no byte is still run and reported.
- **F1 closed.** The rule-9 record is `~/botster-sessions/gates/rc-r86-dp5-index42-rule9-record.md`.
  - It covers: the event (Mac, the first run after a build, seed 2 of the default seeds 0-7); the retained output, which has only the INCONCLUSIVE line; and a source analysis showing that `progress()` cannot keep `more` true while index 41 reads `blocked: true`. The event is a bounded timeout, not a hang.
  - It is classified FLAKY, cause not determined, with the next step named (a tap trace) if it recurs.
  - The Linux pool run: `~/botster-sessions/gates/botster-core-detached-5e953398-5e953398-pool-20261011-152757-22340.log` (sha256 `d58550acba05657cda36fed9a056f84fbfd7f3da3677abe0051cbdf38236d14f`). Core v1 `5e953398925351c12e463a7b0a946b79d9841edc` (#285, #284) + the R-86 impls, contracts `.39` + the port of #66 with R-89 and F2. `BOTSTER_SEEDS=0-31` is in the command. Real tier PASS (8.96 s), testkit PASS (7.64 s), msa1, exit 0.
  - The port does not include `2efda09`, which changes only `idle_wait`, and `await_quiet` does not use `idle_wait`.

Still to do:
- **The main merge is not disjoint.** Main `c0c2abc09f208fea39896972373da53cb90dd830` includes #61, which changes `crates/botster-core-conformance/src/driver.rs` (the R-85 check) and `tests/driver_steps.rs`. #66 changes the same two files. So the merge needs a delta review (Q7), not only the base-merge check. I review how the resolution keeps both R-85's check and R-86/R-89's wait-loop changes.
- One gate on the exact merged head.

## Merge delta review: no finding on 21f9406ff989da76407d78abdaaa7a6286671da4 (final CLEAN waits for a green gate)

The head `21f9406f` merges main `3c68880b011fdb60f7bca8124eef350e2d4d4708` into the reviewed head `2efda094b1fa2eeed5f7cb8ba4ac3a0eb952ae13` (reviewed base main `7c4004a0ba6c9734fbe8d2072f68addde2d58596`). Its parents are `2efda094` and `3c68880b`. RC's record is `~/botster-sessions/gates/botster-contracts-rc-r86-final-merge-delta-21f9406f.txt`.

- **The PR's own lines.** For each of the six files (`core-fence-exceptions.txt`, the `dp_5` transcript, `driver.rs`, `fake.rs`, `harness.rs`, `driver_steps.rs`), the sorted +/- lines of `7c4004a..2efda094` equal those of `3c68880b..21f9406f`. The new diff has no other file.
- **Main's lines.** For each of the 33 files that main changed since `7c4004a`, the sorted +/- lines of `7c4004a..3c68880b` equal those of `2efda094..21f9406f`. So the merge drops no line of #61, #68 or the other main changes, and `driver.rs` keeps the R-85 run-time check next to `progress(share)`, `pending(deadline)`, `core_pump` and the R-89 checks.
- **The conflict.** `git merge-tree --write-tree` gives tree `bf7f836cc5cca79bca69c423b2edd756b798086a` with one conflict, in `driver_steps.rs`. The head differs from that tree only in this file. The resolution removes the three conflict markers and puts one blank line between the two appended blocks: this PR's R-89 test block, then #61's R-85 tests. Neither block changes. The patch-id changes only because of this block order.
- 016c's delta review on the same head has no finding.

**The gate is not green.** `~/botster-sessions/gates/botster-contracts-rc-r86-route-progress-21f9406f-pool-20261011-164521-79451.log` ends with `exit 141 after 25 s on msa1`, during the nextest build. The log has no failed step. Exit 141 is SIGPIPE. This run is not evidence. The final CLEAN needs one green gate on `21f9406f`.

Correction (cause of exit 141): RC stopped that gate on purpose after 25 s (`jobq stop jobq-botster-contracts-21f9406f-20261011164521-4e28`, then the client), because the lead put the #288 `st_6` reproduction first in the pool order. The log is not a gate result, and it is not a red gate. The ONE gate on `21f9406f` runs after that job.

## Delta 21f9406f → 63f4f5c8689d49bd44ade56d62fcc0c59d80f1bb: no finding (final CLEAN waits for a green gate)

The gate on `21f9406f` was RED at mutants only: `MISSED driver.rs:679:53: replace > with < in CoreDriver::next_event` (the term `report.events_posted > 0`). All other nine steps PASS. Log: `~/botster-sessions/gates/botster-contracts-rc-r86-route-progress-21f9406f-pool-20261011-164851-10039.log`.

`63f4f5c8` is one commit on `21f9406f`. It changes only `tests/driver_steps.rs` (+11 -3), and makes no `driver.rs` change.
- The R-89 test gets a fifth source, `core_posted` (`harness.posted_every = 1`). In the test `Counting` Core, every pump after the third then reports `events_posted: 1` and `more: false`, and no poll returns the wanted event. So in `next_event` only the `events_posted > 0` term gives `pending(deadline)`. The mutant makes the wait fall to `idle_wait`.
- RC reports that the mutant fails the test with "no matching event: the Core is idle". The gate's mutants step on `63f4f5c8` must show this mutant caught.
- The doc comment names five sources. The other four cases are unchanged.

The final CLEAN needs one green gate on `63f4f5c8`, with 679:53 caught.

### 016c's MEDIUM on 63f4f5c8 (I missed it): the R-89 test goes over the 2 s budget

I confirm it. BUILD.md Testing rule 4: a default-tier test over 2 s fails CI (`.config/nextest.toml`: `slow-timeout = { period = "2s", terminate-after = 1 }`). `a_step_deadline_ends_a_wait_whose_core_or_client_always_reports_progress` runs its cases one after another, and each case waits for its 200 ms `Deadline` to expire. On `21f9406f` the gate measured 1.608 s for eight cases (about 201 ms each). With `core_posted`, ten cases take at least 2.0 s, plus setup. So the default tier fails the test.

Fix (RC's choice): a shorter step deadline with margin, or a split into two tests. `core_posted` stays, so that `driver.rs:679:53` stays caught. My earlier "no finding" on this delta is withdrawn; this finding is open.

### 4a8c917e5d52d6a116e502dec3a6359ea0ceaf4f: the budget MEDIUM is closed (final CLEAN waits for a green gate)

One commit on `63f4f5c8`, test-only (`tests/driver_steps.rs`, +41 -37). RC did not split the test. All ten cases now start on their own threads (`collect()` spawns every thread before the first `recv_timeout`), and then the test reads each result with the same 10 s watchdog. So the test takes about one step deadline, not one per case, and it does not grow when a source is added. This is better than the split that I recommended.
- The 200 ms step deadline and the 10 s watchdog are unchanged. Each `timer:` marker is directly above its call.
- Each case builds its own `Harness`. `driver_steps.rs` has no static, thread-local, environment write or shared file, so the parallel cases are independent.
- `core_posted` stays. RC reports the reversal on the committed test: `driver.rs:679` `> 0` → `< 0` fails `core_posted pump_until` ("the Core is idle"). Locally the test takes 0.20 s.

No open finding. The final CLEAN needs one green gate on `4a8c917e`, whose mutants step shows `driver.rs:679:53` caught and whose per-test time for this test is under 2 s.
