# Integration review: #184 (ci/high-tier-paths.txt, BUILD.md risk tiers rule 5; branch stage1/p3-high-tier-paths)

Reviewer: integration reviewer (Opus), `sess-1791517267-0168-401cc27284f329b42ec84da0de9d91e0`. This reviewer runs no
build, test or gate.

## Round 1 — CLEAN on head b1992a0f

Reviewed head: `b1992a0f349466deef5b3aef75ce4b9595a637c2`, one new file (`ci/high-tier-paths.txt`, 36 lines) on v1
`b520f832`. v1 is now `aaac0c0d` (#183 merged); `git merge-tree --write-tree origin/v1 b1992a0f` has no conflict. The
stated tier is HIGH by rule 1 (a change to a HIGH-path list is gate-decision code), which is correct. P3's gate log:
`…-p3-high-tier-paths-b1992a0f-pool-20261009-083957-49200.log` (no Rust change). This reviewer did not read it.

The rule is BUILD.md "Risk tiers" rule 5 (contracts main `56bd0a5`): the list holds process control (spawn, signal,
wait, reap), the PTY, fd handoff, adoption and restart, durable storage, and auth, token and proof checks.

### Checked, no finding

- **Every entry exists at the head** and names a production file. This reviewer listed the production sources of the
  crates in those areas (`botster-core-sys`, `botster-core`, `botster-worker`, `botster-worker-core`, `botster-core-link`,
  `botster-core-host`, `botster-guardian-core`) and checked each one against the areas:
  - **Process control.** `signal.rs`, `process.rs`, `payload.rs` and `real.rs` are listed. The host's decision to signal
    (`flows.rs:633`, `IdentityState::Matches` before `SignalGroup`) is not listed, and P3 calls that out. This reviewer
    accepts it for one reason: the real edge checks again at the moment of the signal. `Children::signal_group`
    (`process.rs:186-191`) returns unless `identity_state` is `Matches`, and `signal.rs` refuses 0, 1 and our own group. So
    a defect in the sans-IO decision cannot reach an unrelated process without a defect in a listed file.
  - **The PTY.** The worker driver (`main.rs`, `io_decisions.rs`) and the drain (`drain.rs`) are listed.
  - **Adoption and restart.** `adopt.rs` and the row codec `session.rs` are listed. #176's endpoint code is in `real.rs`
    and the worker's `main.rs`, which are both listed.
  - **Durable storage.** `storage.rs`, `storage/**` (`row_path.rs`) and `lock.rs` are listed.
  - **Auth, token and proof.** `entropy.rs`, `proof.rs`, `hello.rs`, `launch.rs` and the three proof checks
    (`inbound.rs`, `worker.rs`, `guardian-core/src/link.rs`) are listed. `botster-worker/src/command_line.rs` only passes the
    token on to `WorkerLaunch::parse` (in the listed `launch.rs`).
  - **fd handoff.** No code exists yet (P4a), so no entry is right.
- **Shared crates need no entry.** The testkit, `botster-test-process` and `botster-core-edges` (used by the host, sys,
  core and testkit crates) are HIGH by rule 3 whatever they change.

Observation (not counted): `flows.rs` also draws each session's token (an `Entropy` request) and puts it in the spawn
(`flows.rs:172`). Rule 5 names "token and proof checks", and the draw is not a check, so the list need not hold it. A later
change that reuses or shortens a token there would be STANDARD by this list, and only the reviewer's own judgment would
raise it. P3 may add a narrow note to the file's header, so reviewers raise such a change.

VERDICT: CLEAN (0 open) at b1992a0f349466deef5b3aef75ce4b9595a637c2

### Correction after round 1 (same head b1992a0f) — CLEAN WITHDRAWN (the P3 package reviewer's F57, missed here)

The P3 package reviewer's F57 MEDIUM (round 109, `577a4b61`) is real, and it is in this reviewer's scope (the list
decides which PRs get integration review). Round 1 accepted the omitted decision files for one reason only: the real edge
checks the identity again before a signal, so a sans-IO defect cannot signal an unrelated process. That reason does not
cover the order, the timing or the absence of a process action, which rule 5 also puts in the area ("spawn, signal, wait,
reap"). Examples at the head:
- `botster-guardian-core/src/guardian.rs` `settle` (`:433`) decides that `ReapService` comes only after the leader is drained
  and the tree is killed. An early reap leaves descendants with no owner, and no OS check sees it. No real guardian edge
  exists yet, so this file is the whole process-control decision of the guardian, including the kill target and the
  census before TERM or KILL (`:534`, `:543`).
- `botster-core-host/src/run.rs` `kill_payload` (`:275`) chooses the stop-grace signal, and `run.rs` row recovery and
  `session_of_row` (`:667`, `:739`) decide to adopt after a restart.
- `flows.rs` Remove (`:597`, `:618`), `driver.rs:274`, `admit.rs` (`:285`, `:751`) and `engine.rs` (`:463`, `:608`), as F57
  lists them.

Fix: F57's (the six files, and the corrected fallback text in the PR description). This reviewer adds no finding of its
own.

VERDICT: NOT CLEAN at b1992a0f349466deef5b3aef75ce4b9595a637c2 (1 open: F57 MEDIUM, the package reviewer's finding,
confirmed here)

## Round 2 — CLEAN on head b9c3ea2a

Reviewed head: `b9c3ea2a8ffbdc6317241de248af24fd053bc720`, two commits on `b1992a0f` (5 files, +215 -9). v1 is `aaac0c0d`,
and `git merge-tree --write-tree origin/v1 b9c3ea2a` has no conflict. P3's gate log:
`…-p3-high-tier-paths-b9c3ea2a-pool-20261009-085207-67495.log` (928 default and 243 slow tests, 13 mutants, 13 caught).
This reviewer did not read it. The tier stays HIGH (rule 1: the list, and now gate code in `xtask`).

- **F57 closed.** The list adds `driver.rs`, `run.rs`, `flows.rs`, `admit.rs` and `engine.rs` of `botster-core-host`, and
  `botster-guardian-core/src/guardian.rs`, each with its reason. The header now says that a file that decides when or whom
  to spawn, signal, adopt or reap is listed. `flows.rs` names the session token draw (the round 1 observation). The PR body
  corrects the fallback claim: once the list exists, new rule-5 code must extend it.
- **No other process-action file is missing.** A search of production sources for `SignalGroup`, `KillTree`, `Reap` and
  `Spawn` actions finds only listed files, `core-host/src/io.rs` (the action types only; each dispatch is in the listed
  `driver.rs`) and the shared crates (rule 3).
- **`cargo xtask high-tier`** (gate code):
  - `verdict` reports, per line, an entry with no reason, a glob other than a trailing `/**`, and an entry that matches no
    tracked file. `matches` takes `<dir>/**` as files under `dir` only (not `dir` itself and not a sibling such as `dx`).
    Each rule has a unit test.
  - `outcome` is the pass-or-fail decision, with its own test. `command` is I/O only: it reads the list, calls the existing
    `fsutil::tracked_files`, and passes the result through `?`. So its `-> Ok(())` exclusion hides no decision.
  - `the_repository_list_has_no_problem` checks the real list against the walked tree, because the mutants copy has no
    `.git`.
  - The step runs in `taint_job`, after `signals`, and it has a `COMMANDS` and usage entry, with the dispatch test.

Condition (the same as #181's, because both PRs change `taint_job`, `COMMANDS` and `.cargo/mutants.toml`): the PR that
merges second has a conflict, so `base-merge-check` cannot carry this CLEAN to it. That merge needs a new review of the
conflict resolution (the union) and a full gate on the merge commit.

VERDICT: CLEAN (0 open) at b9c3ea2a8ffbdc6317241de248af24fd053bc720
