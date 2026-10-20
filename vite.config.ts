import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// The Tauri window loads the dev server in development and `ui/dist` in builds
// (src-tauri/tauri.conf.json). Vitest settings live here too. The `@/` alias comes
// from tsconfig.json.
export default defineConfig({
  root: "ui",
  plugins: [react()],
  resolve: { tsconfigPaths: true },
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
  test: {
    // Help coverage must read current glob membership, including newly added topic files.
    // Vitest 5's persistent transform cache reused an empty inventory during verification.
    fsModuleCache: false,
    environment: "jsdom",
    // GitHub's runners are several times slower than a developer computer under a full
    // parallel run; a 1 s local test exceeded the 5 s default there. Longer walkthrough
    // budgets set in test files still apply.
    testTimeout: 20_000,
    include: ["src/**/*.test.{ts,tsx}"],
    setupFiles: ["src/lib/test/setup.ts"],
    restoreMocks: true,
  },
});
