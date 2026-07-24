# Isolate Ghostty Zig Caches Across Cargo Target Directories

## Target And Context Loaded

- Ticket: `Core: isolate Ghostty Zig caches across Cargo target directories`.
- Target repository: `trybotster/botster-core`.
- Target ID: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.
- Assigned branch/worktree: the Project Pipelines ticket worktree for this run.
- Repository charter: [[botster-core-playbook]].
- Role and surface playbooks: [[planner-playbook]], [[botster-planner-playbook]], [[botster-terminal-ghostty-playbook]], and [[project-pipelines-playbook]].
- Architecture and task notes: [[botster-architecture]], [[cli-patterns]], [[ghostty shadow terminal integration belongs outside botster core]], [[session-process-owns-vt-parser-hub-rpc-snapshots]], [[libghostty-vt-embedder-callback-architecture-and-constraints]], [[pinned libghostty exposes synchronous exact mouse mode state]], [[botster cli integration tests require ghostty submodule initialization]], [[botster build rs mise exec can override path zig selection]], [[vendored ghostty non-semver tags block rust builds at exact tag commits]], [[botster pipeline reviewers must bypass rtk summaries for cargo gate evidence]], [[prefer framework and library components over custom solutions]], [[plan agents must author vault context as wikilinks not home paths]], and [[vault example paths are not repository placement conventions]].
- Project Pipelines workflow notes: [[project pipeline orchestration belongs in a device-level botster plugin]], [[project pipelines needs an operator workbench not more primitives]], [[project pipelines ui contract belongs in the plugin readme]], [[botster orchestration should spawn agents with explicit target ids]], and [[botster orchestration prompts must bind agents to explicit worktrees]].
- Personal context: [[identity]] and [[goals]].
- Repository evidence inspected: root and adapter READMEs, workspace and adapter manifests, `build.rs`, `build_support.rs`, default daemon feature wiring, CI, current hub lockfile consumer, Ghostty adapter ADR, historical plan placement guidance, and prior adapter plans.

## Problem And Production Path

`botster-terminal-ghostty/build.rs` already defaults Zig's global cache to
`OUT_DIR/zig-global-cache`, so it changes with `CARGO_TARGET_DIR`. The command
still runs from the shared vendored Ghostty checkout without a local-cache
override, which makes Zig use `vendor/ghostty/.zig-cache` as its local cache.
That local cache can refer to generated executables in the first target
directory's global cache. A later build of the same source revision into a
fresh Cargo target directory then reuses the incompatible local state and
fails while spawning a missing generated executable.

The changed production path is the existing default
`botster-core-daemon -> botster-terminal-ghostty/libghostty-vt -> build.rs ->
zig build` path. This is build correctness, not scaffold-only work.

## Plan Review Reconciliation

- Negative control: acceptance now requires raw failure evidence from the exact
  two-target sequence on unmodified `16bf08f`, followed by raw success evidence
  from the fixed revision. The CI invariant must also fail on the baseline.
- Cache design: both defaults remain per-`OUT_DIR`; the plan records the
  hermetic-correctness rationale, rejected stable-machine-global alternative,
  cold-build tradeoff, and timing evidence.
- Mechanism: use unconditional `ZIG_LOCAL_CACHE_DIR`, leave the existing global
  environment and rerun behavior unchanged, and prove ambient-local precedence.
- Shared archive: remove only the installed archive as controlled test setup
  before the second build and require its regeneration; `--prefix` remains an
  explicit deferral.
- Helper test: include the build-script-only module by path with a targeted
  dead-code allowance, then run the repository's strict all-target Clippy gate.
- Native prerequisites: initialize and verify the currently missing submodule,
  and record the available mise-managed Zig `0.15.2`, before cache evidence.

## Scope

- Derive a Zig local cache directory from the same Cargo `OUT_DIR` that already
  scopes the default global cache.
- Set `ZIG_LOCAL_CACHE_DIR` unconditionally on the child Zig command so the
  shared vendored checkout is no longer the local cache owner. Zig 0.15.2 also
  supports `--cache-dir`, but the environment variable is the smaller,
  symmetric change beside the existing `ZIG_GLOBAL_CACHE_DIR` child
  environment.
