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

## Round 2 — CLEAN on head a7c73feb

Reviewed head: `a7c73febcfeafca0f462ea14d5121a516d5a1a10`. Delta `b9903f94..a7c73feb`: five commits, fast-forward, in
`botster-core` `real.rs`, `.cargo/mutants.toml`, two host test files and `DESIGN.md`. v1 is still `ee7dd16c`. This reviewer
ran no build, test or gate.

- **H1 CLOSED.** The A28 decisions are pure functions. They are not excluded, and they have default-tier tests:
  - `AcceptFailures::take`: WouldBlock gives `None`; any other error is counted and recorded.
  - `broken_after` and `LinkIo::record`: a refusal breaks the link, and a broken link stays broken.
  - `edge_diagnostics`: `a_failed_accept_is_counted_and_reported` asserts every key.
- **The shells.** The excluded functions now only call these decision functions, except for one remaining branch: the
  `if !io.apply(..) { WakeEdge::signal }` of `set_read_interest` and `set_write_interest`.
  - The Linux slow test `a_link_that_the_poll_refuses_wakes_the_host_and_fails` catches that branch's `delete !` mutant.
    It asserts no wake after an accepted change and a wake after a refused one, for both functions.
  - The `LinkIo::apply` entry stays. Its `-> bool` mutants are caught by the Linux slow tests that the entry names.
  - The real-edges section of `mutants.toml` now names the new decision functions and both slow tests.
  - `RealEdges::diagnostics` names `edge_diagnostics` and its test.
- **The one uncaught hand mutation.** At the `accept_link` call site, `broken_after(&registered, None)` → `None` is not
  caught. cargo-mutants does not generate argument substitutions, and the PR body gives the reason: a first registration
  is refused only on kernel resource exhaustion.
- **The hold.** Neither new slow test starts a process. `Core::open` spawns nothing, and the tests use real sockets and the
  poll only. The one wait is a socket read timeout, with its timer marker directly above the call, as a failsafe for a
  link that the host does not close.
- **Evidence (from the READY):** static, test-budget and slow passed, and the no-terminate in-diff mutation run had 0
  missed and 0 TIMEOUT, on this exact head. The P5 package findings F16 to F18 are the package reviewer's. The gate on
  this exact head is the lead's check.

VERDICT: CLEAN (0 open) at a7c73febcfeafca0f462ea14d5121a516d5a1a10

### Correction after round 2 — the CLEAN at a7c73feb is WITHDRAWN

Round 2 said: "The one wait is a socket read timeout". That is false. The new test
`a_link_that_the_poll_refuses_wakes_the_host_and_fails` ends with `client.read_to_end(&mut rest)` (`real.rs:794`), with no
read deadline. If `link_close` kept the peer open, the test would block forever, and a no-terminate mutation run would
show it as a TIMEOUT. The adjacent host test sets a marked deadline (`real.rs:837`). The P5 package reviewer found this
(P5-F19 MEDIUM), and the finding is theirs. This reviewer missed it, although the rule is to grep every changed test file
for raw blocking reads and waits before a CLEAN. `real.rs:704` and `:722` are on v1 and are outside this PR's diff.

VERDICT: NOT CLEAN at a7c73febcfeafca0f462ea14d5121a516d5a1a10 (P5-F19 open, the package reviewer's; no integration
finding open)

## Round 3 — CLEAN on head 9343f20d

Reviewed head: `9343f20df613c2756c7cdf781fc3dd6bfa9bfad1`. Delta `a7c73feb..9343f20d`, one commit, test only. v1 is still
`ee7dd16c`.

- **P5-F19 fixed (the package reviewer's finding).** `real.rs:792-795` sets a marked 8 s read timeout before
  `read_to_end`. This is the same value as the adjacent host test, so the PR adds no new time value. If the link stays
  open, the read ends with a timeout error, and the `ConnectionReset` assertion fails instead of the test hanging.
- **Mechanical check, whole PR.** I grepped every added line of `origin/v1..9343f20d` under `crates/` for `read_line(`,
  `read_to_end`, `read_exact`, `.recv()`, `.wait()`, `wait_with_output`, `sleep(`, `.join()`, `loop {` and `while`. The
  only blocking calls are the two `read_to_end`s, at `:798` and `:844`, and both are now bounded. The other hits are
  comments.
- Round 2's review of H1 stands.

VERDICT: CLEAN (0 open) at 9343f20df613c2756c7cdf781fc3dd6bfa9bfad1
