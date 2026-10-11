# Core #275 — oracle controls

## Round 3 — 2026-10-11 — final gate and merge receipt

Verdict: CLEAN (0 open) at `ad1a38098a474f3ca9770bdbc828be7be92e4783`.
Base: `b6fd4b98e5f95aba4a6a1dc13efb609036be36f2`.
Package source CLEAN carries at `a03c864a`. The package record still calls its final gate pending.
Sol does not claim a package final-gate verdict. Sol read the complete current PR body and the final gate evidence.
The body records both source verdicts, the Q7 carry, the successful base check, and the final gate.

Gate: `~/botster-sessions/gates/botster-core-stage1-rc-oracle-controls-ad1a3809-pool-20261011-103813-74978.log`.
SHA256: `8bb1048390ea3328c8b6888a80805ef472c717c9f6f40c1e3ba14432ad13641b`.
Sol read all ten step results, both suite summaries, all forty stage-1 results, and all thirteen stage-2 results.
Sol also read all 33 reported catchers. All 33 raw caught identities equal the reported identities.
All ten steps pass in 463.3 seconds. Default: 1738 passes and 478 skips. Slow: 556 passes and 2029 skips.
The OU-9 attach test, ST-5 testkit test, and real oracle test pass in 0.250, 0.216, and 0.064 seconds.
Stage 1 has twenty caught, thirteen missed, and seven unviable mutations.
The unmutated stage-2 baseline passes: 556 passes and 2717 skips. Stage 2 catches all thirteen misses.
Final result: 33 caught, seven unviable, zero missed, zero timeout. The complete log has no FLAKY token.
The job exits zero after 472 seconds with zero queue wait. The wrapper exits zero after 473 seconds.

The lead merged this head as `84b1e9be2c58044b27958ed39fe1fffa2a0eb907`.
Sol verifies both merge parents. Its tree equals the reviewed head tree, `27307134a2c87a065d10d45729ec45988f0ce3d5`.
This round closes Sol's final gate and body requirements. Both source findings remain closed.
The historical scanner-budget failure and its exception remain in Round 1.
This verdict accepts no later head, pin, or pending removal. Sol ran no execution job.

## Round 1 — 2026-10-11

Head: `147853f8a8ea9c39ab58433d29304c3e75eb8ce5`.
Reviewed base: `73da7b8662b8112f993ae3db19f0a2fa141ff4c2`.
Risk: HIGH. Open findings: 0.
Package verdict: `44bbf97dd1cac7a9884aab2b6750125f507aa0fd`, `verdicts/rc-real-controls.md`, round 2.
Sol read both complete #275 package rounds, all thirteen own changed files, the full live body, and the relevant pinned controls.
The review includes the correction and the required base-import delta review.

### Package findings close

RC-OC-1 MEDIUM closes. Both tiers derive expected CPR replies from an independent libghostty terminal.
The helper uses the session's size, prefix, and request. It does not call the control or its reply helper.
The tests retain different-width and no-prefix checks. The real payload test compares the complete visible screen of its source terminal.

RC-OC-2 MEDIUM closes. The replacement test proves session isolation through the harness controls.
It checks a capture of each session, a resize of the second session, and the first session's unchanged oracle screen.
The correction removes the direct Changed-record assertions and the added Shared::check assertions.
The existing resume boundary checks remain. No new accessor exposes a private record for an assertion.

### Interfaces and conformance accounting

The shared module owns the argument decoding and the three oracle reads. Each tier supplies its session lookup.
The real harness advertises and dispatches these controls only when tapped. The testkit registers the same three control names.
libghostty supplies query replies, screen text, and snapshot decoding. The PR adds no terminal encoder or private production test data.

The testkit screen uses the reviewed ModelLog anchor and the worker step rule to select consumed output.
The real screen uses acknowledged program output and refuses unknown output history under R-62 condition 1.
Both history controls use the shared size/profile refusal under condition 3.
Both wrappers record Resize and SetColorProfile before admission, as the lead authorized.
Fresh query terminals and session-free payload reads do not use this history refusal.
The resume change retains its independent anchor, capture cut, and comparator.

Only `conf::ou_9_attach_is_sync_baseline_in_pump` and `conf::st_5_reads_after_exit` leave testkit pending.
Both pass in the exact-head default suite. Both enter real pending.
Sol read both complete transcripts at the active Contracts pin.
The OU-9 real form still needs hold_handoff, release_handoff, and route_stream_written.
ST-5's print program does not satisfy known_output_history. Its real refusal remains explicit.
The minimum `conf::ev_8_no_route_shadow_answers_at_once` remains pending. Future route-input work is separate.
The PR changes no Contracts pin, frozen fake, or production machine behavior.

### Base import and inherited exclusions

