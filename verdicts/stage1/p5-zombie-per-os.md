# Integration review: PR #258

## Round 1: zombie identity per OS

- Head: `f289f5d48f4fd74759266a943ff82c0a970ab805`.
- Base and sole parent: `c8907b74bd247002784843e1a12f6487b08c579f`.
- Tree: `6355ad46cf4c6ea5d73070075712cd1d745fc48c`.
- READY: `msg_plugin-w_1791715056_160430`.
- Package CLEAN: `3c721d2d41119ac00e5e59147caaa19dd4810696`, verdicts/p5-zombie-per-os.md.
- Package receipt: `msg_plugin-w_1791715264_609756`.
- Risk: HIGH. The shared testkit changes.

Sol read the complete one-file delta, surrounding proof, production process adapters, full package verdict, and retained logs.
The delta adds twelve lines and deletes five lines. No finding remains open.

The existing proof owns a child and observes its exit without reaping it before the identity check.
Linux retains the zombie's start time and reports `Matches`. macOS refuses its process information and reports `Absent`.
The changed assertion accepts those existing OS facts. It retains the required false liveness result on both OSes.
It also retains the live-process and different-start-time checks. No production process behavior changes.
No new helper, retry, timeout, polling, dependency, transcript, pin, pending entry, exclusion, or acceptance list appears.

Sol checked Prior art against OLD CORE `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
Its Rust source has no `start_time` occurrence. Its local_process.rs documents the macOS zombie group-signal case.
This PR copies no old implementation. It reuses the current process identity, owned-child, exit-observation, and liveness helpers.

Linux gate:
`~/botster-sessions/gates/botster-core-stage1-p5-zombie-per-os-f289f5d4-pool-20261011-033008-99299.log`.
The header identifies the exact head and reviewed base. All ten steps pass in 248.7 seconds.
The default tier passes 1616 tests in 11.641 seconds. The slow tier passes 466 tests in 29.059 seconds.
The changed zombie proof passes in 0.043 seconds. The test-and-documentation diff has no mutant; no mutation run starts.
Fuzz starts no trial because the diff changes no crate with a decoder harness.
The job and wrapper exit 0 after 318 seconds; the job queues one second and runs 317 seconds.
Real-pending remains 58, with zero newly passing entries. The raw report still includes unfenced AD-7; R-59's limit remains.

Mac log:
`~/botster-sessions/gates/botster-core-stage1-p5-zombie-per-os-f289f5d4-pool-20261011-033607-45113.log`.
The source head is exact. The header records later base `d86bfcc747718452efe57b2aa207492acde96573`, which this head does not import.
The focused run supplies platform evidence. It supplies no gate acceptance for that later base.
Worker prebuild and clippy pass. All nine real slow-control tests pass in 2.301 seconds; the changed proof takes 0.105 seconds.
The job and wrapper exit 0 after 50 seconds; the job queues two seconds and runs 48 seconds.
The previously read #257 Mac log retains the old `Absent` versus `Matches` failure. The new expectation removes that assertion failure.

This CLEAN covers only the exact head and reviewed base above.
Current v1 is d86bfcc7. A later import needs the applicable base check or delta review and its own full exact-head gate.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN (0 open) at f289f5d48f4fd74759266a943ff82c0a970ab805
