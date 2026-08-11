# Plan: Race-free terminal mode revision for mode-dependent input

**Ticket:** `ticket_1786478568_882200`  
**Run:** `run_1786479064_760292`  
**Plan visit:** 6 (cycle-free Ghostty worker host + correlated gated-input RPC)  
**Base SHA:** `747be95b8922130d3e2c3f6844e3dbe1deeb2faa`  
**Human binding:** `question_1786481243_140177` — worker atomic admit is correctness boundary  

Supersedes visits 1–5. Visit 5 correctly locked worker Ghostty admit but did not specify a **cycle-free Cargo host** or **correlated request/result failure bounds**.

## Findings addressed this visit

| Finding | Severity | Resolution |
| --- | --- | --- |
| Select a cycle-free host for the Ghostty worker | high | **Locked:** move `botster-session-worker` **binary target** into `botster-core-daemon` (already depends on `botster-core` + `botster-terminal-ghostty`). Preserve executable name and path contract. |
| Define worker request correlation and failure bound | medium | **Locked:** correlated `request_id`, parent demux of interleaved frames, fail-closed timeout/disconnect/exit/malformed/stale, tests listed below. |
| Prior product locks (token, complete ModeFlags, worker atomic admit, human Q&A) | — | **Retained** |

## Target

| Field | Value |
| --- | --- |
| `target_id` | `tgt_1f7bce66eb304881980f9b4a2a5ae3fe` |
| `target_repository` | `botster-core` |
| Playbook | [[botster-core-playbook]] |

## Playbooks / notes loaded

- [[planner-playbook]], [[botster-planner-playbook]], [[botster-core-playbook]], [[cli-patterns]], [[botster-architecture]]
- [[botster core contract surface needs consumer proof]]
- [[botster core public surface needs a narrow start here path]]
- [[botster core public enums are breaking until non exhaustive is decided]]
- [[public dto field additions are source breaking without non exhaustive]]
- [[scratch cargo patch redirects measure downstream dto breakage]]
- [[synced state types are allowed while pushed event variants are forbidden]]
- [[pinned libghostty exposes synchronous exact mouse mode state]]
- [[ghostty shadow terminal integration belongs outside botster core]]
- [[botster packages should enforce core hub cli plugin provider boundaries]]
- [[core daemon lifecycle metadata is registry backed restart state]]
- Human `question_1786481243_140177`

Not loaded: [[project-pipelines-playbook]], [[botster runtime teardown lenses]] (`teardown_class_applies=false`).

## Context

**Dependency cycle (exact):**
- `botster-session-worker` binary is currently `[[bin]]` of package **`botster-core`**.
- `botster-terminal-ghostty` depends on `botster-core` (`default-features = false`).
- Worker-local Ghostty requires the worker **host package** to depend on `botster-terminal-ghostty`.
- Therefore the worker binary **cannot** remain in package `botster-core` without a Cargo cycle.

**Existing host that already depends on both:** `botster-core-daemon` → `botster-core` + `botster-terminal-ghostty`.

**Worker path contract today:** `CoreDaemonConfig::with_worker_path` points at executable named `botster-session-worker` (tests/main resolve sibling binary). Name and restart/adoption semantics must be preserved.

**Protocol reality:** mode-gated admit is request/response while `FRAME_PTY_OUTPUT` and metadata can interleave; parent must demux without hanging or mis-associating replies.

## Product decision — locked (full stack)

### 1. Public contract

```text
ModeFreshnessToken {
  mode_generation: u64,  // high-entropy epoch of worker mode owner
  mode_revision:   u64,  // complete ModeFlags counter within epoch
}
```

- On `ModeFlagsReady` / `read_mode_flags` (worker-authoritative for production worker-backed path).
- Mode-dependent input: `expected_mode_freshness: Option<ModeFreshnessToken>`.
- Nested token preferred.
- Bump over **complete** production `ModeFlags`.
- No `ModeChanged` push events.
- `None` → plain `FRAME_PTY_INPUT` (unchanged).

### 2. Correctness boundary (human + visit 5)

**Session worker atomic mode-gated admit** against **worker-local Ghostty**:

1. Drain/apply all PTY output available before the barrier to worker Ghostty; update token; emit pre-barrier output frames.
2. Compare expected token to current worker Ghostty `ModeFreshnessToken`.
3. Mismatch → zero PTY input bytes; return current ModeFlags + token.
4. Match → write input **before** later terminal output.

Parent `drain_runtime_for_readback` is **optimization only**. Forbidden: parent compare then plain `FRAME_PTY_INPUT`.

Ghostty adapter stays in `botster-terminal-ghostty`; worker **hosts** it; core package does not take Ghostty as a library dependency.

### 3. Cycle-free Ghostty worker host (visit 6 lock)

**Chosen host:** package **`botster-core-daemon`**.

