# Contracts pin review

## PR #161 P5 Round 1 — 2026-10-09

- Exact head: `f81578858b137528aca82496904e89d16d6262ea`.
- PR branch: `stage1/contracts-v0.1.14`; P5 work branch: `stage1/p5-pin-v0.1.18`.
- Accepted v1 base: `c869dbeaf3a2922e4f4e7202f8e55490f7ce99fa`.
- Tree: `f5912d9af570edc253291949084457cb46587abd`.
- Previous reviewed head: `dea90ed4ee502432a70e19ec93b26c4a81558773`.
- Previous CLEAN verdict commit: `d1a3869e68f90c0ab69a951141304776e0818eb8` on stage1/review-p6.
- Contracts tag: `contracts-v0.1.18`, commit `ae4abc9bf71a2a80f3609d91a0c3c71c5b863efc`.

The prior review history remains in `d1a3869e:verdicts/p6-contracts-v0.1.14.md`.
The lead assigned #161 to P5 and retargeted the pin to v0.1.18 in the shared lead handoff.
The reviewer read the prior verdict, complete merge delta, pin delta, changed contract API, relevant amendments, current PR body, and supplied gate.
The reviewer changed no product code and ran no builds, tests, mutation jobs, or gates.

### Merge and pin

Merge `0fe5ba7ad6330bf3fa067fc23948e47c8e420fc1` has the previous reviewed head and accepted v1 head as its parents.
The reviewer compared each retained edit against its corresponding base.
All seven files retain the prior pin edits, including the lockfile sources, ledger, pending IDs, and three R-30 citations.
The merge adds no other change relative to accepted v1.

All six workspace contracts dependencies use v0.1.18.
All nine contracts lockfile entries use the exact tagged commit.
After normalization of those source entries, the lockfile is identical to the merged v0.1.17 lockfile.
No package version, checksum, or dependency list changes.
The new tag contains R-35 and R-36 and the Core A13, A15, and A16 crate changes.

### Contract API changes

Observation::ClipboardWrite now carries `contents: Option<Vec<ClipboardContent>>` with the contract's own type.
The serde field options match the contract event's options. The link adds no second type or byte encoding.
The host passes contents directly into Event::ClipboardWrite and preserves selection, total_bytes, and reason.
This path preserves a missing value, an empty list, and the order and bytes of each MIME representation.
The adapted host proof checks the MIME name and bytes after the observation travels through the link.
The queue proof retains ClipboardWrite as a droppable event.

No worker source emits this observation yet. The PR body assigns its later binding-to-wire mapping to P3's M2b work.
This pin review does not prove the complete A13 or A14 clipboard behavior.

The new CoreLimits.max_key_text_bytes defaults to 256 and validates the range 1 to 4,096 in the contract crate.
Core's open check calls that validator. The only CoreLimits struct literal in Core's source uses the default for remaining fields.
The A15 host admission check remains separate work, as the PR body and pending list state.

OpOutput::ServiceEnd replaces OpOutput::ServiceExit in the contract crate.
Core has no StopService completion that needs this adaptation yet. A16 remains pending under P7.
The route codec, probe script, and test-support source remain unchanged between the two contracts tags.
The pin delta adds no dependency, mutation exclusion, platform-specific code, or native binding change.

### Ledger and review scope

The checked-in Core ledger equals the new tag's sorted Core IDs, byte for byte: 675 IDs.
The new tag adds no Core ID relative to v0.1.17.
The pending IDs remain identical to the previous reviewed head.
Only the two A15 owner comments change in the new pin commit.
They now name P1 lifecycle, worked by P5, as stage1/plan owners.py requires.
The deferred and withdrawn copies match the new tag, byte for byte. The Core deferred table remains unchanged.
The PR body has the required Prior art note and states the previous review and new API scope.

### Supplied exact-head gate

The full Linux gate is `~/botster-sessions/shared/core-stage1/gate-logs/pin-v0.1.18-f8157885.log`.
It names the exact reviewed head and accepted v1 base.
The default tier reports 920 tests passed and 675 skipped. The slow tier reports 243 tests passed and 962 skipped.
The in-diff mutation run reports two caught, zero missed, zero timeouts, and zero unviable.
All ten listed CI stages pass. Both link decoder fuzz runs complete without a crash. The job and gate exit zero.
This Linux evidence does not establish execution of Mac-only code.
The earlier red gate remains historical evidence; this verdict uses the new exact-head gate.

