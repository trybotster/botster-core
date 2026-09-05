# Isolated Hub resize classification

Date: 2026-09-04. Implementer: Codex. Coordinator: root Codex. Reviewer: Fable.
Status: source approved; all reviewed focused validation checks passed after the coordinator released the window.
Results: `2026-09-04-isolated-hub-resize-validation.md`. The earlier hold and proposed commands below record preparation history.

## Source review and provenance follow-up

Fable approved the source patch in review commit `e211dae`. Executable verification remains pending.
Review: `/Users/jasonconigliari/botster-sessions/trybotster-botster-core-foundation-resize-review/docs/reports/2026-09-04-hub-resize-classification-patch-review.md`.
The coordinator retained the resource hold after that review.

The coordinator reports exporting Core with `git archive 5923bf1847979e2897796fadb9863183ffa5e3f1`, piped to tar in the isolated Core directory.
The coordinator separately exported Ghostty `eb72ec61304ea256be1d86ed8fa961c84e43ecbd` into Core's vendor directory.
I independently compared every tracked blob with its Git object identity from `git ls-tree -r -z`.
The comparison hashes file bytes with Git's blob header. For symbolic links, it hashes the link target.
All 338 Core blobs and all 5,846 Ghostty blobs matched. Core's Ghostty gitlink also matches the approved Ghostty commit.
The comparison excludes generated files, including `zig-out`. It verifies tracked content, not filesystem permission bits or generated artifacts.
The companion `2026-09-04-resize-export-provenance.json` records counts, revisions, and SHA-256 checksums of the exact Git tree listings.
This source inspection ran no build or test.

Before eventual execution, apply Fable's command cautions:

- Keep the wrapper's Node asset check. Record Node availability before using the wrapper.
- The wrapper adds `--workspace`. If Cargo rejects its combination with `-p botster-hub`, remove only the package selector.
- Require exactly one executed classification test with the unique filter.
- Record the selected `RUSTUP_TOOLCHAIN=1.97.0` and actual `rustc --version` in the results.
- Change only the busy class for the negative control. Restore that class immediately after the negative-control command.

## Source identity

- Replacement session: `sess-1788570969-003c-9a66d9610480b52a05aabcfbdc52f3aa`.
- Own worktree: `/Users/jasonconigliari/botster-sessions/trybotster-botster-core-foundation-nonblocking-resize-codex`.
- Own branch: `foundation/nonblocking-resize-codex`.
- Initial HEAD: `93acae3f98adbc21dc981d113c4eb2f31ead4ad0`; clean.
- Current HEAD: `5923bf1847979e2897796fadb9863183ffa5e3f1`; clean before this report.
- Hub source: `/private/tmp/botster-resize-downstream.g2c6fu`.
- Hub export base: `12b4a5482457fa9204f6c08916810abaaef682be`.
- Core export: `/private/tmp/botster-resize-core.0yQvP0`, approved Core `5923bf1847979e2897796fadb9863183ffa5e3f1`.

Core was already implemented and approved. I did not reimplement Core or edit Grok's frozen worktree.
I verified branch identity, clean status, and ancestry before the fast-forward.
The first attempt failed because the sandbox blocked Git's per-worktree metadata lock.
The coordinator authorized the normal sandbox escalation. The exact `git merge --ff-only 5923bf1847979e2897796fadb9863183ffa5e3f1` then succeeded.
I did not reset, force, remove locks, or change permissions.

## Change and scope

The coordinator approved `ExplicitResizeBusy(_) => "explicit_resize_busy"`.
The new arm preserves the exhaustive match. It adds no wildcard fallback.
The focused regression supplies private path text as the session identifier.
It requires the exact `explicit_resize_busy` class and preserves the exact `control_plane_failed` class.
These assertions exclude session text from the class and keep busy separate from control failure.

`managed_session_core_error_class` currently serves managed-session spawn diagnostics.
This regression proves classification only. It does not establish resize reachability through spawn.
Core still owns resize mechanics. Hub scheduling and retry behavior do not change.

Before editing, I compared isolated `src/runtime.rs` byte-for-byte with the recorded Hub commit. They matched.
Only isolated `src/runtime.rs` receives this patch. The diff below excludes pre-existing validation setup.
No Hub branch was committed, published, or merged. No active dependency pin was changed.
No active Hub ticket or TUI worktree was edited.

