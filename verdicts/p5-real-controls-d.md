# P5 real controls D — #257

## Round 1 — 2026-10-11

- PR: https://github.com/trybotster/botster-core/pull/257.
- Branch: `stage1/p5-real-controls-d`.
- Exact reviewed head: `96f6f387bc9924a6ac436cc4820b2e166203d572`.
- Parent: `dfaddba357e75812966c5e81e4f13d100cb82dd7`.
- Tree: `a17862601497eb6cc41131ebf449c4b87890fddd`.
- Reviewed base: `5ac94fabf41909a733267a62b692c1a4ee72bf62`.
- READY: `msg_plugin-w_1791714590_963c5d`.
- Risk: HIGH. BUILD rule 3 applies to the shared testkit and process-test crates. Process control also uses HIGH paths.
- Scope: all twelve changed files, 1169 additions and 21 deletions. The reviewer read the complete delta and relevant unchanged composition.
- Authority: BUILD.md, R-55 including the `af17b97` amendment, and approved plan 23am. The reviewer also checked approved plan 23an.
- Plan 23an pin: `/Users/jasonconigliari/botster-sessions/pins/stage1-plan.05b18303.md`.
- Plan SHA256: `361935e2e23c7327d1d30cbc84dd6c9fbe41108d0e97a1304505928e647c4a63`.

### Open findings

**D-F1 — MEDIUM: the reaper proof polls the thread count.**

Evidence: `crates/botster-core-testkit/src/real.rs:1810`. `end_with_reaper` repeatedly reads `platform::threads()` and calls `yield_now()` until the count changes. BUILD Testing rule 5 bans polling. The cleanup deadline bounds the loop but does not make it an event wait.

Required change: remove the polling proof and preserve the required R-55 condition 2 proof. An accepted steward change can define another proof. A notification alone proves a reap, but it does not prove thread exit. The current production closure calls `notify()` before it returns.

P5 accepts the finding in `msg_plugin-w_1791714778_b40c34`. P5 explains why a host cannot join these reapers after the crash. The crash drops `Children`, while live workers retain detached reapers. The lead referred the proof question to the steward.

The reviewer read `shared/steward/drafts/r55-cond2-b.md`. The lead records steward acceptance of that draft. The draft requires counts at the crash, a notification for each reap, no later edge call, and a named reaper function. A separate test must run that function on an owned thread and join it. The spawn wrapper must only detach the function. This draft does not repair the reviewed head. Its final contract record and the corrected source and evidence remain pending.

**D-F2 — MEDIUM: the new macOS function has no mutation evidence.**

Evidence: `crates/botster-test-process/src/platform/macos.rs:11` adds `threads()`. Linux does not compile this function. The supplied Mac logs contain focused tests and clippy, but no mutation run.

Required change: supply the focused Mac mutation log for the changed function on the replacement exact head. Report each outcome under the HIGH mutation rules. Keep the known v1 zombie assertion failure separate. Plan section 8 and the standing Mac rule require this evidence.

P5 accepts the finding in `msg_plugin-w_1791714827_67aeea` and plans the required exact-head run.

**D-F3 — MEDIUM: the snapshot omits the root directory permissions.**

Evidence: `crates/botster-core-testkit/src/real/snapshot.rs:30`. `snapshot()` starts with an empty map. `walk()` records entries below `data_dir` but does not record `data_dir` itself. A Drop that changes the root permissions therefore passes both comparisons. R-55 condition 3 permits only the socket change.

Required change: record the root directory kind and permission bits. Prove that a changed root mode fails the comparison. Preserve the documented exclusion of inode numbers and timestamps.

P5 accepts the finding in `msg_plugin-w_1791714827_67aeea`. P5 reports correction `12691cae1bbbaf1bf7f8f9e61e95c46aa6149983` in `msg_plugin-w_1791714994_e86ea8`. That correction is not gated or reviewed here. A later READY needs its own exact-head review.

### Closed finding

**D-F4 — LOW: the PR body overstated the snapshot and row-file test count.**

The original body claimed seven tests. The reviewed source and both OS logs show five: three snapshot tests and two row-file tests. The reviewer sent `msg_plugin-w_1791714846_354aab`. A later read of the published body confirms five, with the two component counts. D-F4 is closed as a body correction. That read reports a later head, `12691cae1bbbaf1bf7f8f9e61e95c46aa6149983`; this verdict does not review that head.

### Source conclusions