No finding remains open in this pin and merge delta.
This verdict closes no conformance ID, deferred behavior, or separate package proof hold.
Integration owns its cross-package review. The lead owns the merge decision.

VERDICT: CLEAN



## PR #182 Round 1 — 2026-10-09

- Exact head: `217de31035b309dbebec582fc412323f06256785`.
- Branch: `stage1/p5-pin-v0.1.19`.
- Accepted v1 base and parent: `13d7db0925cd080b5734a7b118c7fb440aa28c28`.
- Tree: `cb41e321af1cf1e9aeaabc39805f1c67677c2e63`.
- Contracts tag: `contracts-v0.1.19`, commit `636bc1babcb410464bc40a5862895a1dc3260f5c`.

The reviewer read the complete seven-file delta, changed contract API, A7-1 and A9-1, current PR body, and supplied gate.
The reviewer changed no product code and ran no builds, tests, mutation jobs, or gates.

### Pin and artifacts

All six workspace contracts dependencies use v0.1.19.
All nine contracts lockfile entries use the exact tagged commit.
After normalization of the contracts source entries, the lockfile is identical to the accepted base's lockfile.
No package version, checksum, or dependency list changes.
The checked-in Core ledger matches the tagged ledger, byte for byte: 675 IDs.
All Core conformance files remain unchanged, including the pending list and deferred table.
The deferred and withdrawn copies match the new tag, byte for byte.
The reviewer compared all nine changed Core transcripts as JSON values.
Each transcript changes only InvalidInput from its string form to its object form.

### InvalidInput adaptation

The contract type is now `InvalidInput { field: Option<String> }`.
Its serde options encode the error as an object, with an omitted field when the field is None.
The numeric variant value, caller category, and synchronous timing remain unchanged.
Core uses the contract type directly and adds no alternative error type or serialization rule.

The existing invalid helper returns field None for input checks whose clauses name no field.
The new invalid_field helper supplies the field for the existing A7-1 attach checks.
The checked names are route_tag, owner, route_limits.max_frame_bytes, route_limits.max_screen_frame_bytes, and query_deadline.
The comparison bounds and check order remain unchanged. The checks still return before route reservation.

The adapted host tests compare each required field name and compare None for the other input errors.
The query-deadline cases check the missing value, a positive value below 1 ms, and a value above the maximum.
They retain acceptance at 1 ms and at the maximum.
The worker-link proof also requires query_deadline for the missing deadline of an answering route.
The refusal script uses the contract's object form for its InvalidInput samples and transport case.

The PR body states the remaining boundaries:

- Core has no A9-1 check against the route's own attached-frame size yet. A9-1 requires route_limits.max_frame_bytes for that later refusal.
- The connect_deadline check remains with the pending AttachWebRtc path.
- The A15 host text check remains separate work and will use the text field.

This verdict does not certify those deferred checks or close their pending IDs.
The PR body has the required Prior art note and describes the contract field and helper.
The delta adds no dependency, mutation exclusion, platform-specific code, or native binding change.

### Supplied exact-head gate

The full Linux gate is `~/botster-sessions/shared/core-stage1/gate-logs/pin-v0.1.19-217de310.log`.
It names the exact reviewed head and accepted v1 base.
The default tier reports 920 tests passed and 675 skipped. The slow tier reports 243 tests passed and 962 skipped.
The in-diff mutation run tests 20 mutants: 17 caught, zero missed, zero timeouts, and three unviable.
All ten listed CI stages pass. Fuzz reports no changed crate with a decoder harness and runs no harness.
The remote job and gate exit zero. This Linux evidence does not establish execution of Mac-only code.

No finding remains open in this pin and error-payload delta.
This verdict closes no conformance ID or separate package proof hold.
Integration owns its cross-package review. The lead owns the merge decision.

VERDICT: CLEAN

## PR #188 Round 1 — contracts-v0.1.20 — 2026-10-09

