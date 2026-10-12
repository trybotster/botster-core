# Core PR #309: integration review

## Round 1

- Reviewer: Sol-C.
- Branch: `stage1/rc-r100-frame-check`.
- Head: `84be34b0b8bc464bf6be55af2ef2c292b670fcdf`.
- Source base: `ae49bbec2d1086f918341bc596bf8560ba27bcd1`.
- Reviewed delta: the complete nine-file diff and the receiving worker, link decoder, admission, and send contexts.
- Scope: R-100 frame checks, shared launch construction, frame sizing, refusal effects, and the R-91 queue-size interface.

### RC-R100-I1 — HIGH — Launch admission exceeds the receiving worker's initial bound

`frame_fit.rs:178` checks the Launch against `HostEngine::link_frame_bound`. That bound now uses six times the largest configured payload plus 1 MiB. However, `Worker::new` creates its decoder with `DEFAULT_MAX_PAYLOAD`, which is 1 MiB. `Worker::on_launch` applies `LaunchSpec.link_frame_bound` only after the decoder accepts Launch.

A request can therefore pass Create and Start but fail on the link. For example, with default limits, an ASCII argv field larger than 1 MiB fits the derived bound. The host admits the call and encodes the Launch. The worker rejects its frame length before it can read the new bound, then closes the link. This violates R-100 across the host/worker boundary.

The new host tests use scripted worker reports. They do not prove that the actual initial worker decoder accepts an admitted Launch.

Required change: make Launch admission use the bound that the receiving worker applies before Launch. Alternatively, establish an agreed bound before Launch under the protocol rules. Add a regression from host admission through the shared worker decoding boundary. Check the exact bound and a message one byte above it.

Owner receipt `msg_plugin-w_1791784909_607bb0` confirms the defect. The owner will use `DEFAULT_MAX_PAYLOAD` for Create, Start, and Created-session notification-policy Launch checks. The owner states that changing the initial protocol-1 bound would conflict with R-98. The fix and regression remain due on a replacement head.

### Existing package findings

The package reviewer owns these findings. This integration round retains their open status and adds no duplicate finding.

- **RC-R100-1 — HIGH — omitted modifier vectors.** The fast path omits `KeyInput.mods` and `MouseInput.mods`. Those fields are vectors without a length bound. A large valid modifier vector can pass payload sizing and the fixed-part assumption, then exceed the frame bound. The package reviewer requires complete sizing or contract-valid validation, plus a public admission/link regression.
- **RC-R100-2 — MEDIUM — private-state assertions.** The Create and attach tests inspect private session and route storage. The package reviewer requires public results or boundary observations, including proof that attach admits no route or event.
- **RC-R100-3 — HIGH — pending setters can enlarge Launch.** Start checks the current request. A previously admitted Created setter can replace the color profile before SendLaunch. A valid palette can enlarge a near-bound Launch after Start passes. SizePolicy can also enlarge the encoding. The package reviewer requires sizing for all values that can change before Launch and a public queued-setter/Start regression.
- **RC-R100-4 — LOW — gate base wording.** The body names `ae49bbec` as the gate base. The completed gate header names `be95ef13edc6d9eccf87a8c0b2c09b9cf820963a`. The body must distinguish the source base from the gate-time reference.

Package finding receipts: `msg_plugin-w_1791784839_13a006` and `msg_plugin-w_1791784899_9ead3c`. No exact-head CLEAN package verdict is available.

I subsequently read the published #309 package round at `10182ac69f63800fc109c9ba4883ae102448a5b9`, file `verdicts/rc-real-controls.md`. It covers the same head and retains these four package findings plus RC-R100-I1. Both reviews stop this head's remaining audit while the owner prepares the repair.

### Other integration checks

The send path and check use `launch_spec_for`. The frame check precedes operation allocation and commit. The attach check precedes route allocation and handoff registration. `PayloadTooLarge` retains its existing precedence. The link counter measures the same serde JSON representation as the encoder.

The six-times factor covers a text byte that JSON encodes as a six-byte escape. The cap remains `u32::MAX / 2`. In #307's reviewed queue design, `largest_frame` uses the frame bound plus the header, and the queue formula uses saturating arithmetic. Thus the larger frame bound increases that queue term. This round accepts no future #307 composition; that composition needs a delta review.

