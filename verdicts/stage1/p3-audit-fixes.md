# PR #163 — P3 audit fixes integration review

## Round 1 — Full delta

Reviewed head: `0403470b8a08bdcbb6d89aab2a98ca3242c8e37b`.
Base and merge base: current v1 `144b0234fb632bcbb5176b17c2fe55f3239405df`. The PR does not undo a merged PR.
Reviewed delta: `144b023...0403470`, 24 files, 7 commits.
Audit source: `audits/v1-pr134-141.md` at `02acc4b`, findings A3, A8, A11, A30, A31, A52 and A53.
Package verdict: none yet on this head (P3 reviewer `sess-1791169299-010e`). CLEAN needs that verdict on the same head.
This reviewer ran no build, test or gate. The review used the source at the head and the PR description.

Scope of this review:
- the host (P1) to binding (P2) dependency and the IN-9 bound (A3);
- the real driver (botster-worker) and the testkit edge (P6) for the drain contract of `Action::DrainPty` (A30, A31);
- the sys exit watch and its consumer in the driver (A52, A53);
- the real-process fixtures (A8, A11), with BUILD.md testing rules 5 and 10;
- test quality: hacks, tests that assert a literal or check a value against itself, and hand-written terminal bytes.

### Accepted parts

- **Host to binding.** `botster-core-host` now depends on `botster-terminal-ghostty`. The facade and the testkit already link it.
  The encoders run with no I/O, clock, thread or randomness, so the machine crate stays sans-IO.
  5.1A says Core computes the worst case, so the host is the right owner of the computation.
- **One stored size.** `begin` computes the size once (`payload_size`). `PayloadTooLarge`, the IN-5 lane bound and both
  `Unknown{max_payload_bytes}` paths (`flows.rs`, `run.rs`) read `pending.held_bytes`. The order of the sync checks is unchanged:
  argument checks, then `PayloadTooLarge`, then `PendingLimit`, then the capacity codes.
  `Internal` for a failed encoder construction is an "either" code in the contract error table (fault class), so a sync `Internal` is allowed.
- **Early stop.** `longest > limit / times` implies `times * longest > limit`, so the early stop never refuses a write that fits.
- **No hand-written terminal bytes.** The 64, 192 and 6400 pins are deleted. The host tests get each expected size from the
  binding or from the given payload. The binding tests compare the bound with the explicit-modes encoders, which an existing
  test equates with the terminal path. These tests use no literal terminal byte (BUILD.md rule 2, ruling P35).
- **Enumeration.** The key states are all six key options with all 32 kitty flag sets. MACOS_OPTION_AS_ALT is fixed to TRUE
  on both the state path and the terminal path. The mouse states are five events with five formats, on the largest screen.
  `ANY_BUTTON_PRESSED` is not varied. Motion under `ANY` tracking gives the same report shape, so the maximum is unchanged.
- **A31 drain.** On Linux, `n_tty_read` flushes the port buffer (`tty_buffer_flush_work`) before it returns `EAGAIN`.
  So the flushing read is a real flush. The drain stays bounded by count, plus one chunk, plus one count measured after the flush.
- **A53.** The `report_exit` call on hello and `exit_reported` are removed. The link can reach `Ready` only once at M1, so no
  exit can be waiting at hello. The P5 reconnect path is recorded in the `report` doc.
- **A52 start time.** `a_start_time_of_zero_never_matches_a_live_process` tests the real `identity_state`.
  Changing the link type to `Option<u64>` stays with P5 (lead ruling).
- **A8.** The prebuilt worker now starts through the `GroupGuard` prefix, with `process_group(0)` and `exec`. The worker
  pid is the pid of the spawned child. The guard's anchor kills the group when the runner dies. The payload guard is unchanged.
- **A11.** These fixture waits now have deadlines, each with a timer marker: accept, the FIFO open and line read, the worker
  exits through `observe_exit` (`WNOWAIT`, the reap stays last), link reads and writes, the FIFO EOF reads, and the reap and
  drop of the slow_payload payload. The production `Payload::drop` is unchanged.

### Findings