- Override any ambient `ZIG_LOCAL_CACHE_DIR` rather than treating it as caller
  configuration. Local isolation is a build invariant, not optional
  configurability.
- Keep the existing `ZIG_GLOBAL_CACHE_DIR` opt-in behavior: an explicitly
  configured global cache remains caller-owned, while the default remains
  target-specific under `OUT_DIR`.
- Leave the working global-cache environment mechanism and existing
  `cargo:rerun-if-env-changed=ZIG_GLOBAL_CACHE_DIR` directive unchanged. Do not
  convert it to `--global-cache-dir`, and do not add a rerun directive for the
  ambient local-cache variable because the build intentionally ignores and
  overrides that value.
- Add a focused test for default and configured cache-path resolution.
- Add a cheap Linux CI invariant after the existing feature-enabled workspace
  build: fail if `vendor/ghostty/.zig-cache` exists. The current unfixed build
  creates that path, so this guards the root regression without adding a
  second cold ReleaseFast Ghostty build to every pull request.
- Update the adapter README with the target-scoped cache contract and the
  retained explicit global-cache override.

### Cache Contract Decision

Keep both default caches below Cargo's `OUT_DIR`: local at
`OUT_DIR/zig-local-cache` and global at `OUT_DIR/zig-global-cache`. This
preserves the existing global-cache isolation and makes the pair coherent for
each Cargo build-script instance. It deliberately rejects changing the default
global cache to Zig's stable machine-level cache: that would be a broader
cross-worktree sharing policy change, while the ticket asks to repair the
accidentally shared local cache.

The tradeoff is that distinct Cargo target directories do not reuse either Zig
cache and therefore perform cold native work. That is accepted for hermetic
correctness: the existing global cache was already cold per target, and
checkout-local reuse is the broken behavior being removed. The plan does not
add a second full build to pull-request CI; implementation evidence must record
the wall time of the required two-target control/proof so the cost is visible.

## Non-Scope

- No deletion or cleanup of existing Cargo, Zig, or vendored-checkout caches.
- No requirement for hub or other consumers to set cache environment variables,
  delete shared caches, patch dependencies, or override dependency sources.
- No change to Zig selection, the required Zig version, Ghostty pin, build
  flags, archive repacking, install output, FFI, runtime behavior, snapshots,
  terminal contracts, or daemon feature selection.
- No `--prefix`/install-directory isolation in this ticket. The shared
  `vendor/ghostty/zig-out` output is considered explicitly, but the reported
  defect is the mismatched cache pair. Acceptance must prove the second build
  regenerates the archive rather than false-passing on that shared output.
- No new cache manager, service object, dependency, configuration layer, or
  generalized build abstraction.
- No changes in `botster-core`, hub source, clients, TUI, web, plugins, Rails,
  MCP, or Project Pipelines product behavior.
- No rewrite of historical plans or unrelated documentation.

## Ownership Boundaries And Cross-Repository Dependencies

- The concrete cache fix belongs to `botster-terminal-ghostty`, which owns
  vendored Ghostty and Zig build integration inside the `botster-core`
  repository.
- Backend-neutral terminal contracts remain untouched in `botster-core`.
- `botster-core-daemon` remains the first-party production consumer and needs
  no code change; its default `ghostty-terminal` feature is the in-repository
  production wiring proof.
- `botster-hub` is a downstream validation consumer only. This Core run must
  not edit the Hub repository or its lockfile. After the Core commit is
  available through the normal dependency update, Hub should repeat the
  ticket's two-target build as downstream proof without dependency overrides.
- There is no cross-repository implementation prerequisite and therefore no
  dependency ticket to register. A Hub lockfile update is a post-merge
  integration validation, not a prerequisite for implementing the Core fix.

## Assumptions And Unknowns

- Assumption: Cargo's `OUT_DIR` is the correct scope because it is already
  unique to the selected `CARGO_TARGET_DIR`, profile, target, package, feature
  set, and build-script fingerprint.
