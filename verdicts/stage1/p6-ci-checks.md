# Integration review: #181 (P6 PR B, the CI checks; branch stage1/p6-ci-checks)

Reviewer: integration reviewer (Opus), `sess-1791517267-0168-401cc27284f329b42ec84da0de9d91e0`. This reviewer runs no
build, test or gate. Scope: the cross-package parts (the shared crates, the merge of #177, the gate's behavior for every
PR, and the carries from #171 and #177). The internals of the xtask checks are the P6 package reviewer's.

## Round 1 — CLEAN on head f5652517 (a v1 merge and a full gate are required before merge)

Reviewed head: `f5652517262ee7ce6d64b1787d2b02fc28df72ae`. Its last v1 merge is `c4a83e54` (v1 `da43a1b1`, #177). Range
`da43a1b1..f5652517`: 30 files, +5,081 -275. P6's evidence logs are in the PR body. This reviewer did not read them.

### The merge of #177 (`c4a83e54`)

`git merge-tree --write-tree 83383983 da43a1b1` conflicts in `.cargo/mutants.toml`, `xtask/Cargo.toml`, `xtask/src/ci.rs`
and `xtask/src/main.rs`. The real merge differs from the trial tree only in those four files, and each resolution is the
union:
- `ci.rs`: `signals` is in the `use` list, in the job description and in `taint_job`, with the PR B checks.
- `main.rs`: the `signals` command is in the usage text, `COMMANDS` and the test list.
- `Cargo.toml`: one `syn` (the workspace version with the `full` and `visit` features) and `quote`.
- `mutants.toml`: the `taint_job` reason names `signals`, and the #177 `clippy_job` and `test_budget kill` entries are
  kept.

### Not on current v1, and a base-merge-check is not enough

v1 is now `c869dbea` (#178, #179 and #180 merged after `da43a1b1`). `git merge-tree --write-tree origin/v1 f5652517` has
no conflict. But PR B adds checks that read the whole tree: process-check, mutants-cited, gate-decisions, the
platform-only code derivation, and the `syn` timers. A base-only merge can bring v1 code that these checks have never
read, in files that PR B does not touch, so `base-merge-check` (no shared path, the same own diff) cannot show that the
merged tree passes. **Condition:** this CLEAN carries over to the v1 merge only with a passing full gate on that merge
commit, in addition to `base-merge-check`.

### Checked, no finding

- **The shared crates.**
  - `botster-core-sys/src/process.rs` only documents the macOS start-time unit and adds a macOS test of it. The old
    off-macOS exclusion of `start_time` is replaced by the cfg derivation and one equivalent-mutant entry (`+` with `-`:
    `s * 1_000_000 - u` with `u < 1_000_000` is still one-to-one, and only equality has a meaning).
  - `botster-test-process`:
    - `run_to_completion` (stdin null, both pipes read by one `poll`, the exit by the deadline, and kill and reap on the
      drop) and `both_to_eof` (both descriptors non-blocking, a pass that reads nothing after a ready `poll` fails at once).
      `O_NONBLOCK` is set on the read ends only, so the child's write ends do not change.
    - The new `NO_PROGRESS` checks in `read_line` and `read_to_end`.
- **The #170 G2 ruling.** `base_merge/tests.rs` `Repo::git` and the new `fsutil::test_git` use `run_to_completion` within
  `Deadline::cleanup()`, with git's user and system configuration off.
- **The #171 carries for PR B.**
  - The mutants step uses the `mutants` nextest profile (no terminate-after, so a hang is a TIMEOUT) and `--max-fail
    1:immediate`.
  - The off-macOS exclusions come from `cfg` (the derivation), not from hand entries.
- **The two #177 items.**
  - The process-check allowlist entry for `signal.rs` `a_signal_to_our_own_group_reaches_this_process` is right: its read
    is bounded by `set_read_timeout` (the F55 fix), which a token check cannot see. PR C moves it to `Bounded`.
  - `clippy_job` now gives its exit to the tested `tools::succeeded` (`only_a_successful_run_passes`), and its exclusion
    names that decision.
- **The new exclusions** (`gate_decisions` and `mutants_cited` `command -> Ok(())`): each names the tested decisions inside
  the body, and each body has no operator of its own.

### Carry (not counted)

The lead's #177 enforcement decision gives P6 the ban of `allow(clippy::disallowed_methods)` outside an allowlist. At this
head, no xtask check mentions that attribute. The carry stays open until P6 names the PR that adds it. When that ban
lands, the `signals` token scan shrinks to `Command::new` with kill program literals.

VERDICT: CLEAN (0 open) at f5652517262ee7ce6d64b1787d2b02fc28df72ae (the carry-over to a v1 merge needs base-merge-check
and a full gate on that merge)

### Correction after round 1 (same head f5652517) — CLEAN WITHDRAWN (the P6 package reviewer's B7, missed here)

The P6 package reviewer's B7 HIGH is real, and it is in this reviewer's scope (a shared crate). `run_to_completion`
starts the tool with `OwnedChild::spawn`, not `spawn_group` (`child.rs:36` against `:44`). A tool that starts a child
(for example `git` with a hook, a credential helper or `ssh`) passes the pipes to that child. Then `both_to_eof` waits
until the deadline, and on the timeout the drop kills and reaps only the leader. The descendants stay, with no owner. This
is the leak that `botster-test-process` exists to prevent. This reviewer checked only that the leader is killed and
reaped. Fix: start the tool with `spawn_group`, so the drop ends every member of its group, and add a test whose tool
leaves a child that holds stdout open.

The package reviewer's other findings (B1 to B6, B8, B9) are in the xtask checks and theirs. B5 (an exclusion of
`mutation_decision` that passes because it names its callee `mutation_verdict`) touches the exclusions that round 1
accepted. Round 1 checked only the reasons, not the gate-decisions rule that accepts them.

VERDICT: NOT CLEAN at f5652517262ee7ce6d64b1787d2b02fc28df72ae (1 open here: B7 HIGH, the package reviewer's finding,
confirmed here; B1 to B6, B8 and B9 are theirs)

## Round 2 — CLEAN on head a4b803e4 (a v1 merge and a full gate are required before merge)

Reviewed head: `a4b803e435553d7e8f9851991515189d62dfe67d`. Delta from round 1 (`f5652517`): two v1 merges (`4f26bc9b` with
v1 `c869dbea`, `d9dff45c` with v1 `13d7db09`) and the fixes of the P6 package reviewer's B1 to B9.

- **The merges.** Each tree is the tree of `git merge-tree --write-tree` of its parents (`34e8a463` and `2aff1d08`). So the
  crate changes outside the PR's own files (core-host, core-link, testkit, the Ghostty binding and the contracts pin) are
  v1's.
- **B7 closed** (`ac048e17`, `botster-test-process`, this reviewer's scope). `run_to_completion` starts the tool with
  `OwnedChild::spawn_group`. In group mode, `exit_by` does not reap the leader and does not set `status`, so the drop's
  `end()` runs `end_members` over the whole group on every path, also after a successful exit. The real test
  `a_child_that_the_tool_leaves_is_ended_when_the_run_returns` has a FIFO-blocked `cat` that holds a watched pipe. In the
  "done" case the run succeeds, and in the "late" case it fails `TimedOut`. Both then see the pipe's end of file, so the
  `cat` is gone. With `OwnedChild::spawn`, that end of file never comes. `read.rs` only removes `buffered()` (its last user
  was the old error text), and its test now reads the second reader's bytes back.
- **B1 to B6, B8, B9** are in the xtask checks and are the package reviewer's. `.cargo/mutants.toml` and the `.config` files
  do not change after round 1, so the stricter gate-decisions rule (B5) accepts the exclusions that round 1 read.
- **The gate log** (`…-a4b803e4-pool-20261009-084712-58261.log`) names the head. Its base line says `aaac0c0d`, but the
  head's tree contains v1 only to `13d7db09`. The default tier runs 1006 tests and the slow tier 248, all pass. The mutants
  step reports 549 caught, 0 missed, 0 timeout and 25 unviable. Exit 0.

Observation (not counted): the new test clears close-on-exec on the pipe writer. Under nextest (one test per process) no
other spawn inherits it. Under `cargo test` (threads in one process), a concurrent spawn in the same binary could inherit
the writer and delay the end of file until its deadline.

**Condition** (unchanged from round 1, and more needed now): v1 is `a14e9dc2`, which adds #182, #183, #185 and #186 after
this head's base. #186 changes `crates/botster-core-sys/tests/common/guard_platform.rs`, which is real-process test code
that the new process-check and timers read. So the v1 merge needs base-merge-check AND a full gate on the merge commit.
If #184 merges first, the union resolution of `taint_job`, `COMMANDS` and `.cargo/mutants.toml` also needs this reviewer.

### Carry (not counted)

The attribute ban (`allow(clippy::disallowed_methods)` outside `.config/allow-attributes.txt`) stays with P6's next PR
(the lead's #177 decision).

VERDICT: CLEAN (0 open) at a4b803e435553d7e8f9851991515189d62dfe67d (the carry-over to a v1 merge needs base-merge-check
and a full gate on that merge)

## Round 3 — CLEAN on head 7bb34d76 (delta from a4b803e4)

Reviewed head: `7bb34d7618238f116613319f0883fae0d1fe82f7`, a fast-forward from `a4b803e4`. P6's gate log
`gates/…-7bb34d76-pool-20261009-102251-20084.log` names this head and base v1 `67fd748a`. Results: 1047 default and 248
slow tests passed; the mutants job had 602 mutants (576 caught, 0 missed, 0 timeout, 26 unviable); exit 0. This reviewer
read its header and summaries.

- **The three v1 merges** (`927fd2e3`, `21c32f4a`, `7bb34d76`) each have the tree of `git merge-tree --write-tree` of
  their parents.
- **The PR's own delta.** Compared with `git merge-tree --write-tree a4b803e4 67fd748a` (`7512033e`), the head changes
  only `xtask/src/{gate_decisions,mutants_cited,platform_code,process_check,timers}.rs`, their tests, and
  `.config/process-check-allow.txt`. The step wiring (`taint_job`, `COMMANDS`, `.cargo/mutants.toml`) does not change.
  The fixes for B3, B5, B6 and B8 are inside the checks. By the lead's rule, the package reviewer owns them.
- **Cross-package effect.** The allowlist renames the two `guard_platform.rs` entries to
  `a_wait_for_a_pid_with_no_thread_group_task_follows_the_kernels_answer`, which is the test name at the head after
  #186. No entry is added. The checks read the whole tree, and the gate passes on it, #190's `base_merge` code included.

The conditions stay:
- The v1 merge (v1 is now `1f157c29`; `git merge-tree --write-tree 7bb34d76 origin/v1` has no conflict) needs
  base-merge-check and a full gate on the merge commit.
- #184 is not in v1 (its branch is at `57380b2b`). The PR of #181 and #184 that merges second needs this reviewer's
  review of the union of `taint_job`, `COMMANDS` and `.cargo/mutants.toml`.

VERDICT: CLEAN (0 open) at 7bb34d7618238f116613319f0883fae0d1fe82f7

## Round 4 — CLEAN on head 24fb122f (delta from 7bb34d76; plan 23c/23d closed forms)

Reviewed head: `24fb122f32cb691bf073292f12b6a61448260c55`, a fast-forward from `7bb34d76`. P6's gate log
`gates/…-24fb122f-pool-20261009-115821-64845.log` names this head and base `7aec2bb9`, which is the current v1 tip. It is a
full Linux pool gate: 1094 default and 248 slow tests passed; the mutants job had 644 mutants (615 caught, 0 missed, 0
timeout, 29 unviable); exit 0. This reviewer read its header and summaries.

- **The two v1 merges** (`fc54187c` of `c06f5b98`, `24fb122f` of `7aec2bb9`) each have the tree of
  `git merge-tree --write-tree` of their parents. The head contains the v1 tip, and the gate's base is that tip. So this
  gate meets the Merge bullet of plan 23d while v1 stays at `7aec2bb9`.
- **The PR's own delta.** Compared with `git merge-tree --write-tree 7bb34d76 7aec2bb9` (`12437910`), the head changes the
  five source-reading checks and their tests, `.cargo/mutants.toml` and `.config/process-check-allow.txt`. The step wiring
  does not change. The closed-form fixes of B3/B8, B5 and B6 are inside the checks, and the package reviewer owns them.
- **`.cargo/mutants.toml` (workspace config).** When the comments are removed, the file is the same as before, so no
  exclusion is added, removed or changed. Only the reason comments change: the proofs move into the
  `decision (proof, …)` form, and the other names get backticks.
- **The new allowlist entry** (`crates/botster-core-testkit/src/worker/tests.rs`, the F63 test, `blocking-read`, owner P3).
  It covers `go_rx.recv()` in the control thread (`worker/tests.rs:422`). That wait ends in both cases: the main thread
  sends `go` after the end thread holds the owner, or a failed `recv_timeout` panics the main thread, and the unwind drops
  `go_tx`. With the old lock order, the control thread blocks on the owner after `go`, not in this wait. So the reason in
  the entry is correct.

The #184 condition stays: #184 is not in v1. Whichever of #181 and #184 merges second needs this reviewer's review of the
union of `taint_job`, `COMMANDS` and `.cargo/mutants.toml`. If v1 moves before the merge, the new v1 merge needs
base-merge-check and a full gate on the merge commit.

VERDICT: CLEAN (0 open) at 24fb122f32cb691bf073292f12b6a61448260c55
