# Contract steward handoff (Botster v1 paused, third time, 2026-10-05)

Botster v1 work is **paused** by the user (orchestrator relay, 2026-10-05: token budget). Earlier pauses: 2026-10-01 and 2026-10-02. At this pause: manifest **final35**, rulings **R-1 to R-34**, no amendment in flight, and no open question from a lead. This file lets a fresh contract steward take over. Read it first, then `docs/handoffs/orchestrator.md`, `docs/BUILD.md` and `docs/steward-rulings.md`.

**Resume the previous steward:** `claude --resume a7d7106d-f276-4edb-a69e-e2b62b5bd6e9` (the working directory was `~/botster-sessions/trybotster-botster-hub-audit-plan-20260930`; the repo checkout it edits is `~/Projects/botster-contracts`).

## 1. The role

The steward (BUILD.md Roles):
- decides what the contracts mean (rulings);
- runs the amendment path;
- keeps the joint manifest.

It is a side channel for **spec meaning only**, not a layer in the bubble-up chain (pair → lead → orchestrator → user). It messages the orchestrator **only** for an amendment that changes scope or needs a decision, or for a blocker (user rule, BUILD.md, 2026-10-01). Routine amendments, manifests and rulings go to the state log `~/botster-sessions/botster-v1-orchestrator-state.md` and to the leads affected. **The same rule applies downward:** agents the steward directs (the Amendment reviewer) send the steward only QUESTION, BLOCKED or a terminal event (a verdict). Every brief the steward writes must include this rule.

## 2. Manifest state

- **Current: final36**, in `frozen/current/MANIFEST.md`. Tag `manifest-final36` on main `60a4169` (2026-10-09; crate PR 13 ACCEPT at `80ab368`, gate green at `80ab368` and at the merge head `60a4169`).
- **Staging: final37** (the WebRTC move: Core A17 c2, HC A8 c3, HP A9 c1, codec rev37) on `steward/final37` (`13e5a8b`, worktree `~/botster-sessions/botster-contracts-steward-final37`). It waits for the pair's crate change (`shared/foundation/brief-final37-crate.md`, merge condition: all 22 withdrawn ids). **At merge:** restore "their crate change is on this final branch" in the MANIFEST WebRTC section and its acceptance-record item (reviewer finding 1 of `e39155d`), then ask the reviewer for the merge review. P4c waits for `manifest-final37`.
- **Publish rule (orchestrator, 2026-10-09):** an amendment is FINAL only with its crate change merged on the same final branch. If main has moved, merge main into the final branch and run the gate again on the merge head before main fast-forwards.
- **Layout** (orchestrator decision, README rule 1):
  - `frozen/final6/` is the record of the joint freeze. The user signed off on final6 on 2026-09-30.
  - `frozen/current/` is **add-only**: an accepted amendment adds its file, and `frozen/current/MANIFEST.md` is the only file ever edited.
  - The CI job `frozen` fails on any other change to an existing file.
  - Each manifest commit is tagged `manifest-finalN` (`manifest-final6` to `manifest-final36` exist). final7 to final10 were published as `frozen/finalN/` directories, which now exist only at their tags.
