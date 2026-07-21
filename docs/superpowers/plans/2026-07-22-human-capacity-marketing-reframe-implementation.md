# Riot Human-Capacity Marketing Reframe Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reframe Riot's existing marketing site around ordinary collective life and human capacity, with `/why-riot/` as the canonical story and `/privacy/` as a compact factual reference.

**Architecture:** Keep the dependency-free nine-route static site and its byte-identical `marketing/public/` mirror. Expand the existing marketing contract before changing copy, then make bounded HTML/documentation edits, add the contract to the existing Node CI job, and finish with browser, visual, accessibility, and independent editorial evidence. No route, runtime dependency, protocol behavior, or deployment configuration changes.

**Tech Stack:** Static HTML/CSS/inline SVG, Node.js assertions, Playwright Chromium, npm scripts, GitHub Actions.

---

## File map

- `scripts/marketing/protocol-page-contracts.mjs`: single deterministic static and browser contract for route inventory, navigation, mirrors, claims, resource safety, canonical links, page structure, status markup, and local HTTP behavior.
- `marketing/index.html` and `marketing/public/index.html`: distinct homepage hero plus site-wide navigation/footer and bounded claim edits; preserve the current desktop app-screen composition.
- `marketing/why-riot/index.html` and its public mirror: canonical human-capacity narrative, inline ordinary-life illustration, exact status markup, compact mechanism/boundary sections, Solnit attribution, and participation links.
- `marketing/privacy/index.html` and its public mirror: concise public-first privacy reference and canonical link.
- The remaining six source pages and public mirrors: navigation/footer normalization and bounded site-wide claim corrections only.
- `marketing/public/sitemap.xml` and `marketing/README.md`: exact nine-route inventory and accurate source/mirror/preview documentation.
- `README.md` and `docs/product/product-brief.md`: mark private groups as direction rather than shipped and replace seizure-resistance absolutes with bounded mechanism/prerequisite language.
- `package.json` and `.github/workflows/ci.yml`: expose and run the blocking marketing contract, including Chromium provisioning.
- `docs/marketing/2026-07-22-human-capacity-implementation-review.md`: reproducible visual, HTTP, contrast, forced-colors, first-read, and semantic-audit evidence.

### Task 1: Replace obsolete marketing expectations with the approved contract

**Files:**
- Modify: `scripts/marketing/protocol-page-contracts.mjs`
- Modify: `package.json`
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Add route-normalization and bounded-element helpers**

Add helpers that normalize local routes through `new URL(href, "https://local.invalid")`, strip query/fragment, collapse terminal `/index.html`, enforce trailing slashes, extract only `.sitenav-links` and `<footer>` local hrefs, compare both ordered arrays and sets, and extract bounded `article[data-capability]`, `li[data-carry-path]`, and `span.chip[data-status]` elements. Add safe resource/value parsing that rejects malformed percent escapes, controls, backslashes, active data URLs, unsafe schemes, malformed `srcset`, event handlers, forms, `<base>`, `srcdoc`, remote runtime resources, and unsafe decoded SVG favicons.

- [ ] **Step 2: Replace the four legacy assertion regions named by the design**

Retain the screenshot, builder, source-ledger, paired-story, guide, and protocol-page assertions outside the named regions. Replace the legacy homepage framing block, footer loop, top-nav loop, and Why Riot audience-depth block with assertions for:

```js
const allSitePaths = ["/", "/why-riot/", "/guide/", "/about/", "/privacy/", "/open-source/", "/community/", "/releases/", "/protocols/"];
const primaryNavPaths = ["/", "/why-riot/", "/guide/", "/about/", "/open-source/", "/community/", "/releases/", "/protocols/"];
const exactStatusText = {
  prototype: "Available in the prototype",
  local: "Tested locally",
  development: "In development",
  direction: "Direction, not shipped",
};
```

