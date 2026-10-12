# Contract amendment verdicts

## ui-contract 1.4, candidate 1 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-09-30.

Candidate: `amendments/ui-contract-v1.4-realize-completeness-candidate1.md`, commit `8b920a9`.
SHA-256: `bf0b4e00acf45e18a1f30eea28718f537807140d590a80d97e87c239acc08360`.

Review authority: `docs/BUILD.md`, `README.md`, `frozen/final6/MANIFEST.md`, UI 1.0–1.3, and steward rulings R-2 and R-3.

1. **Part B reuses a frozen clause ID.** UI 1.1 already defines UI-CF-5 for URL, badge, and home fallbacks. UI 1.2 cites UI-CF-5(a) and refines it for asset URLs. The candidate gives UI-CF-5 a different meaning without naming a replacement. Assign the capability clause an unused ID. Preserve the frozen fallback clause and its references.

2. **Part B does not preserve the capability vocabulary.** Its `features` definition includes contract features and `display_metric`/`display_badge`, but omits frozen `external_open` (UI 1.1 UI-CF-5) and `embed` (UI 1.2 UI-EM-6). `ui_embed` gates a view; `embed` declares the client's ability to embed. Preserve both names and their separate meanings. Also resolve absent `screens`: “all values” claims every screen, while UI-SL-3 requires the client's own screen and otherwise requires a fallback. Define this case explicitly and cite conformance coverage.

3. **UI-NV-3 changes admission without a complete reservation rule.** It adds `confirm`, `cancel`, `submit`, `optional`, and `group:<name>` roles, then rejects names matching “a role word.” The clause does not enumerate the resulting reserved set. UI-ID-1 lists the original set; UI 1.2 UI-SA-5 adds `warning`. UI-FM-7 governs property identity inside forms. State the exact additions and the affected admission clauses. Explain whether a previously valid form property such as `cancel` becomes invalid. Resolve that compatibility change against UI-VR-1 before calling the amendment additive MINOR.

4. **UI-NV-4 and UI-NV-5 leave the new shapes incomplete.** They name `selections`, `drafts`, `focus`, `cleared_selections`, and `cleared_drafts` without defining their value shapes or keys. UI-ID-5 requires form-specific draft keys. UI-RZ-2d also requires the renderer to apply stamped errors and reload changes, but the proposed `local_changes` shape contains only clear operations. Define the input and output shapes completely. State how errors and reload changes reach the renderer. Preserve the frozen local-state rule, including scroll, or name the exact change. Add vectors for these cases under the listed test IDs.

5. **UI-NV-6 does not define how copy keys appear in a realized node.** It requires keys instead of default text, but names no prop or tagged value shape. A renderer cannot distinguish authored text `cancel` from the default copy key `cancel`. The parameterized `view_unavailable { view_id }` key also has no value shape. Define the representation and its permitted props in the UiNode schema. Cover authored text that equals a key and parameterized keys in `conf::ui_nv_6_copy_keys_not_text`.

Every proposed clause has a conformance test ID. The candidate adds no numeric bound. The findings concern frozen compatibility and missing contract behavior. This review does not accept feature code or authorize changes to frozen files.

## ui-contract 1.4, candidate 2 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-09-30.

Candidate: `amendments/ui-contract-v1.4-realize-completeness-candidate2.md`, commit `c693231`.
SHA-256: `6a33b4bf91bb5ede852e534106854f3982869070d336359801f3a577e18388ef`.

The review uses the same authority as the candidate 1 review. UI-CF-6 resolves the clause ID and capability findings. UI-NV-3 preserves the reserved set. UI-NV-6 distinguishes authored text from copy keys. The following findings remain in the expanded state rules.

1. **UI-NV-4 can discard a draft during descriptor recovery.** `cleared_drafts` includes a property removed from its action's schema during reload. Frozen UI 1.3 UI-AC-7(b) requires the client to keep the user's draft across the `stale_descriptor` re-read. A descriptor change must not discard what the user typed. State how the client retains that draft when the new schema removes a property or the new tree removes its button. Preserve the frozen recovery rule and add conformance coverage for this case.

2. **UI-NV-4 does not ensure form-before-confirm order.** It emits only the first pending overlay of each button in opening order. UI-NV-5 permits a confirmation entry before a form entry for the same button. Frozen UI-RZ-2b requires the form to show first whenever both are pending. Give form entries explicit precedence, or define an input rule that ensures this result. Cover both input orders under the overlay conformance ID.

3. **UI-NV-5 contradicts the error shape and failure display rules.** `errors.forms` and `errors.actions` contain strings, but the clause emits every entry as its node's `error` prop. UI-NV-2 defines that prop as `{ code, message? }`. Define how each entry maps to a valid node and prop. Preserve UI-AC-7(a): a direct or row action failure shows an adjacent `error`-tone `text` node, and a form failure shows a form-level message. A button error prop alone does not specify that frozen output. Add conformance coverage for field, form, and direct-action failures.

4. **UI-NV-4 does not define a complete, deterministic clear operation.** An unavailable option clears its draft and stamps `option_unavailable`. The same clause clears errors whose drafts were cleared, without specifying which error survives or the application order. UI-FM-4 requires the unavailable-option error to remain visible. Also define the state after clearing: removing a draft entry can expose the same unavailable schema default again under UI-FM-5. For example, a draft and its schema default can both name a row absent from the options. Explain how the next realization keeps the cleared value and produces no new changes, as UI-NV-4 promises. Cover existing errors, replacement errors, and an unavailable default under the idempotence conformance ID.

Every proposed clause has a conformance test ID. The candidate adds no numeric bound. This verdict reviews the amendment only. It does not review feature code.

## ui-contract 1.4, candidate 3 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-09-30.

Candidate: `amendments/ui-contract-v1.4-realize-completeness-candidate3.md`, commit `78eb8bd`.
SHA-256: `363199658917b45af41215dad3f70f2197f60ad3536b0dc6c684b91f7111d42f`.

The review uses the same authority as the earlier reviews. The candidate resolves all findings from candidates 1 and 2.

1. **Frozen compatibility passes.** UI-CF-6 preserves UI-CF-5, `external_open`, `embed`, and the screen fallback. UI-NV-3 preserves the reserved set. The amendment names its additions to the adopted vocabulary and frozen props. It changes no frozen file.

2. **Draft retention and overlay order pass.** UI-NV-4 retains entries for absent properties and buttons across descriptor recovery. UI-NV-5 excludes removed properties from submission. The form takes precedence over confirmation in either input order. These rules preserve UI-AC-7(b), UI-FM-1, UI-RZ-2b, and UI-AC-4.

3. **Error handling and state changes pass.** UI-NV-3 and UI-NV-5 define separate mappings for field, form, and action errors. UI-NV-4 replaces an earlier field error when it stamps `option_unavailable`. The explicit `{ unset: true }` draft prevents an unavailable default from returning. The output reflects the applied changes, and another realization produces no changes. These rules close the candidate 2 error and clearing gaps.

4. **Conformance traceability passes.** Every new clause has a test ID. The candidate names coverage for draft retention, both overlay orders, all three error mappings, authored text equal to a copy key, and state-change idempotence.

5. **Scope and user rulings pass.** The candidate states shared input and output behavior. It adds no numeric bound. It preserves local capabilities, client-owned state, Hub admission, client realization, and the frozen amendment path. Copy keys follow UI-PT-5 and steward ruling R-3.

This ACCEPT applies to the contract amendment at the matched hash. It does not accept feature code, prove that conformance tests pass, or add a manifest entry. The steward owns the manifest update.

## Hub→client Amendment 4, candidate 1 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-09-30.

Candidate: `amendments/hub-client-contract-v1-amendment-4-candidate1.md`, commit `c2a2ac3`.
SHA-256: `a1d42a0ee196ec7b4bb9c658544ea3a558506d64e8cff10821f84f8930500955`.

Review authority: `docs/BUILD.md`, `README.md`, `frozen/final7/MANIFEST.md`, the Hub→client base and Amendments 1–3, and steward ruling R-8.

1. **The request field closes the stated gap.** EN-1a carries the separate action parameter that VB-6 and MC-2 already require. The name comes from DS-1's `actions` map, including contributed names under DS-6. The action selector stays outside `args`; the action's own arguments remain subject to `args_schema`.

2. **Frozen scope passes.** The amendment names the changes to EN-1 and EN-8. It preserves P2's verb set, descriptor validation, action dispatch, and the binding-neutral semantics of TB-0. The MCP mapping agrees with MC-2. No frozen file changes.

3. **Idempotency passes.** EN-8a prevents two different actions from sharing one cached result or pending request. It preserves EN-8's other comparison fields, normalized arguments, caller scope, retention window, and `idempotency_key_reused` failure.

4. **Conformance traceability passes.** Both new clauses have test IDs. They cover required action selection, action absence for other verbs, descriptor names, and a reused key with a different action. Existing TB-0 and MC-2 tests continue to govern binding equivalence.

5. **Behavior and user rulings pass.** The amendment specifies request behavior and errors, with no implementation mechanism or new numeric bound. It agrees with ruling R-8 and the amendment path in BUILD.md and README.md.

This ACCEPT applies to the contract amendment at the matched hash. It does not accept feature code or prove that conformance tests pass. The steward owns the manifest update.

## Core Amendment 4, candidate 1 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-09-30.

Candidate: `amendments/core-contract-v1.17-amendment-4-candidate1.md`, commit `802316b`.
SHA-256: `e78576d3455b1fa88f6a63581038441a5135f408280cc97cbe2cf47590089ddb`.

Review authority: `docs/BUILD.md`, `README.md`, `frozen/final8/MANIFEST.md`, the Core base and Amendments 2–3, and steward ruling R-9.1.

1. **Frozen compatibility passes.** A4-1 adds a synchronous read to SV-4 and the A2-1 operation table. It preserves the spawn completion, the applied and unsupported lists, the service lifecycle, and the A2-5 event enum. It changes no frozen file.

2. **The read closes the stated gap.** The host can read the original report after successful spawn and after adoption. This preserves the report when the original completion no longer exists under ID-2. The host reads recorded bounds and does not re-apply them. Service adoption remains consistent with SV-6 and the adoption guarantees cited by the amendment.

3. **Lifetime and failures are explicit.** The report remains readable in Exited and Lost until RemoveService frees the id. Unknown ids, unsuccessful spawns, and removed ids return UnknownService. These rules agree with A2-1, SV-9, and SV-10.

4. **Conformance traceability passes.** A4-1 has test IDs for equality with the spawn completion, adoption, reads after exit until removal, and unknown or removed services. FakeCore uses script data for the bounds, consistent with Core section 11.

5. **Behavior and user rulings pass.** The amendment states read behavior without prescribing storage or transport mechanisms. It adds no numeric bound. The read posts no event and takes no slot, consistent with the synchronous service calls and AM-4. It agrees with ruling R-9.1 and the amendment path.

This ACCEPT applies to the contract amendment at the matched hash. It does not accept feature code or prove that conformance tests pass. The steward owns the manifest update.

## ui-contract 1.4 erratum 1, candidate 1 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-09-30.

Candidate: `amendments/ui-contract-v1.4-erratum-1-candidate1.md`, commit `eb0ba64`.
SHA-256: `8065db8124d1dd6bc0b41eb22926d4f6d5d4e4583ee4cdb77b8b7314fd6b0b0f`.

Review authority: `docs/BUILD.md`, `README.md`, `frozen/final9/MANIFEST.md`, UI 1.0–1.4, and steward ruling R-10.

1. **Frozen consistency passes.** UI-NV-8 corrects only the error-tone wording in UI-AC-7(a), UI-NV-3, and UI-NV-5. `danger` belongs to UI-CF-4's existing tone set and is already available for confirmation under UI-AC-3. UI-SA-5 keeps its separate `warning` tone.

2. **The correction closes the stated gap.** Failure text nodes have one explicit tone token. The `error` role segment and error prop retain their meanings under UI-RZ-2 and UI-NV-2. Error shapes, node placement, and state changes under UI-NV-3–5 remain unchanged.

3. **No version change is required.** The erratum resolves contradictory wording against an existing closed vocabulary. It adds no tone, removes no valid tone, and changes no established valid tone meaning under UI-VR-1. `ui_contract_view: "1.4"` and `ui_1_4` remain appropriate.

4. **Conformance traceability passes.** UI-NV-8 has the test ID `conf::ui_nv_8_error_tone_is_danger`. The correction gives every renderer the same token under UI section 16.

5. **Scope and user rulings pass.** The erratum states output behavior, adds no numeric bound or implementation mechanism, and follows ruling R-10 and the amendment path. No frozen file changes.

This ACCEPT applies to the erratum at the matched hash. It does not accept feature code or prove that conformance tests pass. The steward owns the manifest update.

## Core Amendment 5, candidate 3 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-01.

Candidate: `amendments/core-contract-v1.17-amendment-5-candidate3.md`, commit `49f594f`.
SHA-256: `d34552cdadc2d885ca1fea273bac1c684b96aebfb13794ce6a707ca88c70b69e`.

The steward superseded candidates 1 and 2 during review. Candidate 3 preserves uncertain registry-write injection and names the worker by role. Those corrections do not resolve the findings below.

Review authority: `docs/BUILD.md`, the Core contract and accepted amendments in `frozen/current`, section 13, and steward rulings R-1, R-5, and R-7. The adopted hybrid direction is not disputed.

1. **A5-2 promises operations that permutation cannot provide.** It must vary class K coalescing and route output chunking while never inventing, altering, or dropping an event or byte. Coalescing retains the latest keyed value under EV-6; it does not merely reorder existing events. Changing route frame boundaries also requires framing work, and those worker-owned frames do not pass through CoreApi. Assign these variations to the injected producer, scheduler, and transport edges. Keep the decorator limited to permitted delivery variations. Name conformance coverage for every retained section 11 variation, including spurious wakes and partial progress.

2. **A5-2 does not preserve state and capacity when it delays events.** EV-5(a) releases an operation slot when the host polls Completed. A decorator that polls real Core, buffers Completed, and later returns it to the host releases that slot too early. EV-5(b) also makes a state transition and its event one atomic step. Delaying an event alone cannot delay the underlying state seen by reads. Define how seeded variation occurs before the real transition or consumption when these guarantees require it. Preserve OR-1, admission ordering, capacity accounting, and TM-3's due-deadline rule. Cover a delayed completion followed by admission, and a state read during delayed event delivery.

3. **A5-3 does not specify the timing and effects of a scripted refusal.** A code allowed by A2-1 is insufficient. ER-0 distinguishes synchronous refusals from asynchronous failures. RegistryFailed is asynchronous; returning it directly from begin would violate ER-0. Injecting its completion would also conflict with A5-2's prohibition on invented completions. Define synchronous refusal behavior separately from failures after admission. Preserve operation IDs, events, ownership, effects, and capacity under ER-0 and AM-4. Use real edge failures for asynchronous outcomes, or name and constrain a separate mechanism. Add conformance coverage for both timings.

Every new clause has test IDs. The candidate adds no numeric bound and keeps the testkit outside the facade and C ABI. The rejection concerns preservation of the frozen test guarantees, not the decision to use real Core with injected edges.

## Hub→client Amendment 5, candidate 1 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-01.

Candidate: `amendments/hub-client-contract-v1-amendment-5-candidate1.md`, commit `bc94c71`.
SHA-256: `5ac1c3226cc5e8cfbcc1b07ff8ad1866ef23b65dbca1d5945e4914626fb1b067`.

Review authority: `docs/BUILD.md`, the Hub→client contract and accepted amendments in `frozen/current`, TB-0, and steward rulings R-1, R-5, and R-7. The adopted hybrid direction is not disputed.

1. **A5-3 excludes necessary adversarial server transcripts.** It requires every transcript served by the player to pass against the real Hub. A client test for an unknown server frame or a future operation state cannot require the current real Hub to generate that value. EN-6 and the operation row require client handling of such inputs. Separate shared semantic transcripts from client-only adversarial transcripts. Shared semantics must pass against the real Hub. Client-only inputs must have clause authority and validate the client's required response without requiring the Hub to emit them. Keep recordings outside the oracle. Add explicit conformance coverage for this distinction.

2. **A5-4 cannot vary every listed condition by permuting transport frames.** A different resync snapshot cut requires the producer to select a different valid source state under WT-3. Moving already-produced frames does not change that cut safely. Moving shutdown delivery is also different from changing when the Hub shuts down outstanding operations. Define which variations belong to the injected scheduler and producer edges, and which only change delivery. Preserve baseline and batch atomicity, stream order, frame boundaries, and shutdown responses. Apply the same distinction to the player through scripted alternatives with clause-cited expectations. Add coverage proving these variants remain valid, rather than merely reordered.

Every new clause has test IDs. The candidate adds no numeric bound. It preserves real bindings, the client's own adapter, one Rust/wasm player, and the HP frame players for external plugin authors. The rejection concerns the oracle and seeded coverage, not the hybrid direction.

## Core erratum 1, candidate 1 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-01.

Candidate: `amendments/core-contract-v1.17-erratum-1-candidate1.md`, commit `ca65dff`.
SHA-256: `0a7578bb6eabe7c1b2eca6f673965d13eddb7cc59e1332e3187b1f9b62a37709`.

1. **Frozen consistency passes.** E1-1 changes only section 13's worker-name sentence. LC-1 already selects the worker through `worker_path`. Hub→client RT-3 retains the Botster install name `botster-worker` and its sibling-only lookup rule.

2. **The correction closes the naming conflict.** A standalone C embedder supplies the path to the worker built from Core. Core requires no basename. The packaged worker remains part of the C deliverable, and section 13's other requirements remain unchanged.

3. **Conformance traceability passes.** E1-1 has the test ID `conf::e1_1_worker_found_by_host_supplied_path`. The historical source filename remains non-normative and unchanged.

4. **Scope passes.** The erratum states the host-to-Core agreement, adds no numeric bound, and follows the amendment path. It changes no frozen file. Acceptance is independent of the rejected Amendment 5 candidates.

This ACCEPT applies to the erratum at the matched hash. It does not accept feature code or prove that conformance tests pass. The steward owns the manifest update.

## Core Amendment 5, candidate 4 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-01.

Candidate: `amendments/core-contract-v1.17-amendment-5-candidate4.md`, commit `da52e32`.
SHA-256: `72c05f4b5ef8e3808536d23f0a7f56640e2d4b6fa1b66c0c7433ba157b1e89e8`.

Review authority: `docs/BUILD.md`, the Core contract and accepted amendments in `frozen/current`, the accepted Core worker-name erratum, and steward rulings R-1, R-5, and R-7.

1. **The seeded-variation findings are resolved.** A5-2 assigns every section 11 variation to its source. Real Core performs coalescing and framing. The scheduler controls progress before transitions; poll_events controls consumption without buffering already-consumed events. Spurious wakes have a separate edge.

2. **State and capacity guarantees are preserved.** Completion slots remain reserved until the host polls Completed. State transitions and their events remain atomic. Due deadlines execute in their pump. The candidate names tests for deferred completion capacity, reads during transitions, due deadlines, and every retained variation.

3. **Failure timing is explicit.** A5-3 permits scripted refusals only before admission and only for a call's synchronous codes. Asynchronous failures come from edges and use real Core's completion path. The uncertain registry-write case remains available under AD-7. The candidate preserves ER-0 and AM-4 and names tests for both timings.

4. **The replacement retains the required scope.** The testkit uses the same worker code and real libghostty. It remains outside the facade and C ABI. Core and Hub retain their conformance runs, released-version pins, and named real-process tests. A5-5 names the text replacements, including removal of the fake's size budget. No new numeric bound is added.

5. **Traceability and user rulings pass.** Each behavioral clause has test IDs. The architecture matches the adopted BUILD.md decision. The Hub retains tests with no worker process, PTY, or sleeps. This acceptance does not treat in-process evidence as proof of OS-only behavior.

All findings from the candidate 3 review are resolved. This ACCEPT applies to the matched contract amendment. It does not accept feature code or prove that tests pass. The steward owns the manifest update.

## Hub→client Amendment 5, candidate 2 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-01.

Candidate: `amendments/hub-client-contract-v1-amendment-5-candidate2.md`, commit `da52e32`.
SHA-256: `eb097128f35117e4af7c2dafff9da302af27165a44024dfe309c6c31c12ccb22`.

Review authority: `docs/BUILD.md`, the Hub→client contract and accepted amendments in `frozen/current`, TB-0, steward rulings R-1, R-5, and R-7, and the accepted Core Amendment 5 candidate 4.

1. **The oracle finding is resolved.** Shared semantic transcripts must pass against the real Hub. Client-only transcripts cite the client's obligation and assert only client behavior. The player can supply unknown, future, malformed, or cut inputs without requiring the real Hub to generate them. Recordings remain outside the oracle.

