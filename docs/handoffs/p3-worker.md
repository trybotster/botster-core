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

## Tuning exclusion correction (2026-10-04)

Package verdict `57973d879474ba7df4047efc7a2a5ff61974ef18` closes F14 in source at `34bad40`.
It keeps both tuning entries open. The regexes were too broad, and debug assertions did not enforce the bounds in release builds.
The correction matches only LINK_CAPACITY at core.rs:33:33 and READ_CHUNK at worker.rs:38:30.
Both positive-value assertions now apply in all builds. The zero-value tests still check the same construction path.
The `34bad40` Linux job remains pending. It cannot prove this correction.
Review and Linux evidence for the corrected exact head remain required.

## Constant mutant-name correction (2026-10-04)

The focused testkit job at `34bad40b4ee3d97eab1866b991c4e2ba5251a7dd` passed baseline and tested 178 mutants.
Result: 120 caught, 1 missed, 57 unviable, and 0 timeouts.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-34bad40b-linux-20261004-140843-56057.log`.
Artifacts: `~/botster-sessions/gates/artifacts-trybotster_botster_core_stage1_p3_m1_v1-bcb322fa-20261004140843-56057/target-mutants.out/`.
The only miss is READ_CHUNK arithmetic. The constant's actual mutant name has no trailing `in`.
Both exact-location regexes now use the actual constant name. The next Linux job must prove their match and the corrected all-build checks.
Package verdict `b9f665b9cb38b7c9651834c47e522a5cf835d210` conditionally accepts both designs, pending corrected-head proof.

## Testkit proof and payload-edge correction (2026-10-04)

Corrected focused testkit mutation at `4960d73873d8301575e312f8fbc596059300c813` passed.
Result: 176 tested, 119 caught, 57 unviable, 0 missed, and 0 timeouts.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-4960d738-linux-20261004-141936-76354.log`.
Package verdict `ce1e810323a2ccac0e71afff49651923216fb037` accepts both tuning exclusions and closes all 71 original testkit entries.
All five worker-core entries are also closed. F14 remains closed. F13 retains 26 payload entries and 65 real driver entries.
The lead's Mac hold permits reading, editing, and committing. All heavy jobs remain on Linux, one at a time.

The payload correction adds default-tier checks of errno preservation and fallback mapping.
Slow tests check the nonblocking flag, pid, debug output, pending output, input delivery, and leader reaping on drop.
The reaping test only queries waitid with WNOWAIT. Production alone reaps the payload.
Test readers cap output from bounded scripts, so a false read count fails an assertion instead of looping without end.
The existing independent group guard remains on every exit path.
Next: focused Linux payload.rs mutation in the slow profile. No real-process exclusion has been added.

The focused payload job at `5172a53553636dc94d9c86c98b0969499b1955dd` failed baseline compilation.
The test used Errno::ACCES, but rustix names the constant Errno::ACCESS. The correction changes only that name.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-5172a535-linux-20261004-143306-97940.log`.
No mutant ran. This job supplies no mutation evidence. The corrected Linux job is next.

## F15 and F16 correction (2026-10-04)

Slow payload mutation at `b6b1660bb2c63828c75e51b0ec95cab8f585b9a3` passed baseline and tested 30 mutants.
Result: 21 caught, 4 missed, 4 unviable, and 1 timeout.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-b6b1660b-linux-20261004-143410-99127.log`.
Artifacts: `~/botster-sessions/gates/artifacts-trybotster_botster_core_stage1_p3_m1_v1-bcb322fa-20261004143410-99127/target-mutants.out/`.
Package verdict `2a5a1666ae4cc522efde37bd93ceea85cebfdabe` opens F15 for premature EOF and F16 for the reaping observation.
Integration record `e90b9eecf0f392c2352cf198ce7b92e2ae7f7ef2` carries both findings.

