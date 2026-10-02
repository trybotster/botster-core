# Stage 1 pause status review

## Round 1

Reviewed PR #132, branch `stage1/pause-status`, head `fa19b34b33779c28814542810bb9ded021e3e88e`.
Scope: accuracy and information that a new agent needs to resume. This review covers documentation only.

Evidence sources:

- The state log at `~/botster-sessions/botster-v1-orchestrator-state.md`, through the 2026-10-02 09:46 entry.
- The pushed Core branch heads, read with `git ls-remote` on 2026-10-02.
- Core `v1` at `41ebcc05efe7c1c5a6e7209d35f91825453caac8`.
- Contracts BUILD.md at `59a16468ba220c6c1badc3d2bad5bb1c81153eba`, including the new Ghostty fork policy.
- Contracts at `a8db5c9a0f43fc440564989fd38a55121fdda38b`.
- The P1 gate log named in its handoff.

The named package and reviewer heads match the remote branches. The seven merge trees match their listed PR heads.
The contracts tags in the package Cargo.toml files match the status table. I ran no tests, builds, or gates.

### PS1 — MINOR — The P1 status omits the final gate result

Location: `docs/stage1-status.md`, sections 4 and 7, action 2.

The P1 row says "mutation 1161 → all closed" and lists "the fsync-test move to the slow tier" as remaining work.
The row also says that the gate was still running at the pause.

At this same reviewed head, the P1 handoff addendum says the gate "was cancelled at the pause, after 2543 s".
It records one TIMEOUT and two MISSED mutants as open. The gate log confirms these results and exit 125.
Core commit `ff376cceea6f68db75ca76ebbe1a574ea3ece5fd` already moved the real-disk storage tests to the slow tier.

Required change: update the P1 row with the cancelled gate result and the three open mutation results.
State that the storage-test move is complete. Remove that move from the remaining work and resume action 2.
Keep the requirement for a green gate after the fixes and delta review.

### PS2 — MINOR — The P2 sequence uses the old fork head and omits the final delta review

Location: `docs/handoffs/p2-libghostty.md`, "Order the lead set (do not skip steps)".

Step 1 says "Reviewer reviews fork `ada251c`". Step 4 says "Merge `origin/v1` into the branch, then run `botster-gate`".
The sequence gives no step for the required upstream sync before review and pin recording.
It also changes the binding head after CLEAN without an explicit delta review before the gate.
The opening branch section says to tell the reviewer that the head changed, but the numbered sequence omits the review itself.

BUILD.md at `59a1646`, "Ghostty fork policy", requires the sync before every pin move and before patching.
Its merge rule requires CLEAN on the exact head. Status section 7 correctly records the upstream sync as resume action 1.

Required change: replace the old sequence with the resume sequence from status section 7.
Review the surviving fork stack and binding at their new heads after the upstream sync.
Require a delta review after the Core branch incorporates `origin/v1`, before its exact-head gate and merge.

### PS3 — LOW — The P1 ownership list names the wrong repository

Location: `docs/handoffs/p1-lifecycle.md`, "Ids".

The handoff says "docs/stage1-clauses/p1-lifecycle.txt in the contracts repo".
The list belongs to botster-core on `stage1/plan`, as status section 1 correctly states.
It exists at plan commit `27ffaa4006c3f979fdceb897cbde09de6c25c15e`.

Required change: name botster-core, branch `stage1/plan`, as the source of this ownership list.

### PS4 — LOW — The P6 handoff treats the fixed driver defect as an open question

Location: `docs/handoffs/p6-testkit.md`, "What is left of scope 2".

The handoff says "Known open question for the driver" for `conf::a2_2_cancel_race_reports_the_real_outcome` and `await_pty`.
The state log records the accepted fix and tag `contracts-v0.1.13` at 09:34.
At contracts commit `a8db5c9`, the driver checks `pty_input` again after each pump before it declares the Core idle.
The P3 handoff also states that v0.1.13 fixes this defect.
P6 remains on v0.1.9, so its pin move remains necessary.

Required change: state that v0.1.13 contains the driver fix.
Replace the open-question instruction with the pin move and verification that P6 still needs.

VERDICT: NOT CLEAN (4 open)
