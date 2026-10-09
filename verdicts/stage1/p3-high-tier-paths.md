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
