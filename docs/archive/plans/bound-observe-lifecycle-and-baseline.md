# Archived plan: bound observe and baseline by item, byte, and elapsed

Ticket: `ticket_1786690597_161141`
Target: `botster-core` / `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
Pipeline: `botster_stack_delivery`

This file is historical implementation intent. Living contract:
`docs/architecture/control-plane-lifecycle-journal.md`.

Review `review_1786693957_225629` changed three shipped rules from the
draft below:

- Resume identity is `(pass_id, last_visited)`, not pass id alone.
- A pass snapshots remaining live ids once. Later slices do not rescan
  or absorb mid-pass births. Elapsed starts at API entry.
- The Hub-shaped Stage A helper executes one observe slice per owner
  turn and returns the resume cursor to the caller.

# Control-plane lifecycle journal wake and page API

Historical plan for the Core control-plane lifecycle journal. Not living
architecture. See `docs/README.md`.

Revision 4 shipped `observe_lifecycle`, `take_journal_advanced_wake`, and
`lifecycle_changes_page`. That work made zero-client exit independent of
Hub terminal Drain. It still visits every live session and every baseline
row in one call.

**Revision 5 is the approved implementation contract for ticket
`ticket_1786690597_161141`.** It bounds observe and baseline by item,
encoded-byte, and elapsed budgets so Hub Stage A can keep owner-turn
budgets. Hub ticket `ticket_1786663582_169720` consumes this surface and
must not be implemented here. This revision includes Plan Review
corrections: pre-visit reserved-error admission
(`finding_1786691714_228412`) and JSON-safe reservation
(`finding_1786692121_415346`). Slice messages are sanitized to a
fixed alphabet so encoded size cannot exceed the reservation.

This ticket is **runtime-teardown class** because it changes the
production observe walk that discovers terminal-state versus live-runtime
facts. Session exit must still advance the journal without a terminal
client and without Hub terminal Drain. Slicing must not hide an exit or
present a partial suffix as a finished pass. [[botster runtime teardown
lenses]] applies. Required lens answers are in the Revision 5 section
and must appear in Plan gate evidence.

## Revision 5 — bound observe and baseline

### Target

- Target repository: `botster-core`
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Spawn-target name: `botster-core`
- Spawn-target path is the admitted `botster-core` target, not the
  ambient pipeline session directory.
- Subject revision at Plan: `a047574` on branch
  `project-pipelines/ticket_1786690597_161141`
- Repository playbook: [[botster-core-playbook]]
- Hub session-type eligibility parent: does not apply
- Project Pipelines package/plugin paths: out of scope

Resolved from `list_spawn_targets` via ticket `target_id`. Not inferred
from the process working directory.

### Playbooks and notes loaded

Role and overlay:

- [[planner-playbook]]
- [[botster-planner-playbook]]
- [[botster-core-playbook]]
- [[botster-architecture]]
- [[cli-patterns]] (mixed-generation index only; ownership from the charter)
- [[spa-patterns]] (loaded per planner overlay; no SPA surface in this ticket)
- [[botster runtime teardown lenses]]
- [[botster-runtime-reviewer-playbook]]
- [[botster-runtime-verifier-playbook]]
- [[project pipeline orchestration belongs in a device-level botster plugin]]
- [[project pipelines needs an operator workbench not more primitives]]
- [[project pipelines ui contract belongs in the plugin readme]]
- [[botster orchestration should spawn agents with explicit target ids]]
- [[botster orchestration prompts must bind agents to explicit worktrees]]
- [[plan steps need reviewable plan artifacts]]
- [[vault example paths are not repository placement conventions]]
- [[botster core historical plan docs should not sit beside living architecture]]
- [[plan agents must author vault context as wikilinks not home paths]]
- [[pipeline vault checklists must cite exact resolvable note titles]]
- [[pipeline artifacts should cite vault notes by wikilink not home path]]
- [[pipeline artifacts should use path neutral worktree references]]
- [[botster-core uses CI-owned Cargo commands because it has no test script]]
- [[prefer framework and library components over custom solutions]]
- [[colon worktree paths break cargo dyld library paths]]
- [[hearth gate runs require restoring a pipeline wiped gitignore before attribution]]
- [[cross repo dependency registration must use dependency repo target]]

Not loaded:

- [[project-pipelines-playbook]] — Project Pipelines package/plugin paths
  are out of scope.
- [[botster-hub-playbook]] / [[botster-hub-client-playbook]] /
  [[botster-web-playbook]] / [[botster-tui-playbook]] /
  [[botster-tui-kit-playbook]] / [[botster-terminal-ghostty-playbook]] —
  not the target repository charter. Cross-repo seams are named below
  without substituting those charters.

Targeted atomic notes:

- [[Core control-plane lifecycle journal advances without a terminal client or Hub terminal Drain]]
- [[lifecycle guards evaluated before the reconciling drain are one call stale]]
- [[botster core hosts need an explicit drain loop contract]]
- [[terminal subscription lifecycle is Core owned while host session policy is Hub owned]]
- [[botster core contract surface needs consumer proof]]
- [[botster core public surface needs a narrow start here path]]
  (read this Plan revisit; new DTOs stay on the daemon facade, not
  crate-root mechanism sprawl)
- [[botster core public enums are breaking until non exhaustive is decided]]
- [[public dto field additions are source breaking without non exhaustive]]
- [[Hub embeds CoreDaemon behind one client admission point]]
- [[Core terminal subscription ownership is session, subscription, and generation]]
- [[Core ClientWorker bind requires a live attach generation]]
  (read this Plan revisit; slicing must not change bind/generation
  admission)
- [[Core subscription hard-stop is synchronous close and drop on the host tick]]
- [[botster data plane bypasses the hub through session and client actors]]
- [[Hub synchronizes plugin workers with session lifecycle events and a baseline]]

Implicated by the charter but not independently re-read this visit
(constraints already applied via the charter and Revision 4):

- [[core daemon lifecycle metadata is registry backed restart state]]
- [[persisted core session metadata is revalidated against the current size cap]]
- [[proposed ProcessExited closes terminal subscriptions but not the host session]]

### Context loaded

Project `project_1786663508_823105` (`Botster Non-Blocking Event Plane`)
assigns Core the control-plane lifecycle journal, bounded journal pages,
and one coalesced wake. Hub later owns one canonical session projection
over those pages. Hub must never use terminal Drain, terminal silence,
terminal output, `ProcessExited` decoding, or attach state to infer
lifecycle.

Revision 4 (`5e1c1fa`) already shipped:

- `CoreDaemon::observe_lifecycle(now_seconds)` walks **every** live
  engine session in deterministic `SessionId` order, calls per-session
  `drain_runtime_once`, reconciles independently, retains incidental
  terminal egress on `pending_drain`, and returns
  `ObserveLifecycleResult { session_errors }`. It does not call
  `drain_runtime_all_once`.
- `take_journal_advanced_wake` and `lifecycle_changes_page` are the
  bounded consume path. Safe order is take, page until caught up or
  resync, take, re-page if woke.
- `lifecycle_baseline()` loads **every** registry row plus the journal
  watermark. `lifecycle_changes(after)` remains the unbounded
  compatibility reader.
- Isolated Hub-shaped consumer in
  `crates/botster-core-test-support/tests/consumers/hub-lifecycle-shaped`
  still calls unbounded `lifecycle_baseline()` to install a projection.
- Architecture test `lifecycle_api_types_are_control_plane_only` rejects
  terminal bodies on the lifecycle types in `api.rs`.

Hub Stage A (`ticket_1786663582_169720`) cannot keep owner-turn and
ready-operation budgets if one observe or baseline call still visits
every live session. That Hub ticket is the consumer. This run only
publishes the bounded Core surface.

### Scope

Surgical change on the shipped `CoreDaemon` lifecycle source:

1. Keep Core authoritative for lifecycle facts. Do not move Hub
   projection, host retention, worktree membership, or plugin policy
   into Core.
2. Add a sliced observe path that visits live sessions in deterministic
   `SessionId` order under `max_sessions`, `max_encoded_result_bytes`,
   and `max_elapsed`.
3. Return the last visited `SessionId`, whether this pass completed,
   retained per-session errors, and an explicit resync reason when the
   resume cursor cannot be honored.
4. A later observe call with that cursor resumes after the last visited
   id. Do not restart the full walk unless the caller requests a new
   pass (`resume = None`).
5. Do not call `drain_runtime_all_once`. Keep per-session
   `observe_session` / `drain_runtime_once`. Keep incidental terminal
   egress on the pending-drain path. Return no terminal bytes, phases,
   snapshots, attach state, or `ProcessExited` frames.
6. Add a paged `lifecycle_baseline` that returns one snapshot sequence,
   bounded rows, bounded encoded bytes, next row cursor, and an explicit
   `complete` flag. Freeze the row set at snapshot mint so later pages
   at that sequence reconstruct today's full baseline. An incomplete
   page must have `complete = false` and must not be usable as finished
   ended evidence.
7. Keep existing `observe_lifecycle` and `lifecycle_baseline` as
   compatibility wrappers that perform one unbounded pass / one full
   snapshot. Document the unbounded cost in rustdoc. Hub Stage A and
   the isolated Hub-shaped consumer must call the bounded forms.
8. Keep `take_journal_advanced_wake` and `lifecycle_changes_page`
   unchanged except snapshot-sequence glue: the paged baseline snapshot
   identity is the journal watermark (`SessionLifecycleCursor`) at mint
   time so Hub can apply `lifecycle_changes_page` after `snapshot_end`.

### Non-scope

- Hub projection, host-bridge fulfillment, plugin session-family
  delivery, `snapshot_begin` / `snapshot_chunk` / `snapshot_end` Hub
  frames, or Workspaces cleanup.
- ClientWorker, terminal adapters, terminal protocol, attach, or Drain
  semantics, except preserving pending-drain retention.
- Changing `lifecycle_changes` field layout or the wake bit contract.
- Project Pipelines package/plugin work.
- Inventing a repository test wrapper.

### API contract

Publish new types next to the existing lifecycle types in `api.rs` so
the control-plane architecture scan covers them. Do not add fields to
`ObserveLifecycleResult`, `SessionLifecycleBaseline`, or
`SessionLifecycleChanges` ([[public dto field additions are source
breaking without non exhaustive]]).

Sliced observe:

```text
observe_lifecycle_slice(
    now_seconds,
    resume: Option<&ObserveLifecycleCursor>,
    budget: ObserveLifecycleBudget { max_sessions, max_encoded_result_bytes, max_elapsed },
) -> Result<ObserveLifecycleSlice, SessionLifecyclePageError>
```

Reuse `SessionLifecyclePageError::BudgetTooSmall` for an undersized
successful-slice envelope. Do not invent a second page-error enum.

The walk has two representations of the same outcomes:

1. **Typed internal outcome** (not a public DTO):
   `last_visited`, `complete`, `resync_required`, and
   `Vec<ObserveLifecycleSessionError>` with the existing typed
   `CoreDaemonError`. `observe_session` writes only into this
   structure. The compatibility wrapper returns it as
   `ObserveLifecycleResult`. It never reconstructs a
   `CoreDaemonError` from a string.
2. **Public slice DTO** derived from that outcome after the walk:
   serializable `{ session_id, message }` errors. `message` is
   **not** raw UTF-8 truncation. Every public message is passed
   through `sanitize_observe_slice_error_message` before it is
   stored or encoded. Sanitization is lossy for Hub logs only. It
   is not the wrapper's error type.

Sanitize rule (finding `finding_1786692121_415346`):

A 256-byte UTF-8 cap does not bound `serde_json` size. Quotes,
backslashes, and control bytes expand (`"` → `\"`, NUL →
`\u0000`, 256 NULs → 1,538 JSON bytes). The reservation is
therefore defined in **encoded JSON bytes**, not raw message
bytes.

