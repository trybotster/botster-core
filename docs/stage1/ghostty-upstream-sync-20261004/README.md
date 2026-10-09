# Evidence for the Ghostty upstream sync of 2026-10-04

These files are the raw evidence for `docs/stage1/ghostty-upstream-sync-20261004.md`.

| File | What it is |
|---|---|
| `mac-zig.sh` | The Mac run script for the recorded runs: it prints the head, the count of tracked changes, the Zig path and version, the Build Summary of `zig build test-lib-vt --summary all`, and the exit status of each step. |
| `mac-sync-c370ef4d9.log` | The first Mac run of the synced stack `c370ef4d9` (`botsterq run --exclusive`, botsterq exit 0). An earlier version of `mac-zig.sh` produced it: that version printed only the last 60 lines of the test output, so the log has the exit statuses (both 0) but no Build Summary line. |

Pending (lead HOLD on heavy jobs, 2026-10-04): the recorded Mac runs of `779907e0e` and `c370ef4d9`, the empty-cache
Zig package list, and the Linux runs.