2. **The seeded-coverage finding is resolved.** The real Hub's scheduler produces resync cuts, completion order, route-end timing, and shutdown timing. The player uses clause-cited alternatives. The transport decorator varies delivery while preserving stream order, batches, baselines, frame bounds, and shutdown answers before close. WT-8 correctly identifies cross-stream ordering; the frozen section 12 reference to WT-7 was a citation error.

3. **Client and binding guarantees remain.** The player has one Rust implementation compiled to wasm for TypeScript. It serves the real bindings through each client's own adapter. Scripted faults retain the R-1.4 cases. Internal in-memory transport remains plumbing and cannot satisfy a binding clause.

4. **Replacement scope is explicit.** A5-5 names the section 12 and test-note replacements, including the informative CLI references. Real-binding and acceptance runs remain. HP section 19's frame players remain available to external plugin authors without Hub or Core.

5. **Traceability and user rulings pass.** Each behavioral clause has test IDs, including the two transcript kinds and valid producer and delivery variations. The candidate adds no numeric bound and follows the adopted hybrid architecture and amendment path.

All findings from the candidate 1 review are resolved. This ACCEPT applies to the matched contract amendment. It does not accept feature code or prove that tests pass. The steward owns the manifest update.

## Core Amendment 6, candidate 1 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-01.

Candidate: `amendments/core-contract-v1.17-amendment-6-candidate1.md`, commit `37f083b`.
SHA-256: `8332c662eba22d585b11e919adbbb2587a9f9bd57f9d8f3012400c1f55898289`.

Review authority: `docs/BUILD.md`, manifest final12 in `frozen/current`, the Core base and accepted amendments, and Hub→client RT-2. A6-1 preserves the real service preamble and lane checks while keeping OS-specific proof in named real-process tests.

1. **A6-3 does not cover worker loss during Remove.** It defines no reachable worker as a Lost session with no stray worker found. A worker can also disappear or lose its control link after Remove starts, including after partial deletion or after deletion but before the report arrives. Remove admits Created, Exited, and Lost under LC-7 and has only RegistryFailed as an asynchronous error under A2-1. Define the report and continued teardown for each loss case. `Deleted` must require evidence that every file is gone; `delete_failed, paths` must not imply a complete known list when the worker's report is lost. Preserve AD-6 identity checks before acting on a stray worker. Add conformance coverage for worker loss during cleanup and loss of the cleanup result.

2. **A6-2 and A6-4 do not identify the exact deferred proof obligations.** The phrase “every per-worker feature test that needs an N−1 worker” leaves the ledger set open. `conf::dp_12_unreachable_or_n_minus_1_worker_leaves_focused_unknown` includes an unreachable-current-worker case that remains testable at protocol 1. Enumerate the deferred IDs or named cases, and keep the current-worker branches active. Also distinguish having a previous worker from having a previous worker that lacks a particular capability. The last protocol-1 binary need not lack `focus_report`, which the frozen contract already requires. State how missing-capability cases are proven when that pinned binary supports the capability. Keep the real previous-version adoption obligation and give the deferral and activation rules conformance coverage.

3. **A6-3 prescribes an internal storage mechanism.** “Core records no per-file name in the registry (no registry write per upload)” constrains Core internals rather than the host-to-Core behavior. README.md rule 3 distinguishes agreements between parties from internals; rule 4 requires behavior rather than today's mechanism. State cleanup ownership, evidence, reports, and failures without prescribing the registry's representation or write schedule. If a user ruling requires that mechanism, cite the ruling explicitly. No such authority appears in this candidate.

The candidate names its frozen replacements and gives each behavioral clause test IDs. The protocol number 1 defines version identity, not a new resource bound. The rejection concerns incomplete cleanup outcomes, deferred coverage, and an internal mechanism requirement. This review does not accept or review feature code.

## Core Amendment 6, candidate 2 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-01.

Candidate: `amendments/core-contract-v1.17-amendment-6-candidate2.md`, commit `0aa03e5`.
SHA-256: `b1e674cc0100823bf0acdb7ef0b4bf8bf8cbe9d729b1c80f6bb4d7ad935676d3`.

The review uses the same authority as the candidate 1 review. A6-3 now distinguishes complete deletion, exact failed paths, and an unknown outcome. It covers worker and report loss, requires trusted evidence, and leaves the storage mechanism to Core. A6-2 enumerates the deferred set and its activation conditions. One inconsistency remains.

1. **A6-2 gives the active unreachable-worker case the wrong cause.** It says the testkit's process edge “ends the worker” to prove `conf::dp_12_unreachable_or_n_minus_1_worker_leaves_focused_unknown`. AD-2 distinguishes WorkerGone, with no process of that identity, from WorkerUnreachable, with an alive worker and no connection within the deadline. The frozen FocusChanged rule explicitly names `Lost(WorkerUnreachable)` for this unknown-focus case. Ending the worker does not prove that condition. Keep the worker alive and prevent its connection through the deadline. Require the case to reach `Lost(WorkerUnreachable)` and verify that focus remains unknown. Retain the current test ID and its active status.

The service-lane edge remains consistent. Every behavioral clause has test IDs, and the candidate adds no resource bound. The remaining finding concerns the test subject, not the adopted cleanup report or the decision to defer unavailable previous-version subjects. This review does not review feature code.

## Core Amendment 6, candidate 3 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-01.

Candidate: `amendments/core-contract-v1.17-amendment-6-candidate3.md`, commit `2e97268`.
SHA-256: `572101467891dfc3c60ce7496ba458ff614e8fd9e0a4b3abac9caae30d4bbf5d`.

Review authority: `docs/BUILD.md`, manifest final12 in `frozen/current`, the Core base and accepted amendments, and Hub→client RT-2.

1. **The remaining unreachable-worker finding is resolved.** A6-1 withholds the control link while the worker stays alive. A6-2 requires Lost(WorkerUnreachable) and unknown focus under the existing active DP-12 ID. This preserves AD-2's distinction from WorkerGone and the frozen focus rule.

2. **The service-lane edge preserves the accepted test architecture.** The testkit uses real preamble, lane, and epoch checks. Process outcomes use the existing edge. OS identity, orphan grace, parent-death behavior, descendant enumeration, and group termination retain named real-process proof. A4-1's report remains available.

3. **The protocol and deferred coverage are explicit.** Protocol 1 has no fabricated predecessor. The candidate names deferred IDs, activation conditions, and the unavailable previous-worker focus case. Current-worker coverage remains active. Compatibility and Unknown behavior remain governed by AD-4, with the named first-protocol correction.

4. **Cleanup outcomes are complete and observable.** Remove reports trusted complete deletion, exact failed paths after completed cleanup, or an unknown outcome without path claims. Worker loss, result loss, and failed authentication do not produce false success. The candidate preserves the remaining teardown order and RegistryFailed error. It leaves file discovery to Core and preserves the host's RT-2 directory ownership.

5. **Scope and traceability pass.** A6-4 names the frozen replacements, including the Remove result. Each behavioral clause has test IDs, including withheld control-link behavior and cleanup loss cases. The candidate adds no resource bound and changes no frozen file.

All findings from candidates 1 and 2 are resolved. This ACCEPT applies to the matched contract amendment. It does not accept feature code or prove that tests pass. The steward owns the manifest update.

## Core erratum 2, candidate 1 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-01.

Candidate: `amendments/core-contract-v1.17-erratum-2-candidate1.md`, commit `3eab5a4`.
SHA-256: `6b1f5af5b137f2ed02c86cbfd042679fe49e4d14ec6220389e5f8481213fc968`.

Review authority: `docs/BUILD.md`, manifest final13 in `frozen/current`, the Core contract and accepted amendments, and the pinned libghostty audit.

1. **E2-3 suppresses an actual mode change.** Its second bullet says that a set followed by a reset in one model step produces no change or event. If the mode was already set at the last posted observation, that sequence leaves it reset. The final flags differ from the last posted flags. The first bullet therefore requires `ModesChanged`, while the second bullet forbids it. Condition suppression on the final `ModeFlags` equaling the last posted value. Add conformance coverage for both initial states: initially reset with no final change, and initially set with a final change. Keep the full latest flags and unchanged `Output` bytes in both cases.

2. **E2-1 and E2-2 fit the named authority.** The title correction matches section 6.2 and the model's tracked title state. The `other_modes` correction distinguishes tracked modes from unrecognized mode numbers. Both preserve OU-12 and the single terminal oracle required by R-7. Each clause has test IDs. The candidate adds no resource bound.

The rejection concerns the unconditional suppression rule in E2-3. This review does not review feature code or prove that tests pass.

## Core erratum 2, candidate 2 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-01.

Candidate: `amendments/core-contract-v1.17-erratum-2-candidate2.md`, commit `d0a46f6`.
SHA-256: `d0eb896737ecc54e11baf606afd784b2ee3f6555ff34c2840228dd10537a09bb`.

Review authority: `docs/BUILD.md`, manifest final13 in `frozen/current`, the Core contract and accepted amendments, and the pinned libghostty audit.

1. **E2-3 resolves the rejected contradiction.** It suppresses an event only when the final `ModeFlags` equal the last posted value. Otherwise it posts one event with the final flags. It gives both initial-state examples and separate conformance IDs for unchanged and changed results. This preserves the latest-state behavior of section 6.2 and EV-6.

2. **E2-1 and E2-2 remain consistent.** The title correction matches section 6.2. `other_modes` contains tracked model modes without a normative field. Unrecognized mode numbers produce no model state. These readings preserve the single terminal oracle under R-7 and unchanged bytes under OU-12.

3. **Scope and traceability pass.** The candidate names the three corrected readings and leaves other clauses unchanged. The write-start rule retains current model modes under IN-8, IN-9, and AM-2. Each behavioral clause has conformance IDs. The candidate states observable behavior and adds no resource bound.

The candidate 1 finding is resolved. This ACCEPT applies to the matched contract erratum. It does not accept feature code or prove that tests pass. The steward owns the manifest update.

## Core Amendment 7, candidate 2 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-01.

Candidate: `amendments/core-contract-v1.17-amendment-7-candidate2.md`, commit `635fc21`.
SHA-256: `5fb18f25f85b1b38765b369f22d52cc064f035777ff3aff6e6ab471fb69e20d1`.

Review authority: `docs/BUILD.md`, manifest final14 in `frozen/current`, Core OU-1, EV-8, AM-4, section 9.3 and A2-1, codec TS-1 and TR-3, and steward ruling R-9.2.

1. **A7-1 leaves the query deadline range incomplete.** EV-8(b) requires `query_deadline` from 1 ms through `CoreLimits.max_query_deadline` when the route answers queries. The candidate lists only absent and zero values. A positive value below 1 ms or above the maximum still violates the frozen range. The general refusal rule and the stated list of enforced rules leave these cases ambiguous. Include the full existing range and conformance IDs for both invalid sides and valid boundaries. Do not add a new bound.

2. **A7-1 changes the attach result without naming that change.** It refers to “the `RouteLimits` that `attach` returns.” OU-1 defines `attach(...) -> RouteId`. TS-1 reports route limits in the client attach response, and R-9.2 defines the effective chunk size without changing the Core call's result. State where the existing protocol reports the effective value. If a new Core result or read is needed, name that contract change and give it conformance coverage. A7-2 currently names no such replacement.

3. **The remaining refusal behavior fits existing authority.** Sync `InvalidInput{field}` before route reservation follows AM-4. The screen, frame, connect deadline, and tag bounds come from OU-1. Reducing the WebRTC chunk size follows R-9.2 and TR-3. These clauses have test IDs and add no resource bound.

This REJECT concerns incomplete range coverage and an unnamed result change. It does not review feature code or prove that tests pass.

## Core Amendment 7, candidate 3 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-01.

Candidate: `amendments/core-contract-v1.17-amendment-7-candidate3.md`, commit `e789bdb`.
SHA-256: `0c6af9595216de4d648ef8fea96c6a443835187e20e61c87944faf1d9c09fa30`.

Review authority: the candidate 2 review authority, including the complete OU-1 text.

1. **The lower query deadline finding remains open.** A7-1 now names the full range, but its refusal list still says “absent, zero, or above the maximum.” A positive `Duration` below 1 ms also violates EV-8(b). The listed tests cover absent, zero, above maximum, and valid boundaries. They do not name this positive value below the minimum. Replace “zero” in the range rule with “below 1 ms,” and give that case a conformance ID. Retain the absent case and both valid boundaries. The correction uses the existing bound.

2. **Correction to candidate 2 finding 2.** That finding was incorrect. Although OU-1's signature says `-> RouteId`, the same clause explicitly says “The attach result returns `RouteLimits`.” I overlooked that sentence. The candidate 2 reporting claim therefore had existing authority. I withdraw that finding. Candidate 3's rule on sent chunk sizes also fits R-9.2 and TR-3. Existing reporting requirements remain unchanged.

3. **The other changes pass.** The upper query deadline and valid boundaries now have test IDs. The remaining sync refusal rules preserve AM-4 and the named bounds. The candidate adds no resource bound and changes no frozen file.

This REJECT concerns only the positive query deadline below 1 ms and its missing conformance case. It does not review feature code or prove that tests pass.

## Core Amendment 7, candidate 4 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-01.

Candidate: `amendments/core-contract-v1.17-amendment-7-candidate4.md`, commit `de54498`.
SHA-256: `1d1a40c80cff6253faa0878602d6079c4669ed75f1fd12280cc77b3b9a4b8bcf`.

Review authority: `docs/BUILD.md`, manifest final14 in `frozen/current`, Core OU-1, EV-8, AM-4, section 9.3 and A2-1, codec TS-1 and TR-3, and steward ruling R-9.2.

1. **The remaining query deadline finding is resolved.** A7-1 explicitly refuses absent values, values below 1 ms including zero, and values above the existing maximum. The added conformance ID covers a positive value below 1 ms. Existing IDs cover the other invalid cases and both valid boundaries.

2. **The refusal behavior closes the stated gap.** Both attach calls refuse invalid options synchronously with `InvalidInput{field}` before route reservation. No route, event, or route id results from refusal. The named screen, frame, deadline, and tag bounds come from the frozen contract. Valid boundary values remain valid.

3. **The transport rule preserves existing authority.** An excessive requested WebRTC chunk size causes no refusal. Sent chunks stay within the requested size and the negotiated transport maximum, as R-9.2 and TR-3 require. Existing results and reads remain unchanged. The withdrawal of candidate 2 finding 2 remains in effect.

4. **Scope and traceability pass.** A7-2 names the changed text and the existing DP-3 assertion. Each behavioral clause has conformance IDs. The candidate states behavior, adds no resource bound, and changes no frozen file.

All remaining findings are resolved. This ACCEPT applies to the matched contract amendment. It does not accept feature code or prove that tests pass. The steward owns the manifest update.

## Core Amendment 8, candidate 1 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-01.

Candidate: `amendments/core-contract-v1.17-amendment-8-candidate1.md`, commit `16fcdad`.
SHA-256: `f0a0e3d9b3eaba5e69841e3b568031b901b2b31b92f7dbde2862ef56a5e65768`.

Review authority: `docs/BUILD.md`, manifest final14 and accepted Core Amendment 7, Core ST-6, ST-6b, OU-9, 9B, 9.3, AM-3, AM-4 and A2-1, R-7, HC TS-11, and the pinned libghostty audit.

1. **A8-1 retains an incomplete resume condition.** Successful completion can reduce a reservation from `max_snapshot_bytes` to `total_bytes`. Failed completion releases the reservation without creating a capture to release or expire. Either can make a refused capture admissible. The candidate nevertheless says the resume condition stays release or expiry only. Define the retry condition for restored byte capacity, including these completion cases. State how the host observes that condition and preserve AM-4's completion polling rule. Name the replacement to 9.3's `CaptureLimit` resume text. Add conformance coverage for refusal followed by a smaller successful capture and by a failed capture.

2. **A8-2 does not name the full change to ST-6b.** ST-6b requires the resume invariant at every cut and permits a ground-state cut as an alternative. Its existing exclusion text concerns state the model does not hold. A8-2 also permits immediate failure when the model holds pending state that the format cannot carry. A8-3 names no replacement or qualification to ST-6b for this new failure case. State that qualification explicitly, including how the existing every-byte-offset conformance test handles a failed capture. Keep the invariant exact for every offered snapshot. Add a conformance ID for the separately stated resync failure, or explicitly name a case under an existing ID that proves it. The baseline test alone does not identify that case.

3. **The proposed outcomes use existing authority.** Reserving the existing snapshot maximum can preserve the retained-byte bound without a new number. `SnapshotTooLarge` already completes captures asynchronously and closes routes with a typed reason. The candidate preserves exactly one completion and rejects partial snapshots. Its test IDs cover reservation, completion, failure, initial baseline failure, and successful resume behavior.

This REJECT concerns capacity resumption, the scope of the ST-6b qualification, and resync test coverage. It does not review feature code or prove that tests pass.

## Core Amendment 8, candidate 4 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-01.

Candidate: `amendments/core-contract-v1.17-amendment-8-candidate4.md`, commit `0d0bd7e`.
SHA-256: `b2c545f2b16b213ac224486fa63f768f900be9848d2140a05d9b43a5228a2485`.

The steward withdrew candidates 2 and 3 before the verdict landed. This review applies to candidate 4.

Review authority: the candidate 1 review authority, including EV-5(a).

1. **A8-1's held-byte formula conflicts with its polling rule.** The formula counts `total_bytes` for a completed capture and the maximum only for a capture in progress. The later rule retains the maximum until the host polls `Completed`. Between completion and polling, those rules give different totals. A failed operation also has no open capture, but its reservation must remain until polling. Define held bytes as retained completed captures plus reservations for admitted operations whose completion has not been polled. Distinguish those sets without counting a successful capture twice. Test successful and failed completion before polling as well as after polling.

2. **The ST-6b qualification excludes the existing oversized-snapshot outcome.** A8-2 first permits `SnapshotTooLarge` when the encoded snapshot exceeds `max_snapshot_bytes`. It later says a snapshot is not offered “only” when the model cannot carry pending sequence state. Its test rule also fails every refusal within that continuation limit. A ground-state snapshot can exceed the byte bound with no pending sequence at all. Include both failure grounds in the qualification and test rule. Require evidence for the applicable bound, and preserve exact resume for every offered snapshot. Keep ordinary oversized-snapshot coverage as well as continuation-limit coverage.

3. **The candidate resolves the prior missing scope and test items.** It names the `CaptureLimit` resume replacement and the ST-6b qualification. It adds retry tests and a resync failure ID. These additions use existing outcomes and bounds. The remaining findings concern contradictions within the revised rules.

This REJECT does not review feature code or prove that tests pass.

## Core Amendment 8, candidate 5 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-01.

Candidate: `amendments/core-contract-v1.17-amendment-8-candidate5.md`, commit `7bf8d0d`.
SHA-256: `c6918a9bf19d739422abc775441a9aa42811bbe044f1a18cd39f20215e2a8006`.

Review authority: `docs/BUILD.md`, manifest final14 and accepted Core Amendment 7, Core ST-6, ST-6b, OU-9, 9B, 9.3, AM-3, AM-4, EV-5(a) and A2-1, R-7, HC TS-11, and the pinned libghostty audit.

1. **The reservation accounting is consistent.** An admitted capture counts the existing maximum until the host polls its completion. After polling, a retained successful capture counts its actual bytes; a failed operation counts nothing. Each capture counts once. Added test IDs cover success and failure before and after polling. Release and expiry retain their existing behavior.

2. **The snapshot qualification preserves both failure grounds.** A8-2 and the named ST-6b replacement permit `SnapshotTooLarge` for an encoded snapshot beyond the byte bound or pending state that cannot be carried. Every offered snapshot still meets the exact resume invariant. The every-byte-offset corpus fits the byte bound and injects no resource failure. It therefore permits only the stated continuation-limit failure. Existing oversized-snapshot coverage remains active.

3. **The outcomes and conformance coverage fit existing authority.** Captures complete asynchronously exactly once. Failed captures free their reservation under A8-1. Baseline and resync failures close the route with the existing typed reason and no partial baseline. Named test IDs cover each outcome. The stated continuation limit does not create a new numeric bound or a new CoreLimits field. R-7's single terminal oracle remains unchanged.

4. **Scope and traceability pass.** A8-3 names the changes to the retained-byte bound, capture rule, resume condition, snapshot invariant, error meaning, route outcome, and existing capture-bound assertion. The candidate states behavior and preserves all other frozen clauses. Different encoded sizes at one model revision do not weaken the frozen-state guarantee.

All remaining findings are resolved. This ACCEPT applies to the matched contract amendment. It does not accept feature code or prove that tests pass. The steward owns the manifest update.