- Safe alphabet: ASCII
  `A-Z a-z 0-9` space `.` `:` `_` `/` `+` `-`.
  No `"`, `\`, controls, or multibyte code points.
- Map each input **byte**: keep it if it is in the alphabet,
  otherwise replace it with `?`. Take at most
  `OBSERVE_LIFECYCLE_SLICE_MAX_ERROR_MESSAGE_BYTES` (256) bytes.
  The result is always 0..=256 bytes of the safe alphabet.
- `reserved_error(session_id)` uses that same `session_id` and a
  message of exactly 256 `x` bytes. Because `x` is in the
  alphabet, `serde_json` encodes the message as a 258-byte JSON
  string (`"` + 256 + `"`). Every sanitized actual message encodes
  as a JSON string of length `2 + actual.len()` with
  `actual.len() <= 256`, so
  `encoded(actual_error) <= encoded(reserved_error)` for the same
  `session_id`.
- Do not put unsanitized `Display` text on the slice DTO. The
  typed wrapper still keeps the original `CoreDaemonError`.

Pre-visit byte admission (finding `finding_1786691714_228412`):

`observe_session` mutates the journal and `pending_drain`. Core
cannot roll back a visit. Therefore the byte budget is decided
**before** the visit, using `reserved_error`, not the unknown
post-visit message.

- After cursor validation, if there is at least one remaining live
  session, `minimum_bytes` is the `serde_json` length of a
  successful slice that contains the current pass/cursor,
  `last_visited = next_session`, `complete` as it would be if that
  visit finished the pass, and `session_errors = [reserved_error
  (next_session)]`. If `max_encoded_result_bytes < minimum_bytes`,
  return `Err(BudgetTooSmall { minimum_bytes })` and visit nothing.
- If there are no remaining live sessions, `minimum_bytes` is the
  empty successful slice (`complete = true` when the pass is
  already done).
- Before each later visit of session `S`, build that same
  reserved-error candidate on top of the **already committed**
  slice (actual sanitized errors from prior visits in this call,
  never reserved placeholders). Visit `S` only when
  `encoded(candidate) <= max_encoded_result_bytes`, remaining
  `max_sessions > 0`, and elapsed has not expired.
- After a visit, replace the reservation with the typed outcome:
  on success, no slice error row; on failure, the sanitized
  message. The committed encoding is always `<=` the reserved
  candidate, so a successful slice always fits.
- Never drop a retained error to make the encoding fit. If the
  reserved candidate does not fit, do not visit.
- Zero item or zero elapsed still visit no remaining session.
  Zero byte budget fails `BudgetTooSmall` when any session remains.
- Resync outcomes are control results and are not required to
  satisfy the byte budget, matching `lifecycle_changes_page`.

Other slice rules:

- `resume = None` mints a new pass id and starts at the first live
  `SessionId`.
- `resume = Some(cursor)` continues that pass only when
  `cursor.pass_id` matches the daemon's open pass. Otherwise return
  `resync_required` with empty progress and `complete = false`.
  Never present a guessed suffix as `complete = true`.
- After a visit, `last_visited` is that `SessionId` even if the
  session then exits or is absent from the next live list.
- `complete = true` only when this pass has attempted every live
  session that existed in deterministic order from the pass start
  (sessions that appear after the start are included only if they
  sort after `last_visited`; do not go back). After `complete`, the
  next useful call is a new pass.
- If `last_visited` is no longer live, resume at the first live id
  strictly greater than it. That is still the same pass.
- Per-session drain errors stay retained on the typed outcome and
  on the slice DTO. A sibling later in the same slice still runs.
- Elapsed is a host-tick yield bound, not session policy. Core may
  read `Instant` only to honor `max_elapsed`. Do not use
  `now_seconds` as the elapsed clock.

`observe_lifecycle(now_seconds)` becomes a wrapper over the same
typed walk: new pass, unbounded item/byte/elapsed budgets, return
`ObserveLifecycleResult` from the typed internal errors. Existing
full-pass tests keep working. Do not serialize and re-parse.

Paged baseline:

```text
lifecycle_baseline_page(
    snapshot: Option<&SessionLifecycleCursor>,
    after: Option<&SessionId>,
    max_rows,
    max_bytes,
) -> Result<SessionLifecycleBaselinePage, SessionLifecyclePageError>
```

- `snapshot = None` mints a frozen snapshot: `load_all()`, sort by
  `SessionId`, record the current journal watermark as
  `snapshot_sequence`, retain the row vec in daemon memory.
- Later pages with that snapshot cursor return the next rows from the
  freeze. They must not re-read a mutated registry. Hub Stage A
  interleaves observe slices between baseline pages; observe may append
  journal changes. Those changes are live deltas after the snapshot
  watermark, not mutations of the frozen rows.
- `complete = true` only on the page that includes the last frozen
  row, or on an empty snapshot that has no rows. Every earlier page
  has `complete = false`.
- `next` is the next `SessionId` to request, or `None` when complete.
- Reuse `SessionLifecyclePageError::BudgetTooSmall` for an undersized
  successful-page envelope. Reuse `SessionLifecycleResyncReason` for a
  dropped or foreign snapshot (`SourceChanged` when `source_id`
  mismatches). Add `#[non_exhaustive]` variant
  `SnapshotUnavailable` only if SourceChanged/CursorExpired cannot
  name a dropped in-process freeze without lying. Downstream matches
  already wildcard unknown reasons.