- **What each manifest added:**

  | Manifest | Added | Reviewer ACK |
  |---|---|---|
  | final7 | ui-contract 1.4 (`ui-contract-v1.4-realize-completeness-candidate3.md`) | `eaef7d9` |
  | final8 | HC Amendment 4: `req.action` (`hub-client-contract-v1-amendment-4-candidate1.md`) | `2280df4` |
  | final9 | Core Amendment 4: `service_report` (`core-contract-v1.17-amendment-4-candidate1.md`) | `6398ba4` |
  | final10 | ui-contract 1.4 erratum 1: error tone is `danger` (`ui-contract-v1.4-erratum-1-candidate1.md`) | `9cd09df` |
  | final11 | Core erratum 1: worker file name not fixed by Core (`core-contract-v1.17-erratum-1-candidate1.md`) | merge `19d4e66` |
  | final36 | HC Amendment 7 (upgrade preview `breaking` list, `static_origin`), Core Amendment 18 (`last_output_at` is the host's observation), UI 1.4 erratum 2 (`Text` props), with their crate change | `1b2fab7`, `e46d275`, `37991e4` |
  | final35 | Core Amendment 15: `CoreLimits.max_key_text_bytes` (default 256, range 1 to 4,096, user ruling R-34) bounds a key's `text` (`core-contract-v1.17-amendment-15-candidate2.md`) | `87c440a` |
  | final34 | Core Amendment 16: `StopService` completes with `ServiceEnd`; per-reason `Lost` outcomes; a recorded exit is final (`core-contract-v1.17-amendment-16-candidate3.md`) | `2bc82c8` |
  | final33 | Core Amendment 14: `clipboard_bytes` bounds the decoded payload (decoded size, then contents size with aliases); the model decode limit equals the bound (`core-contract-v1.17-amendment-14-candidate3.md`) | `5fd6ac9` |
  | final32 | HC Amendment 6: `package.install` result names the package; detached worktree head (`hub-client-contract-v1-amendment-6-candidate1.md`) | `c68a61a` |
  | final31 | HP Amendment 8: the Hub makes the PD-4 checks against the bound document; MINOR additivity checked at admission; `breaking` pairs in the upgrade preview (`hub-plugin-contract-v1.1.amendment-8-candidate2.md`) | `717ee0d` |
  | final30 | Core Amendment 13: `ClipboardWrite` for every clipboard write the model recognizes, all representations atomic; the worker alone acknowledges OSC 5522; EV-3 replaced; the shadow never answers a clipboard read (`core-contract-v1.17-amendment-13-candidate5.md`) | `584bf8e` |
  | final29 | HP Amendment 7: lock history holds previous locks; rollback steps back; entries keep their admitted limit (`hub-plugin-contract-v1.1.amendment-7-candidate3.md`) | `f3960d9` |
  | final28 | Lua binding r9 Amendment 2: flat tool, prompt and resource entries (`hub-plugin-lua-binding-v1.1-r9-amendment-2-candidate1.md`) | `fba9f37` |
  | final27 | HP Amendment 6: publish status decided at admission; `pending_gap` vs `resync_scheduled` by projection (`hub-plugin-contract-v1.1.amendment-6-candidate2.md`) | `1fa13a7` |
  | final26 | Lua binding r9 Amendment 1: json constructors, bare conversion values, flat handler descriptors (`hub-plugin-lua-binding-v1.1-r9-amendment-1-candidate1.md`) | `21c9b36` |
  | final25 | HP Amendment 5: one entity record is `{id, entity}`; client schema unchanged, `id` a core field (`hub-plugin-contract-v1.1.amendment-5-candidate3.md`) | `b8121e9` |
  | final24 | HP Amendment 4: `write_pending_bytes` is a Hub-wide total; `backpressured{write_pending_full}` (`hub-plugin-contract-v1.1.amendment-4-candidate2.md`) | `847b5a9` |
  | final23 | HP Amendment 3: the scripted services are real-Hub edges; WP-3 non-claim test withdrawn (`hub-plugin-contract-v1.1.amendment-3-candidate1.md`) | `d33b4b0` |
  | final22 | Core erratum 4: a query reply's admission cost (`core-contract-v1.17-erratum-4-candidate1.md`) | `041b963` (rebased `a1902f2`) |
  | final21 | Core Amendment 12: hold a worker report; a script point inside the `Remove` cleanup (`core-contract-v1.17-amendment-12-candidate2.md`) | `c3c6bce` |
  | final20 | Core Amendment 11: impostor frames after a failed handshake (`core-contract-v1.17-amendment-11-candidate1.md`) | `383e814` |
  | final19 | Core erratum 3: due deadlines and the `pump_events` budget (`core-contract-v1.17-erratum-3-candidate3.md`) | `b1837f2` |
  | final18 | Core Amendment 10: impostor at the adopt handshake; corrupted registry row (`core-contract-v1.17-amendment-10-candidate1.md`) | `3e350c9` |
  | final17 | Core Amendment 9: `max_frame_bytes` carries the route's own `attached` frame; A3-1 eligibility tests withdrawn; the testkit makes the compression choice (`core-contract-v1.17-amendment-9-candidate1.md`) | `fc32271` |
  | final16 | Core Amendment 8: capture byte reservation; an unformable snapshot is `SnapshotTooLarge` (`core-contract-v1.17-amendment-8-candidate5.md`) | `98565fd` (rebased `7809d9a`) |
  | final15 | Core Amendment 7: an attach option out of range is sync `InvalidInput{field}` (`core-contract-v1.17-amendment-7-candidate4.md`) | `7d613ae` |
  | final14 | Core erratum 2: title is OSC 0/2; `other_modes` are tracked model modes; `ModesChanged` only on a net change (`core-contract-v1.17-erratum-2-candidate2.md`) | `19668c9` |
  | final13 | Core Amendment 6: testkit service-lane edge and withheld control link; first worker protocol 1 and the enumerated deferred set; `RemoveReport` for uploads (`core-contract-v1.17-amendment-6-candidate3.md`) | `5d2dd45` |
  | final12 | Core Amendment 5 and HC Amendment 5: real implementations with injected edges replace FakeCore and FakeHub (`core-contract-v1.17-amendment-5-candidate4.md`, `hub-client-contract-v1-amendment-5-candidate2.md`) | `773cc44` |

- **Checks at final12:** `cargo xtask manifest` (40 files) and `cargo xtask ledger --check` (2,355 ids) pass. The 24 new A5 test ids are in `conformance/pending.txt`.

## 3. How to publish an accepted amendment (the recipe)

1. The draft goes in `amendments/<contract>-…-candidateN.md`. Run `chmod 444`, record its sha256, and commit it.
2. Send the Amendment reviewer a review request by `post_message`, with the file, commit and sha256 in the **payload** field.
3. On REJECT, write candidate N+1 with a "Supersedes" history block. Never edit a published candidate.
4. On ACCEPT, publish it:
   - copy the file into `frozen/current/` and `chmod 444` it;
   - add a manifest section, a table row (file, sha256, ACK commit) and an acceptance-record line to `frozen/current/MANIFEST.md`;
   - retitle the manifest to finalN+1;
   - add the file to its contract's list in `contracts.toml`.
5. Run `cargo xtask ledger` to regenerate the ledger. Add the new test ids that `cargo xtask ledger --check` reports to `conformance/pending.txt`, keeping it sorted.
6. Run the merge gate's ledger job, not only the two checks: `botsterq run --deadline 15m -- sh -c 'cargo xtask manifest && cargo xtask ci --job ledger'`. The ledger job also runs `replacement-map --check`, so every new id must have an entry in `conformance/replacement-map.json`. **Before pushing, send the new ids to the Stage 0 lead**, who maps them (a proof kind such as core-testkit). Push only when the job passes. (final14 put main's ledger job red because this step was missing; the Stage 0 lead fixed it in 75c4542.)
   - **Id states (Stage 0 `ledger-status`, main `d03438a`):** an id is transcript, pending, deferred (`conformance/deferred.txt`) or withdrawn (`conformance/withdrawn.txt`). Every deferred or withdrawn line cites an authority clause in the current manifest. When an amendment or ruling defers or withdraws an id, give the Stage 0 lead the id and its authority, or add the line on the final branch yourself.
   - **Final branch flow:** prepare `steward/finalN` from main and push it; the Stage 0 lead adds the replacement-map entries, runs the ledger job and fast-forwards main; then tag `manifest-finalN` on the manifest commit.
   - **Crate first (orchestrator, 2026-10-09):** an amendment is not FINAL until its crate change (types, fields, variants, codes, limits, regenerated schemas and TS) is merged on the same final branch. Do not fast-forward main or tag `manifest-finalN` without it. Why: finals 30 to 35 shipped text only, and Core A15 was blocked on a missing `CoreLimits.max_key_text_bytes`. The crate-catchup pair (brief `~/botster-sessions/shared/foundation/brief-crate-catchup.md`) closes the backlog and adds a CI check that fails a manifest amendment whose symbols the crate lacks.
