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
  { app: "supply-board", name: "Needs & Offers", seededAction: "Post item", emptyAction: "Post item" },
  { app: "roll-call", name: "Events", seededAction: "Create event", emptyAction: "Create event" },
  { app: "quick-poll", name: "Decisions", seededAction: "Add a crossing guard", emptyAction: "Ask a new question" },
  { app: "chat", name: "Chat", seededAction: "Send", emptyAction: "Send" },
  { app: "dispatches", name: "Dispatches", seededAction: "Write a dispatch", emptyAction: "Write a dispatch" },
];

const LIFECYCLE_APPS = [
  { app: "supply-board", name: "Needs & Offers", action: "Post item", root: "items", existing: "Existing supply request", field: "What is needed or offered?", draft: "A valid request" },
  { app: "roll-call", name: "Events", action: "Create event", root: "events", existing: "Existing block gathering" },
  { app: "quick-poll", name: "Decisions", action: "Ask a new question", root: "proposals", existing: "Existing community decision?" },
  { app: "chat", name: "Chat", action: "Send", root: "messages", existing: "I can bring extra tea.", field: "Message", draft: "A valid message" },
  { app: "dispatches", name: "Dispatches", action: "Write a dispatch", root: "posts", existing: "The garden gate is open again" },
];

test("Frozen Checklist primary flow", async ({ page }) => {
  await page.goto("/apps/checklist/?state=seeded");
  await page.getByLabel("New item").fill("Bring extension cord");
  await page.getByRole("button", { name: "Add", exact: true }).click();
  await expect(page.getByText("Bring extension cord", { exact: true })).toBeVisible();
});

test("Chat primary flow", async ({ page }) => {
  await page.goto("/apps/chat/?state=seeded");
  await page.getByLabel("Message").fill("I can bring tea.");
  await page.getByRole("button", { name: "Send" }).click();
  await expect(page.getByText("I can bring tea.", { exact: true })).toBeVisible();
});

