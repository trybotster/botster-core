# botster-core-testkit: M0b design note

## RealCoreHarness over Core's own composition (plan 23l)

Scope: lead rulings of 2026-10-09 (plan 23l, R-43). `RealCoreHarness::open` calls `botster_core::open_parts` and then
`HostDriver::open(cfg, EdgeTap::new(edges, rows))`. `Core::open` is `HostDriver::open` over the same parts, and `Core`
only delegates to its driver, so every id runs on Core's own code. Core has no test branch and no feature flag.

The boundary is `HostEdges` (`src/edge_tap.rs`). `EdgeTap` passes every call to the inner `RealEdges` unchanged:

- Inbound edges are "take the next item" calls (`link_recv`, `accept_link`, `poll_process_exit`). The tap can take
  ahead, hold the items, and hand them out in order. The driver reads every link until `WouldBlock` and drains new
  links and exits on every pump, so a held item reaches the engine at the next pump. The tap signals the wake edge.
- `edges_quiet` (`{quiet}`) is a take-ahead that finds nothing new and holds nothing. Bytes that are still inside a
  worker process are not visible here. So quiet means "nothing arrived and nothing is unread", never "the worker
  finished". A transcript that needs "the worker finished" must wait for an event of that.
- `break_control` (`{session}`) calls the inner `link_close` on the session's link. `RealEdges` drops its end of the
  stream (it does not call `shutdown`). The worker reads EOF, and every later call of the driver on that `LinkId` gets
  the closed-link state: `Ok(0)` on `link_recv`, `BrokenPipe` on `link_send`. `RealEdges` never reuses a `LinkId` and
  keeps no raw descriptor number, so a call never reaches a descriptor that the OS gave out again
  (`tests/slow_edge_tap.rs`).
- The process record: every registry row that passes through `write_row` or `read_rows` is decoded with Core's own
  `Row::decode`. The rows of a data directory are kept across handles and outlive `Remove`.
  `RealCoreHarness::session_processes(dir, session)` gives the instance, the worker identity and the payload identity.
  The hello of each link (or the `connect_worker` call) names the link's instance.
- The harness keeps only a `Weak` to each tap. The data directory's lock (LC-2) ends with the driver.
- `injects_clock` is true (R-43 A): Core reads no clock, and `pump` takes `now` from the runner. Workers and payloads
  follow real time, so `progress_is_injected` is false (steward ruling R-46): the driver never jumps the clock to Core's
  next deadline, and the clock moves only by a transcript's `advance_clock`.

The real tier's trials (`tests/suite/mod.rs`, plan 23l and 23q):

- It runs every id that is not pending, deferred or withdrawn, except the Core A20-1 testkit-proven ids
  (`botster_core_conformance::TESTKIT_PROVEN`, derived from the contract): each is listed as `testkit-proven`, not run
  and not a failure. `cargo xtask lists` refuses such an id in `core-real-pending.txt`.
- An id of `conformance/core-real-pending.txt` runs only under `--ignored` and is reported as pending-real. The file was
  initialized once from the gate's real run and only shrinks.
- The report gives the minimum counts of `conformance/minimum-core.txt`, the canonical minimum list: "testkit-passing /
  69" on the testkit tier, and "real-passing / 68, real-accepted / 69" on the real tier. The denominators are derived:
  68 leaves out the A20-1 members of the list, and real-accepted adds back each A20-1 member that is not in
  `core-pending.txt`. The real tier's report run is `--ignored`, after the nextest run of the same job passed every
  trial that must pass, so it counts those trials.

Prior art: none copied. The scripted inner edges of `edge_tap/tests.rs` are new.

## P6 oracle controls: current phase

Scope: `brief-p6-oracle.md`, plan pin `bdda2359`, contracts `contracts-v0.1.13`.
The testkit owns the oracle handles, the control reports, and the statement reports.
P2 owns terminal semantics and snapshot decoding in `botster-terminal-ghostty`.
P1 and P3 own harness dispatch and worker integration.
This phase does not edit `harness.rs`, `program.rs`, `core.rs`, or `worker.rs`.

