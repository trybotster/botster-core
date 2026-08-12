# Plan: Core Ghostty client read-only projection for TUI GHOSTSNP render

- **Ticket:** `ticket_1786509045_208932`
- **Run:** `run_1786509197_353111`
- **Step:** `botster_stack_plan` (visit 2 after Plan Review `changes_required`)
- **Base ref:** worktree @ `2c5171a6cb3b073c53620a9838d8b08480dd215c` (matches Plan Review origin/main)
- **Target repository:** `botster-core` (`trybotster/botster-core`)
- **Target id:** `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- **Pipeline:** `botster_stack_delivery`
- **Runtime-teardown class:** does **not** apply
- **Revision:** 2 — addresses `review_1786510154_588114`

## Plan Review disposition

Previous plan rejected (`review_1786510154_588114`, verdict `changes_required`). This rewrite addresses:

| Finding | Severity | Resolution in this plan |
| --- | --- | --- |
| Client install does not match consumed Hub Snapshot | blocker | Public install takes **opaque GHOSTSNP bytes only** (Hub `DaemonOpaqueHistoryPayload.decoded_bytes()` shape). No dimensions/format inputs. Dimensions from decoded Ghostty. Hub 89dae7e-shaped fixture required. |
| Public projection lacks downstream consumer proof | high | **In-ticket** clean scratch consumer / non-owning fixture maps public projection fields into a Ratatui-shaped cell buffer. TUI product code stays out of this worktree. |
| Renderer / full-scrollback / OSC acceptance under-specified | high | Pin minimum public projection fields and mandatory acceptance cases (grapheme, wide cells, resolved FG/BG, attributes, cursor, top/bottom/delta history, **real OSC** palette/specials). |
| Duplicate Plan vault checklists | low (process) | Reuse `checklist_1786509519_980369`; do **not** create another Plan checklist. |

## Repository routing

| Field | Value |
| --- | --- |
| `target_repository` | `botster-core` |
| `target_id` | `tgt_1f7bce66eb304881980f9b4a2a5ae3fe` |
| Repository playbook | [[botster-core-playbook]] |
| Concrete package charter (same monorepo) | [[botster-terminal-ghostty-playbook]] |
| Package path | `crates/botster-terminal-ghostty` |

Resolved via `list_spawn_targets`: name `botster-core`. Human answer `question_1786508866_164600` routes ownership here. Do not use a separate Ghostty spawn target.

## Playbooks and notes loaded

### Role / repository / surface overlays
1. [[planner-playbook]]
2. [[botster-planner-playbook]]
3. [[botster-core-playbook]]
4. [[botster-terminal-ghostty-playbook]]
5. [[botster-architecture]]
6. [[cli-patterns]]

### Targeted atomic notes
- [[ghostty shadow terminal integration belongs outside botster core]]
- [[session-process-owns-vt-parser-hub-rpc-snapshots]]
- [[libghostty-vt-embedder-callback-architecture-and-constraints]]
- [[pinned libghostty exposes synchronous exact mouse mode state]]
- [[ghostty scrollbar state is the source of truth for tui terminal scroll]]
- [[ghostty-snapshot-api-page-init-gotcha]]
- [[botster clients restore visible terminal state from readscreen before buffered live output]] — documents Hub opaque history fields; this ticket’s install uses those **decoded bytes**, not ReadScreen text
- [[botster core contract surface needs consumer proof]]
- [[botster core public surface needs a narrow start here path]]
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
- Hub / TUI / kit ownership charters as implementation surface — consumer shape only

## Context loaded

### Ticket intent
Extend `crates/botster-terminal-ghostty` with the smallest reusable **client-facing read-only projection** so first-party TUI can install and render Hub GHOSTSNP without decoding terminal truth in `app.rs` or `botster-tui-kit`.

### Consumed Hub Snapshot artifact (authoritative for install input)

Verified at Hub pin **`89dae7e15a844bcb7411b83b32581121720e23eb`** (`botster-hub-client`):

```text
DaemonEvent::Snapshot {
  session_id: String,
  subscription_id: String,
  #[serde(flatten)]
  history: DaemonOpaqueHistoryPayload {
    payload_base64: String,
    payload_encoding: Base64,
    bytes: usize,
  },
}
```

- Client obtains install bytes only via `history.decoded_bytes()` (validated base64 length).
- **No dimensions, no format label, no rows/cols** on the data-plane Snapshot event.
- `DaemonEvent::Scrollback` uses the **same** opaque history shape; GHOSTSNP install must never accept Scrollback as a product path (caller must not pass it; Core rejects non-GHOSTSNP bodies).
- Control-path `DaemonCaptureSnapshot` metadata (rows/cols/format/payload_bytes) is **not** the data-plane install carrier and must not be required by the client projection install API.

### Code reality at base `2c5171a`
| Surface | State |
| --- | --- |
| Session `GhosttyTerminal` | import/export via `TerminalSnapshotPayload`, VT write, color/mode reads, **write_pty OSC answers** |
| Client-facing install from Hub **bytes only** | **Missing** |
| Cell/style projection + scroll navigation | **Missing** |
| Downstream-shaped public consumer proof | **Missing** |

Vendored pin: `5e9ba17a22ba8e40bf8de7d3e7555b8378cb1880`. CLI `ghostty_vt.rs` remains prior art for render-state and scrollbar FFI.

### Architecture
```
Hub data plane (consumer; not implemented here)
  DaemonEvent::Snapshot.history.decoded_bytes() ──► opaque GHOSTSNP bytes
  DaemonEvent::Scrollback.history                 ── never pass to install
  DaemonEvent::TerminalOutput.data                ── later live bytes

