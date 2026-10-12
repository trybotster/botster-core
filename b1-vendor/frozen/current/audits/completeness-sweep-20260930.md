# Completeness sweep of the frozen package (2026-09-30)

Owner: plan auditor. Requested by: the orchestrator, for the user's two questions. Package: `contracts-v1-joint-manifest-final4.md` (`c6388d00…68fa`). Read-only.

- **(A) "Where does it live":** every file name, directory layout, default path, socket, environment variable, URL path and config key that a second party must agree on.
- **(B) "Every command fully specified":** inputs with types, outputs with every enum value defined, specific failures, and the mutation and commit point.

## The part records (each gap has file:line evidence and the smallest amendment text)

| Part | File | Gaps (high / medium / low) |
|---|---|---|
| A. Where it lives (all contracts) | `completeness-sweep-A-20260930.md` | 33 (6 / 16 / 11) |
| B1. HC, codec, UI and CLI commands | `completeness-sweep-B1-20260930.md` | 67 (12 / 41 / 14) |
| B2. HP, Lua and Core calls | `completeness-sweep-B2-20260930.md` | 61 (14 / 33 / 14) |
| **Total** | | **161 (32 / 90 / 39)** |

## Gaps by owner. Every fix goes through the amendment path: a new file, a Sol ACK and a manifest entry.

| Owner | Gap ids | Amendment |
|---|---|---|
| **HC (plan auditor)** | A: G-1, G-2 (verbs), G-7 to G-12, G-15 (MC-1c), G-16, G-17 (`route_path`), G-20, G-23 to G-27, G-30, G-33. B1: H-1 to H-30 (with the new failures table A.9b). | HC Amendment 2 |
| **Codec (plan auditor; Core adopts it)** | B1: R-1 to R-10 | codec revision 27, pinned by HC Amendment 2 and Core Amendment 2 |
| **HP and Lua (0016)** | A: G-2 (clauses), G-3 to G-6, G-13, G-14, G-15 (PK-6), G-18, G-19, G-22, G-28, G-29. B2: B2-1 to B2-40. | HP Amendment 2 and a Lua revision |
| **Core (0019)** | A: G-11 (entry). B2: B2-41 to B2-61. Re-pin codec revision 27. | Core Amendment 2 |
| **UI (0017)** | A: G-17 (URL form), G-32. B1: U-1 to U-11. | ui-contract amendment |
| **CLI (001b)** | A: G-21, G-31 (G-10, G-26 with HC). B1: C-1 to C-12. | CLI candidate |

## Findings across parts

- **HP Appendix A is not complete per call.** It lists every call's effect and commit, and LN-2 matches its 57 calls. But failure lists, enum values, and handler and event shapes have gaps (B2).
- **HC needs a per-action failures column.** About 12 of about 55 A.9 actions state their failures. B1 drafts a new table A.9b, so the frozen A.9 does not change.
- **The Hub configuration file is missing everywhere** (G-1). Every "Hub parameter" has no agreed file, location or key syntax.

## Status

The amendments are with their owners. This record is updated with each amendment's hash and Sol ACK. The orchestrator gets one report when all amendments are ACKed, or earlier if a gap needs the user.

| Owner | Amendment file | sha256 | Sol ACK |
|---|---|---|---|
| UI (0017) | `ui-contract-v1.3-completeness-candidate9.md` (MINOR 1.3, feature `ui_1_3`) | `20d36f50a11d6ecb019940f5da879a110a3929b620ee51ea1db87b96ebc1c6c7` | **Sol Hub A CANDIDATE ACK**; Web and TUI confirmed |
| CLI (001b) | `cli-spec-v1-candidate28.md` | `aef3741d…cb86` | Sol Hub A to review against HC Amendment 2 |
| HP (0016) | `hub-plugin-contract-v1.1.amendment-2-candidate7.md` (A2-1..A2-53) | `c961d54faac7ed63c2f5f556b145d70f79d13749628225b4d16b7559a4b547fa` | **Sol Hub B CANDIDATE ACCEPT**; Lua r8 pending |
| Core (0019) | `core-contract-v1.17-amendment-2-candidate4.md` (= candidate3 with the pin moved to rev30) | `5a596f4407795616395dbb41b38a7e64381ac54dfe8d0a2a9166c36ae1a5b180` | **Sol Core CANDIDATE ACK** |
| HC (plan auditor) | `hub-client-contract-v1-amendment-2-candidate6.md` | `483bce824b138379cc8ac019d4777581c78fa50709fddcb3bb350f2dab463e85` | **Sol Hub A CANDIDATE ACK** |
| Codec (plan auditor) | `route-codec-v1-extract-rev30.md` | `17f2d2667aba7e3d79b857d85eaf4751612f60594dcedb6a7d36209668ef62f6` | **Sol Core and Sol Hub A CANDIDATE ACKs** |
| CLI (001b) | `cli-spec-v1-candidate34.md` | `c53f8d1d93954a6603f7aa0c12bb6d67940c0ae9311f98b2a59a09477172128c` | **Sol Hub A design ACK** |
| Lua (kit writer and 0016) | `hub-plugin-lua-binding-v1.1-r9.md` (read-only; r8 rejected) | `2d48f64702b0319b51ac9244256a2b2ab6089f188dc2b18b9371a3a1fc71dc84` | Sol Hub B review pending |
