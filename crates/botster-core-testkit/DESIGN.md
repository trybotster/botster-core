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

- Command: a throwaway example (not committed) that ran `run_transcript` for every Core transcript over `TestkitHarness`, once with
  the default seed set (0 to 7) and once with `BOTSTER_SEEDS=0-31`. Measured at `b97b605`.
- Result: 132 transcripts, 32 seeds, 1.5 ms in total, about 11 µs per transcript, worst 0.53 ms (`conf::am_1_create_then_start_same_turn`).
  The 8-seed run gave the same time.
- **Limit of this measurement:** every transcript stops at its first step, because `open` reports that no Core exists. The numbers
  cover the failing `open` path of the harness. They do not show the cost of a passing Core transcript. R4 needs a new measurement when
  P1 provides the engine.