- Exact head: `e05d603bde49985015031f676dc64950fa2d7324`.
- Branch: `stage1/p5-pin-v0.1.20`.
- Accepted v1 base and parent: `a14e9dc2b61a5426485f9c7f0f829c900e03bdc7`.
- Tree: `6506cce83b99ed52fa3fb5cfd6bf6a747712cffc`.
- New contracts tag: `contracts-v0.1.20` = `03891658e793e5400ba46b5bc003b5d9f952f5e2`.
- Previous tag: `contracts-v0.1.19` = `636bc1babcb410464bc40a5862895a1dc3260f5c`.
- Risk tier checked first: HIGH, BUILD.md at contracts `56bd0a5347a537d25bbee65a67854e0e317a9b9a`, rule 4.

### C20-F1 — LOW — CLOSED — The PR contains its required Prior art note

The initial PR body lacked the note required for every PR by pair-common.md.
The reviewer sent this finding directly to P5 and copied integration.
P5 added the note: nothing was reused or hand-rolled; the pin changed through cargo update with no code change.
The reviewer verified the edited PR body and unchanged exact head.
This closes C20-F1 within this round.

### Pin and contracts checks

The reviewer read the complete three-file Core delta, new contracts delta, new transcripts, control documentation, R-37, PR body, and supplied gate.
All six workspace contracts dependencies name contracts-v0.1.20.
All nine contracts lock entries name the same tag and full commit 03891658.
Replacing their source strings with the previous strings reproduces the entire previous lockfile byte for byte.
No version, checksum, dependency list, or other lock data changes.
The pin comment names the correct tag, commit, and rulings through R-37.

The only contracts crate change is twelve added lines in botster-core-conformance/tests/vocabulary.rs.
They recognize five documented controls: hold_handoff, release_handoff, oracle_screen_payload, alloc_window, and alloc_peak.
No crate implementation or API changes.
The only Core transcript changes are two additions:

- `conf::dp_3_frame_limit_checked_before_allocation`
- `conf::ou_9_attach_is_sync_baseline_in_pump`

Both are marked fake: structural under R-12. A structural fake result does not prove real Core behavior.
The allocation transcript compares allocation peaks for lengths 4097 and 0x7FFFFFFF and expects the typed frame-too-large close.
R-37 permits an observer only at the allocator boundary in botster-core-testkit, without worker instrumentation, waits, or locks.
The observer must measure the process that runs the route reader. A different process must report unsupported_control.
The baseline transcript holds handoff, verifies synchronous registration, changes the model, then compares the released baseline with the fresh oracle screen.
It reads the screen payload through a libghostty oracle and does not write expected terminal bytes.

Both new transcript ids remain in Core's pending list, as the lead requires. Their controls belong to P4a.
The reviewer checked that the pending-id set is identical to the accepted base.
Only the two A15 reason comments change to name contracts-v0.1.20.
The new tag still contains no A15 transcript.
The 675-id Core ledger, Core deferral table, and copied contracts deferred and withdrawn files remain byte-identical to the base.
The copied status files match the new tag.
The contracts ledger, deferrals, withdrawals, and replacement-map JSON are unchanged between tags.
No existing Core transcript or frozen/current contract text changes.
The candidate Core A17, HC A8, and HP A9 documents do not change the frozen contracts or withdraw ids in this pin.
This review does not approve those candidate amendments or the future P4a control implementation.

### Supplied exact-head gate and scope

The full Linux gate is `~/botster-sessions/shared/core-stage1/gate-logs/pin-v0.1.20-e05d603b.log`.
It names the exact reviewed head and accepted v1 base.
The default tier reports 941 tests passed and 656 skipped.
The slow tier reports 243 tests passed and 962 skipped.
All ten listed CI stages pass. Both the job and gate exit zero.
Mutants reports that the diff changes no Rust source and produces no outcomes.json; no mutation campaign ran.
Fuzz reports no changed crate with a decoder harness and runs no harness.
The unchanged harness keeps both new transcript ids pending and never counts them as passes.

No package finding remains open. This verdict closes no conformance id or separate proof hold.
Integration must supply its separate CLEAN before this HIGH pin move merges.
The reviewer changed no product code and ran no builds, tests, mutation jobs, or gates.
The lead owns the merge decision.

VERDICT: CLEAN

## PR #188 Round 2 — v1 merge delta — 2026-10-09

