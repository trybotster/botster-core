# Integration review: #170 (P6 base-merge-check, branch stage1/p6-base-merge-check)

Reviewer: integration reviewer (Opus), `sess-1791517267-0168-401cc27284f329b42ec84da0de9d91e0`. This reviewer runs no
build, test or gate.

## Round 1 — NOT CLEAN on head df04024b

Reviewed head: `df04024b20e4213944a12360a991930facaf03bc`. Base: v1 `ee7dd16c73a6991b6ef9b3a85d84fd93e57a3230` (the
branch starts on it; no merge). Scope: `xtask` (base_merge, main, caps), `botster-core-sys/tests/common/guard_platform.rs`
(the slow tests of `botster-core` and `botster-worker` include it), and `.cargo/mutants.toml`.

### Checked, no finding

- **The base-merge-check logic matches BUILD.md `629c177` (Q7) and plan r22 section 8.**
  - (1) `merge-tree` of the reviewed head and the new merge base.
  - (2) The base's changed paths (`reviewed_base..new_base`) do not meet the reviewed diff's paths.
  - (3) `new_base..new` is byte-identical to `reviewed_base..reviewed`, with `--binary --full-index --no-renames`, so
    modes and gitlinks are covered.
  - Conditions 2 and 3 together force the new tree to equal the clean trial merge. So the check does not need to prove
    "only a merge commit": any other change would alter the diff.
  - The check fails safe. With a stale `origin/v1`, the new merge base is too old, and condition 3 fails. A merge of a
    branch that is not v1 also changes the diff.
  - The two descent checks (the new head descends from the reviewed head; the base moves forward) are extra safety.
- **The decisions are pure functions with tests:** `conflicts` (it parses the `--name-only --no-messages` output),
  `overlap`, `first_difference`, `ancestry` and `judge`.
- **main.rs.** `choose` is a pure lookup over `COMMANDS`. A test keeps `USAGE` and `COMMANDS` equal. The `run` shell
  matches an exhaustive enum with no wildcard, so its exclusion hides no arm.
- **guard_platform.rs (P6 owns real-process test code).** `pidfd_open` EINVAL means "gone" when the flags are empty and
  the pid is positive. A reaped process-group leader whose pid is still the pgid of live members has no `PIDTYPE_TGID`
  task, so `pidfd_open` refuses it with EINVAL. That is the race of the red gate at `a980c51f`. The new Linux test forces
  the same kernel check with a non-leader thread id, with no race and no child process.
- **caps.rs.** The "every cargo spawn is capped" test now walks the module directories, and it asserts that
  `base_merge/tests.rs` is among the files.

### G1 MEDIUM — the check's pass/fail exit is decided inside the excluded shell

`xtask/src/base_merge.rs` `command` ends with:

    let (report, pass) = judge(&facts).map_err(anyhow::Error::msg)?;
    print!("{report}");
    if !pass {
        bail!("base-merge-check failed");
    }

`.cargo/mutants.toml` excludes every mutant `in command`, so the `delete !` mutant is never tested. With that mutant, a
failed check exits 0, and a passed check exits non-zero. The printed report still says "result: FAIL", but any caller
that trusts the exit status would skip a required review round.