- Unknown snapshot + caller did not request a new snapshot: resync,
  `complete = false`, no rows. Recovery is `snapshot = None`.
- One cached freeze at a time. A new snapshot request replaces an
  incomplete freeze. Drop the freeze after a complete page.
- `lifecycle_baseline()` stays the full one-shot reader and must equal
  the concatenation of pages at the snapshot minted from the same
  `load_all()`.

Method is `&mut self` because it owns the freeze. That is new surface,
not a change to the existing `&self` wrapper.

### Production path

Hub (later) and the isolated consumer (this ticket):

1. `observe_lifecycle_slice` until `complete` or budget yield.
2. `take_journal_advanced_wake`.
3. `lifecycle_baseline_page` until `complete` when installing or
   resyncing a projection. Treat `complete = false` as not finished
   ended evidence.
4. `lifecycle_changes_page` after the snapshot watermark.
5. Take again; re-page if woke.

Zero-client natural exit still uses observe (sliced or wrapper) plus a
bounded journal page. No `CoreDaemon::drain`. No terminal client.

### Runtime-teardown lens answers

| Field | Answer |
| --- | --- |
| `teardown_class_applies` | yes — this ticket changes the production observe walk that reconciles SessionIo/ClientWorker exit into the control-plane journal; terminal-state vs live-runtime divergence remains the defect class |
| `teardown_isolation` | One session observe updates only that session's journal row and may hard-stop only that session's terminal subscriptions. A per-session drain error is retained and does not stop later ids in the same slice. Unvisited sessions in this pass are simply not yet observed; they stay live. The wake bit stays process-wide. |
| `teardown_bounds` | One slice is one non-blocking host tick bounded by item, encoded-result, and elapsed budgets. It must not `block_on(close)`, wait for PTY death, or call `drain_runtime_all_once`. Existing ClientWorker hard-stop remains synchronous close+drop on the tick that observes `ProcessExited` for a visited bound subscription. An unvisited exiting session waits for a later slice or a new pass. Page/baseline reads do not block. |
| `late_message_matrix` | Revision 4 matrix plus the rows below. |
| `production_path_proof` | Worker-backed or local self-exit, zero attaches, no `CoreDaemon::drain`. Host runs sliced observe (N, yield, resume) until a later slice publishes `Exited`, then a bounded journal page shows that upsert. Dropped observe cursor returns resync or requires a new pass; a suffix walk must not report `complete`. Paged baseline at one snapshot reconstructs `lifecycle_baseline()` when read to `complete`. Red-on-revert: switching the slice to `drain_runtime_all_once`, restarting the walk on resume, or marking an incomplete baseline `complete` fails those tests. |
| `ownership_identity` | Observe pass identity is an opaque `ObserveLifecyclePassId` plus `last_visited SessionId`. Journal identity remains `(source_id, sequence)`. Baseline snapshot identity is that journal cursor at mint. Terminal subscription identity remains `(session_id, subscription_id, generation)`. Delayed Drain or a late `ProcessExited` must not append a second identical `Exited` upsert. A stale observe pass id must not complete a different pass. |
| `sibling_fail_closed_policy` | Success: siblings keep running. A retained observe error does not fail the daemon or unvisited siblings. An incomplete slice is not a sibling sacrifice. Ultimate wrapper/slice failure does not kill sibling sessions. |

Late-message matrix additions:

| Message | Tag / owner | After terminal failure / exit | Residual sweep |
| --- | --- | --- | --- |
| `observe_lifecycle_slice` | `(pass_id, last_visited)` | stale pass → resync, `complete = false`; per-session drain errors retained | later ids in the slice still run; unvisited ids wait |
| `observe_lifecycle` wrapper | new unbounded pass | same per-session retain as today | full remaining live set |
| `lifecycle_baseline_page` | snapshot `SessionLifecycleCursor` | unknown/dropped snapshot → resync, `complete = false` | recover with `snapshot = None` |
| Spawn / Attach / Bind / Drain / Input / `remove_session` / `lifecycle_changes_page` | unchanged from Revision 4 | unchanged | unchanged |

