# Plan: Gate IncrementalAttach uses in engine/botster.rs behind local-runtime

- Ticket: `ticket_1787251441_640678`
- Run: `run_1787251456_699480` (pipeline `botster_stack_delivery`, step `botster_stack_plan`)
- Target repository: `botster-core` (`trybotster/botster-core`)
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Base: `8fce204` on `main`; branch `project-pipelines/ticket_1787251441_640678`

## Repository playbook loaded

- [[botster-core-playbook]] (repository ownership charter)

## Other playbooks and atomic notes loaded

- [[planner-playbook]]
- [[botster-planner-playbook]]
- [[botster-core local process runtime is feature-gated from contract-only embeds]]
- [[TUI bin only Core 8fce204 builds require local runtime feature unification]]
- [[botster-core uses CI-owned Cargo commands because it has no test script]]
- [[botster core contract surface needs consumer proof]]
- [[rustdoc intra doc links break on feature gated items]]
- [[vault example paths are not repository placement conventions]]
- [[botster-architecture]] and [[cli-patterns]] (scanned for feature-gate guidance)
- Not loaded: [[botster runtime teardown lenses]]. This ticket changes one `cfg` attribute on a build
  configuration. It does not change peer lifecycle, SessionIo/ClientWorker teardown, ownership, or
  terminal-state behavior. Runtime-teardown class does not apply.
- Not loaded: [[project-pipelines-playbook]]. No Project Pipelines package or plugin path is in scope.

## Context loaded

- `crates/botster-core/Cargo.toml`: `default = ["local-runtime"]`; `local-runtime = ["dep:portable-pty", "dep:libc"]`.
- `crates/botster-core/src/engine/botster.rs` (2464 lines): `IncrementalAttach` is defined at line 94-103
  under `#[cfg(feature = "local-runtime")]`. Every use is inside gated items
  (`WorkerBackedBotsterEngine` impl, `DefaultBotsterEngine` impl, gated tests) except the free function
  `suppress_attach_terminal_output` at lines 1764-1788, which has no `cfg` attribute.