## Core Amendment 9, candidate 1 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-02.

Candidate: `amendments/core-contract-v1.17-amendment-9-candidate1.md`, commit `063e962`.
SHA-256: `860604ba6a54d42d9dc5dd9e00fed9e4ba2bde1c2d6be44b420e4b0fdce8b042`.

Review authority: `docs/BUILD.md`, manifest final16 in `frozen/current`, Core DP-3, OU-1, A3-1, A5-1, A5-2 and A7-1, and codec A.4 and TS-1.

1. **A9-1 closes the attached-frame gap.** It derives the lower frame bound from the route's own selected features, format, and applied limits. This preserves DP-3's frame-size measurement and A7-1's synchronous refusal before reservation. Tests cover values below and equal to the derived bound. The rule adds no fixed numeric bound.

2. **A9-2 justifies the test withdrawal.** The required limits object in `attached` exceeds the worst-case minimum notification report. A9-1 therefore prevents an attached route from reaching the ineligible branch. The guard remains unchanged. The replacement test compares against the worst-case minimum for each route's `route_queue_bytes`, preserving the widest count and required `truncated` field in that comparison. Existing shortening, count, and inflated-charge tests remain active.

3. **A9-3 preserves the real worker path.** The testkit scripts or seeds only the compression choice that DP-3 leaves open. Negotiation and minimum payload size still determine eligibility. Real code produces the compressed bytes, enforces bounds, and accounts for the queue. No edge changes terminal bytes or replaces produced frames. Named tests cover the decision point and ineligible payloads.

4. **Scope and traceability pass.** A9-4 names the lower-bound addition, the test replacements, and the scheduler extension. Every behavioral clause has conformance IDs. The candidate preserves the frozen codec and other contracts. Its derived bounds use existing frame content and accounting authority.

This ACCEPT applies to the matched contract amendment. It does not accept feature code or prove that tests pass. The steward owns the manifest update.

## Core erratum 3, candidate 1 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-02.

Candidate: `amendments/core-contract-v1.17-erratum-3-candidate1.md`, commit `5cdb45e`.
SHA-256: `58f7cf96296cdbade06ddcdc4385820e5b50c83d2e28a07912bdf1c643ee9057`.

Review authority: `docs/BUILD.md`, manifest final17 in `frozen/current`, Core TM-3 through TM-6, EV-5, 9B, ST-6, LC-5, and A5-2.

1. **E3-1 does not preserve mandatory-queue parking.** Item 2 says an atomic transition runs whenever event budget remains. EV-5(b) also requires room in the mandatory queue. Item 3 says carried steps run first, but a carried transition can encounter that full queue. TM-6 requires that blocked work remain parked, with no runnable `more`, no deadline, and no wake loop until polling frees room. State the separate conditions for budget deferral and mandatory-queue blocking. Specify that a carried step which encounters a full queue becomes parked under EV-5(b) and TM-6. It must not stop other runnable work. Add conformance coverage for a carried deadline transition encountering a full mandatory queue.

2. **The unqualified due-order guarantee conflicts with item 1.** Suppose an earlier event-bearing deadline is carried because the pump has no event budget. A later capture expiry becomes due in that same pump. Item 1 requires expiry immediately, but the earlier effect remains unprocessed. The statement that no effect is reordered and the unchanged TM-5 rule cannot both hold. State the ordering exception for effects without events, including effects that run while an earlier transition is parked. Name the TM-5 qualification if its scope changes. Add a mixed-deadline test that proves the chosen order without delaying expiry or the required kill.

3. **The budget correction otherwise fits existing authority.** Whole-step deferral preserves atomic events. Runnable budget deferrals can use `more` and the level-triggered wake. Capture expiry and the stop-grace kill retain their required effects. The candidate gives test IDs for its four rules and adds no numeric bound.

This REJECT concerns queue blocking and the scope of deadline ordering. It does not review feature code or prove that tests pass.

## Core erratum 3, candidate 2 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-02.

Candidate: `amendments/core-contract-v1.17-erratum-3-candidate2.md`, commit `9f20fd5`.
SHA-256: `814f06938cc993a2507ba734d51eaea3afaf20d11df210d147fb183cbea2444a`.

Review authority: the candidate 1 review authority.

1. **Due order still blocks an otherwise runnable deadline.** Item 5 requires all event-posting steps to keep due order among themselves. An earlier stall close can be parked on a full mandatory queue while a later `Silent` has event budget and needs no mandatory slot. Item 6 requires that `Silent` in the first pump with budget, but item 5 prevents it until the stall close runs. Define due order among runnable deadline steps. Permit a later runnable step to bypass a parked step, while preserving the parked step's due time and priority when it becomes runnable. Qualify the carried-step priority in item 3 accordingly. Add a test with an earlier parked mandatory transition and a later runnable class K deadline.

2. **The candidate resolves the other prior findings.** Item 4 distinguishes mandatory-queue room from event budget and preserves parking and wake on room. Item 5 permits immediate effects without events to overtake deferred transitions. Both cases have added conformance IDs. No numeric bound changes.

This REJECT concerns ordering among parked and runnable event-posting steps. It does not review feature code or prove that tests pass.

## Core Amendment 10, candidate 1 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-02.

Candidate: `amendments/core-contract-v1.17-amendment-10-candidate1.md`, commit `350e40a`.
SHA-256: `9fc505025fdf7292dd30b919684ddd3bb78257ebbae3b26eb4a528e6d656b768`.

Review authority: `docs/BUILD.md`, manifest final17 in `frozen/current`, Core AD-2, AD-6, SV-6, A5-1 and A6-1, and steward ruling R-14.3.

1. **A10-1 supplies the missing authentication input.** The process edge presents an impostor with a wrong token or InstanceId. Core's real AD-6 check decides the result. The impostor supplies only a handshake answer and never receives a signal. Separate test IDs cover both mismatches.

2. **A10-2 preserves real registry recovery.** The storage edge damages bytes that Core's own encoder produced. Core's real decoder rejects the damaged row, and real recovery produces Lost(RegistryCorrupt). The edge neither defines a second format nor invents the result. The clause has a conformance ID.

3. **Existing abilities cover the other named reasons.** Guardian loss uses the process edge. Epoch exhaustion uses the scripted service and real epoch checks under A6-1 and SV-6. Read-only boundary observers retain R-14.3's conditions and do not add injected behavior.

4. **Scope and traceability pass.** A10-3 names the two edge additions and the existing lost-reasons test. The candidate preserves production authentication, decoding, and recovery behavior. It adds no numeric bound and changes no frozen file.

This ACCEPT applies to the matched contract amendment. It does not accept feature code or prove that tests pass. The steward owns the manifest update.

## Core erratum 3, candidate 3 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-02.

Candidate: `amendments/core-contract-v1.17-erratum-3-candidate3.md`, commit `36845b7`.
SHA-256: `77291006b7ccc89e101917315e2248d8c8a31a5210a3e0120f69ebc63b83ae3b`.

Review authority: `docs/BUILD.md`, manifest final17 in `frozen/current`, Core TM-3 through TM-6, EV-5, 9B, ST-6, LC-5, and A5-2.

1. **The remaining ordering finding is resolved.** Item 5 orders runnable steps and explicitly bypasses steps parked on mandatory-queue room. A later class K Silent therefore runs with available event budget. A parked step retains its due-time position when room returns. The added conformance ID proves the parked-transition and runnable-Silent case.

2. **Deferral and parking remain distinct.** Event budget defers a whole atomic step and keeps runnable work signaled. Missing mandatory-queue room parks the transition under EV-5(b) and TM-6 until polling restores room. Eventless effects still run in their due pump, including capture expiry and the stop-grace kill. The named TM-5 qualification states the ordering exception.

3. **The correction closes the deadline-budget conflict.** Each pump retains the existing event cap. Carried runnable steps have priority, and Silent retains its since value and once-per-idle-period rule. Tests cover budget exhaustion, due order, parking, wake on room, immediate effects, and runnable bypass.

4. **Scope and traceability pass.** The candidate names the readings changed in TM-3, TM-4, TM-5, and A5-2. Each behavioral rule has conformance IDs. It adds no numeric bound, preserves the remaining frozen clauses, and states behavior rather than an internal mechanism.

All remaining findings are resolved. This ACCEPT applies to the matched contract erratum. It does not accept feature code or prove that tests pass. The steward owns the manifest update.

## Core Amendment 11, candidate 1 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-02.

Candidate: `amendments/core-contract-v1.17-amendment-11-candidate1.md`, commit `78ca378`.
SHA-256: `beac85d513a1bc6e464f384b15e2b4f2a8d69b42af79fdbc8ee58db38106274e`.

Review authority: `docs/BUILD.md`, manifest final19 in `frozen/current`, Core AD-6, LC-7, A6-3, A10-1, and A5-1.

1. **A11-1 closes the missing test input.** The impostor can send scripted worker-shaped frames after Core rejects its handshake. Core's real authentication and handling remain the subject. The impostor implements no worker logic. The added ID proves that these frames cause no session state, report, or event changes.

2. **The Remove result preserves A6-3.** An unauthenticated cleanup claim supplies no trusted evidence of deletion. The named Remove case reports outcome_unknown without paths. Teardown continues and frees the session id under LC-7. The existing unauthenticated-result test now has the boundary input it needs. AD-6's prohibition on signalling the impostor remains unchanged.

3. **Scope and traceability pass.** A11-2 names the replacement to A10-1's restriction. The candidate changes only the allowed scripted input and states observable outcomes. Each behavioral clause has a conformance ID. It adds no numeric bound and changes no frozen file.

This ACCEPT applies to the matched contract amendment. It does not accept feature code or prove that tests pass. The steward owns the manifest update.

## Core Amendment 12, candidate 1 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-02.

Candidate: `amendments/core-contract-v1.17-amendment-12-candidate1.md`, commit `811660c`.
SHA-256: `7dfbbe0ac4493b502dd746fd8377e22209856b9611af972365a777f69d54cdff`.

Review authority: `docs/BUILD.md`, manifest final20 in `frozen/current`, Core A5-1, A6-1, A6-3, AD-6, and steward ruling R-14.3.

1. **The worker-loss test does not reach the claimed cleanup condition.** The worker produces and writes a cleanup result before the edge holds it. Cleanup can already be complete at that point. Ending the worker then proves loss while a result is unread, not loss while cleanup is incomplete. A6-3 requires separate coverage for worker loss during cleanup and a lost result. Keep the report hold for result-loss coverage. Give the worker-loss case a script point after cleanup starts but before it completes or produces its result. Core's real cleanup and recovery must still decide the outcome. Name any additional edge ability needed to reach that condition, with a conformance ID. Do not relabel a completed cleanup as incomplete merely because Core has not read its report.

2. **The report hold otherwise fits the boundary model.** The worker's real code writes the report. Releasing it preserves normal handling, and breaking the link can test an unread lost result. The candidate prohibits dropping a report on a continuing reliable link and leaves outcomes to Core. The observer retains R-14.3's conditions. The named replacement and test IDs add no numeric bound.

This REJECT concerns the worker-loss test's subject. It does not review feature code or prove that tests pass.

## Core Amendment 12, candidate 2 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-02.

Candidate: `amendments/core-contract-v1.17-amendment-12-candidate2.md`, commit `b2259ac`.
SHA-256: `bc0f20ae2577ef54ea91ce05828ac7123f10d0b7dc98eab970bce64ea2898a9d`.

Review authority: `docs/BUILD.md`, manifest final20 in `frozen/current`, Core A5-1, A5-2, A6-1, A6-3, AD-6, LC-7, and steward ruling R-14.3.

1. **A12-1b resolves the worker-loss finding.** The scheduler stops real cleanup after it starts and before its result exists. The process edge then ends the worker. The test checks the files to prove partial cleanup. Real recovery reports outcome_unknown without paths, completes teardown, and frees the id. Separate IDs cover the existing worker-loss case and the partial-cleanup report.

2. **A12-1a preserves the distinct result-loss proof.** The worker's real code produces the report. The edge holds it at the control-link boundary, then releases it or breaks the link before Core reads it. It does not drop a report on a continuing reliable link. Real Core decides the outcome, and the observer retains R-14.3's conditions.

3. **Scope and traceability pass.** A12-2 names both additions to the existing edge list. The scheduler changes where real work stops, and the process edge supplies loss; neither invents a result or implements cleanup separately. Each behavior has a conformance ID. The candidate adds no numeric bound and changes no frozen file.

The candidate 1 finding is resolved. This ACCEPT applies to the matched contract amendment. It does not accept feature code or prove that tests pass. The steward owns the manifest update.

## Core erratum 4, candidate 1 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-02.

Candidate: `amendments/core-contract-v1.17-erratum-4-candidate1.md`, commit `5b70cf7`.
SHA-256: `d67542855e4d38ad222e5882d1ec735956de3ce9fa71063b7aaaf5125bcfe5c4`.

Review authority: `docs/BUILD.md`, manifest final21 in `frozen/current`, Core EV-8, AM-2, IN-10, R-7, and codec TS-3 and A.5.

1. **The client cost is exact and observable.** E4-1 counts the received input frame's type byte and JSON body, including base64. It excludes binding delimiting under TS-3. Tests can compute the cost from the actual frame without a second terminal parser or encoder.

2. **The fallback cost uses the real reply.** A shadow fallback has no client frame. Its reservation counts the PTY reply bytes produced by the terminal oracle. The program edge can observe those bytes. The rule adds no numeric bound and preserves R-7.

3. **Admission semantics remain consistent.** The worker reserves the full stated cost at AM-2 and releases it on write completion. E4-1 preserves EV-8's refusal, query retention, and fallback parking rules. It changes no query consumption rule, reply ordering, contiguous-write guarantee, or input revision rule.

4. **Scope and traceability pass.** The candidate names only EV-8(e)'s cost reading. Separate IDs cover client cost, fallback cost, and refusal with a cost one byte above available capacity. The rule states behavior and leaves all other frozen clauses unchanged.

This ACCEPT applies to the matched contract erratum. It does not accept feature code or prove that tests pass. The steward owns the manifest update.

## Hub→plugin Amendment 3, candidate 1 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-02.

Candidate: `amendments/hub-plugin-contract-v1.1.amendment-3-candidate1.md`, commit `9f289fb`.
SHA-256: `77d0d2809ba8ff05e75b3bd795026674b0e8db1b7d5ea579579b82f3d019634f`.

Review authority: `docs/BUILD.md`, manifest final22 in `frozen/current`, HP section 19, NT-1 through NT-10, PM-14, MP-10 and WP-3, R-1.3, Core A5, and HC A5.

1. **A3-1 preserves the adopted test design.** Scripted network, git, catalog, and MCP endpoints supply inputs through the real Hub's injected edges. The real Hub performs the checks, package operations, proxy behavior, and lifecycle handling. The plugin-side FakeHub scripts HostResults and implements no such Hub semantics. This matches R-1.3 and BUILD.md.

2. **The required proofs remain active.** Existing IDs now run against the real Hub with deterministic endpoints and no real network. The Telegram-shaped sequence, unload cancellation, package operations, and MCP operations retain their required outcomes. The real-run portions remain unchanged. The added ID checks the plugin-side player's limited role.

3. **A3-2 justifies the withdrawal.** WP-3's final sentence states the scope of trust in served browser code and makes no further behavior claim. Its withdrawn ID has no observable outcome to test. WP-3's three relay tests remain active, preserving acceptance checks, separation from route bytes and secrets, and denial-only relay failure. The contract's trust statement remains unchanged.

4. **Scope and traceability pass.** A3-3 names the three service readings, section 19's FakeHub reading, and the withdrawn test. Every new behavioral clause has conformance IDs. The candidate adds no numeric bound and changes no frozen file or production behavior.

This ACCEPT applies to the matched contract amendment. It does not accept feature code or prove that tests pass. The steward owns the manifest update.

## Hub→plugin Amendment 4, candidate 1 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-02.

Candidate: `amendments/hub-plugin-contract-v1.1.amendment-4-candidate1.md`, commit `9284edb`.
SHA-256: `7ab9e7a0207f6c5428ac57cf66abebf1fb2164ee127ef0c6bbe46cacb0a8ed4e`.

Review authority: `docs/BUILD.md`, manifest final23 in `frozen/current`, HP SW-1, SW-2, SW-5, SW-6, SW-7 and A2-12, and HC A.9b and H-9's refusal mapping.

1. **The total does not distinguish typing from waiting.** A4-1 sums one write's text for each session with a pending write, then says a typing write also counts. SW-6 permits a typing write and a newer waiting write in the same session. The total must include both, once each. Define the sum over all accepted unfinished writes, including each typing write and each waiting write. A replacement must subtract only the waiting write it replaces; it must retain the typing write's charge. Add conformance coverage for typing plus waiting in one session, and replacement of that waiting write while typing continues. This closes the ambiguity without changing SW-6 or adding a bound.

2. **The remaining refusal rule fits existing authority.** The Hub-wide interpretation uses the existing write_pending_bytes parameter. The new detail code belongs to the backpressured failure already allowed by A2-12. Effect none preserves the old write on refusal. HC maps that failure to busy. The per-write refusal keeps its stated priority. The candidate names its replacements and supplies IDs for total refusal, replacement, and completion capacity.

This REJECT concerns the total and replacement accounting when a session has both a typing write and a waiting write. It does not review feature code or prove that tests pass.

## Hub→plugin Amendment 4, candidate 2 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-02.

Candidate: `amendments/hub-plugin-contract-v1.1.amendment-4-candidate2.md`, commit `0284f7f`.
SHA-256: `5a0a60df1424ba41ab73a38154707927498cbd36eefbc41654d75964b83e822d`.

Review authority: `docs/BUILD.md`, manifest final23 in `frozen/current`, HP SW-1, SW-2, SW-5, SW-6, SW-7 and A2-12, and HC A.9b and H-9's refusal mapping.

1. **The accounting finding is resolved.** The total counts every held safe write once, including both the typing write and waiting write in one session. Replacement subtracts only the waiting write and retains the typing charge. Two added IDs cover these combined states. Moving a waiting write to typing does not remove its charge before completion.

2. **The refusal preserves existing write behavior.** Exceeding the existing Hub-wide parameter gives backpressured with write_pending_full, effect none, and a retryable result. Refusal changes no accepted write. The per-write text_too_long check retains priority. Completion, replacement, cancellation, and abandonment restore the applicable capacity. HC's existing mapping gives busy.

3. **Scope and traceability pass.** A4-2 names the SW-5 reading and the SW-1 and Appendix A refusal additions. Each behavioral clause has conformance IDs. The candidate states behavior, adds no numeric bound, and preserves all other frozen clauses.

The candidate 1 finding is resolved. This ACCEPT applies to the matched contract amendment. It does not accept feature code or prove that tests pass. The steward owns the manifest update.

## Hub→plugin Amendment 5, candidate 1 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-02.

Candidate: `amendments/hub-plugin-contract-v1.1.amendment-5-candidate1.md`, commit `942aadc`.
SHA-256: `63da8f4056085e32c18b41bd2e02f83f3737df6c85c2deecd3c21d12712f7466`.

Review authority: `docs/BUILD.md`, manifest final24 in `frozen/current`, HP EN-1 through EN-4, A2-5, A2-7, WF-6 and Appendix B, and HC DS-1 and DS-2.

1. **The client schema projection remains ambiguous.** A5-1 makes the declared schema validate only entity, but clients receive a flat row with id. EN-1 projects the declaration to clients, and DS-2 requires one declaration. For example, a family schema that forbids additional properties validates an entity with a status field, but rejects the projected row because id is absent from that schema. State how the client KindDescriptor schema validates the flat row, including its fixed string id, while the plugin declaration validates entity only. Preserve one declaration and prohibit a second hand-written schema. Add a conformance ID that checks describe and validates an actual projected row, including a family schema with additionalProperties false. Name the resulting projection reading in A5-2.

2. **The id restriction needs an explicit scope.** A schema that declares “a property named id” could mean any property at any depth. An entity carrying an id member describes the top level. State whether the restriction applies only to the entity's top-level identifier slot or also to nested application objects. A nested referenced resource can have its own id without replacing the row's identifier. Give the chosen scope a conformance case so schema admission and data validation agree.

3. **The record wrapper otherwise closes the layout gap.** Provider records use id and entity. Publication keeps its existing envelope, and merge patches apply to the entity body. The string identifier and flat row follow WF-6. Named tests cover the wrapper, body validation, id rejection, and row values. The candidate adds no numeric bound.

This REJECT concerns schema projection and the scope of the reserved id member. It does not review feature code or prove that tests pass.

## Hub→plugin Amendment 5, candidate 2 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-02.