7. Commit. Update the "Current manifest" line and the amendments table in `README.md`. Push. Tag `manifest-finalN` and push the tag. Watch CI on the newest commit (`gh run`). An older run shows "cancelled" when a newer push supersedes it.
8. Send DONE to the orchestrator, and tell the lead which ids are now live.

**Gotcha:** the Amendment reviewer merges its verdicts into the shared local `main` with merge commits. Use **`git pull --ff-only`** in this checkout, never `--rebase`: a rebase tries to re-apply verdict commits and conflicts in `verdicts/amendments.md`. If that happens, run `git rebase --abort`. `main` is then a fast-forward of origin.

## 4. Rulings (full text in `docs/steward-rulings.md`)

| Ruling | One line |
|---|---|
| R-1 | Phase 0 fakes: TS FakeHub may wrap wasm; bindings; plugin-side FakeHub scripted; fault hooks; FakeCore placement. Partly superseded by R-12. |
| R-2 | ui realize crate: UTF-8 byte ids, `container` by mapping shape, UiNode vocabulary stolen from botster-hub@9e1ba42, capability names, `stale`, action access via `kind`. |
| R-3 | ui pending items are normative (the vectors bind every renderer); copy is keys, not text. |
| R-4 | HP frame bytes are assigned by the generated artifact; `Cancel` is state-changing; `log.write` reuses HostCall/HostResult on lane 3. |
| R-5 | FakeCore budget: no terminal model, no encoder, no shared encoder crate. Superseded as a budget question by R-12. |
| R-6 | HP manifest: four required keys, absent = empty; the frame check order is len, JSON object, epoch, type. |
| R-7 | **Standing rule (the user):** libghostty is the only terminal oracle; terminal ids are real-only. |
| R-8 | `req.action` carries the `invoke` action name (HC A4). |
| R-9 | `service_report(id)` (Core A4); route chunk bound is the transport's max at attach. |
| R-10 | The "error" tone is `danger` (ui erratum 1). |
| R-11 | The `session` row: required set, absent not null, `exit` always present, typed `attention`. |
| R-12 | Which parts of R-1 and R-5 Core A5 and HC A5 supersede. |
| R-13 | SGR-pixels (1016) mouse positions are zero-based; the +1 is only for cells. |
| R-14 | `Unsupported{named_key}` = modifier keys without kitty flag 8, else real-only; `wheel_direction` real-only; `CursorReadUnsupported` = a read in `Starting` before the model exists; `control_link_stats` allowed as a measurement. |
| R-15 | Remove under a full mandatory queue keeps LC-7 order: step 1 (route close + event) waits for room; steps 2–5 wait for it; EV-5(c) covers steps that need no event. |
| R-16 | StopAll: a failed or uncertain Stopping-row write is best effort; the stop still ends the payload; StopAll completes per LC-12. |
| R-17 | A query's `request_bytes` are the bytes the terminal model assembled (a C0 control executed inside is its own query). |
| R-18 | An affected query's id is learned by determinism (two runs, same seed), not by a worker-internal observer; stale-answer proof at the program edge. |
| R-19 | A synchronously refused `attach` leaves the transport with the caller (DP-2); the crate's by-value signature must return it on refusal. |
| R-20 | "Completes in the next pump" (LC-5, SZ-2, the Created Resize row) is fixed; the seeded scheduler never defers it. |
| R-21 | Session-family snapshot: no chunk count, union = set, no delta inside; EV-3: four statuses live, `rejected_foreign`/`rejected_audience` reserved (static table); NT-5 origin recheck tested with authority-shaped secret values. |
| R-22 | `relay.deliver` to a completed or expired attempt (same instance) is `rejected`; `not_found` only for another plugin or an earlier instance. |
| R-23 | `hub/route.ended` subscription is refused by the Hub at registration; `lb_21_route_ended_is_client_only` is proven Hub-side. |
| R-24 | MCP descriptor uncarriable = a duplicate key (WF-2, HC A.1) or an integer outside ±(2^53−1) (WF-3, HC A.1); only that tool is omitted. |
| R-25 | CLI goldens only for what the spec fixes (JSON, `config`, `context`, the `running binary:` line); human-readable status/list-worktrees assert content, not layout. |
| R-26 | MP-12: descriptor bytes bounded by `mcp_proxy_message_bytes`; `mcp_proxy_tools_max` is per server, per-plugin total = `mcp_servers_max` × it; over → server unavailable, list discarded. |
| R-36 | A successful `Adopt(id)` of a `Lost(WorkerUnreachable/WorkerVersion)` row follows the worker's report (`Running`/`Exited`/`Lost` again); a `Lost` row keeps identity, not intent; the host re-issues `Stop` if it still wants one; `Adopt` is never refused for missing intent. |
| R-35 | Adoption of a `Starting` row whose payload never launched performs AD-7 step 4 (`Launch`); a failed launch takes LC-4's ordinary start-failure outcome; a launch in progress is not repeated; `Stopping` with no launch becomes `Exited{HostStop}`; `Running`/`Exited` with no payload is `Lost(RegistryCorrupt)`; Core never reports `Lost(Other)` (a host-side mapping). |
| R-34 | User rulings 2026-10-05: A15 numbers (default 256, range 1 to 4,096); the Web client encodes keys with restty against `modes` and sends `bytes` (no contract change). |
| R-33 | MIME types past the model's 64-type limit are ignored by the model, so they are outside A14's decoded size and limit (reading (a)); no decoding only to count. |
| R-32 | libghostty's 64 MiB OSC 5522 limit must call the callback (no contents, full length) and send no acknowledgement of its own: a fork patch; the worker then posts `TooLarge` and acknowledges EIO (A13 unchanged). |
| R-31 | `oracle_resume_every_cut` reuses one session per item (captures are reads; release each before the next), compares at the first ground-state offset or the end; no offset sampling; one beyond-limit string (limit + 1) per kind; rulings bind from main, and the orchestrator cuts tags. |
| R-30 | A8-2 publishes only the continuation limit (in the format spec), no pending-state size bound; the every-cut helper may prove fit by an exact oracle encoding plus spec framing (per-capture fields at their maximum); an inconclusive cut within the limit does not pass the every-offset id. |
| R-29 | HC wire members: envelope `type`; watch and pairing sub-frames in `frame`; `event_gap` is `event` with `frame: gap`; `if_rev` in `args`; times are Unix ms except `expires_at` (s); `rescan_packages` result lists; `rev` an opaque string on every row. |
| R-28 | The kitty flag stack is the model's (the oracle ignores `CSI = 63;1 u`: no event); a no-net-change set posts nothing; a release without flag 2, or in legacy mode, is `NotReported` (`release_event` has no v1 stimulus). |
| R-27 | Lua binding refusals (LB-2, LB-11, LB-15) fix the envelope the plugin sees, not where it is checked; transcripts never assert HostCall presence or absence. |