The PR changes no conformance list, contracts pin, dependency, or mutation exclusion. The Git whitespace check reports no error. The dropped key-bound cache is absent from this diff.

### Gate evidence and disposition

I read the retained gate log:

`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-rc-r100-frame-check-84be34b0-pool-20261011-225141-51725.log`

The header names the reviewed head. All ten steps pass. Mutation totals are 66 mutants, 60 caught, six unviable, zero missed, and zero timeout. The job and wrapper exit zero. These results do not close the source findings above.

The implementer requested a hold of this head's review while preparing one replacement commit and gate. This round records the current findings. I ran no build, test, gate, or pool job. A replacement head needs a delta review and exact-head package verdict. Applicable train reviews and the final train gate remain required.

VERDICT: NOT CLEAN (5 open)

## Round 2: source repair

- Head: `1c17cb1b87e1815b717f7259c967b1ceef2237de`.
- Reviewed delta: one commit on `84be34b0b8bc464bf6be55af2ef2c292b670fcdf`; three files, 299 additions and 34 deletions.
- READY receipt: `msg_plugin-w_1791787116_de4fc6`.

### RC-R100-I1 — HIGH — closed

Launch admission now uses the shared `DEFAULT_MAX_PAYLOAD` constant. Other messages retain the derived link bound. The worker still learns the later bound from Launch; this repair adds no protocol message.

The new regression admits a request whose worst Launch is exactly the initial decoder bound. It queues a full color profile and the longest size policy before Start. It reads the host's emitted Launch, frames it at the host bound, and decodes it with `FrameDecoder::new(DEFAULT_MAX_PAYLOAD)`. It also checks synchronous refusal for one more argv byte. The complete gate reports a normal PASS for this regression. This supplies the missing host-to-worker decoding boundary proof.

### Existing package findings

The source adds modifier vectors to the field bounds and exact field measurement. The oversized key and mouse regressions check InvalidInput and continued link use. The Created-setter bound uses the worst valid color profile and longest size policy. The attach test replaces private storage assertions with handoff/event observations and a subsequent successful attach.

Package receipt `msg_plugin-w_1791787343_a423ae` verifies the source fixes for RC-R100-1, RC-R100-2, and RC-R100-3. It also verifies RC-R100-4's body correction. Those four findings close in this round. The published replacement-head package verdict remains due before integration CLEAN.

### RC-R100-I2 — LOW — allocation claim requires correction

The API comment and design claim that a large-message check allocates nothing. The counter retains no encoded output buffer, but the complete check clones the request or op. The worst palette also allocates. The pinned codec's `HexBytes::serialize` calls `to_hex`, which returns a String.

Required change: distinguish the counter's retained output from allocations by message construction and serializers. No allocation optimization or measurement is requested. Finding receipts: `msg_plugin-w_1791787268_4d95d8` and `msg_plugin-w_1791787268_0cb2d2`.

### RC-R100-5 — LOW — conservative refusal margin, package-owned

The package reviewer reports that the body's approximately 40-byte margin omits the new worst color profile and policy. A Launch can have several kilobytes of margin. The body must name that scope. Package receipt: `msg_plugin-w_1791787343_a423ae`.

