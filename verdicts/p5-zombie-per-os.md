# P5 zombie identity per OS — #258

## Round 1 — 2026-10-11

- PR: https://github.com/trybotster/botster-core/pull/258.
- Branch: `stage1/p5-zombie-per-os`.
- Exact reviewed head: `f289f5d48f4fd74759266a943ff82c0a970ab805`.
- Parent and reviewed base: `c8907b74bd247002784843e1a12f6487b08c579f`.
- Tree: `6355ad46cf4c6ea5d73070075712cd1d745fc48c`.
- READY: `msg_plugin-w_1791715056_1ef068`.
- Risk: HIGH, BUILD rule 3. The shared testkit changes. The proof covers OS-dependent process identity.
- Authority: BUILD.md, AD-6, AD-7, EV-5(c), the lead's B9 direction, and approved plan 23an.
- Plan pin: `/Users/jasonconigliari/botster-sessions/pins/stage1-plan.05b18303.md`.
- Plan SHA256: `361935e2e23c7327d1d30cbc84dd6c9fbe41108d0e97a1304505928e647c4a63`.

The reviewer read the complete one-file delta: twelve additions and five deletions. No finding remains open, including LOW.

### Source conclusions

The test now expects `Absent` for an unreaped zombie on macOS and `Matches` on Linux. The worker-control documentation and test documentation state both cases. Production behavior does not change.

The reviewer checked the existing process adapters. macOS reads `BSDInfo` through libproc. A refused read gives no start time, and `identity_state` maps that result to `Absent`. Linux reads field 22 of `/proc/<pid>/stat`; an unreaped zombie retains that start time. The unchanged identity guard sends no signal unless the state is `Matches`.

The proof retains an owned child until teardown. It observes the child's exit without reaping it before the assertion. It still proves that a live recorded process lives, an identity with another start time reads `Reused` and does not live, and a zombie does not live. The fix changes only the intermediate platform fact. It preserves the required AD-6 and liveness outcomes.

The change adds no timeout, retry, polling, process helper, exclusion, dependency, pin, list entry, or transcript. `git diff --check` reports no whitespace error.

The Prior art note is present. The reviewer checked old Core at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`. A Rust-source search finds no `start_time` occurrence. The named `local_process.rs` code documents the macOS zombie group-signal case. This PR copies no old code and reuses existing process-test helpers.

### Evidence

Linux gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p5-zombie-per-os-f289f5d4-pool-20261011-033008-99299.log`.

The header names the exact reviewed head and base. All ten steps pass in 248.7 seconds. The job and gate exit zero after 318 seconds on `msa1`.

- Default tier: 1616 passed, 484 skipped, 11.641 seconds.
- Slow tier: 466 passed, 1980 skipped, 29.059 seconds.
- The changed zombie proof passes in 0.043 seconds.
- The mutation step finds no mutant in this test-and-documentation diff. No mutation run starts.
- Fuzz passes; the diff changes no crate with a decoder harness.
- Reported minimum: testkit 55/69, real 40/68, accepted 41/69. This PR changes no acceptance list. R-59 still excludes unfenced AD-7 from the accepted real count.

Mac focused log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p5-zombie-per-os-f289f5d4-pool-20261011-033607-45113.log`.

The header names the exact reviewed head. It records the later base `d86bfcc747718452efe57b2aa207492acde96573`; that base is not imported into this reviewed head. This focused run is platform evidence, not the merge gate. Worker prebuild and clippy pass. All nine `real::slow_controls` tests pass in 2.301 seconds. The changed zombie proof passes in 0.105 seconds. The job and gate exit zero after 50 seconds.

The reviewer checked the earlier Mac failure at `shared/core-stage1/gate-logs/zombie-mac-c121ec31-tmp.log`. It reports `Absent` against the old `Matches` assertion and exits 100. The #257 Mac log on `96f6f387` shows the same old assertion failure. The shared catalog B9 records that kernel behavior and both identity states. The corrected head retains the zombie's false liveness result on both OSes.

### Scope and next step

This package CLEAN applies only to the exact head and reviewed base above. It requires Sol's separate integration CLEAN. Current v1 is `d86bfcc747718452efe57b2aa207492acde96573`, after #256. A later base import requires the applicable base check or delta review and its exact-head Linux gate. This verdict accepts no future head.

The reviewer changed no product code and ran no builds, tests, gates, mutations, or measurement jobs.

VERDICT: CLEAN