Candidate: `amendments/hub-plugin-contract-v1.1.amendment-5-candidate2.md`, commit `ccbd656`.
SHA-256: `da3c249e7dd122cb57f6f03f9f0b9f92044f8c99ca439b671ccc6d89c6ef0b1d`.

Review authority: the candidate 1 review authority, including HC's JSON Schema draft 2020-12 requirement.

1. **The projection does not preserve entity validation.** Adding id to properties and required while keeping every other keyword unchanged fails for object constraints. For example, an entity schema with one required status field, additionalProperties false, and maxProperties 1 accepts the entity. Its projected flat row has status and id, so the unchanged maxProperties rejects it. A nested allOf branch that forbids additional properties can also reject id when only the root properties change. State the observable equivalence: the client schema accepts a flat row exactly when its fixed id is valid and the remaining entity body satisfies the declared schema. Preserve one declaration, but do not require unchanged keywords when that breaks their meaning. Add conformance coverage for maxProperties and a composed object schema, in addition to additionalProperties false. Do not silently restrict the previously allowed schema language.

2. **The reserved-id scope finding is resolved.** The candidate reserves the entity's top-level id member and permits nested application objects to carry id. The added nested-id test covers that distinction. The record wrapper, publication envelope, patch target, and named artifact changes retain their prior coverage. No numeric resource bound is added.

This REJECT concerns the correctness of the generated client schema. It does not review feature code or prove that tests pass.

## Hub→plugin Amendment 5, candidate 3 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-02.

Candidate: `amendments/hub-plugin-contract-v1.1.amendment-5-candidate3.md`, commit `3999055`.
SHA-256: `347f020fc27878e0b949f626328767cbd5c02462d35317526461af999adc6a50`.

Review authority: `docs/BUILD.md`, manifest final24 in `frozen/current`, HP EN-1 through EN-4, A2-5, A2-7, WF-6 and Appendix B, and HC DS-1 and DS-2.

1. **The schema finding is resolved.** The client descriptor retains the entity declaration unchanged and identifies id as a fixed string core field. The stated validation equivalence checks id separately and validates the row without id against the declaration. This preserves object constraints and composed schemas without a second hand-written schema. Added IDs cover the descriptor, maxProperties, and composed closed schemas. The additionalProperties test remains active.

2. **Record and identifier behavior is consistent.** Provider records use id and entity. Publication retains its existing envelope, and merge patches apply to the entity body. Only the entity's top-level id is reserved; nested application objects may carry id. Tests cover both restrictions and nested members. Clients receive the stated flat row.

3. **Scope and traceability pass.** A5-2 names the handler-result shape, EN-1 reading, plugin-kind readings in DS-1 and DS-2, and generated artifacts. Each behavioral clause has conformance IDs. The candidate adds no numeric bound and preserves the other frozen clauses.

All remaining findings are resolved. This ACCEPT applies to the matched contract amendment. It does not accept feature code or prove that tests pass. The steward owns the manifest update.

## Lua binding r9 Amendment 1, candidate 1 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-02.

Candidate: `amendments/hub-plugin-lua-binding-v1.1-r9-amendment-1-candidate1.md`, commit `e5ce02d`.
SHA-256: `6426ede664def95914eb15ccde550a59fc6e7eb123b220fc7f8f649967990083`.

Review authority: `docs/BUILD.md`, manifest final25 in `frozen/current`, Lua LB-3, LB-6, LB-8, LB-9 and LB-10, HP LM-4, A2-5 and A2-8.

1. **LBA1-1 resolves the constructor exception.** json.array returns the array itself. json.empty_array is a read-only value, and mutation raises as a programming error. The mutable constructor remains available. The null sentinel and its encoding remain unchanged. Named IDs cover array encoding, the direct result, and read-only behavior.

2. **LBA1-2 defines the conversion results.** Base64 and JSON conversions retain the LB-3 envelope and place the bare result in value. Binary data remains a Lua byte string. Decoded null retains the sentinel. Crypto retains its named result tables. Tests cover the conversion shapes and base64 round trip.

3. **LBA1-3 preserves the handler wire contract.** Lua descriptor fields are flat alongside the handler metadata and callback. entity_kind avoids collision with the handler kind and maps to descriptor.kind on the wire. Existing handler kinds and payloads remain unchanged. Unknown fields and missing required fields fail the load. The descriptor tests and added entity-provider ID cover these rules.

4. **Scope and traceability pass.** LBA1-4 names the affected helper and descriptor readings. The candidate states public Lua behavior, adds no numeric bound, and changes no frozen file or Hub wire shape.

This ACCEPT applies to the matched contract amendment. It does not accept feature code or prove that tests pass. The steward owns the manifest update.

## Hub→plugin Amendment 6, candidate 1 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-02.

Candidate: `amendments/hub-plugin-contract-v1.1.amendment-6-candidate1.md`, commit `b4cfcba`.
SHA-256: `2f15ce7fe1c1d3a1a56814797a4216cdd9cd1c119cc0fe385b3e0c3be6ed3b90`.

Review authority: `docs/BUILD.md`, manifest final26 in `frozen/current`, HP EN-2, EN-3, EN-4, EN-9 and A2-7.

1. **The status decision needs an admission-time condition.** The candidate defines accepted as a promise that every current subscriber will receive the change in order. EN-3 permits a later overflow, refusal, or handler failure to replace missed changes with reset and baseline recovery. An admission result cannot guarantee future delivery. The candidate also leaves the observation point open when this publish itself creates the first gap: checking before enqueue gives accepted, while checking after enqueue gives a gap status. Define the exact point at which the Hub evaluates gap state, including gaps caused by this publish. Define accepted by that state, without adding a future-delivery promise. Add a conformance case where there is no prior gap and this publish triggers overflow. Preserve EN-3's recovery and EN-9's one shared family projection.

2. **The other status distinctions fit existing authority.** Store-projected recovery uses committed collection state; computed recovery obtains a provider snapshot. Both admitted statuses require no plugin action under A2-7. Existing tests plus the added IDs cover an already-pending gap and no gap. The stimulus uses the real Hub's scheduler and queue, and the candidate adds no numeric bound.

This REJECT concerns the status observation point and the accepted delivery promise. It does not review feature code or prove that tests pass.

## Hub→plugin Amendment 6, candidate 2 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-02.

Candidate: `amendments/hub-plugin-contract-v1.1.amendment-6-candidate2.md`, commit `b1b41f0`.
SHA-256: `eca4d97b3fd270ed7370e54c7fef3f39e47a277c67205490f350938dedaf96bc`.

Review authority: `docs/BUILD.md`, manifest final26 in `frozen/current`, HP EN-2, EN-3, EN-4, EN-9 and A2-7.

1. **The status observation finding is resolved.** The Hub evaluates subscriber gap state at admission after offering the change to all current subscribers. This includes a gap caused by that publish. Added coverage proves the current-publish overflow case.

2. **Accepted now states admission behavior.** It means no gap is pending at that point and the change is queued in order for each subscriber. It makes no future-delivery promise. Later gaps still require EN-3 reset and baseline recovery. A separate test covers that distinction.

3. **Recovery statuses preserve existing obligations.** A store-projected family with a gap gives pending_gap and rebuilds from committed collection state. A computed family with a gap gives resync_scheduled and obtains a provider snapshot. The change remains admitted, and the plugin takes no recovery action. EN-9's shared family projection remains unchanged.

4. **Scope and traceability pass.** A6-2 names the A2-7 readings. Test IDs cover both projection kinds, existing gaps, newly caused gaps, and no gap. The test stimulus uses real Hub scheduling and bounded queues. The candidate adds no numeric bound and changes no frozen file.

The candidate 1 finding is resolved. This ACCEPT applies to the matched contract amendment. It does not accept feature code or prove that tests pass. The steward owns the manifest update.

## Lua binding r9 Amendment 2, candidate 1 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-02.

Candidate: `amendments/hub-plugin-lua-binding-v1.1-r9-amendment-2-candidate1.md`, commit `fd0423e`.
SHA-256: `e56dd05df277df9c0a2baef8f41badb502d08a1bfc3b681d170aa3096625c165`.

Review authority: `docs/BUILD.md`, manifest final27 in `frozen/current`, Lua LB-10 and LBA1-3, and HP MT-1, A2-5, PC-1 and IV-8.

1. **The entry shapes close the Lua registration gap.** Tool, prompt, and resource entries use flat descriptor fields and a call function. The host retains the function and sends its handler id with the matching wire handler and descriptor. The stated fields match MT-1 and A2-5 without changing the wire shape.

2. **Identity and plugin-call behavior remain intact.** An explicit id names the handler; the host supplies one when omitted. IV-8 still governs stable strings, duplicate rejection, and dispatch identity. callable_by remains an optional tool declaration and preserves PC-1's explicit plugin-call opt-in.

3. **Scope and traceability pass.** LBA2-2 names the LB-10 entry reading. Unknown fields, an explicit Lua handler field, and missing required fields fail the load. Named IDs cover tool entries, handler-field rejection, prompts, and resources. The candidate adds no numeric bound and changes no frozen file.

This ACCEPT applies to the matched contract amendment. It does not accept feature code or prove that tests pass. The steward owns the manifest update.

## Hub→plugin Amendment 7, candidate 1 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-02.

Candidate: `amendments/hub-plugin-contract-v1.1.amendment-7-candidate1.md`, commit `b241c39`.
SHA-256: `681e30f3e27380b11bf8101dad34c14f9240290bfb7ac0c84509de9995819ae1`.

Review authority: `docs/BUILD.md`, manifest final28 in `frozen/current`, HP PM-3, PM-9, A2-29, A2-52 and ST-9.

1. **The trimming rule conflicts with protected existing history.** A7-1 requires at most lock_history_max entries and drops the oldest excess after an update. Its final sentence says a live change applies only to new admissions. Under A2-52, existing lock entries retain their admitted capacity. For example, history admitted under a larger maximum must not disappear merely because a later update uses a smaller maximum. Define how trimming treats protected existing entries after a live reduction. If the reduction cannot preserve their obligation, use A2-52's refusal at reload_config with the key named. Add a conformance case that reduces the limit with existing history, then updates and rolls back. It must prove either preserved prior capacity or refusal of the incompatible reload. Do not silently weaken A2-52.

2. **The fixed-limit rollback rule closes the stated gap.** Current and previous locks are distinct. Successful updates retain bounded previous versions, and rollback consumes the newest previous entry without adding the replaced lock. Empty history uses the existing no_history failure with effect none. The worked example and named IDs cover counting and repeated rollback without a new numeric bound. PM-3 and ST-9 still govern durable locks.

This REJECT concerns live-limit changes and existing lock obligations. It does not review feature code or prove that tests pass.

## Hub→plugin Amendment 7, candidate 2 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-02.

Candidate: `amendments/hub-plugin-contract-v1.1.amendment-7-candidate2.md`, commit `37b9759`.
SHA-256: `2c1b5144fed929fd07d36c0dc50bcfead108e7fa70eec4120320a0175a87682e`.

Review authority: the candidate 1 review authority.

1. **The global maximum still contradicts the reduction rule.** The history bullet still requires “at most lock_history_max entries.” The worked reduction legitimately retains three entries under a current limit of one. Qualify that bullet for entries protected by their admitted limits. State the effective rule rather than retaining an unconditional current-limit cap.

2. **The aging rule needs one observation list.** An entry's count of newer entries can change when another entry is dropped. In the v6 example, v2 has three newer entries before v4 is dropped, but only two afterward. The stated outcome drops v2, so the intended count uses the complete post-push list before any drops. State that explicitly: decide all eligibility from that list, then remove the entries that fail. This describes the resulting history without prescribing its implementation. Add that assertion to the reduction test so sequential recounting cannot produce a different retained history.

3. **The protection approach resolves the prior capacity concern.** Entries retain their admitted L and reload itself trims nothing. The candidate adds reduction and rollback test IDs. Fixed-limit counting, rollback consumption, and no_history remain consistent. No numeric bound is added.

This REJECT concerns the conflicting global cap and the aging observation point. It does not review feature code or prove that tests pass.

## Hub→plugin Amendment 7, candidate 3 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-02.

Candidate: `amendments/hub-plugin-contract-v1.1.amendment-7-candidate3.md`, commit `3fddf4a`.
SHA-256: `8f8b450752a1af666a1e0c712cc75c65cc82ba38f57c0898c770c567f1a4893a`.

Review authority: `docs/BUILD.md`, manifest final28 in `frozen/current`, HP PM-3, PM-9, A2-29, A2-52 and ST-9.

1. **The live-limit findings are resolved.** The history bound now permits entries protected by larger admitted limits. Reload trims nothing. Each entry ages under its own admitted limit. All newer-entry counts use the complete post-push list before drops. The reduction test asserts both retained histories, so sequential recounting cannot change the result.

2. **Rollback behavior is complete.** Current locks do not count as history. Rollback consumes the newest previous lock and drops the replaced lock without creating a roll-forward entry. Empty history gives the existing no_history failure with effect none. The fixed-limit example and named tests cover the full sequence.

3. **Scope and traceability pass.** A7-2 names the PM-3 reading. The candidate preserves A2-52's admitted capacity and PM-3's durability under ST-9. Each behavioral rule has conformance IDs. Example values add no numeric bound. The candidate states observable history behavior and changes no frozen file.

All remaining findings are resolved. This ACCEPT applies to the matched contract amendment. It does not accept feature code or prove that tests pass. The steward owns the manifest update.

## Core erratum 5, candidate 1 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-02.

Candidate: `amendments/core-contract-v1.17-erratum-5-candidate1.md`, commit `21f4461`.
SHA-256: `1f91606cc5de6ebc956045bfcb8c86bdd347535411e0d3dd107a5ba8235476e9`.

Review authority: `docs/BUILD.md`, manifest final29 in `frozen/current`, Core EV-3, EV-7, EV-8 and OU-12, and terminal oracle rule R-7. The model evidence below uses libghostty fork `85a8d8e`.

1. **The non-OSC-52 selection rule changes the requested destination.** E5-1 assigns `c` to every other sequence. The pinned model accepts OSC 5522 `loc=primary`. Its `Metadata.parse` selects `.primary`; `WriteState.init` retains that location; `kittyClipboardCommit` reports that location in the write callback. Thus these sequences do not all write the system clipboard. Define the observable selection mapping for each model destination. Add a conformance case for an OSC 5522 write to the primary selection. The current default-to-`c` test cannot prove correct destination handling.

2. **The bytes rule does not cover the new payload shapes.** The pinned callback carries `contents`, a list of MIME representations. OSC 5522 can commit more than one representation, and an empty list clears the destination. The existing `ClipboardWrite` has one `bytes` value and no MIME field. Saying that `bytes`, `total_bytes` and `TooLarge` are unchanged does not specify which representation that value carries. Define host-visible behavior for a non-text representation, multiple representations, and a clear. Define what `total_bytes` counts and how the existing bound applies. Add conformance IDs for these cases. Preserve EV-7 transparency without adding an unstated representation filter.

3. **The proposed extension has a valid purpose and limited scope.** E5-1 names its changes to EV-3 and section 6.2. It preserves raw Output and the one-actor rule. It adds no numeric bound. The named tests cover the basic extension, but they do not resolve findings 1 and 2.

This REJECT concerns destination and payload behavior. It does not review feature code or prove that tests pass. Model evidence: `src/terminal/kitty/clipboard_command.zig`, `src/terminal/kitty/clipboard_write.zig`, `src/terminal/stream_terminal.zig` and `src/terminal/clipboard.zig` at `85a8d8e`.

## Core Amendment 13, candidate 1 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-02.

Candidate: `amendments/core-contract-v1.17-amendment-13-candidate1.md`, commit `23d5cda`.
SHA-256: `ce5ac3f270d61d19aa5b86f006d5a3ab02d60ad5d8d20bcdaa810efaf321cbef`.

Review authority: `docs/BUILD.md`, manifest final29 in `frozen/current`, Core P7, EV-2, EV-3, EV-6, EV-7, EV-8, AM-2, OU-12 and section 6.2; HC P7 and its raw-output rule; terminal oracle rule R-7.

1. **The EV-3 replacement leaves two event shapes.** A13-2 changes EV-3's title and `bytes: none`, but EV-3 also spells out `ClipboardWrite{id, selection, bytes, total_bytes, reason?}`. That signature remains unchanged while section 6.2 and A13-1 use `contents`. Name the signature replacement in EV-3, or replace EV-3 in full. This is required to make the amended contract consistent.

2. **The new program reply has no complete ownership rule.** A13-1 requires Core to answer OSC 5522 with a delivery status. HC P7 assigns the raw sequence's side effect to the client, which can also produce its protocol status. EV-8 requires client-first arbitration for reply-needing queries and restricts automatic replies to `reply_terminal_query`; AM-2 governs every PTY input. The candidate does not state how an OSC 5522 acknowledgement relates to those rules. A synchronous model callback does not settle reply ownership. Define who supplies the one program-visible acknowledgement with an answering client, a refusing client, and no client. Define how any model-produced reply reaches the existing input admission path. Name any changed Core or HC clauses. Add conformance cases that prove one reply, the intended status, and contiguous admission under input pressure. The unchanged one-actor statement covers clipboard actions but does not resolve acknowledgement ownership.

3. **“Delivery to the host” lacks an observation point.** ClipboardWrite remains class D under EV-2 and can be dropped before the host polls it. Workers also continue while the host is absent. A13-1 does not state whether SUCCESS means callback completion, worker retention, host queue admission, or host observation. These outcomes differ, and Core cannot promise actual host delivery for a droppable event. State the exact condition that produces SUCCESS and preserve EV-2/EV-6 loss behavior. Add cases with no connected host and with a full event queue. Do not imply that SUCCESS proves a platform write or guarantees later event delivery.

4. **The earlier destination and payload findings are resolved.** The location mapping preserves primary selection. The event carries all MIME representations in order, distinguishes clear from an empty value, and counts all representation bytes against the existing bound. These clauses have conformance IDs and add no numeric bound. Raw Output remains available. The remaining findings concern the new reply rule and the incomplete replacement text.

This REJECT applies to the matched amendment. It does not review feature code or prove that tests pass.

## Core Amendment 13, candidate 2 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-02.

Candidate: `amendments/core-contract-v1.17-amendment-13-candidate2.md`, commit `5a20a10`.
SHA-256: `6df9e73bc0a620ab16e281cbbb00a8f13e3fa022e336577d64ccce9a1d27a31a`.

Review authority: the candidate 1 review authority, plus HC TS-QR and the pinned libghostty clipboard-read callback documentation at `85a8d8e`.

1. **The final sentence narrows clipboard reads without authority.** It says a clipboard read “is OSC 52 only.” EV-3 routes clipboard reads to EV-8 without that restriction. EV-8 covers every reply-needing query and permits `Encoded` for untyped forms. Only the typed `ClipboardRead{selection}` table row is specific to OSC 52. The pinned header explicitly reports OSC 5522 reads through the clipboard-read callback, and the pinned handler marks them as `.kitty_clipboard_read` queries. Remove the “OSC 52 only” claim. State that clipboard reads remain governed by EV-8; if needed, distinguish the OSC 52 typed row from other untyped reads. This amendment need not add a read feature or read tests, because it claims no change to reads. It must not exclude existing query coverage.

2. **All three candidate 1 findings are resolved.** A13-2 replaces EV-3 in full. A13-1b assigns the acknowledgement to the worker and makes clients suppress their own acknowledgement under TS-QR. It specifies contiguous admission through AM-2 without an input revision change. The status observation precedes host delivery and explicitly preserves class D loss. Named tests cover answering, refusing and absent clients, input pressure, host absence, event loss and the oversize status.

3. **The write rules otherwise pass.** Selection preserves the model destination and the non-empty OSC 52 string. Contents preserve all MIME representations and distinguish clear from an empty value. The existing byte bound applies to the sum. The candidate adds no numeric bound. Raw Output and the one-actor rule remain intact. The synchronous callback evidence explains the acknowledgement decision; the clauses define the observable status and input behavior.

This REJECT concerns only the unsupported restriction on reads. It does not review feature code or prove that tests pass.

## Core Amendment 13, candidate 3 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-02.

Candidate: `amendments/core-contract-v1.17-amendment-13-candidate3.md`, commit `26d69f7`.
SHA-256: `2701bef104598c74db4ff913891fd1ba350f2fd6f0ada2ff37fef8feba951d65`.

Review authority: the candidate 2 review authority, especially EV-8's shadow-answer invariants, and steward ruling R-17.

1. **The corrected read note adds an incompatible fallback.** The note now says the client answers an untyped clipboard read or declines, “and the shadow answers otherwise.” EV-8 explicitly says that a clipboard read is never answered by the shadow. A query with no shadow answer, left unanswered by the client, is answered by nobody. Remove the unconditional shadow-answer statement. State that EV-8 governs the read, including its prohibition on shadow clipboard answers. This correction preserves existing behavior and requires no new read clause or test.

