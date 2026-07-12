# Community Miniapp Suite Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship eight polished, seeded, editable, independently packaged Riot miniapps—Chat, Dispatches, Wiki, Photo Wall, Tasks, Needs & Offers, Events, and Decisions—without making them depend on the later Activity Feed.

**Architecture:** Each app is a separate framework-free HTML/CSS/JS bundle using only the existing `window.riot` bridge and app-scoped Willow data. Four existing starter fixtures are evolved into Tasks, Needs & Offers, Events, and Decisions; four new fixture directories provide Chat, Dispatches, Wiki, and Photo Wall. Canonical committed CBOR artifacts, starter-catalog verification, browser interaction tests, and screenshot review make the source, runtime behavior, and presentation independently verifiable.

**Tech Stack:** HTML5, CSS custom properties, vanilla JavaScript, `window.riot`, Node.js 26, Playwright 1.61, Rust/Cargo, canonical CBOR starter packer, Swift/WebKit host tests

---

## File Map

- `fixtures/apps/{chat,dispatches,wiki,photo-wall}/`: new independent miniapps.
- `fixtures/apps/{checklist,supply-board,roll-call,quick-poll}/`: existing apps evolved into Tasks, Needs & Offers, Events, and Decisions while preserving their storage-safe bridge patterns.
- `fixtures/apps/_shared/tokens.css`: review source for the suite’s common visual tokens; each packed app receives an exact `tokens.css` copy because bundles cannot import across app identities.
- `scripts/apps/miniapp-contracts.mjs`: source/manifest/security/accessibility contract checks for all eight apps.
- `scripts/apps/miniapp-browser.spec.mjs`: Playwright fake-host behavior and responsive screenshot tests.
- `scripts/apps/miniapp-preview-host.mjs`: local HTTP host that injects a deterministic `window.riot` mock before app code and exposes seeded, empty, error, and post-action states.
- `crates/riot-core/src/apps/starter.rs`: catalog order and embedded artifact pairs.
- `crates/riot-core/tests/apps_starter.rs`: exact catalog names, IDs, distinctness, and all-app source/artifact drift.
- `fixtures/demo/riverside/content.json` and its drift test: update only if evolving Checklist changes the pinned app ID.
- `docs/quality/2026-07-12-miniapp-visual-review.md`: screenshot findings and final per-app approval.

### Task 1: Build the shared UX foundation and automated fixture contracts

**Files:**
- Create: `fixtures/apps/_shared/tokens.css`
- Create: `scripts/apps/miniapp-contracts.mjs`
- Create: `scripts/apps/miniapp-preview-host.mjs`
- Create: `scripts/apps/miniapp-browser.spec.mjs`
- Create: `scripts/apps/playwright.config.mjs`
- Test: `scripts/apps/miniapp-contracts.mjs`
- Test: `scripts/apps/miniapp-browser.spec.mjs`

- [ ] **Step 1: Write the failing foundation contract**

Require the shared token source to provide the exact accessibility primitives. The Playwright self-test separately requires the preview host to serve an existing fixture with the deterministic mock bridge:

```js
const tokens = await readFile(join(repoRoot, "fixtures/apps/_shared/tokens.css"), "utf8");
assert.match(tokens, /min-height:\s*44px/);
assert.match(tokens, /:focus-visible/);
assert.match(tokens, /prefers-reduced-motion/);
```

- [ ] **Step 2: Run the contract and verify RED**

Run: `node scripts/apps/miniapp-contracts.mjs`

Expected: FAIL because the shared token source and preview host do not exist.

- [ ] **Step 3: Add the shared tokens and preview host**

Create the token source with the approved responsive/accessibility baseline:

```css
:root {
  color-scheme: light dark;
  --paper: #f6f1e7;
  --surface: color-mix(in srgb, var(--paper) 88%, white);
  --ink: #181713;
  --muted: #68645b;
  --line: color-mix(in srgb, var(--ink) 16%, transparent);
  --accent: #176bff;
  --radius: 18px;
  --shadow: 0 12px 32px rgb(30 24 12 / 10%);
  --space-1: 0.375rem;
  --space-2: 0.75rem;
  --space-3: 1rem;
  --space-4: 1.5rem;
  font-family: ui-rounded, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
}
* { box-sizing: border-box; }
body { margin: 0; background: var(--paper); color: var(--ink); }
button, input, textarea, select { font: inherit; }
button, [role="button"], input, select { min-height: 44px; }
:focus-visible { outline: 3px solid var(--accent); outline-offset: 3px; }
@media (prefers-reduced-motion: reduce) { *, *::before, *::after { scroll-behavior: auto !important; transition: none !important; } }
```

