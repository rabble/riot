import { realpathSync } from "node:fs";
import { createRequire } from "node:module";

// Run with the exact command recorded in playwright.config.mjs. Anchoring a
// require at npx's real CLI/worker entry exposes Playwright's public `test`
// export without committing a package.json solely for this smoke suite.
const requireFromPlaywright = createRequire(realpathSync(process.argv[1]));
const playwrightPackage = requireFromPlaywright("playwright/package.json");
if (playwrightPackage.version !== "1.61.1") {
  throw new Error(`Playwright 1.61.1 required; got ${playwrightPackage.version}`);
}
const { expect, test } = requireFromPlaywright("playwright/test");

for (const state of ["seeded", "empty"]) {
  test(`Checklist has a usable ${state} preview`, async ({ page }) => {
    await page.goto(`/apps/checklist/?state=${state}`);

    await expect(page.locator("h1")).toHaveCount(1);
    const primaryAction = page.getByRole("button", { name: "Add" });
    await expect(primaryAction).toBeVisible();
    if (state === "seeded") {
      await expect(page.locator("#items li")).toHaveCount(2);
      await page.getByLabel("New item").fill("Share the route");
      await primaryAction.click();
      await expect(page.locator("#items li")).toHaveCount(3);
      await expect(page.getByText("Share the route", { exact: true })).toBeVisible();
    } else {
      await expect(page.locator("#empty")).toBeVisible();
      await expect(page.locator("#items li")).toHaveCount(0);
    }

    const dimensions = await page.evaluate(() => ({
      clientWidth: document.documentElement.clientWidth,
      scrollWidth: document.documentElement.scrollWidth,
    }));
    expect(dimensions.scrollWidth).toBeLessThanOrEqual(dimensions.clientWidth);

    const box = await primaryAction.boundingBox();
    expect(box, "primary action must have a layout box").not.toBeNull();
    expect(box.height).toBeGreaterThanOrEqual(44);
  });
}
