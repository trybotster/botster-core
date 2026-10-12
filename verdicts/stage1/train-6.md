# Train 6 integration review

## Round 1 — combined source CLEAN

Train head: `3e655eff46d36f8c337339a1a17d401c8c662594`.
Train tree: `4b7ace5012cf0543ed64dea143e5db5932c500c2`.
Train base: `2683de7ccbb1360d124422b3b6a2dd196b046c7e`.
Lead request: `msg_plugin-w_1791832618_747ce8`.
Lead receipt correction: `msg_plugin-w_1791832646_032a15`.

The frozen merge order is:

| Member | Frozen head | Merge commit |
| --- | --- | --- |
| #303 | `df461c83ffa8761c691e5ee07cf50f20784029f7` | `3837b2c72f7eb4c961afa1cdc5ee3078f2dafe9f` |
| #309 | `cfcc438d8eac85108d64237bdc11fa215dc85104` | `d5a59447fbd942c788a08c660cfaa07bf3dfcdf4` |
| #308 | `c3bd7fcd17861e6f164bbda5eb38638cab71f00e` | `b40b5235a00fc3be951282f181bdb2cffc12ae44` |
| #311 | `2c5d09a0f3e29e443fd8b33d8406bb84cb37940d` | `3e655eff46d36f8c337339a1a17d401c8c662594` |

Each merge has the previous train commit and its frozen member as parents.
Each member's merge base against the train base is `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
The reviewer verifies the pushed train head and remote v1 base once.
The lead reports no conflicts and a merge with rerere disabled.

### Review records

The reviewer reads each complete applicable member delta verdict on this exact train identity:

| Member | Package verdict | Integration verdict |
| --- | --- | --- |
| #303 | `3cd1a853b2d5190452a70098c5bb4d5d9e527202`, `verdicts/p5-r97-worker-clock.md`, Round 4 | This commit, `verdicts/stage1/p5-r97-worker-clock.md`, Round 4 |
| #309 | `4b800553694f1a3043e9a036f1320ca200a880de`, `verdicts/rc-real-controls.md`, train 6 | `4dc077b6c534d2b509128c59e403e43dbe312516`, `verdicts/stage1/rc-r100-frame-check.md`, train 6 |
| #308 | `8c4ef068fd7007a4634fe08a991667466721e275`, `verdicts/gs-gate-speed.md`, train 6 | `8fa6b68360b90613d0341cb26e49fb76d718f07d`, `verdicts/stage1/gs-flaky-signature.md`, Round 5 |
| #311 | `8a47bd75d5beb4b93119a69eb4f46e3d049fc0e2`, `verdicts/p3-worker.md`, Round 199 | `4dc077b6c534d2b509128c59e403e43dbe312516`, `verdicts/stage1/p3-resize-latest.md`, Round 6 |

All member source deltas are CLEAN. Their earlier finding closures and execution limits remain applicable.
The corrected #308 package record gives Sol the combined review and final receipt.
Sol-B and Sol-C supply member deltas only.

### Independent composition checks

Complete train full-index diff SHA-256: `9d37bc58e641962e6b917c2c8c7c90bbdef52d8ec8b8164fcfb69ab3238de24b`.
The diff has 36 paths, 4095 additions, and 169 deletions.
Each insertion has exactly its member's own path set: seven, twelve, three, and seventeen paths respectively.
Every inserted added-line and deleted-line set equals its member delta, excluding hunk positions and object indices.
No additional change appears in an insertion.
Final member blobs remain exact in two of seven, eleven of twelve, two of three, and twelve of seventeen paths respectively.
The reviewer reads the overlap differences and affected final contexts.

All nine conformance text sets satisfy the three-way formula after each ordered merge.
All 36 comparisons pass. No base removal returns and no member addition disappears.
Final counts are pending 387, real-pending 38, held four, ledger 691, minimum 69, real-only 26, real-proofs seven,
deferred three, and withdrawn 27.
Pending has sixteen removals from the train base and no added identifier.
The member verdicts cover those removals. #303 changes only its existing startup-clock comment.
The train retains contracts `.44`. It imports no `.46` pin, new status removal, or #316 exception change.

### Combined #303 and #311 interfaces

The testkit uses the production `Worker` through `SharedWorker` and the existing Sim binding.
`run_at_clock` advances the Sim to `WorkerClock::now` before it runs ready work.
`run_ready` adds no program step and runs no host pump.
The resize result enters the same machine on that worker clock. It adds no deadline.
`Worker::handle` still expires adoption before it handles each input.
The worker deadline query still covers orphan and candidate deadlines; it adds no stop-grace deadline.
The host owns stop grace and retains the `KillPayload` path.

