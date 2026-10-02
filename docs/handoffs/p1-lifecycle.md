# Handoff: Core Stage 1 package P1 (session registry and lifecycle)

Written at the pause ordered by the lead. A new agent can resume from this file.

## Where everything is

- Repository: `trybotster/botster-core`. Worktree: `~/botster-sessions/trybotster-botster-core-stage1-p1-lifecycle`.
- Branch `stage1/p1-lifecycle`, pushed head `ff376cceea6f68db75ca76ebbe1a574ea3ece5fd`. It is based on `origin/v1` `67124ec`
  (the merge of P6's testkit PR #129). It has 30 or so commits. Nothing is merged. Do not force-push; push normally.
  A backup of an older state is in the local branch `backup-pre-rebase-r6`.
- Branch `stage1/p1-testkit-wiring`, pushed head `95a58545a9d5369098ed0074ee150d1b6ca16491`. It is P1's whole branch (rebased on
  `67124ec`) with the testkit wiring still in it. The wiring is `crates/botster-core-testkit/src/core.rs` (SimEdges, TestkitCore,
  Directories), `harness.rs`, `lib.rs` and `Cargo.toml`. By the lead's decision this wiring is NOT in the P1 PR. It lands in a
  stacked PR together with P3's worker. P3 (session `sess-1790940814-00b7-10ebbedc6ec6f9fe6ea13a273ee6925d`) builds that stack:
  base `stage1/p1-lifecycle`, my wiring commit unchanged, plus its own commit that restores P6's v1 refusal plumbing
  (`refusals`, `with_refusals`, `fail_next`, `check_crates` statement, `injects_clock` true, v1 unit tests; P6 asked for this;
  `Faults` stays an open struct and the host scheduler handle is exposed for `set_overrides`). That stacked PR needs my reviewer's
  CLEAN and the integration reviewer's CLEAN (`sess-1790904158-0094-cc72b6fd9b7ca209c7ce6391291f9dbd`). P6 owns the testkit
  crate (`sess-1790912515-00a4-1cceee51efc7648a00f107f10856129f`).
- Contracts pin: tag `contracts-v0.1.9` (`7f72acf`, manifest final22). Ledger 643 ids, 639 pending, 2 deferred, 2 withdrawn
  (the A9-2 ids). `cargo xtask lists` passes.
- People: lead `sess-1790903471-008f-8b9f78eef51d48aba5a45748495fd673`; reviewer
  `sess-1790919735-00ab-587e599e62295456963b7cdd0c7b574e` (its verdicts are in branch `stage1/review-p1`,
  file `verdicts/p1-lifecycle.md`, worktree `~/botster-sessions/trybotster-botster-core-stage1-review-p1`).
- Rules that hold (see also `~/.claude/CLAUDE.md` and the memory index): heavy cargo through
  `botsterq run --label "core ..." --deadline ... -- env -u RUSTUP_TOOLCHAIN CARGO_BUILD_JOBS=4 NEXTEST_TEST_THREADS=4 cargo ...`;
  one heavy job at a time; never kill by name; never change a timeout or deadline value; no `git branch -D`; never force-push now;
  report to the lead only QUESTION, BLOCKED or DONE; `post_message` takes `{session_uuid, payload}`.
  The gate: `~/botster-sessions/shared/tools/botster-gate --deadline 120m <worktree>` (runs on the Linux test host, on the exact
  committed head, about 50 minutes with the mutants step; commit first; exit 65 = uncommitted tracked changes).

## What P1 contains (crates)

- `crates/botster-core-host`: the sans-IO `HostEngine` (admission table, op table, one event queue with classes M/K/D/L,
  deadlines, flows Create/Start/Stop/Remove, StopAll, captures, routes, adoption shell) and `HostDriver<E: HostEdges>`
  (pump budgets, link framing, held frames, scheduler choice points). `DESIGN.md` there records every choice where the contract is
  silent, "Round 2" and "Round 3 decisions". Tests are in `src/tests/*.rs` (about 206).
- `crates/botster-core-link`: the private control-link wire (frames, hello, messages, launch args, token proof). Its `lib.rs`
  header documents the worker-control signal for P3.
- `crates/botster-core-sys`: real edges: `FileStorage`/`DataDir` (atomic write, fsync, flock, epoch), `OsEntropy`, `Children`
  (process spawn with reaper threads that wake the host at a worker exit, identity by pid and start time).
- `crates/botster-core`: the facade `Core::open` and `impl CoreApi for Core`; `src/real.rs` has `RealEdges` and `PollWake` (mio).
  `terminfo_source` is a marked placeholder until P2's binding is on v1 (TI-1, finding F14, scope-deferred; the a2_8 ids stay
  pending).
