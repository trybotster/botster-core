# Plan: Core current Ghostty color profile read consistent with GHOSTSNP

- **Ticket:** `ticket_1786478568_861361`
- **Run:** `run_1786479066_837170`
- **Step:** `botster_stack_plan` (revisit after Plan Review `changes_required`)
- **Base ref:** `origin/main` @ `747be95` (worktree refreshed from stale `ff11569`)
- **Target repository:** `botster-core` (`trybotster/botster-core`)
- **Target id:** `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- **Pipeline:** `botster_stack_delivery`
- **Runtime-teardown class:** does **not** apply

## Plan Review disposition

Previous plan rejected (`review_1786480192_584047`). This rewrite addresses:

| Finding | Severity | Resolution in this plan |
| --- | --- | --- |
| Stale runtime / color-profile facts | high | Rebased worktree to `747be95`; code reality rewritten from landed APIs |
| Public API lacks color↔snapshot consistency | high | Require **one public CoreDaemon atomic result** (colors + GHOSTSNP from same terminal borrow); no separate race-prone pair of public calls as the product surface |
| Acceptance does not prove GHOSTSNP color content | high | Require capture → import GHOSTSNP into **fresh** Ghostty → re-read profile → full equality; then mutate live and prove prior pair stable |
| Incomplete runtime/Ghostty review context | medium | Load runtime reviewer/verifier overlays + Ghostty submodule/Zig/pin/cache notes; exact native gate recorded |
| Conditional cross-repo dependency wording | low | Record existing `dependency_1786479063_760119` (Hub → this Core ticket) with both target ids |

## Repository routing

| Field | Value |
| --- | --- |
| `target_repository` | `botster-core` |
| `target_id` | `tgt_1f7bce66eb304881980f9b4a2a5ae3fe` |
| Repository playbook | [[botster-core-playbook]] |
| Concrete backend charter (same monorepo) | [[botster-terminal-ghostty-playbook]] |

Independently resolved via `list_spawn_targets`: name `botster-core`, path is the botster-core checkout; this pipeline worktree tracks that target at `747be95`.

## Playbooks and notes loaded

### Role / repository / runtime overlays
1. [[planner-playbook]]
2. [[botster-planner-playbook]]
3. [[botster-core-playbook]]
4. [[botster-terminal-ghostty-playbook]]
5. [[botster-architecture]]
6. [[cli-patterns]]
7. [[botster-runtime-reviewer-playbook]] — required for daemon/worker/terminal changes
8. [[botster-runtime-verifier-playbook]] — required verification overlay for same surface

### Targeted atomic notes
- [[coredaemon must expose terminal truth used by the production hub path]]
- [[ghostty shadow terminal integration belongs outside botster core]]
- [[session-process-owns-vt-parser-hub-rpc-snapshots]]
- [[botster core contract surface needs consumer proof]]
- [[botster core public surface needs a narrow start here path]]
- [[split terminal runtimes drop color probe responses before client attachment]]
- [[pinned libghostty exposes synchronous exact mouse mode state]]
- [[libghostty-vt-embedder-callback-architecture-and-constraints]]
- [[botster cli integration tests require ghostty submodule initialization]]
- [[botster build rs mise exec can override path zig selection]]
- [[vendored ghostty non-semver tags block rust builds at exact tag commits]]
- [[zig local and global caches must share cargo build scope]]
- [[plan steps need reviewable plan artifacts]]
- [[vault example paths are not repository placement conventions]]
- [[botster core historical plan docs should not sit beside living architecture]]
- [[plan agents must author vault context as wikilinks not home paths]]
- [[pipeline vault checklists must cite exact resolvable note titles]]

### Explicitly not loaded
- [[project-pipelines-playbook]] — not package/plugin work
- [[botster runtime teardown lenses]] — not runtime-teardown class
- Hub / web / TUI / Restty ownership charters — consumer tickets only

## Context loaded

### Ticket intent
Production **CoreDaemon** current-color read for session Ghostty that returns palette + special colors (foreground, background, cursor) after OSC mutations, with a **revision or ordering boundary** vs GHOSTSNP so attach/reconnect cannot disagree.

Human constraints (`question_1786478263_847168`):
1. Ghostty owns current palette/specials after session start
2. Hub startup profile is **initial/reset baseline only**; config changes must not rewrite durable session color state
3. GHOSTSNP remains authoritative durable state for late attach/reconnect

Out of scope: Hub client DTOs (`ticket_1786471489_718500`), Restty themes.

### Project / dependency graph (recorded, not speculative)
- Project: Ghostty-only terminal cutover (`project_1786468118_227513`)
- **Existing dependency (do not re-register):**
  - `dependency_1786479063_760119`
  - **ticket:** `ticket_1786471489_718500` (Hub) **depends_on** `ticket_1786478568_861361` (this Core ticket)
  - Hub target: `tgt_7e208a0c76a44980a83b63af976b1f22` (`botster-hub`)
  - Core target: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe` (`botster-core`)