The readiness reader now fails on EOF before readiness. This releases its independent group guard on panic.
The reaping test starts an isolated observer with --exact and --test-threads=1.
The observer owns no other direct child. A reused payload PID cannot name another child of that observer.
The query uses WNOWAIT and NOHANG. It never reaps the production payload and never blocks.
The outer test independently owns the observer's Child handle and retires it immediately after wait.
The payload group still has its independent guard, including on observer death.
Three exact equivalence arguments await package review; no payload exclusion exists yet.
The wait_unreaped fallback-sign argument remains incomplete because spawn-error cleanup can race the exit watch.
Next: corrected Linux slow mutation. F13, F15, and F16 remain open pending review and evidence.

## Observer lifetime and exit-watch correction (2026-10-04)

Package verdict `0dd5ba819ab6d73ef9cc749e53163b865e48a14e` closes F15 and F16 in source and opens F17.
F17 requires the observer to end when the outer test process dies.
The observer now starts through the existing GroupGuard registration prefix in its own process group.
The outer test owns the group's anchor. Socket EOF on parent death makes that anchor kill the observer group.
The observer's independent PayloadGuard then ends the payload group on observer death.
The observer still owns no other direct child while it queries the retired payload PID.

The TERM-only slow caller now sends SIGKILL after observing the exit and before reap.
Three exact-function exclusions record the reviewed reap and bitwise equivalence arguments with recheck conditions.
No function-wide process-glue exclusion exists yet.
The exit-watch decision now uses an injected wait operation. Production and the error test use the same loop.
A default test supplies EINTR then ECHILD and checks retry plus the negative unknown-exit code.
This tests the fallback-sign mutant instead of asserting that the error path is unreachable.
The `3d13dda` Linux job remains pending. It does not prove this later correction.

## Mac baseline failure and guard correction (2026-10-04)

Package verdict `1cf33021330677bd9e868698b4804931028ec6a5` verifies the `3d13dda` Linux artifacts.
The run caught the read-zero and drop-no-op mutants. F13 retains 65 driver entries and one fallback-sign entry.
The lead released the Mac and prohibited new Linux jobs. New heavy jobs use Mac, one at a time.
The focused Mac job at `ebed102` reached its 45-minute deadline during baseline. No mutant ran.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-ebed1022-mac-20261004-150213-23937.log`.
The input test failed its pending-output assertion. Its cleanup then blocked. The other 59 tests passed.
This is a new cleanup finding. The passing Linux run does not prove Mac cleanup.

The payload anchor now explicitly joins the leader's group before registration completes.
A shell can put its background command in a separate group. The anchor must own the payload group.
The shell provides its group id while it waits for registration. The anchor still signals only its current group on EOF.
The pending-output test now waits within a ten-second deadline for more than one program byte.
The correction does not assume that descriptor readiness means the full readiness word has arrived.
Both causes remain unverified until the focused Mac baseline runs.
Next: run the Mac payload baseline before another mutation job.

## Current v1 binding merge (2026-10-04)

The lead lifted the Linux hold. Future heavy jobs prefer Linux, one job at a time.
The active Mac payload baseline at `a532a729` may finish. No second heavy job has started.
The lead merged PR #138 into v1 at `38bbe580013d02175636395a28a4c359b20c32b2`.
P3 merged origin/v1 without conflicts. The merge adds binding inspection, snapshot restoration, and image-limit APIs.
The binding also adds hyperlink, parser state, cell, colour, and cursor reads.
No P3 worker or payload source changed in the merge. The mutation configuration merged without conflict.
Both reviewers must review the delta. The next full gate must use the resulting current-v1 tree.
Package verdict `5297f096118cdbbb396b324018519022bc3b0b6a` keeps F13, F18, and F19 open.
Integration record `ae4b728d2f725e7defbae71fd67d918ff7794ca9` records the same findings.
F18 requires a queued-output synchronization proof. F19 requires finite Mac panic cleanup with a payload waiting for input.
The current Mac baseline result remains pending.

## Queued output and panic-cleanup proof (2026-10-04)

Package verdict `8d0bab64f290c2784213b856b92bab22d373f265` finds no new P3 issue in the v1 merge.
Integration record `d843a2804c3022623c5d427498b4f9f798d73b66` also accepts the merge delta.
F13, F18, and F19 remain open. Neither reviewer ran gates.
The input test now uses a FIFO marker after the program writes its PTY output.
The test waits once for FIFO readability, checks IN, and consumes the marker.
The production pending-output assertion remains. It can now distinguish the query failure from incomplete program output.
The test no longer repeats readiness events without consuming data.
A separate test panics after the marker while the payload waits for input.
The test requires independent guard cleanup and production reaping to finish within ten seconds.
The guard never reaps the production payload. No production source changed.
The active Mac baseline uses the earlier head. These new proofs still require a run.

## M2b paging obligation and resource hold (2026-10-04)

Package verdict `1038131d3d1f12bb91d3ec76820e77e0ca0860ea` accepts the FIFO and panic-test source delta.
F13 retains 66 original entries. F18 and F19 still require corrected-source Mac evidence.
The lead restored the Mac hold. Future gates and heavy jobs must use Linux, one at a time.
The already active Mac baseline remains pending. It uses the earlier source and cannot close the new proof requirements.

The lead assigned the R-30 and A8-2 paging obligation to M2b.
P6 will add `crates/botster-terminal-ghostty/GHOSTSNP.md` through `stage1/p2-ghostsnp-spec`.
The M2b PR must complete its Worker paging section in the same PR as the paging code and cite that code.
The recommended design uses contiguous slices of native encoded bytes without in-band framing.
`Page{index, bytes, last}` carries metadata outside those bytes, so that design adds zero framing bytes.
If M2b adds in-band framing, the specification must state its exact size.
P6's every-cut fit check computes its bounds from that section.

## Second Mac baseline result (2026-10-04)

The `a532a729` Mac baseline ended with exit 124 after 2700 seconds.
It ran 14 tests: 13 passed. The input test reported no program output after its ten-second wait.
Cleanup then blocked until the job deadline terminated the test after 2688.563 seconds.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-a532a729-mac-20261004-154924-48598.log`.
Explicit group membership alone did not fix Mac cleanup. F18 and F19 remain open.
The later FIFO and bounded panic tests were not in that run.
Next: run those corrected tests on Linux. Corrected-source Mac evidence still requires the lead to lift or except the Mac hold.

