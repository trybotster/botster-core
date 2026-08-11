# Implementation report: Race-free terminal mode revision for mode-dependent input

## Target repository and target_id
- Target repository: `botster-core` (`trybotster/botster-core`)
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Run: `run_1786479064_760292`
- Ticket: `ticket_1786478568_882200`
- Plan: Plan visit 6 (`docs/archive/plans/race-free-terminal-mode-revision.md`)
- Base SHA: `747be95b8922130d3e2c3f6844e3dbe1deeb2faa`
- `teardown_class_applies`: false

## Repository playbook and other playbooks/notes applied
- [[implementer-playbook]]
- [[botster-implementer-playbook]]
- [[botster-core-playbook]] (primary ownership charter)
- [[cli-patterns]], [[botster-architecture]]
- [[botster core contract surface needs consumer proof]]
- [[botster core public surface needs a narrow start here path]]
- [[public dto field additions are source breaking without non exhaustive]]
- [[scratch cargo patch redirects measure downstream dto breakage]]
- [[ghostty shadow terminal integration belongs outside botster core]]
- [[pinned libghostty exposes synchronous exact mouse mode state]]
- [[test script required for rust tests not cargo test]] (workspace uses `BOTSTER_ENV=test cargo test --workspace`)
- Human binding `question_1786481243_140177` (worker atomic admit is correctness boundary)
- Not loaded: [[project-pipelines-playbook]]; [[botster runtime teardown lenses]] (`teardown_class_applies=false`)

## Files changed
### Packaging / host
- `crates/botster-core/Cargo.toml` — removed `botster-session-worker` binary
- `crates/botster-core/src/bin/botster-session-worker.rs` — deleted (moved)
- `crates/botster-core-daemon/Cargo.toml` — hosts `[[bin]] botster-session-worker`
- `crates/botster-core-daemon/src/bin/botster-session-worker.rs` — Ghostty-hosted worker + atomic mode-gated admit

### Protocol / runtime / public surface
- `crates/botster-core/src/contract/session_protocol.rs` — `ModeFreshnessToken`, mode-gated request/result frames (`0x19`/`0x1a`), `ModeFlagsPayload`
- `crates/botster-core/src/contract/actor.rs` — `ModeFlagsReady.mode_freshness`
- `crates/botster-core/src/runtime/worker_process.rs` — correlated wait demux, timeout, fail-closed matrix, pending output buffer
- `crates/botster-core/src/engine/botster.rs` — worker-authoritative mode probe + `mode_gated_pty_input`
- `crates/botster-core/src/engine/managed_session_runtime.rs` — local token tracking on mode observation
- `crates/botster-core-daemon/src/daemon.rs` — `mode_gated_input`, timeout config, token cache for retained readback
- Exports updated in `lib.rs` / `contract/mod.rs` / `runtime/mod.rs` / daemon `lib.rs`

### Tests / docs / support
- daemon integration tests for admit/stale, race (b), post-drain hold, timeout, packaging
- protocol round-trip + frame constants
- test-support fake `ModeFlagsReady`
- README + durable-session worker protocol docs (binary provenance)

## Ownership boundaries preserved
- `botster-core` stays Ghostty-free (no `botster-terminal-ghostty` dependency; no session-worker bin)
- `botster-terminal-ghostty` remains concrete Ghostty adapter ownership
- `botster-core-daemon` hosts production `CoreDaemon` **and** the Ghostty-enabled `botster-session-worker` binary (cycle-free)
- Parent pre-admit drain is optimization only; worker atomic admit is correctness boundary
- Parent dual-shadow still owns screen/snapshot/OSC write_pty injection; worker Ghostty is mode-token/admit authority

## Cross-repo dependencies or separately routed work
- Hub ticket `ticket_1786471489_718500` depends on this Core contract (`ModeFreshnessToken` / mode-gated input)
- Hub product projection of `mode_gated_input` and client encoding against tokens is **out of scope** (separate Hub run)
- Scratch hub path-patch `cargo check -p botster-hub` against this Core revision: **pass** (Core SHA base `747be95b…`, Hub SHA `891cc796…`)
- Hub-shaped standalone DTO compile of `ModeFlagsReady` + `ModeFreshnessToken`: **pass**

## Deviations from plan
1. **No new `CoreDaemonError` variant** for mode-gated timeout/fail-closed. Failures map through existing `CoreDaemonError::Engine(...)` so Hub exhaustive matches on `CoreDaemonError` remain source-compatible without a Hub co-change in this ticket. Typed stale rejection is still a successful `ModeGatedInputOutcome::Gated { admitted: false, ... }`.
2. **Parent Ghostty dual-shadow retained** for screen/snapshot/OSC write_pty; worker Ghostty does not inject `drain_pty_writes` (avoids double OSC replies). Plan allowed dual-shadow risk; worker remains token/admit authority.
3. **Deterministic hold test seam**: `BOTSTER_SESSION_WORKER_HOLD_PTY_OUTPUT_MS` env var for post-parent-drain / pre-admit hold proof (test-only).
4. Adopt mid-wait fail-closed is covered by disconnect/timeout fail-closed path + existing adopt continuity of live worker generation; a dedicated adopt-mid-wait flaky integration test was not added beyond fail-closed matrix units.

## Tests and downstream proof run
Commands (with `PATH` including mise Zig 0.16.0; `BOTSTER_ENV=test`):
- `cargo fmt --all`
- `cargo clippy --workspace --all-targets -- -D warnings` — pass
- `cargo test --workspace` — pass
- Focused: `worker_backed_mode_gated_*` (admit/stale, race b, hold, timeout) — pass
- `worker_binary_is_hosted_by_daemon_package_not_core` — pass
- `worker_backed_mode_flags_include_kitty_and_mouse_from_ghostty_authority` — pass
- Scratch hub path-patch compile — pass
- Production path evidence: `CoreDaemon::mode_gated_input(Some(token))` → correlated `FRAME_MODE_GATED_PTY_INPUT` → worker atomic drain/compare/write-or-reject → `FRAME_MODE_GATED_PTY_INPUT_RESULT` (not parent-compare + plain `FRAME_PTY_INPUT`)

## Unverified behavior or residual risk
- Concurrent gated requests are parent-serialized (reject second while first in flight); worker processes one frame fully before the next (single-threaded loop)
- Dual Ghostty (parent screen vs worker modes) can theoretically diverge if parent and worker apply different VT streams; production path applies the same PTY output bytes to both
- `BOTSTER_SESSION_WORKER_HOLD_PTY_OUTPUT_MS` is process-global env; tests clear it, but parallel suites that also set it could interact
- Hub product clients are not yet encoding mode-dependent input against tokens (Hub ticket dependency)
- Reconnect/adopt mid-wait is fail-closed via disconnect/timeout; not a separate live-adopt chaos test

## Missing vault guidance discovered
1. Session worker binary that hosts Ghostty must not live in package `botster-core` (dependency cycle with `botster-terminal-ghostty`)
2. Mode-gated worker RPC needs correlation + bounded fail-closed wait (timeout, disconnect, exit, mismatched `request_id`)
3. Worker atomic Ghostty admit is correctness; parent drain is optimization only
4. Prefer not adding new exhaustive `CoreDaemonError` variants for additive fail-closed paths when Hub matches exhaustively; map through `Engine` unless Hub co-evolves

## Runtime-teardown class
- `teardown_class_applies=false` — no lens implementation required
