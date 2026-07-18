import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./web",
  outputDir: "target/playwright-results",
  reporter: "line",
  timeout: 120_000,
  workers: 1,
  expect: { timeout: 30_000 },
  use: {
    baseURL: "http://127.0.0.1:8000",
    viewport: { width: 1000, height: 700 },
  },
  webServer: {
    command: "python3 -m http.server 8000 --bind 127.0.0.1",
    url: "http://127.0.0.1:8000/web/",
    reuseExistingServer: true,
  },
  projects: [
    {
      name: "chrome",
      grepInvert: /high-DPI/,
      use: { ...devices["Desktop Chrome"], channel: "chrome" },
    },
    {
      name: "firefox",
      grepInvert: /high-DPI/,
      use: { ...devices["Desktop Firefox"] },
    },
    {
      name: "webkit",
      grepInvert: /high-DPI/,
      use: { ...devices["Desktop Safari"] },
    },
    {
      name: "chrome-hidpi",
      grep: /high-DPI/,
      use: {
        ...devices["Desktop Chrome"],
        channel: "chrome",
        deviceScaleFactor: 2,
      },
    },
  ],
});
