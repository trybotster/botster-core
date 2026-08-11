# Ghostty-only terminal runtime authority (Core)

Ticket: `ticket_1786471489_484901` — Make Botster Core the Ghostty-only terminal runtime authority
Run: `run_1786471511_632427`
Pipeline step: Implement (`botster_stack_implement`)
Status: implemented (awaiting Review)
Plan Review: `review_1786472662_676314` (approved cold-turkey Ghostty plan)

## Target

| Field | Value |
| --- | --- |
| Target repository | `botster-core` (`trybotster/botster-core`) |
| Target id | `tgt_1f7bce66eb304881980f9b4a2a5ae3fe` |
| Target path (spawn authority) | resolved from `target_id` by Botster |
| Pipeline worktree | Botster-managed ticket worktree for this run (do not use ambient checkout) |
| Workspace crates in scope | `botster-core`, `botster-core-daemon`, `botster-terminal-ghostty`, `botster-core-test-support` |
| Project | Botster Ghostty-only terminal cutover (`project_1786468118_227513`) |

## Plan Review corrections (this revision)

| Finding | Decision locked in this plan |
| --- | --- |
| `[product] Resolve the cold-turkey daemon feature cutover` | **Hard cutover:** `botster-core-daemon` always builds and constructs Ghostty only. Remove optional `ghostty-terminal` feature, remove `cfg(not(feature = "ghostty-terminal"))` constructors and plain snapshot lanes on the daemon, remove CI `cargo … -p botster-core-daemon --no-default-features` lanes. `PlainTerminalScreenRuntime` may remain only as a **botster-core library/test harness**, never as a daemon construction path. |
| `[product] Add real Kitty and mouse input production proof` | Worker-backed `CoreDaemon` tests must send **exact Kitty key and mouse input bytes** through `CoreDaemon::input` (production input path) and assert the **child PTY receives those exact bytes**. Mode-state reads remain additional, not sufficient. |
| `[product] Make downstream consumer proof exact and required` | Required, not optional. Exact proof named below. Hub ticket dependency already registered. |
| `[product] Include the Ghostty adapter README in documentation scope` | `crates/botster-terminal-ghostty/README.md` is in affected files; pin text must match submodule reality (`trybotster/ghostty` @ `5e9ba17a…`). |

## Repository playbook loaded

- [[botster-core-playbook]] — primary ownership charter for this target

## Other role / surface playbooks and atomic notes loaded

### Role / surface
- [[planner-playbook]]
- [[botster-planner-playbook]]
- [[botster-architecture]]
- [[cli-patterns]]
- [[botster-terminal-ghostty-playbook]] — concrete Ghostty adapter crate lives in this workspace; still a separate ownership charter

### Atomic notes (terminal authority seams)
- [[ghostty shadow terminal integration belongs outside botster core]]
- [[session-process-owns-vt-parser-hub-rpc-snapshots]]
- [[binary-page-snapshots-replace-vt-in-protocol]]
- [[coredaemon must expose terminal truth used by the production hub path]]
- [[pinned libghostty exposes synchronous exact mouse mode state]]
- [[libghostty-vt-embedder-callback-architecture-and-constraints]]
- [[synced state types are allowed while pushed event variants are forbidden]]
- [[split terminal runtimes drop color probe responses before client attachment]]
- [[botster core contract surface needs consumer proof]]
- [[restty is a client renderer not authoritative terminal infrastructure]]
- [[botster cli integration tests require ghostty submodule initialization]]
- [[botster durable terminal egress is owned by sessionio and clientworker actors]]
- [[initial terminal snapshots must precede live output activation]]
- [[botster clients restore visible terminal state from readscreen before buffered live output]]

### Explicitly not loaded
- [[project-pipelines-playbook]] — Project Pipelines package/plugin paths are out of scope
- [[botster runtime teardown lenses]] — teardown class does **not** apply (see below)

## Runtime-teardown class

