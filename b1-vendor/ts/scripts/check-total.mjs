// Runs a command and fails when it takes over the budget (default 60 s).
// Usage in a package.json: "test": "node ../scripts/check-total.mjs vitest run"
import { spawnSync } from "node:child_process";

const budgetSecs = Number(process.env.BOTSTER_TS_BUDGET_SECS ?? 60);
const [command, ...args] = process.argv.slice(2);
if (!command) {
  console.error("usage: check-total.mjs <command> [args...]");
  process.exit(2);
}
const started = Date.now();
const result = spawnSync(command, args, { stdio: "inherit", timeout: budgetSecs * 1000 });
const secs = (Date.now() - started) / 1000;
console.log(`test-budget: wall time ${secs.toFixed(1)} s (limit ${budgetSecs} s)`);
if (result.error?.code === "ETIMEDOUT" || secs > budgetSecs) {
  console.error(`test-budget: FAIL the tier took over ${budgetSecs} s`);
  process.exit(1);
}
process.exit(result.status ?? 1);
