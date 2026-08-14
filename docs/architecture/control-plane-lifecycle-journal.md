# Control-plane lifecycle journal wake and page API

Plan for ticket `ticket_1786663581_962361`: make session lifecycle
progress independent of Hub terminal Drain by updating the existing
control-plane journal from Core runtime and ClientWorker lifecycle facts,
then publishing one coalesced journal-advanced wake and a bounded page API.

This is a Core mechanism ticket in the Botster Non-Blocking Event Plane.
Living design belongs here under `docs/architecture/`, not under the retired
`docs/plans/` stub. See `docs/README.md`.

Revision 4 is the approved implementation contract. Core now publishes
`observe_lifecycle`, `take_journal_advanced_wake`, and
`lifecycle_changes_page` on `CoreDaemon`. `SessionLifecyclePageError` is
`#[non_exhaustive]` with first variant `BudgetTooSmall`.

This ticket is **runtime-teardown class** because it changes
terminal-state versus live-runtime observation: session exit must advance
the control-plane journal without a terminal client and without Hub
terminal Drain. [[botster runtime teardown lenses]] applies. Required lens
answers are in this document and must appear in Plan gate evidence.

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
  `_` is a consumer defect.

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