- `.cargo/mutants.toml`: the `exclude_re` list. Every entry has its reason and names the proof (a slow-tier test, or the reason that
  the mutant is equivalent or unreachable). The reviewer judges each entry.

## Decisions to keep (do not rediscover)

- R-15: `Remove` under a full queue closes routes first, then steps 2 to 5 (LC-7).
- R-16: the Stopping row of a `StopAll` target is best effort; a failed write does not stop the stop. A plain `Stop` keeps
  `RegistryFailed`.
- R-19: every synchronous `attach` refusal returns `AttachRefused{error, transport}` and hands the caller's transport back.
- R-20: ops with a fixed "next pump" timing are never deferred by the scheduler: LC-5 (`Stop` of an ended payload), SZ-2 (`Resize`
  to the current size), the A2-1 `Resize` row in `Created`. Implemented as `PendingOp.fixed_timing` and
  `HostEngine::never_deferred`. Also: a Stop flow whose end an edge reported is not deferred.
- Erratum 3 (E3-1): an effect with no event runs in its due pump whatever the event budget (`Work::Deadline`); `Silent` is
  `Work::Silent`, needs budget, is carried first in the next pump; parked steps are not in `ready()`.
- AM-2: only the first not-yet-forwarded `WriteInput` of a session is offered by `ready()` (begin order).
- Worker observations that arrive while the start flow is not through are held and applied one per step after `Running` and the
  completion of `Start` (`Session.held_obs`).
- F7 (AD-6): the host never signals a bare payload group and never sends SIGKILL to the worker on the broken-link stop path. It
  sends `GroupSignal::EndPayload` (SIGUSR1, to the verified worker process only, by pid and start time) at the stop and again at
  `stop_grace`. Meaning: end your payload (graceful request, then group kill after `stop_grace`, from the leader you hold unreaped,
  `waitid` with `WNOWAIT`), keep running, keep serving the final model. Idempotent. After the link returns or at adoption the worker
  reports the payload exit as usual. P3 implements the handler.
- One event per step (9B `pump_events`): `complete()` defers to its own step if the input already posted; metadata and detach are
  split; failed handoffs are queued in the driver and fed one per step.
- The pin moves: contracts-v0.1.6 -> v0.1.8 -> v0.1.9 were each their own commit.

## Review state (reviewer verdicts in `stage1/review-p1`)

- Last verdict: CLEAN at `0ac2b84`, then CLEAN at `2016886` (delta). Since then the branch changed a lot (rebase onto v1, pin moves,
  R-20, reaper threads, AM-2, observation order, mutation tests, testkit split, storage tests to slow tier). No finding is open in
  the reviewer's last list (F1 to F20 closed; F14 is an authorized scope deferral). The reviewer must re-review the delta from
  `2016886` to the final head before any DONE; the lead said any commit after CLEAN needs a delta review.
- Findings that came later from P3 (all fixed and confirmed on 32 seeds by P3): AM-2 write order; end of payload reported by an edge
  must not be deferred (in_7); observations before Running (in_8, in_9 family); Remove waiting for `stop_grace` (reaper threads).
- Open question for later: `am_2_no_interleave_across_paste_markers` needs the route plane (P4a), not P1.

## Mutation and gate status

- First full gate with mutants: 1161 mutants, 388 missed. After the work: 1018 mutants, 863 caught, 41 missed (at `19788e0`). All 41
  were then given a killing test or a named `exclude_re` entry (equivalent or unreachable mutants). The mutants step has NOT been seen
  to pass yet, because the later gates failed before it ran or had a baseline problem (below).
- Gate failures, in order, and why:
  - Gates before `ad46408` failed on budgets of real-I/O tests in `botster-core` (opening a real Core takes about 1.1 s of process
    start plus I/O on the Linux host): fixed by moving those tests to the slow tier (`slow_tests` modules in `real.rs` and `lib.rs`).
  - `21adfb2` failed in the slow tier on `a_worker_that_exits_before_it_connects_ends_the_start_at_once` (took 6.7 s: the disk of the
    host is slow, three fsyncs in a start): the test now uses a 120 s startup deadline (no timeout value of the code was changed).
  - `ad46408` failed twice on host disk speed: the existing `botster-core-sys` storage tests (real fsync, normally 0.01 s) took
    2 to 3 s (once as the 2 s test budget, once as a nextest TIMEOUT in the cargo-mutants baseline, exit 4).
  - Lead's instruction: real fsync tests belong in the slow tier. Done at `ff376cc`: the `botster-core-sys` storage tests that write
    to a real disk are in `slow_tests` (feature `slow`); the pure tests stay in the default tier; `.cargo/mutants.toml` has two
    entries for the file-backed storage code that name the slow tests. A gate on `ff376cc` was started before the pause; its log is
    `~/botster-sessions/gates/botster-core-stage1-p1-lifecycle-ff376cce-linux-20261002-090058-59146.log`. Read it first: if the
    mutants step passes, the remaining step is fuzz; if the host disk is still the cause, tell the lead (he asks the infra engineer
    for a host check).
  - The earlier mutants-step log with the full missed list (41) is `...-19788e06-linux-20261002-074946-42723.log`.