`OracleHandle` owns a separate libghostty terminal behind a retained handle.
The driver feeds `consumed_output` after each completed worker model step.
The driver supplies only the consumed prefix, including a query when the step ends at a query.
The oracle uses `vt_write_until_query` and drains its callbacks after that step.
A different consumption count or lost callbacks produces an error.
The driver applies each accepted resize through `OracleHandle::resize`.
The oracle never reads the subject's cached modes, text, title, cwd, or notification.

Planned dispatch after the stacked P1/P3 PR lands:

1. Create one `OracleHandle` with each session's size and history configuration.
2. Retain a clone in the harness's session map.
3. Feed consumed output at the worker model boundary, after each model step.
4. Apply accepted resize and terminal configuration changes at that boundary.
5. Dispatch `oracle_state`, `oracle_modes`, `oracle_screen`, `oracle_cursor`, and `oracle_notification` through the retained handle.
6. Parse `oracle_encode` input into the contract's key or mouse type.
7. Pass explicit modes to the binding when the control supplies them.
8. Otherwise use the oracle's current modes.
9. Convert binding output bytes into the control's JSON result with the contract codec.
10. Keep the binding's typed zero result when the encoder produces no output.

For `oracle_query_reply`, resolve the session size and decode the optional prefix and request with the contract codec.
Call the public helper with the optional host color profile.
The helper uses a fresh libghostty shadow and reports its answerability and encoded reply.
For `oracle_hyperlinks`, read all Core capture pages or decode the route baseline's screen payload.
Pass the native snapshot bytes and source configuration to the public helper.
The helper restores the bytes through libghostty and reads cell URIs, including history.

Key repetition remains a transaction concern of the dispatch adapter.
The key encoder returns one event's encoding, as P2 specifies.
Focus reports and paste markers also come from libghostty.

`statement_runs::run_deterministic` calls `run_script_events` with a fresh harness for every requested run.
The factory must reuse the seed and configuration.
The helper preserves event order and instance values.
The conformance driver checks normalized equality and session identity.

`statement_runs::error_codes_reachable` validates every scripted sync case against `refusal::ROWS`.
The dispatch adapter must execute each probe through real Core or its refusal layer.
The adapter returns true only after it observes the requested code at the requested timing.
A table entry alone never proves reachability.
The helper reports every missing probe in `not_reached`.
The helper reports the statement's explicit exclusions without treating those exclusions as reached codes.

All existing pending ids remain pending.
Oracle unit tests cannot prove conformance dispatch while the harness cannot open Core.
The terminal snapshot controls also depend on the separate P2 API PR.
The acceptance check remains the conformance transcript through real Core after dispatch lands.

The P2 API and GHOSTSNP spec PRs have merged.
`snapshot_controls::CapturePages` receives all Core `read_page` bytes in page order and the source configuration.
The adapter passes the actual session terminal at the capture revision to `oracle_restore`.
Each restore field checks its own state group, including history attributes and hyperlinks.
The adapter records every output chunk consumed after that revision for `oracle_resume`.
The resume comparison uses native snapshot bytes, including pending parser input and saved state.
The version control changes only the native envelope version and invokes the typed decoder.
The graphics control reads the native limit on the actual model and restored instance.
Both constructors set that limit to zero before input. Libghostty enforces it.
Constructor tests in P2 check native image lookup after image stimuli on both screens.

`every_cut::CutSession` separates the helper from the protected dispatch files.
The adapter creates a fresh real Core session for each offset, writes the prefix, runs the fence, captures, and reads all pages.
It then writes the suffix and returns the exact consumed chunks in order.
An offered capture also returns consumed chunks after its revision and before the suffix.
The helper replays those chunks first, so a preceding ground-state cut can meet the same resume invariant.
The adapter must verify that no resource failure or held snapshot is injected.
The helper checks the session size and history configuration.
It reads the actual model's semantic failure and continuation status at every cut.
It reads both statuses again after capture and before the suffix.
Unavailable retention within the independently measured limit remains a mismatch even when a ground-state offer resumes correctly.
A separate diagnostic terminal retains at least the full input length and measures pending input.