- This Core run does **not** implement Hub DTOs; Hub remains blocked on this ticket closing.

### Code reality at `747be95` (re-audited)

#### Already landed — **do not re-implement**
| Surface | Landed behavior |
| --- | --- |
| Ghostty COLOR constants | `GHOSTTY_TERMINAL_DATA_COLOR_{FOREGROUND,BACKGROUND,CURSOR,PALETTE}` + OPT setters in `sys.rs` |
| Special index contract | `COLOR_INDEX_FOREGROUND = 0x1000`, `BACKGROUND = 0x1001`, `CURSOR = 0x1002` (reserved indexes in `TerminalColorProfile.colors`) |
| `GhosttyTerminal::read_color_profile` | Full 256 palette + optional specials; `GHOSTTY_NO_VALUE` → omit special |
| `GhosttyTerminal::apply_color_profile` | Host/default apply via OPT color setters |
| Trait | `TerminalScreenRuntime::{color_profile,set_color_profile}` |
| Screen state | Ghostty `screen_state()` populates `color_profile` |
| Host config seam | `CoreDaemonConfig::with_terminal_color_profile` — policy-free initial baseline only |
| Managed apply | `prepare_color_profile` applies profile on `SetColorProfile` request path |
| OSC probe replies | `write_pty` / `drain_pty_writes` + worker-backed OSC 10/11/12 integration test |
| Conformance helpers | `assert_color_profile_authority`, `assert_special_color_defaults`, Ghostty authority conformance tests |
| Unit coverage | Ghostty `color_profile_round_trips_palette_and_special_colors` |

#### Still missing — **this ticket's real scope**
| Gap | Why it matters |
| --- | --- |
| No public `CoreDaemon` color read | Hub production path uses CoreDaemon only ([[coredaemon must expose terminal truth used by the production hub path]]) |
| No public atomic colors+GHOSTSNP result | Separate `read_color` + `capture_snapshot` can race with live PTY drain/output |
| `RetainedTerminalState` freezes only text/snapshot/mode | Post-exit color read would disagree with frozen GHOSTSNP if colors are omitted |
| No `GetColorProfile` / `ColorProfileReady` SessionIo pair | Optional if atomic CoreDaemon method is the sole public surface; prefer smallest set Hub can project |
| No GHOSTSNP **import** equality proof for OSC-mutated colors | Magic/`format` checks do not prove snapshot carries palette/specials |
| `capture_terminal_state` returns screen (which may embed colors) + snapshot + modes, but daemon never exposes colors from that | Internal helper is not a production Hub seam |

### Ghostty pin / build gates (for Implement + Verify)
- Vendored pin: trybotster/ghostty `5e9ba17a22ba8e40bf8de7d3e7555b8378cb1880`
- Zig: exact `0.16.0` (`mise.toml`; CI installs pinned tarball)
- Submodule: `git submodule update --init crates/botster-terminal-ghostty/vendor/ghostty`
- Native gate: `cargo test -p botster-terminal-ghostty --features libghostty-vt`
- Cache isolation: Zig local+global under Cargo `OUT_DIR`; CI asserts `libghostty-vt.a` exists and checkout `.zig-cache` is absent
- Worktree path has no `:`; `.gitignore` matches HEAD (64 bytes)

### Production entry point that must change
Hub-facing production path: **`CoreDaemon`**. Success is a new (or extended) CoreDaemon method Hub can call that returns **current colors and GHOSTSNP from one terminal ownership critical section**, including live drain-before-read and retained post-exit freeze. Ghostty helper existence alone is not acceptance.