### Repository ownership and cross-repo seams

- Core owns the sliced observe walk, the frozen baseline snapshot, and
  the control-plane DTOs.
- Hub owns projection, host retention, plugin session-family
  publication, and Stage A slice scheduling. Register Hub
  `ticket_1786663582_169720` as depending on this ticket / this
  `target_id`. Do not implement Hub here.
- Web and TUI consume Hub entity/state later. Out of scope.
- Isolated Hub-shaped consumer in `botster-core-test-support` is the
  required downstream-shaped proof
  ([[botster core contract surface needs consumer proof]]).

### Assumptions and unknowns

- Assumed: Hub Stage A will call `resume = None` to start a pass and
  pass the returned cursor to continue. That matches "caller requests
  a new pass."
- Assumed: `max_elapsed` uses `std::time::Instant` as a yield bound.
  Tests prove the elapsed stop with `Duration::ZERO` (visits none
  remaining) rather than a flaky sleep. If Plan Review requires a
  injected clock, add a test-only deadline override on
  `CoreDaemonConfig` without making production policy clocked.
- Assumed: freezing one baseline snapshot in daemon memory is
  acceptable. Hundreds of sessions is the same working set as today's
  full `lifecycle_baseline()`.
- Assumed: adding `SnapshotUnavailable` to the existing
  `#[non_exhaustive]` resync enum is allowed glue if SourceChanged
  cannot describe a dropped freeze.
- Unknown until Implement measures encodings: exact
  reserved-error `minimum_bytes` (256 `x` plus the next
  `session_id` JSON) and empty-baseline-page `minimum_bytes`.
  Tests compute them from `reserved_error` the same way
  `lifecycle_changes_page` already does.
- Not assumed: Hub in-tree compilation against this worktree. Isolated
  consumer is the in-repo proof. Live Hub pin remains Hub's ticket.

### Affected surfaces

- `crates/botster-core-daemon/src/api.rs` — new slice/page DTOs,
  budget type, pass/cursor types, optional resync variant.
- `crates/botster-core-daemon/src/daemon.rs` — slice walk, pass state,
  frozen baseline, wrappers, rustdoc unbounded-cost warnings.
- `crates/botster-core-daemon/src/lib.rs` — re-exports.
- `crates/botster-core-daemon/tests/daemon_integration_test.rs` —
  slice budgets, resume, dropped cursor, paged baseline reconstruct,
  incomplete `complete = false`, existing zero-client and sibling
  tests still pass through the wrapper.
- `crates/botster-core-test-support/tests/consumers/hub-lifecycle-shaped`
  and `lifecycle_journal_consumer_test.rs` — Stage A uses sliced
  observe + paged baseline; must not be forced through unbounded
  forms.
- `docs/architecture/control-plane-lifecycle-journal.md` (this file),
  `docs/architecture/core-daemon.md`, root `README.md` host-loop
  section.

Likely untouched: ClientWorker, terminal adapters, terminal protocol,
plugin admission, Hub/Web/TUI/Workspaces trees.

### Risks

- Resume without a pass id would let a dropped cursor walk a suffix
  and report `complete`. Pass identity is required.
- Re-reading the registry per baseline page would mix post-snapshot
  observe mutations into a snapshot that Hub treats as one sequence.
  Freeze is required.
- Post-visit encoding of an unbounded error cannot be the admission
  oracle: `observe_session` already mutated state. Reserved-error
  admission before the visit is required. A raw UTF-8 cap is not
  an encoded-size cap; sanitize to the safe alphabet so JSON
  escaping cannot blow the reservation. Sanitized slice messages
  must not become the wrapper's `CoreDaemonError`.
- Using `Instant` contradicts "Core does not call wall clocks for
  policy." Document elapsed as a yield bound, not policy. Do not
  drive lifecycle state from it.
- Compatibility wrappers that stay cheap-to-call can keep Hub on the
  unbounded path. Isolated consumer and rustdoc must make the bounded
  path the Stage A entry. Do not delete the wrappers.
- Slicing can delay `Exited` for an unvisited session until a later
  slice. That is the product trade. Tests must show resume eventually
  publishes it without Drain.

### Acceptance checks and tests

Focused during development (no wrapper script):

- Resume: observe `max_sessions = N` over `> N` live sessions, yield,
  resume the same pass, assert the second slice does not re-visit the
  first N ids, and a later slice reports `complete`.
- Each budget stops remaining visits: item count, reserved-error
  encoded bytes, and `max_elapsed = Duration::ZERO`.
- Long-error and JSON-escape boundary: inject drain errors whose
  `Display` text is (a) longer than 256 bytes, (b) 256 NULs, (c)
  quotes and backslashes, (d) control bytes, and (e) multibyte
  UTF-8. For each case, `encoded(actual_error) <=
  encoded(reserved_error(session_id))`, the slice message contains
  only the safe alphabet and is at most 256 bytes, the full slice
  encodes `<= max_encoded_result_bytes`, and the wrapper still
  returns the original typed `CoreDaemonError`. A budget that fits
  the empty envelope but not `reserved_error(next_session)` returns
  `BudgetTooSmall` and visits nothing. After one committed error, a
  remaining budget too small for another reserved error does not
  visit the sibling; the sibling still exits on the next slice. No
  retained error is dropped.
- Dropped / foreign pass cursor: `complete = false`, explicit resync,
  no suffix presented as complete.
- New pass (`resume = None`) after a partial pass restarts from the
  first live id.
- Paged baseline pages at one snapshot reconstruct
  `lifecycle_baseline()` row set and order when read until
  `complete = true`.
- Every incomplete page has `complete = false`.
- Observe between baseline pages does not change already-minted
  snapshot rows.
- Zero-client natural exit: sliced observe (not Drain, not attach)
  plus `lifecycle_changes_page` shows `Exited`. Wrapper
  `observe_lifecycle` still satisfies the existing full-pass tests.
- Architecture scan still rejects terminal bodies on lifecycle types,
  including the new slice/page DTOs.
- Isolated Hub-shaped consumer compiles against slice + paged
  baseline + wake + `lifecycle_changes_page`, never calls Drain or
  the unbounded baseline/observe forms on the Stage A path, and
  wildcards unknown page/resync variants.

Repository gates ([[botster-core uses CI-owned Cargo commands because
it has no test script]]):

```sh
BOTSTER_ENV=test cargo test --workspace
cargo fmt --all -- --check
BOTSTER_ENV=test cargo clippy --workspace --all-targets -- -D warnings
BOTSTER_ENV=test cargo test --doc --workspace
```

Worktree hygiene: this ticket worktree path has no `:`. Tracked
`.gitignore` is present and matches HEAD; restore from HEAD if a later
step wipes it. Never truncate it.

Delivery: direct-merge into `main`. No pull request.