The preview host must serve one app at `/apps/<directory>/`, inject a deterministic mock implementing `get`, `put`, `list`, `watch`, `whoami`, and `profile`, and accept `?state=seeded|empty|error|post-action`. It must never be included in packed bundles.

- [ ] **Step 4: Add the browser smoke skeleton**

Configure phone and desktop projects and assert the shared interaction contract:

```js
export default defineConfig({
  testDir: ".",
  testMatch: "miniapp-browser.spec.mjs",
  use: { baseURL: "http://127.0.0.1:43117" },
  projects: [
    { name: "phone", use: { viewport: { width: 390, height: 844 } } },
    { name: "desktop", use: { viewport: { width: 1280, height: 800 } } },
  ],
  webServer: { command: "node scripts/apps/miniapp-preview-host.mjs", port: 43117, reuseExistingServer: false },
});
```

The initial browser self-test opens Checklist through the preview host, checks one `h1`, a visible primary action, no horizontal overflow, and a 44px minimum primary-action height. The exact suite list and app-specific assertions are added incrementally in Tasks 2–4, so every committed task is green.

- [ ] **Step 5: Run foundation checks and commit**

Run:

```bash
node scripts/apps/miniapp-contracts.mjs
npx playwright test --config scripts/apps/playwright.config.mjs
```

Expected: the foundation contract and browser self-test pass. Commit only the foundation files:

```bash
git add fixtures/apps/_shared scripts/apps
git commit -m "test(apps): add miniapp UX and browser contracts"
```

### Task 2: Evolve the four existing starters

**Files:**
- Modify: `fixtures/apps/checklist/{riot-app.json,index.html,style.css,app.js}`
- Create: `fixtures/apps/checklist/tokens.css`
- Modify: `fixtures/apps/supply-board/{riot-app.json,index.html,style.css,app.js}`
- Create: `fixtures/apps/supply-board/tokens.css`
- Modify: `fixtures/apps/roll-call/{riot-app.json,index.html,style.css,app.js}`
- Create: `fixtures/apps/roll-call/tokens.css`
- Modify: `fixtures/apps/quick-poll/{riot-app.json,index.html,style.css,app.js}`
- Create: `fixtures/apps/quick-poll/tokens.css`
- Test: `scripts/apps/miniapp-contracts.mjs`
- Test: `scripts/apps/miniapp-browser.spec.mjs`

- [ ] **Step 1: Add failing app-specific browser scenarios**

Create the source contract with the first four approved apps, then add these exact primary flows:

```js
const APPS = [
  ["checklist", "Tasks"],
  ["supply-board", "Needs & Offers"],
  ["roll-call", "Events"],
  ["quick-poll", "Decisions"],
];
```

For each listed app require `riot-app.json`, `index.html`, `tokens.css`, `style.css`, and `app.js`; exact manifest name; `riot.watch`, `riot.whoami`, and `ensureSeeded`; and absence of `innerHTML =` and `fetch(`. Add these browser flows:

```js
const flows = {
  checklist: async (page) => { await page.getByLabel("New task").fill("Bring extension cord"); await page.getByRole("button", { name: "Add task" }).click(); await expect(page.getByText("Bring extension cord")).toBeVisible(); },
  "supply-board": async (page) => { await page.getByLabel("What is needed or offered?").fill("Two folding tables"); await page.getByRole("button", { name: "Post item" }).click(); await expect(page.getByText("Two folding tables")).toBeVisible(); },
  "roll-call": async (page) => { await page.getByRole("button", { name: "Create event" }).click(); await page.getByLabel("Event title").fill("Courtyard supper"); await page.getByRole("button", { name: "Save event" }).click(); await expect(page.getByText("Courtyard supper")).toBeVisible(); },
  "quick-poll": async (page) => { await page.getByRole("button", { name: "Add a crossing guard" }).click(); await expect(page.getByText(/1 vote/)).toBeVisible(); },
};
```

Run the four filtered Playwright tests and confirm they fail on missing labels/actions.

- [ ] **Step 2: Implement Tasks**

Rename the visible product to Tasks while keeping the `checklist` directory. Seed stable keys such as `tasks/seed-welcome`, store `{ text, created_at, added_by_id, assigned_to_id, completed }`, and support add, assign-to-me, and completion toggles. Keep drafts on failure and render all user content with `textContent`.

- [ ] **Step 3: Implement Needs & Offers**

Rename Supply Board, use `items/<id>`, seed both kinds, and store `{ kind: "need"|"offer", text, created_at, added_by_id, resolved_by_id }`. Present two visually distinct columns on desktop and stacked sections on phone. The primary flow posts an item; the secondary action marks it resolved.

