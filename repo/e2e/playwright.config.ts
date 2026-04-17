import { defineConfig, devices } from "@playwright/test";

/**
 * Playwright config for the SilverOak fullstack E2E suite.
 *
 * `BASE_URL` points at the nginx proxy (service `proxy` in compose network —
 * `http://proxy:80`).  The proxy forwards `/` to the frontend and `/api/*` to
 * the backend API, so every browser action exercises the full FE↔BE path.
 */
const BASE_URL = process.env.BASE_URL ?? "http://proxy:80";

export default defineConfig({
  testDir: "./tests",
  fullyParallel: false,
  // Tests mutate shared state (cart, requisitions) — keep sequential for
  // deterministic outcomes.
  workers: 1,
  retries: process.env.CI ? 1 : 0,
  timeout: 60_000,
  reporter: [
    ["list"],
    ["html", { outputFolder: "playwright-report", open: "never" }],
    ["json", { outputFile: "playwright-report/results.json" }],
  ],
  use: {
    baseURL: BASE_URL,
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    video: "retain-on-failure",
    // The Yew app is a single-page WASM bundle.  Give navigation some headroom
    // on cold starts where the browser compiles the .wasm module.
    navigationTimeout: 30_000,
    actionTimeout: 15_000,
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
});
