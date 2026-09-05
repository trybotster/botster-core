# Registry identity validation

Date: 2026-09-04. Implementer: Codex. Coordinator: root Codex. Reviewer: Fable.

The focused registry tests and affected daemon tests pass after one test correction.
All three negative controls failed at their intended assertions. Strict daemon Clippy and workspace formatting pass after a separate authorized lint correction.
The original cleanup and Clippy failures remain in the evidence directory.
Full workspace and downstream acceptance remain pending.

## Revisions and environment

- Worktree: `/Users/jasonconigliari/botster-sessions/trybotster-botster-core-foundation-registry-identity`.
- Branch: `foundation/registry-identity`.
- Initial clean candidate: `cfc51fb7a7528e6c0c848a81375c514ff7a468e7`.
- Test termination correction: `3f29a8de1e8764ac6d134fa8a6cfb029319cb299`.
- Separate test lint correction and final source: `d10e57aafb0578857b033fbba0cdc857cf41a2b7`.
- Ghostty: `eb72ec61304ea256be1d86ed8fa961c84e43ecbd`.
- Rust: `1.97.0 (2d8144b78 2026-07-07)`.
- Cargo: `1.97.0 (c980f4866 2026-06-30)`.
- Zig: `0.16.0`, from the existing mise installation.

Every Cargo command used `BOTSTER_ENV=test`, `RUSTUP_TOOLCHAIN=1.97.0`, and `CARGO_BUILD_JOBS=2`.
The Rust 1.97 toolchain directory preceded the existing PATH. Commands used a non-login shell.
`CARGO_TARGET_DIR` was unset. This worktree used its own `target` directory.
The worker fixture invoked its existing nested `cargo build -p botster-core-daemon --bin botster-session-worker` command with the same environment.
No Cargo commands ran concurrently.

The coordinator released the source-only hold in message `msg_plugin-w_1788587909_2ff231`.
I prepared only this worktree's dependency with:

```sh
git submodule update --init --no-fetch crates/botster-terminal-ghostty/vendor/ghostty
```

Git cloned the declared dependency and checked out the exact gitlink revision.
The final submodule status is clean. Its native archive exists. Its source directory has no `.zig-cache` directory.
No frozen export, Hub file, active dependency pin, or main branch changed.
No new agent or pipeline was created.

## Commands and results

`2026-09-04-registry-validation-evidence.json` records every exact Cargo command, environment, exit status, result count, source hash, and evidence hash.
Logs are in `/private/tmp/botster-registry-validation`.
The table summarizes command results. Some groups overlap; counts must not be added as distinct test coverage.

| Log | Check | Result |
| --- | --- | --- |
| 01 | Registry unit tests | 11 passed |
| 02 | Exact-operation unit tests | 3 passed |
| 03 | Baseline collision identities | 1 passed |
| 04 | Foreign baseline identity | 1 passed |
| 05 | Worker restart collision, original | Cleanup assertion failed, exit 101 |
| 05b | Same test, temporary process diagnostics | Same failure, exit 101 |
| 05c | Same test, corrected termination check | 1 passed in 0.48 seconds |
| 06 | Legacy adoption rejection | 1 passed |
| 07–09 | Three negative controls | Each failed at its intended assertion, exit 101 |
| 10 | Daemon library after source restoration | 29 passed |
| 11 | Baseline integration group after restoration | 9 passed |
| 12 | Registry integration group | 12 passed |
| 13 | Oversized metadata adoption | 1 passed |
| 14, three logs | Persistence and shutdown failures | Each passed 1 test |
| 15 | Workspace formatting | Passed |
| 16 | Strict daemon Clippy, original | Existing lint failed, exit 101 |
| 17 | Strict daemon Clippy, authorized correction | Passed |
| 18 | Existing worker adapter test | 1 passed |
| 19 | Final workspace formatting | Passed |

The registry group includes the unreadable-record permission test and malformed-record scans.
The three persistence tests exercise read-only directory errors during persistence, resize, and shutdown.
The library group includes baseline work limits and exact-operation scan limits.
The expected panic in `pump_retention_rejects_foreign_route` passed as a `should_panic` test.