Steward ruling R-30 (contracts-v0.1.17) permits native encoded length plus independent format framing as fit evidence.
The framing source must count each per-capture field at its largest allowed size.
It must obtain these sizes from the GHOSTSNP spec, never from Core's capture.
The native measurement uses the same format, version, size, history setting, and zero image limit.
`UnknownFraming` is the current dispatch source because the spec's worker paging section remains pending P3 M2b.
Every unknown fit sets `inconclusive: true`.
Every known non-fitting cut also sets `inconclusive: true` because the corpus prerequisite fails.
An offered capture whose independently framed size exceeds the maximum adds a mismatch.
The every-cut id remains pending for worker paging and real Core dispatch.
Unit adapters and synthetic framing in tests prove helper behavior only.

The slow process-group tests retain `OwnedGroup` for every spawned group.
Their children block on a pipe held by the test.
Closing that pipe ends the child without a timer or CPU loop.
Drop and panic still kill the owned group and reap its leader.
The parent-exit regression starts a fixture in the outer test's owned group.
The fixture starts a child in that group, holds its input pipe, and exits without running Rust cleanup.
The outer test waits for EOF before it drops its group guard.
EOF proves that the child closed its inherited output after the parent exited.

Prior art: P2 provides every terminal read and encoder.
No old botster-core source or test was copied.
The new testkit code keeps handles and forms reports; it implements no terminal semantics.

Scope: P6 milestone M0b (plan 6.2). Plan pin `555bc433`, contracts `contracts-v0.1.1`. The `RefusalScript` layer, `RealCoreHarness`
and the controls that need real Core come later.

## Prior art

