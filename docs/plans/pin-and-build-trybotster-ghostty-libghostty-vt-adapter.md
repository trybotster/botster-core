# Pin And Build TryBotster Ghostty Libghostty-vt Adapter

## Context Loaded

- Pipeline context: ticket `ticket_1780289833_171227`, run `run_1780289849_170236`, returned Plan step `botster_plan`, gate `botster_plan_gate`.
- Review context: Plan Review returned `changes_required` with five open findings: scope overreach, license attribution mechanics, PII scan gap, raw cargo evidence, and unresolved source pin mechanism.
- Current pipeline state: no prior implementation artifacts; one non-blocking human scope question remains open. Re-plan proceeds with the smaller interpretation requested by review unless the human later confirms full runtime scope.
- Required playbooks: `planner-playbook` and `botster-planner-playbook`.
- Botster vault overlays: `botster-architecture`, `cli-patterns`, `spa-patterns`, `project pipeline orchestration belongs in a device-level botster plugin`, `project pipelines needs an operator workbench not more primitives`, `project pipelines ui contract belongs in the plugin readme`, `botster orchestration should spawn agents with explicit target ids`, and `botster orchestration prompts must bind agents to explicit worktrees`.
- Ghostty/build/review notes: `ghostty shadow terminal integration belongs outside botster core`, `libghostty-vt embedder callback architecture and constraints`, `botster cli integration tests require ghostty submodule initialization`, `botster build rs mise exec can override path zig selection`, `botster pipeline reviewers must bypass rtk summaries for cargo gate evidence`, and `botster review and verify must scan all committed artifacts for pii`.
- Repo context inspected: root workspace manifest, adapter crate manifest/source/tests, Ghostty adapter ADR, Ghostty shadow-terminal architecture note, README terminal-screen boundary text, and absence of an existing `.gitmodules` file.

## Botster Layer And Worktree Assumptions

- Botster layer touched: Rust adapter crate plus docs. No Lua plugin, hub, TUI, SPA, Rails relay, MCP, or Project Pipelines UI behavior should change.
- Assigned worktree: pipeline-provided ticket worktree.
- Spawn target from current run context: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.
- This plan assumes implementers stay in the assigned pipeline ticket worktree and do not edit any ambient checkout directly unless the pipeline workspace is explicitly re-bound by a human.

## Scope

- Add the Ghostty fork pin to the adapter crate as a crate-local `crates/botster-terminal-ghostty/vendor/ghostty` git submodule, because Ghostty precedent uses an exact checked-out fork and this ticket needs a durable fork commit pin.
- Add or update `.gitmodules` for that first workspace submodule and document the feature-enabled initialization step:
  - default workspace builds may leave the submodule uninitialized;
  - feature-enabled builds/tests require `git submodule update --init crates/botster-terminal-ghostty/vendor/ghostty`.
- Add feature-gated build support in `botster-terminal-ghostty` only:
  - a feature such as `libghostty-vt` that enables the native build path;
  - `build.rs` and crate-local build support for source discovery, Zig selection, and static link directives;
  - Zig `0.15.2` resolution using explicit candidates such as `BOTSTER_ZIG`, `ZIG`, mise-managed Zig, `zig` from `PATH`, and `mise exec -- zig`, with diagnostics that reveal the resolved Zig version without printing machine-specific user paths;
  - `zig build -Demit-lib-vt -Doptimize=ReleaseFast -Dsimd=false -Dcpu=baseline`;
  - platform link handling copied from the proven CLI path where applicable, including macOS archive/runtime handling if required.
- Make default workspace tests pass without Ghostty or Zig. When `libghostty-vt` is enabled and Ghostty/Zig is unavailable, the build must fail with a clear feature-gated error naming the missing precondition, not a raw missing `build.zig`, wrong Zig version, or linker failure.
- Add one minimal feature-gated FFI linkage smoke test as the anti-scaffold proof for this build ticket. It should link the static `libghostty-vt` artifact and exercise a tiny real symbol path such as terminal create/write/free if the C API permits that safely.
- Update adapter docs to identify the pinned Ghostty fork commit, why the fork is needed, required Zig version and flags, submodule initialization, default skip behavior, feature-enabled error behavior, platform link notes, no-restty boundary, and license obligations.
- Preserve the Ghostty fork's MIT license and copyright text in the vendored submodule and document that this vendored source is MIT-licensed while the workspace crate metadata still points at the workspace license.

## Non-Scope

- No implementation of the full five-method `TerminalScreenRuntime` adapter in this ticket.
- No snapshot export/import wrapper, formatter-backed `screen_state`, rich safe runtime wrapper, callback integration, render-state iteration, or production session-runtime wiring in this ticket.
- No Ghostty, Zig, build script, native parser, or restty dependency in `botster-core`.
- No migration of production CLI/session runtime to the adapter.
- No generated broad binding surface.
- No browser rendering, restty/WASM work, TUI changes, SPA changes, Rails relay work, Lua plugin work, Project Pipelines UI work, or adjacent cleanup.

## Assumptions And Unknowns

