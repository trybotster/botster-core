# P3 handoff: session worker, PTY and terminal (Stage 1 Core)

## Resume on 2026-10-04: M1

- Worktree: `~/botster-sessions/trybotster-botster-core-stage1-p3-m1-v1`.
- Branch: `stage1/p3-m1-v1`, cut from merged P1 `1d25d093301072bd0c13a122c68d8f1b1ca0815c`.
- The branch merged current v1 `393f403047beb1583ab3a711afbe2c37255c7e64` in merge `1a49733` without conflicts.
  PR #137 gives real Core the pinned binding identity.
  The M1 follow-up also gives TestkitCore that identity.
  The TI-1 identity transcript joins the M1 proof list. Its id remains pending for real-harness proof.
  All three a2_8 ids remain pending under the lead's confirmed rule.
  The environment id waits for spawn_record in M2a. The tic id waits for the slow-tier process control.
- Focused TI-1 Linux job at code head `9a6099d4125bdf2a23db0290c506507fc0f3509f`: exit 0.
  Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-9a6099d4-linux-20261004-125359-14517.log`.
  The identity transcript and five M1 lifecycle transcripts passed.
  The following commits change only pending-list comments and this handoff.
- Reviewer: `sess-1791142479-0100-7411ff7507ce89a5e7416311610290d3`.
- The lead will staff the integration reviewer when M1 enters review.
- P1 wiring `befe0ff`, the M1 commits, `f60e7d9`, and the lock fix `46b1694` applied without conflicts.
- The restack keeps `pub mod candidate`, `RefusalLayer`, and P1's merged host fixes.
- The contracts pin remains `contracts-v0.1.13`. The binding remains v1's P2 binding on Ghostty `3f8eb68`.
- The old pushed branches remain unchanged.
- M1 now has an independent test guard for each PTY payload group.
  A member starts inside the payload session before the shell body runs.
  That member kills its current group on socket EOF.
  The test closes the socket on Drop and panic. Production alone reaps the payload.
  The guard holds group membership after a natural leader exit and never signals a cached group id.
- The broken-cleanup test kills the worker with SIGKILL before a test panic.
  The test checks that the payload still holds its FIFO before guard cleanup.
  The test then requires FIFO EOF after guard cleanup.
- Focused Linux job: head `88f7bfecce6187a0084df4ca7b0f1104e08dae81`.
  Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-88f7bfec-linux-20261004-124221-89834.log`.
  The job builds the candidate and runs worker-core units, M1 transcripts, slow_payload, and slow_session.
  The job failed before tests because its custom command omitted the required parallelism variables.
  This job supplies no test evidence.
- Corrected focused Linux job at `049751aed4b97fc7b817c0201119c0e363a73935`: exit 0.
  Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-049751ae-linux-20261004-124610-1358.log`.
  Worker-core: 37 unit tests passed. The five M1 ids passed under the selected CI seeds.
  slow_payload: 5 tests passed, including 2 helper entries. slow_session: 9 tests passed, including 2 helper entries.
  This result precedes the F12 fix and does not prove that fix.
- PR: https://github.com/trybotster/botster-core/pull/136.
- Integration reviewer: `sess-1791143089-0101-8ce5f4942328f5697c410ea4da89c466`.
- Full Linux gate at `0044bf7346ca486bf09b5265dbfc0c75f93dd978`: exit 1.
  Package CLEAN: `9aa2f82919dc03b9eb5c880602f932466aab2052`.
  Integration CLEAN: `9cb4de0a3682d50ddb6e9bd5e51866945c11fb80`.
  Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-0044bf73-linux-20261004-130018-20279.log`.
  Formatting and clippy passed. The timer check failed at slow_session.rs recv_timeout.
  The deadline comment was two lines before the call. The correction moves it next to the call.
  Tests, mutation, and fuzz did not run in this gate. Both reviewers must review the correction.
- Package reviewer cleared `ed92707f51f80ddd31f3b3c9fb88f4151e94cdbb`.
  Verdict: `51216ed44581d639c4cf0ac94a19c5242630764f`. F12 is CLOSED.
  The guard registers SIGTERM before registration.
  The regression sends the graceful group signal before worker SIGKILL and test panic.
