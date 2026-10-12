// A vitest reporter (vitest 3 and later): per-test times, slowest first, plus `test-times.json` in the package directory.
// The file has the same shape as `target/test-times.json` of `cargo xtask test-budget`.
import { writeFileSync } from "node:fs";
import { relative } from "node:path";
import type { Reporter, TestModule } from "vitest/node";

export default class TimeReporter implements Reporter {
  onTestRunEnd(testModules: ReadonlyArray<TestModule>): void {
    const times: Record<string, number> = {};
    for (const module of testModules) {
      const file = relative(process.cwd(), module.moduleId);
      for (const test of module.children.allTests()) {
        const secs = (test.diagnostic()?.duration ?? 0) / 1000;
        times[`${file} ${test.fullName}`] = Math.round(secs * 1000) / 1000;
      }
    }
    const rows = Object.entries(times).sort((a, b) => b[1] - a[1]);
    console.log(`\ntest-budget: ${rows.length} test(s), slowest first`);
    for (const [name, secs] of rows) console.log(`  ${secs.toFixed(3).padStart(8)}s  ${name}`);
    writeFileSync("test-times.json", JSON.stringify({ tier: "default", tests: times }, null, 2) + "\n");
  }
}
