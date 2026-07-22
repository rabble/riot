# Public Marketing Clarity Correction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove visual status clutter, inaccurate/fear-inducing generic exposure copy, and internal LLM process metrics from Riot's public marketing pages while keeping public/private product boundaries accurate and calm.

**Architecture:** Keep the dependency-free nine-route static site and exact `marketing/public/` mirrors. Extend the existing marketing contract before HTML changes, add one tested production verifier using Node built-ins and the pinned Playwright package, then capture refreshed editorial/visual evidence before deploying the exact committed artifact.

**Tech Stack:** Static HTML/CSS, Node.js assertions and test runner, Playwright Chromium, Cloudflare Wrangler.

---

### Task 1: Specify the clarity correction test-first

**Files:**
- Modify: `scripts/marketing/protocol-page-contracts.mjs:357-403`

- [ ] **Step 1: Add scoped failing homepage assertions**

Immediately before the Why Riot status contract, add:

```js
const comparisonTable = block(home, /<table\b[^>]*class="contrast"[^>]*>[\s\S]*?<\/table>/i, "homepage comparison table");
assert.doesNotMatch(comparisonTable, /<span\b[^>]*class="[^"]*\bchip\b/i, "homepage comparison must not contain status chips");
assert.match(home, /<section\b[^>]*id="status"/i, "homepage detailed status section must remain");
assert.match(home, /href="\/protocols\/"/i, "homepage protocol detail link must remain");

for (const pattern of [
  /<h3>What Riot does not hide<\/h3>/i,
  /<ul\b[^>]*class="notlist"/i,
  /<p\b[^>]*class="boundary-note"[^>]*>\s*Separate per-community keys/i,
  /<div\b[^>]*class="evidence-box"/i,
  /<div\b[^>]*class="evidence-stats"/i,
  /<div\b[^>]*class="estat"/i,
  /Each research pass/i,
  /adversarial reviewers/i,
  /Research(?:&nbsp;|\s)+passes/i,
  /Sources(?:&nbsp;|\s)+fetched/i,
  /Claims(?:&nbsp;|\s)+verified/i,
]) assert.doesNotMatch(home, pattern, `homepage retains removed internal/fear copy: ${pattern}`);

const evidence = block(home, /<section\b[^>]*id="evidence"[^>]*>[\s\S]*?<\/section>/i, "homepage field history");
assert.match(evidence, /Grounded in the field/i);
assert.match(evidence, /Occupy Sandy[\s\S]*TXTMob[\s\S]*Verificado 19S/i);
```

- [ ] **Step 2: Replace the defensive Why Riot and Privacy expectations**

Replace the old phrase loop and Privacy marker block with:

```js
const boundaries = block(whyRiot, /<section\b[^>]*id="boundaries"[^>]*>[\s\S]*?<\/section>/i, "Why Riot boundaries");
for (const phrase of ["Current public Newswires", "publishing and collaboration", "read, copied, and carried", "Private encrypted groups", "not part of today's prototype"]) {
  assert.ok(boundaries.toLowerCase().includes(phrase.toLowerCase()), `Why Riot missing calm boundary: ${phrase}`);
}
for (const phrase of ["IP addresses", "timing", "radio presence", "device labels", "proximity", "behavioral correlation", "compromised devices", "fabricated gateway"]) {
  assert.doesNotMatch(boundaries, new RegExp(phrase, "i"), `Why Riot retains speculative inventory: ${phrase}`);
}
for (const href of ["/privacy/", "/protocols/", "https://signal.org/"]) assert.ok(boundaries.includes(`href="${href}"`), `Why Riot boundaries must link ${href}`);

const privacyMarkers = ["Public communities", "Data communities can hold", "Private conversation", "This website", "Where to go next"];
let privacyCursor = -1;
for (const marker of privacyMarkers) { const at = privacy.indexOf(marker); assert.ok(at > privacyCursor, `Privacy section missing or out of order: ${marker}`); privacyCursor = at; }
for (const phrase of ["public publishing spaces", "read, copied, and carried", "participant devices", "one company's account or database", "does not yet ship private encrypted groups", "no analytics", "sets no cookies"]) {
  assert.ok(privacy.toLowerCase().includes(phrase.toLowerCase()), `Privacy missing affirmative boundary: ${phrase}`);
}
for (const phrase of ["IP addresses", "timing", "radio presence", "device labels", "proximity", "behavioral correlation", "compromised devices", "fabricated gateway"]) {
  assert.doesNotMatch(privacy, new RegExp(phrase, "i"), `Privacy retains speculative inventory: ${phrase}`);
}
```

