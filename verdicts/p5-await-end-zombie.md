# P5 await_end zombie review — #263

## Round 1 — 2026-10-11

- PR: https://github.com/trybotster/botster-core/pull/263.
- Exact reviewed head: `a540687728a7be9718e9bb11de62aa042ed905b6`.
- Parents: `16e51509064f704c341f5b0df8eebe2e898eef7a` and `c9ae2c2ffd9682d8bac29583205b39d13cc9f2a5`.
- Tree: `e37efd002da07c818c16e43ae71a54e2e055ffde`.
- Reviewed base: `c9ae2c2ffd9682d8bac29583205b39d13cc9f2a5`.
- READY: `msg_plugin-w_1791724872_9bba67`.
- Risk: HIGH. The change affects the shared test-process crate and process observation whose behavior the OS decides.
- Authority: BUILD.md, approved plan 23ar, the lead's P5 assignment, and the lead's 08:55 ruling for the failed initial Linux gate.
- Plan pin: `stage1-plan.a2897c12.md`; SHA256 `2e4d6e962f37ddafe2a68fd50598a1bd218fe3a6a53358f8cccd970884e0daed`.

### Source and proof review

The reviewer read the initial three-file change at `e2c0f1dc69b5c4e81a01ead579ae85dd731c66da` and the correction at `ba7dded6cef46ebc5c04bca0de52876c8b1032ba`. The reviewer read the complete final own diff and published body. No source or body finding remains open.

On macOS, the exit watcher can reject an exiting process or zombie with ESRCH. The old `await_end` returned `Gone` for both. The new `ended_before_the_watch` uses `rustix::process::test_kill_process`, a signal-zero probe, after that rejection. Success or EPERM returns `Exited`. ESRCH returns `Gone`. Another error returns an error. The probe sends no signal and does not reap.

The reviewer read the existing watcher, `exiting_or_gone`, status wait, Linux adapter, child observation, and relevant callers. The reviewer checked the local rustix and kqueue sources. The watcher registers at `watch`. The probe works for children and non-children. It adds no child-only wait, polling loop, timeout change, dependency, or exclusion.

The documented PID reuse limit remains explicit. Another thread can reap the old process between the refused watch and probe. A reused PID can then produce `Exited` instead of `Gone`. Both answers state that the watched process ended. This does not verify the new process's identity or provide an exit status. The pre-existing watcher also documents its PID reuse limit.

The correction preserves every assertion and expected result in `a_running_child_is_a_live_child_and_an_ended_one_is_not`. It adds the existing bounded `await_status` event wait before the zombie assertions. That helper observes status without reaping. `peek` retains EXITED, NOWAIT, and NOHANG. The correction adds no retry. The documentation states that `Exited` can precede an available Mac status. The reviewer relies on the existing source documentation for the XNU ordering and does not independently inspect XNU source.

The new proof owns a child that exits with code 3. `await_status` and `peek` establish an unreaped zombie before `await_end`. The first answer must be `Exited`. `OwnedChild::status` then reaps the child. The next answer must be `Gone`. The same expectations run on both OSes. The proof retains the child owner through every exit path.

### Prior art

The reviewer read old Core at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`. Its fixture exit watcher uses the same exit-filter idea and treats ESRCH as one boolean ended result. Its unsafe calls and boolean answer cannot supply this safe zombie-versus-reaped result. The old launcher watcher supplies no additional implementation to reuse. This PR copies no old code.

The change reuses the current watcher, ESRCH helper, rustix probe, status wait, and child owner. It adds no parallel process observer.

### Earlier failed evidence

The initial Linux log is `~/botster-sessions/gates/botster-core-stage1-p5-await-end-zombie-e2c0f1dc-pool-20261011-045219-1693.log`.
The first seven steps pass. Slow reports 479 passes and one failure among 480 tests. The failure is EV-5, seed zero, step 21: `PendingLimit`, with `pending_ops` equal to four. Mutants and fuzz do not run. The wrapper exits one after 386 seconds. This record stays failed.

The lead's 08:55 ruling permits review before the repair enters v1. It requires the #243 import, which removes the EV-5 cause, and one full gate on the new head. This uses plan 23an case (b). It is not a same-head re-gate. The reviewer relies on the lead's recorded cause decision and the accepted #243 repair.

The initial Mac log is `~/botster-sessions/gates/botster-core-stage1-p5-await-end-zombie-e2c0f1dc-pool-20261011-052558-68175.log`.
Its SHA256 is `c8ad51aa2fd6fb781917e8da7a6eca0ba9580817f453f15175226b29b87750da`.
Prebuild, clippy, and the 84-test suite pass. Nine of ten paired repeats pass. One fails at the unreaped-zombie assertion after `Exited`. The new zombie proof passes in all ten repeats. The job and wrapper exit one after 74 seconds. This record stays failed. The correction adds the status wait described above.

The body reports separate local reversal and repeat results. The reviewer does not independently verify those local claims and requests no repeat job.

### Accepted Mac evidence

Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p5-await-end-zombie-ba7dded6-pool-20261011-053027-93574.log`.

