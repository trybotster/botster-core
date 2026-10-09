# Evidence for the Ghostty upstream sync of 2026-10-09

These files are the raw evidence for `docs/stage1/ghostty-upstream-sync-20261009.md`.

| File | What it is |
|---|---|
| `evidence.sh` | The run script. It runs inside a botster-core gate tree (`botster-gate -- bash docs/stage1/ghostty-upstream-sync-20261009/evidence.sh`): the heads, the Zig version, (1) the lib-vt build with an empty Zig global cache and the fetched packages against `build_data.rs`, (2) `zig build test-lib-vt --summary all`, (3) the binding tests. |
| `mac-run1-d1e747ca.log` | The Mac gate run at Core `d1e747ca`: (1) and (3) pass; (2) did not run (a test-only package fetch failed, `TlsInitializationFailed`). |
| `mac-run2-7095fb76.log` | The Mac gate run at Core `7095fb76` (the script retries the test packages' fetch): (2) and (3) pass; (1) did not run (a fetch from codeberg failed, `HttpConnectionClosing`). |
| `linux-run-7095fb76.log` | The Linux gate run at Core `7095fb76`: (1) and (3) pass; (2) cannot run without network. |

Each log is the whole `botster-gate` output; the jobq log path is in it.
