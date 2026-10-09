# Integration review: #169 (P5 host audit fixes, branch stage1/p5-audit-host-fixes)

Reviewer: integration reviewer (Opus), `sess-1791517267-0168-401cc27284f329b42ec84da0de9d91e0`. This reviewer runs no
build, test or gate.

## Round 1 — NOT CLEAN on head b9903f94

Reviewed head: `b9903f948579fd3958a24a6caf69db3c474a9447`. Base: v1 `ee7dd16c73a6991b6ef9b3a85d84fd93e57a3230` (merged at
`9d794383`; still `origin/v1`). Scope of this review: the package boundaries and the cross-package parts
(`botster-core-link`, `botster-core/src/real.rs` and `lib.rs`, `tests/slow_real_core.rs`, `.cargo/mutants.toml`) and the
merge. The host crate's internals are the P5 package reviewer's.

### Checked, no finding

- **Merge `9d794383`.** I compared the whole tree with `git merge-tree --write-tree b3b5face ee7dd16c`. Only the five
  conflicted files differ (`mutants.toml`, host `tests.rs`, `deadlines.rs`, `lifecycle.rs`, `slow_real_core.rs`). No
  file that merged cleanly was changed by hand.
- **botster-core-link (A45, A28).**
  - `proof.rs` holds the one hex pair: `pub(crate) to_hex` and `from_hex::<N>`, with the public `token_hex` and
    `token_from_hex`. `hello.rs` and `launch.rs` drop their copies. The behavior is the same: lowercase only, and an exact
    length check. The moved test now also refuses upper case.
  - `MsgError` now carries the decoder's reason. It is no longer `Copy`. No crate outside `botster-core-link` names it, so
    the API change breaks no consumer.
- **The hold on real-process tests.** The held real-process parts of #156 and #157 moved to a later PR (`75552515`). The
  one addition to `tests/slow_real_core.rs` is a config refusal (A47) that starts no process.
- **Removed exclusions (A14, A26).**
  - `commit` and `stop_all_start`: their changed lines are in the diff, so the in-diff run covers them.
  - `link_frame_bound`: its changed line (`engine.rs:448`) is in the diff.
  - `HostEngine::forward` (`run.rs:425-487`) has no changed line, so the in-diff run did not cover the removed entry.
    The removed entry matched every `==` and `&&` of `forward`, and there are exactly three (`run.rs:437`, `441`, `467`).
    The PR body names a failing test for each one, applied by hand on the pool, with two diagnostic logs. `run.rs:496` is
    in `wake_launch_waiters`, not in `forward`.
- **A47.** `check_socket_path` runs before `DataDir::open`, so a `data_dir` whose control socket cannot be bound is
  `InvalidConfig{data_dir}`, and nothing is created.

### H1 MEDIUM — whole-function exclusions of the real edges hide the new A28 decisions

`.cargo/mutants.toml:186-218` excludes every mutant of these functions. Its stated reason: "OS wiring with no decision
of its own". #169 does not change that section, but A28 adds decisions inside the excluded functions:

- `<impl HostEdges for RealEdges>::accept_link`:
  - `error.kind() == io::ErrorKind::WouldBlock`: silent `None` for that kind; any other error is counted
    (`accept_failures += 1`) and recorded;
  - a link that the poll refused is accepted broken (`broken: registered.err()...`).
- `LinkIo::apply`: any registration error marks the link broken, and the function returns `self.broken.is_none()`.
- `set_read_interest` and `set_write_interest`: `if !io.apply(..)`, then `WakeEdge::signal`. Without this, the host is
  not woken to find the broken link.
- `<impl HostEdges for RealEdges>::diagnostics`: the new keys `accept_failures` and `last_accept_error`. Its entry
  (`:219-222`) cites only #164's foreign-file test.

The cited proofs (the slow-tier run of 2026-10-04, and #164's test) predate A28. The new default-tier test
`real::tests::a_link_whose_registration_fails_is_broken` asserts `apply`'s return value and `check()`. But `apply` is
excluded, so no mutation run applies that test to `apply`'s mutants. No test is named for the `WouldBlock` decision, the
failure count, or the wake on a refused registration.

This is the class of #167 E4 and #168 N1. `mutants.toml`'s own rule applies: "Recheck each glue entry when its body or
callers change. New pure decisions require default-tier tests." Fix:
1. Make each decision a pure, tested function, as the existing `wake_after`, `interest_of`, `registration` and
   `retry_interrupted` are. For example, an accept outcome, and "wake when the registration was refused".
2. Narrow or remove the `LinkIo::apply` entry, now that a default-tier test calls `apply` directly.
3. Update the section's reason so that it names the new decision functions.
4. Name a proof for the new `diagnostics` keys, or state why they need none.

Mechanical check, in the PR body: list every operator, branch and match arm that A28 added inside each excluded
`real.rs` function, with its test or its reason.

### Observation for the P5 package reviewer (not counted here)

`crates/botster-core/src/lib.rs` `slow_tests`, lines 308 and 369: `assert_eq!(core.diagnostics()["sessions"], 0)` and
`1` became `assert!(core.diagnostics().is_object())`. The `"sessions"` key still exists (`engine.rs:372`), and the PR body
gives no reason. If the intent is "diagnostics keys are not contract", say so. If not, keep the counts.

VERDICT: NOT CLEAN (1 open: H1 MEDIUM)