Also assert the `/privacy/` entry between `## Routes` and the next heading in `marketing/README.md` contains `public publishing`, `participant-held data`, `private conversation`, and `website data`, and excludes `device` and `metadata`.

- [ ] **Step 3: Run RED**

Run `npm run test:marketing`.

Expected: FAIL first at `homepage comparison must not contain status chips`, proving the new assertions exercise the deployed markup rather than a syntax or fixture error.

- [ ] **Step 4: Commit the failing contract**

```sh
git add scripts/marketing/protocol-page-contracts.mjs
git commit -m "test(marketing): specify calmer public pages"
```

### Task 2: Remove clutter and reframe the public boundary

**Files:**
- Modify: `marketing/index.html`
- Modify: `marketing/public/index.html`
- Modify: `marketing/why-riot/index.html`
- Modify: `marketing/public/why-riot/index.html`
- Modify: `marketing/privacy/index.html`
- Modify: `marketing/public/privacy/index.html`
- Modify: `marketing/README.md`

- [ ] **Step 1: Clean the homepage**

Delete the five `span.chip` elements from `table.contrast`; delete the complete `h3`/`ul.notlist`/specific `p.boundary-note` block beginning `What Riot does not hide`; delete the complete `div.evidence-box`. Remove only the now-unused `.notlist`, `.evidence-box`, `.evidence-stats`, and `.estat` CSS. Retain `section#evidence`, its field-history introduction, `section#status`, all claim-specific status labels outside the comparison table, and the `/protocols/` link.

- [ ] **Step 2: Replace Why Riot's boundary panel**

Use this exact content inside the existing `section#boundaries` wrapper:

```html
<div class="wrap boundary-panel">
  <p class="eyebrow">Public by purpose</p>
  <h2>Share openly. Choose privacy deliberately.</h2>
  <p>Current public Newswires are places for publishing and collaboration. Posts are intended to be read, copied, and carried.</p>
  <p>Private encrypted groups are a separate future direction, not part of today's prototype. For a private conversation today, use a purpose-built private messenger such as <a href="https://signal.org/" rel="noopener">Signal</a>.</p>
  <p class="secret-note">Read Riot's short <a href="/privacy/">privacy boundary</a> or the <a href="/protocols/">technical details</a>.</p>
</div>
```

- [ ] **Step 3: Replace Privacy's main content with affirmative copy**

Keep the existing navigation, footer, canonical link, and section classes. Use hero H1 `Public spaces, participant-held data.` and these ordered sections:

```html
<section class="plain" id="public"><div class="wrap">
  <p class="eyebrow">Public communities</p><h2>Public communities</h2>
  <p>Current Newswires are public publishing spaces. Posts are intended to be read, copied, and carried by the people and communities they reach.</p>
</div></section>
<section class="local" id="local-first"><div class="wrap">
  <p class="eyebrow">Participant-held data</p><h2>Data communities can hold</h2>
  <p>Community state can live on participant devices instead of existing only inside one company's account or database. That reduces mandatory centralized collection and makes it easier for a community to move between hosts and transports.</p>
</div></section>
<section class="plain" id="private"><div class="wrap">
  <p class="eyebrow">A separate mode</p><h2>Private conversation</h2>
  <p>Riot does not yet ship private encrypted groups. For a private conversation today, use a purpose-built private messenger such as <a href="https://signal.org/" rel="noopener">Signal</a>.</p>
</div></section>
```

Retain the existing `This website` no-analytics/static-page paragraph but replace its hosting-warning fact box with: `Observed locally and at both production origins: these pages set no cookies and load no third-party resources. Independently hosted copies have their own operator and configuration.` Keep `Where to go next`, shorten it to purpose and technical-detail links, and retain Signal in the private section only.

- [ ] **Step 4: Update README and exact mirrors**