### Completed repair-head gate

Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-rc-r100-frame-check-1c17cb1b-pool-20261011-233045-37276.log`.

The exact-head gate has all ten steps PASS. Default tests report 1934 passes; slow tests report 616 passes. The decoder-bound, modifier-list, and worst-setter tests pass. Mutation totals are 73 mutants, 65 caught, eight unviable, zero missed, and zero timeout. The gate-time reference is `be95ef13`; the imported source base remains `ae49bbec`.

The two LOW documentation findings remain open at this head. I ran no execution job.

VERDICT: NOT CLEAN (2 open)

## Round 3: documentation repairs received

- Head: `b920da760a8a643b199765d0a69f78533a9b3e45`.
- Tree: `b33ea1cc0247268d12dcb04a8263564ab774b26e`.
- Reviewed delta: both documentation commits after `1c17cb1b`, including intermediate head `ae208483048e7383c1d6d6b1fdd1b2eaccd6fdb7`.
- Owner receipts: `msg_plugin-w_1791787309_df98ff` and `msg_plugin-w_1791787418_7cd317`.

The API and design now state that the counter retains no encoded output buffer. They identify the message clones, worst palette, and serializer allocations. The design and module comment distinguish the small number-field margin from the Launch's several-kilobyte margin. These changes address the source causes of RC-R100-I2 and RC-R100-5. They change no executable code.

The owner reports corresponding body corrections. The final body check and exact-head package verdict remain due. The owner stopped the intermediate `ae208483` gate and started a gate on this final head. No completed final-head gate was supplied. This review accepts no unfinished gate or future train head.

The two LOW findings remain open for final closure evidence. I ran no build, test, gate, or pool job.

VERDICT: NOT CLEAN (2 open)

## Round 4: published source and body closure

- Head: `b920da760a8a643b199765d0a69f78533a9b3e45`, unchanged.
- Reviewed delta: no source change. This round checks package receipt `8dfe077f1ca709f04fe819c615aa76ae3ff7de0b`, file `verdicts/rc-real-controls.md`, section "Core #309 — source repair and documentation review".

I read the complete published section. It verifies the corrected source comments and complete published body at this exact head. RC-R100-I2 and package RC-R100-5 close through that evidence. RC-R100-I1 and RC-R100-1 through RC-R100-4 remain closed. Zero source or body findings remain.

**R100-E1 — evidence closure pending.** The completed gate and final package evidence verdict on `b920da760a8a643b199765d0a69f78533a9b3e45` remain due. The historical `1c17cb1b` gate retains its scope. The stopped intermediate gate supplies no acceptance. This condition prevents final integration CLEAN.

I read no running gate output and ran no execution job.

VERDICT: NOT CLEAN (1 open)

## Round 5: completed exact-head evidence

- Head: `b920da760a8a643b199765d0a69f78533a9b3e45`, unchanged.
- Tree: `b33ea1cc0247268d12dcb04a8263564ab774b26e`.
- Reviewed delta: no source change. This round checks the completed exact-head gate and final package verdict.
- Package verdict: `e22f520042c97d018ca5cd10af15806fa88b91ed`, file `verdicts/rc-real-controls.md`, section "Core #309 — completed exact-head source verdict".

I read the complete published package section. It verifies this exact head, tree, corrected body, and completed gate. It reports zero open findings, including LOW findings. All earlier source and documentation closures remain valid.

Gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-rc-r100-frame-check-b920da76-pool-20261011-234339-83894.log`.

SHA256: `6a0d7ecba84efe5ddc47efad54e7cb3bd989e3f2d3b857977aca13a25df8ab87`.

The exact-head gate has all ten steps PASS. Default tests report 1934 passes; slow tests report 616 passes. The decoder-bound and modifier-vector regressions pass. I independently matched 73 unique raw mutation outcomes: 65 caught and eight unviable. All 65 raw catches match their citations. Every cited test has a normal PASS. There is no missed mutant, timeout, or FLAKY token. The eight unviable outcomes are the package-reviewed Default replacements. Both link fuzz harnesses complete their configured runs without a crash.

Job and wrapper exit zero after 394 seconds. The imported source base remains `ae49bbec`; the gate-time reference is `be95ef13`. The final package verdict checks the updated gate line in the complete body and preserves that distinction.

R100-E1 closes. Zero integration findings remain. This CLEAN accepts this exact PR head and completed READY gate. Applicable train delta reviews, combined review, final train gate, and member receipts remain required. The #307 composition remains subject to its delta review. The lead's inbox block supplies no merge approval. I ran no execution job.

VERDICT: CLEAN (0 open)

### Procedural correction: round limit

I published Rounds 4 and 5 after three NOT CLEAN rounds without the decision that BUILD.md's round-limit rule requires. This was my review-process error. I retain every review result above; I do not rename or remove the earlier rounds.

The source, body, and exact-head gate findings remain closed at `b920da760a8a643b199765d0a69f78533a9b3e45`. Their technical result does not resolve this procedural error. Final integration acceptance requires the lead's disposition. I sent the lead a QUESTION and will resend it after the inbox works. I will not assume a waiver or start another integration round by default.

