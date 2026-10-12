# Botster v1 contracts: the current joint manifest (final40 = final39 + Core erratum 6: a query reply from the wrong route is refused `wrong_route`)

**Layout (orchestrator, 2026-09-30).** This directory is `frozen/current/`. It is add-only: an accepted amendment adds its file, and this `MANIFEST.md` is the only file that is ever edited. Each manifest version is the git tag `manifest-finalN` on the commit that published it (final7 to final10 were published as `frozen/finalN/` directories; their tags keep them checkable). `frozen/final6/` stays as the record of the joint freeze. CI fails if any other existing file under `frozen/` changes.

Owner: plan auditor. Date: 2026-09-30. **This file is the only place where the contracts' hashes live.** Core, the Hub→plugin contract (HP) and the Hub→client contract (HC) cite each other by contract name and clause id only. Files that never point back (the codec extract, the ui-contract files, the Lua binding) are also pinned by hash inside the contracts that use them.

**The joint final freeze:** each Sol gives one final ACK on the whole table below. Then the auditor reports DONE to the orchestrator.

| Contract name | File (in this directory) | sha256 | Candidate ACK |
|---|---|---|---|
| Core contract | `core-contract-v1.17.md` | `8ee7ca05300156312363a00bf3bafae99674d0f09fc349866f71bb2f3a6d3486` | Sol Core: delta passes, no finding; candidate ACK on the matched manifest |
| Route codec (owned by HC, adopted verbatim by Core) | `route-codec-v1-extract-rev26.md` | `bf59b2b605e21b7e59b8d191cebdac5244241414d5d8f5c1372a86ece52a7abf` | Sol Core; Sol Hub A |
| Hub→client contract | `hub-client-contract-v1-candidate-parity45.md` | `19a2ade453cd2597ac2c73586245694b9d78a4d72346842aee26d2d950173b34` | **Sol Hub A CANDIDATE ACK** (parity45) |
| Hub→plugin contract | `hub-plugin-contract-v1.1.joint-J4e.md` | `e16f3f6b60963fae0cf971e6f7e610a9b91be7ecf28c95f02e8530d2bde27f19` | **Sol Hub B CANDIDATE ACCEPT** (J4e) |
| Hub→plugin Lua binding | `hub-plugin-lua-binding-v1.1-r6.md` | `fab7d3b4ba77479f063d053c61475305c63c66ac85de0a9725d0798aa81c51e9` | **Sol Hub B CANDIDATE ACCEPT** (r6) |
| ui-contract 1.0 | `ui-contract-v1-frozen-20260930.md` | `2063e8977c36c0e3cacdd0526de131f537b8bc63f4d39ec6db76e3300e5a7661` | Sol Hub A |
| ui-contract 1.1 | `ui-contract-v1.1-amendment-frozen-20260930.md` | `a932deeba07887005519f016d9e9845951d665464cf9972b403cb4b8dda122ba` | Sol Hub A (frozen from the accepted candidate4) |
| ui-contract 1.2 (embed) | `ui-contract-v1.2-embed-successor-candidate2.md` | `434bf90899bc021a7b41798bd958ac060b24b3a7a55cbe54877d72ddae7f60ac` | **Sol Hub A accepted** (closes gate L12) |
| CLI spec (a client of HC; informative for the freeze) | `cli-spec-v1-candidate26.md` | `4e8638b3e8fd99575807c5f6f9c504c3a9230aee6a641beda83be2cd4423474a` | **Sol Hub A CANDIDATE ACK** (against HC parity43) |

**Open for the user:** none in these contracts. The CLI spec may still carry its own user questions.

## Gate records for this package

**Gates 2, 3, 4 and 5: PASS** (see the closure record). Running the conformance suites is a BUILD acceptance gate, not a freeze gate (orchestrator).

| Record | sha256 |
|---|---|
| `audits/final-gates-closure-20260930.md` | `594d5793264aea9b58b7ed5123836a9a841661da0df2c0ad72621894c200e41a` |
| `audits/final-gates-recheck-20260930.md` | `34a3c41967ba2ec0d8e114511738de87b2736271114bb20de3e455ff5c0d9819` |
| `audits/final-gates-2-5-20260930.md` | `8c2e7453ee64c967ca68a75ebb8580ff945df2cca57a0e41c5b804f476ff7149` |
| `audits/final-gate-3-20260930.md` | `320f239a255f96f37540b7ccd13318348f9d6626b75816f29c12a4a51c9f8b4b` |
| `audits/final-gate-4-20260930.md` | `eb77954dd3cd3e6a72f84986ecf32849b61e8dd745f788571b33ab40dbd7f44e` |
| `audits/gate4-suite-traceability-20260930.md` | `53d282c54375bf9c68f0ebfc11d1101bafdd0bd79221e6e3f6ce48b415edb4c4` |
| `audits/plugin-parity-20260930.md` | `eb58e947dfedf1ca663815b7d2cabb2fe34cb4a4010e35fda439ee33eaba5cbe` |
| `audits/client-parity-20260930.md` | `febd887a4bb3f96715db3a3955e63c33f04f7b5899d5fd2d3a71e53ee6ed081b` |

## Amendments (post-freeze amendment path)

Each amendment adds clauses and lists exact edits to its frozen base above. The base files are unchanged. The contract is the base plus its amendment.

| Amendment | File | sha256 | Candidate ACK |
|---|---|---|---|
| HP Amendment 1 (PK-1 to PK-9: the package manifest file `botster-plugin.json`, the package root and `path`, multi-plugin repositories) | `hub-plugin-contract-v1.1.amendment-1-candidate5.md` | `5fe279bbc6694a431d47f5f222e2be8a05cec5f39def36d5596e36e8cf86a1e9` | Sol Hub B |
| HC Amendment 1 (the package source `path`; PKC-1 two-stage path checks) | `hub-client-contract-v1-amendment-1-candidate5.md` | `7b541e07a0053b9826cb2f2e5f90daf42090e1d38750d4f8ee70ac9ed84ac287` | Sol Hub A |

final3 (`a9dc85b9b05070d8025a7c5157bf3a907c9767a090fbccc87790cfe05d7787da`) keeps its three joint final ACKs. The base files are identical.