| Field | Answer |
| --- | --- |
| `teardown_class_applies` | **false** |
| Reason | Ticket is cold-turkey production cutover to one Ghostty terminal authority (parser, snapshot, modes, palette, PTY query replies, production input routing). It is not WebRTC/peer lifecycle, SessionIo/ClientWorker multi-peer teardown, CPU/FD spin, or terminal-state vs live-runtime teardown remediation. One Plan → Implement path. |

## Context loaded

- Pipeline `project_pipelines_current_context`: Plan re-entered after Plan Review `changes_required` (`review_1786472386_402912`); four open product findings; prior plan artifact `artifact_1786471724_602392`.
- Target resolved: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe` → `botster-core`.
- Hub consumer dependency **already registered**: Hub ticket `ticket_1786471489_718500` (`depends_on` this Core ticket via `dependency_1786471500_696870`). Hub cannot start its cutover run until this ticket is closed.
- Submodule truth (filesystem): `crates/botster-terminal-ghostty/vendor/ghostty` → remote `https://github.com/trybotster/ghostty.git`, commit `5e9ba17a22ba8e40bf8de7d3e7555b8378cb1880`. Adapter README currently claims upstream `ghostty-org/ghostty` @ `22d13172…` and “no fork” — **stale; in scope to fix**.
- Repo evidence unchanged from first Plan visit: dual daemon feature path, partial `ModeFlags` (mouse only), no-op `set_color_profile`, missing `write_pty` for pre-attach OSC color replies, CI still has daemon `--no-default-features` lanes.
- Living docs home: `docs/architecture/`.

## Scope

Make Ghostty the **only** terminal runtime authority on every production path in this workspace. Session-side Ghostty owns terminal state, scrollback, Kitty keyboard mode, mouse mode, palette state, and PTY query replies. Production paths export authoritative `GHOSTSNP` snapshots and mode state. Production input routes Kitty and mouse bytes to the child PTY unchanged. Remove all daemon dual-path terminal selection and alternate production parser/snapshot formats.

### In scope

1. **Cold-turkey daemon feature cutover (locked)**
   - `botster-core-daemon` depends on `botster-terminal-ghostty` with `libghostty-vt` **non-optionally** (remove optional feature matrix for terminal backend selection).
   - Single daemon construction path: Ghostty factory only. Delete `cfg(not(feature = "ghostty-terminal"))` engine constructors and plain snapshot expectations on the daemon.
   - CI: remove `cargo clippy/test -p botster-core-daemon --no-default-features` jobs. Default workspace clippy/test remains the production proof lane (Zig 0.16.0 + submodule already required).
   - `botster-core` remains free of a Ghostty dependency. `PlainTerminalScreenRuntime` may remain for **library unit tests / fakes / contract harnesses only**; it must not be reachable from `CoreDaemon` construction.
   - Pure contract embeds that need daemon types without Zig: **out of product scope** for this cold-turkey ticket; do not reintroduce a second production runtime to preserve them. If a pure-contract compile surface is still needed later, it is a separate ticket with human authorization — not Implement discretion on this run.

2. **Authority completeness on Ghostty adapter + session path**
   - Complete production `ModeFlags` from Ghostty: `kitty_enabled`, `cursor_visible`, `bracketed_paste`, `mouse_mode` (existing u8 layout), `alt_screen`, `focus_reporting`, `application_cursor`. Prefer real queries; use fail-closed `TerminalBackendError::Unsupported` only when the pin truly cannot read a field — never silent `Default` zeros for claimed authority.
   - Own palette / color profile on the session Ghostty terminal (read + apply).
   - Own PTY query replies for OSC color probes (10/11/12) via Ghostty write/effects callback writing into the session PTY **before** any client attaches.
   - Mode/color remain **synced state** (probe/read + snapshot contents), not new pushed `ModeChanged` / color-delta events.

