import { defineConfig } from "vitest/config";
import { nodePolyfills } from "vite-plugin-node-polyfills";
import path from "path";
import { fileURLToPath } from "url";

const configDir = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(configDir, "..");

// One worker by default. CI lanes on bigger machines opt in to more with
// VITEST_MAX_WORKERS (scripts/ci/self-hosted/lanes-plan.mjs); anything that is
// not a positive integer keeps the default.
const requestedWorkers = Number.parseInt(process.env.VITEST_MAX_WORKERS ?? "", 10);
const maxWorkers =
  Number.isInteger(requestedWorkers) && requestedWorkers > 0 ? requestedWorkers : 1;
// Optional worker pool. CI Fast on the EX63 sets VITEST_POOL=threads: the same
// 8,764 tests pass and the run is ~18% faster than the default `forks` pool.
const requestedPool = process.env.VITEST_POOL;
const pool = requestedPool === "threads" || requestedPool === "forks" ? requestedPool : undefined;

export default defineConfig({
  root: projectRoot,
  plugins: [
    nodePolyfills({
      include: ["buffer", "process", "util", "os", "crypto", "stream"],
      globals: {
        Buffer: true,
        process: true,
        global: true,
      },
    }),
  ],
  resolve: {
    alias: {
      "@": path.resolve(projectRoot, "src"),
      buffer: "buffer",
      process: "process/browser",
      util: "util",
      os: "os-browserify/browser",
      // Resolve workspace package imports for tests that import the PTT plugin.
      "tauri-plugin-ptt-api": path.resolve(
        configDir,
        "../../packages/tauri-plugin-ptt/guest-js/index.ts"
      ),
    },
  },
  test: {
    globals: true,
    environment: "jsdom",
    maxWorkers,
    minWorkers: 1,
    ...(pool ? { pool } : {}),
    // Clear call history between tests but keep mock implementations from setup.ts
    // (mockReset/restoreMocks wipe vi.fn implementations and break shared mocks like getBackendUrl).
    clearMocks: true,
    mockReset: false,
    restoreMocks: false,
    setupFiles: ["src/test/setup.ts"],
    include: [
      "src/**/*.test.{ts,tsx}",
      "test/*.test.{ts,tsx}",
    ],
    // The PTT plugin's guest-js test (`packages/tauri-plugin-ptt/guest-js/index.test.ts`)
    // is intentionally NOT included here. The app's vitest config injects
    // `vite-plugin-node-polyfills` banner imports (Buffer/process/global) that
    // resolve fine from within `app/` but fail from outside the workspace root
    // on a stricter pnpm CI install (`Failed to resolve import
    // "vite-plugin-node-polyfills/shims/buffer"`). The PTT test only mocks the
    // Tauri JS bindings and doesn't need the polyfills — a future PR can add
    // a self-contained vitest setup at packages/tauri-plugin-ptt/.
    hookTimeout: 30000,
    testTimeout: 30000,
    coverage: {
      provider: "v8",
      include: ["src/**/*.{ts,tsx}"],
      exclude: [
        "src/main.tsx",
        "src/vite-env.d.ts",
        "src/**/*.d.ts",
        "src/test/**",
        "src/__tests__/**",
        "src/**/__tests__/**",
        "src/**/*.test.{ts,tsx}",
        "src/**/types.ts",
        "src/**/types/*.ts",
        "src/types/**",
        // Dev-only visual harnesses (not shipped, not unit-tested by design).
        "src/pages/dev/**",
      ],
      reporter: ["text", "text-summary", "html", "lcov"],
      // thresholds: {
      //   lines: 15,
      //   statements: 15,
      //   functions: 15,
      //   branches: 12,
      // },
    },
  },
});