- Local test commands that pass at `ff376cc`: `cargo nextest run -p botster-core-host -p botster-core-sys -p botster-core-link
  -p botster-core` (default tier), and with `--features botster-core-sys/slow,botster-core/slow` (slow tier); clippy
  `-D warnings` on `--workspace --all-targets` with both slow features is clean.

## Ids

- Owned ids: 124 (docs/stage1-clauses/p1-lifecycle.txt in the contracts repo). All stay in `conformance/core-pending.txt` until
  both harnesses pass them; no id has been removed from the pending list. P3 reports on the testkit (with its worker): 55 of its ids
  pass, including `in_7_lost_worker_ends_pending`, `am_2_next_write_waits_for_completion_or_partial_end`,
  `in_5_lane_bounds_refuse_at_begin`, `in_8_*`, `in_9_*`. The seven `e3_1_*` ids pass in P1's tests but are named after the
  contracts transcripts; keep them pending until the proof runs through a harness. Ids that can only pass through the testkit
  wiring stay pending with the reason "testkit wiring with P3". The a2_8 ids stay pending (F14).
- 23 ids passed on the old testkit at seeds 0 to 1 earlier (listed in my earlier report); re-check after P3's stack lands.

## What is left (in order)

1. Read the `ff376cc` gate log. Fix what it shows. Re-run the gate once on the exact final head (commit first).
2. Ask the reviewer for a delta review from `2016886` to the final head (send the exact SHA; every finding including LOW must
   close). Then run the gate once on the exact CLEAN head and send the lead DONE with the head, the verdict commit and the gate log.
3. Public API snapshot `api/botster-core.txt` must match (`cargo xtask public-api`, update with `--update` and commit if it changes).
4. The stacked testkit PR with P3 (see above); the integration reviewer also reviews it.
5. Follow-ups outside this PR: TI-1 terminfo source from P2 (`botster_terminal_ghostty::terminal_identity()`); `LaunchSpec.limits`
   and `stop_grace_ms` are being added by P3 in its stack (agreed); split any further P1 work into small stacked PRs (the lead's
   rule: this PR is far above the 2,500-line split rule).
6. Write the PR description with the Prior art note (`crates/botster-core-host/DESIGN.md` has it) and the list of mutants exclusions.

## Traps found

- `sed -i` on macOS needs `-i.bak`.
- The Linux host disk can be slow: do not put real fsync or real process tests in the default tier.
- Never edit the worktree during a gate? The gate runs a committed copy, so editing is safe, but it needs a clean tracked tree.
- `World::pump` in the unit tests does not enforce `pump_events`; only `HostDriver` does.
- Tests that count events per pump must use the `Rig` (driver) tests in `src/tests/driver*.rs`.

## Reviewer state

- Reviewer branch: `stage1/review-p1` in `/Users/jasonconigliari/botster-sessions/trybotster-botster-core-stage1-review-p1`.
- Last reviewed implementer head: `2016886ffda434bee0e87242d85e36ecc43ec212` on `stage1/p1-lifecycle`.
- Last verdict: `VERDICT: CLEAN` for the P1 host-side checkpoint.
- Last verdict commit: `44f5590b8ef351229204783dd0aeb8de3a737407`, pushed to `origin/stage1/review-p1`.
- Verdict file: `verdicts/p1-lifecycle.md`. It preserves all findings and closure evidence from eight review rounds.
- OPEN findings: none. F1-F13 and F15-F20 are closed.

F14 remains an authorized scope deferral, not TI-1 conformance. P1 owns the three `a2_8` ids and must wire `botster_terminal_ghostty::terminal_identity()` after P2 lands. The lead confirmed this in message `msg_plugin-w_1790927459_d216e0`. The placeholder comments and pending ids remain required.