- Assumption: caller-supplied `ZIG_GLOBAL_CACHE_DIR` remains supported because
  it is an intentional override already exposed by the build script; local
  cache isolation must not depend on that override.
- Assumption: Zig 0.15.2 honors `ZIG_LOCAL_CACHE_DIR` as the local-cache
  counterpart to the already used `ZIG_GLOBAL_CACHE_DIR`; setting it through
  `Command::env` takes precedence over an ambient value inherited by the build
  script.
- Assumption: the archive remains installed at `vendor/ghostty/zig-out`; the
  ticket targets cache coherence, not install-prefix isolation.
- Current precondition state: the ticket worktree's Ghostty submodule is
  uninitialized (leading `-` in `git submodule status`). The mise-managed Zig
  binary exists and reports `0.15.2`, but implementation must initialize the
  submodule and record both prerequisite checks before native evidence.
- Unknown to verify during implementation: whether the clean CI checkout has
  any Ghostty-generated checkout-local cache path besides `.zig-cache`.
- No ticket ambiguity or convention conflict requires a human question. If
  implementation shows that isolating only the caches cannot prevent the
  reproduced failure, stop and ask before broadening into output-prefix or
  Ghostty-source changes.

## Affected Surfaces And Files

- `crates/botster-terminal-ghostty/build_support.rs`
  - add the minimal local-cache path helper or paired cache-path calculation;
  - preserve explicit global-cache override behavior.
- `crates/botster-terminal-ghostty/build.rs`
  - calculate the local cache path from `OUT_DIR`;
  - set `ZIG_LOCAL_CACHE_DIR` unconditionally beside the existing global-cache
    environment on the production build command.
- `crates/botster-terminal-ghostty/tests/build_support_test.rs` (new)
  - include the build-script-only helper with
    `#[path = "../build_support.rs"]`;
  - put a targeted `#[allow(dead_code)]` on that included module because the
    test binary does not use unrelated Zig-selection helpers;
  - prove default local/global paths share the `OUT_DIR` scope and explicit
    global override does not change local isolation.
- `crates/botster-terminal-ghostty/README.md`
  - document the cache contract without machine-specific paths.
- `.github/workflows/ci.yml`
  - after an existing feature-enabled workspace command, assert that the build
    did not create `vendor/ghostty/.zig-cache`; do not add a second full native
    build to every pull request.

No other file should change unless the implementer records why it is necessary
for the ticket or for cleanup caused directly by the change.

## Risks

- Wiring the local cache only in a helper, or failing to set it on every
  production Zig candidate, would leave the defect intact.
- Respecting an ambient local-cache override would reintroduce shared local
  state; the build must set the local cache explicitly.
- Dropping the existing global-cache override would be an unrelated compatibility
  regression.
- A helper-only test could pass while the production command remains unwired;
  the two-target build must exercise `build.rs`.
- A single clean-target build would miss the regression because the failure
  needs local state from a prior target directory.
- Distinct target directories lose checkout-local Zig cache reuse and perform
  cold native work. This is the explicit correctness-over-reuse tradeoff, not
  an accidental consequence.
- Concurrent builds may still share `zig-out`; install-prefix isolation is
  explicitly deferred unless evidence proves it is required to fix the cache
  failure.
- Native Ghostty builds require the initialized submodule and exact Zig 0.15.2;
  failures from those prerequisites must not be misclassified as cache failures.
- RTK summaries can hide Cargo diagnostics, so Review and Verify need raw
  command output and exit status.

## Acceptance Checks And Tests

Use the repository's actual CI commands and raw Cargo output:

1. `cargo fmt --all -- --check`.
2. `rtk proxy -- cargo clippy --workspace --all-targets -- -D warnings`.
3. `rtk proxy -- cargo clippy -p botster-core-daemon --no-default-features --all-targets -- -D warnings`.
4. `rtk proxy -- cargo test --workspace`.
5. `rtk proxy -- cargo test -p botster-core-daemon --no-default-features`.
6. `rtk proxy -- cargo test --doc --workspace`.
7. `RUSTDOCFLAGS="-D warnings" rtk proxy -- cargo doc --workspace --no-deps`.
8. Record native preconditions before interpreting any build result:
   `git submodule update --init crates/botster-terminal-ghostty/vendor/ghostty`,
   `git submodule status crates/botster-terminal-ghostty/vendor/ghostty`
   without a leading `-`, and the build helper's selected Zig reporting
   `0.15.2`.
