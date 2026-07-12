import { existsSync, realpathSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

async function loadPlaywrightTest() {
  try {
    return await import("@playwright/test");
  } catch (error) {
    if (error?.code !== "ERR_MODULE_NOT_FOUND") throw error;

    // `npx playwright` installs the test runner in its own temporary package
    // tree, which Node does not expose to imports originating in this repo.
    // Resolve that exact running package without adding a second dependency.
    let directory = path.dirname(realpathSync(process.argv[1]));
    while (!existsSync(path.join(directory, "test.mjs")) && path.dirname(directory) !== directory) {
      directory = path.dirname(directory);
    }
    if (!existsSync(path.join(directory, "test.mjs"))) throw error;
    return import(pathToFileURL(path.join(directory, "test.mjs")).href);
  }
}

const { expect, test } = await loadPlaywrightTest();

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
