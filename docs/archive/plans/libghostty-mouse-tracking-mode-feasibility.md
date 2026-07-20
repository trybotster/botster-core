# Libghostty Mouse-Tracking Mode Feasibility

Ticket: `ticket_1784564069_340152`

Run: `run_1784564384_511975`

Status: plan for a research spike; no downstream mode-flags stack is in scope

## Context Loaded

- Pipeline context: returned Plan step `botster_plan`, run step
  `run_step_1784565121_373334`, required gate `botster_plan_gate`, and next step
  `botster_plan_review`.
- Pipeline history: the initial plan had no inherited state. Plan Review
  `review_1784565087_375675` returned four findings: cite the external
  trybotster bitmask precedent, make native execution mandatory, keep the proof
  helper out of the public API, and reconcile the downstream kit's current
  bool/prop behavior. This revision addresses all four without changing the
  source-supported feasibility classification.
- Required role context: [[planner-playbook]] and
  [[botster-planner-playbook]].
- Botster overlays: [[botster-architecture]], [[cli-patterns]],
  [[spa-patterns]],
  [[project pipeline orchestration belongs in a device-level botster plugin]],
  [[project pipelines needs an operator workbench not more primitives]],
  [[project pipelines ui contract belongs in the plugin readme]],
  [[botster orchestration should spawn agents with explicit target ids]],
  [[botster orchestration prompts must bind agents to explicit worktrees]],
  [[botster pipeline needs continuous product owner between agent steps]], and
  [[plan agents must author vault context as wikilinks not home paths]].
- Ticket-specific vault constraints:
  [[ghostty shadow terminal integration belongs outside botster core]],
  [[terminal view prop contract is closed in botster core]], and
  [[synced state types are allowed while pushed event variants are forbidden]].
- Repository revision inspected:
  `84c2ff20f3607ff24fb87d196e132c54365c31c5`. The ticket's earlier evidence at
  `978c436865c215828b02a8b0fcca5f8d89413e96` was rechecked against the current
  worktree.
- Pinned Ghostty source inspected:
  `crates/botster-terminal-ghostty/vendor/ghostty` at
  `76853b34274208fe7c051cfe13eb1c7ee63c469b`
  (`botster-vt-2026.03.31.2`).
- Prior production encoding inspected in the `trybotster` repository at
  `70c002f397007c8b0f3ebfe6b33a503dc7a283f6`.
- Downstream kit state inspected in the `botster-tui-kit` repository at
  `bc066e2581b01fb9e5271794c9a67ba1ace36e42`.
- Worktree and target assumption: all work stays in the pipeline-assigned
  ticket worktree for target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.

## Feasibility Classification

Classification **(a): feasible with no Ghostty-repository change**.

The pinned libghostty C surface already exposes both forms allowed by the
ticket:

- A plain on/off read:
  `GHOSTTY_TERMINAL_DATA_MOUSE_TRACKING` reports whether X10, normal,
  button-event, or any-event tracking is active
  (`vendor/ghostty/include/ghostty/vt/terminal.h:768-776`). Its implementation
  reads the authoritative terminal mode state
  (`vendor/ghostty/src/terminal/c/terminal.zig:692-695`).
- Exact per-mode reads:
  `ghostty_terminal_mode_get(GhosttyTerminal, GhosttyMode, bool *)`
  (`vendor/ghostty/include/ghostty/vt/terminal.h:1033-1049`) resolves the packed
  mode and reads `t.modes`
  (`vendor/ghostty/src/terminal/c/terminal.zig:568-577`).
- The public mode constants include DEC modes 1000, 1002, 1003, and 1006
  (`vendor/ghostty/include/ghostty/vt/modes.h:74-80`), and those exact modes are
  registered in Ghostty's mode table
  (`vendor/ghostty/src/terminal/modes.zig:274-280`).
- VT parsing updates both the durable mode state and Ghostty's derived mouse
  event/format state
  (`vendor/ghostty/src/terminal/stream_terminal.zig:579-611`).
- The symbol is exported by libghostty-vt
  (`vendor/ghostty/src/terminal/c/main.zig:132-143`).

The gap is only in Botster's narrower handwritten binding:
`crates/botster-terminal-ghostty/src/sys.rs:75-133` declares terminal
construction, writes, snapshots, formatters, and freeing, but not
`ghostty_terminal_mode_get` or `ghostty_terminal_get`. The safe
`GhosttyTerminal::screen_state` consequently builds
`TerminalScreenState::new`, which installs default flags
(`crates/botster-terminal-ghostty/src/lib.rs:390-396` and
`crates/botster-core/src/contract/terminal_screen.rs:140-151`).

The established Botster encoding must be preserved:

- DECSET 1000 normal tracking: bit `1`
- DECSET 1003 any-event tracking: bit `2`
- DECSET 1002 button-event tracking: bit `4`
- DECSET 1006 SGR encoding: bit `8`

That mapping is concrete production precedent in
`trybotster@70c002f397007c8b0f3ebfe6b33a503dc7a283f6`:

- `cli/src/ghostty_vt.rs:663-668` declares the same mode-query FFI.
- `cli/src/ghostty_vt.rs:939-944` implements the primitive mode read.
- `cli/src/ghostty_vt.rs:992-1007` defines the exact `1/2/4/8` composition.
- `cli/src/session/protocol.rs:153-163` documents the bit assignments on
  `ModeFlags.mouse_mode`.
- `cli/src/clients/tui/terminal_panel.rs:123-131` requires SGR bit `8` plus at
  least one tracking bit `1 | 2 | 4` before SGR passthrough.

The extracted core contract lost this documentation but retained the same
`ModeFlags.mouse_mode: u8` carrier. The spike must restore and prove the
existing encoding, not define a new numeric-ascending alternative. In
particular, 1006 alone means an encoding is selected, not that tracking is
active.

## Scope

This spike should make the smallest current-repository proof and record the
finding:

1. Extend the feature-gated handwritten sys layer with the existing
   `GhosttyMode = u16` ABI, the four packed DEC mode constants, and
   `ghostty_terminal_mode_get`. No generated bindings or new dependency is
   needed.
2. Add a private, `#[cfg(test)]` fallible helper inside the feature-gated native
   module. It queries the four existing Ghostty modes and composes the existing
   `u8` bitmask while preserving FFI failure. Do not add a public or
   `pub(crate)` adapter method in this spike.
3. Add a feature-gated native unit test beside that private helper. Feed real
   DECSET/DECRST bytes through `GhosttyTerminal::write_output`, then prove `0`,
   each established bit, the combined `1000 + 1006` value, and reset behavior.
   This is the runtime-path proof: PTY-shaped bytes enter the same libghostty
   terminal handle used by the production adapter, and the read comes back from
   that handle's authoritative parsed state.
4. Add a durable architecture finding under `docs/architecture/` that records
   classification (a), the pinned-revision file:line evidence, the Botster
   adapter delta, and the downstream ticket DAG below.

Botster layer touched by this spike: Rust Ghostty terminal adapter and docs
only.

### Proposed downstream reporting shape

The next core ticket should add one required backend-neutral method:

```rust
fn mode_flags(&self) -> ModeFlags;
```

Add it to `TerminalScreenRuntime` and its `Box<dyn TerminalScreenRuntime>`
forwarder without a silent default implementation. Update the plain backend and
all fakes explicitly; the plain backend may honestly return `ModeFlags::default`
because it has no emulator state. The Ghostty backend should populate the
existing `ModeFlags`, including the existing `mouse_mode: u8`; do not introduce
another mouse enum, bool, wire field, or versioned flags type.

`ManagedSessionRuntime` should answer the existing `GetModeFlags` request by
calling this method and returning the existing `ModeFlagsReady`. This direct
probe avoids formatting the screen merely to read modes. `screen_state()` may
also carry the same values, but it should not be the only mode probe.

### Recommended next-ticket DAG

1. **Core + terminal adapter producer:** add the required
   `TerminalScreenRuntime::mode_flags`, populate it from Ghostty, forward boxed
   runtimes, make plain/fake behavior explicit, and make
   `ManagedSessionRuntime` serve the existing `GetModeFlags` /
   `ModeFlagsReady` path with real state.
2. **Hub + hub-client probe:** expose a correlated request/response facade over
   the existing SessionIo probe. Do not add `ModeChanged` or another pushed
   terminal-mode event.
3. **Kit client-owned shadow:** replace, rather than parallel, the current
   renderer-prop source with typed attachment state. At
   `botster-tui-kit@bc066e2581b01fb9e5271794c9a67ba1ace36e42`,
   `crates/botster-tui-kit/src/hit_map.rs:23-24` stores only a bool,
   `crates/botster-tui-kit/src/renderer.rs:597-612` derives it from a
   `terminal_view` `mouse_mode` prop, and
   `crates/botster-tui-kit/src/input.rs:291-297` gates forwarding on that bool.
   Core's closed `terminal_view` contract does not allow this prop, so the kit
   ticket must remove the prop-derived path and hydrate renderer/input state
   through the client-owned attachment shadow. It may retain a derived bool at
   the hit-map boundary if exact bits are unnecessary there; the canonical
   attachment shadow remains the existing `u8`.