- Focused F12 Linux job at `ed92707`: exit 0.
  Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-ed92707f-linux-20261004-124906-8719.log`.
  slow_payload: 5 entries passed. slow_session: 9 entries passed.
  Each binary includes 2 helper entries.
- Integration review at `ed92707`: NOT CLEAN, I1 and I2 MEDIUM.
  Verdict: `ce55a6c465bd5257d4d8f8208ecd9c0cebc63d28`.
  I1 requires current v1. The merge above addresses I1.
  I2 requires the same pinned terminal identity in the testkit. The follow-up addresses I2.
  Both reviewers must review the new exact head.
- Corrected focused Linux job at `049751a`: exit 0.
  Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-049751ae-linux-20261004-124610-1358.log`.
  Worker-core: 37 unit tests passed. The five M1 ids passed under the selected CI seeds.
  slow_payload: 5 entries passed. slow_session: 9 entries passed. Each includes 2 helper entries.
- PR: https://github.com/trybotster/botster-core/pull/136.
- Integration reviewer: `sess-1791143089-0101-8ce5f4942328f5697c410ea4da89c466`.
- Full Linux gate at `0044bf7346ca486bf09b5265dbfc0c75f93dd978`: exit 1.
  Package CLEAN: `9aa2f82919dc03b9eb5c880602f932466aab2052`.
  Integration CLEAN: `9cb4de0a3682d50ddb6e9bd5e51866945c11fb80`.
  Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-0044bf73-linux-20261004-130018-20279.log`.
  Formatting and clippy passed. The timer check failed at slow_session.rs recv_timeout.
  The deadline comment was two lines before the call. The correction moves it next to the call.
  Tests, mutation, and fuzz did not run in this gate. Both reviewers must review the correction.
- Next: finish focused verification, obtain both exact-head CLEAN verdicts, and run one full Linux gate.
- M2a and M2b remain separate PRs after M1 merges. The pause record below lists their remaining scope.

## Pause record (2026-10-02; historical pins and contacts)


Written 2026-10-02 at the lead's PAUSE. The package is paused at a clean point. No gate runs. Nothing is uncommitted.

## Contacts and rules

- Lead: sess-1790903471-008f-8b9f78eef51d48aba5a45748495fd673. Report only QUESTION, BLOCKED, DONE, and M1/M2 merge-ready.
- Reviewer: sess-1790940814-00b8-0d00530997d878a3c8546ce05c0ce6a3.
- P1: sess-1790919734-00aa-d8597316795619ab4c83427e523a0f96. P2: sess-1790913443-00a6-5ccbd06a737415a19f55ad6663316aac. P6: sess-1790912515-00a4-1cceee51efc7648a00f107f10856129f.
- `post_message` takes `{session_uuid, payload}`.
- Read in this order: `pair-common.md`, `brief-p3-worker.md`, the plan pin `~/botster-sessions/pins/stage1-plan.e4862c71.md` (revision 20, P3 owns 156 ids), then `docs/core-testkit-controls.md`.
- BUILD.md is binding:
  - libghostty owns every terminal semantic.
  - Machines are sans-IO, with one worker code path for the binary and the testkit.
  - Edges are injected; there are no test branches.
- Heavy jobs: one at a time through `botsterq run --label "core p3 ..." -- env -u RUSTUP_TOOLCHAIN CARGO_BUILD_JOBS=4 NEXTEST_TEST_THREADS=4 cargo ...`.
- Git:
  - Never force-push. Never `git branch -D`.
  - Push a new branch name only after `git ls-remote` shows that the name is absent.
- Worktree: `~/botster-sessions/trybotster-botster-core-stage1-p3-worker`. The local branch `p3-m2b` is the M2b work.

## Branches (pushed heads)

| Branch | Head | Stacks on | State |
|---|---|---|---|
| `stage1/p3-worker-m1-stack` | `46b16945` | P1 `823a1f1`, plus P1's wiring commit `befe0ff` | M1. Reviewer verdict CLEAN (`30bb483`). Not gated: it waits for P1's merge, then a rebase onto v1, a delta review, and one gate. |
| `stage1/p3-worker-m2a` | `98960e43` | P1 `3512c68`; contains the M1 stack | M2a. Reviewer verdict CLEAN (`5eb1f13`). Not gated, for the same reason. |
| `stage1/p3-worker-m2b` | `7abc54db` | M2a, P2's binding `977d986` (merge `d9a37e4`, ghostty submodule `85a8d8e`), and P1 `21adfb2` (merge `4802257`) | M2b work in progress. Not reviewed. |
| `stage1/p3-worker` | `306b143a` | the original cut | Older base branch. |
| `stage1/p3-worker-m1` | `1911ed09` | | The superseded M1 branch, from before the stack. |

Other heads to know:
- P1's testkit wiring: `stage1/p1-testkit-wiring` @ `95a58545`. P3 is to stack on it. That PR needs P1's reviewer and the integration reviewer.
- P1's lifecycle head: `stage1/p1-lifecycle` @ `ff376cce`.

M1 restores two things that P1's wiring branch dropped, in commit `f60e7d9`:
- `pub mod candidate`;
- the RefusalLayer plumbing (`refusals`, `with_refusals`, `fail_next`).

Keep both when you restack.

## Pins

- Contracts tag in use: **v0.1.9** (through P1).
- **Move the pin to `contracts-v0.1.13` (`a8db5c9a`) in one commit at the next clean point.** The lead asked for this, and it is not done yet. The new tag fixes the driver's await_pty idle check, so `conf::a2_2_cancel_race_reports_the_real_outcome` comes off the pending list. The tag also carries Core A13, which has 11 ids without transcripts so far.
- Binding: M2b uses P2 `977d986`. P2's newer head `52d1f73` (fork `ada251c`) has these changes:
  - SGR pixels without a cell size are reported as given;
  - `kp_equal` is encoded (fork patch 12);
  - the A13 clipboard API.

  **Do not use that head until the lead records the pin.** Fork patches 12 and 13 wait for P2's reviewer.

## Done

- **M1:** the real payload edge (`botster-core-sys` payload, pty-process), the `botster-worker` binary (mio loop), and the Worker machine. The machine covers these:
  - hello and token proof;
  - launch;
  - exit watch, option 2: the leader is kept unreaped (WNOWAIT) until its group kill completes;
  - the payload-group kill: SIGTERM, then SIGKILL after `stop_grace`;
  - EndPayload (SIGUSR1) and Terminate (SIGTERM on the worker);
  - the staged close with `LinkWritten{total}`.
- **M2a:**
  - AM-2 single admission point, IN-2 exact counts, IN-6 cancel, IN-10 guards;
  - bounded driver turns, with one `pty_write` per turn;
  - testkit `WorkerSpawner`/`TestkitCore`, program and process controls, `WorkerKey = (data_dir, InstanceId)`.
- **M2b, so far:**
  - `LaunchSpec.limits` (`dfac28f`).
  - The libghostty model (`bef6177`). Model steps follow plan 2.4: `vt_write_until_query`; the worker drains events after each step; ModesChanged is posted only when the final flags differ (E2-3); shadow replies and model `pty_writes` are reply transactions.
  - Reads: ReadScreen, ReadCursor, ReadModeFlags.
  - IN-9 encoding at the start of a transaction (`88c66b0`): a transaction starts at its first byte on the PTY, and an unstarted write is decided again on `PtyWritable`.
  - The testkit oracles (R-7) (`0471c88`).
  - ST-1: the first model_rev of an instance is an FNV-1a hash of its host epoch and instance id (`fd531f5`).
  - Resize SZ-1 to SZ-3 (`7abc54d`).
- **Probe (all 32 seeds, testkit harness):** 78 P3 ids pass. The probe is a throwaway test file that runs a list of ids. Copy it into `crates/botster-core-testkit/tests/`, run it, and delete it afterwards.
- Worker-core unit tests: 64 pass. Clippy `-D warnings` is clean on the changed crates.

## Left (M2b and after)

1. **Bug in the host: Resize forwarding order (sz_3_* fail).**
   - Symptom: three Resizes begun in one pump reach the worker as a, c, b. The diagnostic shows c Applied, then b Applied, and the final size is b's.
   - Likely place: the host's `forward` order across ops that are ready in one pump (`botster-core-host` run.rs `Next::Forward`). The forwarding must keep the begin order (OR-1).
   - Related bug: `admit.rs`, at the `Op::Resize` arm, completes `same_size` against `s.size`. That value lags while a Resize is in flight. A Resize back to the old size then completes `Applied` at once, but the in-flight one applies later (SZ-2 violation). Shortcut only when no Resize is in flight for the session.
   - Both are in P1's code inside the M2b stack. Coordinate with P1 or fix them in the stack.
2. CaptureSnapshot and the baseline snapshot (ST-5, ST-6, `st_1_reads_carry_model_rev`). The binding now has `snapshot_format()` (P2 `d084244`); `model::snapshot_formats()` returns empty until then.
3. ReadFacts and input records (ST-7).
4. Tap (TP-1, `a2_4_events_lost_kinds_set`).
5. The `disable_history` control (`st_2_screen_history_flag`).
6. **A13 clipboard (11 ids), after the pin moves.** Read `Drained.clipboard_acks` (one entry per OSC 5522 write, in order, never dropped). Admit each entry as one contiguous AM-2 reply transaction (A13-1b). Drain before `vt_write_until_query`: undrained acks over `set_ack_backlog_limit` (1 MiB) return `Error::AckBacklog`. `ClipboardWrite` no longer has an ack field.
7. Unsupported controls owned by others: `attach_stream` (P4a); `control_queue`, `control_link_stats`, `scheduler_hold`, `measure_tap_overhead` (P6); `oracle_hyperlinks`.
8. Remove an id from `conformance/core-pending.txt` only when it passes on both harnesses. The real-process harness does not exist yet.
9. When P1 merges: rebase M1 and M2a onto v1, get delta reviews, run the gate once each (`--on mac` if the Linux disk is full), then report M1 merge-ready (and later M2).

## Pending for upstream transcript fixes (R-28, contracts main `bcbd03c`)

The worker is correct on all four ids below. Keep them pending until the lead names the fixed tag.
- `in_9_kitty_unknown_flag_bits_ignored`: libghostty ignores `CSI = 63 ; 1 u`, so no ModesChanged is posted.
- `in_9_kitty_all_32_flag_combinations_pinned`: a set with no net change posts nothing.
- `in_9_kitty_each_flag`: a release with flag 2 off is `NotReported`.
- `a2_2_unsupported_what_values`: a legacy release is `NotReported`.

These two wait for the new binding pin:
- `in_9_application_cursor_and_keypad`: kp_equal. The test needs `CSI ? 1035 l` for SS3 X; with 1035 on, the key gives `=`.
- `in_9_mouse_each_encoding_and_limits`: SGR pixels.

## Decisions in force

- Exit watch option 2: the leader is unreaped until its group kill completes.
- The PTY comes from pty-process 0.5.3.
- Model steps follow plan 2.4 (revision 19 onward).
- R-20: ops with fixed timing are not deferred.
- R-28: see above.
- A13 is final: ClipboardWrite carries its contents; the OSC 5522 ack is a worker-only AM-2 transaction (A13-1b). Erratum 5 is withdrawn.
- `a2_2_cancel_race` was pending as a driver defect; v0.1.13 fixes it.
- `pty_chunk` is a per-step input cap, per host pump. `pty_input` returns `{"$bytes_hex"}`.
- `Superseded{by}`: on the link, `by` carries the replacing request number; the host maps it to the `OpId`.
- model_rev across instances: a hash start, so a collision has a probability of about changes/2^64, not zero. A 64-bit token cannot be certain to differ across instance ids of any length. Raise it with the lead if a reviewer asks for a guarantee.

## Open review findings

- M1 and M2a: none (both CLEAN).
- M2b: not reviewed yet.

## Mutation findings at 78b88fa (2026-10-04)

The full Linux gate passed formatting, clippy, taint, lists, API checks, prebuild, default tests, and slow tests.
Mutation testing failed: 376 tested, 139 caught, 167 missed, 70 unviable, and 0 timeouts.
Fuzz did not run. This head is not READY.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-78b88fa9-linux-20261004-130525-23750.log`.
Artifacts: `~/botster-sessions/gates/artifacts-trybotster_botster_core_stage1_p3_m1_v1-bcb322fa-20261004130525-23750/target-mutants.out`.

