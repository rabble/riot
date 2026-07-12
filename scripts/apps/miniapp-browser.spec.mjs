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

const LIFECYCLE_APPS = [
  { app: "checklist", name: "Tasks", action: "Add task", root: "tasks", existing: "Existing neighborhood task", field: "New task", draft: "A valid task" },
  { app: "supply-board", name: "Needs & Offers", action: "Post item", root: "items", existing: "Existing supply request", field: "What is needed or offered?", draft: "A valid request" },
  { app: "roll-call", name: "Events", action: "Create event", root: "events", existing: "Existing block gathering" },
  { app: "quick-poll", name: "Decisions", action: "Ask a new question", root: "proposals", existing: "Existing community decision?" },
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

for (const { app, name, action, root, existing, field, draft } of LIFECYCLE_APPS) {
  test(`${name} waits for identity before enabling mutations`, async ({ page }) => {
    await page.goto(`/apps/${app}/?state=delayed-identity`);
    const control = page.getByRole("button", { name: action });
    if (field) await page.getByLabel(field).fill(draft);
    await expect(control).toBeDisabled();
    await expect(control).toBeEnabled();
    await expect(page.getByText(existing, { exact: true })).toBeVisible();
    if (app === "quick-poll") await expect(page.getByRole("button", { name: "First option" })).toBeEnabled();
  });

  test(`${name} keeps mutations disabled when identity fails`, async ({ page }) => {
    await page.goto(`/apps/${app}/?state=identity-error`);
    const control = page.getByRole("button", { name: action });
    if (field) await page.getByLabel(field).fill(draft);
    await expect(control).toBeDisabled();
    await expect(page.getByRole("alert")).toContainText(/identity|shared storage/i);
    await expect(page.getByText(existing, { exact: true })).toBeVisible();
    await expect(control).toBeDisabled();
  });

  test(`${name} preserves existing unmarked data without adding demos`, async ({ page }) => {
    await page.goto(`/apps/${app}/?state=existing-unmarked`);
    if (field) await page.getByLabel(field).fill(draft);
    await expect(page.getByRole("button", { name: action })).toBeEnabled();
    await expect(page.getByText(existing, { exact: true })).toBeVisible();
    await expect.poll(() => page.evaluate((prefix) => riot.list(prefix).then((rows) => rows.length), root)).toBe(1);
    await expect.poll(() => page.evaluate(() => riot.get("meta/seeded").then((value) => value && value.status))).toBe("ready");
  });

  test(`${name} filters malformed rows while rendering valid siblings`, async ({ page }) => {
    await page.goto(`/apps/${app}/?state=malformed`);
    if (field) await page.getByLabel(field).fill(draft);
    if (app === "quick-poll") await expect(page.locator("#question")).toHaveText("No decision is open yet");
    else await expect(page.getByText(existing, { exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: action })).toBeEnabled();
  });
}

test("Tasks enables submit only for a valid ready form", async ({ page }) => {
  await page.goto("/apps/checklist/?state=empty");
  const submit = page.getByRole("button", { name: "Add task" });
  await expect(submit).toBeDisabled();
  await page.getByLabel("New task").fill("Sweep the steps");
  await expect(submit).toBeEnabled();
  await page.getByLabel("New task").fill("   ");
  await expect(submit).toBeDisabled();
});

test("Needs & Offers enables submit only for a valid ready form", async ({ page }) => {
  await page.goto("/apps/supply-board/?state=empty");
  const submit = page.getByRole("button", { name: "Post item" });
  await expect(submit).toBeDisabled();
  await page.getByLabel("What is needed or offered?").fill("A long ladder");
  await expect(submit).toBeEnabled();
  await page.getByLabel("What is needed or offered?").fill("");
  await expect(submit).toBeDisabled();
});

test("Events enables save only for a valid ready form", async ({ page }) => {
  await page.goto("/apps/roll-call/?state=empty");
  await page.getByRole("button", { name: "Create event" }).click();
  const submit = page.getByRole("button", { name: "Save event" });
  await expect(submit).toBeDisabled();
  await page.getByLabel("Event title").fill("Porch concert");
  await expect(submit).toBeEnabled();
  await page.getByLabel("Event title").fill(" ");
  await expect(submit).toBeDisabled();
});

test("Decisions enables submit only for two valid choices", async ({ page }) => {
  await page.goto("/apps/quick-poll/?state=empty");
  await page.getByRole("button", { name: "Ask a new question" }).click();
  const submit = page.getByRole("button", { name: "Post question" });
  await expect(submit).toBeDisabled();
  await page.getByLabel("Question", { exact: true }).fill("When should we meet?");
  await page.getByLabel("Choice 1", { exact: true }).fill("Tuesday");
  await page.getByLabel("Choice 2", { exact: true }).fill("Thursday");
  await expect(submit).toBeEnabled();
  await page.getByLabel("Choice 2", { exact: true }).fill(" ");
  await expect(submit).toBeDisabled();
});

test("Tasks serializes completion and assignment mutations per row", async ({ page }) => {
  await page.goto("/apps/checklist/?state=slow-write");
  const row = page.locator(".task").filter({ hasText: "Existing neighborhood task" });
  const complete = row.locator(".toggle");
  const assign = row.getByRole("button", { name: /Take this/ });
  await complete.click();
  await expect(complete).toBeDisabled();
  await expect(assign).toBeDisabled();
  await expect(complete).toBeEnabled();
  await assign.click();
  await expect(row).toHaveClass(/done/);
  await expect(row).toContainText("Taken by You");
});

test("Needs & Offers resolves and reopens one item", async ({ page }) => {
  await page.goto("/apps/supply-board/?state=seeded");
  const row = page.locator(".card").filter({ hasText: "Six folding chairs" });
  await row.getByRole("button", { name: /Mark resolved/ }).click();
  await expect(row.getByRole("button", { name: /Reopen/ })).toBeVisible();
  await row.getByRole("button", { name: /Reopen/ }).click();
  await expect(row.getByRole("button", { name: /Mark resolved/ })).toBeVisible();
});

test("Events records and cancels an RSVP", async ({ page }) => {
  await page.goto("/apps/roll-call/?state=seeded");
  const row = page.locator(".event").filter({ hasText: "Community garden workday" });
  await row.getByRole("button", { name: /RSVP to/ }).click();
  await expect(row.getByRole("button", { name: /Cancel RSVP/ })).toHaveAttribute("aria-pressed", "true");
  await row.getByRole("button", { name: /Cancel RSVP/ }).click();
  await expect(row.getByRole("button", { name: /RSVP to/ })).toHaveAttribute("aria-pressed", "false");
});

test("Decisions moves one profile's vote instead of adding another", async ({ page }) => {
  await page.goto("/apps/quick-poll/?state=seeded");
  const first = page.getByRole("button", { name: /Add a crossing guard/ });
  const second = page.getByRole("button", { name: /Paint a brighter crosswalk/ });
  await expect(first).toHaveAccessibleName("Add a crossing guard, 0 votes, 0 percent");
  await first.click();
  await expect(first).toHaveAttribute("aria-pressed", "true");
  await expect(first).toHaveAccessibleName("Add a crossing guard, 1 vote, 100 percent");
  await second.click();
  await expect(first).toHaveAttribute("aria-pressed", "false");
  await expect(second).toHaveAttribute("aria-pressed", "true");
  await expect(first).toHaveAccessibleName("Add a crossing guard, 0 votes, 0 percent");
  await expect(second).toHaveAccessibleName("Paint a brighter crosswalk, 1 vote, 100 percent");
  await expect(page.locator("#tally")).toHaveText("1 vote");
});

test("Decisions ignores out-of-order profile results for replaced proposals", async ({ page }) => {
  await page.goto("/apps/quick-poll/?state=profile-race");
  await page.evaluate(() => riot.put("proposals/current", { id: "newer-decision", text: "Which evening works?", options: ["Tuesday", "Thursday"], asked_by_id: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", at: 20 }));
  await expect(page.getByText("Which evening works?", { exact: true })).toBeVisible();
  await expect(page.locator("#asked-by")).toContainText("Sam Chen");
  await page.waitForTimeout(500);
  await expect(page.locator("#asked-by")).toContainText("Sam Chen");
});

test("Decisions empty state is complete and never says loading", async ({ page }) => {
  await page.goto("/apps/quick-poll/?state=empty");
  await expect(page.locator("#question")).toHaveText("No decision is open yet");
  await expect(page.getByText(/Loading the current question/)).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Ask a new question" })).toBeEnabled();
});

test("Events cancel returns focus to Create event", async ({ page }) => {
  await page.goto("/apps/roll-call/?state=seeded");
  await page.getByRole("button", { name: "Create event" }).click();
  await page.getByRole("button", { name: "Cancel" }).click();
  await expect(page.getByRole("button", { name: "Create event" })).toBeFocused();
});

test("Decisions cancel returns focus to Ask a new question", async ({ page }) => {
  await page.goto("/apps/quick-poll/?state=seeded");
  await page.getByRole("button", { name: "Ask a new question" }).click();
  await page.getByRole("button", { name: "Cancel" }).click();
  await expect(page.getByRole("button", { name: "Ask a new question" })).toBeFocused();
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