## Existing validation setup

The coordinator's temporary manifest already overrides all six Core packages with the approved Core export.
The temporary lockfile already reflects those local sources.
The existing test-support fixture loader accepts only the exact local protocol manifest path:
`/private/tmp/botster-resize-core.0yQvP0/crates/botster-terminal-protocol/Cargo.toml`.
It also requires a null Cargo source for that local path.
This fixture adaptation is validation setup, not a production change in this patch.
The isolated Zig cache and previously built matching worker remain in place.
I did not build or verify that worker during this task.

## Validation status and proposed commands

No builds, tests, Cargo commands, or formatting checks ran during patch preparation.
The coordinator must explicitly release the validation window before execution.
The previous adapted compile failed with `E0004` before any daemon test ran.
That prior failure is recorded in the coordinator's downstream validation report.
No passing compilation or runtime result is claimed for this patch.

After release, run from the isolated Hub directory with this environment:

```sh
unset CARGO_TARGET_DIR
export RUSTUP_TOOLCHAIN=1.97.0
export CARGO_BUILD_JOBS=2
export ZIG_GLOBAL_CACHE_DIR=/private/tmp/botster-resize-downstream.g2c6fu/zig-cache
rustc --version
```

First run the focused classification regression through the repository wrapper:

```sh
./test.sh --offline --locked -p botster-hub --lib runtime::tests::explicit_resize_busy_class_is_path_neutral_and_distinct_from_control_plane_failure -- --exact --nocapture
```

Require exactly one executed test. For a negative control, temporarily map only `ExplicitResizeBusy` to `control_plane_failed`.
Run the same test and require failure at the busy-class assertion. Restore the arm before further validation.
Do not run this temporary mutation concurrently with another validation command.

Before daemon checks, build the isolated Hub and confirm the matching worker in the default target directory:

```sh
cargo build --offline --locked -p botster-core-daemon --bin botster-session-worker
cargo build --offline --locked --bin botster-hub
./test.sh --offline --locked --test hub_daemon_lifecycle_test session_entity_subscription_pushes_snapshot_ordered_deltas_and_fresh_reconnect -- --exact --nocapture
./test.sh --offline --locked -p botster-hub --lib live_session_entity_subscription_emits_exact_stale_transition_patch -- --nocapture
./test.sh --offline --locked --test hub_daemon_lifecycle_test session_entity_subscription_observes_attached_natural_exit_with_pending_egress -- --exact --nocapture
```

These proposed checks cover acknowledged geometry, later input, stale-transition wake retirement, and attached exit with pending output.
They do not replace Core's delayed-sibling regression or a complete consumer matrix.
The coordinator controls later formatting, Clippy, broader gates, durable Hub integration, and dependency updates.

## Exact patch

```diff
--- a/src/runtime.rs
+++ b/src/runtime.rs
@@ -4208,4 +4208,5 @@
         CoreDaemonError::MissingModeFlagsResponse(_) => "missing_mode_flags_response",
         CoreDaemonError::ControlPlaneFailed(_) => "control_plane_failed",
+        CoreDaemonError::ExplicitResizeBusy(_) => "explicit_resize_busy",
         CoreDaemonError::BindTerminalAdapter(error) => match error {
             BindTerminalAdapterError::BindBeforeAttach { .. } => {
@@ -5388,4 +5389,17 @@
             managed_session_core_error_class(&generic),
             "runtime.spawn_failed"
+        );
+    }
+
+    #[test]
+    fn explicit_resize_busy_class_is_path_neutral_and_distinct_from_control_plane_failure() {
+        let session_id = SessionId("/private/session/resize-busy".to_string());
+        assert_eq!(
+            managed_session_core_error_class(&CoreDaemonError::ExplicitResizeBusy(session_id.clone())),
+            "explicit_resize_busy"
+        );
+        assert_eq!(
+            managed_session_core_error_class(&CoreDaemonError::ControlPlaneFailed(session_id)),
+            "control_plane_failed"
         );
     }
```

Patched `src/runtime.rs` SHA-256: `b37cbcb1ab63eec8fc28a4d7fb2f73cceef9338a02177960777354efa50b28e3`.