Assert exact primary-nav order/set on all source pages and mirrors, exact nine-route footer sets including self-links, exact nine-path sitemap and README inventories, no `/resilience/` directory/link/sitemap route, distinct homepage and Why Riot H1s, origin-relative canonical links, the Why Riot section/status contracts, Privacy's public-first hierarchy, the code-native illustration accessibility contract, and exact participation/Solnit links. The homepage assertion must require an ordinary-life use case and copy establishing that Riot matters before shutdown or disruption, rather than merely removing unsafe absolute language.

- [ ] **Step 3: Add the finite site-wide claim and resource-safety audit**

Read all nine source pages plus `README.md` and `docs/product/product-brief.md`. Apply every forbidden-claim pattern and static-content predicate from the approved design. Enumerate resource-bearing attributes and CSS URLs with an allowlist, decode and inspect SVG favicons, parse each `srcset` candidate, require `rel="noopener"` on blank-target links, and keep ordinary external HTTP(S) citations allowed.

- [ ] **Step 4: Add authoritative loopback browser checks**

Start a Node HTTP server rooted at `marketing/public/`, launch Chromium, and visit all nine routes in a fresh context. Before navigation attach request/response listeners; after navigation scroll the complete page and wait for idle. Require HTTP 200 with no redirect chain for each canonical route. Request `/resilience/` with redirects disabled and require a direct 404 response with no `Location` header, proving the absent route is not a redirect. Assert an empty cookie jar before and after, no `Set-Cookie`, no resource request outside the loopback origin, safe resolved anchor protocols/fragments, and safe resolved DOM resource URLs. Always close page, context, browser, and server in `finally` blocks.

For every route, capture every request URL, every response's ordered header entries, and every normalized `performance.getEntriesByType("resource")` entry. Re-run the static predicates against the homepage script text for `fetch(`, `sendBeacon(`, `XMLHttpRequest`, `WebSocket(`, `localStorage`, `sessionStorage`, and `document.cookie`; in Chromium also require `document.cookie === ""` and the context cookie jar to remain empty. Serialize the sorted observations to `/tmp/visual-review/riot-human-capacity/browser-evidence.json`; Task 4 records its SHA-256 and full JSON in the committed report. Create the parent directory when absent so the CI contract does not depend on a prior visual-review run.

- [ ] **Step 5: Add the npm and CI entry points**

Add the exact script:

```json
"test:marketing": "node scripts/marketing/protocol-page-contracts.mjs"
```

In the existing `web` job, provision only Chromium after `npm ci --ignore-scripts`, then run `npm run test:marketing` as a separate blocking step immediately after `npm run test:web:unit`.

- [ ] **Step 6: Run the new contract and prove RED before HTML implementation**

Run:

```sh
npm run test:marketing
```

Expected: FAIL on the old homepage H1/navigation/footer/Why Riot/Privacy/status/doc claims (not on a syntax error or browser setup failure). Record the first relevant assertion in the commit message/body or implementation notes.

- [ ] **Step 7: Commit the test-first contract**

```sh
git add scripts/marketing/protocol-page-contracts.mjs package.json .github/workflows/ci.yml
git commit -m "test(marketing): specify human-capacity reframe"
```

### Task 2: Reframe the homepage, Why Riot, and Privacy pages

**Files:**
- Modify: `marketing/index.html`
- Modify: `marketing/public/index.html`
- Modify: `marketing/why-riot/index.html`
- Modify: `marketing/public/why-riot/index.html`
- Modify: `marketing/privacy/index.html`
- Modify: `marketing/public/privacy/index.html`

- [ ] **Step 1: Make the homepage hero distinct without disturbing current screenshots**

Set the hero H1 to `Community tools that travel with people.` and its immediate support to `Riot is a home for public conversation, community decisions, shared tools, and collective memory—carried by the people who make it matter.` Add a prominent `/why-riot/` action labeled `Why Riot exists`. Preserve the current `win-frame` desktop screenshots and honest Prototype label. Add positive copy showing use in ordinary community life before disruption—for example, a festival rota, neighborhood publication, shared meal, or cooperative decision—so shutdown is not the page's sole reason for Riot. Qualify any absolute preservation/availability prose while keeping the existing product evidence and paired-story sections.