SHA256: `bca5890d925d53398a0afdfb87021c721338f1378a80a4a2a12376975ee5676f`.

The header names exact head `ba7dded6cef46ebc5c04bca0de52876c8b1032ba` and base `0c5ac80e512f986edd2ec1455012552a41c7f855`. The pool job uses two CPUs. Worker prebuild and clippy pass. Clippy selects all targets and features with the locked dependencies.

- Crate suite: 84 passed, zero skipped, 10.898 seconds.
- Both affected proofs report PASS by exact name.
- All twenty paired repeats report two passes and 82 skipped. Both proof names occur in every selection.
- The mutation selection excludes only `the_prebuilt_anchor_of_guard_new_holds_the_group_of_a_wrapped_program`. Its prebuilt binary is absent from the mutation copy. Both wait proofs remain selected.
- The unmutated mutation baseline passes in nine seconds of build and eleven seconds of tests.
- The `gone(&error)` guard replacement with `false` in `await_end` is caught at line 47:23.
- The `Ok(Default::default())` replacements of `await_end` and `ended_before_the_watch` are unviable. `Waited` has no `Default` implementation.
- Result: one caught, two unviable, zero missed or timeout, in 35 seconds.
- No match-arm mutant is generated. The match has no `_` arm. The proof checks both zombie and reaped answers without an equivalence exclusion.
- The log prints no individual named mutation catcher. The selected proof results and complete mutation outcomes retain that evidence limit.
- `MAC_EXIT=0`. The job and wrapper exit zero after 61 seconds.

### Import review

Merge `16e51509064f704c341f5b0df8eebe2e898eef7a` has parents `ba7dded6cef46ebc5c04bca0de52876c8b1032ba` and `ddd3b849df5db34d20d944ff6b8de23405fdb5c8`. The final merge imports base `c9ae2c2ffd9682d8bac29583205b39d13cc9f2a5`.

The reviewer verifies all three own blobs against `ba7dded6`: platform.rs, platform/macos.rs, and slow_process.rs. Each is unchanged. The complete own patch against the new base is byte-identical to the previously inspected own patch. All 42 imported paths are disjoint from those three paths. Each imported blob equals the new base.

The base is an ancestor of the final head. The diff check passes. The published PR names the exact reviewed head. The imports add no own manifest, pin, transcript, pending-list, selector, timer, or exclusion change. The reviewer does not repeat the accepted base package reviews.

The prior Mac evidence covers the unchanged own code. It is not an exact-head Mac run on `a5406877`. The imported changes retain their separate accepted base evidence. The exact-head Linux gate below covers the final composition.