3. **Neutral core seams only where required**
   - Extend `TerminalScreenRuntime` / managed-session wiring only for complete mode flags, color profile apply/read, and PTY reply injection.
   - Do **not** move Ghostty FFI, Zig, or vendored source into `botster-core`.

4. **Production-path tests (required set)**
   - See Acceptance checks. Includes real **input** proofs, not only mode reads.

5. **Downstream-shaped consumer proof (required, exact)**
   - See Acceptance checks → Downstream consumer proof.

6. **Docs**
   - Root `README.md`, `docs/architecture/ghostty-shadow-terminal-adapter.md`, this plan, and **`crates/botster-terminal-ghostty/README.md`** (pin, remote, callback/palette/authority language).

### Non-scope

- Hub product policy, Hub client DTOs product surface, Web/TUI/Restty rendering (sibling tickets).
- Project Pipelines package/plugin work.
- Broad actor mailbox taxonomy, WebRTC transport, or Lua plugin refactors.
- Moving concrete Ghostty into `botster-core`.
- Multi-backend registry or optional “pluggable terminal engines.”
- Pushed terminal-mode event stream redesign.
- Re-adding a daemon plain/no-default production lane for CI convenience.

## Repository ownership boundaries and cross-repo dependencies

| Owner | Responsibility on this ticket |
| --- | --- |
| `botster-core` (this target) | Neutral contracts, managed session mechanisms, **single** Ghostty production daemon composition, production-path + consumer-shaped tests |
| `botster-terminal-ghostty` (same workspace) | libghostty-vt FFI, GHOSTSNP, modes/palette/query callbacks, accurate README pin |
| Hub (sibling `ticket_1786471489_718500`, target `tgt_7e208a0c76a44980a83b63af976b1f22`) | One client transport contract over Core authority — **depends on this ticket** (`dependency_1786471500_696870`); implement Hub only after Core closes/merges |
| Web / TUI / Restty | Thin clients; depend on Hub + Core — not this run |

### Registered dependency (record, do not recreate)

- **Hub → Core:** `ticket_1786471489_718500` depends on `ticket_1786471489_484901` (`dependency_1786471500_696870`, status open). This enforces “Hub consumes the merged Core artifact before its own cutover.”
- No reverse dependency (Core does not wait on Hub).

## Assumptions and unknowns

### Assumptions
- Cold-turkey means **zero** daemon plain runtime construction paths after this ticket.
- Package boundary remains: Ghostty concrete code stays in `botster-terminal-ghostty`.
- Mouse `u8` bit layout (1000→1, 1003→2, 1002→4, 1006→8) is load-bearing.
- Mode/color are probeable synced state, not pushed events.
- OSC color query replies are session-owned ([[split terminal runtimes drop color probe responses before client attachment]]).
- Raw `cargo` is the repo test idiom.
- Worktree path has no `:`; no special `CARGO_TARGET_DIR` required.

### Unknowns (Implement resolves without reopening product decisions)
- Exact libghostty-vt data ids / callbacks on pin `5e9ba17a…` for full `ModeFlags`, palette, and write_pty.
- Exact harness technique to observe **child PTY received input bytes** in worker-backed tests (e.g. echo-to-drain with known encoding, or worker-side tee) — must prove production `CoreDaemon::input` path, not a test-only bypass.

No blocking human question: Plan Review closed the discretionary feature-matrix gap.

## Affected surfaces / files (expected)

### `botster-terminal-ghostty`
- `crates/botster-terminal-ghostty/src/sys.rs`
- `crates/botster-terminal-ghostty/src/lib.rs`
- `crates/botster-terminal-ghostty/tests/native_runtime_test.rs` (+ focused native tests)
- **`crates/botster-terminal-ghostty/README.md`** — fix pin remote/commit, authority/callback/palette language
- `build.rs` / `build_support.rs` only if new native symbols require it

