# Integration review: #196 (testkit registry_row, hold_start_at, release_start_at, payload_alive; branch stage1/p5-controls-start)

Reviewer: integration reviewer (Opus), `sess-1791517267-0168-401cc27284f329b42ec84da0de9d91e0`. This reviewer runs no
build, test or gate.

## Round 1 — CLEAN on head 2745781b

Reviewed head: `2745781b7c9bd74b1c6f65eb16f3e97726e39d0b`, four commits on v1 `1dd1657a` (the current v1). The stated tier is
HIGH by rule 3 (the shared testkit), which is correct.

- **Edge only.** `hold_start_at before: payload` makes the worker's edges keep the `SpawnPayload` action (`held_spawn`)
  while the instance is held. On release, the spawn runs at the worker's next turn, and its answer is the machine's
  `Spawned` input. The worker machine and the host do not change, so the AD-7 order that a transcript sees is the host's
  own. While the spawn is held, the worker has no ready input for it, so `edges_quiet` (#193) reports quiet, which is
  correct. `identity` and `running` give `Unsupported` (no minimum id needs them yet). The PR body raises `running` for a
  later ruling: the host writes a `Running` row only for an adoption (R-36).
- **The new state, on every end path.** `held_starts` (per run, by instance) loses the instance on release and in `ended()`.
  `held_spawn` is dropped in `ended()`. `payload_alive` is set at the spawn and cleared when the exit is observed
  (`poll_payload_exit`, also right after a signal), at `ReapPayload`, and in `ended()`. `payload_alive()` also refuses an
  ended worker. A hold on a session whose worker never spawns stays in the set, but an instance id is never reused, so
  nothing reads it. A dropped handle is refused by `session_row`.
- **`payload_alive`** follows its contract text (`docs/core-testkit-controls.md` at `contracts-v0.1.20`: "the kill of
  `Stop` reached the payload", EV-5(c)). The flag is false once the kill's exit is observed, not at the reap.
- **`registry_row`** decodes the stored row with the host's `Row::decode`, and it only reads.
- **The flips.** Against v1, `core-pending.txt` loses five ids. Their replacement-map proofs are `core-testkit+edge`
  (`ad_7_payload_launches_after_durable_identity`), `core-testkit+perturb` (`ev_5_poll_wakes_pending_work`,
  `tm_6_more_means_runnable_not_blocked`, `tm_6_poll_room_unparks_and_wakes`) and `core-testkit`
  (`ev_5_stop_kills_while_queue_full`). None is `slow:*`.
- **The gate log** (`controls-start-2745781b.log`) names the head and base `1dd1657a`. The conformance binary reports 31
  passed and 644 ignored. The default tier runs 986 tests and the slow tier 243, all pass. Mutants: 24 caught, 0 missed,
  4 unviable. Exit 0.

Condition: #195 (`46642d85`, CLEAN here) and this PR both change `ProcessCell`, `controls.rs` and `worker.rs`, and
`git merge-tree --write-tree 2745781b 46642d85` conflicts in `controls.rs` and `worker.rs`. The PR that merges second needs
a review of its union resolution (both fields, both registrations, both end paths) and a full gate. Base-merge-check cannot
carry this CLEAN to that merge.

VERDICT: CLEAN (0 open) at 2745781b7c9bd74b1c6f65eb16f3e97726e39d0b
