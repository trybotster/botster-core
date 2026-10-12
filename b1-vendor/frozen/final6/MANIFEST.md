# Botster v1 contracts: joint manifest (final6 = final5 + Amendment 3: notifications in discarded output)

Owner: plan auditor. Date: 2026-09-30. **This file is the only place where the contracts' hashes live.** Core, the Hub→plugin contract (HP) and the Hub→client contract (HC) cite each other by contract name and clause id only. Files that never point back (the codec extract, the ui-contract files, the Lua binding) are also pinned by hash inside the contracts that use them.

**The joint final freeze:** each Sol gives one final ACK on the whole table below. Then the auditor reports DONE to the orchestrator.

| Contract name | File (under `~/botster-sessions/shared/`) | sha256 | Candidate ACK |
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
| Route codec revision 36 (**the current codec; it replaces revision 30**): TS-4c guarantee, `0x0F output_dropped`, the frame bound and `truncated`, worst-case eligibility. Pinned by HC Amendment 3 and Core Amendment 3 | `route-codec-v1-extract-rev36.md` | `1c6b16948450a327fa969a445a4249d82b3370cc80c8983bbb59ecada3d307a0` | Sol Hub A and Sol Core |
| Core Amendment 3 (the worker's output_dropped duty, the feature and its worst-case eligibility; NotificationPolicy in SpawnRequest and SetNotificationPolicy, stored for a Created session; the queue charges the inflated size; no focus change on a stall) | `core-contract-v1.17-amendment-3-candidate10.md` | `10e0424eda74d588a21bba3051e9f31078aa0f66ae56c98001e8b02d24518a80` | Sol Core |

## Joint final ACKs (final6)

| Sol | Verdict |
|---|---|
| Sol Hub A | pending |
| Sol Hub B | pending |
| Sol Core | pending |