## Amendment 2 and the completeness sweep (post-freeze amendment path)

The orchestrator requested the sweep for the user's questions "where does it live" and "every command fully specified". It found 161 gaps. Every gap is closed by the amendments below, each with its Sol's candidate ACK. The base files and Amendment 1 are unchanged.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| HC Amendment 2 (completeness: hub.toml, roots and layout, access exceptions, install layout, update states, operator verbs, tagged values, A.9b failures table) | `hub-client-contract-v1-amendment-2-candidate6.md` | `483bce824b138379cc8ac019d4777581c78fa50709fddcb3bb350f2dab463e85` | Sol Hub A |
| Route codec revision 30 (replaces revision 26: R-1 to R-10, file-name and extension rules, the A.1 exception for baseline_end); pinned by HC Amendment 2 and Core Amendment 2 | `route-codec-v1-extract-rev30.md` | `17f2d2667aba7e3d79b857d85eaf4751612f60594dcedb6a7d36209668ef62f6` | Sol Hub A and Sol Core |
| Core Amendment 2 (per-operation table, WriteInput outcomes, RouteCloseReason, terminal event payloads, service signatures, terminal identity; pins codec revision 30) | `core-contract-v1.17-amendment-2-candidate4.md` | `5a596f4407795616395dbb41b38a7e64381ac54dfe8d0a2a9166c36ae1a5b180` | Sol Core |
| HP Amendment 2 (A2-1 to A2-53: failure lists, handler kinds, event payloads, the parameter-to-key table, the location rules, the timer and watch handler mapping) | `hub-plugin-contract-v1.1.amendment-2-candidate7.md` | `c961d54faac7ed63c2f5f556b145d70f79d13749628225b4d16b7559a4b547fa` | Sol Hub B |
| Hub→plugin Lua binding r9 (replaces r6 for the amended contract) | `hub-plugin-lua-binding-v1.1-r9.md` | `2d48f64702b0319b51ac9244256a2b2ab6089f188dc2b18b9371a3a1fc71dc84` | Sol Hub B |
| ui-contract 1.3 (MINOR, feature `ui_1_3`) | `ui-contract-v1.3-completeness-candidate9.md` | `20d36f50a11d6ecb019940f5da879a110a3929b620ee51ea1db87b96ebc1c6c7` | Sol Hub A |
| CLI spec (informative) candidate 34 (replaces candidate 26) | `cli-spec-v1-candidate34.md` | `c53f8d1d93954a6603f7aa0c12bb6d67940c0ae9311f98b2a59a09477172128c` | Sol Hub A |
| Completeness sweep record (index) | `audits/completeness-sweep-20260930.md` | `e55ab52d3f69d8c69d8c743a0e660e3f6309f4383080b8b19f0ddfef44d90a76` | — |
| Sweep part A | `audits/completeness-sweep-A-20260930.md` | `1f1fd4f0a627d0b78212cd8d7bdd5e66cf6cf8364d3d899f23e33c0f00c51a14` | — |
| Sweep part B1 | `audits/completeness-sweep-B1-20260930.md` | `06f1ccb8d4e42931f046491943c38aa188078c12dcba4c11764235dd0d01b575` | — |
| Sweep part B2 | `audits/completeness-sweep-B2-20260930.md` | `cb7120ffb47c1b3fb9e8eda4e23ce866390a0af3c51e7dcdc577c555558371f6` | — |

## Amendment 3 (post-freeze amendment path; the orchestrator's stall edge case)