- [ ] **Step 2: Build the Why Riot social-purpose page**

Replace the current three-audience technical essay with ordered semantic sections:

1. Hero H1 `People are the infrastructure.` followed immediately by the approved meals/meetings/stories/decisions/celebrations/care/shared-work thesis.
2. `A community is something people do`, with an original inline SVG/collage depicting ordinary cooking, gardening, meeting, publishing, music, and shared work; use adjacent prose plus `aria-hidden="true"`.
3. `<section id="tools">` with exactly four `article[data-capability="publish|meet|coordinate|carry"]` cards and exact status chips/phrases from the design. Carry has exactly six `li[data-carry-path]` rows and no blanket card chip.
4. `The future is a practice`, establishing ordinary use before difficult conditions.
5. A short `More than one path` mechanism section with participant-held data, signed-record limits, locally useful already-held state, multiple bounded paths, replaceable hosts, the labeled aspiration `A community should be able to leave a provider without leaving one another.`, and `/protocols/` detail link.
6. One compact honest-boundaries panel with public plaintext/readable/copyable content, incomplete privacy/transports, device/network/host risks, signature limits, prototype limits, explicit `/privacy/` and `/protocols/` detail links, and the Signal threat-model recommendation.
7. `Build it with us`, linking `/guide/`, `/community/`, `/releases/`, and the source repository, followed by a small non-endorsement Solnit lineage note linking the publisher page.

Add `<link rel="canonical" href="/why-riot/">`, keep meaning/navigation script-free, preserve skip link/landmarks/focus/reduced-motion behavior, and make status labels visually subordinate.

- [ ] **Step 3: Compact Privacy into a citeable factual reference**

Add `<link rel="canonical" href="/privacy/">`. Replace the defensive hero and repeated manifesto/tables with four ordered sections: `Public means public`; `What local-first changes—and what it does not`; `This website`; and `Where to go next`. Include the exact public Newswire plaintext/readable/copyable/no-confidential-boundary and no-private-groups facts; local custody plus metadata/radio/device/copy/pseudonymity/gateway risks; static-code no-tracking disclosure bounded by Cloudflare's edge; and Why Riot/Protocols/Signal links with the same threat-model caveat.

- [ ] **Step 4: Keep source and public versions byte-identical**

Apply the same patch to each source/mirror pair, then verify:

```sh
cmp marketing/index.html marketing/public/index.html
cmp marketing/why-riot/index.html marketing/public/why-riot/index.html
cmp marketing/privacy/index.html marketing/public/privacy/index.html
```

Expected: all commands exit 0.

- [ ] **Step 5: Run the focused contract**

```sh
npm run test:marketing
```

Expected: remaining failures are limited to site-wide nav/footer/doc inventory/claim work in Task 3; the three changed-page structure, copy, canonical, status, accessibility, and mirror assertions pass.

- [ ] **Step 6: Commit the core narrative change**

```sh
git add marketing/index.html marketing/public/index.html marketing/why-riot/index.html marketing/public/why-riot/index.html marketing/privacy/index.html marketing/public/privacy/index.html
git commit -m "feat(marketing): put people at the center of Riot"
```

### Task 3: Reconcile navigation, inventories, and product claims site-wide

**Files:**
- Modify: `marketing/guide/index.html`
- Modify: `marketing/public/guide/index.html`
- Modify: `marketing/about/index.html`
- Modify: `marketing/public/about/index.html`
- Modify: `marketing/open-source/index.html`
- Modify: `marketing/public/open-source/index.html`
- Modify: `marketing/community/index.html`
- Modify: `marketing/public/community/index.html`
- Modify: `marketing/releases/index.html`
- Modify: `marketing/public/releases/index.html`
- Modify: `marketing/protocols/index.html`
- Modify: `marketing/public/protocols/index.html`
- Modify: all six files from Task 2 where shared navigation/footer changes remain
- Modify: `marketing/public/sitemap.xml`
- Modify: `marketing/README.md`
- Modify: `README.md`
- Modify: `docs/product/product-brief.md`

