import { defineConfig, devices } from "@playwright/test";

// E2E smoke runs against the DEV server with the fixture backend enabled —
// zero gateway required, CI-runnable. Production bundles exclude fixtures by
// design, so prod smoke happens at embedding cutover against a real gateway.
export default defineConfig({
  testDir: "e2e",
  timeout: 30_000,
  fullyParallel: true,
  retries: process.env["CI"] === undefined ? 0 : 1,
  reporter: [["list"]],
  use: {
    baseURL: "http://127.0.0.1:5199",
    trace: "retain-on-failure",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
    command: "npm run dev -- --port 5199 --strictPort --host 127.0.0.1",
    url: "http://127.0.0.1:5199",
    reuseExistingServer: process.env["CI"] === undefined,
    env: { VITE_PRISM_FIXTURES: "1" },
  },
});