## Corrected Linux payload proof and diagnostic preparation (2026-10-04)

The corrected baseline at `4b1d39cb` passed all 15 slow_payload tests in 0.022 seconds.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-4b1d39cb-linux-20261004-163542-77986.log`.
Focused slow payload mutation on the same head passed baseline and tested 28 mutants.
Result: 23 caught, 5 unviable, 0 missed, and 0 timeouts.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-4b1d39cb-linux-20261004-163631-78503.log`.
The run supplies current-source evidence for the fallback-sign correction and the three reviewed equivalence exclusions.
F13 closure still requires package review. The 65 driver entries remain unresolved.

The lead denied a Mac exception. Corrected-source Mac evidence must wait until the orchestrator lifts the Mac hold.
The lead requires a job deadline that covers the incremental build, such as twenty minutes.
Code test deadlines remain unchanged. A cleanup hang remains a defect finding.
The panic test now prints process and group state if its ten-second cleanup wait fails.
The independent cleanup already runs in its own thread before diagnostics start.
Diagnostics only read process state. They never signal or reap a process.
The diagnostic labels the recorded payload pid without claiming that ownership still exists after cleanup starts.
The queued-output failure prints its observed count before the assertion without a blocking process query.
No production source changed. These diagnostics still require source review and compilation.

## GHOSTSNP specification merge (2026-10-04)

P3 merged origin/v1 at `a3b8af5be21389423439fb3c09d6a81d924c987d` without conflicts.
PR #139 adds the GHOSTSNP specification and four lines of binding rustdoc.
No production code or P3 exclusion changed. Both reviewers must review this documentation delta.
P3 read the specification. Its Worker paging section remains pending until M2b implements CaptureSnapshot.
The specification counts host metadata outside page bytes and requires M2b to specify worker framing independently.
The next full M1 gate must use this current-v1 tree.

## Driver mutation fixture (2026-10-04)

