# botster-core-testkit: M0b design note

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