### Exact-head Linux evidence

Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p5-await-end-zombie-a5406877-pool-20261011-061730-64021.log`.

SHA256: `6ba1d8ac45570f7604a0636da911e657d8f8264f56ebb04ed37c541ef91ee6ea`.

The header names the exact reviewed head and base. All ten CI steps pass in 161.0 seconds. The job and wrapper exit zero after 180 seconds on `msa1`.

- Default tier: 1683 passed, 481 skipped, 11.391 seconds.
- Slow tier: 500 passed, 2012 skipped, 38.862 seconds.
- Slow coverage: 1032 slow-only tests checked.
- The affected running-child proof passes in 0.003 seconds. The zombie-versus-reaped proof passes in 0.002 seconds.
- `ev_5_repeated_attach_detach_without_polling_stays_bounded` passes in both tiers. Its real trial passes in 0.164 seconds.
- Minimum: testkit-passing 55/69; real-passing 40/68; real-accepted 41/69.
- The real-pending report retains 58 IDs, with 33 passing under the ignored selection. This PR removes none.
- The Linux mutation step passes because the own diff has no Linux-compiled mutant. The changed runtime function is Mac-only and has the supplied Mac mutation evidence.
- Fuzz passes. The own diff changes no crate with a decoder harness.

### Verdict scope

Zero findings remain open. This CLEAN covers the exact head, base, source, body, and supplied evidence. It does not cover a later import. The lead retains merge order.

Sol's inspections at `b747c2f672c6cc8e1d82c6710b946671abcfd536` and `6bbcd93a47232b6e7f40baf1c6069f8a6d60ec50` agree with the initial source and corrected Mac checks. Those inspections issue no formal CLEAN. This package artifact permits the separate formal integration review to finish.

The reviewer changed no product code and ran no tests, builds, gates, mutations, reversal, measurement, or executable merge-check jobs.

VERDICT: CLEAN

## Round 2 — 2026-10-11 — base import delta

- Exact reviewed head: `2c8f6ea45f3815fda9728cf4d2900971fd0d3bc7`.
- Parents: `312b48a16cb37163ebb1c897cf9d8a4f8b2374d6` and `2b8cf08230dc3145dfe3a436bc34feee946c9b07`.
- Tree: `63b3c6fbe0733cac4ba07feee1bb045b9c0f8088`.
- Reviewed base: `2b8cf08230dc3145dfe3a436bc34feee946c9b07`.
- Previous CLEAN head: `a540687728a7be9718e9bb11de62aa042ed905b6`.
- Previous package verdict: `216e9b18e45f740b875787402c496ea1bb438c3d`.
- READY: `msg_plugin-w_1791726036_21a713`.
- Risk: HIGH. The approved plan remains 23ar with the pin and hash recorded in round 1.

### Source and body

The reviewer reads both shared-path import deltas from #257. `platform.rs` imports the accepted thread-count exports, descriptor-count helper, and count proof. `platform/macos.rs` imports the accepted libproc thread-count helper before the process observer. These additions change no own wait decision.

All three own added and deleted line sequences remain identical to the prior CLEAN head. The `Waited` documentation, ESRCH branch, signal-zero probe, and PID reuse limit remain intact. `slow_process.rs` retains the exact prior CLEAN blob. The prior source, proof, and prior-art conclusions remain applicable within their recorded limits.

The second import changes only `ci/remote/job.sh`. The reviewer reads its complete delta. It measures free space at Docker's data root, checks each deciding result, and stops with exit 75 if the result is invalid. Its blob equals the new base. This inspection supplies no separate formal #266 verdict or independent acceptance of its execution claims.

All three own blobs equal the inspected `312b48a1` blobs. All fourteen combined imported-only blobs equal the new base. The base is an ancestor of the reviewed head. The diff check passes. The published PR names the exact head and correctly states both overlaps and the Mac evidence limits. No source or body finding remains open.

Sol's inspection records `1d5368b0685999f0589771cad183566cf6368fcd` and `62fefba97708c7300c8d909664dd068ec64d75e9` agree with these checks. They do not issue formal imported-head CLEAN verdicts.

### Canonical base checks

The first log is `~/botster-sessions/gates/botster-core-stage1-p5-await-end-zombie-312b48a1-pool-20261011-062601-85073.log`.
Its SHA256 is `7bbb2346033494b2fb25b0dc37190efd3f31fac6905def9a6b060d956f5649b1`.
It checks the prior CLEAN head against `312b48a1` and base `ee7c6fca8c0781db09203ac478a8e824cdd3e4ab`. The job and wrapper exit one after nine seconds.

The final log is `~/botster-sessions/gates/botster-core-stage1-p5-await-end-zombie-2c8f6ea4-pool-20261011-063131-22650.log`.
Its SHA256 is `8645a78d47ae430019b94fbd3a87ec4c8718f13a10b5f7da12c52288ff996e36`.
It names the exact prior CLEAN head, new head, and base. The job and wrapper exit one after 107 seconds.

The reviewer reads both complete logs. Conditions (1) and (4) pass. Conditions (2) and (3) fail on `platform.rs` and `platform/macos.rs`. Both pending files equal the respective base. These failures require both reviewer delta rounds. They supply no automatic CLEAN transfer.

### Supplied Mac evidence

Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p5-await-end-zombie-312b48a1-pool-20261011-062836-92523.log`.