Package verdict `8b7213ac034bece6e2840e3d881cba4e1a54de36` verifies the retrieved payload outcomes.
The fallback-sign mutant is CaughtMutant. All 26 original payload entries are closed: 23 caught and three accepted equivalences.
F13 retains 65 driver entries. F18 and F19 remain open for Mac evidence.
Raw payload artifacts are in `/private/tmp/p3-payload-evidence/`.

The real session fixture now lives in `crates/botster-worker/tests/common/session.rs`.
The integration test still starts the prebuilt candidate binary.
The binary's slow unit tests start a test observer that calls the production Driver directly.
Cargo-mutants rebuilds that test executable, so the observer contains the mutated driver.
Both fixtures use the same session tests, control codec, payload guard, and worker ownership rules.
The observer's launch data matches the fixture. No production test branch or second worker machine exists.
The fixture move preserves the real worker tests. It does not establish mutation closure by itself.
Next: run the slow driver unit baseline on Linux, then inspect its focused mutation results.

## F20 driver-observer lifetime correction (2026-10-04)

The Linux driver unit baseline at `e4593e86` passed all ten tests in 0.214 seconds.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-e4593e86-linux-20261004-164407-87814.log`.
Focused driver mutation on that head is active. No second heavy job has started.
Package review opens F20 because the test observer lacks independent cleanup on parent death.
The observer now starts in its own group through the existing GroupGuard registration prefix.
The anchor joins that group before Driver runs. Socket EOF on parent death makes the anchor end the observer group.
The payload retains its separate guard. Production alone reaps the payload.
OwnedWorker drops both guards before it retires its retained observer Child handle.
The prebuilt integration worker launch remains unchanged.
A new test kills only its retained parent Child and requires observer pipe EOF within ten seconds.
The parent helper starts a real Driver observer and payload before it reports readiness.
The test never signals an observer or payload through a cached pid.
This correction requires source review and an exact-head Linux baseline after the active mutation run ends.

## Driver decisions and Mac diagnostic release (2026-10-04)

Package verdict `344e35f5de499153fd3f9352e558019403913e15` closes F20 with exact-head runtime evidence.
Integration verdict `80dc9ed183c8db9ab125dc5b106e7a2344ddaa57` confirms that closure.
F13 retains 60 entries from the earlier driver run: 41 misses and 19 timeouts.
The failed mutation log is `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-e4593e86-linux-20261004-164448-88453.log`.
The run tested 69 mutants: five caught, 41 missed, four unviable, and 19 timed out.
No timeout counts as caught. Code deadlines remain unchanged.

The Driver now calls pure functions in `src/io_decisions.rs` for readiness, deadline selection, and error classification.
Default tests exercise all readiness combinations, deadline boundaries, and interrupted, blocked, and failed I/O results.
The real Driver remains the only adapter. The refactor preserves its previous decisions and the shared Worker machine.
No exclusion was added. Original mutant relocation and the changed code still require independent review and mutation evidence.

The lead lifted the Mac hold and authorized the queued payload diagnostic once with a twenty-minute job deadline.
The diagnostic uses the current corrected FIFO, panic-cleanup, and process-state tests.
A hang remains a finding. The code test deadlines remain ten seconds.
Next: run that Mac diagnostic before the next Linux driver job.

## Confirmed Mac defects and correction (2026-10-04)

The authorized Mac diagnostic at `7b541365` ended with exit 124 after 1201 seconds.
It ran 15 tests: 13 passed and two failed. Compilation took 1.19 seconds.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-7b541365-mac-20261004-165836-99257.log`.
The FIFO marker arrived before pending_output returned zero. F18 is a production-query finding.
The panic test failed after 10.127 seconds. The leader and anchor both had PGID 99486 and states ?Es and ?E.
The query test then blocked until external SIGTERM after 1197.209 seconds.
Package verdict `7f675b347ce2265f89bd1bdbe9e9e0baba496c43` and integration verdict `6b4bbe36442a2e66984a374152d217a922548349` retain F18 and F19.