| Action | Detail |
| --- | --- |
| Move binary target | `[[bin]] name = "botster-session-worker"` lives in `crates/botster-core-daemon` (source path e.g. `src/bin/botster-session-worker.rs`, moved/adapted from `botster-core`) |
| Dependencies | Daemon package already has `botster-core` + `botster-terminal-ghostty` with `libghostty-vt` — **no cycle** |
| Executable name | Keep **`botster-session-worker`** (not rename to daemon) |
| Path contract | `CoreDaemonConfig::with_worker_path` / tests continue to resolve `botster-session-worker` next to daemon or via explicit path |
| Remove from core package | Delete `[[bin]] botster-session-worker` from `crates/botster-core/Cargo.toml` so core stays Ghostty-free |
| Packaging / CI | Workspace `cargo build` produces the binary from daemon package; update any tests that assumed `CARGO_BIN_EXE` from `botster-core`; document in README/architecture if binary provenance changes |
| Restart/adoption | Unchanged protocol + path semantics |

**Rejected:** adding `botster-terminal-ghostty` to package `botster-core` (cycle).  
**Rejected:** moving Ghostty FFI into `botster-core`.  
**Alternative allowed only if needed:** new workspace crate `botster-session-worker` depending on core + ghostty — larger than moving the bin into daemon; **do not** take this unless daemon packaging is proven wrong. Default is daemon-hosted binary.

### 4. Correlated mode-gated RPC + failure bounds (visit 6 lock)

#### Wire shape (illustrative names; Implement fixes ids)

**Request frame** (e.g. `FRAME_MODE_GATED_PTY_INPUT`):
```text
{
  request_id: RequestId,           // required correlation
  expected: ModeFreshnessToken,
  data: bytes
}
```

**Result frame** (e.g. `FRAME_MODE_GATED_PTY_INPUT_RESULT`):
```text
{
  request_id: RequestId,           // must match
  admitted: bool,
  mode_flags: ModeFlags,           // current after pre-barrier apply
  mode_freshness: ModeFreshnessToken,
  // optional: error_kind for protocol/runtime failure
}
```

#### Parent `WorkerProcessRuntime` routing while waiting

When a mode-gated request is in flight for session S:

1. Continue reading worker frames in order.
2. **Interleaved** `FRAME_PTY_OUTPUT` / metadata / process-exited / etc. are applied to the **normal** parent demux paths (shadow, pending drain, lifecycle) — do not drop them.
3. Only a **result frame with matching `request_id`** completes the wait.
4. **One outstanding mode-gated request per session** (serialize). A second concurrent gated request is rejected or queued behind the first — pick serialize-for-simplicity: **reject concurrent gated input** with a typed busy/in-flight error (fail closed, no silent reorder).

#### Fail-closed cases (must not hang; must not apply wrong result)

| Condition | Behavior |
| --- | --- |
| Matching result, `admitted=true` | Success; bytes already written in worker |
| Matching result, `admitted=false` | Typed stale-token error; surface returned modes/token; zero new parent writes |
| **Timeout** (explicit bound) | Fail closed; do not invent admit; do not send plain PTY input fallback |
| Worker disconnect / control socket death | Fail closed |
| Worker process exit before reply | Fail closed |
| Malformed result / missing fields | Fail closed |
| Result `request_id` ≠ outstanding | **Ignore as stale** (log/count); keep waiting until timeout or match — never complete wrong request |
| Result after wait already completed / no outstanding | Drop stale |
| Reconnect/adopt mid-wait | Fail closed in-flight wait; client must retry after re-probe (new outstanding request on new parent ownership) |

#### Timeout bound

- Explicit duration on the parent wait (config on `WorkerProcessRuntimeOptions` or `CoreDaemonConfig`, with a **documented default** suitable for tests and production, e.g. low seconds not unbounded).
- Tests must be able to use a short timeout.

#### Worker-side correlation

- Echo `request_id` on result.
- Process one gated admit fully before starting another for that session (same serialize rule).
- Never write input on mismatch.

### 5. Token epoch across adopt

Worker-owned token continues across **daemon adopt of the same live worker**. New generation only on new worker/session ownership. Overflow fail-closed for gated admit.

### 6. Source-evolution

Coordinated `0.1.0` additive fields; scratch hub `cargo check` with Core + Hub SHAs.

## Scope

**In:**
- All visit-5 product semantics (token, worker Ghostty admit, races, optional parent drain).
- **Move** `botster-session-worker` binary into `botster-core-daemon`; wire Ghostty there; remove bin from `botster-core`.
- Protocol: correlated mode-gated request/result; parent demux + fail-closed bounds.
- Tests: host path/binary provenance; interleaved output during wait; timeout; worker exit; stale reply; post-parent-drain pre-worker-admit hold; races (a)/(b); Kitty/mouse; adopt continuity; scratch hub; workspace CI.

**Out:**
- Hub product implement.
- ModeChanged events.
- Ghostty FFI inside `botster-core` package.
- Parent-only validation as correctness.
- New workspace crate unless daemon host proves unworkable (default: no new crate).

## Ownership boundaries

| Package | Role |
| --- | --- |
| `botster-core` | Contracts, frames, engine/runtime **library** (no Ghostty dep, no session-worker bin) |
| `botster-terminal-ghostty` | Concrete Ghostty adapter |
| `botster-core-daemon` | Production `CoreDaemon` **and** `botster-session-worker` binary host with Ghostty |
| Hub | Consumer projection (scratch compile only) |