- [ ] **Step 4: Implement Events**

Replace Roll Call’s surface with event creation and RSVP while retaining its safe ID-segment/profile-resolution helpers. Store events at `events/<id>` with `{ title, starts_at, place, created_by_id }` and RSVPs at `rsvps/<event-id>/<profile-id>` with `{ attending, at }`. Seed three upcoming events using relative dates calculated once during initialization.

- [ ] **Step 5: Implement Decisions**

Rename Quick Poll, preserve its per-person vote coordinates, and present the current proposal as a decision card with accessible result bars. Store the current proposal at `proposals/current` and votes at `votes/<proposal-id>/<profile-id>`. Seed “How should we make the school crossing safer?” with three choices.

- [ ] **Step 6: Verify RED→GREEN, copy tokens exactly, and commit**

Run:

```bash
for app in checklist supply-board roll-call quick-poll; do cmp fixtures/apps/_shared/tokens.css "fixtures/apps/$app/tokens.css"; done
node scripts/apps/miniapp-contracts.mjs
npx playwright test --config scripts/apps/playwright.config.mjs --grep 'checklist|supply-board|roll-call|quick-poll'
```

Expected: all four flows pass at phone and desktop widths. Commit only these fixture directories and their focused tests:

```bash
git add fixtures/apps/checklist fixtures/apps/supply-board fixtures/apps/roll-call fixtures/apps/quick-poll scripts/apps
git commit -m "feat(apps): polish community coordination tools"
```

### Task 3: Build Chat and Dispatches

**Files:**
- Create: `fixtures/apps/chat/{riot-app.json,index.html,tokens.css,style.css,app.js}`
- Create: `fixtures/apps/dispatches/{riot-app.json,index.html,tokens.css,style.css,app.js}`
- Test: `scripts/apps/miniapp-contracts.mjs`
- Test: `scripts/apps/miniapp-browser.spec.mjs`

- [ ] **Step 1: Add failing Chat and Dispatches flows**

Append `["chat", "Chat"]` and `["dispatches", "Dispatches"]` to the source contract’s `APPS` list, then add:

```js
chat: async (page) => {
  await page.getByLabel("Message").fill("I can bring tea.");
  await page.getByRole("button", { name: "Send" }).click();
  await expect(page.getByText("I can bring tea.")).toBeVisible();
},
dispatches: async (page) => {
  await page.getByRole("button", { name: "Write a dispatch" }).click();
  await page.getByLabel("Title").fill("Garden gate repaired");
  await page.getByLabel("Dispatch").fill("The east entrance is open again.");
  await page.getByRole("button", { name: "Publish" }).click();
  await expect(page.getByText("Garden gate repaired")).toBeVisible();
},
```

Run filtered tests and confirm both fail because the directories are absent.

- [ ] **Step 2: Implement Chat**

Use append-only `messages/<created-at>-<random>` records shaped as `{ text, created_at, author_id }`. Seed a short conversation between three stable demo IDs, resolve names at render time, group consecutive messages visually, keep the composer pinned without covering content, and scroll only when the reader was already near the bottom.

- [ ] **Step 3: Implement Dispatches**

Use append-only `posts/<created-at>-<random>` records shaped as `{ title, body, summary, created_at, author_id }`. Seed three editorially realistic posts. Provide list and detail states plus an in-context composer; publishing returns to the new post. Bound title/body lengths in the UI and retain both fields on failure.

- [ ] **Step 4: Verify and commit**

Run contracts, filtered Playwright tests, exact token comparisons, and narrow-width overflow checks. Expected: both primary flows pass in both viewport projects.

```bash
git add fixtures/apps/chat fixtures/apps/dispatches scripts/apps
git commit -m "feat(apps): add chat and dispatches"
```

### Task 4: Build Wiki and Photo Wall

**Files:**
- Create: `fixtures/apps/wiki/{riot-app.json,index.html,tokens.css,style.css,app.js}`
- Create: `fixtures/apps/photo-wall/{riot-app.json,index.html,tokens.css,style.css,app.js}`
- Create: `fixtures/apps/photo-wall/{courtyard.svg,tool-library.svg,street-feast.svg}`
- Test: `scripts/apps/miniapp-contracts.mjs`
- Test: `scripts/apps/miniapp-browser.spec.mjs`

- [ ] **Step 1: Add failing Wiki and Photo Wall flows**

Append `["wiki", "Wiki"]` and `["photo-wall", "Photo Wall"]` to the source contract’s `APPS` list, making the committed default contract cover all eight apps. Then add:

```js
wiki: async (page) => {
  await page.getByRole("link", { name: "Meeting guide" }).click();
  await page.getByRole("button", { name: "Edit page" }).click();
  await page.getByLabel("Page text").fill("Meet by the blue gate at 18:00.");
  await page.getByRole("button", { name: "Save page" }).click();
  await expect(page.getByText("Meet by the blue gate at 18:00.")).toBeVisible();
},
"photo-wall": async (page) => {
  await page.getByLabel("Caption").fill("Fresh paint on the library shelves");
  await page.setInputFiles('input[type="file"]', "fixtures/apps/photo-wall/courtyard.svg");
  await page.getByRole("button", { name: "Share photo" }).click();
  await expect(page.getByText("Fresh paint on the library shelves")).toBeVisible();
},
```

Run filtered tests and confirm both fail because the directories are absent.

- [ ] **Step 2: Implement Wiki**

Use stable `pages/<slug>` records shaped as `{ title, body, updated_at, updated_by_id }`. Normalize slugs to lowercase ASCII/digits/hyphens, seed four useful pages, show a list/detail layout on desktop and drill-in navigation on phone, and expose plain-text editing with save/error feedback.

- [ ] **Step 3: Implement Photo Wall**

Use `photos/<created-at>-<random>` records shaped as `{ caption, data_url, created_at, author_id }`. Seed bundled local SVG scenes as data URLs. For uploads: decode into an `Image`, draw to canvas with longest edge at most 1280px, encode JPEG starting at quality 0.82, lower quality until the data URL is at most 350 KiB, and refuse larger results with “That photo is still too large — choose a smaller one.” Show a preview before saving and preserve the caption after failure.

- [ ] **Step 4: Verify and commit**

Run contracts, both viewport projects, the upload-size rejection scenario, exact token comparisons, and CSP-safe source checks. Expected: wiki edit and photo upload pass without network calls or HTML interpolation.

```bash
git add fixtures/apps/wiki fixtures/apps/photo-wall scripts/apps
git commit -m "feat(apps): add wiki and photo wall"
```

### Task 5: Pack and embed the exact eight-app catalog

**Files:**
- Modify: `crates/riot-core/src/apps/starter.rs`
- Modify: `crates/riot-core/tests/apps_starter.rs`
- Modify: `crates/riot-core/examples/pack_starter.rs` only if a required resource extension is unsupported
- Create/Modify: `fixtures/apps/*.manifest.cbor`
- Create/Modify: `fixtures/apps/*.bundle.cbor`
- Modify: `fixtures/demo/riverside/content.json` only if its pinned Tasks/Checklist ID changes
- Modify: `crates/riot-core/tests/demo_fixture_drift.rs` only if the fixture’s expected app identity changes

- [ ] **Step 1: Write the failing eight-app catalog test**

Set the expected names in catalog order:

```rust
const EXPECTED_STARTERS: &[&str] = &[
    "Tasks",
    "Needs & Offers",
    "Events",
    "Decisions",
    "Chat",
    "Dispatches",
    "Wiki",
    "Photo Wall",
];

#[test]
fn shipped_catalog_is_the_approved_community_suite() {
    let apps = verify_starter_catalog(STARTER_CATALOG);
    assert_eq!(apps.len(), EXPECTED_STARTERS.len());
    assert_eq!(apps.iter().map(|a| a.manifest.name.as_str()).collect::<Vec<_>>(), EXPECTED_STARTERS);
}
```

Generalize the drift test to loop over `STARTER_APPS` rather than rebuilding only Checklist. Run `cargo test -p riot-core --test apps_starter --locked` and confirm RED on names/count/artifact drift.

- [ ] **Step 2: Update the catalog source list and embedded pairs**

Use this exact order:

```rust
pub const STARTER_APPS: &[&str] = &[
    "checklist",
    "supply-board",
    "roll-call",
    "quick-poll",
    "chat",
    "dispatches",
    "wiki",
    "photo-wall",
];
```

Add matching `starter_app!` pairs. Do not introduce trust shortcuts or keys.

- [ ] **Step 3: Repack canonical artifacts and pin deliberate IDs**

Run:

```bash
scripts/apps/repack-starter.sh
cargo test -p riot-core --test apps_starter --locked
```

Copy the eight emitted IDs into the pinned-ID table. If Tasks changes the Riverside fixture ID, update the full exact ID there and regenerate the fixture through its existing packer; never truncate or handwave an ID.

- [ ] **Step 4: Run catalog and fixture verification**

```bash
cargo test -p riot-core --test apps_starter --locked
cargo test -p riot-core --test demo_fixture_drift --locked
cargo test -p riot-core apps::starter --locked
cargo fmt --all -- --check
```