- `Crashable` forwards the complete `CoreApi`. `CrashHandle` holds the slot weakly. A driver handle drop therefore still drops Core and releases its lock. A crash takes Core once and drops it outside the slot lock.
- `host_crash` rejects nonquiet tapped edges. The control drops its temporary tap reference before the crash. The proof checks that the tap no longer exists, workers still run, and the restored node is a socket with no listener.
- The snapshot compares node kinds, permission bits, file bytes, and link targets below the root. D-F3 covers the omitted root permissions.
- `reuse_pid` requires one unheld data directory and a decoded stored row. It checks the worker identity before the group kill and waits for the worker to end.
- The bystander is an owned FIFO child in its own group. Its pid differs from recorded pids in the selected directory. Its start time differs from the old worker's. Failed candidates end through `OwnedChild` before another candidate starts.
- The new row uses Core's `serde_json` encoding and `Row::decode`. The control restores the old worker field before comparison, so every other field must remain equal. The row replacement uses a new mode-0600 file, file sync, rename, and directory sync.
- `bystander_alive` reads live group membership. The harness retains the child until teardown. Field order removes bystanders before the FIFO and root.
- For a reused-pid session, `signals_received` returns the complete tap record. `HostEdges` has one signalling method, `signal_group`; the tap records each call before forwarding it. Other sessions retain the stand-in rule.
- The crash slot preserves the refusal layer and service call behavior. The added signal assertion checks the tap's public record of an actual forwarded call.
- The only list change removes `conf::ad_6_reused_pid_is_never_killed` from `core-pending.txt`. The runner skips its testkit trial and requires its real trial. No pin, transcript, timeout, exclusion, or real-pending change occurs.
- The reviewer read the pinned AD-6 transcript. Its adoption, empty signal record, removal, and live bystander expectations cite AD-6 and the related lifecycle clauses.
- The Prior art note is present. Read-only checks confirm the named old thread counter, descriptor listing, and stale-socket fixture. The new code reuses existing safe libraries and current process helpers. This PR copies no old code.
- `git diff --check` reports no whitespace error.

### Supplied Linux evidence

Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p5-real-controls-d-96f6f387-pool-20261011-031010-57472.log`.

The header names the exact reviewed head and base. All ten CI steps pass in 926.4 seconds. The job and gate exit zero after 939 seconds on `msa1`.

- Default tier: 1620 passed, 484 skipped, 14.684 seconds.
- Slow tier: 472 passed, 1983 skipped, 36.449 seconds.
- The reviewer counted 148 distinct real conformance PASS IDs, including AD-6. AD-6 passes in 0.193 seconds.
- Both new real control proofs pass. All five snapshot and row-file tests run. The count helper test runs in the default tier.
- Testkit conformance: 206 passed. Strict pending: 373 of 393 trials; 20 real-only trials omitted and two held.
- Lists: ledger 690, pending 463, real-pending 58, deferred two, withdrawn eighteen.
- The runner reports testkit-passing 56/69 through its named real-process proof rule for AD-6. It does not execute AD-6 on the testkit.
- The runner reports real-passing 41/68 and real-accepted 42/69. R-59 still excludes unfenced AD-7 from acceptance: counted real is 40/68, plus that unfenced pass. This PR moves no pin and claims no AD-7 closure.
- Mutation stage 1: 111 outcomes, 31 caught, 52 missed, zero timeout, 28 unviable, 181.4 seconds.
- The unmutated slow baseline passes 472 tests in 35.268 seconds; the full baseline step takes 50.3 seconds.
- Mutation stage 2: exactly the 52 stage-1 miss identities, each once. Outcomes: 48 caught, four unviable, zero missed or timeout, 571.5 seconds.
- The reviewer parsed all 163 mutation outcome records and compared the identity multisets. Final result: 79 caught and 32 unviable; zero missed or timeout. The full mutation step takes 806.0 seconds.
- Fuzz passes. The diff changes no crate with a decoder harness.

### Supplied Mac evidence

Focused log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p5-real-controls-d-96f6f387-pool-20261011-032609-73248.log`.

The header names the exact reviewed head and base. Worker prebuild and clippy pass. The focused suite reports 104 passed and one failed in 5.038 seconds. Both new control proofs, all five snapshot and row-file tests, and the count helper pass.

The failure is the unchanged v1 test `a_recorded_process_that_ended_and_is_not_reaped_does_not_live`. Its assertion expects a zombie to retain a matching identity. macOS reports `Absent`. This is catalog B9, assigned to a separate fix. The suite exits 100, so it is not a green Mac suite. The chained AD-6 command does not execute.

Separate AD-6 log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p5-real-controls-d-96f6f387-pool-20261011-032831-86517.log`.

The header again names the exact reviewed head and base. AD-6 passes in 5.287 seconds. The job and gate exit zero after 49 seconds. D-F2 remains open because neither log runs Mac mutations.

### Next review

Sol message `msg_plugin-w_1791714939_b8c421` identifies another scope gap under R-55 (2) condition 4. The pid guard reads only `self.rows[&data_dir]`. It omits historical recorded identities in other harness directories. The reviewer supports that finding in `msg_plugin-w_1791715029_804605`; Sol owns its formal integration finding. The replacement must cover that guard's scope. No ruling limits the condition to one directory.

The current v1 is `c8907b74bd247002784843e1a12f6487b08c579f`, after #255. This review does not accept that import or any later head. The replacement READY must name the exact head, applicable base review, corrected proof, Linux gate, and Mac mutation evidence.

The reviewer changed no product code and ran no tests, builds, gates, mutations, or measurement jobs. Findings went directly to P5 and Sol. No NOT CLEAN report went to the lead.

VERDICT: NOT CLEAN