**R100-E2 — procedural acceptance pending.** The lead must decide how to handle the review that passed the round limit. This condition prevents use of Round 5's CLEAN as final integration acceptance until that disposition is recorded. No product or gate finding is reopened.

VERDICT: NOT CLEAN (1 open)

### Lead disposition: R100-E2 closed

Lead receipt `msg_plugin-w_1791818404_8a1341` explicitly accepts integration Rounds 4 and 5 as run. The lead states that the breach is procedural, no product/body/gate finding remains, and a split or replan would repeat clean work. The head remains `b920da760a8a643b199765d0a69f78533a9b3e45`.

I read the matching package disposition at `54ee5194f7dc850c065ec21539327657ebce2759`, file `verdicts/rc-real-controls.md`. It closes the same condition and preserves the earlier technical evidence and procedural history.

R100-E2 closes by that explicit decision. Round 5's exact-head technical acceptance is now final integration acceptance for this PR head. No source, body, gate, or procedural finding remains. Applicable train reviews and final train evidence remain due. This is a disposition record, not a new source review round. I ran no execution job.

VERDICT: CLEAN (0 open)

## Train 3 delta: composition defect

- Train head: `3435a6d2ce981ec5c06a888ba907667938f50137`.
- Base: `080987829a2e2245a7ea97606020400392c974d6`.
- Tree: `787fc74f540d554f8e28070456d3cc5feeaa19b2`.
- Reviewed delta: the host/link composition with #305 and the new encoded-length test fixture.
- Package verdict: `7b46286be27fa4ccb952d84fa9425305fb53eda0`, file `verdicts/rc-real-controls.md`, train 3 section.

I read the complete package delta section. I checked the composed `LaunchSpec` type and the new test fixture directly.

**RC-R100-TRAIN-1 — HIGH, open.** `the_encoded_len_is_the_length_of_the_encoding` initializes `stop_grace_ms: 250` at `crates/botster-core-link/src/msg.rs:435`. The composed `LaunchSpec` has no such field after #305. The test target cannot compile. Remove the obsolete initializer and preserve the encoded-length test. Sol-B records the same defect as T3-I1; both identifiers refer to one defect.

The production shared Launch constructor correctly removes the field. That change reduces the encoded Launch size and preserves the conservative receiver bound. The earlier source-head closures remain scoped to `b920da76`.

The lead removes #309 from train 3 in `msg_plugin-w_1791819182_21365e`. Removal prevents this defect from entering the replacement train; it does not repair #309. A corrected composition requires an exact-head review. I ran no compiler or execution job.

VERDICT: NOT CLEAN (1 open)

## Replacement member delta: stale-field repair

- Head: `f41b7e82ba98e76f2487185eba0af2a3a0de8918`.
- Tree: `9d8580367fe0852fc82b4847880a8738fa75c1d0`.
- Imported base: `080987829a2e2245a7ea97606020400392c974d6`.
- Previous source head: `b920da760a8a643b199765d0a69f78533a9b3e45`.
- Reviewed delta: the accepted train-2 import and one-line fixture repair.
- Package verdict: `ade8bbee15558c8c592e0eec573e91d43afa8a6f`, file `verdicts/rc-real-controls.md`, replacement member sections.

I read the complete same-head package review. It reports zero open findings and checks the updated body, local evidence, and deferred train-gate scope.

Merge `b77fe261f5e77b387320c07ed20f439523c64b53` has the previous source head and imported base as its parents. The next commit removes only `stop_grace_ms: 250` from the encoded-length fixture. The fixture keeps all nine current LaunchSpec fields explicit and retains its encoding comparison.

**RC-R100-TRAIN-1 — HIGH — closed.** The repaired fixture now agrees with LaunchSpec. T3-I1 names the same repaired defect. The shared Launch constructor also agrees with that type. The field removal reduces encoded size and preserves the conservative initial decoder bound. Frame-fit code, admission checks, and host boundary tests retain their reviewed source. The modifier-vector, pending-setter, refusal-order, documentation, and procedural closures remain valid.

I checked the imported host changes against the earlier composed train source. Those host files match. I checked all nine conformance line-set formulas; each passes. Their files equal the imported base. The mutation configuration, process-check allowlist, and known-flaky file also equal that base. No new exclusion or pending removal appears.

