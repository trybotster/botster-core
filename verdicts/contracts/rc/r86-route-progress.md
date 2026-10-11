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
