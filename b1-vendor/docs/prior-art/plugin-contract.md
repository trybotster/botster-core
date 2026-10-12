# Prior art: botster-plugin-contract (BUILD.md rule 0)

Feature: `plugin-contract` (HP J4e with A1 and A2). Written by the implementer, 2026-10-01.

## What exists

**Vault (`~/knowledge/notes/`).** Read in full or in part:
- `botster plugin frame schema owns immutable type and lane bytes`, `botster plugin call class selects the lane independently of frame type`, `botster plugin cancel remains state changing despite best effort`: steward ruling R-4. The crate pins the bytes; a call class picks the lane, not the frame type.
- `botster plugin frames validate framing before epoch state and type`: steward ruling R-6.2. The decoder follows this order.
- `botster plugin manifests require exactly four fields with optional defaults`: steward ruling R-6.1. The manifest type requires `botster_plugin_api`, `name`, `version` and `entrypoints`.
- `botster package manifests and lockfiles should declare capabilities and provenance`, `botster packages should enforce core hub cli plugin provider boundaries`, `botster is a lua plugin platform not an agent tool`: boundary and provenance facts that the lock record and the capability types follow.

**Existing code, read-only.**
- `botster-hub@9bc6cd7d4963ae5cccd642931619b9cac5c8daac`: the plugin test fixtures `crates/botster-hub-test-support/fixtures/plugin-contract-matrix/botster-package.json` and the `crates/botster-plugin-test-kit/fixtures/*/botster-package.json`.
- The plugin repos in `~/Projects/`: `botster-workspaces` (`botster-package.json` at 7ec8fac, and its fixtures), `botster-project-pipelines` (`botster-package.json` at 643c4d7). `botster-messaging` and `botster-orchestrator` hold only an initial commit with no manifest.

**Core rebuild plan (`~/botster-sessions/shared/core-rebuild-plan-v1.md`).** Read for the plugin transport: the plan keeps the generic service mechanism (lanes, epochs, guardian) in Core and moves the plugin-specific half (invocations, host port, `Loaded`) to the Hub (rows 10, 85, 86; decision R13). This crate therefore holds only the Hub-to-plugin frames and tables on the service lanes. It has no Core type and no process-host mechanism.

**Parity audit.** `frozen/final6/audits/plugin-parity-20260930.md` (G-5 target scope, G-6 filesystem roots, G-7 plugin-writable secrets) and its resolution in HP A1 and A2.

## Manifest check against the real manifests

The real manifests use the legacy file name `botster-package.json` and these keys: `name`, `version`, `kind`, `botster`, `description`, `source {type, path}`, `capabilities`, `surfaces`, `navigation`, `entrypoints` (an array of `{runtime, path, bootstrap}`), `configuration`, `features`, `events.emitted`, and a `plugin_db` capability with a scope.

Each difference is a contract fact. MN-1 is a closed key set, and the Lua binding (Part C) lists the mechanical migration. None is a bug in the schema:

| Legacy | v1 contract |
|---|---|
| `botster-package.json` | `botster-plugin.json` (PK-1) |
| `kind: "plugin"` | no key; the file name gives the package kind (PK-6) |
| `botster: ">=0.1.0"` | `botster_plugin_api: 1` and `requirements` (MN-1, PD-2) |
| `surfaces`, `navigation` | `views` of the UI contract (VW-1, VW-6) |
| `configuration` | `config` (MN-1, CF-1) |
| `events.emitted` | `events.declared` (MN-3) |
| `entrypoints: [{runtime, path}]` | `entrypoints: {service: {lua}}` (MN-1, A2-47) |
| `{surface: "plugin_db", scope}` capability | implied by `storage.collections`; never written (MN-2) |
| `source {type, path}` | informational `source`; the install records the truth (PD-1, PM-1) |
| `features` | `requirements.features` (PD-2) |

What carries over unchanged: `name`, `version`, `description`, and the capability shape `{surface, scope?}` (for example `{surface: "session_actions", scope: "session_type_spawn"}` is valid under MN-2, and `{surface: "mcp"}`, `{surface: "surfaces"}` too).

## Reused and rejected

- **Reused (as data only, no code):** the vocabulary of surfaces and scopes and the manifest field names that HP kept. No old code is in the tree, so no `Stolen-From` commit exists.
- **Rejected:** the legacy key set (above) and the `plugin_db` capability entry. The contract replaces them.
- **Libraries.** `schemars` makes the schemas (the design's choice). The `semver` crate checks versions (PD-1) instead of a hand-written parser. `jsonschema` is a dev-dependency and checks that code and artifacts agree.
- **Hand-rolled, with the reason.** The frame decoder's duplicate-key check (WF-2) is a small `serde` visitor: `serde_json` keeps the last of two equal keys and no maintained crate refuses them. The framing itself (WF-1) is contract-mandated (R-4), so it is not a library choice.