2. **The candidate 2 finding is resolved.** The note no longer excludes OSC 5522 reads. It distinguishes typed OSC 52 reads from untyped model-recognized reads. R-17 governs the exact request bytes. The write clauses are unchanged and retain the passing assessment from candidate 2.

This REJECT concerns only the new fallback statement in the closing read note. It does not review feature code or prove that tests pass.

## Core Amendment 13, candidate 5 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-02.

Candidate: `amendments/core-contract-v1.17-amendment-13-candidate5.md`, commit `19aa88e`.
SHA-256: `ec5debacd1d0affd4de08eaef3f1747646b80da05504e304cf890ac2c35edb20`.

Review authority: `docs/BUILD.md`, manifest final29 in `frozen/current`, Core P7, EV-2, EV-3, EV-6, EV-7, EV-8, AM-2, IN-10, OU-12 and section 6.2; HC P7 and TS-QR; steward rulings R-7 and R-17.

1. **All prior findings are resolved.** The read note preserves EV-8 coverage for typed and untyped clipboard reads. It explicitly prohibits shadow clipboard answers and retains the no-answer outcome when no client answers. Candidate 4 was withdrawn without review. Candidate 5 records the correct candidate 3 verdict pointer.

2. **Destination and payload behavior are complete.** Selection preserves the non-empty OSC 52 string or maps the model destination. Contents carry all MIME representations in order. A consumer commits the representations atomically. Clear differs from an empty value. The existing byte bound applies to their sum, and an oversize event reports TooLarge. EV-3 and section 6.2 have one event shape.

3. **Acknowledgement behavior is consistent.** The worker alone supplies the OSC 5522 acknowledgement through AM-2. Clients suppress their autonomous acknowledgement under TS-QR. The status reports acceptance at the stated observation point, without promising host delivery or a platform write. Input pressure does not drop or split the acknowledgement. The transaction does not advance input_rev. Class D loss remains governed by EV-2 and EV-6.

4. **Scope and traceability pass.** A13-2 names the replacements. The write clauses retain their conformance IDs, including client ownership, input pressure, host absence, event loss and oversize status. The candidate adds no numeric bound. It preserves raw Output, the one-actor rule and the frozen query rules. The corrected read note changes no read behavior.

This ACCEPT applies to the matched amendment. It does not accept feature code or prove that tests pass. The steward owns the manifest update.

## Hub→plugin Amendment 8, candidate 1 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-02.

Candidate: `amendments/hub-plugin-contract-v1.1.amendment-8-candidate1.md`, commit `ddab9e4`.
SHA-256: `1e774a16aba69d4f6a69b3090f1952ef1474e467b5273da5ee06b42ef1015bf9`.

Review authority: `docs/BUILD.md`, manifest final30 in `frozen/current`, HP PD-3, PD-3b, PD-4, PD-5, PD-6, PD-9, PC-1, PC-4, PC-5 and PM-7; CLI-PL4 and its error table.

1. **The additive result check permits incompatible type changes.** A8-2 checks property presence, required sets and enums, but it does not preserve `type` or recursively preserve `items`. A MINOR document can change a result property from string to integer while retaining its name and required status. Both schemas have no enum, so every listed condition holds. An array can likewise change its item type. Define recursive result compatibility for the full PD-3b subset, including root types, nested properties and array items. Add conformance cases for incompatible result type and item changes. A8-2 must close the MINOR-additivity gap it claims to close.

2. **Nested unknown properties have conflicting outcomes.** Suppose the bound document has no `extra` property. The export adds optional `extra`, an object whose `required` includes `token`. The result contains `extra.token`. A8-1 says optional unknown `extra` is ignored and the call succeeds. It also makes unknown `extra.token` required at the same place and makes the call fail. Define whether ignoring an optional unknown property also ignores its entire value, or whether descendants are checked independently. State the traversal and the resulting outcome as behavior. Add a conformance case for an unknown optional object with a required child, including the corresponding case through array items.

3. **Runtime failure is consistent with the frozen admission rule.** PD-3b(c) permits the export to add required result properties; PD-4 requires the consumer to fail closed on an unknown required field. A8-1's runtime failure preserves both rules for a direct unknown required property. Refusing such an export at admission would change PD-3b(c), which this candidate does not name as a replacement. A8-2 compares interface documents, so it does not forbid a provider export from adding required result properties to an unchanged document. This distinction must remain explicit. The runtime failure can preserve committed host calls and report effect unknown after dispatch.

4. **The preview and refusal additions fit the existing upgrade rules.** A8-3 names consumer/interface pairs and uses interface versions and declared ranges. It preserves accepted-breaking degradation and the refusal of calls to unmet requirements. The refusal occurs before an operation starts, with effect none. Its consumer list matches CLI-PL4. These additions have conformance IDs and add no numeric bound.

This REJECT concerns result additivity and nested unknown-field behavior. It does not review feature code or prove that tests pass.

## Hub→plugin Amendment 8, candidate 2 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-02.

Candidate: `amendments/hub-plugin-contract-v1.1.amendment-8-candidate2.md`, commit `2f58be9`.
SHA-256: `6c20861148d5a0c78047d063e332a8a601d94825e57755d0cc6049ec2e2ade01`.

Review authority: `docs/BUILD.md`, manifest final30 in `frozen/current`, HP PD-3, PD-3b, PD-4, PD-5, PD-6, PD-9, PC-1, PC-4, PC-5 and PM-7; CLI-PL4 and its error table.

1. **Both candidate 1 findings are resolved.** The result relation preserves types, enums, required sets, existing properties and array items recursively. Unknown properties are judged once. The Hub ignores an optional unknown property with its whole value and does not judge its descendants. Known array items remain subject to the check. The examples and added conformance IDs cover nested objects and array items. The reported path uses a defined order.

2. **Runtime and admission rules remain distinct.** A8-2 compares interface documents. PD-3b(c) still permits additional required result properties in an export. A8-1 rejects a returned unknown required property against the bound document and preserves committed host calls with effect unknown after dispatch. The candidate does not silently change export admission to avoid this runtime failure.

3. **Preview behavior is consistent.** A8-3 preserves interface-version range checks, consumer/interface acceptance pairs, and accepted-breaking degradation. Unaccepted pairs produce the named refusal before any operation starts, with effect none. The fields and consumer list match the CLI rules. This clause is unchanged from the passing candidate 1 assessment.

4. **Scope and traceability pass.** A8-4 names the affected frozen clauses. Each new behavioral clause has conformance IDs. The candidate adds no numeric bound and defines observable validation, result and refusal behavior. It preserves the frozen interface schema subset and package resolution rules.

All remaining findings are resolved. This ACCEPT applies to the matched amendment. It does not accept feature code or prove that tests pass. The steward owns the manifest update.

## Hub→client Amendment 6, candidate 1 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-02.

Candidate: `amendments/hub-client-contract-v1-amendment-6-candidate1.md`, commit `0082564`.
SHA-256: `7deaee5d711bf446eac07bebdab3d1511a53da71c2743b00376c39c92a7fc5ce`.

Review authority: `docs/BUILD.md`, manifest final31 in `frozen/current`, HC section 6 and A.7b as amended by HC Amendment 2; HP CP-8, PD-1, PM-5 and PM-8; CLI-PL3, CLI-PL10 and CLI-WT1; steward ruling R-25.

1. **The install result closes the key gap.** The terminal result identifies the admitted package row and manifest version. It applies to successful installation and an operation end that leaves the package staged for approval. A failed end carries no result. The CLI can read the named package without reading files or comparing concurrent package lists. The candidate preserves the existing approval rule and operation result location.

2. **The worktree fields close the detached-head gap.** Branch presence identifies a branch checkout; branch absence identifies detached HEAD. The full hexadecimal head identifies the commit in either case. The candidate specifies absence rather than null and adds no fixed object-name length. The fields support the detached marker required by CLI-WT1 and R-25 without changing CLI layout.

3. **Scope and traceability pass.** A6-3 names both row replacements. Each new clause has conformance IDs for its normal and exceptional case. The candidate states observable results and row fields, adds no numeric bound, and changes no other frozen behavior.

This ACCEPT applies to the matched amendment. It does not accept feature code or prove that tests pass. The steward owns the manifest update.

## Pause state — 2026-10-02

The user paused Botster v1 through the orchestrator and the contract steward. No amendment review or unfinished verdict remains. The last verdict is HC Amendment 6, candidate 1 — ACCEPT, commit `c68a61a`, merged to main. The reviewer stops after recording and pushing this pause state.

## Core Amendment 14, candidate 1 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-04.

Candidate: `amendments/core-contract-v1.17-amendment-14-candidate1.md`, commit `0d2fa62`.
SHA-256: `50ef8dfa6eef53568d2d3375148738dc73a941e3532f4d7a4647c15174aa831b`.

Review authority: `docs/BUILD.md`, manifest final31 in `frozen/current`, Core A13-1, A13-1b, EV-3 and 9B; HC P7; steward rulings R-7 and R-32. Model evidence uses `src/terminal/kitty/clipboard_write.zig` and `clipboard_command.zig` at fork `3f8eb68`.

1. **The size definition does not cover aliases or metadata.** OSC 5522 aliases can expose the same decoded data under several MIME names. The final representation sum then exceeds the decoded payload size even without replaced chunks, contrary to A14-1's equality claim. The model also decodes MIME names and alias metadata, so “every byte the model decodes” does not identify payload bytes alone. Define which decoded bytes count and how aliases count. Add an alias conformance case. State whether an accepted write can have final `total_bytes` above clipboard_bytes. If aliases permit that outcome, explain how the event's retained contents still satisfy the claimed memory bound without prescribing storage mechanisms.

2. **The worked replacement example does not follow the model's chunk rule.** Consecutive chunks for the same MIME type continue one decoding stream. A replacement occurs when that MIME type returns after a different MIME region. The example sends text/plain and then text/plain again, so it does not establish the claimed replacement. Use a model-valid replacement sequence and make the replaced-chunk test exercise it. Keep the example's decoded count consistent with every region it includes.

3. **The optional decode-limit setting does not establish the stated memory guarantee.** A14-1 says Core “may” set option 39 and then the model never holds more than clipboard_bytes. The cited fork reserves decode capacity before checking the decoded length; the limit alone does not prove that allocation guarantee. Also, R-32 explicitly permits TooLarge when clipboard_bytes exceeds the model limit and the model cannot carry the write. A14-1 instead requires contents and SUCCESS for every decoded size within clipboard_bytes and says every TooLarge total is above that bound. Define the observable payload-retention guarantee and the model-limit relationship. Name any replacement of R-32's exception. Do not make a particular library option sufficient by assertion. Add conformance coverage for the memory guarantee and the relation between the configured and model limits.

4. **The proposed total_bytes change has no separate HC consumer.** HC P7 carries raw clipboard sequences and defines no clipboard event frame. A13's event shape remains unchanged. The A13 oversize and acknowledgement test IDs must follow the amended size rule; they do not require a new wire shape. Existing no-replacement, no-alias cases retain their result. A14 adds test IDs and no numeric bound, but findings 1 to 3 prevent acceptance of its sizing and memory claims.

This REJECT concerns the size definition, the replacement example and the memory guarantee. It does not review feature code or prove that tests pass.

## Core Amendment 14, candidate 2 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-04.

Candidate: `amendments/core-contract-v1.17-amendment-14-candidate2.md`, commit `a9efd6c`.
SHA-256: `f105f2f0e634ef7ae2c23fdeac8a6508eefa0b13fb0fcc4dc5202235a5b7e992`.

Review authority: the candidate 1 review authority and the same pinned model evidence.

1. **The over-limit report cannot establish the new maximum in every case.** A14-2 requires TooLarge total_bytes to equal the larger of decoded size and contents size. A14-3 says that after the model limit, the model counts only decoded size and reports no contents. Consider decoded payload of N + 1 bytes with one alias to that payload. The decoded size is N + 1, but the final contents size is 2 × (N + 1). A14-2 requires the latter; the stated report supplies only the former. Define the outcome when decoded payload was discarded and aliases make the final contents larger. Either define an explicit report rule for that case or require the model to report enough size evidence without retaining the discarded payload. Name any required change to R-32's report. Add a combined over-decoded-limit and alias test, including its total_bytes assertion. Separate replacement-only and alias-only tests do not cover this case.

2. **The peak statement adds an unsupported upper bound.** A14-3 says the momentary peak exceeds clipboard_bytes by at most one chunk's decoded size. The pinned model requests capacity with decoder.maxLen(payload), then uses ArrayList capacity growth before it knows the actual decoded length. Neither that source nor the cited clauses establish the claimed peak bound on allocated memory. Keep the retained-payload guarantee distinct from allocation capacity. Remove the unsupported quantitative peak claim, or supply existing authority and conformance coverage for it. The clause can state that temporary decode storage and allocator capacity are outside the retained-payload guarantee without inventing a new bound.

3. **The earlier definition and example findings are otherwise resolved.** Payload bytes exclude metadata. Aliases count in contents size rather than decoded size. The replacement example changes MIME regions before returning to text/plain. The configured model limit equals clipboard_bytes, and A14-4 names the R-32 exception that no longer applies. The A13 test reading and lack of a separate HC clipboard consumer are explicit. These corrections have conformance IDs and preserve the acknowledgement owner.

This REJECT concerns the combined alias/decoded-limit outcome and the unsupported allocation peak bound. It does not review feature code or prove that tests pass.

## Core Amendment 14, candidate 3 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-04.

Candidate: `amendments/core-contract-v1.17-amendment-14-candidate3.md`, commit `8bce2f8`.
SHA-256: `39c50621e569924ffabd2b66e8a5b94b7500c52a3c3372e215ce86eaa86ed16f`.

Review authority: `docs/BUILD.md`, manifest final31 in `frozen/current`, Core A13-1, A13-1b, EV-3 and 9B; HC P7; steward rulings R-7 and R-32.

1. **All prior findings are resolved.** A14-2 checks decoded payload size first. An over-limit report uses that size without requiring discarded contents. Otherwise, the contents check counts all alias entries. The combined example and test fix the outcome when aliases accompany discarded payload. The replacement example uses separate MIME regions. Payload excludes metadata.

2. **The retained-payload guarantee is precise.** The model limit equals clipboard_bytes. The named R-32 exception no longer applies, while its report remains unchanged. Temporary decode storage, allocator capacity and metadata are outside the guarantee. The candidate adds no unsupported allocation peak bound. Events carry at most clipboard_bytes of contents.

3. **The changed total_bytes meaning is explicit and scoped.** Accepted writes report final contents size. TooLarge reports the size from the deciding step, which is always above the bound. A14-4 names the A13 size and status replacements and the 9B reading. It preserves existing A13 cases without replacements or aliases. HC P7 has no separate clipboard event frame, so the change needs no HC wire replacement. A13's acknowledgement ownership, input admission and raw-output rules remain intact.

4. **Traceability and authority pass.** Each new behavioral clause has conformance IDs, including the combined case and the configured-limit boundary. The candidate reuses the existing clipboard_bytes parameter and adds no numeric bound. It states the model-limit relationship and the observable retention, event and acknowledgement outcomes.

This ACCEPT applies to the matched amendment. It does not accept feature code or prove that tests pass. The steward owns the manifest update.

## Core Amendment 15, candidate 1 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-04.

Candidate: `amendments/core-contract-v1.17-amendment-15-candidate1.md`, commit `c75cf39`.
SHA-256: `5ecf3e036a5de52e1a9a8ecb78ac57b0493f9c4e430c5196dff3f038581ecc6e`.

Review authority: `docs/BUILD.md`, manifest final33 in `frozen/current`, Core IN-5, IN-9, section 5.1A, section 9.3 and 9B; Core A7-1; the user's amendment-review rule against numeric bounds without existing authority; terminal oracle rule R-7.

1. **The new numeric values have no stated authority.** The candidate gives max_key_text_bytes a default of 256 and a range of 1 to 4,096. No cited clause establishes these byte values for key text. ST-7's 256-byte retention default governs retained input records, not valid key input. max_key_repeat's range governs an event count, not text bytes. max_query_text_bytes establishes a different parameter and range; its existence does not authorize these numbers. The synchronous-work concern can justify a separate parameter, but it does not justify its chosen default and range. Derive the values from existing authority or obtain a user ruling that authorizes them. Reusing an unrelated parameter solely because its numbers match would not resolve this finding.

2. **The refusal codes are explicit and consistent with the named changes.** A15-2 extends InvalidInput for an over-limit key text field, with no OpId. The route refusal uses the existing too_large code, preserves the route and has effect none. A15-1 gives the raw-text check precedence over the encoded-size check; IN-9 still governs encoded payload size afterward. Thus InvalidInput for this field and PayloadTooLarge for an excessive encoded write have distinct meanings. These choices do not require a new error code.

3. **The proposed behavior otherwise closes the stated input gap.** The parameter bounds UTF-8 bytes before worst-case encoding work. Text and Paste keep their existing meanings. Shifted and base-layout keys remain single scalars. A15-2 names the field, limit-table and error-meaning changes. Conformance IDs cover host and route refusals, the boundary and invalid configuration. R-7 remains the encoder authority. The amendment must not turn a particular exhaustive probing implementation into an additional contract requirement.

This REJECT concerns only the unsupported numeric default and range. It does not review feature code or prove that tests pass.

## Core Amendment 16, candidate 1 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-04.

Candidate: `amendments/core-contract-v1.17-amendment-16-candidate1.md`, commit `8d7a39e`.
SHA-256: `82b0ca587918ce9d804781bd583274d8fb70013fabcd9be2e3135e26b2c9aada`.

Review authority: `docs/BUILD.md`, manifest final33 in `frozen/current`, Core A2-1, A2-5, SV-5, SV-6, SV-8, SV-9, SV-10, ER-0 and EV-5.

1. **The Lost rule assumes that every loss means a dead guardian.** A2-5 defines service loss reasons as GuardianLost or EpochExhausted. SV-6 makes an exhausted epoch counter Lost(EpochExhausted); it does not require guardian death. A16-1 nevertheless says the guardian is gone for every Lost service and uses that premise to prohibit signals and state changes. Define the StopService outcome for EpochExhausted with a live guardian. If every terminal Lost service deliberately makes StopService a no-op, state that rule directly and name the SV-9 exception, rather than deriving it from guardian loss. If a live guardian can stop that service, define the resulting completion and state event. Add conformance coverage for EpochExhausted as well as GuardianLost.

2. **The result does not define the payload uncertainty field.** ServiceState's Lost value is `{reason, payload_may_remain}`. The proposed ServiceEnd is `Lost(reason)`, but the GuardianLost example says “with its payload_may_remain.” The candidate does not say whether ServiceEnd includes that field or the caller reads it from the existing state. Define one exact result shape. Preserve the recorded uncertainty without guessing an exit or payload death. Make the lost-result test assert that shape and the intended uncertainty behavior.

3. **The result union addresses the stated gap.** StopService is already admitted in Lost, so an Exited-only result cannot represent every admitted outcome. An explicit lost outcome is appropriate. The candidate preserves the admission states and adds no numeric bound or error code. It names the A2-1 result replacement and has conformance IDs for guardian loss and loss during a stop. Existing RegistryFailed errors and the ordinary completion event remain governed by the frozen operation rules.

This REJECT concerns the other service loss reason and the exact lost-result shape. It does not review feature code or prove that tests pass.

## Core Amendment 16, candidate 2 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-04.

Candidate: `amendments/core-contract-v1.17-amendment-16-candidate2.md`, commit `a06a3a4`.
SHA-256: `1e74e1d6d1906e9ae9f1c7e0d76f93c0b06e8bed89072c059943a20c262f8794`.

Review authority: the candidate 1 review authority, especially A2-1, ER-0 and EV-5.

1. **The GuardianLost case now prohibits its completion event.** Candidate 1 correctly said no ServiceState event is posted. Candidate 2 says “No event is posted.” An admitted StopService must still produce Completed with ServiceEnd; ER-0 and EV-5 preserve that operation path. Restore the precise statement that no ServiceState event is posted because the state does not change. The completion event remains required. Make the GuardianLost conformance case assert both facts.

2. **The in-flight loss rule conflicts with the recorded-exit rule.** “From any admitted state” includes Exited. A16-1 already requires StopService of Exited to return its recorded exit. Losing the guardian after Core recorded an exit does not erase that observation or establish payload uncertainty. Limit the in-flight GuardianLost outcome to a stop whose exit is not yet known. State that a recorded exit remains the Exited result. Add a case where the guardian ends after Core records the exit but before the stop completion is delivered.

