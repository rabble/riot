# Builder and Indymedia Lineage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a prominent, sourced homepage credit for @rabble and connect Riot's lineage to documented government action against Indymedia.

**Architecture:** Extend the existing static Lineage section with one responsive builder card, then mirror the homepage byte-for-byte into the Cloudflare assets directory. Pin the editorial facts and outbound sources in the existing Node contract.

**Tech Stack:** static HTML/CSS, Node.js assertions, Playwright, Cloudflare Workers assets

---

### Task 1: Pin the editorial contract

**Files:**
- Modify: `scripts/marketing/protocol-page-contracts.mjs`

- [x] Add assertions for the `builder` anchor, @rabble credit, Riot Willow-implementation attribution, 2017 and 2026 Indymedia actions, and the four approved source links:

```js
assert.match(home, /<section[^>]+id="builder"[\s\S]*Built by @rabble/i);
assert.match(home, /Riot[^.]*Willow implementation[^.]*@rabble/i);
assert.doesNotMatch(home, /Evan Henshaw(?:-Plath| Plath)/i);
assert.match(home, /2017[\s\S]*Linksunten[\s\S]*2026[\s\S]*complete ban/i);
for (const href of builderSources) assert.match(home, new RegExp(`href="${escapeRegex(href)}"`));
```

- [x] Run the contract and verify RED because the builder section is absent:

```sh
node scripts/marketing/protocol-page-contracts.mjs
# Expected: AssertionError matching the missing section#builder
```

### Task 2: Add the builder card

**Files:**
- Modify: `marketing/index.html`
- Modify: `marketing/public/index.html`
- Modify: `marketing/README.md`

- [x] Add the desktop Builder navigation link, responsive builder card inside Lineage, and footer credit using this structure:

```html
<a href="#builder">Builder</a>
<section id="builder" class="builder-card">
  <p class="builder-kicker">Built by @rabble</p>
  <h2>Decades into the same unfinished work.</h2>
  <div class="builder-grid">
    <article><h3>A long line of movement infrastructure</h3><p>Builder history and sources.</p></article>
    <article><h3>Seizure resistance is not theoretical</h3><p>Dated Indymedia actions and sources.</p></article>
  </div>
</section>
```

- [x] Use precise copy: “Riot and the Willow implementation inside it are being built by @rabble.” Follow it with the protest.net, Indymedia, TXTMob, Odeo/Twitter, Planetary, Nos, Divine, and Riot history; publish no legal name and do not claim sole authorship of the Willow specification.
- [x] Link exactly these four sources:

```text
https://www.cjr.org/business_of_news/local-news-indymedia-network-25-anniversary.php
https://www.nos.social/team/rabble
https://theanarchistlibrary.org/mirror/c/cg/crimethinc-german-government-shuts-down-indymedia.bare.html
https://www.heise.de/en/news/Acute-threat-Interior-ministers-demand-complete-ban-of-Indymedia-11350956.html
```

- [x] Mirror and verify GREEN:

```sh
cp marketing/index.html marketing/public/index.html
node scripts/marketing/protocol-page-contracts.mjs
# Expected: protocol marketing contracts: PASS
```

### Task 3: Visual and release verification

**Files:**
- Verify: `marketing/public/index.html`

- [x] Inspect desktop and mobile browser renders:

```sh
npx playwright screenshot --browser=chromium --viewport-size=1440,1000 http://127.0.0.1:4173/#builder /tmp/riot-builder-desktop.png
npx playwright screenshot --browser=chromium --viewport-size=390,844 http://127.0.0.1:4173/#builder /tmp/riot-builder-mobile.png
```

- [x] Verify source/public equality, whitespace, HTTP, and contract:

```sh
cmp -s marketing/index.html marketing/public/index.html
git diff --check -- marketing/index.html marketing/public/index.html marketing/README.md scripts/marketing/protocol-page-contracts.mjs
curl -fsS http://127.0.0.1:4173/#builder >/dev/null
node scripts/marketing/protocol-page-contracts.mjs
```

- [x] Pull, stage only the named paths, inspect, and commit:

```sh
git pull --rebase --autostash
git add -- docs/superpowers/specs/2026-07-13-builder-lineage-design.md docs/superpowers/plans/2026-07-13-builder-lineage.md marketing/README.md marketing/index.html marketing/public/index.html scripts/marketing/protocol-page-contracts.mjs
git diff --cached
git commit -m "feat(marketing): credit Riot's builder and lineage"
```

- [ ] Deploy, compare live HTML, push, and open:

```sh
(cd marketing && CI=1 WRANGLER_SEND_METRICS=false npx wrangler deploy)
curl -fsSL https://riot-protest-net-marketing.protestnet.workers.dev/ -o /tmp/riot-builder-live.html
cmp -s marketing/public/index.html /tmp/riot-builder-live.html
git push origin main
open https://riot-protest-net-marketing.protestnet.workers.dev/#builder
```
