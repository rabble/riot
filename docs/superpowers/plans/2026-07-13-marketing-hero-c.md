# Marketing Hero C Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the abstract homepage mesh with approved Hero C, using real iPhone screenshots inside device chrome while preserving Riot's message and improving miniapp comprehension.

**Architecture:** Keep the site dependency-free. Copy the four approved PNG captures into source and public asset mirrors, render them with semantic image markup and CSS-only phone frames, and pin the composition in the existing marketing contract. The desktop hero uses top-aligned copy and device columns; the mobile hero stacks copy above a compact screenshot cluster.

**Tech Stack:** static HTML/CSS, PNG assets, Node.js assertions, Playwright, Cloudflare Workers assets

---

### Task 1: Pin Hero C in the marketing contract

**Files:**
- Modify: `scripts/marketing/protocol-page-contracts.mjs`

- [x] Assert the four source/public asset pairs are byte-identical.
- [x] Assert the homepage contains the approved phone-frame hierarchy, real-screen proof label, supporting miniapp copy, top-aligned desktop rule, responsive mobile rule, and no abstract mesh markup.
- [x] Run `node scripts/marketing/protocol-page-contracts.mjs` and verify RED because Hero C and its assets are absent.

### Task 2: Implement the approved hero

**Files:**
- Modify: `marketing/index.html`
- Modify: `marketing/public/index.html`
- Create: `marketing/assets/screenshots/{spaces,apps,compose,checklist}.png`
- Create: `marketing/public/assets/screenshots/{spaces,apps,compose,checklist}.png`

- [x] Copy the approved simulator screenshots without modifying their pixels.
- [x] Replace the mesh with one main Spaces phone, three supporting phone thumbnails, and an honest simulator-evidence label.
- [x] Add “Communities carry their own tools” copy covering checklists, alerts, decisions, and events.
- [x] Top-align the desktop columns and stack the copy and phone cluster at mobile width without page-level overflow.
- [x] Update the public mirror and run the contract GREEN.

### Task 3: Visual, commit, and release gate

**Files:**
- Modify: `marketing/README.md`

- [x] Capture and inspect the hero at 1440×1000 and 390×844 with Playwright.
- [x] Verify `git diff --check`, source/public equality, asset hashes, local HTTP 200, and the focused contract.
- [x] Pull with autostash, stage only this plan and the claimed marketing paths, inspect `git diff --cached`, and commit.
- [ ] Deploy with Wrangler, verify live HTTP status and byte hashes, push `main`, and open the live hero locally.