3. **The candidate 1 findings are resolved.** The candidate gives EpochExhausted and GuardianLost separate stop behavior. It names the SV-9 exception and the EpochExhausted-to-Exited transition. ServiceEnd's Lost value explicitly includes reason and payload_may_remain. The tests cover both loss reasons and the exact uncertainty field. The candidate adds no numeric bound or error code.

This REJECT concerns event scope and preservation of an observed exit. It does not review feature code or prove that tests pass.

## Core Amendment 16, candidate 3 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-04.

Candidate: `amendments/core-contract-v1.17-amendment-16-candidate3.md`, commit `b7ee145`.
SHA-256: `3ad8c7a500e888fe6df8c08634c8bcb0718191e9f6d509d0331c525c0867c96f`.

Review authority: `docs/BUILD.md`, manifest final33 in `frozen/current`, Core A2-1, A2-5, SV-5, SV-6, SV-8, SV-9, SV-10, ER-0, AM-3 and EV-5.

1. **All prior findings are resolved.** GuardianLost produces no ServiceState change but still posts the operation's Completed. A recorded exit remains the Exited result despite later guardian loss. In-flight loss produces Lost only before Core records an exit. The added tests cover both event scope and exit preservation.

2. **Each service loss reason has a defined stop outcome.** GuardianLost preserves the state's reason and payload_may_remain without signals or a fabricated exit. EpochExhausted uses the verified guardian to run SV-9, posts the observed Exited state and completes with that exit. A16-2 explicitly names this transition and the GuardianLost exception to SV-9. The id remains governed by RemoveService.

3. **Scope and traceability pass.** ServiceEnd has an exact result shape. A16-2 names the A2-1 result and value-type changes. Admission states, existing async errors and ordinary completion rules remain unchanged. Each new behavior has a conformance ID. The candidate adds no numeric bound or error code and states observable outcomes rather than a new implementation procedure.

This ACCEPT applies to the matched amendment. It does not accept feature code or prove that tests pass. The steward owns the manifest update.

## Core Amendment 15, candidate 2 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol). Date: 2026-10-04.

Candidate: `amendments/core-contract-v1.17-amendment-15-candidate2.md`, commit `93fd8bb`.
SHA-256: `ef636555c12b610683d6afe6ab381833b362e2a395af5d430af3622227b80c10`.

Review authority: `docs/BUILD.md`, manifest final34 in `frozen/current`, Core IN-5, IN-9, section 5.1A, section 9.3 and 9B; Core A7-1; terminal oracle rule R-7; the user ruling recorded as R-34.

1. **The numeric-authority finding is resolved.** R-34 records the user's authorization of max_key_text_bytes with default 256 and configurable range 1 to 4,096. The candidate cites that ruling. The values no longer depend on an unrelated retention parameter or an analogy to query text.

2. **The passing behavior from candidate 1 is unchanged.** The UTF-8 byte check precedes the key's other size checks. A host refusal is synchronous InvalidInput with the text field named and no OpId. A route refusal is too_large with effect none and the route open. A value at the text bound passes that check; IN-9 still governs the worst-case encoded size. Text, Paste and single-scalar alternate keys retain their existing meanings.

3. **Scope and traceability pass.** A15-2 names the key field, the new CoreLimits row and the InvalidInput meaning. The existing conformance IDs cover host and route refusals, the boundary and invalid configuration. The only new numeric values now have explicit user authority. The amendment preserves R-7 as the encoder authority without requiring one probing implementation.

All remaining findings are resolved. This ACCEPT applies to the matched amendment. It does not accept feature code or prove that tests pass. The steward owns the manifest update.

## Core Amendment 18, candidate 1 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol), session `sess-1791583644-0173-0d5cd1af9ae86fa35dce9a5a444c9820`. Date: 2026-10-09.

Candidate: `amendments/core-contract-v1.17-amendment-18-candidate1.md`, commit `8c000c5ebd9d042974f52e6fc3e9d6da7a405c45`.
SHA-256: `287b645d9c8d62ad1745d19846fa1342e5ecd8c060f3e19c1efbb0db818b2fd8`, verified with `shasum -a 256` on the committed file.

Review authority: `docs/BUILD.md`; manifest final35 and `contracts.toml`; Core ST-4, AD-3, TM-1, TM-3, TM-4 and erratum 3 E3-1; the amendment-review brief.

1. **MEDIUM — A18-1 does not define the adopted silence event's `since`.** The candidate clears `last_output_at` and starts an adopted idle period without output. TM-4 still expresses its threshold using `last_output_at`, which A18-1 defines as optional unix time. The event still requires `Silent{id, since}`. Define the monotonic start separately from the unix stamp. Name the adoption point that supplies both values. Define `since` when adoption starts an idle period before this host observes output. Make the adopted-idle conformance case assert that value. Preserve E3-1's event-budget rule when the silence event becomes due.

2. **MEDIUM — A18-2 does not quote every sentence whose reading changes.** Item 1 names section 6.1 without quoting ST-4's sentence. Items 2 and 3 quote insertion fragments rather than the AD-3 and TM-4 sentences. Quote the affected base sentences exactly. State their amended readings, including the separate monotonic silence calculation from finding 1. This satisfies the brief's exact-quotation rule and makes the changed guarantee reviewable.

3. **PASS — The host-observation distinction addresses the adoption comparison.** A18-1 preserves worker state and excludes the earlier host's output stamp from AD-3's equality guarantee. The candidate names conformance IDs for the reset and later output. It adds no numeric bound or terminal oracle. The existing `conf::ad_3_adopted_equals_started` comparison must exclude only `last_output_at`.

This REJECT concerns the adopted silence event and the replacement text. It does not review feature code or prove that tests pass.


## HC Amendment 7, candidate 2 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol), session `sess-1791583644-0173-0d5cd1af9ae86fa35dce9a5a444c9820`. Date: 2026-10-09.

Candidate: `amendments/hub-client-contract-v1-amendment-7-candidate2.md`, commit `9d21e2419b866213f1232df34a7e8e7dfc16fa54`.
SHA-256: `7ca773704d8659a10522aec04caa436a98dedf1a7154d531d6318a6aef7d1fb7`, verified with `shasum -a 256` on the committed file.

Review authority: `docs/BUILD.md`; manifest final35 and `contracts.toml`; HC section 6, A.9 and Amendment 2; HP PM-7, WP-5, Amendment 8 A8-3 and Amendment 2 A2-41/A2-42; the amendment-review brief.

1. **PASS — A7-1 carries the existing HP preview shape.** Both upgrade previews carry the required breaking list. Each entry includes the consumer, interface, declared range and new interface version. The bulk result carries the list for the whole set and retains each package's list. The acceptance pairs and existing refusals retain their meanings. No fix is required.

2. **PASS — A7-2 exposes the origin that HP already requires.** The optional string carries the recorded origin. Operator visibility and absence for other callers match the stated access rule. HP A2-42 still governs persistence, a taken port and an operator change. The existing HP conformance IDs cover persistence and the taken-port outcome. No fix is required.

3. **PASS — The listener scope line needs no user decision within this amendment.** HP A2-41/A2-42 require the listener error but define no client field for that error. HC defines package state and install status, with no listener resource or listener-status field. A7-2 preserves that scope. It does not waive the listener error or permit a silent port change. Adding a client-visible listener field would require a separate amendment; this candidate does not select that addition. No fix is required.

4. **PASS — Authority and traceability are sufficient.** The replacement fragments match the cited base text. The candidate cites conformance IDs for both projections and adds no numeric bound. `git cat-file -t` confirms the candidate commit and superseded candidate commit `85c64b3`. No fix is required.

This ACCEPT applies to the matched amendment. It does not make the amendment FINAL. BUILD.md requires its crate change on the same final branch. The steward owns that check and the manifest update.


## Core Amendment 18, candidate 2 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol), session `sess-1791583644-0173-0d5cd1af9ae86fa35dce9a5a444c9820`. Date: 2026-10-09.

Candidate: `amendments/core-contract-v1.17-amendment-18-candidate2.md`, commit `7476abc`.
SHA-256: `6fcd07624da4dbc24d6c07aea29526443390d28219cce9bbe9b1c5e2f8edafeb`, verified with `shasum -a 256` on the committed file.

Review authority: `docs/BUILD.md`; manifest final35 and `contracts.toml`; Core ST-4, AD-3, TM-1, TM-3, TM-4 and erratum 3 E3-1; candidate 1 verdict `15a2a96`; the amendment-review brief.

1. **PASS — A18-2 resolves the silence finding.** The adoption state event defines the adoption point. The host supplies its monotonic start and unix stamp in that pump. The idle start supplies the deadline and `since` separately. Output replaces that idle start and sets `last_output_at`. E3-1 still carries a due silence step when the pump exhausts its event budget. The added conformance IDs cover `since`, later output and budget exhaustion. No fix is required.

2. **PASS — A18-3 resolves the replacement-text finding.** The ST-4, AD-3 and TM-4 quotations match the base exactly. Their amended readings state the host observation and monotonic silence rule. AD-3's comparison excludes `last_output_at` and nothing else. No fix is required.

3. **PASS — Scope and authority remain sufficient.** The candidate adds no numeric bound or terminal oracle. `git cat-file -t` confirms the candidate commit, candidate 1 commit `8c000c5` and verdict `15a2a96`. No fix is required.

This ACCEPT applies to the matched amendment. It does not review feature code or prove that tests pass. BUILD.md requires any crate change on the same final branch before the steward publishes FINAL.


## UI 1.4 Erratum 2, candidate 2 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol), session `sess-1791583644-0173-0d5cd1af9ae86fa35dce9a5a444c9820`. Date: 2026-10-09.

Candidate: `amendments/ui-contract-v1.4-erratum-2-candidate2.md`, commit `e4724676a546757293499f1f45068d14b0ab8829`.
SHA-256: `84e68d877a94aab893f1012ad637af1958fb6270872841fd65cc699a7d41520c`, verified with `shasum -a 256` on the committed file.

Review authority: `docs/BUILD.md`; manifest final35 and `contracts.toml`; UI section 3, section 12, UI-RZ-2/UI-RZ-2a/UI-RZ-2b, UI-CF-3 and UI-NV-6; the adopted UiNode vocabulary; the amendment-review brief.

1. **MEDIUM — UI-NV-9 restricts the children of the dialog's `body` slot.** The clause says a `Text` there is a button label or a text node's authored string. UI-NV-6 also permits `Text` on other node kinds. An `empty_state` child can carry a copy key in its `title`. The current realization uses that shape for `form_unavailable` when UI-CF-3 refuses a form (`realize.rs`, the form-overlay branch). Form descendants also retain their own permitted props. Define `body` as a node slot whose children retain each kind's `Text` rules. Remove the two-kind restriction and the claim that no copy key is ever realized into a dialog's body. Distinguish a scalar assigned to `body` from `Text` in a child's prop. Make the dialog conformance case include a child whose permitted prop holds a copy key.

2. **PASS — The direct prop correction matches the vocabulary.** `empty_state` has `title` and `description`; its authored part uses `body`. `dialog` has `title`, and `body` is a slot. The UI-NV-6 replacement fragment matches the base. The candidate preserves `ui_contract_view` and `ui_1_4`, adds no numeric bound and names conformance IDs. No fix is required for those corrections.

3. **PASS — The cited commits exist.** `git cat-file -t` confirms the candidate commit and superseded candidate commit `21db794`. This check does not establish runtime conformance.

This REJECT concerns the added restriction on dialog children. It does not review feature code or prove that tests pass.


## Core Amendment 17, candidate 1 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol), session `sess-1791583644-0173-0d5cd1af9ae86fa35dce9a5a444c9820`. Date: 2026-10-09.

Candidate: `amendments/core-contract-v1.17-amendment-17-candidate1.md`, commit `79d2bf87ed12e441f6e2d27c9c56a02c373b73a9`.
SHA-256: `6f0f46fc3ead26a489e04147ce7f52bf3faec1c99a6fe247b28286d604bc3813`, verified with `shasum -a 256` on the committed file.

Review authority: `docs/BUILD.md`; manifest final35 and `contracts.toml`; Core OU-1/OU-2b/OU-3, DP-1/DP-2/DP-3/DP-10/DP-11, 9B, section 13, Amendment 2 and Amendment 7; HC TS-1, TB-0 and PR-8; R-9.2 and R-7; the user decision and delegated D1/D2 decisions in the brief; the WebRTC inventory.

1. **MEDIUM — A17-4 uses abbreviated quotations for the removed text.** Item 1 quotes `begin(AttachWebRtc{…})`, but the base names `client, session, offer, expected_fingerprint, options`. The no-trickle and channel-authentication sentences also contain substituted ellipses. Item 2 abbreviates the deadline and authentication text. Items 4 and 5 abbreviate the queue and framing sentences. Item 9 identifies a sentence range without quoting the sentences. Quote each affected sentence exactly from the base. Quote the affected Amendment 2 and Amendment 7 text where their readings change. State the new reading beside each quotation. The brief requires exact quotations; an inventory reference does not replace them.

2. **PASS — Removing Core's `Unsupported{binding_auth_unavailable}` follows the stated boundary.** Core receives an already-connected Stream from its host. Core does not authenticate that stream's remote peer. HC TS-1 and TB-0 retain the Hub's binding-authentication refusal, and PR-8 retains mutual authentication before route acceptance. The withdrawn Core test uses `AttachWebRtc` with an empty expected fingerprint; that operation disappears. This removal does not permit the Hub to attach an unauthenticated caller. No fix is required for that removal.

3. **PASS — The transport and ownership changes follow the user decision.** The ordinary stream, its kernel progress point and descriptor ownership remain Core's. The Hub owns remote authentication, chunking and its bridge. A17 names the removed types, errors, limits, features and test cases. Service `SendError::NotConnected` has a separate meaning and remains unchanged. The candidate adds no numeric bound or terminal oracle. `git cat-file -t` confirms its commit. No fix is required for that scope.

This REJECT concerns the replacement quotations. It does not review feature code or prove that tests pass. The WebRTC set also requires an agreed codec revision and the crate changes before FINAL.


## HC Amendment 8, candidate 1 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol), session `sess-1791583644-0173-0d5cd1af9ae86fa35dce9a5a444c9820`. Date: 2026-10-09.

Candidate: `amendments/hub-client-contract-v1-amendment-8-candidate1.md`, commit `a748cc5e1b0d9a222248f7e7d6f633fd1de116a0`.
SHA-256: `84d577cb1f02b670c8eddfdad905b832f53b91652aed7a1529a370dded82d5b8`, verified with `shasum -a 256` on the committed file.

Review authority: `docs/BUILD.md`; manifest final35 and `contracts.toml`; HC HS-9, BP-1/BP-4/BP-6/BP-7, TS-1/TS-7/TS-8/TS-9, TR-2/TR-3/TB-W2/TB-W4, PR-8 and Amendment 2; codec revision 36; Core OU-3 and 9B; the user decision and delegated D1/D2 decisions in the brief; the WebRTC inventory.

1. **MEDIUM — A8-3's two-route test requires more than round-robin submission guarantees.** The rule explicitly permits payload that the association already holds. Route A can submit all its chunks while route B has none ready. B's input can then arrive after A starts, as the proposed test allows. Round-robin submission cannot put B ahead of A's final chunk already in the association. Define the readiness condition for the fairness guarantee. Give the test a controlled point where A still has unsubmitted chunks and B has a ready chunk. Assert the stated submission order with controlled transport delivery. Preserve the allowance for payload already submitted. Do not require an unconditional receipt order from only the stated input timing.

2. **MEDIUM — A8-3 changes the unit of `max_chunk_bytes` without changing TR-3.** TR-3 defines a message as `[u8 flags][chunk]` and bounds the chunk. A8-3 bounds every message by the same value. A chunk at the existing bound produces a message one flag byte larger. Define the bound consistently as chunk payload bytes, or name an explicit replacement of TR-3's sizing rule. Make the boundary conformance case count the flag byte separately. Reusing the existing payload bound requires no new numeric authority.

3. **MEDIUM — A8-4 requires draining even when Core closes a failed route.** Its final sentence requires the Hub to send every frame it read before closing the channel. Core can close a route with `StallTimeout` while the remote peer still takes no payload. The bridge can then remain unable to send those frames. TS-7 requires the route to end with `stalled`; TS-9 permits loss on a failed route. Limit mandatory draining to TS-8's healthy closes. State the failed-close outcome and its reason under TS-9. Add conformance coverage for a Core stall close with an undrainable bridge queue, as well as a healthy Core close that drains.

4. **MEDIUM — A8-7 omits the original text for several replacements.** TR-2, TB-W2, BP-4 and TB-W4 receive new text without exact quotations of their old sentences. BP-1, BP-7 and the removed action rows also lack the required old text. Quote each affected sentence or row exactly from the listed HC base and amendments. Identify any different codec wording separately. Apply the same exact-quotation rule to both artifacts.

5. **PASS — A8-4 has existing numeric authority for the moved queue parameter.** Core 9B defines `route_send_buffer_bytes` with default 1 MiB and range 4 KiB to 64 MiB. Core OU-3 defines its unacknowledged-payload purpose. A8-4 transfers that same parameter, default, range and purpose to the Hub. This transfer does not require a new user ruling for the numbers. The authenticated shared connection and route label also follow D1, D2 and PR-8. `git cat-file -t` confirms the candidate commit. No fix is required for that numeric authority.

This REJECT concerns fairness coverage, the chunk-size unit, failed-close draining and replacement quotations. It does not review feature code or prove that tests pass. FINAL also requires the matched codec revision and crate changes.


## HP Amendment 9, candidate 1 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol), session `sess-1791583644-0173-0d5cd1af9ae86fa35dce9a5a444c9820`. Date: 2026-10-09.

Candidate: `amendments/hub-plugin-contract-v1.1.amendment-9-candidate1.md`, commit `a748cc5e1b0d9a222248f7e7d6f633fd1de116a0`.
SHA-256: `0235a4c185eaa5beb03e61cb94c4f8d3b93d3fef863b27b26bdc5e40a69dccdd`, verified with `shasum -a 256` on the committed file.

Review authority: `docs/BUILD.md`; manifest final35 and `contracts.toml`; HP WP-5, WP-6 and ST-9; the user decision in the brief; the proposed Core A17 and HC A8 boundaries; the WebRTC inventory.

1. **PASS — A9-1 assigns the server configuration to the new endpoint.** The Hub uses the stored ICE configuration for its peer connections. Workers receive no ICE configuration and open no peer connection. This follows the user's decision to move WebRTC to the Hub. The replacement test must assert both the Hub's use and the absence of worker delivery. No fix is required.

2. **PASS — The replacement preserves WP-6's other rules.** The quoted fragment matches WP-6 exactly. The client still fetches the configuration from the web origin. Durability, URL validation, the existing bound, the trust rule and credential secrecy retain their meanings. The candidate adds no numeric bound or API shape. It replaces the endpoint test ID and preserves the other WP-6 IDs. No fix is required.

3. **PASS — The cited candidate commit exists.** `git cat-file -t` confirms `a748cc5e1b0d9a222248f7e7d6f633fd1de116a0`. This check does not establish runtime conformance.

This ACCEPT applies to the matched HP amendment. Core A17 and HC A8 remain required dependencies before the steward publishes the WebRTC set as FINAL. BUILD.md also requires the matched codec and crate changes on the same final branch.


## UI 1.4 Erratum 2, candidate 3 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol), session `sess-1791583644-0173-0d5cd1af9ae86fa35dce9a5a444c9820`. Date: 2026-10-09.

Candidate: `amendments/ui-contract-v1.4-erratum-2-candidate3.md`, commit `7b79bac3a2b37c395aa4e0a5f4ca83ba240ea4de`.
SHA-256: `624a0a5966ba92a3df4559e37167f8b42f155cfae25cc09e5b451cd384b7f04c`, verified with `shasum -a 256` on the committed file.

Review authority: `docs/BUILD.md`; manifest final35 and `contracts.toml`; UI section 3, section 12, UI-RZ-2/UI-RZ-2a/UI-RZ-2b, UI-CF-3 and UI-NV-6; the adopted UiNode vocabulary; candidate 2 verdict `9157984`; the amendment-review brief.

1. **PASS — UI-NV-9 resolves the restriction on dialog children.** A dialog's `body` remains a node slot. Each child retains its kind's permitted `Text` props. An `empty_state` child can carry a copy key in its `title`. The clause no longer limits slot children to buttons and text nodes. The dialog conformance case includes a child with a copy key and refuses a scalar `body`. No fix is required.

2. **PASS — The prop correction remains precise.** The part's `body` maps to the `empty_state` node's `description`. The `dialog` node permits `Text` in `title`. The replacement fragment matches UI-NV-6 exactly. The candidate preserves `ui_contract_view` and `ui_1_4`, adds no numeric bound and names conformance IDs. No fix is required.