### Vault gaps

- [[Core control-plane lifecycle journal advances without a terminal client or Hub terminal Drain]]
  still says one observe call visits every live session. After this
  lands, capture that Stage A uses the sliced form and that the
  wrapper is the unbounded compatibility path.
- [[botster core hosts need an explicit drain loop contract]] should
  mention observe/baseline budgets as part of the host loop.
- Do not rewrite those notes in this Plan visit.

## Revision 4 — shipped wake and page API

Revision 4 remains the parent contract for wake, page, resync, and
zero-client observe-without-Drain. The text below is the shipped
Revision 4 plan, kept as historical contract for those surfaces.

## Target

- Target repository: `botster-core`
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Spawn-target name: `botster-core`
- Spawn-target path is the admitted `botster-core` target, not the ambient
  pipeline session directory.
- Current subject revision: `0f89b01` on branch
  `project-pipelines/ticket_1786663581_962361`
- Addresses open finding `finding_1786683950_438902`
- Repository playbook: [[botster-core-playbook]]
- Hub session-type eligibility parent: does not apply
- Project Pipelines package/plugin paths: out of scope

Resolved from `list_spawn_targets` via ticket `target_id`. Not inferred from
the process working directory.

## Playbooks and notes loaded

Role and overlay:

- [[planner-playbook]]
- [[botster-planner-playbook]]
- [[botster-core-playbook]]
- [[botster-architecture]]
- [[cli-patterns]] (mixed-generation index only; ownership from the charter)
- [[spa-patterns]] (loaded per planner overlay; no SPA surface in this ticket)
- [[botster runtime teardown lenses]]
- [[botster-runtime-reviewer-playbook]]
- [[botster-runtime-verifier-playbook]]
- [[project pipeline orchestration belongs in a device-level botster plugin]]
- [[project pipelines needs an operator workbench not more primitives]]
- [[project pipelines ui contract belongs in the plugin readme]]
- [[botster orchestration should spawn agents with explicit target ids]]
- [[botster orchestration prompts must bind agents to explicit worktrees]]
- [[plan steps need reviewable plan artifacts]]
- [[vault example paths are not repository placement conventions]]
- [[botster core historical plan docs should not sit beside living architecture]]
- [[plan agents must author vault context as wikilinks not home paths]]
- [[pipeline vault checklists must cite exact resolvable note titles]]
- [[pipeline artifacts should cite vault notes by wikilink not home path]]
- [[pipeline artifacts should use path neutral worktree references]]
- [[botster-core uses CI-owned Cargo commands because it has no test script]]
- [[prefer framework and library components over custom solutions]]
- [[colon worktree paths break cargo dyld library paths]]
- [[hearth gate runs require restoring a pipeline wiped gitignore before attribution]]

Not loaded:

- [[project-pipelines-playbook]] — Project Pipelines package/plugin paths
  are out of scope.
- [[botster-hub-playbook]] / [[botster-hub-client-playbook]] /
  [[botster-web-playbook]] / [[botster-tui-playbook]] /
  [[botster-tui-kit-playbook]] / [[botster-terminal-ghostty-playbook]] —
  not the target repository charter. Cross-repo seams are named below
  without substituting those charters.

Targeted atomic notes:

- [[hub drain advances non attached session lifecycle]]
- [[lifecycle guards evaluated before the reconciling drain are one call stale]]
- [[botster core hosts need an explicit drain loop contract]]
- [[proposed ProcessExited closes terminal subscriptions but not the host session]]
- [[proposed transport lifecycle lets control connections outlive terminal subscriptions]]
- [[transport ownership north star for modular Botster is proposed]]
- [[terminal subscription lifecycle is Core owned while host session policy is Hub owned]]
- [[public protocol versions host control and Core terminal planes independently]]
- [[botster core contract surface needs consumer proof]]
- [[botster core public surface needs a narrow start here path]]
- [[botster core public enums are breaking until non exhaustive is decided]]
- [[public dto field additions are source breaking without non exhaustive]]
- [[core daemon lifecycle metadata is registry backed restart state]]
- [[persisted core session metadata is revalidated against the current size cap]]
- [[botster engine command surface uses botsterengine as facade]]
- [[Hub embeds CoreDaemon behind one client admission point]]
- [[Core terminal subscription ownership is session, subscription, and generation]]
- [[Core ClientWorker bind requires a live attach generation]]
- [[Core subscription hard-stop is synchronous close and drop on the host tick]]
- [[sessionio hard socket death must fan process exited to clientworkers]]
- [[botster durable terminal egress is owned by sessionio and clientworker actors]]
- [[botster data plane bypasses the hub through session and client actors]]
- [[Hub synchronizes plugin workers with session lifecycle events and a baseline]]
- [[Core class-aware plugin admission reserves request-response executors]]
  (sibling Event Plane work; do not change here)

## Context loaded

### Ticket and project

Project `project_1786663508_823105` (`Botster Non-Blocking Event Plane`)
assigns Core the control-plane session lifecycle journal, bounded journal
pages, and one coalesced lifecycle wake. Hub later owns one canonical
session projection over those pages. Hub must never use terminal Drain,
terminal silence, terminal output, ProcessExited decoding, or attach state
to infer lifecycle.

This ticket's closed parent is
`ticket_1786661004_845807` (`Core: push ClientWorker terminal egress and
own terminal subscription teardown`). That work shipped adapter-bound
ClientWorker push, ProcessExited-as-final-terminal-frame, and subscription
hard-stop. It did not decouple control-plane journal progress from
`CoreDaemon::drain`.

Hub consumer ticket `ticket_1786663582_169720` already depends on this
ticket. Do not implement Hub projection, package events, or client event
subscriptions here.

### Current Core code

`CoreDaemon` already has an in-memory lifecycle journal:

- `lifecycle_baseline()` returns registry rows plus a source-generation
  watermark.
- `lifecycle_changes(after)` returns every retained change after a cursor,
  or an empty list with `source_changed`, `cursor_expired`, or
  `cursor_ahead`.
- Journal capacity defaults to 1,024. Duplicate repeated observations do
  not append.
- `SessionLifecycleChange` is already control-plane-only: upsert/remove of
  `DaemonSession` + host metadata + optional `SessionLifecycleState`.

The journal does **not** yet have:

- a coalesced journal-advanced wake bit
- `lifecycle_changes_page(after, max_changes, max_bytes)`
- a next-cursor-plus-watermark page type
- a control-plane progress path that can observe exit without
  `CoreDaemon::drain`

Today `append_lifecycle_upsert` runs from spawn, adoption, resize persist,
shutdown, explicit remove, and `reconcile_lifecycle_observations`.
Reconciliation of natural process exit is reached only from
`CoreDaemon::drain` / `drain_subscription` and shutdown. The existing
worker-backed proof
`worker_backed_lifecycle_source_drives_projection_through_exit_and_removal`
attaches a client and loops `drain` until the journal shows `Exited`.

