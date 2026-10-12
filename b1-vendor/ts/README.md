# ts/

The TypeScript packages live here. `packages/hub-contract` (`@trybotster/hub-contract`) is the first one. It wires the shared test settings below. A package sits two levels below this directory, so its scripts use `../../scripts/check-total.mjs`.

## Shared test settings (BUILD.md "Testing: fast and lean")

| File | What it does |
|---|---|
| `vitest.shared.ts` | `testTimeout: 2000` and `hookTimeout: 2000`, and the time reporter. A package's `vitest.config.ts` merges it: `export default mergeConfig(shared, defineConfig({ ... }))`. |
| `time-reporter.ts` | Prints per-test times, slowest first, and writes `test-times.json` in the package directory. |
| `scripts/check-total.mjs` | Runs a command and fails when it takes over 60 s (`BOTSTER_TS_BUDGET_SECS` overrides, for tests of the script). |

A package wires them like this:

```json
{ "scripts": { "test": "node ../../scripts/check-total.mjs vitest run", "test:slow": "vitest run --config vitest.slow.config.ts" } }
```

The default tier is in-process only. The slow tier uses its own config (`include: ["test-slow/**"]`, no `testTimeout` limit) and runs as the `slow` CI job.
A timer in a test needs `// timer: deadline — <reason>` on its line or the line before (`cargo xtask timers` checks it).

## `@trybotster/hub-contract`

Types, JSON Schemas and validators of the Hub→client contract (HC parity45 with A1 to A4). Everything under `src/generated/` is generated; `src/transport.ts` and `src/validate.ts` are handwritten.

```sh
cargo xtask codegen                       # Rust types -> schemas/ (the source of truth)
npm run codegen -w @trybotster/hub-contract   # schemas/ -> src/generated/ (json-schema-to-typescript); add `-- --check` to fail on a diff
npm run build && npm run typecheck && npm test
npm run test:consumer -w @trybotster/hub-contract   # plain `node --test` against the built package
```

| Import | Holds |
|---|---|
| `@trybotster/hub-contract` | the frame, `describe`, watch, event, error and kind row types; `inventory` (every non-exhaustive set and its known names); `errorRegistry`; `kindRegistry` (verbs, actions, effects); the validators; `HubTransport` |
| `@trybotster/hub-contract/route` | the route frames of the codec (TS-1 `route_limits`, TS-9 `route.ended`) |
| `@trybotster/hub-contract/ui` | the ui-contract types that the `view` and `navigation` kinds reference |

`HubTransport` is the client's view of a binding (HC TB-0): `send`, pull `receive`, `progress`, `close`. A client core writes one adapter per binding against it. This package depends on no client's own interfaces.