4. **TUI consumption:** use the hydrated shadow when deciding whether mouse
   events belong to Botster chrome or the focused child terminal. Preserve the
   kit's documented full-stream policy from
   `docs/plans/complete-terminal-sgr-mouse-passthrough.md:24-43`: once
   passthrough is enabled, do not filter `Moved` merely because the child
   selected 1002 rather than 1003. The existing bits decide whether the client
   can enable SGR passthrough; they do not silently narrow the already-complete
   forwarded event stream. Prove attach, detach/reattach, wheel, drag, movement,
   release, and reset behavior.

Each later ticket depends on the previous one. None may expand `terminal_view`
props or add a TUI-side DECSET parser.

## Non-Scope

- No `TerminalScreenRuntime` extension, core facade, managed-runtime request
  implementation, hub/hub-client transport, TypeScript surface, kit state or
  hit-map API, or TUI behavior in this spike.
- No change to the pinned Ghostty gitlink or the `trybotster/ghostty`
  repository.
- No Ghostty parser change, callback/event stream, Botster `ModeChanged` event,
  client-side DECSET/DECRST parser, or parallel terminal emulator.
- No `terminal_view` UiNode schema or prop change.
- No new mouse encoding, version suffix, optional configuration, generated FFI
  surface, dependency, broad refactor, or adjacent cleanup.
- No shipped safe adapter helper. The demonstration query helper is private and
  test-only; the only production delta is the minimal handwritten declaration
  needed to link the existing Ghostty symbol.

## Assumptions And Unknowns

- Assumption: the current worktree pin
  `76853b34274208fe7c051cfe13eb1c7ee63c469b` is the revision this spike must
  classify. A future Ghostty upgrade requires rechecking the C ABI.
- Assumption: the cited `trybotster@70c002f` production encoding is the missing
  source of truth for the extracted core's undocumented `u8`; if a newer
  explicit product decision supersedes it, that decision requires a human
  answer rather than silent renumbering.
- Assumption: a private test-only helper plus native runtime test is the
  permitted demonstration code described by the ticket; it is not approval to
  ship a safe adapter API or wire the full stack.
- Assumption: the synchronous query is safe after
  `ghostty_terminal_vt_write` returns. The plan does not query reentrantly from
  Ghostty's synchronous mode-change callback.
- Unknown: whether downstream core work should populate every existing
  `ModeFlags` field in one ticket or land mouse reporting first. The core ticket
  must choose explicitly; it must not falsely default fields claimed as
  authoritative.
- Known downstream gap: the kit currently has a bool derived from a
  schema-invalid `terminal_view.mouse_mode` prop, not a typed attachment shadow.
  The kit ticket must remove that source and reconcile its derived bool with the
  client-owned `u8` shadow while preserving full-stream forwarding.
- Assumption: X10 tracking (DEC mode 9) deliberately remains outside the
  established Botster `u8`. Ghostty's aggregate mouse-tracking bool includes
  X10, so downstream code must not substitute that bool for the exact bitmask.
- No human question is required for this plan: the ticket's classification,
  non-goals, bitmask shape, and locked probe-to-shadow design resolve the
  plausible interpretations.

## Affected Surfaces And Files

Expected spike changes:

- `crates/botster-terminal-ghostty/src/sys.rs`: minimal mode type/constants and
  existing FFI function declaration.
- `crates/botster-terminal-ghostty/src/lib.rs`: private test-only fallible
  helper and real libghostty DECSET/DECRST unit proof.
- `docs/architecture/libghostty-mouse-tracking-mode-feasibility.md`: written
  finding and downstream DAG.

Reference-only surfaces, not changed in this spike:

- `crates/botster-core/src/engine/terminal_screen.rs`
- `crates/botster-core/src/contract/terminal_screen.rs`
- `crates/botster-core/src/contract/session_protocol.rs`
- `crates/botster-core/src/engine/managed_session_runtime.rs`
- `crates/botster-core/src/contract/actor.rs`
- `crates/botster-terminal-ghostty/vendor/ghostty/**`

## Risks

- Treating `GHOSTTY_TERMINAL_DATA_MOUSE_TRACKING` as sufficient would lose the
  distinction between 1000/1002/1003 and the 1006 SGR format bit.
- Treating 1006 alone as active tracking would make the TUI steal mouse input
  for a child that selected an encoding but did not enable reports.
- Reordering the established 1002/1003 bits would silently break existing kit
  and TUI consumers.
- Swallowing `ghostty_terminal_mode_get` errors as `false` would produce
  authoritative-looking default state.
- A callback-based demonstration would bias later work toward the explicitly
  forbidden pushed event stream. The proof must remain a synchronous read.
