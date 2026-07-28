const port = Number.parseInt(process.env.RIOT_MINIAPP_PORT || "43117", 10);

export default {
  testDir: ".",
  testMatch: "miniapp-browser.spec.mjs",
  outputDir: "../../target/miniapp-playwright-results",
  metadata: {
    command: "npx --yes playwright@1.61.1 test --config scripts/apps/playwright.config.mjs",
  },
  use: {
    baseURL: `http://127.0.0.1:${port}`,
  },
  projects: [
    { name: "phone", use: { viewport: { width: 390, height: 844 } } },
    { name: "desktop", use: { viewport: { width: 1280, height: 800 } } },
  ],
  webServer: {
    command: `PORT=${port} node scripts/apps/miniapp-preview-host.mjs`,
    cwd: "../..",
    port,
    reuseExistingServer: false,
  },
};
