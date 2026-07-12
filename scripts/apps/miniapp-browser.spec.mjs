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

const APPS = [
  { app: "checklist", name: "Tasks", seededAction: "Add task", emptyAction: "Add task" },
  { app: "supply-board", name: "Needs & Offers", seededAction: "Post item", emptyAction: "Post item" },
  { app: "roll-call", name: "Events", seededAction: "Create event", emptyAction: "Create event" },
  { app: "quick-poll", name: "Decisions", seededAction: "Add a crossing guard", emptyAction: "Ask a new question" },
];

test("Tasks primary flow", async ({ page }) => {
  await page.goto("/apps/checklist/?state=seeded");
  await page.getByLabel("New task").fill("Bring extension cord");
  await page.getByRole("button", { name: "Add task" }).click();
  await expect(page.getByText("Bring extension cord", { exact: true })).toBeVisible();
});

test("Needs & Offers primary flow", async ({ page }) => {
  await page.goto("/apps/supply-board/?state=seeded");
  await page.getByLabel("What is needed or offered?").fill("Two folding tables");
  await page.getByRole("button", { name: "Post item" }).click();
  await expect(page.getByText("Two folding tables", { exact: true })).toBeVisible();
});

test("Events primary flow", async ({ page }) => {
  await page.goto("/apps/roll-call/?state=seeded");
  const before = await page.locator("#events > li").count();
  await page.getByRole("button", { name: "Create event" }).click();
  await page.getByLabel("Event title").fill("Lantern walk");
  await page.getByRole("button", { name: "Save event" }).click();
  await expect(page.locator("#events > li")).toHaveCount(before + 1);
  const event = page.locator(".event").filter({ hasText: "Lantern walk" });
  await expect(event).toHaveCount(1);
  await expect(event).toContainText("Place to be decided");
});

test("Events preserves a typed place", async ({ page }) => {
  await page.goto("/apps/roll-call/?state=seeded");
  await page.getByRole("button", { name: "Create event" }).click();
  await page.getByLabel("Event title").fill("Tool swap");
  await page.getByLabel("Place").fill("Library steps");
  await page.getByRole("button", { name: "Save event" }).click();
  const event = page.locator(".event").filter({ hasText: "Tool swap" });
  await expect(event).toHaveCount(1);
  await expect(event).toContainText("Library steps");
});

test("Decisions primary flow", async ({ page }) => {
  await page.goto("/apps/quick-poll/?state=seeded");
  await page.getByRole("button", { name: "Add a crossing guard" }).click();
  await expect(page.locator("#tally")).toHaveText("1 vote");
});

test("Tasks preserves a draft after a write error", async ({ page }) => {
  await page.goto("/apps/checklist/?state=error");
  await page.getByLabel("New task").fill("Keep this task draft");
  await page.getByRole("button", { name: "Add task" }).click();
  await expect(page.getByLabel("New task")).toHaveValue("Keep this task draft");
  await expect(page.getByRole("alert")).toContainText("draft is safe");
});

test("Needs & Offers preserves a draft after a write error", async ({ page }) => {
  await page.goto("/apps/supply-board/?state=error");
  await page.getByLabel("What is needed or offered?").fill("Keep this item draft");
  await page.getByRole("button", { name: "Post item" }).click();
  await expect(page.getByLabel("What is needed or offered?")).toHaveValue("Keep this item draft");
  await expect(page.getByRole("alert")).toContainText("draft is safe");
});

test("Events preserves a draft after a write error", async ({ page }) => {
  await page.goto("/apps/roll-call/?state=error");
  await page.getByRole("button", { name: "Create event" }).click();
  await page.getByLabel("Event title").fill("Keep this event draft");
  await page.getByLabel("Place").fill("Courtyard");
  await page.getByRole("button", { name: "Save event" }).click();
  await expect(page.getByLabel("Event title")).toHaveValue("Keep this event draft");
  await expect(page.getByRole("alert")).toContainText("draft is safe");
});

test("Decisions preserves a draft after a write error", async ({ page }) => {
  await page.goto("/apps/quick-poll/?state=error");
  await page.getByRole("button", { name: "Ask a new question" }).click();
  await page.getByLabel("Question", { exact: true }).fill("Keep this question draft?");
  await page.getByLabel("Choice 1", { exact: true }).fill("First choice");
  await page.getByLabel("Choice 2", { exact: true }).fill("Second choice");
  await page.getByRole("button", { name: "Post question" }).click();
  await expect(page.getByLabel("Question", { exact: true })).toHaveValue("Keep this question draft?");
  await expect(page.getByRole("alert")).toContainText("draft is safe");
});

for (const { app, name, seededAction, emptyAction } of APPS) {
  for (const state of ["seeded", "empty"]) {
    test(`${name} has a usable ${state} preview`, async ({ page }) => {
      await page.goto(`/apps/${app}/?state=${state}`);
      await expect(page.locator("h1")).toHaveCount(1);
      const primaryAction = page.getByRole("button", { name: state === "seeded" ? seededAction : emptyAction });
      await expect(primaryAction).toBeVisible();
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
}