- [ ] **Step 1: Normalize all nine primary navigation blocks**

In every source page and mirror, make `.sitenav-links` use this exact order and no Privacy link: Home, Why Riot, Using Riot, About, Open source, Community, Get the app, Protocol field guide. Preserve each appropriate `aria-current="page"`; the separate brand root link remains outside `.sitenav-links`.

- [ ] **Step 2: Normalize all nine footers**

Every footer must contain all nine local routes, including Privacy and its own current route, exactly once as a normalized path set. Keep page-specific Back to top links only if they are fragment-only and do not alter the local-route set.

- [ ] **Step 3: Perform the bounded claim audit**

Search all nine source pages for the finite forbidden patterns and semantic equivalents. Rewrite only unsafe absolutes, naming mechanisms and prerequisites: already-held data on a functioning device; exchange through an actually available compatible path; participant-held copies and replaceable gateways reduce single-provider dependence without guaranteeing a complete reachable copy, publishing, access, persistence, or censorship resistance.

- [ ] **Step 4: Reconcile product documentation**

In both product documents label private encrypted groups `Direction, not shipped`. Replace `no server to raid` / `no server to seize` with the bounded participant-copy and replaceable-gateway explanation plus the explicit absence of survival guarantees. Do not rewrite unrelated architecture/history.

- [ ] **Step 5: Make route documentation exact**

Update `marketing/README.md` so the `## Routes` ordered list is exactly all nine routes and every source, public-mirror, asset, and local-preview inventory also names the nine-route reality. Correct the desktop screenshot description. Keep the deployment instructions intact. Update `marketing/public/sitemap.xml` only if required to preserve its exact nine-route normalized set.

- [ ] **Step 6: Verify all mirrors, claims, and contracts are GREEN**

```sh
npm run test:marketing
npm run test:web:unit
git diff --check
```

Expected: all pass with exit 0.

- [ ] **Step 7: Commit site-wide reconciliation**

```sh
git add marketing README.md docs/product/product-brief.md
git commit -m "docs(marketing): reconcile routes and public claims"
```

### Task 4: Perform visual, accessibility, and independent editorial verification

**Files:**
- Create: `docs/marketing/2026-07-22-human-capacity-implementation-review.md`

- [ ] **Step 1: Read and follow the visual-review skill**

Verify `npx playwright --version`; install Chromium only if unavailable. Serve `marketing/public/` on an available loopback port. Capture `/`, `/why-riot/`, and `/privacy/` at 1456×900 and 390×844 under `/tmp/visual-review/riot-human-capacity/`, plus forced-colors Why Riot/Privacy desktop captures when Chromium supports it.

- [ ] **Step 2: Verify layout and accessibility evidence**

For every standard capture assert `document.documentElement.scrollWidth <= document.documentElement.clientWidth`. Inspect heading order, keyboard focus, reduced motion, navigation wrapping, SVG behavior, and status hierarchy. Walk visible text/control computed colors at both sizes, resolve flat backgrounds, calculate WCAG ratios (4.5:1 normal, 3:1 large/bold), and manually resolve any transparent-background ambiguity.

On `/why-riot/` at 390×844, remove every `<style>` and stylesheet `<link>` after load to simulate unavailable CSS. Capture `why-riot-no-css.png`, assert no horizontal overflow, confirm the inline illustration remains in logical DOM order, and verify its intrinsic markup neither overlaps nor hides the following prose. Record the screenshot hash and result in the report.

- [ ] **Step 3: Record durable browser and screenshot evidence**

Create the review report with exact capture commands, route, viewport, screenshot path, SHA-256, overflow result, no-CSS result, forced-colors support/outcome, computed color pairs and ratios, every request URL, ordered response headers, ordered performance-resource entries for all nine routes, `document.cookie` and browser-context cookie-jar results, and SHA-256 of `marketing/public/why-riot/index.html` plus the browser-evidence JSON itself.

- [ ] **Step 4: Run three isolated first-read reviews**