3. **PASS — The cited commits exist.** `git cat-file -t` confirms the candidate commit, superseded commits `e472467` and `21db794`, and verdict `9157984`. This check does not establish runtime conformance.

This ACCEPT applies to the matched erratum. It does not review feature code or prove that tests pass. BUILD.md requires its crate change on the same final branch before the steward publishes FINAL.


## Core Amendment 17, candidate 2 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol), session `sess-1791583644-0173-0d5cd1af9ae86fa35dce9a5a444c9820`. Date: 2026-10-09.

Candidate: `amendments/core-contract-v1.17-amendment-17-candidate2.md`, commit `6bdec668e04403f3f12730b070def58ce24c9c9c`.
SHA-256: `41236a068cab6c6262f1e01bea9ec808efe1701707295bc9cde621b98f9f68e8`, verified with `shasum -a 256` on the committed file.

Review authority: `docs/BUILD.md`; manifest final35 and `contracts.toml`; Core OU-1/OU-2b/OU-3, DP-1/DP-2/DP-3/DP-10/DP-11, 9B, section 13, Amendment 2 and Amendment 7; HC TS-1, TB-0 and PR-8; R-9.2 and R-7; the user decision and delegated D1/D2 decisions in the brief; the WebRTC inventory; candidate 1 verdict `28b78cc`.

1. **PASS — A17-4 resolves the quotation finding.** The script confirms all 45 quoted base passages match the listed Core base files. The removed table rows and old feature-row fragment also match. Each changed passage has its new reading or removal instruction. No fix is required.

2. **PASS — DP-3 retains the stream limits.** TR-3 leaves Core's required codec clauses because Core no longer performs data-channel chunking. The replacement removes the rule about buffering chunks. The per-type frame limits and the check before allocation remain. Removing `max_chunk_bytes` from `attached.limits` still requires the agreed codec revision. No fix is required.

3. **PASS — The accepted scope from candidate 1 remains.** Core receives an ordinary connected Stream and keeps its descriptor ownership and kernel progress rule. The host owns the remote peer's authentication and bridge. Removing Core's binding-authentication refusal preserves HC TS-1, TB-0 and PR-8. The candidate lists the removed operations, values, errors, limits, features and test cases. Service `SendError::NotConnected` retains its separate meaning. No fix is required.

4. **PASS — Authority and traceability remain sufficient.** The candidate adds no numeric bound or terminal oracle. Its clauses retain their conformance IDs. `git cat-file -t` confirms the candidate commit, candidate 1 commit `79d2bf8` and verdict `28b78cc`. No fix is required.

This ACCEPT applies to the matched Core amendment. It does not review feature code or prove that tests pass. FINAL requires the agreed codec, the matched HC amendment and the crate changes on the same final branch.


## HC Amendment 8, candidate 2 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol), session `sess-1791583644-0173-0d5cd1af9ae86fa35dce9a5a444c9820`. Date: 2026-10-09.

Candidate: `amendments/hub-client-contract-v1-amendment-8-candidate2.md`, commit `066005bd11fd71629a9a09620af3eee601d537d3`.
SHA-256: `1f37db34879c8f5ea1ad4ae4e30d40db1980e501d5d91aacd67a70a980584e24`, verified with `shasum -a 256` on the committed file.

Review authority: `docs/BUILD.md`; manifest final35 and `contracts.toml`; accepted HC A7; HC BP-1/BP-4/BP-6/BP-7, TS-1/TS-7/TS-8/TS-9/TS-11, TR-2/TR-3/TB-W2/TB-W4, PR-8, A.3 and Amendment 2; codec revision 36; Core OU-3 and 9B; the user decision and delegated D1/D2 decisions in the brief; candidate 1 verdict `2b245fa`.

1. **MEDIUM — A8-3's boundary test adds an exact chunk partition that its rule does not require.** The clause and revised TR-3 bound each chunk payload by `max_chunk_bytes`. They do not require a sender to fill each non-final chunk. The test nevertheless requires a frame of `2 × max_chunk_bytes + 1` bytes to produce exactly three chunks sized `max_chunk_bytes`, `max_chunk_bytes` and 1. For `max_chunk_bytes` greater than 1, four chunks sized `max_chunk_bytes`, `max_chunk_bytes - 1`, 1 and 1 also satisfy the stated bound and preserve the frame. Remove the exact count and partition from the outbound assertion. Assert each chunk's payload bound, its separate flag byte and exact frame reconstruction. Keep the explicit inbound case with a chunk payload exactly at the bound. If exact outbound packing is intended as an additional contract rule, state that rule explicitly instead of adding it only through the test description.

2. **PASS — The four candidate 1 findings are resolved in the stated behavior.** The fairness rule applies when both route chunks are ready and identifies the last-served channel. It governs submission order and preserves payload already in the association. The test uses a controlled transport point and asserts submission order. The chunk unit now excludes the flag byte. Healthy closes drain; failed closes discard the bridge queue and close the channel. The script confirms all 21 old quotations and the claimed codec copies at their cited lines. No further fix is required for those findings.

3. **PASS — The added healthy-drain failure rule has existing bound authority.** A8-4 uses `route_limits.stall_close_after_ms`, an existing duration for a route that makes no progress. It defines the Hub's failed-close outcome when a healthy Core close leaves an undrainable bridge queue. The outcome is `stalled`, under TS-9, and it has its own conformance ID. This addition supplies a finite failure outcome without selecting a new numeric value. No user ruling is required for the number. No fix is required for that addition.

4. **PASS — The added survival replacements follow the user decision.** The TS-8, TS-11 and A.3 replacements scope the surviving byte path to `local-stream` and state that `webrtc` routes end at Hub restart. Their old quotations match the base. These replacements align those sentences with A8-5. They need no separate user decision. No fix is required for those additions.

5. **PASS — The queue parameter and publication conditions remain explicit.** `route_send_buffer_bytes` retains Core 9B's default 1 MiB and range 4 KiB to 64 MiB. Its Hub configuration key uses the existing restart rule. The candidate supplies the codec revision 37 replacement map. `git cat-file -t` confirms the candidate commit, candidate 1 commit `a748cc5` and verdict `2b245fa`. FINAL still requires the matched codec and crate changes on the same final branch.

This REJECT concerns only the exact outbound partition in the boundary test. It does not review feature code or prove that tests pass.


## HC Amendment 8, candidate 3 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol), session `sess-1791583644-0173-0d5cd1af9ae86fa35dce9a5a444c9820`. Date: 2026-10-09.

Candidate: `amendments/hub-client-contract-v1-amendment-8-candidate3.md`, commit `d0630d5f231d49430f41ab10fe9af6da490f5b8b`.
SHA-256: `2f2728a285d215b094ac35ef58dcadd1d3104535aa325cdc215df83be085aa74`, verified with `shasum -a 256` on the committed file.

Review authority: `docs/BUILD.md`; manifest final35 and `contracts.toml`; accepted HC A7; HC BP-1/BP-4/BP-6/BP-7, TS-1/TS-7/TS-8/TS-9/TS-11, TR-2/TR-3/TB-W2/TB-W4, PR-8, A.3 and Amendment 2; codec revision 36; Core OU-3 and 9B; the user decision and delegated D1/D2 decisions in the brief; candidate 2 verdict `14cde98`.

1. **PASS — A8-3 resolves the remaining boundary-test finding.** The outbound test checks the chunk payload bound, the separate flag byte and exact frame reconstruction. It requires no exact chunk count or partition. The inbound case retains a chunk payload exactly at the bound. The script confirms that only the title, supersession paragraph and this test description changed from candidate 2. A8-3's rule is unchanged. No fix is required.

2. **PASS — The previous passing findings remain valid.** Fairness applies to ready chunks and governs submission order. It does not reorder payload already submitted. Healthy Core closes drain, and failed Core closes discard the bridge queue. A healthy drain that makes no progress uses the existing `stall_close_after_ms` duration and ends with `stalled`. The TS-8, TS-11 and A.3 replacements scope restart survival to `local-stream`. These behaviors retain their conformance IDs. No fix is required.

3. **PASS — Numeric authority and replacement text remain sufficient.** The Hub queue parameter retains Core 9B's default 1 MiB and range 4 KiB to 64 MiB. No new numeric value requires a ruling. The 21 checked old quotations and the checked codec copies remain unchanged from candidate 2. The codec revision 37 replacement map remains explicit. No fix is required.

4. **PASS — The cited commits and verdicts exist.** `git cat-file -t` confirms the candidate commit, superseded commits `066005b` and `a748cc5`, and verdicts `14cde98` and `2b245fa`. This check does not establish runtime conformance.

This ACCEPT applies to the matched HC amendment. It does not review feature code or prove that tests pass. FINAL requires the agreed codec revision and the matched Core, HP and crate changes on the same final branch.


## Route codec revision 37, file check — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol), session `sess-1791583644-0173-0d5cd1af9ae86fa35dce9a5a444c9820`. Date: 2026-10-09.

Candidate: `frozen/current/route-codec-v1-extract-rev37.md`, staged commit `26ef7dc124f2c8f215f9d5e2eb27a96114af03ed` on `steward/final37`.
SHA-256: `30e254d64999d3bb8285ac683e0b8a6428d36f4cd577a1dd0b8bbedb25cb2e9a`, verified with `shasum -a 256` on the committed file.

Review authority: HC A8 candidate 3, its A8-7 and "Route codec revision 37" section, ACCEPT `9c6cc0b`; codec revision 36; the staged final37 manifest and `contracts.toml`; `docs/BUILD.md`; the steward's file-check request.

1. **MEDIUM — The final37 manifest asserts a crate change that this staged commit does not contain.** Its WebRTC section says, "Their crate change is on this final branch". At `26ef7dc`, Core's `op.rs` still defines `AttachWebRtc` and imports `Answer`. Core's `route.rs` still defines `WebRtc`, `Answer`, `connect_deadline` and `max_chunk_bytes`. Core's `limits.rs` still defines `max_connect_deadline`, `default_route_chunk_bytes` and `route_send_buffer_bytes`. These symbols contradict the listed Core A17 changes. Replace the assertion with the staging status until the crate change merges. Make the same correction in the final37 acceptance-record item. BUILD.md requires the actual crate change before FINAL. This finding does not require merging the staging branch now.

2. **PASS — The codec file exactly matches the accepted transformations.** The script reconstructs revision 37 from revision 36 with exactly 14 accepted old/new replacements, the title change and the inserted revision note. The reconstructed file equals the staged file byte for byte. The Hub's `binding_auth_unavailable` refusal and the terminal-query late-answer note remain unchanged. No codec text fix is required for the requested transformation check.

3. **PASS — The manifest identities and codec selection match.** All four hashes in the final37 WebRTC section match their staged files. Each cited verdict commit exists. `contracts.toml` selects revision 37 as the only active codec and moves revision 36 to `superseded`. Each selected contract file exists. The clause override names HC A8-3's boundary-test ID. No fix is required for those entries.

4. **PASS — The staging boundary remains intact.** The reviewer read `26ef7dc` by commit. The reviewer did not merge into `steward/final37` or merge that branch into main. `git cat-file -t` confirms the staged commit and ACCEPT `9c6cc0b`.

This REJECT concerns only the manifest's claim that the crate change is already present. The codec transformation check passes. This review does not accept crate code, prove that tests pass or authorize FINAL publication.


## Manifest final37, merge review — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol), session `sess-1791583644-0173-0d5cd1af9ae86fa35dce9a5a444c9820`. Date: 2026-10-09.

Candidate: `frozen/current/MANIFEST.md` and the final37 branch tree, commit `8fe53382edf628763f9149bf50e10103c8b3cb7f` on `steward/final37`, replacing the requested head `f64405ad1bcd57644059e0fa60e312cee300debe`.
SHA-256 of the manifest: `bcb121a6ccec54c557b75712a765124d220b7565c1cd741600c188b0289f58d2`, verified with `shasum -a 256` on the committed file.

Review authority: `docs/BUILD.md`; accepted Core A17 candidate 2 (`a33ef87`), HC A8 candidate 3 (`9c6cc0b`) and HP A9 candidate 1 (`9cd8dc9`); codec file-check verdict `e39155d`; `contracts.toml`; the final37 manifest; the steward's merge-review request and updated head.

1. **MEDIUM — The binding-authentication withdrawal cites the wrong clause.** `conformance/withdrawn.txt` records `conf::ou_1_unavailable_binding_auth_is_unsupported_and_never_attached_unauthenticated` as "withdrawn by Core A17-1". The accepted Core A17 Ids section explicitly gives this separate withdrawal authority as "A17-4 item 2"; A17-4 item 2 replaces OU-1's refusal. Change this ledger entry to "withdrawn by Core A17-4 item 2", retaining `replaced by none`. The manifest says that the withdrawal authorities are in this file, so that citation must match the accepted amendment. No contract change is needed.

2. **PASS — The earlier manifest finding is resolved.** The crate commit `1a389818a779d016edcc53052a289e01e76eacb5` is an ancestor of the reviewed head. The previously cited Core definitions of `AttachWebRtc`, `WebRtc`, `Answer`, `connect_deadline`, `max_chunk_bytes`, `max_connect_deadline`, `default_route_chunk_bytes`, `route_send_buffer_bytes` and the binding-authentication refusal are absent. Restoring the two manifest statements that the crate change is on the branch is supported by the tree. No further fix is required for finding 1 of `e39155d`.

3. **PASS — Identities, counts and immutable files match.** The ledger adds 23 withdrawals: 15 Core A17 ids, five HC A8 ids, two superseded A8 candidate 1 ids and HP A9's `wp_6` id. All 15 Core identities exactly match accepted A17. None of the 23 remains pending. The crate delta removes the corresponding 16 existing transcripts and adds 22 pending ids. All four hashes in the WebRTC manifest section match their committed files, and those files are byte-identical to the previously checked staged versions. Codec revision 37 retains SHA-256 `30e254d64999d3bb8285ac683e0b8a6428d36f4cd577a1dd0b8bbedb25cb2e9a`. No fix is required for these counts or identities.

4. **PASS — The later delta stays within the requested scope.** `1a38981..f64405a` contains the two manifest statements, the R-9.2 supersession note and the imported main documentation for R-42, R-43 and the handoff. `f64405a..8fe5338` contains only the imported `36536d3` changes to `botster-conformance/src/order.rs`, its `tests/run.rs` and Core conformance `tests/clock.rs`. These introduce the reviewed real-harness limits helper and its uses; no contract or manifest text changes in that delta. The R-9.2 note matches A17-4 item 15 and HC A8-3. No fix is required for that delta.

5. **PASS — Commit checks and the review boundary hold.** `git cat-file -t` confirms the reviewed heads, crate commit, imported main commit and cited verdicts. The reviewer read final37 by commit and did not merge that branch. This is a merge review of the manifest claims and listed delta, not a second crate-code review. The steward reports the crate review ACCEPT and its green gate at `1a38981`; publication still requires a green gate on the exact corrected final37 head.

This REJECT concerns only the withdrawal-authority citation in finding 1. The prior codec transformation check and amendment ACCEPTs remain valid.


## Manifest final37, merge review reconsidered — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol), session `sess-1791583644-0173-0d5cd1af9ae86fa35dce9a5a444c9820`. Date: 2026-10-09.

Candidate: `frozen/current/MANIFEST.md` and the unchanged final37 branch tree, commit `8fe53382edf628763f9149bf50e10103c8b3cb7f` on `steward/final37`.
SHA-256 of the manifest: `bcb121a6ccec54c557b75712a765124d220b7565c1cd741600c188b0289f58d2`, verified again with `shasum -a 256` on the committed file.

Review authority: `docs/BUILD.md`; accepted Core A17 candidate 2 (`a33ef87`), HC A8 candidate 3 (`9c6cc0b`) and HP A9 candidate 1 (`9cd8dc9`); codec file-check verdict `e39155d`; `contracts.toml`; the final37 manifest; `xtask/src/status.rs`; `conformance/replacement-map.json`; the steward's reconsideration request; prior merge-review verdict `fce5c31`.

1. **PASS — The withdrawal preserves its authority in the supported format.** This supersedes finding 1 and the REJECT in `fce5c31`. The reviewer missed the parser restriction and the exact citation already present in the replacement map. `parse_withdrawn` requires exactly two words for the authority; `authority_exists` requires a current clause row. `Core A17-4 item 2` cannot occupy that field, and A17-4 is a replacement section rather than a clause row. `Core A17-1` is the current clause that restricts Core to Stream. The accepted amendment expressly explains this withdrawal by the absence of Core-side peer authentication for Stream. The replacement-map entry preserves the precise edit authority: "Core A17-1 (the text change is A17-4 item 2): withdrawn; no replacement (Stream has no Core-side peer authentication; conformance/withdrawn.txt)". Read together, these records trace the withdrawal to the accepted amendment without changing its meaning. No ledger or guard change is required.

2. **PASS — The prior passing merge findings remain valid on the unchanged head.** The crate change `1a38981` is an ancestor and the previously cited removed Core symbols are absent. All 23 withdrawals are present, none remains pending, the 16 corresponding existing transcripts are removed and 22 new ids are pending. The four frozen WebRTC files and their manifest hashes remain unchanged; their hashes were checked again. The post-crate delta consists only of the listed manifest wording, R-9.2 note, main documentation imports and the later reviewed `36536d3` real-harness helper import. No further fix is required.

3. **PASS — Identity and publication boundary remain explicit.** The branch still resolves to the reviewed full head. `git cat-file -t` confirms that head, the crate commit and the cited verdicts. The reviewer read final37 by commit and did not merge or publish it. This ACCEPT reviews the manifest claims and listed delta; the crate review and runtime gate remain their own evidence. The steward must obtain a green gate on the exact final37 head before publication, as stated in the merge request.

This ACCEPT supersedes the final37 merge-review REJECT `fce5c31`. The accepted amendment set and passing codec transformation check remain valid. No change to final37 is required by this review.


## Core Amendment 19, candidate 1 — REJECT

Reviewer: Botster v1 Amendment Reviewer (Sol), session `sess-1791583644-0173-0d5cd1af9ae86fa35dce9a5a444c9820`. Date: 2026-10-09.

Candidate: `amendments/core-contract-v1.17-amendment-19-candidate1.md`, commit `a7156e8cfe77b36b24ef3ec234ae6eb52d837407`.
SHA-256: `733955c159481a242551ee01438c5d877be9c6d763bd269e21d37d080f1e411b`, verified with `shasum -a 256` on the committed file.

Review authority: `docs/BUILD.md`; manifest-final37 and `contracts.toml`; Core DP-3/DP-5b, OU-1, EV-8, AD-4, section 3, A2-1, A7-1, A9-1/A9-2 and E2-2; codec revision 37 A.1/A.4/A.6 and TS-5b; R-7; the matching codec types and generated schema; the steward's review request.

1. **MEDIUM — A19-1 does not give a finite widest value for `terminal_query.query_id`.** EV-8 says that the id is opaque and minted by the worker. Codec A.4 repeats that it is opaque; `TerminalQuery.query_id` is a `String`, and its generated schema has no length bound. A19-1 supplies bounds for `request_bytes_base64` and `answer_within_ms`, but none of its field rules bounds this string. The codec's rule for encoding a u64 identifier as a decimal string does not define this opaque query id as a u64. Thus the cited schema admits arbitrarily long ids, and cannot yield the finite worst-case query frame that this clause requires Core to compute at attach. Supply a finite sender-side bound or representation rule for the worker's minted id, known for the target worker at attach, with its authority. If an existing contract already supplies that bound, cite it explicitly. A new numeric bound requires its own authority or user ruling. Include the resulting largest encoded id in the worst-case frame test; do not silently substitute an implementation's current counter width for a contract rule.

2. **MEDIUM — The above-ceiling outcome has no stated conformance case.** A19-1 newly says that a configuration whose route floor exceeds `CoreLimits.max_route_frame_bytes` still passes `open`, while every attach with those options is refused synchronously. The described five new tests do not exercise this combination. The floor-minus-one test alone cannot detect an implementation that accepts a cap equal to the ceiling even though it is below the floor, or an implementation that rejects the configuration at `open`. Add or explicitly extend a named conformance case: use an otherwise valid configuration whose query-inclusive floor exceeds the ceiling; assert that `open` succeeds and that an attach at the ceiling (including the default cap) returns `InvalidInput{field: route_limits.max_frame_bytes}` without a route or event. Also establish that disabling query answers allows an attach when its remaining floor fits. Name the id in the clause and Ids section.

