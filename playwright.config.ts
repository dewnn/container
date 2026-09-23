import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/e2e",
  timeout: 30_000,
  use: { baseURL: "http://127.0.0.1:1427", headless: true },
  webServer: {
    command: "pnpm exec vite --host 127.0.0.1 --port 1427 --strictPort",
    url: "http://127.0.0.1:1427",
    reuseExistingServer: !process.env.CI,
    timeout: 30_000,
  },
});