P3 must implement the EndPayload worker handler. P1 defines EndPayload as SIGUSR1 to the identity-verified worker process alone. The worker requests payload termination, kills its payload group after stop_grace using the retained unreaped leader, and keeps serving the final model. The signal is idempotent. The worker reports the payload exit after link recovery or adoption. Worker-dependent conformance ids remain pending for P3. The interface is in `botster-core-link/src/lib.rs` and `botster-core-host/DESIGN.md`.

The current contract pin is `contracts-v0.1.6`, commit `caa029cfcc9c0bfd1f59a05d35d7c110ea99f3e0`, manifest final19. R-15 preserves Remove order under queue pressure. R-16 makes the StopAll target's Stopping row best effort. Accepted erratum 3 candidate 3 defines deadline effects, event budgets, carried-step priority, and parking. The verdict records these rules.

The reviewer ran no tests, builds, or gates. The implementer reported a failed gate at `0ac2b84` because a harness-count assertion expected one link harness instead of two. The last reviewed head changes only that assertion to two. It needs its own gate result; the reviewer has not received that result.

Any commit after `2016886` requires a delta review. The CLEAN verdict does not establish end-to-end conformance or Stage 1 acceptance. The scope exclusions and unproved ids remain in the verdict and conformance pending list.

Implementer session: `sess-1790919734-00aa-d8597316795619ab4c83427e523a0f96`. Lead session: `sess-1790903471-008f-8b9f78eef51d48aba5a45748495fd673`.

The reviewer worktree has an unrelated modified `.gitignore`. The reviewer did not stage or commit it. Never poll while waiting; end the turn and receive messages once after a doorbell. Do not spawn agents. The user's direct instruction permits only QUESTION or BLOCKED reports to the lead.

The lead requested pause via the user and orchestrator. Review work stops at this checkpoint.

## Added by the lead at the pause: host defects found by P3 (fix in P1)
P3 found two defects in P1's host code while building M2b (details in `p3-worker.md`, "Left", item 1):
1. **Resize forwarding order** (`sz_3_*` fail): three Resizes begun in one pump reach the worker as a, c, b. The host's forwarding of ops that are ready in one pump (`botster-core-host` run.rs `Next::Forward`) must keep the begin order (OR-1, SZ-3).
2. **Same-size shortcut** (`admit.rs`, `Op::Resize` arm): it compares with `s.size`, which lags while a Resize is in flight, so a Resize back to the old size completes `Applied` at once while the in-flight one applies later (SZ-2). Take the shortcut only when no Resize is in flight for the session.
Also note: the P1 reviewer's last CLEAN is on `2016886`; everything since (mutation tests, the fsync-test move, rebases onto v1, the contracts-v0.1.9 pin) is unreviewed delta and needs a delta review before the gate.

## Addendum: result of the gate on ff376cc (it was cancelled at the pause, after 2543 s)

Log: `~/botster-sessions/gates/botster-core-stage1-p1-lifecycle-ff376cce-linux-20261002-090058-59146.log`.
Passed: fmt, clippy, taint, lists, public-api, prebuild-worker, test-budget, slow (the storage move worked; the host disk was no
longer a problem). The mutants step ran (961 mutants, baseline OK in 43 s build + 1 s test) and was cancelled part way. Before the
cancel it had found three things that are still open:

- TIMEOUT `crates/botster-core-host/src/flows.rs:698:17: delete match arm (StartPhase::RowIdentity, Ok(())) in HostEngine::flow_row`:
  the mutant makes a test loop until the 20 s limit. A timeout is a review finding. Add a test that fails fast for this arm
  (a Start whose identity row write succeeds must reach `SendLaunch` or `AwaitHello`), or find the loop that a test needs a bound for.
- MISSED `inbound.rs:586: delete match arm Flow::Remove(_) in HostEngine::on_link_closed`: my test
  `a_link_closing_during_the_remove_teardown_leaves_the_uploads_unknown` (tests/flow_edges.rs) did not kill it. Check that the link is
  still registered when the test closes it (the worker may already be gone), and that the expected `OutcomeUnknown` comes from this arm.
- MISSED `inbound.rs:653: delete ! in HostEngine::run_parked`: the parked close that posts must not be parked again. Add an
  assertion that `parked_work()` is false after the parked close posted (`tests/losses.rs`,
  `a_route_event_waits_in_a_full_queue_and_posts_after_a_poll`), and a case where the close still finds no room.

Mutants after those three in the run order (the run goes by file) were not reached; expect a few more in `queue.rs`, `run.rs`,
`session.rs` and `botster-core*`. The list of 41 at `19788e0` is in `...-19788e06-linux-20261002-074946-42723.log`; the entries that
were added to `.cargo/mutants.toml` for them are in the file now.
