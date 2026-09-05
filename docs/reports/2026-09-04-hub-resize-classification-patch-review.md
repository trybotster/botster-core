# Review: isolated Hub resize classification patch

Date: 2026-09-04. Reviewer: Fable (session `sess-1788561640-0030-9ab4035a5a3867e8f4970b3a6e440510`). Read-only source review. No builds, tests, or format commands were run. The temporary integration file and the active ticket were not edited.

Patch report: `botster-core-foundation-nonblocking-resize-codex/docs/reports/2026-09-04-isolated-hub-resize-classification.md`.
Target: `/private/tmp/botster-resize-downstream.g2c6fu/src/runtime.rs` over Hub `12b4a5482457fa9204f6c08916810abaaef682be`, with all six Core packages patched to the export of approved Core `5923bf1847979e2897796fadb9863183ffa5e3f1`.

## Source-review verdict

**Approved as source.** Executable verification remains pending and is reported separately below.

## Scope

- The export is not a git checkout. I diffed the patched `src/runtime.rs` against `git show 12b4a54:src/runtime.rs`. Exactly the two hunks in the patch report appear: the match arm at `:4210` and the test at `:5394-5405`. Nothing else in the file differs.
- I compared every tracked file under `src` and `tests` at `12b4a54` with the export. Only `src/runtime.rs` differs.
- SHA-256 of the patched file matches the report: `b37cbcb1ab63eec8fc28a4d7fb2f73cceef9338a02177960777354efa50b28e3`.
- `Cargo.toml` lines 60-66 patch the six Core crates to the Core export path. The export contains the `ExplicitResizeBusy` variant and the `session_is_stopping_or_exited` guard, which are the `5923bf1` markers.

## Correctness

- `managed_session_core_error_class` (`:4168`) is an exhaustive match over `CoreDaemonError`. Core `5923bf1` adds `ExplicitResizeBusy(SessionId)` as the last variant, so the match fails to compile without an arm. This was the recorded `E0004` downstream failure. The new arm restores exhaustiveness with no wildcard, so the next Core variant will again fail loudly. Correct.
- The class string is a literal with no session text. The function's existing tests already require class strings without path characters. The new test uses a path-like session id and asserts exact equality for both `explicit_resize_busy` and `control_plane_failed`, which proves both redaction and separation from control failure. Correct.
- Reachability: the only production caller is the spawn diagnostics path at `:2172`. Spawn cannot produce `ExplicitResizeBusy`, so the arm is compile-driven, not behavior-driven. The report says so. No Hub scheduling, retry, or terminal semantics change. This keeps Core as the owner of resize mechanics.

## Proposed commands

- The classification test command is correct in intent. One caution: the wrapper always passes `--workspace`, and the command adds `-p botster-hub`. If Cargo rejects that combination on the pinned toolchain, drop `-p botster-hub`. The test name filter is unique across the workspace, so the run still executes exactly one test.
- The negative control is valid: mapping only `ExplicitResizeBusy` to `control_plane_failed` fails the first assertion and leaves the second passing, which isolates the busy class. Restore the arm before any other command.
- The wrapper runs a Node asset check before Cargo. The environment block does not mention Node. Hub `12b4a54` has no `rust-toolchain` file, so `RUSTUP_TOOLCHAIN=1.97.0` is a coordinator choice and should be recorded with the results.
- The three daemon and lifecycle tests cover acknowledged geometry with later input, stale-transition wake retirement, and attached natural exit with pending egress. That is the representative set the plan asked for. They do not replace Core's delayed-sibling regression, which already passed on `5923bf1`.

## Pending executable verification

Not run, by instruction. Required before acceptance, in order: the classification test (one executed test), the negative control (fails at the busy assertion, then restored), the worker and Hub builds from the same Core export, then the three daemon and lifecycle tests. Record exit statuses and the toolchain.

## Note on provenance

The Core export directory has no git metadata. Its identity rests on content markers and the coordinator's record. Record how the export was produced from `5923bf1` (for example the archive command and a tree checksum) alongside the validation results.

## Executable verification review (read-only inspection of Codex's run)

Inspected: `/private/tmp/botster-resize-downstream.g2c6fu/foundation-validation` (24 files), Codex's `2026-09-04-isolated-hub-resize-validation.md`, `2026-09-04-resize-export-provenance.json`, `2026-09-04-resize-validation-evidence.json`, and `final-source-provenance.json`. No builds or tests were run by me.

**Downstream approval: granted for the stated scope.** No remaining finding.

Checks performed:

- Test selection. `classification-green.log` ran exactly one test, `runtime::tests::explicit_resize_busy_class_is_path_neutral_and_distinct_from_control_plane_failure`, 1 passed, with every other workspace library target at 0 executed. The wrapper accepted `-p botster-hub` with `--workspace`. `classification-negative.log` fails that one test at `src/runtime.rs:5397` with left `control_plane_failed`, right `explicit_resize_busy`, exit 101. `entity-resize.log`, `stale-transition.log`, and `attached-exit.log` each ran exactly the intended test, 1 passed, exit 0.
- Restoration. `classification-restored.sha256`, `final-hashes.json`, and my own `shasum` of `src/runtime.rs` all give `b37cbcb1...`, the approved patch hash. The arm reads `explicit_resize_busy` at `:4210`.
- Runtime inputs. `worker-build.log` compiles the six Core crates from `/private/tmp/botster-resize-core.0yQvP0`. `Cargo.toml` patches all six Core crates to that export. I compared 178 Rust and manifest files in the export against Core `5923bf1`: no differences. Codex's provenance record checked all 338 tracked Core blobs and 5,846 Ghostty blobs with no mismatches, and the Ghostty gitlink is `eb72ec6`; I spot-checked two Ghostty files against that commit. The Hub tree differs from `12b4a54` in exactly the four recorded adaptations: `Cargo.toml` (Core patch table), `Cargo.lock`, `crates/botster-hub-test-support/build.rs` (fixture loader accepts the exact export manifest path), and `src/runtime.rs` (the approved patch). I read the `build.rs` and `Cargo.toml` diffs; both are validation setup only.
- Evidence integrity. All 24 hashes in Codex's evidence index match the files on disk.
- Reporting scope. The validation report claims the classification mapping plus three representative consumer checks for Core `5923bf1` over isolated Hub `12b4a54`. It states that this is not a full Hub, Web, TUI, or Project Pipelines matrix and names no later Hub revision. The changed Hub binary hash between the explicit build and the test rebuild is recorded honestly, with both hashes.
- Cleanup. Process snapshots after each daemon check show no executables from the isolated target directory. Codex correctly limits that claim to those observations.

Note: the four Codex reports are untracked files in the Codex worktree, whose HEAD is the approved `5923bf1`. The coordinator should decide where those reports are committed.

Addendum: Codex committed the four reports as `55d2b539e40df99488c8119f8f2e318b623eeef2` on `foundation/nonblocking-resize-codex`. I verified that the commit adds only the four files under `docs/reports/` over approved Core `5923bf1` (427 insertions, no source change), the worktree is clean, all 24 evidence hashes still match the files on disk, and the isolated Hub `src/runtime.rs` still hashes to the approved `b37cbcb1...`. The downstream approval above covers this commit.
