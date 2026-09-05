# Registry identity broad validation

Date: 2026-09-05. Implementer: Fable. Coordinator: root. Reviewer: Codex.

The remaining CI gates from `.github/workflows/ci.yml` pass on the final source `5726599`.
Reaching that state needed four separate approved commits: two test-only corrections, one dependency-provenance commit, and one formatting-only commit.
Production daemon sources did not change. Two evidence defects are recorded below.
Exact Hub consumer acceptance is separate and is not established here.

## Revisions and environment

- Worktree: `/Users/jasonconigliari/botster-sessions/trybotster-botster-core-foundation-registry-identity`, branch `foundation/registry-identity`.
- Frozen input, focused validation complete: `3b0b51374a6e54659e2f4649bab37cca532b94a5`.
- Clippy test-lint correction: `3da05cbc8653ad87984ae3da604a38b721660fbe`.
- Parked-exit test correction: `af798291b08f161de981827458e7926a27cc1a80`.
- Consumer fixture lock provenance: `c1ab0ffccb974aedce46278765eda19cd87119ca`.
- Formatting correction and final source: `57265996877349a69d3bef9a02e2222dac0bd868`.
- Production daemon sources unchanged since the approved `cfc51fb` candidate: `registry.rs` sha256 `c32c58c0…`, `daemon.rs` sha256 `41a296f5…`.
- Ghostty `eb72ec61304ea256be1d86ed8fa961c84e43ecbd`. Rust 1.97.0. Node 22.21.1. Zig 0.16.0 from mise, resolved by the ghostty build script through `HOME`.
- Every command: non-login zsh, `BOTSTER_ENV=test`, `RUSTUP_TOOLCHAIN=1.97.0`, `CARGO_BUILD_JOBS=2`, `CARGO_TARGET_DIR` unset, worktree `target` reused.
- One arm at a time, only inside root-assigned windows, never concurrent with Web or Hub Cargo or Node work. A pre-run guard checked HEAD, a clean tree, and no owned build process before each arm.

## Commands and results

`2026-09-05-registry-broad-validation-evidence.json` records every command, UTC start, exit status, source revision, PIDs, counts, and log hash.
Logs are in `/private/tmp/botster-registry-broad-validation`. Failed logs are preserved unchanged. Reruns use a numbered `-rerunN` suffix.
Counts from different arms overlap and must not be added.

| Log | Source | Check | Result |
| --- | --- | --- | --- |
| 01 | 3b0b513 | Workspace Clippy, strict | `clippy::useless_conversion` in botster-core test code, exit 101 |
| 01-rerun | 3da05cb | Workspace Clippy after correction | Passed in 10.80 s; log body later overwritten, see below |
| 02 | 3da05cb | Workspace tests | 804 passed, 1 failed (`session_registry_state_does_not_reconcile_parked_exit`), exit 101; later crates and doc tests not reached |
| 00 | af79829 | Exact corrected parked-exit test | 1 passed |
| 02-rerun | af79829 | Workspace tests, CI-exact | 79 binaries and 7 doc-test sections: 984 passed, 0 failed, 1 ignored (opt-in `many_pty_load_100`) |
| 03 | c1ab0ff | Contract-only core library | 41 passed |
| 04 | c1ab0ff | Doc tests, explicit CI command | 9 passed (also covered inside 02-rerun) |
| 05 | c1ab0ff | `cargo doc --workspace --no-deps` | Passed |
| 06 | c1ab0ff | Node terminal-protocol smoke | Passed; no network block |
| 01-rerun (second write) | c1ab0ff | Workspace Clippy | Passed in 0.72 s |
| 07 | c1ab0ff | `cargo fmt --all -- --check` | One hunk at the Clippy correction site, exit 1 |
| 07-rerun2 | 5726599 | `cargo fmt --all -- --check` | Passed |
| 01-rerun2 | 5726599 | Workspace Clippy, final source | Passed in 9.15 s |
| 08 | 5726599 | `hub-lifecycle-shaped` fixture, `cargo test --quiet --offline --locked` | 9 passed |
| 09 | 5726599 | `hub-data-plane-shaped` fixture, `cargo test --quiet --offline --locked` | 2 passed |

Arms 02 through 06 ran on identical Rust tokens to `5726599`; the last two commits changed only lock files and whitespace. Root did not require a full workspace rerun for them.

## Existing Clippy lint