SHA256: `0e1a52f1a079e94172a0561eb66add5c105a77c86b90e3821b73aa61eb5acc8f`.

The header names `312b48a16cb37163ebb1c897cf9d8a4f8b2374d6` and base `ee7c6fca8c0781db09203ac478a8e824cdd3e4ab`. The pool job uses two CPUs. Worker prebuild passes. Locked clippy passes for all targets and features of the test-process crate.

- Crate suite: 85 passed, zero skipped, 11.002 seconds.
- The imported counts proof and both affected wait proofs pass by exact name.
- All twenty paired selections pass both wait proofs, with 83 skipped each.
- The in-diff mutation selection retains both wait proofs. It excludes only the same prebuilt-anchor proof because its binary is absent from the mutation copy.
- The unmutated baseline passes in fifteen seconds of build and twelve seconds of tests.
- The ESRCH guard replacement with `false` is caught at macos.rs:60:23.
- The `Ok(Default::default())` replacements of `await_end` and its helper are unviable at lines 53:5 and 83:5.
- Result: one caught, two unviable, zero missed or timeout, in 53 seconds.
- The log prints no individual named mutation catcher. The proof results and mutation outcomes retain that evidence limit.
- `MAC_EXIT=0`. The job and wrapper exit zero after 164 seconds.

This record covers the unchanged executable source in the final head. It is not an exact-head Mac run on `2c8f6ea4`. The later import changes only the Linux gate script. The reviewer requests no repeat Mac job.

### Exact-head Linux evidence

Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p5-await-end-zombie-2c8f6ea4-pool-20261011-063327-26304.log`.

SHA256: `369fdd1a08b1ab3f027d7df5738e2ed982ca89a263912548fcb19b1a0d33b32d`.

The header names the exact reviewed head and base. All ten CI steps pass in 391.5 seconds. The job exits zero after 398 seconds. The wrapper exits zero after 399 seconds on the Linux pool node.

- Default tier: 1688 passed, 481 skipped, 17.297 seconds.
- Slow tier: 515 distinct PASS identities, 2016 skipped, 53.923 seconds.
- Slow coverage: 1046 slow-only tests checked.
- The counts proof passes in the default tier. The affected running-child proof passes in 0.004 seconds. The zombie-versus-reaped proof passes in 0.003 seconds.
- EV-5 repeated attach/detach passes in both tiers, at 0.323 and 0.448 seconds.
- Real conformance: 151 distinct PASS identities.
- Minimum: testkit-passing 56/69; real-passing 41/68; real-accepted 42/69. The increase comes from imported #257, not a new own pending removal.
- The real-pending report retains 58 IDs, with 34 passing under the ignored selection. This PR removes none.
- The own diff has no Linux-compiled mutant. The mutation step starts no Linux run. The Mac-only runtime change has the supplied mutation evidence above.
- Fuzz passes. The own diff changes no crate with a decoder harness.

The earlier green `312b48a1` Linux gate is not the final head's gate. The exact-head record above supplies final Linux acceptance. The failed initial Linux and Mac logs from round 1 remain failed.

### Verdict scope

Zero findings remain open. This CLEAN covers the exact head, base, import composition, published body, and supplied evidence. The prior PID reuse, exit-status, local-claim, and kernel-source limits remain explicit. A later import remains outside this verdict.

The reviewer changed no product code and ran no tests, builds, gates, mutations, reversal, measurement, or executable merge-check jobs.

VERDICT: CLEAN