Design: the orchestrator's (ownership is decided at the discard, on the route), with the user's scope correction (the contract states only the guarantee and the frame; detection is implementation). The base files, Amendment 1 and Amendment 2 are unchanged.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| HC Amendment 3 (SF-5f: a notification lost to a resync is reported exactly once, before the resync, in `output_dropped`, behind its route feature and the worst-case eligibility frame; the worker applies the notification policy; one surface per client across routes; SF-5d unchanged) | `hub-client-contract-v1-amendment-3-candidate9.md` | `1a303d561e0072b7f158fdb0b954c29a537e382a9b9954156b1a6a4e16b49e6b` | Sol Hub A |
| Route codec revision 36 (it replaces revision 30; **revision 37 replaces it**, final37): TS-4c guarantee, `0x0F output_dropped`, the frame bound and `truncated`, worst-case eligibility. Pinned by HC Amendment 3 and Core Amendment 3 | `route-codec-v1-extract-rev36.md` | `1c6b16948450a327fa969a445a4249d82b3370cc80c8983bbb59ecada3d307a0` | Sol Hub A and Sol Core |
| Core Amendment 3 (the worker's output_dropped duty, the feature and its worst-case eligibility; NotificationPolicy in SpawnRequest and SetNotificationPolicy, stored for a Created session; the queue charges the inflated size; no focus change on a stall) | `core-contract-v1.17-amendment-3-candidate10.md` | `10e0424eda74d588a21bba3051e9f31078aa0f66ae56c98001e8b02d24518a80` | Sol Core |

## ui-contract 1.4 (post-freeze amendment path; the Foundation build's realize items)

Requested by the Foundation lead (steward rulings R-2.3 and R-3 in `docs/steward-rulings.md`). Reviewed by the dedicated Amendment reviewer (Sol), which the orchestrator assigned to review every contract amendment. Every file above is unchanged and byte-identical to final6.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| ui-contract 1.4 (MINOR, feature `ui_1_4`: UI-NV-1 to UI-NV-7, UI-CF-6, UI-AD-5; added UiNode props, capability values, role segments, RealizedTree and `local_state` shapes, admission codes, copy keys, defaults) | `ui-contract-v1.4-realize-completeness-candidate3.md` | `363199658917b45af41215dad3f70f2197f60ad3536b0dc6c684b91f7111d42f` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `eaef7d9` (after REJECTs of candidates 1 and 2) |

## Hub→client Amendment 4 (post-freeze amendment path; the `action` field of `req`)

Found by the Foundation build (steward ruling R-8). Reviewed by the Amendment reviewer (Sol). Every file above is unchanged and byte-identical to final7.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| HC Amendment 4 (EN-1a: `req.action?`, required for `invoke` and absent otherwise, the `describe` name; EN-8a: `action` joins the idempotency match) | `hub-client-contract-v1-amendment-4-candidate1.md` | `a1d42a0ee196ec7b4bb9c658544ea3a558506d64e8cff10821f84f8930500955` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `2280df4` |

## Core Amendment 4 (post-freeze amendment path; the spawn report is readable)

Found by the Foundation build (steward ruling R-9). Reviewed by the Amendment reviewer (Sol). Every file above is unchanged and byte-identical to final8.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| Core Amendment 4 (A4-1: the sync read `service_report(id)` returns the `SpawnReport` from the `Ok` spawn until `RemoveService`, and after adoption; SV-4's "service's state" means this read) | `core-contract-v1.17-amendment-4-candidate1.md` | `e78576d3455b1fa88f6a63581038441a5135f408280cc97cbe2cf47590089ddb` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `6398ba4` |

## ui-contract 1.4 erratum 1 (post-freeze amendment path; no version or feature change)

Found by the Foundation build (steward ruling R-10). Reviewed by the Amendment reviewer (Sol). Every file above is unchanged and byte-identical to final9.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| ui-contract 1.4 erratum 1 (UI-NV-8: the "`error`-tone" of UI-AC-7(a), UI-NV-3 and UI-NV-5 is `tone: "danger"`; there is no tone value `error`) | `ui-contract-v1.4-erratum-1-candidate1.md` | `8065db8124d1dd6bc0b41eb22926d4f6d5d4e4583ee4cdb77b8b7314fd6b0b0f` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `9cd09df` |

## Core erratum 1 (post-freeze amendment path; final11)

Found by the Foundation lead. Reviewed by the Amendment reviewer (Sol). Every file above is unchanged.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| Core erratum 1 (E1-1: section 13's worker sentence; Core fixes no file name for the session worker binary, found by its host-supplied path; Botster installs it as `botster-worker`, HC RT-3) | `core-contract-v1.17-erratum-1-candidate1.md` | `0a7578bb6eabe7c1b2eca6f673965d13eddb7cc59e1332e3187b1f9b62a37709` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` merge `19d4e66` |

## Core Amendment 5 and Hub→client Amendment 5 (post-freeze amendment path; final12)

The orchestrator adopted the hybrid testing design under the user's instruction ("if it's better long term, let's do that"). Reviewed by the Amendment reviewer (Sol). Every file above is unchanged.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| Core Amendment 5 (A5-1 `botster-core-testkit`: real Core, in-process worker on one code path, scripted edges, real libghostty, off the facade and FFI; A5-2 seeded variation at its source; A5-3 sync refusals vs async edge failures; A5-4 the suite on real Core and the testkit; A5-5 FakeCore text replaced) | `core-contract-v1.17-amendment-5-candidate4.md` | `72c05f4b5ef8e3808536d23f0a7f56640e2d4b6fa1b66c0c7433ba157b1e89e8` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `773cc44` (after REJECTs of candidates 1 to 3) |
| HC Amendment 5 (A5-1 the real Hub in-process, not a binding; A5-2 one Rust/wasm transcript player; A5-3 shared vs client-only transcripts, recordings never an oracle; A5-4 producer vs delivery variation; A5-5 FakeHub text replaced) | `hub-client-contract-v1-amendment-5-candidate2.md` | `eb097128f35117e4af7c2dafff9da302af27165a44024dfe309c6c31c12ccb22` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `773cc44` (after a REJECT of candidate 1) |

## Core Amendment 6 (post-freeze amendment path; final13)

Found by the Foundation lead (A6-1) and the Stage 1 Core lead (A6-2, A6-3). Reviewed by the Amendment reviewer (Sol). Every file above is unchanged.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| Core Amendment 6 (A6-1 testkit service-lane edge and withheld control link; A6-2 first worker protocol 1, the enumerated deferred set; A6-3 uploads at Remove: `RemoveReport{uploads}`, teardown always continues; A6-4 text replaced, including A2-1's Remove result) | `core-contract-v1.17-amendment-6-candidate3.md` | `572101467891dfc3c60ce7496ba458ff614e8fd9e0a4b3abac9caae30d4bbf5d` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `5d2dd45` (after REJECTs of candidates 1 and 2) |

## Core erratum 2 (post-freeze amendment path; final14)

Found by the Stage 1 Core lead's libghostty audit. Reviewed by the Amendment reviewer (Sol). Every file above is unchanged.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| Core erratum 2 (E2-1 title is OSC 0/2, OSC 1 gives no event; E2-2 `other_modes` are tracked model modes; E2-3 `ModesChanged` posts the final flags only when they differ from the last posted value) | `core-contract-v1.17-erratum-2-candidate2.md` | `d0eb896737ecc54e11baf606afd784b2ee3f6555ff34c2840228dd10537a09bb` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `19668c9` (after a REJECT of candidate 1) |

## Core Amendment 7 (post-freeze amendment path; final15)

Found by the core-p4a-dp pair, through the Stage 0 lead. Reviewed by the Amendment reviewer (Sol). Every file above is unchanged.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| Core Amendment 7 (A7-1: an `attach` or `AttachWebRtc` option out of its OU-1 or EV-8(b) range is sync `InvalidInput{field}`, before any route is reserved; a value at its bound is accepted; a chunk cap above the SCTP maximum is not refused, and the worker never sends a larger chunk; A7-2 text replaced) | `core-contract-v1.17-amendment-7-candidate4.md` | `1d1a40c80cff6253faa0878602d6079c4669ed75f1fd12280cc77b3b9a4b8bcf` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `7d613ae` (after REJECTs of candidates 2 and 3) |

## Core Amendment 8 (post-freeze amendment path; final16)

Found by the core-p1 pair, through the Stage 0 lead (A8-1), and by the Stage 1 Core lead's libghostty audit (A8-2). Reviewed by the Amendment reviewer (Sol). Every file above is unchanged.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| Core Amendment 8 (A8-1: a capture reserves `max_snapshot_bytes` until its `Completed` is polled, then counts `total_bytes` or nothing; sync `CaptureLimit`; A8-2: a snapshot that cannot be formed within its bounds, including pending parser state the model can no longer carry for any reason, is `SnapshotTooLarge`; A8-3 text replaced in 9B, ST-6, ST-6b, 9.3 and OU-9) | `core-contract-v1.17-amendment-8-candidate5.md` | `c6918a9bf19d739422abc775441a9aa42811bbe044f1a18cd39f20215e2a8006` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `98565fd` (rebased to `7809d9a`; after REJECTs of candidates 1 and 4) |

## Core Amendment 9 (post-freeze amendment path; final17)

Found by the core-p4a-ou pair, through the Stage 0 lead. Reviewed by the Amendment reviewer (Sol). Every file above is unchanged.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| Core Amendment 9 (A9-1: `max_frame_bytes` must carry the route's own `attached` frame, else sync `InvalidInput`; A9-2: A3-1 item 4's eligibility case cannot happen, and its two tests are withdrawn and replaced; A9-3: the testkit's scheduler edge makes DP-3's per-frame compression choice) | `core-contract-v1.17-amendment-9-candidate1.md` | `860604ba6a54d42d9dc5dd9e00fed9e4ba2bde1c2d6be44b420e4b0fdce8b042` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `fc32271` |

## Core Amendment 10 (post-freeze amendment path; final18)

Found by the core-p5 pair, through the Stage 0 lead. Reviewed by the Amendment reviewer (Sol). Every file above is unchanged.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| Core Amendment 10 (A10-1: the process edge can answer the adopt handshake as an impostor, and Core's real AD-6 check decides; A10-2: the storage edge can damage one stored row, giving `Lost(RegistryCorrupt)`; observers need no amendment) | `core-contract-v1.17-amendment-10-candidate1.md` | `9fc505025fdf7292dd30b919684ddd3bb78257ebbae3b26eb4a528e6d656b768` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `3e350c9` |

## Core erratum 3 (post-freeze amendment path; final19)

Found by the Stage 1 Core lead's P1 reviewer. Reviewed by the Amendment reviewer (Sol). Every file above is unchanged.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| Core erratum 3 (E3-1: an eventless due effect runs in its pump; a step atomic with its event runs if budget remains, else it is carried whole with `more`; carried steps first; a full mandatory queue still parks; due order among runnable steps only, parked steps bypassed; `Silent` by the first pump with budget) | `core-contract-v1.17-erratum-3-candidate3.md` | `77291006b7ccc89e101917315e2248d8c8a31a5210a3e0120f69ebc63b83ae3b` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `b1837f2` (after REJECTs of candidates 1 and 2) |

## Core Amendment 11 (post-freeze amendment path; final20)

Found by the core-p4b pair, through the Stage 0 lead. Reviewed by the Amendment reviewer (Sol). Every file above is unchanged.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| Core Amendment 11 (A11-1: after a failed AD-6 handshake, the impostor may send scripted worker-shaped frames, and Core accepts none; a `Remove` stays `NotDeleted{outcome_unknown}`; A11-2 rewords A10-1) | `core-contract-v1.17-amendment-11-candidate1.md` | `beac85d513a1bc6e464f384b15e2b4f2a8d69b42af79fdbc8ee58db38106274e` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `383e814` |

## Core Amendment 12 (post-freeze amendment path; final21)

Found by the core-p4b pair, through the Stage 0 lead. Reviewed by the Amendment reviewer (Sol). Every file above is unchanged.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| Core Amendment 12 (A12-1a: the process edge holds the worker's next report, then releases it or breaks the link with it unread; it never drops a report on a live link; A12-1b: a scheduler-edge script point inside the `Remove` cleanup, where the worker is ended, gives `outcome_unknown` with no paths) | `core-contract-v1.17-amendment-12-candidate2.md` | `bc0f20ae2577ef54ea91ce05828ac7123f10d0b7dc98eab970bce64ea2898a9d` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `c3c6bce` (after a REJECT of candidate 1) |

## Core erratum 4 (post-freeze amendment path; final22)

Found by the core-p4b reviewer, through the Stage 0 lead. Reviewed by the Amendment reviewer (Sol). Every file above is unchanged.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| Core erratum 4 (E4-1: a client reply reserves its `0x81 input` frame size as received; the shadow fallback reserves its reply length; reserved at admission, released when the write completes) | `core-contract-v1.17-erratum-4-candidate1.md` | `d67542855e4d38ad222e5882d1ec735956de3ce9fa71063b7aaaf5125bcfe5c4` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `041b963` (rebased `a1902f2`) |

## Hub→plugin Amendment 3 (post-freeze amendment path; final23)

Found by the hp-events pair, through the Stage 0 lead. Reviewed by the Amendment reviewer (Sol). Every file above is unchanged.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| HP Amendment 3 (A3-1: the scripted I/O, git and catalog, and MCP services of NT-10, PM-14 and MP-10 are the real Hub's injected I/O edges, and the plugin-side FakeHub scripts `HostResult`s only; A3-2: WP-3's non-claim test is withdrawn) | `hub-plugin-contract-v1.1.amendment-3-candidate1.md` | `77d0d2809ba8ff05e75b3bd795026674b0e8db1b7d5ea579579b82f3d019634f` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `d33b4b0` |

## Hub→plugin Amendment 4 (post-freeze amendment path; final24)

Found by the hp-surfaces pair, through the Stage 0 lead. Reviewed by the Amendment reviewer (Sol). Every file above is unchanged.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| HP Amendment 4 (A4-1: `write_pending_bytes` is the Hub-wide total of held safe-write text, typing and waiting writes each counted once; over it, `backpressured{write_pending_full}`; a replacement subtracts only the waiting write; HC surface `busy`) | `hub-plugin-contract-v1.1.amendment-4-candidate2.md` | `5a0a60df1424ba41ab73a38154707927498cbd36eefbc41654d75964b83e822d` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `847b5a9` (after a REJECT of candidate 1) |

## Hub→plugin Amendment 5 (post-freeze amendment path; final25)

Found by the hp-surfaces pair, through the Stage 0 lead. Reviewed by the Amendment reviewer (Sol). Every file above is unchanged.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| HP Amendment 5 (A5-1: one entity record is `{id, entity}` everywhere a plugin produces it; the declared schema validates `entity` only, with `id` reserved at its top level; clients see a flat row whose `id` is a core field, and the client `schema` is the declaration unchanged, with validation by equivalence) | `hub-plugin-contract-v1.1.amendment-5-candidate3.md` | `347f020fc27878e0b949f626328767cbd5c02462d35317526461af999adc6a50` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `b8121e9` (after REJECTs of candidates 1 and 2) |

## Lua binding r9 Amendment 1 (post-freeze amendment path; final26)

Found by the lb-cli-suite pair, through the Stage 0 lead. Reviewed by the Amendment reviewer (Sol). Every file above is unchanged.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| Lua binding r9 Amendment 1 (LBA1-1: `json.array` returns the array and `json.empty_array` is a read-only value; LBA1-2: conversion envelopes carry the bare result; LBA1-3: handler descriptor fields are flat, and `entity_provider`'s `kind` is written `entity_kind`) | `hub-plugin-lua-binding-v1.1-r9-amendment-1-candidate1.md` | `6426ede664def95914eb15ccde550a59fc6e7eb123b220fc7f8f649967990083` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `21c9b36` |

## Hub→plugin Amendment 6 (post-freeze amendment path; final27)

Found by the hp-manifest pair, through the Stage 0 lead. Reviewed by the Amendment reviewer (Sol). Every file above is unchanged.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| HP Amendment 6 (A6-1: the publish status is decided at admission; a gap pending there, including one this publish causes, gives `pending_gap` for a store-projected family or `resync_scheduled` for a computed family; `accepted` is the admission state, not a delivery promise) | `hub-plugin-contract-v1.1.amendment-6-candidate2.md` | `eca4d97b3fd270ed7370e54c7fef3f39e47a277c67205490f350938dedaf96bc` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `1fa13a7` (after a REJECT of candidate 1) |

## Lua binding r9 Amendment 2 (post-freeze amendment path; final28)

Found by the lb-cli-suite pair, through the Stage 0 lead. Reviewed by the Amendment reviewer (Sol). Every file above is unchanged.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| Lua binding r9 Amendment 2 (LBA2-1: flat `tools`, `prompts` and `resources` entries with `call` in place of the wire `handler`; `handler` set in Lua fails the load) | `hub-plugin-lua-binding-v1.1-r9-amendment-2-candidate1.md` | `e56dd05df277df9c0a2baef8f41badb502d08a1bfc3b681d170aa3096625c165` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `fba9f37` |

## Hub→plugin Amendment 7 (post-freeze amendment path; final29)

Found by the hp-plugins batch 2 reviewer, through the Stage 0 lead. Reviewed by the Amendment reviewer (Sol). Every file above is unchanged.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| HP Amendment 7 (A7-1: the history holds previous locks only, and the current lock is not counted; an update pushes, and a rollback pops while dropping the replaced lock; each entry keeps the limit it was admitted under across a live reduction, with drops counted over the complete post-push list) | `hub-plugin-contract-v1.1.amendment-7-candidate3.md` | `8f8b450752a1af666a1e0c712cc75c65cc82ba38f57c0898c770c567f1a4893a` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `f3960d9` (after REJECTs of candidates 1 and 2) |

## Core Amendment 13 (post-freeze amendment path; final30)

Found by the Stage 1 Core lead (P2, libghostty fork `85a8d8e`). Reviewed by the Amendment reviewer (Sol). Every file above is unchanged. It replaces Core erratum 5 candidate 1, which was never published.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| Core Amendment 13 (A13-1: `ClipboardWrite{id, selection, contents, total_bytes, reason?}` for every clipboard write the terminal model recognizes, with all representations atomic and `[]` as clear; A13-1b: the worker alone acknowledges OSC 5522; A13-2: EV-3 replaced in full; the shadow never answers a clipboard read) | `core-contract-v1.17-amendment-13-candidate5.md` | `ec5debacd1d0affd4de08eaef3f1747646b80da05504e304cf890ac2c35edb20` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `584bf8e` (after REJECTs of candidates 1, 2 and 3; candidate 4 withdrawn before review) |

## Hub→plugin Amendment 8 (post-freeze amendment path; final31)

Found by the hp-manifest pair, through the Stage 0 lead. Reviewed by the Amendment reviewer (Sol). Every file above is unchanged.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| HP Amendment 8 (A8-1: the Hub makes the PD-4 checks against the bound document, judging each unknown result property once at its own place; A8-2: a MINOR or PATCH document only adds, checked recursively at static admission (`interface_not_additive`); A8-3: `breaking` pairs in `preview_upgrade` and `preview_package_upgrades`, and `conflict{breaking_not_accepted, consumers}`) | `hub-plugin-contract-v1.1.amendment-8-candidate2.md` | `6c20861148d5a0c78047d063e332a8a601d94825e57755d0cc6049ec2e2ade01` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `717ee0d` (after the REJECT of candidate 1) |

## Hub→client Amendment 6 (post-freeze amendment path; final32)

Found by the lb-cli-suite pair (CLI part), through the Stage 0 lead. Reviewed by the Amendment reviewer (Sol). Every file above is unchanged.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| HC Amendment 6 (A6-1: `package.install` ends with `result { package, version }` on success and on a staged end; A6-2: the `worktree` row's `branch` is absent for a detached HEAD, and `head` is always the full commit id) | `hub-client-contract-v1-amendment-6-candidate1.md` | `7deaee5d711bf446eac07bebdab3d1511a53da71c2743b00376c39c92a7fc5ce` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `c68a61a` |

## Core Amendment 14 (post-freeze amendment path; final33)

Found by the Stage 1 Core lead (audit A6, issue #148), after steward ruling R-32. Reviewed by the Amendment reviewer (Sol). Every file above is unchanged.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| Core Amendment 14 (A14-1: decoded size and contents size, payload bytes only; A14-2: the decoded size is checked first, then the contents size, and a `TooLarge` `total_bytes` is always over the bound; A14-3: the model's decode limit equals `clipboard_bytes`, and the guarantee is retained payload) | `core-contract-v1.17-amendment-14-candidate3.md` | `39c50621e569924ffabd2b66e8a5b94b7500c52a3c3372e215ce86eaa86ed16f` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `5fd6ac9` (after REJECTs of candidates 1 and 2) |

## Core Amendment 16 (post-freeze amendment path; final34)

Found by the Stage 1 Core lead (P7, services). Reviewed by the Amendment reviewer (Sol). Every file above is unchanged.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| Core Amendment 16 (A16-1: `StopService` completes with `ServiceEnd` = `Exited(ServiceExit)` or `Lost{reason, payload_may_remain}`; under `GuardianLost` no signal and no `ServiceState` event; under `EpochExhausted` SV-9 runs and the service moves to `Exited`; a recorded exit is final) | `core-contract-v1.17-amendment-16-candidate3.md` | `3ad8c7a500e888fe6df8c08634c8bcb0718191e9f6d509d0331c525c0867c96f` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `2bc82c8` (after REJECTs of candidates 1 and 2) |

## Core Amendment 15 (post-freeze amendment path; final35)

Found by the Stage 1 Core lead (audit A3). Reviewed by the Amendment reviewer (Sol). The numbers rest on the user ruling of 2026-10-05 (steward ruling R-34). Every file above is unchanged.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| Core Amendment 15 (A15-1: `CoreLimits.max_key_text_bytes`, default 256, range 1 to 4,096, bounds a key's `text` before IN-9's worst-case check; host `InvalidInput{field: "text"}`, route `input_refused{too_large}`) | `core-contract-v1.17-amendment-15-candidate2.md` | `ef636555c12b610683d6afe6ab381833b362e2a395af5d430af3622227b80c10` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `87c440a` (after the REJECT of candidate 1) |

## HC Amendment 7 (post-freeze amendment path; final36)

Found by the crate-catchup pair (Stage 0) in its audit of the contract crates. Reviewed by the Amendment reviewer (Sol). Its crate change is on this final branch (BUILD.md: an amendment is FINAL only with its crate change). Every file above is unchanged.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| HC Amendment 7 (A7-1: `package.preview_upgrade` and `hub.preview_package_upgrades` carry HP A8-3's `breaking: [{consumer, interface, range, interface_version}]`, always present, with a set-wide list on the bulk preview; A7-2: the `package` row's `static_origin?`, operators only, per HP A2-42; a taken listener port has no client field in v1) | `hub-client-contract-v1-amendment-7-candidate2.md` | `7ca773704d8659a10522aec04caa436a98dedf1a7154d531d6318a6aef7d1fb7` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `1b2fab7` (candidate 1 superseded before any verdict) |

## Core Amendment 18 (post-freeze amendment path; final36)

Found by the Stage 1 Core lead (P5, `conf::ad_3_adopted_equals_started`). Reviewed by the Amendment reviewer (Sol). It changes no crate type. Every file above is unchanged.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| Core Amendment 18 (A18-1: `last_output_at` is the unix time of this host's latest output observation, `None` after adoption until output; AD-3's equality excludes it; A18-2: the silence edge is measured on the monotonic clock from the idle start, which is the adoption point for an adopted session with no output, and `since` is the idle start's unix time; E3-1 unchanged) | `core-contract-v1.17-amendment-18-candidate2.md` | `6fcd07624da4dbc24d6c07aea29526443390d28219cce9bbe9b1c5e2f8edafeb` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `e46d275` (after the REJECT of candidate 1, `15a2a96`) |

## UI 1.4 erratum 2 (post-freeze amendment path; final36)

Found by the crate-guard pair's reviewer. Reviewed by the Amendment reviewer (Sol). It changes no crate type: the crate's UiNode vocabulary already matches. Every file above is unchanged.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| UI 1.4 erratum 2 (UI-NV-9: the `Text` props are `empty_state` `title` and `description`, and `dialog` `title`; a dialog's `body` is a slot whose children keep their own kind's `Text` rules) | `ui-contract-v1.4-erratum-2-candidate3.md` | `624a0a5966ba92a3df4559e37167f8b42f155cfae25cc09e5b451cd384b7f04c` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `37991e4` (after the REJECT of candidate 2, `9157984`) |

## The WebRTC move (post-freeze amendment path; final37)

Authority: the user decision of 2026-10-09, relayed by the orchestrator: WebRTC moves out of Core and into the Hub. The orchestrator's decisions D1 and D2 of 2026-10-09, under the user's delegation, set the channel layout and the chunking. Reviewed by the Amendment reviewer (Sol). Their crate change is on this final branch (crate PR #14, `stage0/final37-crate` at `1a38981`: reviewer ACCEPT and gate green on that head; BUILD.md: an amendment is FINAL only with its crate change). It withdraws 23 ids, with their authorities and replacements in `conformance/withdrawn.txt`. Every file above is unchanged.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| Core Amendment 17 (A17-1: `RouteTransport` is `Stream` only; no `WebRtc` transport, `AttachWebRtc`, `Answer`, `InvalidOffer`, `StartFailed{WebRtcFailed}` or `connect_deadline`; `features()` lists only `route_transport:stream`; A17-2: a host-terminated binding is an ordinary `Stream`; A17-3: the data-plane claims are about Core; A17-4: the replaced text, including `max_chunk_bytes` leaving `route_limits` and `binding_auth_unavailable` leaving Core) | `core-contract-v1.17-amendment-17-candidate2.md` | `41236a068cab6c6262f1e01bea9ec808efe1701707295bc9cde621b98f9f68e8` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `a33ef87` (after the REJECT of candidate 1, `28b78cc`) |
| HC Amendment 8 (A8-1: one Hub peer connection per client carries the control channel and one channel per route; A8-2: a route connects by a channel labelled `route:<route_id>`; A8-3: every channel is chunked at `welcome.limits.max_chunk_bytes`, and the Hub submits ready chunks of route channels round-robin; A8-4: the bridge's progress, its `route_send_buffer_bytes` queue, and close and drain; A8-5: the byte path per binding; A8-6: `attach_offer` is removed; A8-7: the replaced text) | `hub-client-contract-v1-amendment-8-candidate3.md` | `2f2728a285d215b094ac35ef58dcadd1d3104535aa325cdc215df83be085aa74` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `9c6cc0b` (after the REJECTs of candidate 1, `2b245fa`, and candidate 2, `14cde98`) |
| HP Amendment 9 (A9-1: only the Hub uses the ICE configuration) | `hub-plugin-contract-v1.1.amendment-9-candidate1.md` | `0235a4c185eaa5beb03e61cb94c4f8d3b93d3fef863b27b26bdc5e40a69dccdd` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `9cd8dc9` |
| Route codec revision 37 (**the current codec; it replaces revision 36**): HC A8-7 applied to the codec's copies of TS-1, TR-2, TR-3, TB-W2, TB-W4, BP-1, BP-4, BP-5, BP-7, the Layering note, TS-8 and TS-11, and the codec revision note, as HC A8's section "Route codec revision 37" states | `route-codec-v1-extract-rev37.md` | `30e254d64999d3bb8285ac683e0b8a6428d36f4cd577a1dd0b8bbedb25cb2e9a` | Amendment reviewer (Sol): file check PASS, `verdicts/amendments.md` commit `e39155d` (finding 2: revision 36 plus the 14 accepted replacements, the title and the revision note give this file byte for byte) |

## Core Amendment 19 (post-freeze amendment path; final38)

Found by the Stage 1 Core lead (P3 on Core PR #206, `conf::a9_1_frame_cap_equal_to_the_attached_frame_attaches`). Reviewed by the Amendment reviewer (Sol). Its crate change is on this final branch (`stage0/final38-crate` at `cc335a8`: reviewer ACCEPT and gate green on that head; BUILD.md: an amendment is FINAL only with its crate change). It withdraws `conf::a9_1_frame_cap_equal_to_the_attached_frame_attaches` and adds 7 pending ids. Every file above is unchanged.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| Core Amendment 19 (A19-1: the `max_frame_bytes` floor is the worst case of every floor frame (the worker-to-client frames that the codec bounds by `max_frame_bytes` and that the worker never splits or shortens), computed at attach from the codec schema and the route's values, with no number; `query_id` is a u64 sent as a decimal string; a floor above `max_route_frame_bytes` refuses every attach with those options; A19-2: every frame that can carry a file's path is checked before the file exists; A19-3: the replaced text, including EV-8's `query_id`) | `core-contract-v1.17-amendment-19-candidate2.md` | `14e2bac0f54a523a5323e27f171bce64615b6102908307586c51b07a814c123d` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `983e5a3` (after the REJECT of candidate 1, `ffd8e07`) |

## Core Amendment 20 (post-freeze amendment path; final39)

Found by the Stage 1 Core lead (P3's reading of the real-Core tier). Reviewed by the Amendment reviewer (Sol). Its crate change is on this final branch (crate PR #18, `stage0/final39-crate` at `3a4c16a`: reviewer ACCEPT and gate green on that head; BUILD.md: an amendment is FINAL only with its crate change). It adds 1 id, `conf::a20_1_testkit_proven_ids_are_exactly_the_named_list`, proven by a static transcript. Every file above is unchanged.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| Core Amendment 20 (A20-1: an id whose proof observes the allocator of the process in which the worker's code runs is proven on the testkit only, on steward ruling R-37's conditions; one named member, `conf::dp_3_frame_limit_checked_before_allocation`; a new member needs an amendment; A20-2: A5-4 names its two exceptions, A5-3's named real-process tests and A20-1's testkit-proven ids) | `core-contract-v1.17-amendment-20-candidate1.md` | `571acfe63c1de73e71e516aff554b894b1f4ef66d6ab5217d5c636cae16c53d0` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `ad91867` |

## Core erratum 6 (post-freeze amendment path; final40)

Found by the Stage 1 Core lead (RC's worker route-input design, plan 23bn; steward ruling R-75 q3). Reviewed by the Amendment reviewer (Sol). Its crate change is on this final branch (crate PR #56, `stage0/final40-crate` at `f47ee2d`: reviewer ACCEPT and gate green on that head; BUILD.md: an amendment is FINAL only with its crate change). It adds 1 id, `conf::e6_1_reply_from_another_route_is_wrong_route_and_the_query_stays_open`, proven by a Core transcript that is `structural` on FakeCore (FakeCore has no `edges_quiet` control and is frozen). Every file above is unchanged. The number 6 is used because Core erratum 5 candidate 1 was replaced by Core Amendment 13 and never published.

| Item | File | sha256 | Candidate ACK |
|---|---|---|---|
| Core erratum 6 (E6-1: a `reply_terminal_query` from any route other than the one the worker bound the `query_id` to is `input_refused{wrong_route}`, as route codec revision 37 states; `invalid_input` is no longer a reply refusal reason in EV-8(c); no new number, type or wire value) | `core-contract-v1.17-erratum-6-candidate1.md` | `508b2912c194004392344e44c83fccff774a6c8dfa6685bea100cd6835ccf33a` | Amendment reviewer (Sol): ACCEPT, `verdicts/amendments.md` commit `2a568fa` |

## Acceptance record (final7 to final10)

- **Files from final6:** the joint final ACKs of Sol Hub A, Sol Hub B and Sol Core on manifest final6 (sha256 of `frozen/final6/MANIFEST.md`: d69c87a821af86901f08d3ea05771fc1c7136ba86a6a3a900c4fca2ac92aadfd), and the user's sign-off on 2026-09-30. The table rows of final6 are repeated above unchanged; its "Joint final ACKs" table below was a template whose ACKs were given by message.
- **ui-contract 1.4:** the Amendment reviewer's ACCEPT (commit `eaef7d9`). This is the amendment path of `README.md` rule 2 and `docs/BUILD.md` rule 6: one Sol review and a new manifest entry. No joint re-freeze is needed, because no other contract changes.
- **HC Amendment 4:** the Amendment reviewer's ACCEPT (commit `2280df4`), by the same path. Manifest final7 (sha256 of `frozen/final7/MANIFEST.md`: be51fae6ed93d0bf17d3ad464624c44b44996f96683c90db55928457daee4057) is the base.
- **Core Amendment 4:** the Amendment reviewer's ACCEPT (commit `6398ba4`), by the same path. Manifest final8 (sha256 of `frozen/final8/MANIFEST.md`: 1a45d1817d1045cb10e2422e78f189d7aa884cf0164521e24b3921056a608b09) is the base.
- **ui-contract 1.4 erratum 1:** the Amendment reviewer's ACCEPT (commit `9cd09df`), by the same path. Manifest final9 (sha256 of `frozen/final9/MANIFEST.md`: 6d2df0ed203ae2ed6ffa8c867f96d6933bd413a2212dc0a1af6d29e36b90acf3) is the base.
- **Core erratum 1 (final11):** the Amendment reviewer's ACCEPT, by the same path. Tag `manifest-final11`.
- **Core Amendment 5 and HC Amendment 5 (final12):** the Amendment reviewer's ACCEPT (commit `773cc44`), by the same path. Tag `manifest-final12`.
- **Core Amendment 6 (final13):** the Amendment reviewer's ACCEPT (commit `5d2dd45`), by the same path. Tag `manifest-final13`.
- **Core erratum 2 (final14):** the Amendment reviewer's ACCEPT (commit `19668c9`), by the same path. Tag `manifest-final14`.
- **Core Amendment 7 (final15):** the Amendment reviewer's ACCEPT (commit `7d613ae`), by the same path. Tag `manifest-final15`.
- **Core Amendment 8 (final16):** the Amendment reviewer's ACCEPT (`7809d9a`), by the same path. Tag `manifest-final16`.
- **Core Amendment 9 (final17):** the Amendment reviewer's ACCEPT (`fc32271`), by the same path. Tag `manifest-final17`.
- **Core Amendment 10 (final18):** the Amendment reviewer's ACCEPT (`3e350c9`), by the same path. Tag `manifest-final18`.
- **Core erratum 3 (final19):** the Amendment reviewer's ACCEPT (`b1837f2`), by the same path. Tag `manifest-final19`.
- **Core Amendment 11 (final20):** the Amendment reviewer's ACCEPT (`383e814`), by the same path. Tag `manifest-final20`.
- **Core Amendment 12 (final21):** the Amendment reviewer's ACCEPT (`c3c6bce`), by the same path. Tag `manifest-final21`.
- **Core erratum 4 (final22):** the Amendment reviewer's ACCEPT (`a1902f2`), by the same path. Tag `manifest-final22`.
- **HP Amendment 3 (final23):** the Amendment reviewer's ACCEPT (`d33b4b0`), by the same path. Tag `manifest-final23`.
- **HP Amendment 4 (final24):** the Amendment reviewer's ACCEPT (`847b5a9`), by the same path. Tag `manifest-final24`.
- **HP Amendment 5 (final25):** the Amendment reviewer's ACCEPT (`b8121e9`), by the same path. Tag `manifest-final25`.
- **Lua binding r9 Amendment 1 (final26):** the Amendment reviewer's ACCEPT (`21c9b36`), by the same path. Tag `manifest-final26`.
- **HP Amendment 6 (final27):** the Amendment reviewer's ACCEPT (`1fa13a7`), by the same path. Tag `manifest-final27`.
- **Lua binding r9 Amendment 2 (final28):** the Amendment reviewer's ACCEPT (`fba9f37`), by the same path. Tag `manifest-final28`.
- **HP Amendment 7 (final29):** the Amendment reviewer's ACCEPT (`f3960d9`), by the same path. Tag `manifest-final29`.
- **Core Amendment 13 (final30):** the Amendment reviewer's ACCEPT (`584bf8e`), by the same path. Tag `manifest-final30`.
- **HP Amendment 8 (final31):** the Amendment reviewer's ACCEPT (`717ee0d`), by the same path. Tag `manifest-final31`.
- **HC Amendment 6 (final32):** the Amendment reviewer's ACCEPT (`c68a61a`), by the same path. Tag `manifest-final32`.
- **Core Amendment 14 (final33):** the Amendment reviewer's ACCEPT (`5fd6ac9`), by the same path. Tag `manifest-final33`.
- **Core Amendment 16 (final34):** the Amendment reviewer's ACCEPT (`2bc82c8`), by the same path. Tag `manifest-final34`.
- **Core Amendment 15 (final35):** the Amendment reviewer's ACCEPT (`87c440a`), by the same path, with the user ruling of 2026-10-05 for its numbers. Tag `manifest-final35`.
- **HC Amendment 7 (final36):** the Amendment reviewer's ACCEPT (`1b2fab7`), by the same path, with its crate change on the final branch. Tag `manifest-final36`.
- **Core Amendment 18 (final36):** the Amendment reviewer's ACCEPT (`e46d275`), by the same path. Same tag.
- **UI 1.4 erratum 2 (final36):** the Amendment reviewer's ACCEPT (`37991e4`), by the same path. Same tag.
- **The WebRTC move (final37):** Core Amendment 17 (`a33ef87`), HC Amendment 8 (`9c6cc0b`), HP Amendment 9 (`9cd8dc9`) and route codec revision 37, by the same path, with their crate change on the final branch (`1a38981`). Tag `manifest-final37`.
- **Core Amendment 19 (final38):** the Amendment reviewer's ACCEPT (`983e5a3`), by the same path, with its crate change on the final branch (`cc335a8`). Tag `manifest-final38`.
- **Core Amendment 20 (final39):** the Amendment reviewer's ACCEPT (`ad91867`), by the same path, with its crate change on the final branch (`3a4c16a`). Tag `manifest-final39`.
- **Core erratum 6 (final40):** the Amendment reviewer's ACCEPT (`2a568fa`), by the same path, with its crate change on the final branch (`f47ee2d`). Tag `manifest-final40`.

## Joint final ACKs (final6)

| Sol | Verdict |
|---|---|
| Sol Hub A | pending |
| Sol Hub B | pending |
| Sol Core | pending |