Strict workspace Clippy on `3b0b513` failed on one pre-existing `clippy::useless_conversion` in `crates/botster-core/src/engine/plugin_worker.rs` test code (commit `bb334d7`, 2026-08-13, present in base main `55d2b53`).
Root approved removing only the redundant `.into_iter()`; commit `3da05cb`. Test behavior is unchanged.
That hand-formatted edit left a five-line chain that rustfmt collapses to one line; arm 07 caught it and root approved the formatting-only commit `5726599`.

## Parked-exit test failure and correction

`session_registry_state_does_not_reconcile_parked_exit` (commit `8fce204`, 2026-08-18, present in base main, unchanged by this branch) failed once under full-workspace parallel load at its positive assertion: after one `observe_session_lifecycle` pass the record was still `Running`.

Cause, from source. The fixture waits only for OS-level termination of the PTY child.
The local runtime queues `ProcessExited` only after its reader thread observes EOF on the PTY master: in `local_process.rs`, `drain_output` gates `queue_exit_output` on `reader_finalization_complete`, which requires `reader_finished`, set by the reader loop when `read()` returns `Ok(0)`.
One observe pass between kernel exit and reader EOF harvests the exit code but publishes no exit event.
The second argument of `observe_session_lifecycle` is `now_seconds`, a timestamp, not a budget; the caller receives no pending outcome.
Every sibling immediate-exit test loops until publication; this one did not.

Root approved a test-only correction, commit `af79829`: the positive half observes with the existing `wait_for_condition` helper and its unchanged 180 s timeout until the runtime reconciles the parked exit.
The three negative non-mutating assertions still run first. The final Exited, journal-wake, and lifecycle-page assertions are unchanged.
This establishes a reachable fixture race. It does not by itself attribute every historical failure.
The original failure log and its data directory `…/T/botster-core-daemon-exact-registry-state-parked-1788593590261587000` are preserved.

## Consumer fixture lock drift

The registry change added `sha2` to `botster-core-daemon`. The `hub-lifecycle-shaped` and `hub-data-plane-shaped` consumer fixtures under `botster-core-test-support` depend on the daemon by path and carry their own committed `Cargo.lock` files, which the branch had not updated.
Their tests run `cargo test --quiet --offline` inside the fixture without `--locked`, so the arm 02 rerun on `af79829` regenerated both locks, adding one `sha2` dependency edge each. The tests passed.

Reporting correction. The runner's `status-after` snapshot recorded the two modified files at the end of the arm 02 rerun, but the boundary report to root said the tree was clean. That statement was wrong. The actual snapshot is preserved as `status-after-arm02-rerun-actual.txt`.
Root approved committing exactly the two Cargo-generated lines as commit `c1ab0ff`. Both fixtures then passed under `--offline --locked` (arms 08 and 09), and both lock hashes were unchanged at the end of the run.

## Evidence defect: overwritten passing log

The final Clippy run on `c1ab0ff` overwrote `01-clippy-workspace-rerun.log`, the log of the `3da05cb` Clippy pass, because the runner reused a single `-rerun` suffix.
The body of the `3da05cb` pass log is not available. Its sha256 `3dda38921c15c018881c24de1b69ce8e12882ecba3b93cb7599470f34387e533`, exit status 0, PIDs, and summary (Finished in 10.80 s, no lint output) were recorded before the overwrite and are in the manifest.
No failure log was affected. The runner now refuses to open an existing log or pid destination and runs with `noclobber`.

## Observed warnings, not gated

- Arm 03, contract-only build: 21 rustc unused-import and dead-code warnings in `botster-core` lib (20 in its lib test, 16 duplicates). CI does not gate this step on warnings. This branch changes no `botster-core` library source, so they are pre-existing under `--no-default-features`.
- Arm 05, `cargo doc`: 4 rustdoc warnings for public docs linking private items in `botster-core`. Not gated in CI. Pre-existing for the same reason.

## Cleanup

Process snapshots before and after arms are in the evidence directory, including `processes-final.txt`. No process owned by this worktree's `target` remained after any arm. The Node smoke script removed its own temp directories.
One pre-run guard blocked on an unrelated five-day-old `npm exec vite` process because of an over-broad pattern; the pattern was narrowed and the process was not signaled. No unrelated process was signaled at any point. No manual kill was used.

## Remaining

Exact Hub consumer acceptance is separate and not established by these gates. The coordinator owns integration.

## Manifest capture semantics and final state

The evidence manifest was generated before its own commit, while the report and manifest files were untracked. Its `git_status_clean` value of `false` is the true value at that capture time and is preserved unchanged.
`2026-09-05-registry-broad-validation-final-state.json` is a separate, timed post-commit record. It captures the verified HEAD and clean status after the documentation commit `d4b698a`, and it is committed on its own so the capture time and content stay distinct from the manifest.