Use the configured external Codex CLI, whose health check was `ready` during planning, as the explicit isolation mechanism. Run the adapter health command before the gate:

```sh
/Users/rabble/.codex/plugins/cache/metaswarm-marketplace/metaswarm/0.12.0/skills/external-tools/adapters/codex.sh health
```

For each reader role create a fresh mode-0700 temporary directory containing only a byte-for-byte copy of `marketing/public/why-riot/index.html` and that role's exact prompt; do not run from the repository. Invoke a new `codex exec --skip-git-repo-check --sandbox read-only --json -C "$role_dir" "$prompt"` process for each role, preserving its distinct `thread_id` and raw JSONL. Resolve the CLI with `command -v codex`; the absolute adapter path above is only its configured health check, not the execution binary. The prompt explicitly forbids external sources and names the single local HTML input. Do not reuse or resume sessions, expose earlier responses, include the design, or mount the repository. If the adapter health check is not ready, use a fresh in-product reviewer thread with the same single-file input; the verification cannot pass without four genuinely isolated session IDs.

Collect verbatim JSON for Community participant/organizer, Potential partner/institution, and Builder/technical reader. Record session IDs, exact prompts and prompt hashes, verbatim answers, element-by-element orchestrator scoring, and require 4/4, 5/5, and 5/5 respectively with no privacy/disaster/protocol-first primary impression.

- [ ] **Step 5: Run one isolated semantic claim audit**

Create a fourth mode-0700 temporary directory containing only the ordered nine public HTML files, `README.md`, `docs/product/product-brief.md`, and the exact audit prompt from the design. Run a new read-only Codex CLI process from that directory with no repository mount or prior-session context. Record ordered file hashes, prompt/hash, raw JSONL/thread ID, verbatim result JSON, and require `PASS` with zero findings.

- [ ] **Step 6: Fix and re-run any failed verification**

For a failure, write a focused failing assertion when automatable, patch the smallest source/mirror surface, rerun `npm run test:marketing` and `npm run test:web:unit`, and repeat the affected visual/editorial check with a fresh session. Update rather than erase the report's issue-and-resolution history.

- [ ] **Step 7: Run final verification and commit evidence**

```sh
npm run test:web:unit
npm run test:marketing
git diff --check
git status --short
```

Expected: tests and diff check pass; only the intended review report is uncommitted. Then:

```sh
git add docs/marketing/2026-07-22-human-capacity-implementation-review.md
git commit -m "docs(marketing): record human-capacity review evidence"
```

### Task 5: Final adversarial review and handoff

**Files:**
- Review all files changed since `origin/main`

- [ ] **Step 1: Run the repository's request-code-review and verification-before-completion skills**

Check the final diff against every design acceptance criterion, with special attention to status exactness, unsafe claim equivalents, static resource allowlists, source/public mirror identity, first-read scores, and the fact that no `/resilience/` route or deployment mutation exists.

Record `git diff --name-only origin/main...HEAD` and verify it contains only the declared marketing, contract, CI, package, product-document, plan/design, and review-report files. Confirm the current desktop hero `win-frame` screenshot assertions still pass. Because this is a static-site and documentation change with no data/schema migration, rollback is the ordered `git revert` of this branch's implementation commits; no cache, database, route redirect, or external state cleanup is required.

- [ ] **Step 2: Run the final blocking commands from a clean worktree state**

```sh
npm run test:web:unit
npm run test:marketing
git diff --check origin/main...HEAD
git status --short --branch
```

Expected: all commands pass; the branch is clean and only ahead of `origin/main`.

- [ ] **Step 3: Address any review findings test-first and re-review**

Any blocking finding receives a reproducing assertion, minimal source/mirror fix, complete Task 5 verification rerun, and fresh adversarial review. Commit each coherent correction without rewriting history.

- [ ] **Step 4: Report the branch outcome without deploying**

Summarize changed framing, exact routes affected, verification evidence, review result, and remaining deployment scope. Do not deploy, alter DNS/TLS, push, or claim the live site changed.