The testkit holds one `PtyResized` result and offers it before later payload output and exit inputs.
The production driver queues the result during `ResizePty` and settles queued inputs before its next payload read.
Both edges apply the same row, column, and pixel clamps and return EIO when no PTY remains.
The machine applies the terminal size after successful PTY completion.
It retains failed-size restoration, serialized resize work, route policy order, and adoption request retirement.
The imported train-5 report-order test changes no resize state or clock behavior.

The shared fixture starts with no resize result. Its clock tests and orphan deadline tests retain their original inputs.
All #303 clock functions, wake behavior, and regression additions remain present.
The imported report-hold field and scheduler argument coexist with the clock constructor.
Capture observation stays in host event polling. It neither advances the worker clock nor changes the host deadline query.
No test-only behavior enters a production machine.

The #309 frame check uses the existing serialized `HostMsg::Op` for Resize and SetSizePolicy.
#311 adds worker machine inputs and actions, but no new host message shape.
The host checks frame size after payload validation and before operation allocation.
The launch check still uses DEFAULT_MAX_PAYLOAD before the worker learns its configured bound.
Its worst size and size-policy calculation still bounds the values that #311 uses.
The control queue imports and testkit worker clock do not change that calculation or the decoder bound.

#308 retains the accepted structured EV-2 exception and exact failure matching.
The entry supplies no automatic retry and no replacement for normal or baseline execution evidence.
The composed CI file retains the base step names, seed behavior, and normal/baseline requirements.

### dp3 real-pending question

The reviewer reads the exact `.44` contracts commit `99e5983befde9a96ca4cdb3faea0f7f9a4b930ea`.
The dp3 transcript SHA-256 is `efddda634b96f4f12c9252c72bf387dfa0060749f2dfc967a1ba55bf2d3366fa`.
It adds title output, `await_title`, and `program_ready` before route input.
The `.44` exception list has no dp3 entry.
Thus dp3's structural fence condition is met in the pinned source.

The current review supplies no `.44` real-tier PASS for dp3.
The historical `.43` passes occurred before these fences and cannot establish that result.
The pending comment records those historical passes and the `.44` fence change.
The reviewer retains dp3 in real-pending until valid current-pin pool evidence establishes its removal condition.
The two dp5 exception entries remain in `.44`; those identifiers also stay real-pending.
The dp6 tie transcript remains absent in `.44` and its identifier stays pending.
Sol-C independently confirms the fence state and the absence of new removal evidence.

### Disposition and next evidence

The lead reports list checks PASS, test-target compilation PASS, and strict seeds 0–31 at 281/0.
No pre-check log is retained. These statements are operator reports, not independent gate evidence.
The train gate has not run. The source review identifies zero open composition findings, including LOW.

The reviewer reads the updated train rule in `~/Projects/botster-contracts/docs/BUILD.md`.
Rule commit: `132d0ee5a75994729553096d97bf385896b21683`; inspected repository head: `4ddaee38cb1069637e3dcd7de5d3410791bc6e4f`.
Under the corrected Green rule, Sol sends one final receipt for the whole train after reviewing the completed exact-head gate.
That receipt must name the gate path and SHA-256. Unchanged members require no separate package final receipt.
The lead must still satisfy Q7, equal-tree merge verification, and the rule's operator requirements.
This source CLEAN grants no gate acceptance, retry, future head, or merge.
The reviewer changes no product code and runs no build, test, gate, mutation, reversal, reproduction, measurement,
or executable base check.

VERDICT: CLEAN (0 open; combined source only, final train receipt remains due)

## Round 2 — ONE final whole-train receipt