The first correction adds four worker-core lifecycle tests through inputs and actions.
The tests cover a wrong frame kind, an early PTY drain, EndPayload after Kill, and Signal::Kill after a drained exit.
One exact mutant in Worker::report_exit has an equivalence argument in `.cargo/mutants.toml`.
The package reviewer must judge that argument. Focused Linux mutation verification is next.
Testkit and real process findings remain open. Both exact-head reviews and a full gate remain required.

## Worker-core mutation proof and testkit correction (2026-10-04)

Focused Linux mutation at `fdd2b8e73927d592b75713c55a86c6ea8c60c037` passed: 110 tested, 103 caught, 7 unviable, 0 missed, and 0 timeouts.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-fdd2b8e7-linux-20261004-132453-37330.log`.
Package verdict `df642bba601b71b7b228bbe197dc19f013732417` closes all five worker-core entries.
F13 remains open for 162 other entries. Integration verdict `a9252300a17c492301f1d4ebb22856b2c7f22441` also keeps F13 open.

The next correction adds checks of the injected host edges and testkit facade.
The checks cover rows, seeded choices, wake deadlines, links, process events, directory reopen, and facade forwarding.
The named-worker check covers the worker_named forwarding method.
These tests await Linux compilation and focused mutation evidence. No other entry has a closure yet.

The focused testkit mutation job at `91f8a4260a5e0ffb0721872400ad03da4868498e` failed baseline compilation.
AttachOptions has no Default implementation. The fixture now uses the contract's JSON reader with an explicit file_directory.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-91f8a426-linux-20261004-133151-41107.log`.
This failed job supplies no mutation evidence. The corrected job must compile and run before any testkit entry closes.