Every ruling is also captured in the vault inbox (`/capture`), up to R-12. The fake-mode decision is captured at this handoff.

## 5. In flight and open at the pause (2026-10-02)

- **final32 and final33 are published** (2026-10-04). Stage 0 queued HC A6 (hub-contract change and CLI ids) and Core A14 (joins core-a13) for its resume (STATUS.md `3263c81`).
- **No amendment is in flight.** Every candidate file in `amendments/` has a final verdict. A withdrawn candidate is never published: Core erratum 5 candidate 1 (replaced by Core A13) and Core A13 candidate 4 (a wrong verdict pointer).
- **No lead question is open.** At the pause every question has an answer, through a ruling (R-28, R-29) or an amendment (HP A8, HC A6). The answers that are waiting on others:
  - R-28's four Core transcript fixes are routed by the Stage 0 lead to P3 (the core-a13 feature).
  - R-29's crate shapes are confirmed to the hc-groups pair.
  - The A13 ids belong to P3 (the Stage 1 Core lead's package).
- **Captured in the vault inbox:** every ruling through R-29, and HP A8.
- **Housekeeping (not done, no amendment needed):** frozen HC section 12's open-order list cites "(WT-7)" for cross-stream order, but the clause is WT-8. HC A5-4 restates the list with WT-8.

## 5a. How the Amendment reviewer works

- **Request:** `post_message` with the file, commit and sha256 in the **payload**. The candidate is committed and pushed to origin/main first. The reviewer reads the commit, and it does not need a checkout.
- **Verdict:** the reviewer writes a numbered section in `verdicts/amendments.md` on its branch `review/amendments`, merges it into the shared local `main` of `~/Projects/botster-contracts`, and sends DONE with the verdict commit. **The steward pushes that commit** (`git push origin main` after `git merge-base --is-ancestor origin/main main`).
- **Check the pointer.** A DONE message once named a wrong verdict commit (then corrected it). Run `git cat-file -t <commit>` before you cite a verdict commit in a candidate.
- **REJECT:** write candidate N+1 with a "Supersedes" history block. Candidate files are `chmod 444` and never edited after they are pushed.
- **Context rule:** the reviewer sends only a verdict, a QUESTION or BLOCKED. Every brief to it must say so.

## 6. People

| Role | Session | Notes |
|---|---|---|
| Orchestrator | `sess-1790725330-0002-e5afba7455773a9e0f22332fb4dc3eeb` | Decisions, staffing, user relay. |
| Stage 0 lead 2 (replaces the Foundation lead `…0048` from 2026-10-01) | `sess-1790906371-009d-f607653bc23d9de48c8a47464787a6ba` | Owns the tooling, `contracts.toml` lists and pending ids, and the replacement map. |
| Stage 1 Core lead | `sess-1790903471-008f-8b9f78eef51d48aba5a45748495fd673` | Core packages (P2 libghostty binding, P3 worker). Asks spec questions directly. |
| Amendment reviewer (Sol) | `sess-1791583644-0173-0d5cd1af9ae86fa35dce9a5a444c9820` (since 2026-10-09; brief `~/botster-sessions/shared/steward/brief-amendment-reviewer.md`) | Reviews every amendment (section 5a). The earlier reviewer, `sess-1790827886-0054-…`, was retired at a pause. Before sending a request, check with `list_workspaces` that the reviewer session exists. On 2026-10-09, six requests went to the retired session and were lost. |

Session ids may change after the pause. Use `list_hubs` to find the current ones.