## Worker cleanup failure and correction

The original worker test reached its final cleanup assertion after both session records were removed.
Its five-second bound used `process_exists`, which runs `kill -0`.
That command also reports an unreaped zombie process as present.
A zombie is a terminated process whose parent has not collected its exit status.

One diagnostic rerun added temporary process logging at the same assertion.
A `finally` block restored the original source bytes after that command.
The diagnostic showed:

- Worker PIDs 4864 and 4866 had state `Z` and parent test PID 4857.
- PTY child PIDs 4865 and 4867 were absent.
- Both worker socket paths were absent.

The original runtime owns the spawned `Child` handles in `WorkerProcessSession.child`.
`release_for_restart` sets `release_on_drop`. `WorkerProcessRuntime::drop` then returns before waiting for these children.
Field destruction drops the handles. `WorkerProcessSession::drop` only closes the stall state.
The adopted runtime stores `child: None`, so it cannot wait on the original handles.
The test holds recorded PIDs and socket paths, not `Child` handles.
The diagnostic zombies remained after both daemon owners dropped and before the test process exited.
This is an existing limitation of restart simulation within one process. This registry change does not change production reaping.

The correction uses the existing `process_has_exited` helper:

```diff
-        process_exists(*worker) || process_exists(*child) || socket.exists()
+        !process_has_exited(*worker) || !process_has_exited(*child) || socket.exists()
```

The five-second bound, exact worker identities, adoption checks, sibling PTY echo, teardown sequence, and socket check remain unchanged.
The corrected test verifies worker and child termination plus socket removal. It does not prove that the runtime reaped worker PIDs.
A later process snapshot found none of the four diagnostic PIDs after test-process exit.
The final snapshot found no process whose executable came from this candidate's target directory.
These snapshots do not prove independent process-group cleanup.
No manual signal or kill command was used for cleanup.
I removed only the empty data directories left by the two failed test runs.
I did not change or stop unrelated processes.
The first unprivileged process snapshot failed because the sandbox denied `ps`; the later approved snapshots succeeded.

## Negative controls

Each control changed one production behavior temporarily. Each command compiled and reached its intended behavioral assertion.
Each `finally` block restored the exact original bytes before the next command.
The evidence includes each temporary diff, failure log, command, and original source hash.

1. Replace the private digest encoder with the old sanitizer. The filename assertion received `abc.json` instead of the full versioned SHA-256 filename.
2. Bypass validation in `remove`. The foreign-identity test failed because removal did not return `IdentityMismatch`.
3. Restore filename-stem identity in the baseline index. The baseline test returned no rows instead of `audit:a` and `audit_a`.

The old-encoder control produced unused-import warnings. Those warnings did not cause its failure.
The restored library and baseline groups passed after the controls.

## Existing lint failure

Strict daemon Clippy reported `clippy::collapsible_match` in `worker_bound_adapter_receives_ready_finish_without_drain_snapshots`.
The block came from commit `127f57ee` and existed in base `55d2b53`.
The coordinator authorized only the equivalent match guard in message `msg_plugin-w_1788588468_1c5bab`.
The fallback arm remains unchanged. The correction has its own commit, `d10e57a`.
Strict daemon Clippy and that exact existing test passed afterward.
The original failure remains in `16-daemon-clippy.log`.

## Window release and remaining gates

I released the validation window in message `msg_plugin-w_1788588578_cdbc0f` after all Cargo commands completed.
No further builds or tests are authorized by that completed window.
The coordinator owns the next validation window and the separate Hub consumer.

The following checks did not run here: full workspace tests, workspace Clippy, contract-only tests, doc tests, doc generation, Node smoke, and exact Hub consumer tests.
This report does not claim those gates passed.
The coordinator reported that Astra found no new source blocker and confirmed the production hashes and focused results.
Fable confirmed that the production source remains identical to approved `cfc51fb`.
This report update removes outdated test status from the implementation report. The coordinator retains integration authority.