- Exact head: `b7980e6d27d2fb4ad25b2575cac1326cdff22014`.
- Tree: `c13fb92ab459bbe6cb199a5d6723425c678c3cd0`.
- First parent: the CLEAN pin head `e05d603bde49985015031f676dc64950fa2d7324`.
- Second parent, PR base, and gate base: `8bc21dd516f20f26002f99683b63c6a11adf0d43`.
- Risk tier checked first: HIGH, BUILD.md rule 4, as the current PR body states.

### C20-F2 — LOW — CLOSED — The PR cites the exact merge-head gate

The initial Proof section still called e05d603b the exact head and cited only its 941-test gate.
The reviewer sent C20-F2 directly to P5 and copied integration.
P5 corrected the body to cite b7980e6d and its 946-test default gate.
The body also records the merge composition and keeps the previous gate as earlier evidence.
The reviewer verified the correction and unchanged exact head. This closes C20-F2 within Round 2.
The required Prior art note and C20-F1 closure remain intact.

### Merge composition

Both branches changed core-pending.txt, so the disjoint-path exception does not apply. This head requires a delta review.
The reviewer read the current three-file PR diff and the imported v1 changes.
The diff from old base a14e9dc2 to the CLEAN pin head equals the diff from new base 8bc21dd5 to this head, except index lines.
The delta from the CLEAN pin head to this head equals the v1 delta from a14e9dc2 to 8bc21dd5 exactly.
No extra change entered through the merge.

Cargo.toml and Cargo.lock are byte-identical to the CLEAN pin head.
The six direct pins and nine lock entries therefore retain the verified contracts-v0.1.20 tag and commit.
The imported inbound.rs and flow_edges.rs are byte-identical to the CLEAN #187 head `2b5ba07e5bcff471370fab07e2436a103cfef91b`.
That review's proof ruling and both finding closures remain valid; this merge adds no real-process test code.

The pending-id set equals the new v1 base.
Relative to the previous pin head, only the three #187 ids leave pending: am_3_exactly_one_completion, lc_7_remove_order_and_completion, and or_2_session_order.
Both new contracts transcript ids remain pending. Both A15 comments retain the new tag's name.
The Round 1 contracts, ledger, API, and status checks remain valid.

### Supplied exact-head gate and scope

The full Linux gate is `~/botster-sessions/shared/core-stage1/gate-logs/pin-v0.1.20-b7980e6d.log`.
It names the exact reviewed merge head and accepted new v1 base.
The reviewer checked PASS for all three imported conformance trials and all three #187 regression tests.
The default tier reports 946 tests passed and 653 skipped.
The slow tier reports 243 tests passed and 962 skipped.
All ten listed CI stages pass. Both the job and gate exit zero.
Mutants reports that the diff changes no Rust source and produces no outcomes.json; no new mutation campaign ran.
The imported #187 code had five caught mutants in its reviewed exact-head gate.
Fuzz reports no changed crate with a decoder harness and runs no harness.

No package finding remains open. Integration must supply its separate CLEAN on this HIGH merge head.
This verdict closes no additional conformance id or separate proof hold.
The reviewer changed no product code and ran no builds, tests, mutation jobs, or gates.
The lead owns the merge decision.

VERDICT: CLEAN

## PR #205 Round 1 — contracts-v0.1.21 — 2026-10-09

- Exact head: `dd2002e51390a812401d6d3d0a21575cb60474f9`.
- Tree: `c770dfcdf9981664c170ca59591b20b5d03b5b2a`.
- First parent: `8debbea28d31746c9efd7f6439a9dbbe1ce7e5c8`.
- Second parent, PR base, and gate base: `7e35566614bc61140ec254331db48ed7f568f710`.
- Risk tier checked first: HIGH, BUILD.md rule 4. The PR body names this rule and both reviewers.

The lead authorized this pin move after contracts-v0.1.21 became available.
The reviewer read the four-file Core delta, relevant contracts crate delta, A18 candidate 2, and manifest-final36.
The PR cites #188 as prior art.

All six direct contracts dependencies use contracts-v0.1.21.
The tag resolves to `60a4169978e3f704f46ab0578f9993013fd4b810`, which matches all nine contracts sources in Cargo.lock.
The lock retains 222 packages. Its only other change adds data-encoding to botster-hub-contract's dependencies.
The new contracts manifest requires that dependency. The lock already contains its package.
The pin comment identifies the correct commit and manifest.