Engine lifecycle itself still advances when `drain_runtime_once` routes
`SessionIoEvent::ProcessExited` through `apply_session_event_activity`.
`drain_runtime_once` also calls `apply_client_worker`, which delivers
terminal-plane `ProcessExited` and then hard-stops bound subscriptions.
Those facts already exist. The missing production entry is a daemon method
that observes them without returning terminal Drain results to the host.

Current docs still say lifecycle consumption does not advance runtimes and
that the host must drain first. That sentence is the defect this ticket
removes for control-plane observation. Page/baseline reads stay
side-effect-free.

Hub already forwards `lifecycle_baseline` / `lifecycle_changes` and seeds
entity reconciliation from the baseline. Hub still discovers natural exit
by draining. This run does not change Hub.

### Convention conflict

[[botster-runtime-reviewer-playbook]] still says no-Attach fast-exit proof
must use `Drain` to emit the exact exited lifecycle event before
`ListSessions` reports exited. [[hub drain advances non attached session
lifecycle]] records that same historical daemon contract.

This ticket and the Event Plane project charter supersede that Drain
requirement. Implement must not keep Drain as the required no-Attach
progress oracle. After this change lands, those two notes become capture
candidates (see Vault gaps). Do not rewrite the vault in this Plan visit.

## Scope

Surgical change on the existing `CoreDaemon` lifecycle source:

1. Keep Core authoritative for session and process lifecycle. Do not move
   host retention, worktree, or plugin policy into Core.
2. Add `CoreDaemon::observe_lifecycle(now_seconds)` as the control-plane
   progress tick. One call is one bounded pass over live sessions in
   deterministic `SessionId` order. **Do not call
   `drain_runtime_all_once`.** That primitive returns on the first
   non-exited `SessionNotFound` error and would skip later siblings.
   Observe must call per-session `drain_runtime_once` (or the same
   per-session output/route/apply sequence), reconcile that session's
   lifecycle observations, retain incidental terminal egress on the
   existing pending-drain path, then continue to the next session.
   `SessionNotFound` plus engine-exited is a continue. Any other error
   is retained for that session and does not stop the remaining pass.
   After every session has been attempted, return a control-plane
   `ObserveLifecycleResult` that reports retained per-session errors.
   Return no terminal bytes, phases, snapshots, attach state, or
   `ProcessExited` frames.
3. Update the journal from those Core runtime and ClientWorker lifecycle
   facts. Repeat observations still do not append.
4. Set one coalesced `journal_advanced` pending bit inside
   `append_lifecycle_change`. The wake is one bit, not a queue. Duplicate
   appends before take stay one bit.
5. Add `take_journal_advanced_wake() -> bool` that clears the bit. Page
   and baseline never clear the bit. Append always sets it.
6. Add `lifecycle_changes_page(after, max_changes, max_bytes) ->
   Result<SessionLifecyclePage, SessionLifecyclePageError>`.
   `SessionLifecyclePage` carries ordered changes, next cursor, source
   watermark, and the existing explicit resync reasons.
   Publish `SessionLifecyclePageError` as `#[non_exhaustive]` with the
   first variant `BudgetTooSmall { minimum_bytes }`. This is an explicit
   compatibility decision under [[botster core public enums are breaking
   until non exhaustive is decided]]: first publication stays open to
   later typed page errors without an exhaustive-match break. Do not
   publish the enum as exhaustive. Downstream matches must handle
   `BudgetTooSmall` and include a wildcard. Existing
   `SessionLifecycleChangeKind` and `SessionLifecycleResyncReason` are
   already `#[non_exhaustive]`; follow that contract. Algorithm:
   1. Validate the cursor first. A changed source, expired cursor, or
      cursor-ahead returns `Ok` with empty `changes` and the exact
      resync reason. That is a control outcome, not a successful page.
      It is not required to satisfy `max_bytes`. Never return a
      partial suffix.
   2. For a valid cursor, encode the empty successful page (no resync,
      empty changes, next = `after`, current watermark). Let
      `minimum_bytes` be that exact encoded length.
   3. If `max_bytes < minimum_bytes`, return
      `Err(BudgetTooSmall { minimum_bytes })`. Do not return a
      successful page or a partial page.
   4. Otherwise greedily include the next change only when the fully
      encoded successful page still has `len() <= max_bytes` and the
      item count is still `<= max_changes`.
   `max_bytes` is the maximum encoded size of a **successful**
   `SessionLifecyclePage`. It includes metadata, cursors, watermark,
   the resync field, list delimiters, separators, and changes. Do not
   exclude the fixed page envelope.
7. Keep `lifecycle_changes(after)` as the compatibility unbounded reader
   so current Hub source keeps compiling. Do not add fields to
   `SessionLifecycleChanges`.
8. Preserve `TransportEgress::ProcessExit` / ClientWorker `ProcessExited`
   as the terminal-plane frame. Do not require attach or
   `CoreDaemon::drain` to observe exit on the control plane.
9. Update living docs: this file, `docs/architecture/core-daemon.md`, and
   the README host-loop / lifecycle-projection paragraphs.

### Non-scope

- Hub session projection, maintenance slices, plugin session-family
  consumption, or removing Hub's current Drain-based discovery.
- Package events, `events.emit`, client event subscriptions, Web, TUI.
- Changing ClientWorker queue policy, bind/generation rules, or adapter
  close hard-stop.
- Moving `ProcessExited` onto the control plane or putting terminal bytes
  in journal records.
- Automatic host-session cleanup or `remove_session` policy.
- A replacement test wrapper. Use the repository Cargo gates.
- Ratifying every proposed north-star vault note.

## Repository ownership and cross-repo dependencies

Core owns:

- Session/process lifecycle facts
- The in-memory journal, wake bit, and page API
- Terminal-plane `ProcessExited` delivery and subscription close

Hub owns:

- Whether and when to consume the wake and pages
- Host retention, worktrees, plugin publication, and cleanup after exit
- Unix/WebRTC adapters and route policy

Do not implement Hub in this run. The Hub consumer is already registered:
`ticket_1786663582_169720` depends on this ticket
(`dependency_1786663627_206668`). No additional dependency is required.

Downstream-shaped proof stays in `botster-core-test-support` consumers,
not in a Hub checkout.

## Assumptions and unknowns

Assumptions:

- `observe_lifecycle` is the implied production entry. The ticket names
  wake and page, not the progress method. Page/wake/baseline stay
  side-effect-free; without a non-Drain progress tick the journal cannot
  advance for zero-client sessions. This is required, not speculative
  configurability.
- Encoded successful-page bytes are `serde_json::to_vec` of the complete
  `SessionLifecyclePage`. Include metadata, next cursor, watermark,
  `resync_required`, the changes array, and JSON separators. Human
  answer `question_1786683509_729694` forbids excluding the envelope.
- Cursor validation precedes the successful-page byte budget. A foreign,
  expired, or ahead cursor returns `Ok` with empty `changes` and the
  exact resync reason even when `max_bytes` is undersized. Resync is a
  control outcome, not a successful page.