## Scope

### In scope
1. **Public CoreDaemon atomic authority read** (preferred minimal product shape):
   - Add something like `CoreDaemon::capture_color_and_snapshot(CaptureColorAndSnapshotRequest) -> CaptureColorAndSnapshotResult`
   - Result **must** include:
     - `color_profile: TerminalColorProfile` (palette + specials via reserved indexes — **reuse landed contract**, do not add optional struct fields)
     - `snapshot: SnapshotReady` and/or `payload: TerminalSnapshotPayload` with `ghostty-terminal-snapshot-v1` / GHOSTSNP
     - correlation ids (`request_id`, `session_id`) consistent with existing readback DTOs
   - Implementation **must** obtain both values under **one** session terminal borrow (extend `capture_terminal_state` or sibling worker method), after the existing drain-before-read path used by other readbacks
   - **Do not** ship separate public `read_color_profile` + `capture_snapshot` as the only consistency story. Standalone color read is acceptable only if it also returns a **shared monotonic terminal state revision** also present on snapshot results and Hub can validate — atomic dual return is the smaller consumer story and preferred

2. **Retained final terminal freeze**
   - Extend `RetainedTerminalState` with frozen `color_profile`
   - Populate from the same atomic capture used for snapshot/mode freeze
   - Atomic CoreDaemon method serves retained pair without re-entering a live terminal

3. **Managed/engine wiring only as needed**
   - Promote color out of `TerminalScreenState` / `runtime.color_profile()` through the worker adapter in the atomic capture
   - Prefer reusing existing Ghostty `read_color_profile` / trait methods — no second color model
   - SessionIo `GetColorProfile`/`ColorProfileReady` only if required for Hub projection of a **non-snapshot** poll; if atomic CoreDaemon result is enough for Hub ticket, skip extra SessionIo surface (charter: consumer proof, not speculative contracts)

4. **Host profile policy remains baseline-only**
   - Keep `with_terminal_color_profile` as spawn/initial defaults
   - After session start, Ghostty OSC ownership wins; do not re-apply host profile on config change mid-session
   - Do not expand SetColorProfile into a durable rewrite API

5. **Tests (no Hub synthesis)**
   - **Worker-backed CoreDaemon production path:** spawn with worker path, apply OSC 4/10/11/12 mutations via session PTY/output path that reaches the Ghostty shadow, call atomic CoreDaemon API
   - **GHOSTSNP content proof:** take returned GHOSTSNP payload → `replay_snapshot`/`import` into a **fresh** `GhosttyTerminal` → `color_profile()` equals the paired profile on every required index (full palette presence per conformance helper if complete; at least OSC-mutated palette entry + all three specials)
   - **Stability:** mutate live terminal again after capture; frozen atomic result (or retained) remains equal to pre-mutation pair; live re-capture differs
   - **Drain-before-read egress retention** parity with snapshot/mode tests
   - Plain / `--no-default-features` lane: fail closed (unsupported/error), no invented Ghostty colors if that lane still exists on current daemon features; if production is Ghostty-only, document and test Ghostty path only
   - Reuse `assert_color_profile_authority` / specials helpers where they fit

6. **Docs**
   - Update `docs/architecture/core-daemon.md` for the atomic color+snapshot CoreDaemon surface and retained freeze
   - Mention reserved special indexes and baseline-only host profile (link living Ghostty authority doc if present)
   - Keep this plan under `docs/archive/plans/` (repo placement)

### Non-scope
- Re-implementing Ghostty `read_color_profile`, reserved indexes, trait methods, host config seam, OSC write_pty replies
- Changing special colors to optional struct fields (conflicts with landed reserved-index contract)
- Hub client DTOs / attach ordering product contract (`ticket_1786471489_718500`)
- Restty themes / client-side OSC answering
- Mode-revision race ticket (`ticket_1786478568_882200`)
- Broad refactors, render-state row iteration, second color authority

## Repository ownership boundaries and cross-repo dependencies

