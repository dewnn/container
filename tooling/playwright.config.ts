import { defineConfig } from "@playwright/test";
import { fileURLToPath } from "node:url";

export default defineConfig({
  testDir: "./tests/e2e",
  outputDir: "../test-results",
  timeout: 30_000,
  use: { baseURL: "http://127.0.0.1:1427", headless: true },
  webServer: {
    cwd: fileURLToPath(new URL("../", import.meta.url)),
    command: "pnpm exec vite --host 127.0.0.1 --port 1427 --strictPort",
    url: "http://127.0.0.1:1427",
    reuseExistingServer: !process.env.CI,
    timeout: 30_000,
  },
});
