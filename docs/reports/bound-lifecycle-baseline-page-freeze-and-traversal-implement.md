# Implementation report: bound lifecycle_baseline_page freeze and traversal

Ticket: `ticket_1786733177_803101`
Run: `run_1786733190_220788`
Step: `botster_stack_implement`
Plan: `docs/archive/plans/bound-lifecycle-baseline-page-freeze-and-traversal.md`
Approved plan artifact: `artifact_1786745506_578393`

## Target repository and target_id

- Target repository: `botster-core`
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Spawn-target name: `botster-core` (`trybotster/botster-core`)
- Independent context resolution matches the approved plan
- Worktree: the pipeline-provided ticket worktree, rebased onto
  `f2f3ce2c1a9a3fe266373b69695d737b2b259d9e`
- Merge policy: `direct` (no PR)
- Runtime-teardown class: yes

## Repository playbook and other playbooks/notes applied

Repository charter: [[botster-core-playbook]]

Role and overlay:

- [[implementer-playbook]]
- [[botster-implementer-playbook]]
- [[botster runtime teardown lenses]]
- [[project-pipelines-playbook]]
- [[botster-core uses CI-owned Cargo commands because it has no test script]]
- [[implement gate must verify committed work and pr link before review]]
- [[implementation artifacts must match actual git state]]
- [[implementation steps must persist report artifacts for review]]
- [[pipeline vault checklists must cite exact resolvable note titles]]

Targeted notes:

- [[Core control-plane lifecycle journal advances without a terminal client or Hub terminal Drain]]
- [[core daemon lifecycle metadata is registry backed restart state]]
- [[botster core contract surface needs consumer proof]]
- [[public dto field additions are source breaking without non exhaustive]]
- [[plugin worker unload deadline can flake under default-concurrency workspace load]]

Checklists: `checklist_1786745907_996770`, `checklist_1786747666_979153`

## Review repair

`review_1786747612_446998` sent Implement back for
`finding_1786747612_443951`. Suffix emission now checks elapsed after
each counted load or clone and before the next encode. It no longer
clones the accumulated page. First-page and later-page unit tests
assert item, encoded-byte, and positive elapsed limits, including
`baseline_page_encodes` and an elapsed-before-encode negative control.

## Botster layers changed

- `botster-core-daemon` public `lifecycle_baseline_page` budget, incremental
  freeze, membership fence, and skip-malformed single-record load
- `botster-core-test-support` isolated Hub-shaped consumer retries setup-only
  and index-in-progress yields
- Living architecture and README host-loop paragraphs

No Hub, Web, TUI, Ghostty crate, plugin-admission, or Project Pipelines
product layer.

## Files changed

Create:

- `docs/archive/plans/bound-lifecycle-baseline-page-freeze-and-traversal.md`
- `docs/reports/bound-lifecycle-baseline-page-freeze-and-traversal-implement.md`

Edit:

- `crates/botster-core-daemon/src/api.rs`
- `crates/botster-core-daemon/src/lib.rs`
- `crates/botster-core-daemon/src/daemon.rs`
- `crates/botster-core-daemon/src/registry.rs`
- `crates/botster-core-daemon/tests/daemon_integration_test.rs`
- `crates/botster-core-test-support/tests/consumers/hub-lifecycle-shaped/src/lib.rs`
- `crates/botster-core-test-support/tests/lifecycle_journal_consumer_test.rs`
- `docs/architecture/control-plane-lifecycle-journal.md`
- `docs/architecture/core-daemon.md`
- `README.md`

## Ownership boundaries preserved

- Core still owns lifecycle facts, freeze identity, and the page budget.
- Hub still owns projection and owner-turn scheduling.
- Terminal bodies stay off this control-plane surface.
- `observe_lifecycle_slice` remains the progress tick.
- `lifecycle_baseline()` remains the unbounded compatibility wrapper.

## Cross-repo dependencies or separately routed work

- Hub ticket `ticket_1786663582_169720` still depends on this surface.
  This run does not implement Hub projection.
- Closed Core blocker `ticket_1786735252_213191` is merged at `f2f3ce2`.
  This run does not change attach-stream code.

## Deviations from plan

- None for product scope. The membership fence also snapshots an
  unmaterialized `membership` entry (`None` row) before save or remove.
  That keeps copy-on-write correct after the index lists an id and
  observe later writes that file.
- Suffix emission now matches the planned per-step elapsed checks.
  Review `finding_1786747612_443951` required this; it is not a new
  product deviation.
- Command set follows the ticket and current CI, not the vault note
  prefix of `BOTSTER_ENV=test` on Clippy and doctest.

## Runtime-teardown lenses implemented

- Isolation: one freeze is one `snapshot_sequence`. A failed mint or
  page returns typed resync and does not stop sibling sessions.
- Bounds: one call is one host tick. Item, encoded-byte, and elapsed
  limits stop directory examination, row copy, and page encode.
- Late-message matrix: mint, later page, foreign source, observe
  between pages, mid-freeze spawn, mid-freeze remove or mutate, observe
  slice, and other write sites keep the planned reject and sweep rules.
- Production-path proof: `CoreDaemon::lifecycle_baseline_page` plus the
  Hub-shaped `install_baseline` consumer.
- Ownership identity: freeze identity is the mint cursor. Setup-only
  and index-in-progress yields keep that identity with `complete = false`.
- Sibling fail-closed: success leaves siblings running. Ultimate page
  failure returns resync or `BudgetTooSmall`.

## Tests and downstream proof run

Focused:

- `cargo test -p botster-core-daemon --lib baseline_freeze_bound_tests`
- `cargo test -p botster-core-daemon --test daemon_integration_test -- lifecycle_baseline`
- `cargo test -p botster-core-daemon --test daemon_integration_test -- lifecycle_api_types_are_control_plane_only observe_slice registry_load_all`
- `cargo test -p botster-core-test-support --test lifecycle_journal_consumer_test`

Workspace gates. Exact ticket commands:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --doc --workspace
BOTSTER_ENV=test cargo test --workspace
```

All four passed on this worktree after the lifecycle-baseline change:

- `cargo fmt --all -- --check` — pass
- `cargo clippy --workspace --all-targets -- -D warnings` — pass
- `cargo test --doc --workspace` — pass
- `BOTSTER_ENV=test cargo test --workspace` — pass under default
  concurrency, including
  `worker_incremental_attach_streams_ready_pages_finish_then_queued_work_and_live_output`
  and the Hub-shaped consumer.

Production entry: `CoreDaemon::lifecycle_baseline_page`. The isolated
Hub-shaped `install_baseline` is the downstream-shaped caller.

## Unverified behavior or residual risk

- Copy-on-write depends on fencing every `registry.save` and
  `registry.remove` while a freeze is open. Current write sites are
  spawn, persist size, mark stale, adopt, shutdown, reconcile, and
  remove. A later new write site can break freeze identity.
- Holding `ReadDir` across owner turns compiled here. A future
  `Send` requirement on `CoreDaemon` can reject that field.
- Default-concurrency workspace load can still hit the documented
  plugin-worker flake.

## Missing vault guidance discovered

- Incremental freeze plus copy-on-write is a new convention. Capture
  path: `~/knowledge/inbox/lifecycle-baseline-page-freeze-membership-is-fenced-by-excluded-ids-and-copy-on-write.md`
- [[botster-core uses CI-owned Cargo commands because it has no test script]]
  still disagrees with current CI and this ticket on `BOTSTER_ENV=test`
  for Clippy and doctest. This run did not edit that vault note.
