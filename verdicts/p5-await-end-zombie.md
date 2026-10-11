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