test("Dispatches primary flow", async ({ page }) => {
  await page.goto("/apps/dispatches/?state=seeded");
  await page.getByRole("button", { name: "Write a dispatch" }).click();
  await page.getByLabel("Title").fill("Garden gate repaired");
  await page.getByLabel("Dispatch").fill("The east entrance is open again.");
  await page.getByRole("button", { name: "Publish" }).click();
  await expect(page.getByText("Garden gate repaired", { exact: true })).toBeVisible();
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

test("Frozen Checklist preserves a draft after a write error", async ({ page }) => {
  await page.goto("/apps/checklist/?state=error");
  await page.getByLabel("New item").fill("Keep this task draft");
  await page.getByRole("button", { name: "Add", exact: true }).click();
  await expect(page.getByLabel("New item")).toHaveValue("Keep this task draft");
  await expect(page.getByRole("alert")).toContainText("Couldn't save");
});

test("Chat preserves a draft after a write error", async ({ page }) => {
  await page.goto("/apps/chat/?state=error");
  await page.getByLabel("Message").fill("Keep this message draft");
  await page.getByRole("button", { name: "Send" }).click();
  await expect(page.getByLabel("Message")).toHaveValue("Keep this message draft");
  await expect(page.getByRole("alert")).toContainText(/draft|safe|try again/i);
});

test("Dispatches preserves both drafts after a write error", async ({ page }) => {
  await page.goto("/apps/dispatches/?state=error");
  await page.getByRole("button", { name: "Write a dispatch" }).click();
  await page.getByLabel("Title").fill("Keep this title");
  await page.getByLabel("Dispatch").fill("Keep this longer dispatch body.");
  await page.getByRole("button", { name: "Publish" }).click();
  await expect(page.getByLabel("Title")).toHaveValue("Keep this title");
  await expect(page.getByLabel("Dispatch")).toHaveValue("Keep this longer dispatch body.");
  await expect(page.getByRole("alert")).toContainText(/draft|safe|try again/i);
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
  await page.getByLabel("Choice 2", { exact: true }).fill("Thursday");
  await page.getByLabel("Choice 3").fill("Saturday");
  await expect(submit).toBeDisabled();
  await page.getByLabel("Choice 1", { exact: true }).fill("Tuesday");
  await expect(submit).toBeEnabled();
  await page.getByLabel("Choice 4").fill("Sunday");
  await page.getByLabel("Choice 2", { exact: true }).fill(" ");
  await expect(submit).toBeDisabled();
});

test("Needs & Offers resolves and reopens one item", async ({ page }) => {
  await page.goto("/apps/supply-board/?state=seeded");
  const row = page.locator(".card").filter({ hasText: "Six folding chairs" });
  await row.getByRole("button", { name: /Mark resolved/ }).click();
  await expect(row.getByRole("button", { name: /Reopen/ })).toBeVisible();
  await row.getByRole("button", { name: /Reopen/ }).click();
  await expect(row.getByRole("button", { name: /Mark resolved/ })).toBeVisible();
});

test("Needs & Offers does not reopen a remotely resolved item from a stale action", async ({ page }) => {
  await page.goto("/apps/supply-board/?state=seeded");
  const row = page.locator(".card").filter({ hasText: "Six folding chairs" });
  await expect(row.getByRole("button", { name: /Mark resolved/ })).toBeVisible();
  await page.evaluate(() => __miniappPreview.remotePut("items/folding-chairs", {
    kind: "need",
    text: "Six folding chairs",
    created_at: 1,
    added_by_id: "a".repeat(64),
    resolved_by_id: "b".repeat(64),
  }));
  await row.getByRole("button", { name: /Mark resolved/ }).click();
  await expect.poll(() => page.evaluate(() => riot.get("items/folding-chairs").then((value) => value.resolved_by_id))).toBe("b".repeat(64));
});

test("Events records and cancels an RSVP", async ({ page }) => {
  await page.goto("/apps/roll-call/?state=seeded");
  const row = page.locator(".event").filter({ hasText: "Community garden workday" });
  await row.getByRole("button", { name: /RSVP to/ }).click();
  await expect(row.getByRole("button", { name: /Cancel RSVP/ })).toHaveAttribute("aria-pressed", "true");
  await row.getByRole("button", { name: /Cancel RSVP/ }).click();
  await expect(row.getByRole("button", { name: /RSVP to/ })).toHaveAttribute("aria-pressed", "false");
});

test("Events does not cancel a remote RSVP from a stale I'm going action", async ({ page }) => {
  await page.goto("/apps/roll-call/?state=seeded");
  const row = page.locator(".event").filter({ hasText: "Community garden workday" });
  await expect(row.getByRole("button", { name: /RSVP to/ })).toBeVisible();
  const key = `rsvps/community-garden-workday/${"1".repeat(64)}`;
  await page.evaluate(({ key }) => __miniappPreview.remotePut(key, { attending: true, at: 20 }), { key });
  await row.getByRole("button", { name: /RSVP to/ }).click();
  await expect.poll(() => page.evaluate((key) => riot.get(key).then((value) => value.attending), key)).toBe(true);
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

test("Decisions refreshes a cached profile after shared data repaints", async ({ page }) => {
  await page.goto("/apps/quick-poll/?state=seeded");
  await expect(page.locator("#asked-by")).toContainText("Alex Rivera");
  await page.evaluate(() => {
    __miniappPreview.setProfile("a".repeat(64), { displayName: "Alex Morgan", tag: "new-name" });
    return riot.put("votes/safer-school-crossing/" + "b".repeat(64), { choice: 0, at: 30 });
  });
  await expect(page.locator("#asked-by")).toContainText("Alex Morgan");
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

test("Chat keeps the final message clear of a resized composer on phone", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/apps/chat/?state=seeded");
  await page.evaluate(async () => {
    for (let index = 0; index < 16; index += 1) {
      await riot.put(`messages/${100 + index}-clearance-${index}`, {
        text: index === 15 ? "Final message stays visible" : `Conversation line ${index + 1}`,
        created_at: 100 + index,
        author_id: index % 2 ? "a".repeat(64) : "b".repeat(64),
      });
    }
  });
  await expect(page.getByText("Final message stays visible", { exact: true })).toBeVisible();
  await page.getByLabel("Message").evaluate((textarea) => { textarea.style.height = "140px"; });
  await expect.poll(() => page.locator("#composer").evaluate((composer) => composer.getBoundingClientRect().height)).toBeGreaterThan(112);
  await page.evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight));
  const clearance = await page.evaluate(() => ({
    messageBottom: document.querySelector("#messages li:last-child").getBoundingClientRect().bottom,
    composerTop: document.getElementById("composer").getBoundingClientRect().top,
  }));
  expect(clearance.messageBottom).toBeLessThanOrEqual(clearance.composerTop);
});

test("Chat refreshes an author's profile after shared data changes", async ({ page }) => {
  await page.goto("/apps/chat/?state=seeded");
  await expect(page.locator(".message").filter({ hasText: "Is anyone heading" }).locator(".meta")).toContainText("Alex Rivera");
  await page.evaluate(() => {
    __miniappPreview.setProfile("a".repeat(64), { displayName: "Alex Morgan", tag: "new-name" });
    return riot.put("messages/99-profile-refresh", { text: "Profile refresh", created_at: 99, author_id: "b".repeat(64) });
  });
  await expect(page.locator(".message").filter({ hasText: "Is anyone heading" }).locator(".meta")).toContainText("Alex Morgan");
});

test("Dispatches refreshes an author's profile after shared data changes", async ({ page }) => {
  await page.goto("/apps/dispatches/?state=seeded");
  const gardenPost = page.locator(".post").filter({ hasText: "The garden gate is open again" });
  await expect(gardenPost.locator(".meta")).toContainText("Alex Rivera");
  await page.evaluate(() => {
    __miniappPreview.setProfile("a".repeat(64), { displayName: "Alex Morgan", tag: "new-name" });
    return riot.put("posts/99-profile-refresh", { title: "Profile refresh", body: "Refresh profile names.", summary: "Refresh profile names.", created_at: 99, author_id: "b".repeat(64) });
  });
  await expect(gardenPost.locator(".meta")).toContainText("Alex Morgan");
});

test("Chat locks its draft while a send is pending", async ({ page }) => {
  await page.goto("/apps/chat/?state=slow-write");
  const message = page.getByLabel("Message");
  await message.fill("Pending chat message");
  await page.getByRole("button", { name: "Send" }).click();
  await expect(message).toBeDisabled();
  await expect(message).toBeEnabled();
  await expect(message).toHaveValue("");
  await expect(page.getByText("Pending chat message", { exact: true })).toBeVisible();
});

test("Dispatches locks both drafts and cancel while publishing", async ({ page }) => {
  await page.goto("/apps/dispatches/?state=slow-write");
  await page.getByRole("button", { name: "Write a dispatch" }).click();
  const title = page.getByLabel("Title"); const body = page.getByLabel("Dispatch"); const cancel = page.getByRole("button", { name: "Cancel" });
  await title.fill("Pending dispatch"); await body.fill("This dispatch is still being published.");
  await page.getByRole("button", { name: "Publish" }).click();
  await expect(title).toBeDisabled(); await expect(body).toBeDisabled(); await expect(cancel).toBeDisabled();
  await expect(page.getByText("Pending dispatch", { exact: true })).toBeVisible();
  await expect(page.locator("#detail-view")).toBeVisible();
});

test("Chat clears a failed-write alert after a successful retry", async ({ page }) => {
  await page.goto("/apps/chat/?state=error");
  const message = page.getByLabel("Message"); await message.fill("Retry this message");
  await page.getByRole("button", { name: "Send" }).click();
  await expect(page.locator("#error")).toBeVisible(); await expect(message).toHaveValue("Retry this message");
  await page.evaluate(() => __miniappPreview.setWriteFailures(false));
  await page.getByRole("button", { name: "Send" }).click();
  await expect(page.getByText("Retry this message", { exact: true })).toBeVisible();
  await expect(page.locator("#error")).toBeHidden();
  await expect(page.locator("#status")).toHaveText(/\d+ messages/);
});

test("Dispatches clears a failed-write alert after a successful retry", async ({ page }) => {
  await page.goto("/apps/dispatches/?state=error");
  await page.getByRole("button", { name: "Write a dispatch" }).click();
  await page.getByLabel("Title").fill("Retry this dispatch"); await page.getByLabel("Dispatch").fill("Keep both fields until this succeeds.");
  await page.getByRole("button", { name: "Publish" }).click();
  await expect(page.locator("#error")).toBeVisible(); await expect(page.getByLabel("Title")).toHaveValue("Retry this dispatch");
  await page.evaluate(() => __miniappPreview.setWriteFailures(false));
  await page.getByRole("button", { name: "Publish" }).click();
  await expect(page.getByText("Retry this dispatch", { exact: true })).toBeVisible();
  await expect(page.locator("#error")).toBeHidden();
  await expect(page.locator("#status")).toHaveText(/\d+ dispatches/);
});

test("Chat typing does not rebuild existing message rows", async ({ page }) => {
  await page.goto("/apps/chat/?state=seeded");
  await page.evaluate(() => { window.__firstChatRow = document.querySelector("#messages li"); });
  await page.getByLabel("Message").fill("Draft without repaint");
  expect(await page.evaluate(() => window.__firstChatRow === document.querySelector("#messages li"))).toBe(true);
});

test("Dispatches returns focus to the index when an open detail becomes invalid", async ({ page }) => {
  await page.goto("/apps/dispatches/?state=existing-unmarked");
  await page.getByText("The garden gate is open again", { exact: true }).click();
  await page.evaluate(() => riot.put("posts/10-existing", null));
  await expect(page.locator("#index-view")).toBeVisible();
  await expect(page.getByRole("button", { name: "Write a dispatch" })).toBeFocused();
});

test("Dispatches clears a failed-publish alert when Cancel abandons the drafts", async ({ page }) => {
  await page.goto("/apps/dispatches/?state=error");
  await page.getByRole("button", { name: "Write a dispatch" }).click();
  await page.getByLabel("Title").fill("Abandon this title");
  await page.getByLabel("Dispatch").fill("This failed draft will be intentionally abandoned.");
  await page.getByRole("button", { name: "Publish" }).click();
  await expect(page.locator("#error")).toContainText("drafts are safe");
  await page.getByRole("button", { name: "Cancel" }).click();
  await expect(page.locator("#index-view")).toBeVisible();
  await expect(page.locator("#error")).toBeHidden();
  await expect(page.locator("#status")).toHaveText(/\d+ dispatches/);
  await page.getByRole("button", { name: "Write a dispatch" }).click();
  await expect(page.getByLabel("Title")).toHaveValue("");
  await expect(page.getByLabel("Dispatch")).toHaveValue("");
});

test("Dispatches focuses its index landmark when read-only detail is invalidated", async ({ page }) => {
  await page.goto("/apps/dispatches/?state=identity-error");
  await expect(page.getByRole("button", { name: "Write a dispatch" })).toBeDisabled();
  await page.getByText("The garden gate is open again", { exact: true }).click();
  await page.evaluate(() => riot.put("posts/10-existing", null));
  await expect(page.locator("#index-view")).toBeVisible();
  await expect(page.locator("#index-view")).toBeFocused();
});