- Assumption: the accepted ADR's recorded trybotster Ghostty fork commit is the intended pin unless implementation discovers a newer accepted commit in repo-local documentation.
- Assumption: this ticket is build/pin/link plumbing plus minimal linkage proof; the full `TerminalScreenRuntime` runtime adapter is a follow-up ticket unless the human scope question explicitly says otherwise.
- Assumption: a crate-local submodule is acceptable even though this is the workspace's first `.gitmodules` entry, because it records an exact fork commit and matches the historical Ghostty integration precedent.
- Assumption: the adapter crate may own `build.rs` because the ticket explicitly assigns Ghostty/Zig build policy to that crate.
- Unknown: exact Linux/macOS static link directives in this smaller workspace. Implementer should copy the proven CLI behavior and record any divergence.
- Unknown: whether CI has Zig `0.15.2`. Default CI must not require it; feature-enabled CI must document or install that precondition.

## Affected Surfaces And Files

- `.gitmodules`: new crate-local Ghostty submodule entry.
- `crates/botster-terminal-ghostty/vendor/ghostty`: pinned trybotster Ghostty fork submodule; its MIT license/copyright text must remain present.
- `crates/botster-terminal-ghostty/Cargo.toml`: feature flag, build dependency metadata if needed, docs/package metadata if needed.
- `crates/botster-terminal-ghostty/build.rs`: feature-gated native build entrypoint.
- `crates/botster-terminal-ghostty/src/build_support.rs` or similar: Zig/tool/source discovery kept crate-local.
- `crates/botster-terminal-ghostty/src/sys.rs` or similar: minimal handwritten FFI declarations for the linkage smoke test only.
- `crates/botster-terminal-ghostty/src/lib.rs`: public docs/exports only as needed for feature-gated linkage proof.
- `crates/botster-terminal-ghostty/tests/*`: default no-Ghostty behavior and feature-gated native linkage smoke test.
- `crates/botster-terminal-ghostty/README.md` or repo README/docs section: fork commit, submodule init, flags, platform notes, no-restty boundary, and license attribution.
- `docs/architecture/ghostty-shadow-terminal-adapter.md`: update only if implementation evidence changes the accepted ADR details.

## Risks

- A default-on build script would make normal workspace tests depend on Ghostty/Zig and violate the ticket.
- Adding Ghostty/Zig to `botster-core` would violate the accepted boundary and vault notes.
- Expanding from linkage smoke test into the full runtime adapter would repeat the reviewed scope error.
- Wrong Zig version or `mise exec` override can select a compiler other than Zig `0.15.2`.
- Omitting `-Dcpu=baseline` can create release artifacts that fail on older hardware.
- Static linking and archive layout are platform-sensitive, especially on macOS.
- Vague build errors would fail the locate-or-skip acceptance criterion.
- The workspace license is not the Ghostty fork license; vendored MIT license/copyright text must be preserved and not misrepresented.
- Build support, docs, or errors can leak machine-specific user paths; committed artifacts need PII scanning.

## Acceptance Checks And Tests

- Raw evidence command: `rtk proxy -- cargo test -p botster-terminal-ghostty` exits 0 without initialized Ghostty or Zig.
- Raw evidence command: `rtk proxy -- cargo test --workspace` exits 0 without requiring Ghostty or Zig.
- Raw evidence command: `rtk proxy -- cargo test -p botster-terminal-ghostty --features libghostty-vt` with initialized Ghostty source and Zig `0.15.2` builds, links static `libghostty-vt`, and runs the minimal FFI linkage smoke test.
- Missing-precondition evidence: with the feature enabled but Ghostty source or Zig `0.15.2` unavailable, raw cargo output captures the exact clear error string naming the missing precondition.
- Raw evidence command: `rtk proxy -- cargo clippy --workspace --all-targets --all-features -- -D warnings` exits 0 when feature preconditions are available; if they are unavailable, evidence must show the exact feature-precondition failure rather than summarized prose.
- Source boundary check: `rg -n "ghostty|libghostty|zig|build\\.rs|restty" crates/botster-core` shows no new Ghostty/Zig/restty/build-script dependency in `botster-core` beyond existing documentation/test guard wording.
- Dependency check: `cargo tree -p botster-core -e=no-dev --offline` does not include Ghostty, Zig, or restty.
- Default dependency direction check: `botster-terminal-ghostty` continues to depend on `botster-core` with `default-features = false` unless a reviewed reason changes it.
- License check: vendored Ghostty submodule retains its MIT `LICENSE`/copyright attribution, and adapter docs/metadata do not imply the vendored Ghostty source is covered only by the workspace license.
- PII check: `rg -n '/[U]sers/|/[h]ome/[^/[:space:]]+|[j]asonconigliari' docs crates .gitmodules` returns no matches in committed artifacts introduced or touched by this ticket.
- No-restty check: no `restty` dependency is added anywhere for this ticket.

## Pipeline Gates And Artifacts

- Plan gate evidence should point to this file and the loaded review context above.
- Checklist evidence should record vault notes read, no convention conflict after re-plan, raw verification evidence, and capture decision.
- Implement and verify gates should attach raw cargo/git output, not RTK summaries.
- The actual changed runtime/user path for this ticket is the adapter crate's feature-enabled native build path: enabling `libghostty-vt` locates initialized Ghostty source, selects Zig `0.15.2`, builds and statically links `libghostty-vt`, and proves a real linked C symbol through the smoke test. The full terminal runtime path remains intentionally deferred.

## Vault Gaps Worth Capturing

- Capture a durable note after implementation if the adapter crate lands a settled pattern for first-submodule Ghostty fork pinning in the extracted botster-core workspace.
- Capture a durable note if the proved platform link behavior differs from the older CLI path.
- Capture a durable note if the current fork commit or C API differs from the accepted ADR's documented build/link expectations.