9. Negative control on unmodified Core `16bf08f`: from a controlled clean
   checkout/cache setup, run the exact two-target sequence that the proposed
   regression evidence will use. Build target A to populate
   `vendor/ghostty/.zig-cache`; before target B, remove only the shared
   `vendor/ghostty/zig-out/lib/libghostty-vt.a` test artifact so target B cannot
   pass by linking target A's archive. Record raw target-B exit 101 and the
   `uucode_build_tables: FileNotFound` diagnostic. If the clean target-A build
   itself fails with that exact diagnostic, record the earlier failure
   honestly instead of claiming an A-success/B-failure sequence, and pair it
   with the baseline-red/fixed-green CI invariant from check 14. This cleanup
   is test setup, never a caller workaround.
10. Positive control on the fixed revision: repeat the same two-target
    sequence with fresh target directories while leaving the populated
    checkout-local `.zig-cache` from the negative control in place. Record a
    before hash plus inode/mtime, do not delete that cache, and require the
    after values to be unchanged. Before B, remove only the shared installed
    archive, then require B to exit 0 and require a new
    `vendor/ghostty/zig-out/lib/libghostty-vt.a` to exist. Record elapsed time
    for both fixed builds. If the baseline sequence does not fail, do not claim
    the two-target sequence as a regression proof; retain CI only as the
    no-checkout-cache invariant and develop a deterministic control before
    passing Implement.
11. Assert that each fixed build created
    `zig-local-cache` and default `zig-global-cache` below its own Cargo
    `OUT_DIR`, and did not modify the pre-existing checkout-local cache. Also
    run one fixed build after moving that polluted cache aside and assert that
    `crates/botster-terminal-ghostty/vendor/ghostty/.zig-cache` was not created.
12. Repeat a fixed build with an ambient `ZIG_LOCAL_CACHE_DIR` pointing at a
    sentinel directory and an explicit `ZIG_GLOBAL_CACHE_DIR`. The sentinel
    local directory must remain unused, the local cache must remain below
    `OUT_DIR`, and the global cache may use the configured location.
13. Inspect the native build output or process evidence to prove production
    `build.rs` set `ZIG_LOCAL_CACHE_DIR`; helper source existence alone is
    insufficient.
14. Confirm the normal CI feature-enabled workspace build followed by
    `test ! -e crates/botster-terminal-ghostty/vendor/ghostty/.zig-cache`
    fails on unmodified `16bf08f` and passes on the fixed revision. This is the
    durable PR guard; it checks the invariant rather than rerunning the full
    two-target reproduction.
15. Downstream proof after the normal Core dependency update: in
    `botster-hub`, with its lockfile pointing at the fixed Core revision, run
    the ticket's `cargo build --locked -p botster-terminal-ghostty` sequence
    against two new `CARGO_TARGET_DIR` values. Both builds must exit 0 without
    deleting Cargo's shared Git checkout cache or overriding dependencies.
16. Review the diff for no new dependencies, no Core contract changes, no
    Ghostty source edits, no cache-deletion workaround, and no machine-specific
    paths in committed artifacts.

## Pipeline Evidence And Vault Gaps

- Project Pipelines checklists record the target routing, vault notes loaded,
  convention fit, repository inspection, planned verification, and capture
  decision.
- The Plan gate should attach this file and the structured scope, ownership,
  risk, and acceptance evidence.
- Convention conflicts: none. The plan keeps concrete Ghostty build policy in
  its owning crate, uses Zig/Cargo primitives, and avoids speculative
  abstractions or caller workarounds.
- Durable vault gap: the reproduced rule that Zig's checkout-local cache can
  retain generated executable references into a target-scoped global cache is
  not currently captured as an atomic gotcha. Capture it after implementation
  confirms the exact fix and two-target evidence. Do not capture the plan's
  hypothesis as shipped behavior before verification.
