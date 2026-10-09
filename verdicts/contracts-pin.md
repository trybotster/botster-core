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
