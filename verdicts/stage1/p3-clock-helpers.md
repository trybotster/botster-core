# Integration review: #178 (P3 clock helpers, the follow-up of #177; branch stage1/p3-clock-helpers)

Reviewer: integration reviewer (Opus), `sess-1791517267-0168-401cc27284f329b42ec84da0de9d91e0`. This reviewer runs no
build, test or gate. The lead assigned this reviewer to the PR (the #177 enforcement decision).

## Round 1 — NOT CLEAN on head 25e1dbe3

Reviewed head: `25e1dbe3b1db19e3de23bf14051dc525d8973fbf`, range `da43a1b1..25e1dbe3` (2 commits, 14 files, +91 -51). The
base is v1 `da43a1b1f85fd0b62da2e69767fc6ea0ba8cd594` (#177 merged), which is the current v1. P3's gate log:
`…-p3-clock-helpers-25e1dbe3-pool-20261009-045931-76781.log` (every step passes; no mutants, because every changed line is
`cfg(test)` code, a `tests/` file or `build.rs`). This reviewer did not read it.

### Checked, no finding

- **Every remaining allowance is on a one-call item.** At the head, `allow(clippy::disallowed_methods)` appears only on:
  - `real_now` in `botster-core-host/src/tests.rs`, `botster-core/src/lib.rs`, `botster-core/tests/common/mod.rs`,
    `botster-core-sys/tests/common/guard_platform.rs` and `botster-worker-core/src/worker/tests.rs`;
  - `now` in `botster-guardian-core/tests/lifecycle.rs` (unchanged);
  - the three statements in `botster-core-sys/src/signal.rs` (unchanged);
  - the five wrappers in `botster-terminal-ghostty/build.rs`;
  - the text fixture in `xtask/src/signals.rs`, which is a string.

  Each helper body is the one call. No file-wide `#![allow]` is left (`slow_real_core.rs`, `slow_facade_worker.rs` and
  `build.rs` lost theirs).
- **The testkit removals.** `botster-core-testkit/clippy.toml` bans only the signal calls, so the seven removed clock
  allowances suppressed nothing. The clock calls in those tests stay legal.
- **No behavior change.** Each `Instant::now()` call became `real_now()`, and each `env::`/`fs::` call in `build.rs`
  became a same-signature wrapper. No deadline value changed.
- **`guard_platform.rs` is compiled into botster-core's slow tests too**, where Core's clock ban applies. The helper
  carries the one allowance for both crates.

### W1 LOW — `botster_core::real_now` is dead code in the default-tier test build

`crates/botster-core/src/lib.rs:250` gates `real_now` with `#[cfg(test)]` only. Its two users, `lib.rs:277` and
`real.rs:812`, are in `mod slow_tests`, which is `#[cfg(test)] #[cfg(feature = "slow")]` (`lib.rs:256-257`,
`real.rs:608-609`). So `cargo test -p botster-core` without `slow` compiles the function with no user, and rustc emits a
`dead_code` warning. CI does not see it: the clippy job runs with `--all-features` (#177), and no test build denies
warnings. Before this PR, the allowance sat inside the slow-only code, so no such item existed. Fix: gate it with
`#[cfg(all(test, feature = "slow"))]`, or move it into the slow-only code.

VERDICT: NOT CLEAN (1 open: W1 LOW)