- Feature-enabled native tests require the initialized submodule and Zig
  `0.15.2`. This environment has both: plan-time preflight resolved Zig
  `0.15.2`, initialized Ghostty at the pinned revision, and passed the existing
  feature-gated suite (15 tests plus one doc test). Implement owns provisioning
  through the adapter's existing `BOTSTER_ZIG`/mise-aware build path. If the
  mandatory native proof cannot execute and pass, Implement is blocked and must
  request a toolchain-capable environment; the finding may say only
  "source-evidenced, runtime-unverified" and the spike must not advance as
  verified.
- The kit currently consumes a `mouse_mode` prop that core's closed
  `terminal_view` schema rejects. Adding a parallel shadow without removing
  that stale path would preserve contradictory authority.
- Adding the full `ModeFlags` stack here would erase the spike boundary and
  prevent honest review of core, transport, kit, and input-routing changes.
- Documentation must cite vault notes as wikilinks and avoid local home or
  worktree paths.

## Acceptance Checks And Tests

Implementation acceptance:

- `cargo test -p botster-terminal-ghostty` passes without requiring native
  Ghostty or Zig.
- The Implement agent must initialize the pinned submodule and provision Zig
  `0.15.2` through the existing `BOTSTER_ZIG`/mise-aware build path. Then
  `cargo test -p botster-terminal-ghostty --features libghostty-vt
  mouse_mode` passes and proves:
  - default and fully reset state is `0`;
  - 1000 is bit `1`;
  - 1003 is bit `2`;
  - 1002 is bit `4`;
  - 1006 is bit `8`;
  - 1000 plus 1006 is `9`.
- This native command is mandatory, and its raw output must be attached. A
  skipped or unavailable native command cannot satisfy the spike. If it cannot
  run, classification (a) remains source-evidenced but runtime-unverified and
  the pipeline must stop for a toolchain-capable environment.
- `cargo test -p botster-terminal-ghostty --features libghostty-vt` passes to
  guard existing construction, write, resize, screen, snapshot, and managed
  session behavior.
- `cargo fmt --all -- --check` passes.
- `cargo clippy -p botster-terminal-ghostty --all-targets --features
  libghostty-vt -- -D warnings` passes with native preconditions available.
- `git diff --submodule=short --exit-code -- crates/botster-terminal-ghostty/vendor/ghostty`
  confirms the Ghostty pin and source are unchanged.
- `rg -n "ModeChanged|terminal_view|mouse_mode"
  crates/botster-core crates/botster-terminal-ghostty docs/architecture`
  is reviewed to confirm the spike adds only the adapter proof and finding, not
  downstream wiring, pushed events, or UiNode changes.
- `rg -n '/[U]sers/|/[h]ome/[^/[:space:]]+|[j]asonconigliari'
  docs/archive/plans/libghostty-mouse-tracking-mode-feasibility.md
  docs/architecture/libghostty-mouse-tracking-mode-feasibility.md` returns no
  matches.
- The written finding explicitly says classification (a), cites pinned
  file:line evidence, states that the demonstrated runtime path is
  DECSET/DECRST bytes -> `ghostty_terminal_vt_write` -> authoritative Ghostty
  mode state -> `ghostty_terminal_mode_get` -> existing Botster `u8` bitmask,
  and records the core -> hub -> kit -> TUI DAG.

No browser, plugin, Rails, or headless-hub harness is required because this
spike changes no such runtime surface.

## Pipeline Gates And Artifacts

- Plan artifact: this file, attached to `botster_plan_gate` with all seven
  required evidence fields.
- Plan Review must verify the classification against the pinned source, the
  exact 1002/1003 bit mapping, the demonstration-only boundary, and the ordered
  downstream DAG.
- Implement artifact must identify every changed line as either FFI proof,
  private test-only runtime proof, or written finding. It must attach raw native
  command output; unavailable native execution blocks completion.
- Review must reject any Ghostty gitlink change, core/hub/kit/TUI wiring,
  pushed mode event, UiNode prop change, duplicate mouse encoding, or unwired
  speculative abstraction.
- Verify must rerun the native runtime path, confirm the Ghostty pin is
  unchanged, scan committed artifacts for local paths, and confirm the written
  finding matches observed behavior.

## Vault Gaps Worth Capturing

- Capture the durable finding that pinned libghostty already supports both
  aggregate mouse-tracking reads and exact per-DEC-mode reads, so Botster needs
  only a narrower handwritten FFI addition rather than a Ghostty fork change.
- Capture the encoding gotcha that Botster maps 1000/1003/1002/1006 to
  `1/2/4/8` per the cited trybotster production implementation, and that 1006
  is a format bit which does not independently enable tracking.
- Capture the migration gotcha that botster-tui-kit currently derives a bool
  from a closed-schema `terminal_view.mouse_mode` prop; the downstream shadow
  ticket must replace that authority while preserving full-stream forwarding.
- Capture after the native proof passes, using an inbox-first source capture;
  do not promote the plan's conclusion to current vault truth before runtime
  verification.