### `botster-core`
- `crates/botster-core/src/engine/terminal_screen.rs`
- `crates/botster-core/src/engine/managed_session_runtime.rs`
- `crates/botster-core/src/engine/botster.rs` — do not advertise plain as production
- Contracts only if additive fields are required for palette/mode export
- Related `crates/botster-core/tests/*`

### `botster-core-daemon` (hard cutover)
- `crates/botster-core-daemon/Cargo.toml` — **non-optional** Ghostty dependency; remove optional feature selection
- `crates/botster-core-daemon/src/daemon.rs` — single Ghostty construction path; delete plain cfg branches
- `crates/botster-core-daemon/tests/daemon_integration_test.rs` — production acceptance suite including Kitty/mouse **input** proofs; remove plain-format dual expectations

### `botster-core-test-support` (required consumer-shaped proof)
- `crates/botster-core-test-support/src/conformance/` (and/or `assertions/`) — hub-facing helpers/fixtures for mode + GHOSTSNP + palette export shapes
- `crates/botster-core-test-support/tests/downstream_conformance_test.rs` or a new focused terminal-authority conformance test file

### CI / docs
- `.github/workflows/ci.yml` — drop daemon `--no-default-features` lanes; keep Zig + submodule for default workspace
- `README.md`
- `docs/architecture/ghostty-shadow-terminal-adapter.md`
- This document

## Risks

| Risk | Mitigation |
| --- | --- |
| Incomplete ModeFlags silently defaulting | Per-field production tests after real VT; Unsupported over fake zeros |
| Mode-only tests mistaken for input proof | Separate PTY-received-byte assertions for Kitty and mouse input |
| PTY color replies reintroduce client authority | Session-side callback + pre-attach proof |
| Public contract bloat | Prefer existing `ModeFlags` / `TerminalColorProfile` / daemon read APIs; additive fields only with named consumer fixture |
| Breaking pure-contract daemon embeds | Accepted by cold-turkey; do not reintroduce dual path |
| Stale adapter README pin claims | Explicit README update in affected files |
| Cross-repo skew | Hub ticket already depends on this Core ticket |

## Acceptance checks / tests

