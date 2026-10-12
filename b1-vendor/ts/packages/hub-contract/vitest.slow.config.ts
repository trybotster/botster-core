import { defineConfig } from "vitest/config";

// The slow tier: real-process tests, no per-test limit. This package has none yet.
export default defineConfig({ test: { maxWorkers: 4, include: ["test-slow/**/*.test.ts"] } });
