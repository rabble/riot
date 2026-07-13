# Protocol Comparison Marketing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish a visually polished `/protocols/` page that accurately compares Riot and Willow with adjacent protocols while keeping Riot's homepage focused on people and communities.

**Architecture:** Keep the marketing site dependency-free and statically deployable through the existing Cloudflare Workers assets configuration. Treat `marketing/` as editable source and `marketing/public/` as a byte-identical deployment mirror. Add one Node contract test that validates route presence, editorial boundaries, source links, accessibility structure, and mirror equality before deployment.

**Tech Stack:** semantic HTML, self-contained CSS, vanilla Node.js assertions, Playwright browser screenshots, Cloudflare Wrangler static assets

---

### Task 1: Pin the publishing contract before markup

**Files:**
- Create: `scripts/marketing/protocol-page-contracts.mjs`

- [ ] Write assertions for the source and public protocol pages, homepage links, required profiles, honest Riot limitations, checked date, semantic landmarks, reduced-motion support, primary-source links, no third-party runtime requests, and byte-identical mirror pairs.
- [ ] Run `node scripts/marketing/protocol-page-contracts.mjs` and verify RED because the `/protocols/` files do not exist.

### Task 2: Build the protocol comparison page

**Files:**
- Create: `marketing/protocols/index.html`
- Create: `marketing/public/protocols/index.html`

- [ ] Build the approved hero, layer taxonomy, situation cards, scrollable orientation matrix, Willow/Riot explainer, concrete checklist data path, standardized protocol profiles, limitations, and primary-source ledger.
- [ ] Use Riot's existing type, color, border, and shadow language while giving the long-form page strong reading rhythm on desktop and mobile.
- [ ] Keep the page self-contained: no JavaScript dependency, analytics, remote fonts, remote imagery, or external runtime fetches.
- [ ] Copy the source page mechanically to the public mirror and verify the pair is byte-identical.
- [ ] Run the contract test and fix only page-specific failures.

### Task 3: Add quiet homepage entry points

**Files:**
- Modify: `marketing/index.html`
- Modify: `marketing/public/index.html`

- [ ] Add a contextual `/protocols/` link inside “For the technically curious” and one footer link without changing the hero, primary calls to action, or homepage navigation hierarchy.
- [ ] Copy the source homepage mechanically to the public mirror and verify the pair is byte-identical.
- [ ] Run the contract test and verify GREEN.

### Task 4: Document and visually verify the static site

**Files:**
- Modify: `marketing/README.md`

- [ ] Document both routes, mirror discipline, contract command, local preview command, and Wrangler deployment command.
- [ ] Serve `marketing/public/` locally and capture `/protocols/` at 390×844 and 1280×800 with Playwright.
- [ ] Inspect both screenshots for overflow, legibility, hierarchy, link affordances, and responsive behavior; iterate until both are visually sound.
- [ ] Verify the homepage links navigate to the new route in a local browser.

### Task 5: Verify, commit, deploy, and prove the live route

**Files:**
- Stage only the seven files named by this plan

- [ ] Run the contract test, `git diff --check`, mirror comparisons, and a local HTTP request for both routes.
- [ ] Re-read every edited file, run `git pull --rebase --autostash`, stage explicit paths only, and inspect `git diff --cached` before committing.
- [ ] Commit the implementation without staging `COLLABORATION.md` or unrelated shared-checkout changes.
- [ ] Run `npx wrangler deploy` from `marketing/`.
- [ ] Fetch the deployed homepage and `/protocols/`, verify successful status and expected content, and report separately what is proven, assumed, and still uncommitted.