## Assumptions / unknowns

### Assumptions
1. Hosting the worker binary in the daemon package is acceptable packaging (same workspace artifact set; path contract preserved).
2. Serialize one gated request per session is acceptable for this ticket.
3. Default timeout is finite and test-overridable.

### Locked
Worker atomic admit; complete ModeFlags; token shape; **daemon-hosted worker binary**; **correlated RPC + fail-closed bounds**.

### Non-blocking implement detail
- Exact frame numeric ids and serde layout.
- Default timeout value.
- Whether parent dual-shadow remains for screen/snapshot vs full worker proxy for mode read.

## Affected surfaces / files

### Packaging / host
- `crates/botster-core/Cargo.toml` — remove `botster-session-worker` bin
- `crates/botster-core/src/bin/botster-session-worker.rs` → move to daemon package
- `crates/botster-core-daemon/Cargo.toml` — `[[bin]] botster-session-worker`
- `crates/botster-core-daemon/src/bin/botster-session-worker.rs` — Ghostty host + atomic admit
- Tests using `env!("CARGO_BIN_EXE_botster-session-worker")` / package-local bin paths
- README / architecture notes on binary provenance if documented

### Protocol / runtime
- `crates/botster-core/src/contract/session_protocol.rs` — frame ids + payloads with `request_id`
- `crates/botster-core/src/runtime/worker_process.rs` — send gated request; demux interleaved frames; wait with timeout; fail-closed
- Actor/transport optional expected token
- `crates/botster-core-daemon/src/daemon.rs` / `api.rs` — mode-dependent input → gated RPC

### Tests / docs / support
- daemon integration tests (new cases below)
- test-support fakes
- architecture durable-session + core-daemon docs

## Risks

| Risk | Mitigation |
| --- | --- |
| Cargo cycle | Daemon-hosted binary; core stays Ghostty-free |
| Hang on missing reply | Explicit timeout + fail-closed |
| Wrong result association | `request_id` match required |
| Binary path breakage | Preserve name; fix CARGO_BIN_EXE package |
| Dual Ghostty divergence | Worker is token/admit authority |
| Hub DTO break | Scratch path-patch compile |

## Acceptance checks / tests

### Functional
1. Worker-authoritative `ModeFreshnessToken` on probe.
2. Kitty + mouse bump revision; no-op does not.
3. Matching gated admit writes; stale returns current state and zero PTY bytes.
4. Races (a) and (b) separate worker-backed proofs.
5. Post-parent-drain / pre-worker-admit hold-and-release → reject, zero stale PTY bytes.
6. Daemon adopt same live worker: token continuity.
7. Plain `None` path unchanged.

### Correlation / failure (new)
8. **Interleaved output:** PTY output frames during gated wait are delivered to parent demux; wait still completes on matching result.
9. **Timeout:** short bound → fail closed; no hang; no plain input fallback.
10. **Worker exit before reply:** fail closed.
11. **Stale/mismatched `request_id`:** does not complete wrong wait; eventual timeout or correct match only.
12. **Reconnect/adopt mid-wait:** in-flight wait fails closed.

### Packaging
13. `botster-session-worker` builds from **daemon** package with Ghostty; **no** `botster-core` → `botster-terminal-ghostty` dependency.
14. Daemon integration tests resolve worker binary successfully (path/name preserved).

### Downstream / CI
15. Scratch hub cargo check with SHAs.
16. `cargo fmt`, workspace clippy `-D warnings`, `cargo test --workspace`.
17. Production path evidence: CoreDaemon mode-dependent input → **correlated mode-gated worker RPC** → atomic worker sequence (not parent-compare + `FRAME_PTY_INPUT`).

## Implementation sequence

1. Move `botster-session-worker` bin to `botster-core-daemon`; fix path/tests; prove cycle-free build with Ghostty link.
2. Host Ghostty in worker; apply PTY output; maintain token.
3. Add correlated request/result frames.
4. Implement worker atomic admit handler.
5. Implement parent wait demux + timeout + fail-closed matrix.
6. Wire CoreDaemon mode-dependent input to gated RPC only.
7. Align probe with worker token authority.
8. Full test matrix (functional + correlation + packaging).
9. Docs; scratch hub; workspace CI.

## Vault gaps (post-implement)

1. Session worker binary that hosts Ghostty must not live in package `botster-core` (dependency cycle).
2. Mode-gated worker RPC needs correlation + bounded fail-closed wait.
3. Worker atomic Ghostty admit is correctness; parent drain is optimization.

## Pipeline / process

- Plan URI: `docs/archive/plans/race-free-terminal-mode-revision.md`
- Checklist: reuse `checklist_1786479306_238043`; skip duplicate `checklist_1786479320_587227`
- Gate evidence must include `plan_uri`, `artifact_id`, `checklist_id`, `target_id`, `target_repository`
- Hygiene: HEAD `747be95b`; `.gitignore` non-empty; path has no colon
- `teardown_class_applies=false`