| Owner | Responsibility |
| --- | --- |
| **botster-core** (this target) | CoreDaemon atomic public API, retained freeze, managed capture promotion, production-path tests |
| **botster-terminal-ghostty** (same monorepo) | Already owns color get/set + GHOSTSNP; Implement may only add tests (import equality) if needed |
| **botster-hub** (`tgt_7e208a0c76a44980a83b63af976b1f22`) | Projects Core atomic result into client contract after this ticket closes |

### Cross-repo dependencies
- **Inbound to this ticket:** none blocking (Ghostty-only authority parent closed)
- **Outbound / consumer:** already registered
  - `dependency_1786479063_760119`: Hub `ticket_1786471489_718500` depends on this Core ticket
  - Targets: Hub `tgt_7e208a0c76a44980a83b63af976b1f22` ← Core `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`

## Assumptions and unknowns

### Assumptions
1. Parent Ghostty shadow (updated on drain) remains the color/snapshot authority for worker-backed sessions; session worker stays PTY-only.
2. Landed reserved indexes (`0x1000`/`0x1001`/`0x1002`) are the durable special-color contract Hub will project.
3. Atomic dual-return CoreDaemon API is the preferred ordering boundary; a revision counter is only the fallback if dual-return is rejected for API size reasons.
4. Host `terminal_color_profile` remains initial/reset only and is not re-applied over live OSC state.
5. GHOSTSNP import on a fresh Ghostty terminal is a valid content oracle for color fidelity.

### Unknowns / Implement checkpoints
1. Exact DTO name (`capture_color_and_snapshot` vs extend an existing capture result) — choose smallest additive CoreDaemon surface.
2. Whether SessionIo get-color is required for Hub or only CoreDaemon atomic method (ask Hub plan only if Hub cannot project a CoreDaemon dual result).
3. Whether post-exit retained path must error if colors were never captured vs always capture colors with snapshot freeze (prefer always freeze both).
4. Completeness of palette in atomic result: Ghostty `read_color_profile` already returns full 256 — keep that; do not sparsify.

## Affected surfaces / files (expected)

### Likely edits
- `crates/botster-core-daemon/src/api.rs` — request/result DTOs for atomic color+snapshot
- `crates/botster-core-daemon/src/daemon.rs` — public method, drain-before-read, retained freeze
- `crates/botster-core-daemon/src/lib.rs` — re-exports
- `crates/botster-core-daemon/tests/daemon_integration_test.rs` — worker-backed + import equality + stability
- `crates/botster-core/src/engine/managed_session_runtime.rs` — atomic capture returns colors with snapshot under one borrow
- `crates/botster-core/src/engine/botster.rs` — promote if engines expose the atomic capture
- Possibly `session_worker` / engine helpers if SessionIo color get is added (only if needed)

### Tests may also touch
- `crates/botster-terminal-ghostty/tests/native_runtime_test.rs` or unit module — optional import equality helper reused by daemon tests
- `crates/botster-core-test-support` — only if dual-result helpers belong in conformance

### Docs
- `docs/architecture/core-daemon.md`
- This plan file

### Explicitly not reworked
- Ghostty color get/set implementation, reserved indexes, `with_terminal_color_profile` baseline seam (consume as-is)

## Risks

| Risk | Mitigation |
| --- | --- |
| Separate public color/snapshot calls race | Ship atomic dual-return as the Hub-facing surface |
| Re-inventing color contract vs reserved indexes | Reuse `TerminalColorProfile` + `COLOR_INDEX_*` only |
| Snapshot magic without color content | Mandatory import-and-compare acceptance |
| Retained path omits colors | Freeze color_profile with snapshot on final retain |
| Host profile reapplied mid-session | Keep baseline-only; tests prove OSC wins after start |
| Build false failures (submodule/Zig/cache) | Exact native gate + pin notes; assert archive before cache absence |
| Overbuilding SessionIo | Prefer CoreDaemon-only until Hub proves need |

## Acceptance checks / tests

### Product acceptance
1. **Worker-backed CoreDaemon atomic capture** after OSC 4 (at least one palette index) and OSC 10/11/12 specials returns:
   - non-empty `TerminalColorProfile` with mutated values at the correct indexes (including `COLOR_INDEX_*` specials)
   - GHOSTSNP payload (`format == ghostty-terminal-snapshot-v1`, magic `GHOSTSNP`)
