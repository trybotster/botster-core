# Isolated Hub resize validation

Status: all approved checks passed. The negative control failed at its intended assertion.
The coordinator released this validation window in message `msg_plugin-w_1788585001_e35f02`.
The Hub implementer holds other Hub builds during this window.

## Inputs and environment

- Hub export base: `12b4a5482457fa9204f6c08916810abaaef682be`.
- Hub directory: `/private/tmp/botster-resize-downstream.g2c6fu`.
- Core export: `/private/tmp/botster-resize-core.0yQvP0`, revision `5923bf1847979e2897796fadb9863183ffa5e3f1`.
- Ghostty revision: `eb72ec61304ea256be1d86ed8fa961c84e43ecbd`.
- Adaptations: approved classification patch, existing six local Core overrides, existing lockfile, and exact-path protocol fixture loader.
- Evidence directory: `/private/tmp/botster-resize-downstream.g2c6fu/foundation-validation`.

Every validation command uses the isolated Hub directory and this environment:

```sh
export RUSTUP_TOOLCHAIN=1.97.0
export CARGO_BUILD_JOBS=2
export ZIG_GLOBAL_CACHE_DIR=/private/tmp/botster-resize-downstream.g2c6fu/zig-cache
unset CARGO_TARGET_DIR
```

Preflight recorded `rustc 1.97.0 (2d8144b78 2026-07-07)`, `cargo 1.97.0 (c980f4866 2026-06-30)`, Node `v22.21.1`, and Zig `0.16.0`.
`preflight.log` records the exact versions and SHA-256 checksums.

| File | SHA-256 before validation |
| --- | --- |
| `src/runtime.rs` | `b37cbcb1ab63eec8fc28a4d7fb2f73cceef9338a02177960777354efa50b28e3` |
| `Cargo.toml` | `72f178e96896468073ead0eee5e531bdea50606863638a009aaeca454b52f0eb` |
| `Cargo.lock` | `dab42e115081ac8ab32d206ef634dab6c0b8a78e5af29cb8f19c06f24346da06` |
| `crates/botster-hub-test-support/build.rs` | `0d41c901e4fac9490d6a16f76f195de47647ad8885a9b57ba385b18629696abb` |

The provenance companion report previously verified all 338 Core blobs and all 5,846 Ghostty blobs against their Git objects.
Generated files do not establish source provenance.

## Classification baseline

```sh
./test.sh --offline --locked -p botster-hub --lib runtime::tests::explicit_resize_busy_class_is_path_neutral_and_distinct_from_control_plane_failure -- --exact --nocapture
```

Log: `foundation-validation/classification-green.log`. Exit record: `foundation-validation/classification-green.exit`.
The wrapper's Node asset check runs its own `cargo run --quiet -p botster-hub-test-support --example node_package_assets -- <temporary-directory>`.
That nested command inherits the selected toolchain and two-job limit. The wrapper does not pass its `--offline --locked` arguments to this nested command.
The Node asset check reported that package assets are current. Cargo then started library test compilation.

The command exited zero. Exactly one test passed; other workspace library targets executed zero tests.
Cargo reported 1m 47s for test compilation and 0.00s for the classification test.
The wrapper accepted `--workspace` with `-p botster-hub`; no fallback command was needed.

## Negative control and restoration

I changed only this production class in the isolated file:

```diff
-        CoreDaemonError::ExplicitResizeBusy(_) => "explicit_resize_busy",
+        CoreDaemonError::ExplicitResizeBusy(_) => "control_plane_failed",
```

I ran the same classification command. The command exited 101 with exactly one failed test.
The assertion failed at `src/runtime.rs:5397:9`:

```text
  left: "control_plane_failed"
 right: "explicit_resize_busy"
```

The Node asset check passed. Compilation took 3.63s; test execution took 0.00s.
A Python `finally` block restored the original bytes immediately after the command returned.
`classification-restored.sha256` confirms the approved `b37cbcb1...` source hash.
No other validation command overlapped the temporary mutation.
Logs: `classification-negative.log`, `classification-negative.exit`, and `classification-restored.sha256` in the evidence directory.

## Matching binary builds

These commands ran sequentially:

```sh
cargo build --offline --locked -p botster-core-daemon --bin botster-session-worker
cargo build --offline --locked --bin botster-hub
```

Both commands exited zero. The worker build took 12.59s; the Hub build took 36.75s.
The build logs resolve the Core crates from the approved isolated export.
Logs: `worker-build.log`, `worker-build.exit`, `hub-build.log`, and `hub-build.exit`.