Train head: `3e655eff46d36f8c337339a1a17d401c8c662594`.
Train tree: `4b7ace5012cf0543ed64dea143e5db5932c500c2`.
Train base and BOTSTER_CI_BASE_REF: `2683de7ccbb1360d124422b3b6a2dd196b046c7e`.
Completed-gate request: `msg_plugin-w_1791834743_9279f8`.
Gate log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-train-6-3e655eff-pool-20261012-123257-88248.log`.
Independently verified SHA-256: `4237d5aadea652758d6040dce166067584356a8addcbf088c667364851aaba5d`.

The combined source review and all eight applicable member source verdicts remain CLEAN at Round 1.
Source record: `a3250c8cb1a1b2ceb34eae8d2a6c4f5c1bffb8be`.
The reviewer verifies all four GitHub member heads once before this receipt.
They equal the frozen heads in Round 1, and each PR still targets v1.
No member changed after its applicable source CLEAN. The updated Green rule requires no separate package final receipt.
The reviewer independently verifies remote train-6 equals this head and remote v1 equals this base.
The base is an ancestor of the train head, and the local train tree equals the reviewed tree.
The frozen order, complete composition hash, interface checks, and all 36 list formulas remain applicable without a source change.

### Completed gate evidence

The retained 29593-line log names this exact head, base, msa1, and both concurrency caps at four.
All ten CI steps PASS. CI total is 1057.4 seconds.
The job exits zero after 1147 seconds: one queued second and 1146 running seconds.
The wrapper exits zero after 1148 seconds.

Default reports 2078 passes, 411 skips, and 14.117 seconds.
Normal slow reports 677 passes, 2078 skips, and 44.324 seconds.
The unmutated Stage 2 baseline reports 687 passes, 2902 skips, and 44.586 seconds; its complete baseline step takes 68.0 seconds.
The reviewer independently parses every PASS identity and ordinal in these three sections.
Each has the complete expected ordinal set, unique identities, and its stated denominator.
The slowest default test takes 1.221 seconds, below the two-second individual budget.
No FLAKY, failure, timeout, panic, SIGSEGV, or SIGABRT result occurs in these unmutated runs.
The printed known-flaky signature is configuration text, not an observed INCONCLUSIVE result.

Both strict summaries report 281 passes, zero failures, and 411 ignored.
Strict conformance runs 300 of 317 pending identifiers with transcripts; seventeen real-only identifiers do not run and four remain held.
The branch sets default/strict seeds to 0–31 and normal real-process seeds to zero.
Held-full runs three budget identifiers at 32 seeds each: 96 ignored, zero failures, and 22.36 seconds.
Minimum accounting reports testkit-passing 68/69, real-passing 66/68, and real-accepted 67/69.

All sixteen removals from core-pending have normal default PASS records.
Twelve also have normal real conformance PASS records.
The remaining four retain real-pending status: dp3 route codec, dp4 write-time modes, dp5 no success acknowledgement, and dp5 optional op.
The train adds those four to real-pending and removes no identifier from that set.
The ignored real run retains 38 real-pending identifiers and reports 29 passes, with zero failures.
It does not count those passes as active conformance completion.

The current `.44` ignored real run now reports dp3 route codec PASS.
This supplies current-pin pool evidence after the fences reviewed in Round 1.
The normal real tier uses seed zero; this receipt makes no full-seed real-tier claim.
The lead assigns dp3's possible removal to a follow-up PR. The train keeps its current status unchanged.
The two dp5 fence exceptions remain. No removal, new pin, or additional control is accepted by this receipt.

The R-97 deadline tests, worker wake tests, no-grace assertion, and combined resize regressions have normal PASS records.
The real PTY resize test passes in `slow_session` and `slow_driver` in normal slow and the unmutated baseline.
This gate supplies no execution of #322's separate FIFO patch or #323's separate status-entry patch.

### Mutation and fuzz audit

Stage 1 reports 212 mutants: 186 caught, one missed, zero timeout, and 25 unviable, in 383.9 seconds.
The reviewer independently parses all 212 unique raw mutant identities.
The Stage 2 identity set equals the Stage 1 miss set exactly.
Stage 2 catches its one mutant, with zero miss, timeout, or unviable result, in 36.5 seconds.
The recovered mutant replaces `Payload::resize` with `Ok(())`.
Its catch is `botster-worker::bin/botster-worker slow_driver::sz_1_resize_sets_the_real_pty_size`.
That test passes in normal slow and the unmutated Stage 2 baseline.

All 186 Stage 1 raw catches match unique citations. Each cited test passes in the normal gate.
The one Stage 2 raw catch also matches its unique citation and both unmutated PASS records.
Final mutation accounting is 212 total, 187 caught, zero missed, zero timeout, and 25 unviable.
Unviable mutants supply no behavioral catch. No FLAKY result or retry-based catch is credited.

The Stage 2 selection omits the named EV-2 real trial through the reviewed known-flaky entry.
The entry prints its structured step, reason, seed, text, owner RC, and #316 removal condition.
EV-2 passes normal slow at 0.129 seconds and the unmutated baseline at 0.126 seconds.
EV-2 supplies no mutation catch citation. The exclusion does not suppress an unmutated failure or authorize a re-gate.
The later `.45`-or-newer pin must still remove the entry and restore the empty-list assertion under the reviewed R107-SI-2 disposition.

Both changed-package decoder fuzz harnesses finish successfully.
`link_decoder` completes 7184664 runs in 61 seconds. `msg_decoder` completes 3881449 runs in 61 seconds.
No crash or failure appears in their retained output.

### Final disposition

Zero composition or gate findings remain, including LOW.
This is the single final receipt for all four frozen members and this exact gated train head.
The lead may apply the Green rule to fast-forward v1 to this head.
The lead must verify the resulting base tree equals the gated tree and verify the remote push with `git ls-remote`.
Those merge verifications remain due; this receipt does not claim that the merge occurred.
No future source, gate, retry, status removal, or pin is accepted.
The reviewer changes no product code and runs no build, test, gate, mutation, reversal, reproduction, measurement,
or executable base check.

VERDICT: CLEAN (0 open; FINAL whole train)