Apple's tty.c implements FIONREAD through ttnread, which counts canonical and raw input queues.
Its TIOCOUTQ counts the output queue. tty_ptmx.c supplies that output count through EVFILT_READ.
Apple's kern_exit.c calls ttywait for a controlling-session leader before it revokes the terminal.
Apple's ps formatter uses E for P_WEXIT before the process reaches SZOMB.
These source facts explain the query failure and support a tty-drain hypothesis for cleanup. Runtime correction proof remains pending.
Sources: apple-oss-distributions/xnu bsd/kern/{tty.c,tty_ptmx.c,tty_dev.c,kern_exit.c}; adv_cmds ps/print.c.
Local source copies are in `/private/tmp/p3-*.c`.

Mac pending_output now uses a fresh safe kqueue Watcher and a zero-wait EVFILT_READ event count.
The query reads no PTY bytes. A repeated-count assertion preserves the queued-output proof.
The Mac-only dependency resolves to kqueue 1.2.1. Its published source was inspected under `/private/tmp/p3-kqueue-source/`.
No unsafe code or terminal parser was added.
Driver now returns a query error instead of treating it as a zero-byte drain.
Payload Drop closes the PTY master after group SIGKILL and before waiting for its leader.
Payload reap also closes the master before its wait. Every current caller still observes exit and kills the group first.
The payload guard requests anchor cleanup, then returns without waiting for anchor EOF.
This lets the production owner close the master. The anchor still signals only its own current group and never reaps the payload.
The three earlier payload equivalence exclusions require review because Drop and reap changed.

Direct slow driver tests now check control readiness/EOF, idempotent link loss, partial writes, totals, and write interest.
They also check PTY readiness, bounded drain completion, and deregistration through the production ReapPayload action.
Their payload guard releases before production cleanup. No payload is taken into an unguarded local owner.
The tests enable rustix net only as a dev dependency for the socket send-buffer bound.
Next: source review and Linux compilation/baseline. A corrected Mac diagnostic needs fresh authorization after the once-authorized run.

## Mac closure and remaining Driver mutations (2026-10-04)

The corrected Linux baseline at `e9efad5e` passed 44 selected tests with zero skipped.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-e9efad5e-linux-20261004-172920-12855.log`.
The once-authorized corrected Mac diagnostic passed 15 tests with zero skipped and exit zero after four seconds.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-e9efad5e-mac-20261004-173007-13507.log`.
Package verdict `9e81e4b61a17e23ca38c6c4092307df40e6e2c24` closes F18 and F19.
Integration verdict `aa434a01e9521ced33b12a56e44f093156c1504e` confirms both closures.
Both reviewers rechecked and accepted the three payload equivalences after Drop and reap changed.
New query and cleanup mutations still require evidence. These selected baselines are not a full gate.

Focused Driver mutation at `e9efad5e` tested 73 mutants: 63 caught, five unviable, three missed, and two timed out.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-e9efad5e-linux-20261004-173244-15398.log`.
Raw evidence is in `gates/artifacts-trybotster_botster_core_stage1_p3_m1_v1-bcb322fa-20261004173244-15398/target-mutants.out`.
The misses are READ_CHUNK multiplication, main no-op, and the Driver::run PTY arm.
The timeouts are flush written += changed to *= and written != changed to ==.
The four running session tests occupied every slot before the direct accounting test ran.
A selected direct-test mutation run must establish assertion failures for these two entries.
No timeout counts as caught. Reviewers must map original entries to the relocated decisions and current caller checks.

F21 requires a descriptor wait in the partial-write test.
The corrected loop polls peer input and queued control output before each read and flush.
The poll uses the remaining ten-second deadline. Control writability follows the returned output readiness.
The test retains complete bytes, cumulative totals, write interest, and the production write path.
Next: review and Linux proof of F21, then close the remaining Driver entries and new payload mutations.

The branch merged current v1 `e8cf15068825888795f3ff2582d98b5c8e9b09e4` in `2a88d7a`.
That merge adds P6 oracle controls and process cleanup tests from PR #140.
Cargo.lock preserves both the P6 sha2 dependency and the P3 worker-core dependency.
The pending list preserves all ids and P6 snapshot comments. Its TI-1 comment now reflects the completed testkit wiring.
The merge requires reviewer checks. The next focused job selects direct Driver tests for flush mutations.

The first post-merge focused job at `ae039b52` stopped before tests with exit 101.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-ae039b52-linux-20261004-174913-23005.log`.
The automatic manifest merge added a duplicate botster-terminal-ghostty dependency entry.
The correction keeps the existing workspace dependency and removes the duplicate path entry.
The job supplies no test or mutation evidence. The corrected focused job must run next.