Set the `/privacy/` route description to `concise public-publishing, participant-held-data, private-conversation, and website-data boundaries.` Copy each changed source page byte-for-byte to its `marketing/public/` peer using the same patch content, then run all three `cmp` commands from the existing plan.

- [ ] **Step 5: Run GREEN and commit**

Run `npm run test:marketing` and `npm run test:web:unit`; both must pass. Then:

```sh
git add marketing/index.html marketing/public/index.html marketing/why-riot/index.html marketing/public/why-riot/index.html marketing/privacy/index.html marketing/public/privacy/index.html marketing/README.md
git commit -m "fix(marketing): remove fear and process theater"
```

### Task 3: Add the reproducible production verifier

**Files:**
- Create: `scripts/marketing/test/verify-live.test.mjs`
- Create: `scripts/marketing/verify-live.mjs`
- Modify: `package.json`

- [ ] **Step 1: Write failing verifier tests**

Create Node tests around an exported `verifyOrigin({ origin, routes, browserFactory })` using loopback HTTP fixtures. The passing fixture serves exact bytes, direct 200s, no cookies, no storage, and a direct 404. Separate tests must reject a redirect, byte mismatch, `Set-Cookie`, nonempty browser cookie/storage, and an off-origin request. Run `node --test scripts/marketing/test/verify-live.test.mjs`; expected FAIL because the module does not exist.

- [ ] **Step 2: Implement the minimal verifier**

Create `verify-live.mjs` with the exact nine-route map from the design. Use `fetch(..., { redirect: "manual" })`, `readFile`, `createHash("sha256")`, `timingSafeEqual` after equal-length checks, and Playwright Chromium. Export `verifyOrigin`; on direct execution verify both production origins and print one JSON object containing origin, route, status, expected/local hash, live hash, headers, cookie/storage/request-origin results, and missing-route result. Exit nonzero on any mismatch.

- [ ] **Step 3: Expose and verify the command**

Add `"verify:marketing:live": "node scripts/marketing/verify-live.mjs"` and change `test:marketing` to `node --test scripts/marketing/test/*.test.mjs && node scripts/marketing/protocol-page-contracts.mjs`. Run `npm run test:marketing`; expected PASS.

- [ ] **Step 4: Commit**

```sh
git add scripts/marketing/verify-live.mjs scripts/marketing/test/verify-live.test.mjs package.json
git commit -m "test(marketing): verify deployed route identity"
```

### Task 4: Visual/editorial review, commit, deploy, and verify

**Files:**
- Modify: `docs/marketing/2026-07-22-human-capacity-implementation-review.md`

- [ ] **Step 1: Capture and inspect desktop/mobile pages**

Serve `marketing/public/` on loopback. Capture `/`, `/why-riot/`, and `/privacy/` at 1456×900 and 390×844. Assert no horizontal overflow. Inspect that comparison rows have no empty badge gaps, the field-history section flows without the removed box, Why Riot's boundary is compact, and Privacy leads with participant value rather than threat language.

- [ ] **Step 2: Refresh editorial evidence**

Run three fresh isolated Why Riot first-read reviews with revision 20's Q4 rubric and one fresh semantic claim audit. Require all four PASS and store new file hashes/session IDs in the implementation review.

- [ ] **Step 3: Run the full predeploy gate and commit the implementation evidence**

Run `npm run test:web:unit`, `npm run test:marketing`, `git diff --check`, exact mirror `cmp` checks, and confirm a clean worktree after updating the report. Commit the report, record the resulting implementation commit with `git rev-parse HEAD`, and do not alter files before deployment.

- [ ] **Step 4: Push, deploy, and run the live verifier**

Push `design/solnit-resilience-page` without force. From `marketing/`, run `CI=1 WRANGLER_SEND_METRICS=false npx wrangler deploy`. From the repo root, run `npm run verify:marketing:live`; require PASS for both origins and all routes.

- [ ] **Step 5: Record and push deployment evidence**

Append the Cloudflare version, immutable implementation commit, exact command, per-route expected/live hashes, direct statuses, header/cookie/storage/request-origin results, and 404 result to the implementation review. Commit as `docs(marketing): record clarity deployment evidence`, push without force, and confirm local/remote equality and a clean worktree.