The new Core crate items are additive constants: CORE_SERVICE_TRANSPORT_UNIX_LANES_1 and ROUTE_FEATURES.
No Core source defines a conflicting name. Core has no service transport environment value to replace yet.
The conformance driver adds `let without`, which removes only the named object field.
Its tests cover present and absent fields, and invalid object and field-name arguments.
The ad_3 transcript excludes only last_output_at from both terminal_state values, as A18-1 requires.
No Core transcript changes in this worktree. ad_3 remains pending for the separate A18 implementation.

The checked Core ledger exactly matches all 679 Core entries in the pinned contracts ledger.
The four new entries are the four A18 IDs. All four enter core-pending.txt and have no transcript in this tag.
The contracts pending file also lists all four. No prior pending ID leaves or returns.
The two A15 reasons name the new tag, which still has no A15 transcript.
Copied contracts deferred and withdrawn files match the new tag exactly.

The merge retains the four new A18 IDs and all nine pending removals from accepted #204.
The final pending set equals the new v1 pending set plus the four new ledger IDs.
No Rust source, mutation exclusion, timeout, or Core behavior changes relative to the gate base.
The head contains the current v1 tip. The gate records that same tip as its base.
The lead owns the plan pin update and clause-list regeneration when this ledger lands, as plan section 6.1 states.

The supplied Linux gate is `~/botster-sessions/gates/botster-core-stage1-p5-pin-v0.1.21-dd2002e5-pool-20261009-162431-34685.log`.
It names the exact reviewed head and base. All ten CI stages PASS; job and gate exit zero.
Default: 1170 tests passed, 566 skipped. Slow: 249 passed, 1027 skipped.
Conformance: 113 passed, zero failed, 501 pending with transcripts, 61 pending without transcripts, two deferred, and two withdrawn.
The default and explicit NEXTEST_PROFILE=slow mutation stages both report no Rust source change and PASS.
No mutation campaign runs. Fuzz reports no changed crate with a decoder harness.
All 39 active minimum IDs have PASS lines. The minimum count stays 39 / 70; no real-harness gain is claimed.

No package finding remains open. Integration must supply its separate exact-head CLEAN for this HIGH pin move.
This verdict approves only the pin move. It closes no pending ID or separate real-driver proof hold.
The reviewer changed no product code and ran no builds, tests, mutation jobs, or gates.

VERDICT: CLEAN

## PR #211 Round 1 — contracts-v0.1.22 — 2026-10-09

- Exact head: `19e918071660a12a5a087960a45d9530d1a0ab7b`.
- Tree: `8428647f63e5da8028f4109788f59939b690195b`.
- Parent: `a831316e6c41683cce4e00fef268e1c2d80fc356`.
- PR and gate base: `4b227d46ef5453ab8a3f7daaa4dc33562b4be83b`.
- Risk: HIGH, BUILD.md rule 4. The reviewer checked this tier first.

The lead assigned this pin review to P5 as the P1 package owner.
The reviewer read the complete eighteen-file Core delta, relevant contracts crate delta, A17 candidate 2, and final37 manifest record.
The PR cites earlier pin-count updates as prior art and lists each required A17 removal.
The approved A17 amendment moves WebRTC to the Hub and leaves Core with the Stream transport.
The lead's stated scope permits removals required by that amendment and the two tests of pinned counts.

### Pin and source checks

All six direct contracts dependencies use `contracts-v0.1.22`.
The tag resolves to `af5771cf962eb26b7074d486dafbb3360ff50d01`.
All nine contracts lock sources name that tag and commit.
Cargo.lock retains 222 packages. Only those nine source fields change; versions, checksums, and dependencies stay unchanged.
The pin comment identifies the correct commit and manifest-final37.

The contract crate removes WebRTC operations, values, errors, features, transport fields, and limits as A17 specifies.
Its prelude removes `Answer` and `OfferRefusal` and adds no names.
The codec removes the route's `max_chunk_bytes` field. Core and worker source have no remaining use of that field.
The tagged conformance driver adds attach-option substitution and removes WebRTC handling.
The tagged runner adds `Limits::real`; this PR does not activate a real harness or alter a Core timeout value.