- After a valid cursor, compute the exact encoded size of the empty
  successful page. If `max_bytes` is below that size — including
  `max_bytes == 0` and `minimum_bytes - 1` — return
  `Err(BudgetTooSmall { minimum_bytes })`. Never return a successful
  page that encodes larger than `max_bytes`.
- After a valid cursor with `max_bytes >= minimum_bytes` and
  `max_changes == 0`, return the empty successful page. Its encoded
  length equals `minimum_bytes` and is `<= max_bytes`.
- If the next change cannot fit the remaining full-page budget after
  some items, stop with the last fitting successful page. If the first
  candidate alone would exceed `max_bytes` after a valid budget, return
  the empty successful page and do not advance next. Recovery from an
  oversized single change is a fresh baseline. Hosts that want at least
  one change must set `max_bytes` to `minimum_bytes` plus one max-size
  record (`MAX_CORE_SESSION_METADATA_LEN` is 64 KiB plus record and
  remaining page envelope).
- Wake is a coalesced hint. The page watermark is the source of truth.
  Safe consumer order: take, page until `next == watermark` or resync,
  take again, and re-page if that second take is true. Never page-then-
  take-then-sleep. Page never clears the wake. Append always sets it.
  `BudgetTooSmall` is not catch-up and not sleep; the host must raise
  `max_bytes` to `minimum_bytes`.
- Existing `lifecycle_changes` remains an unbounded compatibility reader.
  Hub's later ticket switches to page + wake.
- Worktree path has no `:`. Tracked `.gitignore` is present and non-empty.
  No `CARGO_TARGET_DIR` override is required.
- This is not a Hub session-type eligibility consumer.
- `SessionLifecyclePageError` is `#[non_exhaustive]` at first publish.
  That is the compatibility decision. It is not an accepted exhaustive
  break.

Unknowns Implement must not invent:

- Hub maintenance-slice budgets and owner-turn numbers belong to the Hub
  ticket.
- Whether Jason later ratifies the broader north-star vault set. This
  slice is authorized by the Event Plane project and this ticket.

## Affected surfaces and files

Expected production path:

`worker / local runtime ProcessExited`
→ `CoreDaemon::observe_lifecycle`
→ per-session `drain_runtime_once` in `SessionId` order
→ multiplexer lifecycle + `apply_client_worker` for that session
→ `reconcile_lifecycle_observations` / `append_lifecycle_change`
→ coalesced wake bit
→ host safe loop: take → page until caught up → take → re-page if woke

`ObserveLifecycleResult` carries retained per-session errors after the
full pass. Sibling journal upserts from the same tick remain visible
even when an earlier session's drain returns `OutputFailed` or another
non-exited error.

Primary files:

- `crates/botster-core-daemon/src/api.rs` — `SessionLifecyclePage`,
  `SessionLifecyclePageError::BudgetTooSmall`, `ObserveLifecycleResult`,
  keep existing resync reasons
- `crates/botster-core-daemon/src/daemon.rs` — per-session observe,
  wake, full-page byte budget, cursor-before-limits page
- `crates/botster-core-daemon/src/lib.rs` — re-export the new types
- `crates/botster-core-daemon/tests/daemon_integration_test.rs` — zero-client
  exit, wake coalesce, dropped-wake, full-page byte bounds, zero-limit
  resync, same-tick sibling progress after one drain error
- `crates/botster-core-test-support/tests/consumers/` — Hub-shaped
  isolated consumer of observe / wake / page plus the safe consume loop
  and wake/page interleaving harness
- `docs/architecture/core-daemon.md`, `README.md`, this file

Likely untouched unless a compile forces it:

- ClientWorker, terminal adapter traits, terminal protocol crates
- Plugin admission (already shipped on a sibling ticket)
- Hub, Web, TUI, Workspaces

## Runtime-teardown lens answers

| Field | Answer |
| --- | --- |
| `teardown_class_applies` | yes — terminal-state vs live-runtime divergence; journal must advance from runtime/ClientWorker facts without Hub terminal Drain or a terminal client |
| `teardown_isolation` | One session exit updates only that session's journal row and closes only that session's terminal subscriptions. The wake bit is process-wide by ticket contract (one pending bit, not a per-session queue). Sibling sessions stay live. |
| `teardown_bounds` | `observe_lifecycle` is one non-blocking host tick. It must not wait for PTY death, adapter I/O, or Hub Drain. Page/wake/baseline do not block. Existing ClientWorker hard-stop remains synchronous close+drop on the tick that observes `ProcessExited` for bound subscriptions. Do not add `block_on(close)` or a closer thread. |
| `late_message_matrix` | See table below. |
| `production_path_proof` | Worker-backed self-exit, zero attaches, no `CoreDaemon::drain`. Host runs the safe loop (take, page until caught up, take, re-page if woke) until the page shows `Exited`. A second proof drops the wake and still converges by paging from the last cursor. Same-tick sibling proof: earlier session drain errors, later sibling still exits into the journal on that observe. Red-on-revert: removing observe-to-journal wiring or switching observe to `drain_runtime_all_once` fails those tests. Terminal Drain and attach remain available but are not the oracle. |
| `ownership_identity` | Journal identity is `(source_id, sequence)`. Session identity is `SessionId`. Terminal subscription identity remains `(session_id, subscription_id, generation)`. Delayed Drain or a late `ProcessExited` must not append a second identical `Exited` upsert. Daemon restart mints a new `source_id`; old cursors resync with `SourceChanged`. |
| `sibling_fail_closed_policy` | Success: siblings keep running and their journal rows are unchanged. Observe walks sessions itself and continues after a per-session drain error. It does not use `drain_runtime_all_once`. A retained error is reported after the full pass; later siblings in the same tick still reconcile. Registry save failure fails that session's projection without corrupting previously appended sequences. Ultimate observe failure does not kill sibling sessions or the daemon. |

Late-message matrix (ownership-creating or lifecycle-visible messages):

| Message | Tag / owner | After terminal failure / exit | Residual sweep |
| --- | --- | --- | --- |
| Spawn | new `SessionId` | N/A; creates a new row | journal upsert `Running` |
| Attach | `(session, subscription, generation)` | `SessionNotReadable` / unknown after remove | no new generation |
| Bind | live generation | `BindBeforeAttach` or `StaleGeneration` | no new owner |
| `observe_lifecycle` | daemon control plane | idempotent; no duplicate `Exited`; per-session drain errors are retained and later siblings still run | wake coalesces |
| `CoreDaemon::drain` | terminal plane | still returns retained/pending egress; must not be required to learn exit | ProcessExited stays here for attached clients |
| Input / resize | live mutable session | rejected after stopping/exited/failed | no journal noise |
| `remove_session` | explicit host forget | allowed only when terminal | journal `Removed` |
| `lifecycle_changes_page` | cursor `(source_id, sequence)` | `SourceChanged` / `CursorExpired` / `CursorAhead` with empty changes, even when `max_bytes` is undersized | recover via `lifecycle_baseline` |
| `lifecycle_changes_page` undersized budget | valid cursor + `max_bytes < minimum_bytes` | `Err(BudgetTooSmall { minimum_bytes })`; no page | raise `max_bytes` to `minimum_bytes` |