## Focused testkit result and worker edge correction (2026-10-04)

The focused job at `cc34474de63030b74031f5a21d6608bf27a4a843` passed baseline and tested 177 mutants.
Result: 94 caught, 27 missed, 56 unviable, and 0 timeouts.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-cc34474d-linux-20261004-133509-42847.log`.
The actual outcomes.json, missed.txt, caught.txt, and unviable.txt are in `/private/tmp/p3-testkit-evidence/`.
They came from the existing Linux volume at `target/p3-testkit-mutants/mutants.out`.
The failure collector copied the old standard output directory. Its target-mutants.out is not evidence for this focused job.
No tests ran during artifact retrieval.

The next correction adds binding checks for partial writes, EOF, output drains, worker exits, unique identities, and grace deadlines.
The new checks await Linux mutation evidence. release and release_owner remain open.
Two exact arithmetic equivalence arguments await review: control link capacity and worker READ_CHUNK.
No exclusion for either argument exists yet. The remaining real process findings are still open.

## Final testkit findings (2026-10-04)

Focused worker.rs mutation at `2e9811ca0b1c8a790698999efc24c538b5e755d7` tested 100 mutants.
Result: 60 caught, 4 missed, 36 unviable, and 0 timeouts. Baseline passed.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-2e9811ca-linux-20261004-134928-47783.log`.
Artifacts: `~/botster-sessions/gates/artifacts-trybotster_botster_core_stage1_p3_m1_v1-bcb322fa-20261004134928-47783/target-mutants.out/`.
Package verdict `c70c0b288ccbf98da7333b53389cdfdf4230760f` closes 22 additional worker.rs entries.
F13 has 96 open entries: five testkit entries and 91 real process entries.
The reviewer rejected both arithmetic equivalence arguments because readiness and scheduling can change.