Core removes the `AttachWebRtc` admission arm and `AppliedRouteLimits.max_chunk_bytes` construction.
The transport check stays because `RouteTransport` remains non-exhaustive.
Its error detail now names the connected Stream rule.
Tests remove only obsolete WebRTC cases and deleted option or limit fields.
The refusal table removes the deleted operation and retains the remaining rows.
The two pinned-count tests now require at least 25 operations, 26 withdrawn IDs, and seven withdrawals without replacements.
Those counts match the tag. No parser change, new lifecycle behavior, or new process proof enters this PR.
The stream comment and design row remove the obsolete WebRTC scope.

### Ledger and transcript checks

The checked Core ledger exactly matches all 682 Core entries in the tagged ledger.
The copied withdrawn and unchanged deferred files match the tag byte for byte.
The withdrawn file adds 23 IDs across contracts, including 15 Core IDs under A17.
All 15 Core IDs were pending and leave pending. None was a passing or minimum ID.
The three new A17 ledger IDs enter pending. The tag lists them as pending and provides no transcript for them.
No existing pending ID returns. The remaining active conformance ID set is unchanged.

The tag modifies 21 Core transcripts and deletes the 15 withdrawn Core transcripts.
The reviewer read the four changed transcripts that already pass on v1.
The Stop-race correction waits for output after the scripted `ignore_sigterm` step.
The zero-limit transcript removes only the three deleted limits.
All four changed active transcripts have PASS lines at the reviewed head.
Core edits no transcript in this worktree.

All 69 minimum IDs remain in scope. All 41 active minimum IDs have PASS evidence.
Testkit progress remains 41 / 69. This pin review makes no real-harness progress claim.
The lead owns the plan pin and clause-list update after landing.

### C22-F1 — LOW — CLOSED within Round 1

The PR body initially said that 19 Core transcripts changed.
The tag delta has 21 modified transcripts and 15 deleted transcripts; the body already listed all 21 modified transcripts.
The reviewer sent this finding directly to P6 and copied Astra.
P6 corrected 19 to 21 without changing the head.
The reviewer read the complete corrected GitHub body and verified the unchanged exact head.
Astra independently confirms the count and body closure.
No package finding remains open, including LOW findings.

### Supplied gate and scope

Log: `~/botster-sessions/gates/botster-core-stage1-p6-pin-v0.1.22-19e91807-pool-20261009-181249-20493.log`.
The log names the exact reviewed head and current v1 base. The head contains that base.
All ten CI stages report PASS. The job and gate exit with zero.

- Default: 1284 passed, 561 skipped.
- Slow: 254 passed, 1275 skipped.
- Conformance: 121 passed, zero failed, 478 pending with transcripts, 64 pending without transcripts, two deferred, 17 withdrawn.
- Both mutation steps: six tested, four caught, two unviable, zero missed, zero timeout.
- Fuzz: no changed crate with a decoder harness.

The extra outer `NEXTEST_PROFILE=slow` command does not enable slow-feature tests.
The #181 launcher explicitly selects the `mutants` profile, which has no terminate-after.
No mutation exclusion changes occur in this PR.

Integration must publish its separate exact-head CLEAN artifact for this HIGH pin move.
The later #206 merge must remove any uses of fields that A17 deletes.
This verdict closes no pending ID and no separate #210 finding or real-adoption proof duty.
The reviewer changed no product code and ran no builds, tests, mutation jobs, or gates.

VERDICT: CLEAN

## PR #214 — Round 1 — contracts-v0.1.23

Reviewed head: `08792a76fff142b14c72f77a1c3cf3a84e60c76f`.
Tree: `8ca5f6006e95aed22bf6857ac07c2dbae29c2333`.
Parent, PR base, and gate base: `f6128fd6dd96a27f82520c82b9553d038dfa9dba`.
Contracts tag: `contracts-v0.1.23` at `fe3eb952d6f61b89cf7bcbe8c54ab079987ec002`.

The reviewer checked the HIGH tier first. BUILD.md rule 4 requires HIGH for this pin move.
The PR names P5 as the P1 package reviewer and Astra as the integration reviewer.
The Prior art note cites the earlier pin moves and their list and status checks.
The reviewer read the complete six-file delta, the full PR body, and the supplied gate log.
No finding remains open, including LOW findings.

### Pin and dependency checks

All seven direct contracts dependencies use the new tag.
All nine contracts sources in Cargo.lock use the exact tagged commit.
The lock retains 222 packages. Only the nine source fields change; versions, checksums, and dependencies stay equal.
The local contracts tag resolves to the stated commit.
The final38 manifest includes accepted Core A19 candidate 2 and its crate change on the final branch.

