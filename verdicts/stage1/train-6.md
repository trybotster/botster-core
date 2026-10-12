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