### Workspace gates
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace` (Ghostty submodule + Zig 0.16.0 required)
- **Must not exist after this ticket:** `cargo test/clippy -p botster-core-daemon --no-default-features` as a supported product lane

### Production-path proof on default `CoreDaemon` + `with_worker_path`

1. **GHOSTSNP** — `capture_snapshot` bytes start with `GHOSTSNP`; format is Ghostty’s; round-trip preserves content.
2. **Scrollback** — history beyond viewport retained on late attach/reattach.
3. **Mouse mode state** — DECSET/DECRST → `read_mode_flags().mouse_mode` exact bitmask.
4. **Kitty keyboard mode state** — enable/disable → `kitty_enabled` true/false on production read path.
5. **Kitty input (new, required)** — through `CoreDaemon::input`, send exact Kitty keyboard-encoded key bytes (with Kitty mode enabled on the session). Assert the **child PTY receives those exact bytes** (not merely that mode flags flipped).
6. **Mouse input (new, required)** — through `CoreDaemon::input`, send exact SGR/mouse report bytes appropriate to the active mouse mode. Assert the **child PTY receives those exact bytes**.
7. **Palette / color** — Ghostty-owned color state visible via production read without client synthesis.
8. **OSC color query replies** — OSC 10/11/12 with **no client attached**; session generates PTY replies (observable on session path).
9. **Attach ordering** — initial snapshot/state before live `TerminalOutput`.
10. **Reconnect** — reattach gets Ghostty scrollback + modes; never `plain-opaque-v1`.
11. **Single construction path** — workspace/daemon source has no plain daemon backend constructor; default path never emits `plain-opaque-v1`.

### Downstream consumer proof (required — exact)

| Proof | Exact location / shape |
| --- | --- |
| Hub ordering | Existing ticket dependency `dependency_1786471500_696870` (Hub `ticket_1786471489_718500` → this Core ticket). Do not close Core without the production proofs below. |
| Hub-shaped public surface fixture | Add **`botster-core-test-support`** conformance assertion(s) that a Hub-shaped consumer can call for the **final exported** production shapes: (a) `ModeFlags` with real kitty + mouse bits populated from Ghostty authority, (b) `TerminalSnapshotPayload` / snapshot bytes with `GHOSTSNP` magic and Ghostty format label, (c) color/palette authority fields as exported on `TerminalScreenState` or daemon read APIs. Prefer extending `crates/botster-core-test-support/src/conformance/mod.rs` + a focused test under `crates/botster-core-test-support/tests/` named for terminal authority (e.g. `ghostty_terminal_authority_conformance_test.rs`). |
| Production entry point | Same shapes must be proven on **`CoreDaemon`** worker-backed APIs (`read_mode_flags`, `capture_snapshot`, `read_screen`, `input`) — not only adapter unit tests or library `DefaultBotsterEngine` alone. |
| Hub cutover rule | Hub Implement must consume the **merged** Core revision that contains these exports (enforced by ticket dependency). Documentation-only “downstream proof” is **not** acceptable for this ticket. |

### Explicit non-waivers
- Adapter unit tests alone ≠ production-path proof.
- Mode reads alone ≠ Kitty/mouse **input** proof.
- Optional “if public fields change” language is removed; consumer-shaped proof is always required for the authority export surface this ticket ships.
- Do not keep plain daemon path for CI green.

## Implementation sequence

1. Fix `botster-terminal-ghostty` authority (full modes, palette, write_pty replies) + **README pin truth**.
2. Wire managed session + daemon to those authorities.
3. **Hard-cutover** daemon Cargo/features/construction; purge plain cfg paths and dual-format tests.
4. Add worker-backed **input** tests (Kitty + mouse) and remaining production-path proofs.
5. Add **required** `botster-core-test-support` hub-shaped terminal-authority conformance test.
6. Update CI feature matrix; update README + architecture docs.
7. Run workspace fmt/clippy/test gates.

## Vault gaps worth capturing

- Session-side OSC color query ownership after write_pty lands (supersede [[split terminal runtimes drop color probe responses before client attachment]]).
- Production `ModeFlags` completeness matrix for pin `5e9ba17a…`.
- Cold-turkey: `botster-core-daemon` no longer offers a no-default plain terminal lane (durable feature-matrix decision).
- Adapter README pin drift (`trybotster/ghostty` vs claimed upstream) if not already captured.

## Product decision ledger

| Decision | Choice |
| --- | --- |
| Production terminal authority | Ghostty only |
| Daemon feature matrix | **No optional plain terminal path** — Ghostty always |
| Core vs adapter | Core = neutral seams; adapter = concrete Ghostty |
| Mode/color delivery | Synced probes + GHOSTSNP; no pushed mode events |
| Plain backend | Library/test harness in `botster-core` only |
| Kitty/mouse proof | Mode state **and** exact PTY input bytes via `CoreDaemon::input` |
| Downstream proof | Required hub-shaped conformance in `botster-core-test-support` + CoreDaemon production APIs; Hub dependency already registered |
| Client query answering | Forbidden as authority |
| Follow-up OK | Hub/Web/TUI/Restty consumer tickets after Core closes |
| Ask-human threshold | Only if pin cannot support required ModeFlags/query surface without product waiver |

---

Topics:
- [[botster-core-playbook]]
- [[botster-terminal-ghostty-playbook]]
- [[ghostty shadow terminal integration belongs outside botster core]]
- [[session-process-owns-vt-parser-hub-rpc-snapshots]]
- [[coredaemon must expose terminal truth used by the production hub path]]
- [[split terminal runtimes drop color probe responses before client attachment]]
- [[botster core contract surface needs consumer proof]]