- **Old botster-core (`72b2e335`):** read as prior art only. Nothing was reused: no `Stolen-From` commit exists in this branch.
- **Reused libraries:** `rand_chacha` and `rand_core` (the seeded ChaCha8 stream, the generator that botster-contracts uses for its
  seeds); `botster-probe-script` and `botster-route-codec::hex_decode` (the script types and hex decoding, from the contracts, never
  `botster-fake-core`); `libtest-mimic` (P0's conformance harness).
- **Hand-rolled, with reasons:**
  - `Sim` (plan 4.1): turmoil and madsim simulate a tokio runtime and tokio sockets. Core has neither. A loop over sans-IO machines
    is about the size of an event-loop driver.
  - The in-memory duplex (`net.rs`): no maintained sans-IO in-memory stream carries the per-end readable and writable flags that
    plan 2.5 rule 8 needs, passes descriptor objects by value, and takes its read sizes from the scheduler.
  - The probe interpreter (`program.rs`): the contracts state that an in-memory interpreter belongs to the testkit.
  - Uniform sampling in the scheduler: a rejection loop over `next_u64`, so the choices do not depend on the `rand` crate's
    `random_range` algorithm across versions.
- **Rejected:** a `RouteTransport` write-size choice point. A5-2 and plan 2.4 name only the read sizes of the route-transport edge,
  so a write takes what the queue has room for.

## Choices that the plan leaves open

- The session visit stays round-robin (P0's `Production`). It is not an item of A5-2.
- `fork_child` is refused as real-only (a child in the process group needs a real process). `ignore_sigterm` is a flag that is set
  when execution reaches it. The in-process program receives no signal.
- The exit of the program is visible once its `Exit` step has run, independent of unread output. The driver and the scheduler decide
  when the host sees it.

## Stage 0 defect in the pinned probe (not a P6 finding)

The probe binary of `contracts-v0.1.1` runs `PrintAfterInput` through a nested `run`, which ends the process with code 0 before later
steps run. It ends with code 3 on `IgnoreSigterm`. The testkit keeps the ordered semantics that the probe-script format and A5-1
define. A real-harness run that reaches either case stays blocked until Stage 0 fixes the probe under a later tag. P6 does not patch
the probe and does not move the pin.

## Budget check (R4)

- Source and command: `crates/botster-core-testkit/examples/budget.rs`, run as
  `botsterq run --label "core p6 budget" --deadline 20m -- env -u RUSTUP_TOOLCHAIN CARGO_BUILD_JOBS=4 cargo run -q -p botster-core-testkit --example budget --release`.
  The example runs every Core transcript once per seed for seeds 0 to 31 explicitly, because the pinned `run_transcript` stops at
  the first seed that does not pass. It counts the driver constructions and the outcomes.
- Measured at the head that adds the example (the commit after `4f8ec55`; the harness code is unchanged from `4f8ec55`).
- Result: 132 transcripts, 32 seeds, 4224 runs, 4192 driver constructions (131 transcripts x 32 seeds), 32 passed, 4192 not passed.
  Total 2.75 ms: 20.8 µs per transcript over 32 seeds, 650 ns per run, worst run 203 µs
  (`conf::am_1_create_then_start_same_turn`, seed 0). The one passing transcript is `conf::er_deadline_expired`: the pinned
  transcript has kind `withdrawn`, and `run_transcript` returns `Passed` before it constructs a driver.
- **Limit of this measurement:** the 4192 runs that construct a driver end early, and the example does not tell which way. A run
  stops at an absent feature or an unsupported control (`has_control` is false for every op, so a transcript that requires
  `pty_blocked` or `pty_input` stops before setup, for example `conf::am_2_fairness_bound_across_routes`), or at `open`, which
  reports that no Core exists. The numbers are an early-exit measurement of the harness. They do not show the cost of a passing
  Core transcript. R4 needs a new measurement when P1 provides the engine and the controls exist.

# Scope 2, step 1: the pin move, the status files, the base commit, the nightly

Plan pin `c43693ff`, contracts `contracts-v0.1.7` (`f14c895`, manifest final21).

- **Pin move** (its own commit): 599 to 640 Core ids; the 41 new ids are listed in the commit message and added to
  `core-pending.txt`; the two ids that A9-2 withdrew leave `core-pending.txt` and are never pending again. `CoreHarness` gained
  `WorkerRef::file_name`, `worker_named`, `injects_clock` and `statement` at this tag; the testkit answers `file_name: None` and
  `injects_clock: true`.
- **Status files.** The harness reads verbatim copies of the contracts' `conformance/deferred.txt` and `withdrawn.txt`
  (`conformance/contracts-*.txt`), because a test binary cannot find the contracts checkout at run time. `cargo xtask lists` fails
  when a copy is not the pinned file, and `cargo xtask ledger-ids --write` writes them, as it does for the ledger ids. The parser
  is `botster_core_testkit::status` (hand-rolled, about 60 lines: the contracts' own parser is inside their xtask, not a crate);
  xtask and `tests/conformance.rs` share it.
- **Checks added to `lists`:** a withdrawn id is a ledger id and is never pending or deferred; `core-deferred.toml` equals the
  whole-id Core deferrals of `deferred.txt`; a `not-applicable` id is a ledger id that is neither withdrawn nor deferred. The
  harness reports five counts and lists each not-applicable case.
- **Base commit.** `fsutil::base` resolves `BOTSTER_CI_BASE_REF` (or `origin/v1`) to a commit once per run, prints it
  (`base: <ref> = <sha>`), and returns it to every check. The mutation diff and the pending-list check both use it.
- **Nightly.** `ensure_nightly` runs `rustup run nightly-2026-09-30 rustc --version` with auto-install off and fails with
  `missing prerequisite: nightly-2026-09-30`. The `rustup toolchain install` fallback is gone.