The reviewer read the complete dependency delta that Core uses.
The R-46 driver adds `CoreHarness::progress_is_injected`, with `injects_clock` as its default.
An idle wait jumps only when the clock and progress are injected.
With real progress and an injected clock, the step limit bounds the wait and `advance_clock` moves the clock.
TestkitHarness already returns true from `injects_clock` and does not override the new method. Its existing jump behavior stays equal.
The tagged driver tests cover both injected-progress branches and the real-clock deadline bound.
RealCoreHarness is not present on v1. This verdict makes no real-harness acceptance claim.

The codec delta adds only a comment on `TerminalQuery.query_id`.
The comment states the A19-1 u64 decimal-string rule. The type and wire representation stay equal.
The reviewed Core head has no query_id producer.
The plugin-contract delta does not enter Core's dependency graph.
No Core runtime code, transcript, timeout, or mutation exclusion changes in this PR.

### Conformance accounting

The checked Core ledger equals the tag's sorted 689 Core IDs byte for byte.
The ledger adds seven A19 IDs. All seven enter pending, are pending in the tag, and have no tagged transcript.
The copied withdrawn and unchanged deferred files equal the tag byte for byte.
A19-1 withdraws `conf::a9_1_frame_cap_equal_to_the_attached_frame_attaches` and names its replacement.
That ID was pending, had no transcript, and is not a minimum ID. Its removal loses no passing ID.
The global withdrawal count changes from 26 to 27. Seven withdrawals still have no replacement.
The pinned-count test matches those facts.
The revised pending reasons still refer to IDs pending in the new tag.

The tag adds three clause-cited `pump_until RouteClosed` steps to the existing ou_2b transcript.
That ID remains pending in Core. The pin move claims no new pass from this change.
The active suite remains set-equal at 124 IDs. Every active ID has a PASS line in the supplied gate.
The approved plan 23p keeps all 69 minimum IDs. All 44 active minimum IDs have PASS lines.
Minimum testkit progress remains 44 / 69. No A19 ID is minimum.
The plan pin is `~/botster-sessions/pins/stage1-plan.a7980384.md`.
Its SHA256 is `28f2220a44a3e2580baf896049cb10e82e27323f4edca7dc25b51d93827ffd46`.

### Supplied gate and scope

Log: `~/botster-sessions/gates/botster-core-stage1-p6-pin-v0.1.23-08792a76-pool-20261009-195055-94076.log`.
The log names the exact reviewed head and its parent as the base.
All ten CI stages report PASS. The job and gate exit with zero on msa1.

- Default: 1329 passed, 565 skipped.
- Slow: 256 passed, 1294 skipped.
- Conformance: 124 passed, zero failed, 475 pending with transcripts, 70 pending without transcripts, two deferred, 18 withdrawn.
- Both mutation steps: the diff has no mutant, as reported by `cargo mutants --list`. No mutation run starts.
- Fuzz: no changed crate with a decoder harness.

The extra outer `NEXTEST_PROFILE=slow` command does not enable slow-feature tests.
This PR changes no exclusion citation and makes no slow-feature mutation claim.
Integration must publish its separate exact-head CLEAN artifact for this HIGH pin move.
The later A19 implementation and RealCoreHarness work remain separate.
This verdict closes no #210 finding.
The reviewer changed no product code and ran no builds, tests, mutation jobs, or gates.

VERDICT: CLEAN

## PR #216 — Round 1 — contracts-v0.1.24

Reviewed head: `5b150cf4c5d704673ce6c6284723384c48ae5987`.
Tree: `18e34b5a2aa7d4f11a646fdba5de62d15042c6af`.
Parent, PR base, and gate base: `9103dca15664496630c4397ea3a60721427b38da`.
Contracts tag: `contracts-v0.1.24` at `d79aed5e84d4a8bc9b76f6163ca6bc4a9d89b951`.

The reviewer checked the HIGH tier first. BUILD.md rule 4 requires HIGH for this pin move.
The PR names P5 as the P1 package reviewer and Astra as the integration reviewer.
The Prior art note cites the previous pin moves and the ledger-generation rule.
The reviewer read the complete five-file Core delta, full PR body, relevant tagged crate and transcript changes, and supplied gate.