crates/botster-terminal-ghostty (this ticket)
  GhosttyClientProjection (libghostty-vt)
    install_ghostsnp(bytes)     ── magic + decode fail-closed ──► Ghostty state
                                  dimensions from decoded Ghostty (not Hub fields)
    apply_terminal_output(bytes)
    project_viewport()          ── owned cells + styles (pinned fields)
    scrollbar() / scroll_*      ── Ghostty-owned viewport truth
    color_profile()             ── after real OSC (and live VT)
    cursor() / mode_flags()
    NO write_pty answering; NO PTY ownership

Downstream-shaped gate (this ticket, non-owning fixture)
  install Hub-shaped bytes → apply live → map projection → Ratatui-shaped cells

TUI (ticket_1786471490_592122, later)
  attach policy only; pins this crate; maps projection → UI
```

## Scope

### In scope
1. **Public client type** behind `libghostty-vt` (working name `GhosttyClientProjection`).
2. **Hub-shaped install API (blocker fix):**
   - Primary public entry: `install_ghostsnp(bytes: &[u8]) -> Result<(), …>` (name may vary; **bytes-only**).
   - Preconditions: non-empty; starts with magic `GHOSTSNP`; Ghostty decode succeeds.
   - Fail closed on empty, non-magic, corrupt body, decode failure; do not swap live handle on failure.
   - **Do not** require `TerminalSnapshotPayload`, format label, or Hub dimensions as install inputs.
   - After success, **dimensions** come from the decoded Ghostty terminal (query size / projection size). Optional separate `resize` remains for client viewport policy, not for install authenticity.
   - Document: TUI must pass `Snapshot.history.decoded_bytes()` only; never `Scrollback` payloads.
3. **Apply live output** into installed state.
4. **Pinned read-only projection fields** (minimum public contract — high fix):

| Public type | Required fields |
| --- | --- |
| `ProjectedCell` | `grapheme: String` (base + combining as one cell string); `wide: ProjectedWide` (`Narrow` / `Wide` / `SpacerTail` / `SpacerHead`); **resolved** `fg: Rgb`; **resolved** `bg: Rgb`; `bold`, `italic`, `underline`, `inverse`, `faint`, `strikethrough` (bools) |
| `ViewportProjection` | `cols: u16`, `rows: u16`, row-major `cells: Vec<ProjectedCell>` with `len == cols * rows`, `cursor: CursorProjection` |
| `CursorProjection` | `visible: bool`, `in_viewport: bool`, `x: u16`, `y: u16` (defined when `in_viewport`), `style: CursorStyle` (`Block` / `Bar` / `Underline` / `Hollow` as available on pin) |
| `ScrollbarState` | `total: usize`, `offset: usize`, `len: usize` (Ghostty scrollbar; derive lines-from-bottom / scrollback depth in helpers if useful) |
| `ScrollOp` | at least `Top`, `Bottom`, `Delta(i32)` (negative = up) |

   Prefer Ghostty render-state snapshot + color resolution so FG/BG are **RGB ready for a client renderer map** (not unresolved palette indexes alone). Palette indexes may be retained as extra fields only if free; resolved RGB is mandatory.

5. **Scroll navigation** from Ghostty at read time (no Rust mirror offset as truth).
6. **Palette + specials** via existing profile read path after **real OSC** mutations.
7. **Mode flags** via existing `read_mode_flags` when available on runtime.
8. **No session product paths:** do not install `write_pty` answering on the client type.
9. **sys.rs FFI** only as needed (render state, scrollbar, scroll viewport, cursor).
10. **Tests + docs** as in Acceptance.
11. **Downstream-shaped public-API gate** (high fix) — see Acceptance.

### Out of scope
- TUI / kit / hub / web product code in this worktree.
- Changing Hub DTOs or conformance packages.
- Session-worker PTY ownership / session `GhosttyTerminal` OSC reply path (leave intact).
- Requiring `TerminalSnapshotPayload` for the **client install** path (session export may still use it internally to produce bytes for fixtures).
- Restty / second VT parser in `botster-core`.
- Broad CLI module transplant; dirty multi-thread renderer engine.
- Ghostty pin bump unless required symbols missing (then ask human).

## Repository ownership boundaries and cross-repo dependencies

| Concern | Owner | This run |
| --- | --- | --- |
| Neutral core contracts | `botster-core` crate | consume |
| Concrete Ghostty + client projection | `botster-terminal-ghostty` | **implement** |
| Hub Snapshot/Scrollback/TerminalOutput DTO | hub (closed parent) | **consume shape only** (89dae7e) |
| TUI attach + UI map | `ticket_1786471490_592122` | downstream after pin |
| Kit ui-contract pin | `ticket_1786509045_506152` | sibling |

No new cross-repo dependency registration required to start. This ticket remains the blocker for TUI.

### Consumer proof (charter)
Per [[botster core contract surface needs consumer proof]], crate-local sequence alone is insufficient. This ticket **must** ship a **non-owning downstream-shaped fixture** that:
1. Builds real GHOSTSNP bytes (export from a session-shaped terminal or producer helper).
2. Installs via **bytes-only** public API (Hub Snapshot shape: no format/dims).
3. Applies live TerminalOutput-shaped bytes.
4. Maps `ViewportProjection` into a **Ratatui-shaped** structure (`symbol`/grapheme, `fg`/`bg` as RGB→ratatui Color, modifiers from bold/italic/underline/inverse/etc.) **without** depending on `botster-tui` or kit product crates.
5. Asserts mapped buffer content for text, color, and at least one style attribute.

Live multi-process Hub attach remains TUI’s gate after pin; this ticket proves the **public pin surface is consumable** by a TUI-shaped mapper.

## Assumptions and unknowns

### Assumptions
1. Hub 89dae7e Snapshot bytes-only carrier is the install input contract for first-party TUI.
2. Decoded GHOSTSNP fully determines terminal dimensions and content; clients do not fabricate format labels.
3. Pin `5e9ba17` exposes render-state, scrollbar, scroll-viewport, and cursor style data (CLI + headers).
4. Resolved RGB for cell FG/BG is available through render-state color resolution on this pin.
5. Runtime-teardown class does not apply.

### Unknowns (Implement defaults; ask human only if blocked)
1. Final public type names (fields above are the contract; names may polish).
2. If pin cannot resolve RGB for some cells, fail the projection read loudly rather than invent defaults without documenting.
3. Whether `install_ghostsnp` returns the new dimensions explicitly or only via `dimensions()` / `project_viewport()` — either is fine if docs + tests pin the observation path.

## Affected surfaces / files

| Path | Change |
| --- | --- |
| `crates/botster-terminal-ghostty/src/sys.rs` | FFI for render/scroll/cursor as needed |
| `crates/botster-terminal-ghostty/src/lib.rs` | Client projection type + pinned public projection types |
| `crates/botster-terminal-ghostty/tests/*` | native proofs + **Hub-shaped** install fixture + **downstream mapper** gate |
| `crates/botster-terminal-ghostty/README.md` | Public API pin docs (bytes install, field table, non-goals) |
| `docs/archive/plans/core-ghostty-client-read-only-projection-for-tui-ghostsnp-render.md` | this plan |
| Optional on land | short note in `docs/architecture/ghostty-shadow-terminal-adapter.md` |

## Implementation sketch

1. Init submodule when needed.
2. Bind minimal FFI from pin (CLI layout sizes).
3. `GhosttyClientProjection`:
   - Own Ghostty handle **without** write_pty answering.
   - `install_ghostsnp(&[u8])`: magic check → decoder → swap on success → re-arm continuation.
   - `apply_terminal_output`, `resize`, `scrollbar`, `scroll(ScrollOp)`.
   - `project_viewport()`: render_state update → copy owned cells with resolved RGB + attributes + cursor.
   - `color_profile()`, `mode_flags()`, `dimensions()`.
4. Keep session `GhosttyTerminal` for producer/fixture export and session path.
5. Add tests listed below; document pin surface in README.

## Risks

| Risk | Mitigation |
| --- | --- |
| Reintroducing format/dims install inputs | Public API + Hub-shaped fixture reject that design |
| Weak style/color proofs | Pinned fields + OSC-only color tests + wide-cell case |
| Scroll mirror staleness | Query Ghostty scrollbar each read |
| write_pty leak into client | Omit callback; assert no reply capture product path |
| “Consumer proof” that only reuses crate internals | Separate scratch mapper to Ratatui-shaped cells, no TUI crate |
| Over-port CLI | Smallest field set only |

## Acceptance checks / tests

### Commands
```sh
git submodule update --init crates/botster-terminal-ghostty/vendor/ghostty
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p botster-terminal-ghostty --features libghostty-vt
cargo test --workspace
test -e crates/botster-terminal-ghostty/vendor/ghostty/zig-out/lib/libghostty-vt.a
test ! -e crates/botster-terminal-ghostty/vendor/ghostty/.zig-cache
```

### Product proofs (mandatory)

**A. Hub-shaped install (blocker)**
1. Produce real GHOSTSNP bytes via session export (or producer helper).
2. Install with **bytes only** — no format label, no size struct required.
3. Fixture models Hub 89dae7e consumption: treat payload as `DaemonOpaqueHistoryPayload::from_bytes(ghostsnp).decoded_bytes()` equivalent (raw bytes after decode).
4. Fail closed: empty; non-`GHOSTSNP` magic; garbage body; **Scrollback-like non-GHOSTSNP body** never installs.
5. After install, `dimensions()` / `project_viewport()` reflect **decoded** size.

**B. Apply live**
Post-install VT/live bytes update projected graphemes.

**C. Pinned renderer fields (high)**
1. Grapheme content visible in `ProjectedCell.grapheme`.
2. Wide-cell behavior: write a wide character; assert `Wide` / spacer kinds as Ghostty reports (at least one non-`Narrow` case).
3. Resolved FG/BG RGB present on styled cells (SGR colors).
4. At least bold and inverse (or underline) attributes round-trip into bools.
5. Cursor: after positioning writes, `visible`, `in_viewport`, `x`/`y`, and `style` are coherent.

**D. Full retained scrollback navigation (high)**
1. Configure scrollback budget > 0.
2. Write enough lines that markers exist **outside** the live viewport at **distinct** history depths (minimum: one near top of retained history, one just above viewport).
3. Assert `scrollbar()` reports depth (`total > len`).
4. `ScrollOp::Top` surfaces the far marker in projection.
5. `ScrollOp::Bottom` returns to live edge.
6. `ScrollOp::Delta` moves between positions (not only a single scroll-up once).

**E. OSC palette and specials (high)**
1. After install (or on a live client terminal), apply **real OSC** sequences that set palette entry and special colors (OSC 4 / 10 / 11 / 12 as supported), **not** `set_color_profile` as the proof path.
2. `color_profile()` (or projection colors) reflects palette index + FG/BG/cursor specials.
3. Client type does not expose session-style OSC **reply** capture as product behavior.

**F. Downstream-shaped public consumer gate (high)**
In-crate (or workspace test-only) scratch consumer that:
1. Depends only on public `botster-terminal-ghostty` (+ core RGB types as needed) and optionally `ratatui` as a **dev-dependency** for the mapper shape **or** a local `RatatuiCellLike { symbol, fg, bg, bold, … }` struct that mirrors Ratatui’s buffer cell fields without requiring the TUI app.
2. Installs Hub-shaped GHOSTSNP bytes.
3. Applies live output.
4. Maps each `ProjectedCell` → consumer cell fields.
5. Asserts mapped content/color/style — proving the public types can drive a TUI renderer map.
6. Contains **no** `botster-tui` / kit product imports.

**G. Docs**
README documents: bytes-only install; Hub Snapshot consumption; pinned field table; scroll ops; non-goals (no PTY, no OSC answering, no Scrollback install); feature `libghostty-vt`; consumer pin path for TUI ticket.

### Downstream live Hub attach
Owned by TUI `ticket_1786471490_592122` after it pins the landed Core revision. Not waived; **not** a substitute for F above.

## Runtime-teardown class

`teardown_class_applies`: **false**

## Vault gaps (post-implement if still novel)
1. Client projection install is **bytes-only** matching Hub opaque history (no format/dims).
2. Client vs session GhosttyTerminal split (no write_pty answering on client).
3. Pinned `ProjectedCell` field set for TUI/Ratatui mappers.

## Worktree / checklist hygiene
- `.gitignore` non-empty; path has no `:`.
- **Primary checklist:** `checklist_1786509519_980369` (reuse; no new Plan checklist this visit).
- Duplicate checklist IDs from visit 1 remain historical (`checklist_1786509488_280977`, `checklist_1786509500_706140`).

## Product decision ledger

| Item | Decision |
| --- | --- |
| Install carrier | Hub-decoded **bytes only**; magic + decode fail-closed |
| Dimensions | From decoded Ghostty after install |
| Format label | Not an install input |
| Style fields | Pinned table above (resolved RGB + attributes + wide + grapheme) |
| Color proof | Real OSC only (not profile-set substitute) |
| Scroll proof | Top + Bottom + Delta across retained history |
| Consumer proof | In-ticket Ratatui-shaped mapper fixture |
| Non-goals | TUI product code; session PTY/OSC replies; Restty |
| Ask human if | Pin missing required render/scroll symbols |

## Completion evidence fields

| Field | Value |
| --- | --- |
| `plan_uri` | `docs/archive/plans/core-ghostty-client-read-only-projection-for-tui-ghostsnp-render.md` |
| `target_id` | `tgt_1f7bce66eb304881980f9b4a2a5ae3fe` |
| `target_repository` | `botster-core` |
| `repository_playbook` | `botster-core-playbook` |
| `checklist_id` | `checklist_1786509519_980369` (reused) |
| `teardown_class_applies` | `false` |
| `addresses_findings` | `finding_1786510154_891564`, `finding_1786510154_856929`, `finding_1786510155_695961`, `finding_1786510155_612504` |