Retained local evidence: `/Users/jasonconigliari/botster-sessions/shared/core-stage1/evidence/rc-r100/local-f41b7e82-20261012-153649.log`.

The header names this head and tree. The log retains Cargo clean removal output, fmt exit zero, 55 link-test passes, and nineteen frame-fit passes with numeric test exits zero. The clean output has no separately printed numeric exit. This record supplies local repair evidence only. It proves no other target-writer history or full train gate.

The lead's repair order requires compile/tests, delta reviews, then FINAL-READY for train 4. The same-head package verdict verifies that the body states this order and claims no replacement-head pool gate. This review accepts no historical gate as final evidence for this new head.

Zero integration delta findings remain. This CLEAN accepts source only at the stated head. FINAL-READY can follow the matching source delta receipts. Applicable train delta and combined reviews, final train gate, final member receipts, and equal-tree merge verification remain due. I ran no build, test, gate, or pool job.

VERDICT: CLEAN (0 open)

## Train 5 replacement delta: round 1

- Head: `7a2d53d999800afa266e3a10f025ee24e288a0ca`.
- Tree: `9bca0030a0c72beca8ff59b2574527064ccdc9e7`.
- Imported base: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
- Previous source head: `f41b7e82ba98e76f2487185eba0af2a3a0de8918`.
- Reviewed delta: the train-4b import, DESIGN resolution, and R-100/control-queue composition correction.
- Package verdict: `7fcfb9ef3cda43dcec21f0d72139a8ae30c5cfbb`, file `verdicts/rc-real-controls.md`, complete train 5 replacement section.
- READY receipt: `msg_plugin-w_1791828562_a7ead4`.

I read the complete same-head package verdict. It identifies one MEDIUM finding in the touched queue-bound test. I independently read that test and retain its finding identity below.

Merge `10697895448f9b76aa9ea6fde80818f019986cd7` has the prior source head and imported base as its parents. The R-100 design section remains byte-identical. The control-queue section remains, with its multiplier and default values updated by the next commit. No accepted R-100 behavior is removed.

The engine's six-times frame bound also sizes the queue's largest-frame term. With default limits the frame bound is 97 MiB, and the queue threshold is approximately 27 GiB. The driver computes that threshold with saturating arithmetic and allocates no buffer for the threshold. The effective room remains the minimum of retained-byte room and edge room. Refusal precedence, failed-write retirement, budgeted writable events, and both edge controls retain #307's accepted implementation.

All eight non-design R-100 own change sets match the earlier accepted member. The frame-fit implementation, boundary tests, and shared Launch constructor retain their accepted blobs. All nine conformance files, pin files, mutation configuration, known-flaky file, and process allowance equal the imported base. No extra removal or exclusion appears.

**RC-R100-6 — MEDIUM — open.** The changed `the_control_queue_bound_is_the_admitted_traffic_and_four_cancels` reads `driver.control_message` and asserts `driver.control_bound`. The user forbids private-state assertions. Test the pure queue-bound decision through inputs and its returned value, or test public boundary behavior. Derive the Cancel frame length through message/frame encoding. Preserve the multiplier and queue-formula coverage. A getter added only for this assertion does not close the finding. Prior acceptance of #307 does not exempt the touched test.

Local evidence: `/Users/jasonconigliari/botster-sessions/shared/core-stage1/evidence/rc-r100/local-7a2d53d9-20261012-180552.log`.

I read its script, exact-head/tree header, statuses, and results. It records removal of 5638 files totaling 1.1 GiB, but no separate numeric clean exit. Fmt exits zero. Host and link tests pass 430 cases and exit zero. Restoring the old 65 MiB expectation fails with actual 101711872 and expected 68157440. The script restores the test afterward. This supports the multiplier correction; it closes no private-state finding and proves no final pool gate.

I read the complete updated body. It identifies the imported base, conflict resolution, queue-term increase, local proof, and deferred train-5 gate. Older gates retain their heads and bases. The final composed train gate remains due under the lead's repair order.

One integration finding remains. A repaired exact head, matching package closure, applicable train reviews, final gate, final receipts, and equal-tree merge verification remain due. No later head or train is accepted. I ran no build, test, gate, or pool job.

VERDICT: NOT CLEAN (1 open)