Expected: all catalog pairs verify, all source/artifact bytes match, all IDs are distinct and pinned, and the demo fixture remains exact.

- [ ] **Step 5: Commit catalog integration**

```bash
git add crates/riot-core/src/apps/starter.rs crates/riot-core/tests/apps_starter.rs crates/riot-core/examples/pack_starter.rs fixtures/apps fixtures/demo/riverside crates/riot-core/tests/demo_fixture_drift.rs
git commit -m "feat(apps): ship eight-app community catalog"
```

Stage only paths actually changed; preserve unrelated fixture/core work.

### Task 6: Perform blocking visual and accessibility review

**Files:**
- Modify: affected `fixtures/apps/*/{index.html,tokens.css,style.css,app.js}` based on findings
- Create: `docs/quality/2026-07-12-miniapp-visual-review.md`
- Test: `scripts/apps/miniapp-browser.spec.mjs`

- [ ] **Step 1: Capture the critical-state matrix**

Run Playwright for phone and desktop across seeded, composer/editor, post-action, empty, and forced-error states. Store temporary screenshots under `/tmp/riot-miniapp-review/<app>/<viewport>/<state>.png`; do not commit screenshot binaries.

- [ ] **Step 2: Review every screenshot visually**

For each app record PASS/FAIL for hierarchy, distinctive identity, typography, contrast, spacing, overflow, touch targets, keyboard reachability, error clarity, and long-content behavior. Write the results as a table in the quality document with exact filenames and concrete findings.

- [ ] **Step 3: Fix every blocking finding and recapture**

Use the app’s existing accent and shared tokens. Do not solve issues by shrinking body text below 16px, hiding overflowed content, removing focus styles, or replacing realistic seed content with shorter placeholders. Recapture only affected states and update their result to PASS with the fix noted.

- [ ] **Step 4: Run automated accessibility/responsive checks**

```bash
node scripts/apps/miniapp-contracts.mjs
npx playwright test --config scripts/apps/playwright.config.mjs
```

Expected: all eight apps pass both viewports, critical interactions, 44px targets, accessible labels, reduced-motion CSS, safe text rendering, and horizontal-overflow assertions.

- [ ] **Step 5: Commit visual-review fixes and evidence**

```bash
git add fixtures/apps scripts/apps docs/quality/2026-07-12-miniapp-visual-review.md
git commit -m "fix(apps): pass miniapp visual review"
```

### Task 7: Verify runtime integration and demo readiness

**Files:**
- Modify: tests only when an existing assertion must deliberately accept the eight-app catalog
- Create: `docs/product/miniapp-demo-script.md`

- [ ] **Step 1: Write the demo script before final verification**

Document a five-minute path: create/open a profile, open the Apps directory, approve two contrasting apps, exercise one write in each, reopen them, and show all eight available. Include a fallback path using already-seeded content if WebView image selection or nearby transport is unavailable on venue hardware.

- [ ] **Step 2: Run source, browser, core, FFI, and binding gates**

```bash
node scripts/apps/miniapp-contracts.mjs
npx playwright test --config scripts/apps/playwright.config.mjs
cargo test -p riot-core --test apps_starter --locked
cargo test -p riot-core --locked
cargo test -p riot-ffi --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run --locked --package xtask -- generate-bindings
scripts/conference/validate-contracts.sh
```

Expected: every command exits 0. If concurrent source changes make generated bindings stale, rebuild through the repository scripts; do not edit generated Swift/Kotlin manually.

- [ ] **Step 3: Run native host gates**

```bash
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' -derivedDataPath build/ios-derived
xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS -destination 'platform=macOS,arch=arm64' -derivedDataPath build/macos-derived
(cd apps/android && ./gradlew test)
```

Expected: all suites exit 0. Record unrelated pre-existing failures precisely; do not describe the release as ready while a failure can affect catalog display, trust, launch, bridge writes, persistence, or visual rendering.

- [ ] **Step 4: Exercise the real packed app smoke flow**

Open each trusted built-in through the native host, confirm seed content, perform its primary action, close it, reopen it, and confirm persistence. At minimum automate the existing XCUITest path for Tasks plus one browser/host smoke per other app.

- [ ] **Step 5: Final review and commit**

Inspect `git diff --check`, verify no secrets or generated caches are staged, confirm every manifest uses the approved permissions, and commit the demo script and any deliberate test expectation updates:

```bash
git add docs/product/miniapp-demo-script.md apps/ios/RiotTests apps/android/app/src/test crates/riot-core/tests
git commit -m "test(apps): verify community suite demo"
```