#### I1 [HIGH, lowered to MEDIUM in the round 1 addendum] OPEN — A failed exit watch now hangs the worker; the comment says the worker ends

- Location: `crates/botster-core-sys/src/payload.rs` `wait_unreaped_with` (the new `panic!`) and `watch_exit`
  (`.spawn(move || on_exit(wait_unreaped(pid))).map(|_| ())`); `crates/botster-worker/src/main.rs` `exits` channel and the
  `EXIT` token.
- Evidence:
  - The panic runs on the `payload-exit` thread. The workspace sets no `panic = "abort"`. A panic on a spawned thread ends only
    that thread, and `watch_exit` drops its `JoinHandle`.
  - The driver keeps `self.exits.0`, so the receiver never disconnects. The waker never fires. `try_recv` on `EXIT` gets nothing.
  - So the worker never gets `PayloadExited`. It sends no `Exited`. A `Stop` waits forever. The host sees a `Running` session,
    not a lost worker.
- Why: the new comment claims "if this invariant ever broke, the worker ends and its host sees a lost worker". The code does
  the opposite. The old code invented `Code(-1)`. The new code hides the failure as a hang. EV-4 and AM-3 (exactly one
  completion) need an end that the host can observe. A52 asked for "unknown" to be carried, not hidden.
- Required:
  1. Send the watch failure to the driver as a value, for example `on_exit(Result<ExitStatus, Errno>)`.
  2. On a failure, the driver ends the payload group and ends the worker with a failure status, so the host observes a lost link.
     Do not abort the process: an abort skips the payload group kill.
  3. Add a default-tier test of that driver decision. The current unit test proves only that the decision function panics.
  4. Correct the comment.

#### I2 [MEDIUM] OPEN — `begin` does up to 2048 full encodes of a key's text in one synchronous call

- Location: `crates/botster-terminal-ghostty/src/encode.rs` `longest_key_sequence` and `encoded_len`;
  `crates/botster-core-host/src/admit.rs` `payload_size`.
- Evidence:
  - Each state calls the size probe with the whole event. The probe encodes the full associated text, so each call costs
    O(text length).
  - The early stop fires only after the running maximum passes `limit / times`. Text that fits in its worst form never stops early.
  - The kitty bits are the high bits of the state index. So the first 1024 states have kitty flag 16 off, even for a key whose
    text puts it over the limit.
  - `max_paste_bytes` is 1 MiB by default and up to 64 MiB (`limits.rs`). Take a key whose text is just under `limit / 8`.
    `begin` then encodes about 2048 × 128 KiB, roughly 256 MiB, at the default limit. At 64 MiB it encodes roughly 16 GiB.
    Each such call is a single sync host call on the host thread.
  - The `text` comes from a client through the Hub. The default-tier tests use a 4096-byte limit, so no test shows the cost.
- Why: BUILD.md "fast and lean". Plan 9B and TM-6 bound the work of each `pump`. A `begin` with unbounded work stalls every
  session on the host. The contract fixes the bound's value, not the method used to compute it.
- Required:
  - Bound the cost of `begin` so that it does not grow as (number of states) × (text length). Two possible ways:
    - order the states so that the largest forms come first, and stop early;
    - derive the text's share from libghostty once per distinct form, if the implementer can show from libghostty that this
      share does not depend on the state.
    No hand-written table is allowed (BUILD.md rule 2).
  - Record a measurement of `begin` for the worst admissible key at `max_paste_bytes` = 1 MiB and at 64 MiB.
  - If an exact bound cannot be computed at an acceptable cost, ask the lead a QUESTION. Do not choose a bound alone.
- The mouse bound costs 25 encodes and the focus bound 1, so they are not in this finding.

#### I3 [MEDIUM] OPEN — The testkit drain again differs from the real drain (A30)

- Location: `crates/botster-core-testkit/src/worker.rs` `ready` and `take` (`drain: Option<usize>`), with the comment "As the
  real driver: a drain ends when its bound is read or a read finds nothing"; `crates/botster-worker/src/io_decisions.rs` `Drain`.