3. **MEDIUM — A19-2's test does not establish the check of the larger refusal frame.** Its only described case makes `input_done` with the path exceed `max_frame_bytes`. An implementation that checks only `input_done` passes that case while violating the new requirement to check every possible path-carrying frame. Add a case where the encoded path paste and `input_done` fit, but the largest permitted `input_refused` with the path exceeds the cap. It must produce `input_refused{too_large}` without `path`, create no file and paste nothing. The existing A19-2 id may cover both cases if its description says so. Include JSON escaping in the path-size coverage.

4. **PASS — The floor addresses the baseline defect without exempting frames from their bounds.** The candidate includes both `baseline_begin` and `modes`, computes mode widths for the target worker's supported protocol and uses the model's tracked modes under E2-2. It does not replace libghostty with a terminal oracle or impose a new ModeFlags size cap. Screen and history keep their separate bounds, and worker-chosen output chunks remain outside the floor. The existing `attached` frame remains a floor frame, so A9-2's minimum `output_dropped` argument remains valid. These parts need no fix.

5. **PASS — The replacement text and id migration match the base.** A script checks all ten quoted occurrences against their cited base files; each matches exactly. The replacement map updates A9-1 and its OU-1/A7-1 references and keeps DP-5b's existing pre-creation path-paste check. The obsolete exact-attached-frame acceptance id is expressly withdrawn and replaced; its below-attached-frame refusal id stays true. No quotation or withdrawal fix is required.

6. **PASS — Commit identity and publication requirements remain explicit.** `git cat-file -t` confirms the candidate commit, the manifest-final37 tag, the cited measurement tag `contracts-v0.1.21` and the existing A9 ACCEPT `fc32271`. This review checks the amendment's logic and traceability, not runtime conformance. FINAL requires the matched crate change on the same final branch under BUILD.md.

This REJECT concerns the missing query-id sizing rule and the two test-coverage findings. The measured frame sizes in the rationale are observations, not new normative bounds.


## Core Amendment 19, candidate 2 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol), session `sess-1791583644-0173-0d5cd1af9ae86fa35dce9a5a444c9820`. Date: 2026-10-09.

Candidate: `amendments/core-contract-v1.17-amendment-19-candidate2.md`, commit `d13e0ec7e5a2081fa1161ec40a4f0d89b10b79ca`.
SHA-256: `14e2bac0f54a523a5323e27f171bce64615b6102908307586c51b07a814c123d`, verified with `shasum -a 256` on the committed file.

Review authority: `docs/BUILD.md`; manifest-final37 and `contracts.toml`; Core DP-3/DP-5b, OU-1, EV-8, AD-4, A2-1, A7-1, A9-1/A9-2 and E2-2; codec revision 37 A.1/A.4/A.6 and TS-5b; R-7; candidate 1 verdict `ffd8e07`; the steward's review request.

1. **PASS — A19-1 resolves the query-id sizing finding.** The amendment now requires the worker to mint `query_id` as a u64 and send its decimal string under codec A.1's existing representation rule. The largest u64 is `18446744073709551615`, whose encoding has 20 digits. That width follows from the stated type rather than a separately selected size cap. Clients retain opaque-id handling. A19-3 item 6 supplies the exact EV-8 replacement, and a new conformance id checks the representation. The widest-frame test includes the largest encoded id. No fix is required.

2. **PASS — The above-ceiling outcome now has a conformance case.** The new named case requires an otherwise valid configuration to pass `open`, then checks both an explicit ceiling cap and the default cap against a query-inclusive floor above the ceiling. Both attaches must return the sync `InvalidInput` with no reservation and no event. Disabling query answers permits the attach when the remaining floor fits. This distinguishes the correct refusal at attach from a refusal at open or an invalid acceptance at the ceiling. No fix is required.

3. **PASS — The file-path test now checks the larger refusal frame.** Its second case keeps the path paste and `input_done` within the frame cap while the largest permitted `input_refused` with the path exceeds it. A checker of `input_done` alone fails this case. Both cases use a directory path with JSON-escaped characters and require `input_refused{too_large}` without a path, no created file and no PTY paste. No fix is required.

4. **PASS — The earlier passing findings remain valid.** The floor continues to include `baseline_begin` and `modes`, with tracked modes from the target worker's supported model, while screen and history retain their separate bounds. The existing path-paste check remains. The attached frame remains a floor frame, preserving A9-2's minimum `output_dropped` argument. The candidate introduces no terminal oracle or arbitrary ModeFlags cap. The diff from candidate 1 contains the stated fixes, their id lists, supersession text and corresponding replacement text. No further fix is required.

5. **PASS — Quotations, ids and commit identities match.** The script confirms all twelve quoted passages match the base text exactly. Each of the seven new ids appears in the clause table, the Ids section and the test descriptions. The obsolete exact-attached-frame acceptance id remains explicitly withdrawn and replaced; the below-attached-frame refusal id remains valid. `git cat-file -t` confirms the candidate commit, candidate 1 commit `a7156e8`, verdict `ffd8e07`, existing A9 ACCEPT `fc32271`, manifest-final37 and the measurement tag `contracts-v0.1.21`.

This ACCEPT applies to the amendment's logic and traceability. It does not review feature code or prove runtime conformance. FINAL requires the matched crate change on the same final branch under BUILD.md.


## Manifest final38, merge review — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol), session `sess-1791583644-0173-0d5cd1af9ae86fa35dce9a5a444c9820`. Date: 2026-10-09.

Candidate: `frozen/current/MANIFEST.md` and the final38 branch tree, commit `ea990ff683cae816e9ccd4f579d5d05717361de0` on `steward/final38`.
SHA-256 of the manifest: `db4189dec027919663d355d70b35794135247f882f96694914ae34292e3992d6`, verified with `shasum -a 256` on the committed file.

Review authority: `docs/BUILD.md`; manifest-final37; accepted Core A19 candidate 2 (`983e5a3`); `contracts.toml`; the final38 manifest; the accepted amendment's Ids section; the steward's merge-review request.

1. **PASS — The frozen file matches the accepted candidate.** `frozen/current/core-contract-v1.17-amendment-19-candidate2.md` is byte-identical to the candidate reviewed at `d13e0ec`. Its SHA-256 is `14e2bac0f54a523a5323e27f171bce64615b6102908307586c51b07a814c123d`, verified with `shasum -a 256`. The manifest entry has that hash and ACCEPT `983e5a3`. Every prior frozen contract file from manifest-final37 remains unchanged. No fix is required.

2. **PASS — The withdrawal and pending additions match A19.** The crate delta withdraws exactly `conf::a9_1_frame_cap_equal_to_the_attached_frame_attaches`, with authority `Core A19-1` and replacement `conf::a19_1_frame_cap_equal_to_the_floor_attaches_and_carries_its_baseline`. The withdrawn id leaves pending and has a withdrawn proof in the replacement map. All seven new A19 ids, and no other additions in this delta, enter pending. The three retained ids keep their prior states and proofs; the existing above-ceiling-cap transcript is unchanged, and the two retained A9 ids remain pending. No fix is required.

3. **PASS — The crate change is on the reviewed branch.** `cc335a85e872c1b36ab0cc00ec0ed98408636f26` is an ancestor of the candidate head. It adds the A19 crate-symbol table without a GAP reason and records the replaced A9 floor in the audit. `TerminalQuery.query_id` remains `String`; its generated description states the worker's u64 decimal-string rule while preserving the client's opaque handling. The schema and TypeScript carry the matching description. The manifest asserts the presence of the crate change, not a runtime floor implementation. The crate has no floor computation to replace, as the steward's request states. No fix is required for that manifest claim.

4. **PASS — The post-crate delta contains only the requested manifest statements.** `cc335a8..ea990ff` contains one commit and changes only `frozen/current/MANIFEST.md`: the A19 section and its acceptance-record item now state that the crate change is on the branch. Their commit reference and id counts match the reviewed tree. `contracts.toml` selects the accepted A19 file, and every selected contract exists. No fix is required.

5. **PASS — Commit identity and the publication boundary hold.** `git cat-file -t` confirms the reviewed head, crate commit and cited A19 verdicts. The reviewer read final38 by commit and did not merge that branch. The steward reports the crate review ACCEPT and green gate at `cc335a8`; this review does not repeat that crate review or establish runtime conformance. As the steward states, after merging the verdict-bearing main, the steward must gate the resulting exact final38 head before fast-forwarding main and tagging.

This ACCEPT applies to the final38 manifest claims and listed merge delta. No change is required by this review.


## Core Amendment 20, candidate 1 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol), session `sess-1791583644-0173-0d5cd1af9ae86fa35dce9a5a444c9820`. Date: 2026-10-09.

Candidate: `amendments/core-contract-v1.17-amendment-20-candidate1.md`, commit `bed2ac34d8d9d45d7ae7429cb810645a5f1fb2eb`.
SHA-256: `571acfe63c1de73e71e516aff554b894b1f4ef66d6ab5217d5c636cae16c53d0`, verified with `shasum -a 256` on the committed file.

Review authority: `docs/BUILD.md`, including Testing rule 8 and the amendment path; manifest-final38 and `contracts.toml`; Core DP-3, A5-1/A5-3/A5-4; R-37, R-47 and R-7; `docs/core-testkit-controls.md` allocation observer definitions; the steward's review request.

1. **PASS — The amendment supplies an explicit, narrow suite exception.** A5-4 currently requires both tiers to pass the same suite. R-37's observer proves allocations only in the process that holds the worker's route reader, so observing the host process on the real tier cannot establish this id. A20 changes A5-4 through the amendment path rather than granting a new exception by ruling. Its only named member is `conf::dp_3_frame_limit_checked_before_allocation`, and another member requires a Core amendment. The general description of allocator proofs does not authorize the runner to add unnamed members. No fix is required.

2. **PASS — The worker's obligation and its testkit proof remain.** DP-3's check before allocation is unchanged. The id must still pass on the testkit, which runs the same worker code in-process under A5-1. The observer retains R-37's process boundary and measurement conditions; it adds no production worker hook, alternate worker binary, wait or lock. The real tier records this id as testkit-proven with A20-1 as its reason and still runs every other DP-3 id, including an over-limit header refusal. This accepts a limit on direct real-process proof, not a waiver of the worker's allocation rule. No fix is required.

3. **PASS — The new behavior is traceable to a conformance id.** `conf::a20_1_testkit_proven_ids_are_exactly_the_named_list` checks the real runner's exception list against A20-1's exact list. A static list check needs no subject harness. The separate requirement that each listed id passes on the testkit remains a runtime suite requirement; the list check does not replace the allocation test itself. A5-3's real-only tests and A5-4's obligation for all other ids remain. No fix is required.

4. **PASS — The quoted replacement and bound authority are sufficient.** The script confirms all three quoted occurrences match A5's base text exactly. A20-2 gives the added exception sentence, and A20 leaves the existing id's testkit proof unchanged. It adds no numeric bound, terminal oracle, public Core API or worker protocol field. No user ruling is required for a number, and no fix is required.

5. **PASS — Candidate and base identities exist.** `git cat-file -t` confirms the candidate commit and cited base commit `9b1cf0f`. Both that base and manifest-final38 are ancestors of the candidate. This review checks amendment logic and traceability, not the runner implementation or runtime test results.

This ACCEPT applies to the named testing exception. FINAL requires its matched crate and suite-status changes on the same final branch under BUILD.md.


## Manifest final39, merge review — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol), session `sess-1791583644-0173-0d5cd1af9ae86fa35dce9a5a444c9820`. Date: 2026-10-09.

Candidate: `frozen/current/MANIFEST.md` and the final39 branch tree, commit `10b62f0dba79a70ebe9aa25e439d66352ee84566` on `steward/final39`.
SHA-256 of the manifest: `4f353d5b0b862c8e497aca46653602893a79e2df94eab04c7342b2754d7075c4`, verified with `shasum -a 256` on the committed file.

Review authority: `docs/BUILD.md`; manifest-final38; accepted Core A20 candidate 1 (`ad91867`), especially A20-1's member list and test; the final39 manifest and `contracts.toml`; the steward's final merge-review request.

1. **PASS — The frozen file and manifest identity match the accepted candidate.** `frozen/current/core-contract-v1.17-amendment-20-candidate1.md` is byte-identical to the candidate accepted at `bed2ac3`. Its SHA-256 remains `571acfe63c1de73e71e516aff554b894b1f4ef66d6ab5217d5c636cae16c53d0`, verified with `shasum -a 256`. The manifest records that hash and ACCEPT `ad91867`. Every previously frozen contract from manifest-final38 remains unchanged. `contracts.toml` selects A20, and every selected file exists. No fix is required.

2. **PASS — TESTKIT_PROVEN exactly equals the frozen named list.** The exported constant contains only `conf::dp_3_frame_limit_checked_before_allocation`. The static check reads `A20_TXT` through `include_str!` at the frozen A20 file, locates the A20-1 row and extracts ids only from its "The class has" item. It compares that list with the exported constant and rejects an empty, different or duplicate list. Thus other ids named elsewhere in the row are not granted the exception. No fix is required.

3. **PASS — The static transcript reaches the frozen-file check.** `conf::a20_1_testkit_proven_ids_are_exactly_the_named_list` invokes `type_check: a20_1_testkit_proven`. The driver dispatches this to the frozen-text comparison, then checks that the listed allocation id is not pending or withdrawn. The ledger has 2546 ids; its only added id in this delta is the new static check, whose replacement-map proof is `static`. The existing allocation transcript and both pending/withdrawn files remain unchanged from main `fe3eb95`. This preserves the testkit's runtime proof; the static check is not a replacement for that proof. No fix is required.

4. **PASS — The crate change is present and the later delta is limited.** Crate commit `3a4c16a60dc470afbcfb4e450babe2f23af5c6bd` and main `fe3eb95` are ancestors of the reviewed head. `3a4c16a..10b62f0` has one commit and changes only the two requested manifest statements: the A20 section and its acceptance-record item now record the crate change on the branch. The new static id, its frozen file, contract selection and driver support are in that tree. No fix is required for the manifest claims.

5. **PASS — Commit identity and the publication boundary hold.** `git cat-file -t` confirms the reviewed head, crate commit, main commit and A20 ACCEPT. The reviewer read final39 by commit and did not merge that branch. The steward reports crate-review ACCEPT and a green gate at `3a4c16a`; this review does not repeat the crate review or establish runtime conformance. After merging the verdict-bearing main, the steward must gate the exact resulting final39 head before fast-forwarding main and tagging, as stated in the request.

This ACCEPT applies to the final39 manifest claims and requested static-list checks. No change is required by this review.


## Core erratum 6, candidate 1 — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol), session `sess-1791583644-0173-0d5cd1af9ae86fa35dce9a5a444c9820`. Date: 2026-10-11.

Candidate: `amendments/core-contract-v1.17-erratum-6-candidate1.md`, commit `63128469ce1d57cbef00f2b6549f699a27d058fc`.
SHA-256: `508b2912c194004392344e44c83fccff774a6c8dfa6685bea100cd6835ccf33a`, verified with `shasum -a 256` on the committed file.

Review authority: `docs/BUILD.md`; manifest-final39 and `contracts.toml`; Core EV-8(c)/(d); codec revision 37 P9 binding, its revision-27 R-3 note, A.1 and A.5; steward ruling R-75 q3; the steward's review request. The codec's R-3 here is its completeness-sweep item, not steward ruling R-3.

1. **PASS — E6-1 resolves the existing wire-reason conflict.** Codec P9 binding expressly refuses a reply from another route with `input_refused{wrong_route}`. Its reply-refusal list includes `wrong_route`; its revision note expressly reserves `invalid_input` for the route-ending protocol error. EV-8(c)'s conflicting wrong-route refusal and list are the two passages this erratum corrects. The manifest records the codec as owned by HC and adopted verbatim by Core. R-75 q3 identifies this exact inconsistency. No fix is required.

2. **PASS — The replacement text matches the active Core base.** Both replaced passages match the text in `core-contract-v1.17.md` exactly. All three quoted codec passages match revision 37. The script checks every selected Core amendment and erratum and confirms that none restates these passages or contains `already_replied`. The EV-8 query-id representation added by A19 is a separate passage and remains unchanged. No fix is required.

3. **PASS — The conformance case checks the refusal and preserves the valid reply.** The new id sends a reply on r1 for a query bound to r2, requires `wrong_route` on r1 and no PTY write, and keeps r1 open. The following successful reply on r2 establishes that the wrong-route reply did not consume the query. The shadow's fallback and the exactly-once admission rule retain their existing EV-8 meanings. No fix is required.

4. **PASS — The erratum introduces no bound or wire vocabulary.** `wrong_route` is already the codec's `RefusalReason::WrongRoute`, and the codec text remains unchanged. E6-1 corrects the Core reading without adding a type, number, terminal oracle or general exception for malformed inputs. The manifest's A13 record confirms that Core erratum 5 was never published, supporting the candidate's use of number 6. No fix is required.

5. **PASS — Candidate and base identity are established.** `git cat-file -t` confirms the candidate commit and manifest-final39 tag. The selected manifest remains final39. This review checks erratum logic and traceability, not worker implementation or runtime conformance.

This ACCEPT applies to the matched Core correction. FINAL requires the corresponding crate and conformance changes on the same final branch under BUILD.md.


## Manifest final40, merge review — ACCEPT

Reviewer: Botster v1 Amendment Reviewer (Sol), session `sess-1791583644-0173-0d5cd1af9ae86fa35dce9a5a444c9820`. Date: 2026-10-11.

Candidate: `frozen/current/MANIFEST.md` and the final40 branch tree, commit `7871e4b9dbc264e2b1ed3bd211404462a347bc85` on `steward/final40`.
SHA-256 of the manifest: `164864a0078aefbec2fcdbbdf4a38e02b9adda674a3574cc0fab01a74c17b77c`, verified with `shasum -a 256` on the committed file.

Review authority: `docs/BUILD.md`; manifest-final39; accepted Core erratum 6 candidate 1 (`2a568fa`), especially E6-1 and its test description; `contracts.toml`; the final40 manifest; the steward's merge-review request and supplied crate gate log.

1. **PASS — The frozen erratum matches the accepted candidate.** `frozen/current/core-contract-v1.17-erratum-6-candidate1.md` is byte-identical to the candidate reviewed at `63128469`. Its SHA-256 is `508b2912c194004392344e44c83fccff774a6c8dfa6685bea100cd6835ccf33a`, verified with `shasum -a 256`. The manifest records that hash and ACCEPT `2a568fa`. All previously frozen contract files from manifest-final39 remain unchanged. The selected contract set adds E6, and every selected file exists. No fix is required.

2. **PASS — The manifest's crate and id claims match the tree.** Crate commit `f47ee2d75777d9e81a2c7fd022ee2e4157919ecd` and main `2a568fa` are ancestors of the reviewed head. The crate delta adds the E6 crate-symbol table without a GAP reason, the named transcript and its `core-testkit` replacement proof. The ledger adds exactly one id, `conf::e6_1_reply_from_another_route_is_wrong_route_and_the_query_stays_open`, for 2547 total. Pending and withdrawn remain unchanged. No fix is required.

3. **PASS — The transcript expresses E6-1's outcomes.** It obtains the query id from the route's actual `terminal_query`, sends a distinct encoded reply on r1, and requires `input_refused{wrong_route}`. A later refused `paste_commit` on r1 fences its continued operation and forbids both `terminal_query` and `route_closed` before that barrier. The reply on r2 uses different bytes; the transcript fences the absence of its refusal and checks that the PTY input is exactly r2's reply bytes. Thus the wrong-route reply has no PTY effect and does not consume the bound route's query. These are supplied client bytes, not a handwritten terminal-encoding expectation. The manifest accurately labels the transcript `structural` on the frozen FakeCore, preserving the separate testkit behavior proof. No fix is required for the requested transcript and manifest check.

4. **PASS — The final wording is the only post-crate delta.** `f47ee2d..7871e4b` contains one commit and changes only `frozen/current/MANIFEST.md`: its E6 section and acceptance-record item replace staging status with the present crate commit. No contract, table, transcript or code changes follow the reviewed crate head. No fix is required.

5. **PASS — Identity and crate gate evidence are established.** `git cat-file -t` confirms the reviewed head, staging commit `f1472e0`, crate commit and cited E6 ACCEPT. The supplied log `~/botster-sessions/gates/botster-contracts-stage0-final40-crate-f47ee2d7-pool-20261011-122420-83866.log` names the exact full crate commit and reports all ten jobs PASS and `botster-gate: exit 0`. The reviewer read final40 by commit and did not merge that branch. This does not establish a green gate for the final manifest head; publication requires a green gate on the exact head to be published, including any later merge of verdict-bearing main.

This ACCEPT applies to the final40 manifest claims and listed merge delta. It does not repeat the crate review or prove real-Core runtime conformance. No change is required by this review.
