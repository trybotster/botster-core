# Evidence for the Ghostty upstream sync of 2026-10-09

These files are the raw evidence for `docs/stage1/ghostty-upstream-sync-20261009.md`.

| File | What it is |
|---|---|
| `evidence.sh` | The run script. It runs inside a botster-core gate tree (`botster-gate -- bash docs/stage1/ghostty-upstream-sync-20261009/evidence.sh`) and reads the build options and the package list from `build_data.rs`. It prints the heads, the Zig version and the project-local `zig-pkg/` state (also before and after (2b), which runs with `zig-pkg/` moved out), then: (1) the lib-vt build with an empty Zig global cache and the fetched packages against `ZIG_PACKAGES` (needs network); (2a) `zig build test-lib-vt --summary all` in upstream's default configuration (needs network); (2b) `test-lib-vt` with the shipped options in Debug, from `ZIG_PACKAGES` only; (3) the binding tests. |
| `mac-run4-6e4a55aa.log` | **Final Mac run** at Core `6e4a55aa`: (2a), (2b) and (3) pass, with the step trees. `zig-pkg/` (39 entries, from step 2a) is moved out before (2b). (1) did not finish: a fetch from codeberg failed, `HttpConnectionClosing`. |
| `linux-run3-6e4a55aa.log` | **Final Linux run** at Core `6e4a55aa`: (2b), the 7-package proof with no network, and (3) pass, with the step tree; (1) and (2a) do not run, because the gate has no network. |
| `mac-run3-2c636dbe.log` | The Mac run at Core `2c636dbe`: every step passes. Its step (1) is **the empty-cache proof** (`zig-pkg/` absent at the start). Its (2b) ran after (2a) in the same fork tree, so it is not a package proof (F-A6-04). |
| `linux-run2-2c636dbe.log` | An earlier Linux run at Core `2c636dbe`: (2b) and (3) pass; (1) and (2a) do not run. |
| `linux-test-emit-lib-vt-b10a1dda.log` | A Linux check at Core `b10a1dda`: `test-lib-vt -Demit-lib-vt` alone fails, because it still needs the `highway` package for SIMD. |
| `linux-test-binding-options-b10a1dda.log` | A Linux check at Core `b10a1dda`: `test-lib-vt` with the shipped options passes from the package store. This check found the configuration of step (2b). |
| `mac-run1-d1e747ca.log` | An earlier Mac run at Core `d1e747ca`: (1) and (3) pass; (2) did not run (a test-only package fetch failed, `TlsInitializationFailed`). |
| `mac-run2-7095fb76.log` | An earlier Mac run at Core `7095fb76`: (2) and (3) pass; (1) did not run (a fetch from codeberg failed, `HttpConnectionClosing`). |
| `linux-run-7095fb76.log` | An earlier Linux run at Core `7095fb76`: (3) passes. (1) reports exit 0 with no package fetched, so it is not an empty-cache check (see the record). (2) cannot run without network. |

Each log is the whole `botster-gate` output; the jobq log path is in it. Each of the two `b10a1dda` checks ran one
`zig build test-lib-vt` command (it is on the log's `command` line), with a global cache seeded from the whole package
store. The final step (2b) seeds only `ZIG_PACKAGES`.