Plan r22 (CLEAN at `eeb31087`, section 8): "The logic that decides whether a gate step passes is a pure function with
tests ... An exclusion may cover only the I/O shell that calls it, never the decision." This is the same class as #167
E4 (`mutants_job`'s `if !status.success() bail!`).

Fix: put the exit decision in a pure, tested function. For example, `judge` (or a small `verdict(pass)`) returns
`Result<String, String>` with the failed report as the error, and the shell only propagates it with `?`. Then name that
function in the exclusion's reason.

The shell's `ok` closure has a second decision, `if code != Some(0) { bail!(..) }`, which turns a failed git call into
a failed check. Its mutant fails safe here (every call then fails). Still, name it in the reason or make it pure (as
`ancestry` already is). Mechanical check, in the PR body: list every branch inside `command` and its test or reason.

VERDICT: NOT CLEAN (1 open: G1 MEDIUM)

### Correction after round 1 (same head df04024b)

Round 1 said that condition 3 covers "modes and gitlinks". That is false under user git config. The P6 package reviewer
proved a false PASS: with `diff.ignoreSubmodules=all`, a merge carries an unreviewed gitlink change (for example
`vendor/ghostty`), but the porcelain `git diff` hides it from both diffs and from `--name-only`. Every condition then
passes. That finding is the P6 package reviewer's. This reviewer agrees. Its root differs from G1's: the shell trusts git
output that the user's config can change.

What the fix must cover (the whole class, not only submodules): the shell must not read porcelain diff config that can
hide or rewrite content. That config includes `diff.ignoreSubmodules`, `diff.external`/`GIT_EXTERNAL_DIFF`, textconv
drivers, and `diff.relative`. Use plumbing (`git diff-tree -r --binary --full-index --no-renames`) with explicit
`--ignore-submodules=none --no-textconv --no-ext-diff`, or pin the config with `git -c`. A test with a hostile config
must show a FAIL.

VERDICT (unchanged): NOT CLEAN (G1 open; the gitlink false acceptance is the P6 package reviewer's finding)

## Round 2 — NOT CLEAN on head bcd5f18e

Reviewed head: `bcd5f18ea4d2ea6d38377ca5ab3206b5c41707f5`. Delta `df04024b..bcd5f18e`, one commit: `base_merge.rs`, its
tests, `main.rs` and `.cargo/mutants.toml`. v1 is still `ee7dd16c`.

- **G1 CLOSED.**
  - `judge` returns the report as `Ok` on PASS and as `Err` on FAIL, and `succeeded` decides a git exit code. Both are
    pure and tested.
  - `check` and `command` only propagate with `?`, so the exit status comes from `judge`.
  - Only the whole-body `command -> Ok(())` and `run -> Ok(())` replacements stay excluded. Each entry names its decision
    functions and states that the replaced body prints no report, which is the #167 `mutants_job` rule (plan r22,
    section 8).
- **The diff config class (the package reviewer's BM1) is covered.**
  - The diffs use plumbing `diff-tree -r --no-renames --no-ext-diff --no-textconv --ignore-submodules=none
    --no-relative`.
  - git runs with `GIT_CONFIG_NOSYSTEM=1` and `GIT_CONFIG_GLOBAL=/dev/null`, with `GIT_EXTERNAL_DIFF` and `GIT_DIFF_OPTS`
    removed.
  - Repository-local config is still read. The flags defeat its diff settings, and a hostile merge driver cannot matter:
    a conflict needs a path that both sides changed, and condition 2 already fails on any such path.
  - Each hostile-config test first proves that its setting hides the change from a plain `git diff`, then that `check`
    fails on condition 3. That is a good red-on-revert design.

### G2 MEDIUM — new real-process test code outside `botster-test-process`, with an unbounded child wait

`xtask/src/base_merge/tests.rs` `Repo::git` starts `git` with `Command::new("git")...output()` in the default tier. The
tests `a_real_base_only_merge_passes`, the three hostile-config tests and `a_git_failure_fails_the_check` all use it.
Plan r22 (CLEAN at `eeb31087`) applies:
- Section 6.1: "Real-process test code (spawn, group ownership, waits, cleanup, leak detection) lives only in
  `botster-test-process`".
- The section 8 check: "test code outside `botster-test-process` uses no raw `Child::wait` or `wait_with_output`".
  `Command::output()` is that same unbounded wait.

git is a short-lived tool that exits by itself, so this may be outside the rule's intent. But the plan does not say so,
and P6's own check will have to decide it. Close it one of two ways:
- (a) a lead ruling that short-lived tool processes (git, cargo) in xtask tests are outside the real-process rule, with
  that scope written into the check; or
- (b) start git through `botster-test-process`'s bounded wait, after PR A lands.

VERDICT: NOT CLEAN (1 open: G2 MEDIUM; the package reviewer's BM1 to BM3 are theirs)

## Round 3 — CLEAN on head bcd5f18e (lead ruling on G2)

Reviewed head: `bcd5f18ea4d2ea6d38377ca5ab3206b5c41707f5`, unchanged since round 2. v1 is still `ee7dd16c`.

- **G2 CLOSED by lead ruling (a), for #170 only, with no permanent exemption.** The tests prove the HIGH config fix
  today. git runs with no group or descendants, and nextest's leak-timeout sees a git child that holds the output pipes.
  Because BUILD rule 5 requires every wait to be bounded, the lead also ruled on the follow-up:
  - `botster-test-process` gets a bounded `run_to_completion` helper for short-lived tools (git, cargo).
  - The section 8 check covers raw `Command::output`, `status` and `wait` in ALL test code, xtask included.
  - P6 migrates these tests to the helper in PR C.

  Carry for P6 PR C: this reviewer checks that `xtask/src/base_merge/tests.rs` `Repo::git` uses the helper.
- No other finding is open. The P6 package reviewer is CLEAN at this head (`cd12645d`, BM1 to BM3 closed).

VERDICT: CLEAN (0 open) at bcd5f18ea4d2ea6d38377ca5ab3206b5c41707f5
