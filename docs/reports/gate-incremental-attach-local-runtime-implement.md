# Implementation report: Gate IncrementalAttach uses behind local-runtime

Ticket: `ticket_1787251441_640678`
Run: `run_1787251456_699480`
Step: `botster_stack_implement`
Plan: `docs/archive/plans/gate-incremental-attach-local-runtime.md` at `03c1579`
Plan Review: `review_1787252449_390729` approved

## Target repository and target_id

- Target repository: `botster-core` (`trybotster/botster-core`)
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Independent `list_spawn_targets` resolution: admitted `botster-core` at that target id
- Approved plan routing: same repository and target id
- Worktree: the pipeline-provided ticket worktree
- Merge policy: `direct` (no pull request)
- Runtime-teardown class: not applicable. This change restores one `cfg` attribute. It does not change attach phases, teardown, or terminal-state behavior. [[botster runtime teardown lenses]] was not loaded.

## Repository playbook and other playbooks/notes applied

Repository charter: [[botster-core-playbook]]

Role and overlay:

- [[implementer-playbook]]
- [[botster-implementer-playbook]]
- [[botster-architecture]]
- [[cli-patterns]]
- [[spa-patterns]] — loaded per overlay; no SPA surface in this change
- [[botster-core uses CI-owned Cargo commands because it has no test script]]
- [[botster-core local process runtime is feature-gated from contract-only embeds]]
- [[TUI bin only Core 8fce204 builds require local runtime feature unification]]
- [[botster core contract surface needs consumer proof]]
- [[rustdoc intra doc links break on feature gated items]]
- [[vault example paths are not repository placement conventions]]
- [[implementation artifacts must match actual git state]]
- [[implement gate must verify committed work and pr link before review]]
- [[implementation steps must persist report artifacts for review]]
- [[pipeline vault checklists must cite exact resolvable note titles]]
- [[pipeline artifacts should use path neutral worktree references]]

Not loaded:

- [[project-pipelines-playbook]] — package/plugin paths are out of scope
- Other repository charters — this run stays inside `botster-core`
- [[botster runtime teardown lenses]] — not a runtime-teardown-class ticket

Convention conflicts: none.

Implement checklist: `checklist_1787252773_800647` (run-scoped Implement visit). Plan checklist `checklist_1787251709_252513` was left unchanged.

## Botster layers changed

- `botster-core` `local-runtime` feature boundary on `engine/botster.rs`
- Core CI verify job: contract-only library test lane

No Hub, Web, TUI, Ghostty crate, plugin, or Project Pipelines product layer. Attach runtime behavior is unchanged.

## Files changed

Create:

- `docs/reports/gate-incremental-attach-local-runtime-implement.md`

Edit:

- `crates/botster-core/src/engine/botster.rs` — restore `#[cfg(feature = "local-runtime")]` on `suppress_attach_terminal_output`
- `.github/workflows/ci.yml` — add `cargo test -p botster-core --no-default-features --lib` after "Test workspace"

## Ownership boundaries preserved

- Core owns the `local-runtime` feature boundary, `engine/botster.rs`, and Core CI.
- Contract-only embeds stay off `local-runtime`. This run does not enable that feature in TUI or Hub.
- `IncrementalAttach`, attach phases, and `WorkerBackedBotsterEngine` behavior are unchanged.
- Unused `--no-default-features` imports at `botster.rs:15-19` were left in place, as the approved plan required.
- No test wrapper was added.

## Cross-repo dependencies or separately routed work

- botster-hub `ticket_1787251447_191212` (`tgt_7e208a0c76a44980a83b63af976b1f22`) owns the Hub pin roll after merge.
- botster-tui `ticket_1786663585_944018` (`tgt_c3d470bab78549df920a41e8fb0e58d8`) owns the TUI pin roll; it depends on the Hub ticket.
- Those consumers are not prerequisites. This run did not edit those repositories.
- Post-merge `--locked` TUI/Hub consumption remains owned by those tickets.

## Deviations from plan

None.

## Tests and downstream proof run

Red before the gate, on the plan commit:

```
BOTSTER_ENV=test cargo check -p botster-core --no-default-features --lib
```

Exit 101. `error[E0412]: cannot find type IncrementalAttach` at `crates/botster-core/src/engine/botster.rs:1767`.

Green after the gate:

```
cargo fmt --all -- --check
BOTSTER_ENV=test cargo clippy --workspace --all-targets -- -D warnings
BOTSTER_ENV=test cargo test --workspace
BOTSTER_ENV=test cargo test --doc --workspace
cargo doc --workspace --no-deps
BOTSTER_ENV=test cargo check -p botster-core --no-default-features --lib
BOTSTER_ENV=test cargo check -p botster-core --no-default-features --all-targets
BOTSTER_ENV=test cargo test -p botster-core --no-default-features --lib
cargo doc -p botster-core --no-default-features --no-deps
```

All exit 0. Contract-only lib tests: 13 passed. Workspace clippy, tests, and docs were green with no failed crates.

Production consumer path (pre-merge, patched; not `--locked`):

1. Scratch copy of botster-tui main `dc7d600`.
2. `[patch."https://github.com/trybotster/botster-core.git"]` path entries for `botster-core`, `botster-core-test-support`, `botster-terminal-ghostty`, `botster-terminal-protocol`, and `botster-terminal-protocol-client` pointing at this ticket worktree's `crates/<name>`.
3. `cargo build -p botster-tui` (no `--locked`): exit 0 in 53.73s.
4. `cargo tree -p botster-tui -e features,no-dev -i botster-core`: no `local-runtime` feature and no `portable-pty`.
5. Red oracle: the same TUI graph patched at an ungated Core `8fce204` copy of `engine/botster.rs` failed with exit 101 and `error[E0412]: cannot find type IncrementalAttach` at `botster.rs:1767`.

The production entry point is the TUI binary compile of `botster-core` with `default-features = false`. `WorkerBackedBotsterEngine` already called `suppress_attach_terminal_output` from the gated impl (plan: lines 1260 and 1450). The ticket restores compile of that consumer graph; it does not change attach runtime.

CI proof: the new verify step is the README contract-only command. This run executed that command locally and it passed. GitHub Actions has not yet run the workflow on this branch.

## Unverified behavior or residual risk

- GitHub Actions `verify` has not run on this branch. Local execution of the new step command passed.
- `--locked` TUI/Hub pin-roll builds remain on the consumer tickets.
- `--no-default-features` still emits the pre-existing unused-import and dead-code warnings named in the plan. Those are out of scope.
- Botster MCP inside Grok failed handshake (`BOTSTER_SESSION_UUID` was not expanded). Pipeline tools were called through `botster mcp-serve` with the inherited session UUID.

## Missing vault guidance discovered

- Capture now: Core CI omitted the README contract-only lane, so feature-unified workspace tests could not catch this class of break. Inbox: `botster-core CI runs the contract-only test lane because feature unification hides no-default-feature breaks`.
- After merge: update [[TUI bin only Core 8fce204 builds require local runtime feature unification]] with regression origin `968792d` and the fixing revision.
- Follow-up ticket candidate, not captured: `cargo clippy -p botster-core --no-default-features --lib -- -D warnings` is still red with the seven pre-existing warnings named in the plan.

Committed-artifact PII scan: this report uses path-neutral worktree wording and does not record home or session paths.