Both binaries resolve beneath `/private/tmp/botster-resize-downstream.g2c6fu/target/debug`.
The worker SHA-256 remained `23951ace053cbd10b886d2b1c828cd9a944213feba623707950b0aebde463d49`.
The initial explicit Hub build SHA-256 was `e7431a5940dc7289da76deec74fc4cd09f3f99f3e64ba2326d47c1c421852136`.
The subsequent Cargo test command rebuilt the Hub package before daemon execution.
The final Hub SHA-256 is `d1ca8a3ac4a5b4322872dec3819a08316f0eae0a4711fb36dffda9a3296247df`.
The initial explicit build hash does not identify the later test-built Hub artifact.
`binaries-before-daemon.json` and `final-hashes.json` retain both observations and exact realpaths.

## Downstream checks

The commands ran sequentially through the repository wrapper:

```sh
./test.sh --offline --locked --test hub_daemon_lifecycle_test session_entity_subscription_pushes_snapshot_ordered_deltas_and_fresh_reconnect -- --exact --nocapture
./test.sh --offline --locked -p botster-hub --lib live_session_entity_subscription_emits_exact_stale_transition_patch -- --nocapture
./test.sh --offline --locked --test hub_daemon_lifecycle_test session_entity_subscription_observes_attached_natural_exit_with_pending_egress -- --exact --nocapture
```

| Check | Exit | Executed tests | Test duration | Log |
| --- | --- | --- | --- | --- |
| Entity resize, input, and reconnect | 0 | 1 passed | 3.11s | `entity-resize.log` |
| Stale-transition wake retirement | 0 | 1 passed | 0.06s | `stale-transition.log` |
| Attached natural exit with pending output | 0 | 1 passed | 2.85s | `attached-exit.log` |

Each log has a companion `.exit` file. Each Node asset check passed before Cargo tested the selected target.
The stale-transition filter executed only its intended test; other workspace targets executed zero tests.
The daemon harness called its matching worker prebuild during each daemon test; those commands also exited zero.
The first daemon target compiled in 27.72s. Stale-transition library compilation took 3.88s. The final daemon target was current in 0.16s.

The daemon commands used approved sandbox escalation for local sockets, PTYs, process inspection, and harness cleanup.
Process inspection initially failed inside the sandbox. The normal read-only escalation then succeeded.
No automatic approval rejection occurred.

## Final source and cleanup checks

`final-hashes.json` matches all four source/adaptation hashes in the preflight table.
The busy-class source restoration is complete. The manifest, lockfile, and fixture-loader adaptations did not change.

I compared all tracked source blobs with the recorded Git objects after the builds and tests:

- All 338 Core blobs match approved `5923bf1`.
- All 5,846 Ghostty blobs match `eb72ec6`.
- Of 548 Hub blobs, only the four recorded adaptations differ from `12b4a54`.

`final-source-provenance.json` records those counts, exact revisions, differing paths, and Git tree-listing checksums.
This comparison hashes tracked bytes and symbolic-link targets. It excludes generated files and does not verify filesystem mode bits.

The daemon tests use the existing panic-safe harness and its bounded cleanup paths.
The stale-transition unit test explicitly shuts down its session, stops its daemon, and removes its temporary data directory.
All selected tests returned normally. No timeout or manual process termination was needed.

I recorded a process baseline before daemon execution. I recorded another snapshot after each downstream check.
Each later snapshot contains zero processes whose executable comes from the isolated target directory.
The snapshots retain PID, parent PID, process group, process state, and executable name.
The final baseline comparison includes unrelated browser and system processes. I did not signal them.
These external snapshots prove isolated executable absence at those observations; they are not independent process-group membership proofs for every PTY child.
The daemon harness supplies its own owned-child cleanup checks. I do not claim an additional full process-group audit.

Evidence files: `process-baseline.txt`, `process-after-entity-resize.txt`, `process-after-stale-transition.txt`, `process-after-attached-exit.txt`, and `process-new-since-baseline.txt`.

I explicitly returned the validation window to the coordinator in message `msg_plugin-w_1788585464_a9817b`.
No validation command remained running when I sent that message.
No extra suite ran. No active ticket pins, merges, commits, or publication changed.

## Scope of the result

The classification regression and negative control prove the exact diagnostic mapping only.
The three downstream checks provide the reviewed representative consumer evidence for approved Core `5923bf1` and isolated Hub `12b4a54` plus the recorded adaptations.
This result is not a full Hub, Web, TUI, or Project Pipelines matrix.
The coordinator owns durable Hub integration and any later dependency update.
