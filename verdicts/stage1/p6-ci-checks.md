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