The correction tests large control reads, large writes on spawned links, and write interest while output waits.
A separate facade test injects worker reports through the host edge and checks capture release by id and owner.
Libghostty produces the terminal state and every snapshot byte for that test.
The injected reports test facade forwarding. They do not supply worker conformance proof or close any pending id.
No new exclusions exist. Linux compilation and focused mutation evidence remain required.

## F14 correction under the lead's tuning ruling (2026-10-04)

The focused job at `91b58e2c8a8f7d198484e8019f08c50aae0e6dd4` passed: 7 selected mutants caught, 0 missed, and 0 timeouts.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-91b58e2c-linux-20261004-140021-52074.log`.
This supports capture release and write-interest checks. It does not close F14 or the two arithmetic entries.
F14 rejects the one-call assertions because A5-2 permits partial progress. Those assertions are removed.

The lead accepts contract equivalence for pure tuning constants, subject to four conditions.
No clause fixes either positive value. Plan 2.5 rule 7 governs bounded receives; A5-2 governs partial progress.
The edges now receive internal parameters through the same code path. Production retains its defaults.
Checks at 65536, 1088, and 1 cover byte retention, frame order, and complete frames.
A full TestkitCore lifecycle drives the shared Worker at those values and checks ordered completion.
The minimum legal value is 1. The decoder retains partial frames, so neither buffer must hold a whole frame.
Debug assertions reject zero. Named tests check both assertions.
Two exact-constant exclusions record this argument and the proving tests. They await package review and Linux evidence.
No worker conformance id leaves the pending list. Real process findings remain open.
