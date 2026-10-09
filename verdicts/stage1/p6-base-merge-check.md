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