### C24-F1 — HIGH — OPEN at the reviewed head

The new `conf::a20_1_testkit_proven_ids_are_exactly_the_named_list` trial reports PASS without the required Core proof.
Frozen A20 states: "the real-tier runner's list of testkit-proven ids equals the list in A20-1. Each listed id passes on the testkit."
The tagged transcript calls `type_check: a20_1_testkit_proven`.
That check reads the contracts crate's `A20_TXT`, `PENDING_TXT`, and `WITHDRAWN_TXT`.
It does not read Core's list or validate Core's real-tier runner.
Core's runner calls this upstream check without a local A20 check.
Core has no real-tier runner at this head, and no Core runner consumes `TESTKIT_PROVEN`.
Core still lists the class member, `conf::dp_3_frame_limit_checked_before_allocation`, as pending.
The 183rd reported PASS therefore does not prove Core's required A20 test.

Required change for this pin-only PR: add the new A20 ID to `core-pending.txt` with the two missing Core proofs as its reason.
Activate it later when Core's real-tier runner uses the exact class and the class member passes on Core's testkit.
Update the PR body and supply the replacement-head gate.
The reviewer sent the finding directly to P6 and copied Astra. Astra independently confirms the same gap as R1-1 HIGH.
P6 accepted the finding and supplied fix commit `d18b41726b4fffb1e2043d5220b32a34444cd27b`.
Its two-line pending-list delta addresses the source finding. Its gate and updated body are pending, so it has no verdict yet.
This Round 1 verdict applies only to the original head.

### Remaining checks

All seven direct pins and all nine lock sources use the exact new contracts tag.
The local tag resolves to that commit. The lock retains 222 packages and changes only nine source fields.
The final39 manifest records accepted A20 candidate 1 and its crate change on the final branch.
The Core ledger equals all 690 tagged Core IDs byte for byte. The only added ID is the A20 check.
The deferred and withdrawn copies remain unchanged and equal the tag byte for byte.
The pending ID set stays equal at 487. The five revised reasons still refer to IDs pending in the tag.
No prior passing ID is lost. All 182 prior active IDs and all 46 active minimum IDs have PASS lines.
Minimum testkit progress stays 46 / 69. This verdict accepts no real-tier milestone.

The reviewer read the tagged R-46 clock steps and the in_9 reorder.
Three Stop transcripts wait for Stopping before advancing the injected clock by the configured grace.
The key-repeat transcript checks stateless refusals before the accepted write can post an event.
The four changed existing transcripts retain PASS evidence at the Core head.

The R-48 probe change clears only the specified canonical and echo flags on terminal stdin, with VMIN 1 and VTIME 0.
It retains the input, output, control, and signal flags. Non-terminal stdin stays unchanged.
The tagged PTY test checks a byte without a newline, no echo, and the retained terminal flags.
Core's prebuild step installs the probe from the contracts tag.
The supplied upstream gate at the exact tag has a PASS line for the named PTY test and exits zero.
Upstream log: `~/botster-sessions/gates/botster-contracts-p6-r46-transcripts-d79aed5e-pool-20261009-202249-29552.log`.
The upstream gate reports 1990 default passes, 20 slow passes, and three caught mutants.
The Core review adds no runtime code, transcript, timeout, or mutation exclusion.

### Supplied Core gate and scope

Log: `~/botster-sessions/gates/botster-core-stage1-p6-pin-v0.1.24-5b150cf4-pool-20261009-203054-60221.log`.
The log names the exact reviewed head and its parent as the base.
All ten stages report PASS. The job and gate exit zero on msa1.

- Default: 1391 passed, 507 skipped.
- Slow: 256 passed, 1298 skipped.
- Conformance: 183 reported passes, zero failures, 417 pending with transcripts, 70 pending without transcripts, two deferred, 18 withdrawn.
- Both mutation steps: cargo-mutants lists no mutant, so no mutation run starts.
- Fuzz: no changed crate with a decoder harness.

The A20 reported pass does not close C24-F1. A green gate cannot replace the required Core proof.
The reviewer changed no product code and ran no builds, tests, mutation jobs, or gates.
Await replacement-head READY and evidence. No NOT CLEAN report goes to the lead.

VERDICT: NOT CLEAN
