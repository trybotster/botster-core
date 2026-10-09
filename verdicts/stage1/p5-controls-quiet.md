# Integration review: #193 (testkit edges_quiet, the fence of await_quiet; branch stage1/p5-controls-quiet)

Reviewer: integration reviewer (Opus), `sess-1791517267-0168-401cc27284f329b42ec84da0de9d91e0`. This reviewer runs no
build, test or gate.

## Round 1 — CLEAN on head 7d75b70f

Reviewed head: `7d75b70f9fc4d8fc0355d07300a00a7d4706ef29`, base v1 `dc7758dc` (the current v1, #191 merged). The stated tier
is HIGH by rule 3 (the shared testkit), which is correct.

- **The merges** (`fecbc378` of #191's `7d240117`, `b6437670` of #191's `7a56100c`, `7d75b70f` of v1 `dc7758dc`): each tree
  is the tree of `git merge-tree --write-tree` of its parents. `e42b75a9` moves #191's PC-F1 fix onto the new `handles`
  map: `drop_handle` removes the handle's directory and its process table together.
- **`edges_quiet`** reads state and changes nothing (no poll of Core, no clock move). It is quiet when no worker of the
  run has ready work, the host's process table has no exit that it has not polled, and no worker's link holds bytes, a
  descriptor or an end of file that the host has not read (`EndControl::holds_for_peer`, which counts nothing toward a
  closed peer). The other host edges of the testkit are synchronous (`write_row`, `delete_row`, `read_rows`, `fill_random`,
  `spawn_worker`). A spawned worker's link waits in `SimEdges::pending` until `accept_link`, and its hello is counted as
  unread. So the fence covers every edge that can hold a report today. The route transport (P4a) does not exist yet. When
  it lands, it must add its own term here.
- **`no_spurious_wakes`** is registered as `Unsupported` with its reason. An unregistered control gives the same outcome,
  so the registration only records the reason.
- **The flip.** Against v1, `core-pending.txt` loses only `conf::ev_5_no_mandatory_event_lost_or_replaced`. Its
  replacement-map proof is `core-testkit`, so plan 23a lets it leave pending.
- **The gate log** (`controls-quiet-7d75b70f.log`) names the head and base `dc7758dc`. The conformance binary reports 25
  passed and 650 ignored (v1's 24 and `ev_5`). The default tier runs 971 tests and the slow tier 243, all pass. Mutants:
  29 caught, 0 missed, 0 timeout, 3 unviable. Exit 0.

VERDICT: CLEAN (0 open) at 7d75b70f9fc4d8fc0355d07300a00a7d4706ef29