- Evidence:
  - The real driver now has three steps: `Counted`, then one flushing read (`Flush`), then `Flushed(count measured after it)`.
  - The testkit edge has only the first step. It ends the drain as soon as the counted bytes are read.
  - Take a program that writes after `DrainPty` was asked. The real worker reads up to one chunk plus a measured count of that
    output before `Exited`. The testkit worker reads none of it.
- Why: A30 was exactly "drain semantics differ between the real driver and the testkit edge". BUILD.md says that a worker
  behavior that differs between the two runs is a bug in an edge. The comment says the edge matches the driver, and it does not.
- Required: use one drain decision in both drivers.
  - Move `Drain` into a crate that both drivers can use. Two possible crates:
    - `botster-worker-core`, next to `Action::DrainPty`, whose contract it implements;
    - `botster-core-edges`.
  - The testkit then drives the same `Drain`, with `ScriptedProgram::unread` as its count.
  - If the lead rules that the testkit keeps a one-step drain, record the ruling in the comment, with the reason the
    flushing step cannot be observed there.

#### I4 [LOW] OPEN — `Drain::after_read` does not do what its doc says for an empty read

- Location: `crates/botster-worker/src/io_decisions.rs` `Drain::after_read`, the driver's `match found { 0 => Drain::Done, .. }`
  in `read_pty_chunk`, and the test helper `drain_with`.
- Evidence:
  - The doc says: "A read that finds nothing or the end of the output completes the drain (`Drain::Done`)". But
    `after_read(0)` on `Counted(3)` returns `Counted(3)`, and on `Flushed(3)` returns `Flushed(3)`.
  - That rule lives in the driver, and `drain_with` copies it (`0 => Drain::Done`). So the unit tests check a copy of the
    driver's branch, not the decision.
- Required: handle `n == 0` in `after_read` (return `Done`). Call it with every read result from the driver and from the
  test helper. Delete the copied branch from both.

#### I5 [LOW] OPEN — A test name and doc claim a check that the test no longer makes

- Location: `crates/botster-core-host/src/tests/queue_pressure.rs` `retirement_keeps_the_result_path_and_bounds_a_repeated_key`.
- Evidence: the PR deleted the repeated-key assertion. The name and doc still say "a sent repeated key is `Unknown` with the
  bound of every repeat". `l.max_key_repeat = 100` is now dead setup.
  `a_write_in_flight_when_the_link_fails_is_unknown` now covers a repeated key's `Unknown` bound in `lifecycle.rs`.
- Required: rename the test and correct its doc to the part it still checks. Delete the dead limit. In the doc, name the
  test that now covers the repeated key.

#### I6 [LOW] WITHDRAWN (round 1 addendum 2) — `Payload::reap` exists only as an alias of drop, and it needs a mutation exclusion

- Location: `crates/botster-core-sys/src/payload.rs` `Payload::reap`; `.cargo/mutants.toml`, the entry
  `replace Payload::reap with ()`.
- Evidence: the body is `drop(self)`. Its own exclusion entry says that the `()` mutant "is the same code". The function adds
  no behavior. It stays only for its name, and it costs an equivalence entry that each later change must recheck.
- Required: delete `reap`, and call `drop(payload)` at the one production call site and in the slow tests, with the existing
  comment. Delete the exclusion entry. The audit's A53 offers this option.

### Gate evidence still pending (not findings)

- `slow_payload` and clippy with `slow` have not run on this head because of the HOLD. The single gate on the CLEAN head
  covers them, under the lead's gate-evidence closure rule.

VERDICT: NOT CLEAN (6 open)

## Round 1 addendum — I1 severity and the lead's A52 ruling (same head `0403470`)

- The lead's A52 ruling (state log, 2026-10-04) makes the unreachable `Code(-1)` a panic that names the errno and the invariant.
  The package reviewer's F25 (round 74, `9735723`) reports the same facts as I1, rated LOW, and requires a comment correction.
- The wait fails only if the invariant breaks, for example a second reaper in the worker process. So this reviewer lowers I1
  from HIGH to MEDIUM. The finding stays OPEN with the same required behavior.