- Sibling free function `append_engine_output` at line 1753 carries `#[cfg(feature = "local-runtime")]`.
- Regression origin: commit `968792d` ("Keep bound live bytes visible across ProcessExited writer-barrier
  rounds") removed the `#[cfg(feature = "local-runtime")]` line above `suppress_attach_terminal_output`
  (hunk `@@ -1716,7 +1761,6 @@`). At the TUI main pin `fd66efdc` the function was gated.
- No other crate in the workspace references `IncrementalAttach` or `suppress_attach_terminal_output`.
- Reproduction on clean `8fce204`: `BOTSTER_ENV=test cargo check -p botster-core --no-default-features --lib`
  fails with `error[E0412]: cannot find type IncrementalAttach in this scope` at `botster.rs:1767`.
- CI (`.github/workflows/ci.yml`) runs fmt, workspace clippy, workspace test, doc test, `cargo doc --workspace`,
  node smoke, and the Unix PTY test. CI runs no contract-only (`--no-default-features`) lane.
- `README.md` "Local verification" already documents the contract-only lane:
  `cargo doc -p botster-core --no-default-features --no-deps` and
  `cargo test -p botster-core --no-default-features --lib`.
- Downstream pins: botster-tui main `dc7d600` pins Core `fd66efdc` with `default-features = false`
  (`crates/botster-tui/Cargo.toml:9`) and pulls `botster-hub-test-support` only under dev-dependencies.
  botster-hub main pins Core `8fce204`. Hub pin roll is `ticket_1787251447_191212` (target `tgt_7e208a0c76a44980a83b63af976b1f22`).
- Prior-art placement: plans live under `docs/archive/plans/`, implementation reports under `docs/reports/`
  (see `exact-subscription-and-registry-state-queries.md` and `*-implement.md`). `docs/plans/` is a retired stub.

## Scope

1. Restore `#[cfg(feature = "local-runtime")]` on `fn suppress_attach_terminal_output` in
   `crates/botster-core/src/engine/botster.rs` (line 1764). Use the attribute form, not a move into the
   gated `impl WorkerBackedBotsterEngine`. Reason: the attribute form is one line, matches the sibling
   `append_engine_output`, and keeps the diff reviewable against `968792d`.
2. Add one CI step to the `verify` job in `.github/workflows/ci.yml` that runs the README-documented
   contract-only lane: `cargo test -p botster-core --no-default-features --lib`. Place it after
   "Test workspace". Reason: this exact configuration is what `968792d` broke, the README already names
   the command as part of local verification, and no current gate executes it. Feature unification in
   `cargo test --workspace` cannot catch this class of defect.
   This step is the one discretionary change in this plan. The product fix in item 1 does not depend on it.
   Alternative considered: skip the CI lane and rely on downstream pin-roll builds. Rejected because the
   downstream builds run after merge and cannot block a Core regression.
3. Persist the implementation report at `docs/reports/gate-incremental-attach-local-runtime-implement.md`.

## Non-scope

- Do not change `IncrementalAttach`, attach phases, or any runtime behavior.
- Do not change feature defaults, `Cargo.toml`, or the `local-runtime` boundary.
- Do not gate the imports at `botster.rs:15-19` (`TerminalAdapter`, `BindTerminalAdapterError`,
  `DetachTerminalSubscriptionResult`, `TerminalCapabilitySet`, `TerminalSubscriptionGeneration`,
  `TerminalSubscriptionRecord`). They produce `unused_imports` warnings only under `--no-default-features`.
  They are not build failures and are not `IncrementalAttach` uses.
- Do not add a `cargo clippy --no-default-features -- -D warnings` lane. Probe result: with the gate applied,
  that lane fails with 7 pre-existing warnings (the imports above plus dead code in
  `crates/botster-core/src/engine/subscription_multiplexer.rs`: `attach_snapshot`, `begin_snapshot_attach`,
  `snapshot_attach_frame`, `complete_snapshot_attach`). That cleanup is a separate ticket candidate.
- Do not roll TUI or Hub pins. Those are separate tickets on their own targets.
- Do not create a test wrapper script. See [[botster-core uses CI-owned Cargo commands because it has no test script]].

## Ownership boundaries and cross-repo dependencies

- botster-core owns the `local-runtime` feature boundary, `engine/botster.rs`, and its CI.
- Consumers: botster-hub (`ticket_1787251447_191212`, pin roll after merge) and botster-tui
  (`ticket_1786663585_944018`, open; its Review finding `finding_1787251254_962248` triggered this ticket).
  Both are downstream consumers, not prerequisites. No dependency registration is needed for this run.
- No Hub session-type eligibility parent applies.

## Assumptions and unknowns

- Assumption: the attribute form is acceptable to Plan Review. The ticket allows either the attribute form
  or a move into the gated impl.
- Assumption: direct-merge policy applies as in prior Core tickets in this project.
- Verified: botster-tui main (`dc7d600`, pin `fd66efdc`) builds green today with
  `cargo build -p botster-tui --locked`. The failure appears only at Core revisions that include `968792d`.
- Unknown: no separate TUI pin-roll ticket is visible in project `project_1786663508_823105`. The open TUI
  ticket `ticket_1786663585_944018` is expected to roll its own pin. Flag to the human if that is wrong.

## Affected surfaces and files

- `crates/botster-core/src/engine/botster.rs` — one added attribute line at 1764.
- `.github/workflows/ci.yml` — one added step in the `verify` job.
- `docs/archive/plans/gate-incremental-attach-local-runtime.md` — this plan.
- `docs/reports/gate-incremental-attach-local-runtime-implement.md` — implementation report (Implement step).

## Risks

- Low product risk: the function is only called from gated code (lines 1260 and 1450 inside the gated
  `WorkerBackedBotsterEngine` impl). Default-feature builds are unchanged.
- Process risk: feature-unified workspace gates stay green with or without the fix. Only a
  `--no-default-features` command proves the change. Acceptance checks below require it.
- Downstream-proof cost: the patched TUI build needs the Ghostty submodule in the run worktree
  (`git submodule update --init --recursive`; ~526 MB) and Zig 0.16.0 (present on this host via mise).
- `[patch]` rewrites `Cargo.lock`, so the pre-merge TUI proof cannot use `--locked`. The `--locked` proof
  belongs to the pin-roll tickets after merge.

## Acceptance checks and tests

Worktree hygiene: tracked `.gitignore` is 63 bytes and intact; the worktree path has no colon, so no
`CARGO_TARGET_DIR` override is required.

Core gates (charter and CI commands):

```sh
cargo fmt --all -- --check
BOTSTER_ENV=test cargo clippy --workspace --all-targets -- -D warnings
BOTSTER_ENV=test cargo test --workspace
BOTSTER_ENV=test cargo test --doc --workspace
cargo doc --workspace --no-deps
```

Contract-only proof (red before, green after; each command was probed with the one-line gate applied):

```sh
BOTSTER_ENV=test cargo check -p botster-core --no-default-features --lib          # red on 8fce204: E0412 at botster.rs:1767
BOTSTER_ENV=test cargo check -p botster-core --no-default-features --all-targets  # probed green
BOTSTER_ENV=test cargo test  -p botster-core --no-default-features --lib          # probed green: 13 passed
cargo doc -p botster-core --no-default-features --no-deps                         # probed green
```

Downstream-shaped TUI proof (pre-merge, in this run):

1. In the run worktree: `git submodule update --init --recursive`.
2. Copy botster-tui main (`dc7d600`) to a scratch directory (exclude `target` and `.git`).
3. Append to the scratch `Cargo.toml`:
   `[patch."https://github.com/trybotster/botster-core.git"]` with path entries for `botster-core`,
   `botster-core-test-support`, `botster-terminal-ghostty`, `botster-terminal-protocol`, and
   `botster-terminal-protocol-client`, each pointing at the run worktree `crates/<name>`.
4. Run `cargo build -p botster-tui` (no `--locked`). Expected: green.
5. Run `cargo tree -p botster-tui -e features,no-dev -i botster-core`. Expected: no `local-runtime` feature
   in the non-dev graph. This proves the build is contract-only.
6. Red oracle: repeat step 4 with the gate attribute removed (or with the patch pointing at an unmodified
   `8fce204` checkout). Expected: `error[E0412]: cannot find type IncrementalAttach`.

Post-merge proof (owned by pin-roll tickets, not this run):

- `cargo build -p botster-tui --locked` against the merged Core revision.
- Hub `ticket_1787251447_191212` contract-only consumer build against the merged revision.

CI proof: the new CI step runs `cargo test -p botster-core --no-default-features --lib` and is green on
the branch.

## Vault gaps worth capturing

- Core CI omits the README-documented contract-only lane. Capture after the CI step lands:
  "botster-core CI runs the contract-only test lane because feature unification hides no-default-feature
  breaks".
- Update [[TUI bin only Core 8fce204 builds require local runtime feature unification]] after merge with the
  regression origin (`968792d`) and the fixing revision.
- Candidate follow-up ticket, not vault-worthy yet: `cargo clippy -p botster-core --no-default-features --lib -- -D warnings`
  is red with 7 pre-existing dead-code and unused-import warnings.
