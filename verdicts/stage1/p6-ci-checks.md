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

## Round 5 — CLEAN on head a05ab9a3 (delta: the three round-4 fixes only, by the lead's ruling)

Reviewed head: `a05ab9a380c459cd609b5fafd7f4c17e17a9e7da`, one commit on `24fb122f` (4 files: `gate_decisions.rs`,
`platform_code.rs` and their tests). v1 is still `7aec2bb9`, and the head contains it. P6's gate log
`gates/botster-core-stage1-p6-ci-checks-a05ab9a3-pool-20261009-122351-51352.log` names this head and base `7aec2bb9`.
Results: 1095 default and 248 slow tests passed; the mutants job had 652 mutants (623 caught, 0 missed, 0 timeout, 29
unviable); exit 0. This reviewer read its header and summaries.

This reviewer checked each fix for a gate hole. A gate hole here is a form that makes the check accept a decision as an
I/O shell, or skip a rejection.
- **I/O lists.** `io_path` accepts only listed names: 19 free functions of `std::fs` and 13 of `std::env`, plus
  `IO_FUNCTIONS`. Each listed name does file system or environment I/O. `split_paths` and `join_paths` are not listed.
  An I/O call that the lists miss makes the check refuse an exclusion, which is the strict direction.
- **Command bindings.** A typed `Command` parameter or a started `let` counts only when the function binds its name once
  (`visit_pat_ident`, every pattern and closure parameter). A rebound name is not a command, so a start on it is not I/O,
  which is again the strict direction.
- **`#[path]` on an inline module.** `UnlistedForms` visits every module of the file, under any `cfg`. Each rejection
  is an error, and the file adds no exclusion lines. So the rejection no longer depends on the OS of the gate.

Observation for P6's next PR (the lists PR; not counted, by the lead's ruling): `CommandBindings` does not see bindings
inside an unexpanded macro (for example, a statement macro that writes `let cmd = Pure;`). A name bound once outside the
macro and once inside it is counted once. Under plan 23c, a macro in a function body could be an unlisted form that fails
closed.

The conditions stay:
- #184 is not in v1. Whichever of #181 and #184 merges second needs this reviewer's review of the union of `taint_job`,
  `COMMANDS` and `.cargo/mutants.toml`.
- #198 also changes `.cargo/mutants.toml`. Whichever of #181 and #198 merges second needs the union review and a full gate.
- If v1 moves before the merge, the new v1 merge needs base-merge-check and a full gate on the merge commit.

VERDICT: CLEAN (0 open) at a05ab9a380c459cd609b5fafd7f4c17e17a9e7da

### Correction after round 5 (same head a05ab9a3) — CLEAN WITHDRAWN (two gate holes in the command bindings)

Round 5 said that no form makes the check accept a decision as an I/O shell. That is wrong in two cases. Both are gate
holes, so both block #181 by the lead's round-5 ruling.
1. **The P6 package reviewer's B5 (block scope), missed here.** `Index::function` (`gate_decisions.rs:601`) resolves
   the command bindings (`command_locals`, `:575-588`) with the function's scopes before `visit` runs. `visit_block`
   (`:632`) pushes a block's own `use` declarations only later. So in
   `{ use crate::ci::pure::Command; let cmd = Command::new(); cmd.status(); }`, the root of `cmd` resolves through the
   file's `use std::process::Command`. The check then takes `pure::Command::status` as a process start, and a forwarding
   decision passes as an I/O shell. Fix: resolve each binding at its own scope, or reject a command binding under a block
   `use` as an unlisted form (with the form and the file), plus a check-level fixture of the rejected exclusion.