- The fix keeps the ruling: keep the panic, and give it the effect that the ruling's comment states. A comment-only fix that
  documents a silent hang does not close I1. A52 asked that a failure be visible to the host, and a silent hang hides it.
  One possible way:
  - the driver keeps no sender of its own;
  - a guard in the watch closure wakes the driver when the thread ends;
  - the driver treats a disconnected exit channel as a fatal failure: it ends the payload group and exits with a failure status.

VERDICT: NOT CLEAN (6 open)

## Round 1 addendum 2 — I6 withdrawn (same head `0403470`)

- The package reviewer challenged I6. The audit's A53 offers `reap` as `drop(self)` as one of two accepted fixes.
- The body is correct. The `()` mutant is exactly equivalent, and its exclusion entry states that reason. The consuming name
  marks the ownership boundary at `ReapPayload`.
- I6 named no defect, only a preference for the other offered fix. This reviewer withdraws I6. No change is required.

VERDICT: NOT CLEAN (5 open)

## Round 2 — Fixes for I1 to I5

Reviewed head: `82b4269bcb0d1412b94f38c818df5169617570c6`. The base is still current v1 `144b023`.
Reviewed delta: `0403470..82b4269` (`6770c0c`, `82b4269`), 14 files.
This reviewer ran no build, test or gate.

- **I1 CLOSED.** `watch_exit` now gives `io::Result<ExitStatus>`, and `wait_unreaped_with` returns the errno; it does not
  panic. The driver maps the result through `io_decisions::exit_input` and returns the error from `Driver::run`. `run` consumes
  the driver, so the error drops the `Payload`, and its drop kills the group. `main` then prints the error and exits with the
  failure code from `command_line::execute`. So the host sees a lost link. No exit status is invented.
  The default-tier tests cover the watch decision (`an_interrupted_watch_retries_and_a_failed_watch_gives_its_error`) and the
  driver's mapping (`a_failed_exit_watch_is_an_error_and_never_an_exit`). An error value is better than the panic, because one
  tested path carries the A52 effect. That fits the lead's A52 ruling: the unreachable case never invents `Code(-1)`.
- **I2 CLOSED under the lead's ruling** (state log, 2026-10-04: option (b) now; the steward is asked to bound `KeyInput.text`;
  a follow-up PR adds measurements if the steward rules).
  - The kitty flags are now the low five index bits, with flag 16 inverted. So the first 16 probes have "report associated
    text" on. The enumeration still covers all 2048 states.
  - The early stop is unchanged and stays exact. A key over the limit now stops after at most 16 probes.
  - The doc of `longest_key_sequence` states the remaining cost of an admitted key, and it is marked
    "pending steward: KeyInput.text bound".
  - The binding test compares the result with a maximum over the whole mode cube, so the new order changes no expected value.
- **I3 CLOSED.** `Drain` moved to `botster-worker-core::drain`. It is pure, so it fits the machine crate. The real driver and
  the testkit edge now drive the same decision: the testkit uses `ScriptedProgram::unread` as its count, and `after_read` after
  each read.
  - The edge offers `PtyDrained` when a drain is `Done`, or when its next read would find nothing. In the real driver, that
    read returns `EAGAIN` (`found = 0`), so the next step gives `PtyDrained`. The sequence the machine receives is the same.
  - `the_edge_drains_as_the_real_driver_does` checks behavior: the counted bytes, the flushing read that finds the later
    output, and the bound against output written after the measured count. The expected counts come from the test's own
    writes, not from terminal semantics.
- **I4 CLOSED.** `after_read` returns `Done` for `n == 0` as its first arm. The driver, the testkit edge and the test helper
  each call it with every read result. No copied branch remains.
- **I5 CLOSED.** The test is renamed `retirement_keeps_the_result_path_of_a_running_setter`. Its doc names
  `a_write_in_flight_when_the_link_fails_is_unknown` as the proof of the key bound. The dead `max_key_repeat` setup is deleted.
- **I6** stays WITHDRAWN (addendum 2).

The changes to `process_guard.rs`, `payload_guard.rs`, `slow_payload.rs` and the bounded cleanup in `session.rs` answer the
package reviewer's F26 to F28. They are package scope. The shared `GroupGuard` still owns its group and reaps only its anchor,
so the cross-package ownership rule (BUILD.md testing rule 10) is unchanged.