## Risks

- Hiding observe inside page/wake would violate the existing
  "consumption does not advance runtimes" rule and make reads
  non-idempotent. Keep observe separate.
- Per-session `drain_runtime_once` can produce terminal egress. That
  egress must stay on `pending_drain` for later Drain. If it leaks into
  the page API, the architecture tests fail.
- Calling `drain_runtime_all_once` would abort the tick on the first
  drain error (`managed_session_runtime.rs` first-error return). Observe
  must not use that aggregate primitive.
- An undersized `max_bytes` used to return an empty successful page that
  itself exceeded the budget. That is forbidden. Return
  `BudgetTooSmall { minimum_bytes }` after cursor validation. Resync
  still wins over the budget.
- Page-then-take-then-sleep can clear a wake that arrived after the page
  snapshot and leave the cursor behind. The published consumer loop and
  interleaving test exist so Hub cannot invent that order.
- Adding fields to `SessionLifecycleChanges` would break Hub struct
  literals ([[public dto field additions are source breaking without non
  exhaustive]]). New `SessionLifecyclePage` avoids that.
- Keeping unbounded `lifecycle_changes` forever can become a second
  path. Accept it as compatibility only; Hub's ticket owns the cutover.
- Stale reviewer guidance may cause Plan Review to demand Drain proof.
  The charter and this lens table are the override. Product proof is
  observe + wake + page.

## Acceptance checks and tests

Focused during development (no wrapper):

- New daemon test: worker-backed self-exit, **zero attaches**, **no**
  `CoreDaemon::drain`, `observe_lifecycle` until wake, page shows
  `Exited` with registry `Exited` and engine `Exited { code: Some(0) }`.
- Dropped wake: leave the bit uncleared (or take and discard), later
  page from the last good cursor still returns the `Exited` upsert.
- Duplicate wakes: two journal appends before take; `take` is true once,
  then false until another append.
- Full-page byte budget: every `Ok` page with `resync_required == None`
  is a successful page. Serialize it and assert
  `encoded.len() <= max_bytes`. Prove item-count stops and encoded-page
  stops. Next is the last included change; watermark is the source head.
- Undersized budget: with a valid cursor, `max_bytes = 0` and
  `max_bytes = minimum_bytes - 1` return
  `Err(BudgetTooSmall { minimum_bytes })`. `max_bytes = minimum_bytes`
  returns the empty successful page whose encoding equals
  `minimum_bytes`.
- Resync with undersized budget: for each of `SourceChanged`,
  `CursorExpired`, and `CursorAhead`, call the page API with
  `max_bytes = 0` (and again below the empty-success minimum). Require
  `Ok` with empty changes and the exact resync reason, not
  `BudgetTooSmall`.
- Wake/page race: Hub-shaped consumer implements the safe loop (take,
  page until `next == watermark` or resync, take, re-page if woke).
  Interleave journal appends before take, between take and page, after
  page before the second take, and after the second take. After every
  completed loop iteration, either the latest change is applied or a
  wake remains pending. The unsafe page-then-take-then-sleep order is
  documented as forbidden and is not the consumer under test.
- Same-tick sibling isolation: two live sessions, deterministic
  `SessionId` order. The earlier session's `drain_runtime_once` returns a
  non-exited error (`OutputFailed` or equivalent worker I/O failure, not
  the exited-`SessionNotFound` continue path). The later sibling
  self-exits with zero attaches and no `CoreDaemon::drain`. One
  `observe_lifecycle` tick retains the first error in
  `ObserveLifecycleResult` and still publishes the later sibling's
  `Exited` journal upsert.
- Control-plane-only architecture test: `SessionLifecyclePage` /
  change kinds cannot carry `TransportEgress`, snapshot payloads, attach
  phases, or terminal bytes. Source-scan the daemon API lifecycle types.
- ProcessExited preservation: an attached bound-adapter (or unbound
  Drain) path still delivers terminal-plane `ProcessExited` after
  observe has already published control-plane `Exited`.
- Compatibility: existing `lifecycle_changes` tests and the current
  attach+drain lifecycle test still pass. Drain may still update the
  journal; it is no longer required.
- Hub-shaped isolated consumer in test-support compiles against
  observe / wake / page only, never calls Drain, owns the safe
  consume loop plus interleaving harness above, and matches
  `SessionLifecyclePageError` as `BudgetTooSmall { .. }` plus a
  wildcard for unknown future variants. An exhaustive match without
  `_` is a consumer defect. An empty successful page with
  `next != source_watermark` installs a fresh baseline. A consumer
  test uses `max_changes > 0` and `max_bytes` equal to the empty-page
  minimum and asserts the projection reaches the watermark.

Repository gates ([[botster-core uses CI-owned Cargo commands because it
has no test script]]):

```sh
BOTSTER_ENV=test cargo test --workspace
cargo fmt --all -- --check
BOTSTER_ENV=test cargo clippy --workspace --all-targets -- -D warnings
BOTSTER_ENV=test cargo test --doc --workspace
```

Do not create `cli/test.sh` or any replacement wrapper.

## Vault gaps

Capture after implement, not during Plan:

- [[hub drain advances non attached session lifecycle]] is the old
  no-Attach contract. Replace or supersede with: observe + wake + page
  is the control-plane progress path; Drain is terminal-plane only.
- [[botster core hosts need an explicit drain loop contract]] should
  split control-plane observe from terminal Drain in the host loop.
- [[botster-runtime-reviewer-playbook]] no-Attach Drain bullet becomes
  stale the moment this ticket lands.
- New durable claim: Core control-plane lifecycle journal advances
  without a terminal client or Hub terminal Drain.

No inbox capture in this Plan visit. The gap is known and named.

## Plan Review findings (revision 4)

| Finding | Resolution |
| --- | --- |
| `finding_1786683950_438902` public page error enum has no compatibility decision | Publish `SessionLifecyclePageError` as `#[non_exhaustive]`. First variant is `BudgetTooSmall`. Hub-shaped consumer matches that variant plus a wildcard. Not an exhaustive break. |
| `finding_1786683591_529445` undersized budgets violate the page bound | Resolved in revision 3 via human answer `question_1786683509_729694`. |
| `finding_1786682974_103248` full encoded-page byte bound | Resolved in revision 3. |
| `finding_1786682974_155478` zero limits hide resync | Resolved in revision 2. Revision 3 keeps resync ahead of `BudgetTooSmall`. |
| `finding_1786682974_138502` wake/page race | Resolved in revision 2. Unchanged. |
| `finding_1786682974_859083` sibling isolation | Resolved in revision 2. Unchanged. |
| `finding_1786682974_452064` empty Plan completion evidence | Resolved in revision 2. This visit again submits full gate and advance evidence. Reuse ticket checklist `checklist_1786682346_769728`; do not create a second vault checklist. |

## Delivery

- Direct-merge pipeline into `main`.
- Do not open a pull request.
- Do not require human PR sign-off.
- One Plan → Implement path. No dual pipeline for planner variety.