2. **The macro binding (this reviewer's round 5 note), which the lead promoted to a blocker.** A name rebound inside an
   unexpanded statement macro is counted once, so the single-binding rule passes it. P6 adds a commit: a statement macro
   in a function with a command binding fails as an unlisted form.

VERDICT: NOT CLEAN at a05ab9a380c459cd609b5fafd7f4c17e17a9e7da (2 open: B5 block scope HIGH, the package reviewer's
finding, confirmed here; the macro binding, promoted by the lead)

## Round 6 — NOT CLEAN on head 60a80f21

Reviewer: integration reviewer (Astra), `sess-1791575341-0172-bec01ec06119f11c948f14371195fa92`.
Reviewed head: `60a80f21d9e0da84deb47f471faddd4944b00ed7`.
Scope: the lead's replacement delta from `a05ab9a380c459cd609b5fafd7f4c17e17a9e7da`, plus the required #198 union.
HIGH is correct under rules 1 and 3. Gate holes remain blockers under the lead's round-limit ruling.
The reviewer read the complete delta, its tests, shared resolver code, PR description, and named gate log.
The reviewer ran no test, build, or gate.

### R6-1 HIGH — a macro path can name quoted syntax instead of executed code

`Index::visit_macro` (`gate_decisions.rs:797-831`) resolves only the first segment through a `use` binding.
`Uses::of` (`process_check.rs:155-165`) records only `ItemUse`, not module declarations.
Thus a local module can replace the presumed external crate without changing the macro classification:

```rust
mod anyhow { pub use syn::parse_quote as bail; }
fn forwarded(code: Option<i32>) -> Result<(), ()> {
    let _: syn::Expr = anyhow::bail!(std::fs::read("unused"));
    mutation_verdict(code)
}
```

Rust resolves this macro to `syn::parse_quote`, which creates syntax without reading a file.
The index instead accepts `[anyhow, bail]` as an argument macro and records the quoted read as I/O.
With a tested, unexcluded `mutation_verdict` and reason `mutation_verdict (verdicts)`, the whole-body exclusion of `forwarded` passes.

An internal glob has the same problem:

```rust
mod macros { pub use syn::parse_quote as println; }
use self::macros::*;
```

With `println!(std::fs::read("unused"))` in the same forwarding function, `has_foreign_glob` exempts the import.
The index treats the quoted read as executed I/O again.
Reject unsupported macro identities with the form and file, and add check-level rejection fixtures.
The closed-form rule does not require compiler-style resolution.
The package reviewer independently confirmed both cases after receiving this finding.

### R6-2 / B5 HIGH — a local type can replace the stored command identity

The new function rejection checks `CommandBindings::uses`, which only `visit_item_use` sets.
A local type alias requires no `use`:

```rust
use std::process::Command;
fn forwarded(code: Option<i32>) -> Result<()> {
    type Command = pure::Command;
    let cmd = Command::new();
    cmd.status();
    mutation_verdict(code)
}
```

Here `pure::Command` is the existing fixture's pure type, whose `status` does nothing.
The alias is not a pattern binding. `cmd` counts once, and `uses` stays false.
`command_locals` resolves the constructor through the file import and stores `std::process::Command::new`.
`process_start` then treats the pure `status` call as I/O, so the same whole-body exclusion passes.

The package reviewer independently supplied the local-struct variant before reading this type-alias report:
`struct Command; impl Command { fn new() -> Self { Self } fn status(&self) {} }` inside the function.
Both variants have the same root cause and count as one finding.
Reject unsupported local item shadows with the form and file, and add check-level fixtures.

### R6-3 HIGH — opaque macro tokens create a false command binding

`CommandBindings::visit_macro` reads every argument list that parses as expressions.
Unlike `Index::visit_macro`, it does not respect `OPAQUE_MACROS`.
For example:

```rust
use std::process::Command;
struct Pure;
impl Pure { fn status(&self) {} }
static CMD: Pure = Pure;
fn forwarded(code: Option<i32>) -> Result<()> {
    let _: syn::Expr = syn::parse_quote!({ let CMD = Command::new("unused"); });
    CMD.status();
    mutation_verdict(code)
}
```

The binding collector reads the quoted block and stores one apparent command binding named `CMD`.
The expression index skips that opaque macro, but uses the stored binding for the real `CMD.status()` call.
That call operates on the pure static, not a process command.
The forwarding function is again classified as I/O, so its whole-body exclusion passes with the usual decision citation.

Make binding collection respect the same opaque boundary, or reject the unsupported combination with the form and file.
Add a check-level fixture that rejects this exclusion.
This case needs no macro shadow or unusual import.

All three findings come from source tracing through the reviewed delta. The reviewer did not execute these fixtures.
The reviewer sent each finding directly to P6 and the package reviewer.

### Checked fixes, union, and gate evidence

- The prior block-`use` fixture now fails with the form and file. The unlisted `rebind!` fixture also fails.
  The new tests cover bindings in listed macro arguments, external globs, explicit macro aliases, and nearest imports.
  They do not cover the three cases above.
- Merge parents are `8b28d9a18004099fec6ae4146076b5cc63f71e9a` and current v1 `58d6663204b50ce6c42d467e4fd6715ab46145dd`.
  The automatic merge is conflict-free, tree `b26ba7a14b8e288168f891876bd91e9c42ddc1b6`.
  The committed tree differs only by the accepted temporary process-check allowance for #198's bounded accept.
  The reviewer read that added hunk. The allowance adds one site; the legacy `launch` site keeps its separate entry.
- The imported worker, worker-core, testkit, and pending-list paths match v1.
  The two new PTY exclusions retain #198's reviewed regexes and strict decision/proof citations.
  The union has no additional finding. P6 still owes shared bounded accept, migration of this site, and removal of its allowance.
- The named log is `~/botster-sessions/gates/botster-core-stage1-p6-ci-checks-60a80f21-pool-20261009-132005-3418.log`.
  It runs full `cargo xtask ci` on Linux, on this exact head and current v1 base.
  Ancestry is confirmed. All ten steps pass: 1139 default tests, 254 slow tests, and 46 conformance IDs.
  Mutations: 672 tested, 643 caught, 29 unviable, 0 missed, 0 timeout. The gate exits 0.
  Both #198 parent-death proofs and both real-PTY cancellation proofs run and pass.
  `mutants-cited` checks 157 names; `process-check` allows 111 sites; `gate-decisions` checks 1215 xtask mutants.
- The package reviewer's current-head messages confirm R6-1 and R6-2/B5; its committed round was still pending during this review.

The #181/#198 union has been reviewed at this head, but #181 cannot merge with these gate holes.
The separate #181/#184 union condition remains.

VERDICT: NOT CLEAN (3 open: R6-1, R6-2/B5, R6-3; all HIGH) at 60a80f21d9e0da84deb47f471faddd4944b00ed7