Integration findings: all closed (I1 to I5 closed, I6 withdrawn).
Still needed for CLEAN on this head: the P3 package reviewer's verdict on `82b4269`. The gate on the CLEAN head proves the
pending slow-tier evidence (`slow_payload`, clippy with `slow`, native Mac proof).

VERDICT: NOT CLEAN (1 open: package verdict on 82b4269 pending; 0 integration findings open)

## Round 2 note — the I1 test (same head `82b4269`)

- The package reviewer keeps F25 open. `a_failed_exit_watch_is_an_error_and_never_an_exit` tests only `Result::map`. It does
  not prove that `Driver::run` fails, that the payload group ends, or that the link is lost.
- This reviewer agrees that the test is a forwarding check of a one-line helper (the A33 pattern). Round 2 closed I1 on source
  inspection. The actual-path proof is the package reviewer's F25. This reviewer does not open a duplicate finding.
- If the lead rules on the panic form, a new head gets a delta review here.

VERDICT: NOT CLEAN (1 open: package verdict on 82b4269 pending; 0 integration findings open)

## Round 3 — Delta for package findings F25 and F26

Reviewed head: `800606ac2a99c42a7e25886a5aa1ccc2b88070d1`. The base is still current v1 `144b023`.
Reviewed delta: `82b4269..800606a`, one commit, 8 files. This reviewer ran no build, test or gate.

- **Production.**
  - `payload.rs` adds `ExitWatchFailed { errno }`. Its `Display` names the errno and the invariant, and it converts into
    `io::Error` with the errno's kind and the typed source.
  - `main.rs` inlines `Input::PayloadExited(watched?)`, and `io_decisions::exit_input` with its map-only test is deleted. This
    removes the forwarding test noted in the round 2 note.
  - No interface between packages changes. `watch_exit` keeps the `io::Result<ExitStatus>` callback type of round 2.
- **The real-path proof.** `a_failed_exit_watch_ends_the_worker_with_a_failure`:
  - It sends the typed failure through the driver's real exit channel and wakes it. This uses no production seam: the test
    writes the channel that the watch thread writes.
  - It runs `command_line::execute` with `Driver::run`, under a marked deadline. It then asserts the failure exit code and the
    message with the errno and the invariant.
  - It shows that the whole payload group ended: a background group member's FIFO reaches EOF under a deadline.
  - The peer socket is nonblocking. The read loop runs only after `run` has returned and the driver is dropped, so it reads the
    buffered bytes and then EOF, and it cannot block.
  - Each assertion checks a value that the test supplied or an observed process effect. No terminal byte is asserted.
- **Test cleanup** (`process_guard.rs`, `payload_guard.rs`, `slow_payload.rs`, `session.rs`) is the package reviewer's F26
  scope. The shared guard still owns only its group and reaps only its anchor.

Integration findings: none open. Still needed for CLEAN: the P3 package verdict on `800606a`.

VERDICT: NOT CLEAN (1 open: package verdict on 800606a pending; 0 integration findings open)

## Round 4 — Test-only delta for the package finding F25

Reviewed head: `ccba0504b8f0e18d274132e0b88b80b2e364becd`. The base is still current v1 `144b023`.
Reviewed delta: `800606a..ccba050`, one commit, one test file. No production change. This reviewer ran no build, test or gate.

- Round 3 missed the new `sleep 30` group member in `a_failed_exit_watch_ends_the_worker_with_a_failure`. BUILD.md testing
  rule 5 forbids it. The package reviewer's F25 found it.
- Now the background member is the external `/bin/cat` (the vault's FIFO rule). It blocks on the open of a FIFO that nothing
  opens for writing, and it keeps the inherited `held` writer.
  - It uses no CPU and no timer.
  - It ends only when the group is killed. The payload guard owns that group on every exit path.
  - So the EOF on `held` still proves that the whole group ended.

Integration findings: none open. Still needed for CLEAN: the P3 package verdict on `ccba050`.

VERDICT: NOT CLEAN (1 open: package verdict on ccba050 pending; 0 integration findings open)