2. **GHOSTSNP color content proof:** import that snapshot into a **fresh** Ghostty terminal; re-read color profile; assert equality for all required palette/special values against the paired profile from the same atomic result.
3. **Ordering boundary:** while live PTY could mutate, a single atomic call never returns colors from state A and snapshot from state B (enforced by same-borrow implementation + tests that mutate only between captures).
4. **Stability:** after capture, further OSC mutation changes a **new** atomic capture; the previously returned pair (or retained freeze) remains unchanged when re-served from retention / held result.
5. **Drain-before-read:** incidental client egress from internal drain remains available exactly once on next explicit `drain`.
6. **Baseline-only host profile:** if host profile is applied at spawn, OSC mutations after start show in atomic capture (Ghostty owns current state); plan does not re-apply host profile on config change.
7. **Fail-closed / Ghostty-only:** no invented colors on unsupported backends; production Ghostty path is the proof lane.

### Repo gates (Implement + Verify)
```bash
# worktree hygiene already OK (.gitignore matches HEAD; no colon in path)
git submodule update --init crates/botster-terminal-ghostty/vendor/ghostty

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p botster-terminal-ghostty --features libghostty-vt
# if daemon still offers a no-default-features lane on this base, keep it; otherwise document removal
```
CI also requires generated `libghostty-vt.a` and absence of vendored checkout `.zig-cache` (Zig 0.16.0, pin `5e9ba17a22ba8e40bf8de7d3e7555b8378cb1880`).

### Downstream proof policy
- Consumer-shaped proof for this ticket: **worker-backed CoreDaemon atomic API** Hub will project.
- Full Hub attach/reconnect product proof stays on Hub ticket after dependency closes.
- Crate-local Ghostty unit tests alone are insufficient without the daemon production path.

### Runtime review/verify hooks
- Use [[botster-runtime-reviewer-playbook]] / [[botster-runtime-verifier-playbook]] at Review/Verify for ownership, drain, retain, and terminal semantics.
- Not runtime-teardown class; do not force teardown-lens fields.

## Implementation sequence

1. Extend managed `capture_terminal_state` (or sibling) to return authoritative `TerminalColorProfile` with snapshot under one borrow (read via existing `runtime.color_profile()` / screen color field).
2. Add CoreDaemon request/result + public atomic method; wire live drain-before-read and retained freeze of colors.
3. Worker-backed integration: OSC mutate → atomic capture → import GHOSTSNP → equality → re-mutate stability.
4. Docs + export cleanup.
5. Full CI / native Ghostty gates.

## Vault gaps worth capturing

1. **Public color↔GHOSTSNP consistency is an atomic CoreDaemon dual-return (or shared revision), not two independent reads** — capture after Implement if not already vaulted.
2. **GHOSTSNP content proof requires import into a fresh Ghostty terminal and profile equality** — envelope magic is insufficient.
3. **Special colors use reserved TerminalColorProfile indexes 0x1000–0x1002** — already partially in code; capture as durable product note if missing from vault.
4. **Host terminal_color_profile is spawn baseline only; Ghostty owns current after start** — product decision; capture if not present.

## Plan gate evidence map

| Required field | Content |
| --- | --- |
| `target_repository` | `botster-core` |
| `target_id` | `tgt_1f7bce66eb304881980f9b4a2a5ae3fe` |
| `repository_playbook` | [[botster-core-playbook]] |
| `playbooks_notes_loaded` | listed above (includes runtime overlays + Ghostty build notes) |
| `context_loaded` | ticket, dependency_1786479063_760119, base 747be95 landed vs missing, pin/Zig |
| `scope` | Atomic CoreDaemon colors+GHOSTSNP + retain + import proof; reuse landed Ghostty contract |
| `ownership_boundaries_dependencies` | Core owns public dual-return; Ghostty owns backend; Hub dep already registered |
| `assumptions_unknowns` | shadow authority; reserved indexes; dual-return preferred; SessionIo optional |
| `affected_surfaces_files` | primarily core-daemon + managed capture + tests/docs |
| `risks` | races, contract drift, weak snapshot proof, host reapply |
| `acceptance_checks_tests` | worker-backed atomic + import equality + stability + CI/native Ghostty |
| `vault_gaps` | four capture candidates |
| `teardown_class_applies` | `false` |
| `base_sha` | `747be95` |