The retained canonical check fails on DESIGN.md overlap, changed own source, and additions to real pending.
Log: `gates/botster-core-rc-oracle-bmc-147853f8-20261011-100215.log`.
Sol performs the required delta review instead of carrying an unchanged-diff verdict.
Parents are `cd4501eaf326ccfcf5ca904cdefbd3926e5defba` and the reviewed base.
The static merge tree equals the head tree, `cbd5250b5e260fb44b66c9223983929ab6d63f6f`.
All 22 imported paths outside the thirteen own paths equal the base blobs.
All ten own Rust blobs equal their pre-import blobs.
DESIGN.md retains the oracle changes and imports the named-proof policy.
The two pending files retain both own moves and import #273's separate TM-6 move.
The host wake and queued-cancel changes, and the route-gate accept change, do not replace the oracle anchor or dispatch.

The inherited mutation configuration equals the base. The PR adds no exclusion.
Sol read its complete delta from ff913e68 and both newly excluded process-glue bodies.
The inherited patterns exclude only whole-body Ok(()) replacements of real_proofs::check_slow_run and test_budget::command.
Their cited result and budget decisions remain tested. The gate prints both budget summaries and one checked named proof.
No inherited exclusion covers the new oracle reads or the shared refusal.

### Exact-head evidence and limits

Gate: `gates/botster-core-stage1-rc-oracle-controls-147853f8-pool-20261011-100215-28646.log`.
SHA256: `9f925b24bfbfb92b9cf196a5fd757c454f8176312a126aa4a97a992cba8af52f`.
Sol read the exact head/base headers, relevant PASS records, suite summaries, all mutation results and catcher records, and final exits.
All ten steps pass in 450.8 seconds. Default: 1738 passes, 478 skips. Slow: 554 passes, 2029 skips.
The unmutated stage-2 baseline passes. Stage 1 has twenty caught, thirteen missed, and seven unviable mutants.
Stage 2 catches all thirteen misses. All 33 raw caught identities match all 33 reported catcher identities.
Final result: 33 caught, seven unviable, zero missed, zero timeout. The complete log contains no FLAKY token.
The real oracle test passes. Both removed identifiers pass. Job exit zero follows 462 seconds with zero queue wait.
The wrapper exits zero after 463 seconds.

Sol read the complete retained Mac reversal receipt at `3042a0313ef2bacba130bdb2b635f7525f93b343`.
All six named reversals fail the real oracle test. This is a compact historical receipt, before the later record-test correction and import.
The final Linux gate catches those six exact real-tier mutations. The receipt does not establish a separate final-head Mac gate.
The earlier 27-ID Mac table remains an implementer report at 36643dcc plus a local diff.
Sol does not claim an exact-head execution result for that table.

The ff913e68 budget gate fails at 2.003 seconds against two seconds. The cd4501ea gate fails lists before its base import.
Sol read both failure summaries. The explicit lead exception permits the one historical re-gate and preserves the missing plain-v1 failure record.
The PR changes no scanner or budget code. Its added files can affect scan cost; Sol claims no isolated zero-cost measurement.
The new head has its own green gate. Earlier failures remain on the record.

Both source reviews cover this exact head and base. Current v1 has since moved to b6fd4b98.
The next current-v1 import requires the applicable check, any required delta reviews, and a new exact-head gate.
This source CLEAN accepts no later head, gate, merge, pin, or pending removal.
Sol ran no test, build, gate, measurement, or mutant.

VERDICT: CLEAN (0 open) at 147853f8a8ea9c39ab58433d29304c3e75eb8ce5

## Q7 source carry — 2026-10-11

Head: `ad1a38098a474f3ca9770bdbc828be7be92e4783`.
Base: `b6fd4b98e5f95aba4a6a1dc13efb609036be36f2`.
Parents are the accepted 147853f8 head and this base. No own correction follows Round 1.
Sol read the complete canonical PASS check at `gates/botster-core-rc-oracle-bmc-ad1a3809-20261011-103813.log`.
Check SHA256: `9b006b28c7f2539d2da782b508e1733d2a47521fbd3558a4d5538c814d2b4e7d`.
Sol independently confirms both parents and all three imported paths.
Outside the pending files, the complete own diff is byte-identical. It has 52169 bytes.
Own-diff SHA256: `ae788e8b30d7a2cbe068bb2b8197db092b10e10237fa93d73cf2cac3508985e3`.
The two imported non-list blobs equal the base. No imported non-list path overlaps the own paths.
The testkit pending file equals the prior file minus both own removals and the base's separate lc_2 removal.
The real pending blob equals the accepted head. No other pending change occurs.
The static merge tree equals the head tree, `27307134a2c87a065d10d45729ec45988f0ce3d5`.
Sol read the complete package Q7 carry at `a03c864a`, with the same head and base.
Both findings remain closed. Source CLEAN carries under Q7's line-set rule. No delta round is required.
The final exact-head gate remains pending. The earlier gate does not establish this head's gate result.
Sol accepts no future gate, merge, pin, or pending removal. Sol ran no execution job.

VERDICT: CLEAN (0 open) at ad1a38098a474f3ca9770bdbc828be7be92e4783