## Direct flush proof and PTY readiness check (2026-10-04)

The focused flush job at `1c45103c` passed: nine mutants tested, seven caught, and two unviable.
No mutant was missed or timed out. The selected baseline runs six direct entries and skips 23 other entries.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-1c45103c-linux-20261004-175018-23825.log`.
Completed raw evidence was copied without a rerun into `/private/tmp/p3-driver-flush-evidence`.
The written += to *= mutant fails the written > 0 assertion in 0.007 seconds.
The written != to == mutant fails the empty-input assertion in 0.007 seconds.
Integration verdict `b4eab95b5c1da77afa4ca3189d871dbeff7ec9d6` verifies both assertion failures.
Package verdict `3aedba3c62486bd8a8e0d4e1ebd7f641a2ef460e` maps 55 more original driver entries to caught mutants.
Its count still precedes the focused flush evidence. Original-entry closure requires the package's updated count.
Integration I3 is closed by `c2c653a6d132d83053a1a17cc45e0bb6821e7201` after the manifest correction.

F21 now also fails promptly if a ready turn makes no read or write progress.
A new direct test clears PTY readiness through WouldBlock before releasing a FIFO-gated program.
The program writes four MiB through the real PTY, then reports completion through a separate FIFO.
The production Driver::run must receive PTY readiness and resume reads before that completion marker arrives.
The test closes the control peer, releases the independent guard, and waits for driver retirement.
The test checks adapter readiness only. It does not establish terminal semantics or real-harness conformance.
Harness field order now releases the payload guard before Driver drops its production payload.
No production behavior changed. Next: Linux compilation, baseline, and selected PTY/flush mutations.

The first PTY check at `7c2503e0` failed its unmutated baseline during driver retirement.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-7c2503e0-linux-20261004-175451-26664.log`.
Six selected tests passed, including the F21 progress assertion. The PTY test reached the done FIFO marker.
Its driver-retirement channel then timed out after ten seconds. No mutants ran.
DP-8 requires the worker to remain alive after link closure. The test used the wrong retirement condition.
The correction sends the authenticated hello through the link codec, then sends Remove after the output-completion marker.
The adapter proof still starts with a cleared PTY read flag. The independent guard still owns the real payload.

## PTY mutation proof and command-line decisions (2026-10-04)

The corrected PTY/flush job at `e475c284` passed: ten mutants tested, eight caught, and two unviable.
No mutant was missed or timed out. The baseline runs seven selected entries and skips 23 other entries.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-e475c284-linux-20261004-175646-27658.log`.
Completed raw evidence was copied without rerun into `/private/tmp/p3-driver-pty-evidence`.
PTY-arm deletion fails the done-marker poll assertion after 10.019 seconds, with result Failure(100).
Package verdict `6bc2b150259bcb85ee3f0647c1addce615738df0` closes F21.
Integration verdict `d64fcb9bc58b009f569345af102716c97efd3a6a` confirms package closure.
The remaining original-entry count requires reviewers to account for the new PTY proof.

Main now injects the real Driver into command_line::execute.
The new function owns the existing parse decision and status mapping. WorkerLaunch still owns the parser.
Default tests check invalid launch refusal, exact identity delivery, success, driver failure, and missing-token refusal.
Main only reads arguments and token, calls that function with Driver, prints an error, and returns its status.
Two slow tests check the prebuilt main boundary: invalid arguments return 2; a missing control socket returns 1.
The existing session tests still check successful real-driver retirement with code 0.
Both slow fixtures use one shared helper for the prebuilt candidate path.
No mutation exclusion was added. Main's process-glue classification requires reviewer approval after runtime proof.
Next: Linux prebuild, both command-line boundary tests, and default-tier mutation of command_line::execute.
