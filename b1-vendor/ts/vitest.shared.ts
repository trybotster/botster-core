// Shared vitest settings for every `ts/` package (BUILD.md "Testing: fast and lean", design section 9).
// A package's vitest.config.ts merges this: `export default mergeConfig(shared, defineConfig({ ... }))`.
//
// - testTimeout 2000: a default-tier test over 2 s fails.
// - the reporter prints per-test times, slowest first, and writes `test-times.json`.
// - `npm test` runs `node ../scripts/check-total.mjs` after vitest; it fails when the run takes over 60 s.
import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    // At most 4 parallel workers: no gate drives the machine load up (docs/BUILD.md "Resources"; xtask/src/caps.rs).
    maxWorkers: 4,
    testTimeout: 2000,
    hookTimeout: 2000,
    reporters: ["default", new URL("./time-reporter.ts", import.meta.url).pathname],
  },
});
