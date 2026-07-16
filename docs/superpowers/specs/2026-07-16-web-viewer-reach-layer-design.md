# Web Viewer — the Reach Layer

**Date:** 2026-07-16
**Status:** Partly built (gateway spike shipped); records decisions + boundaries.
**Scope:** The public web view of a Riot site as a *reach* surface — mirrorable
static HTML anyone can host, deliberately carrying no trust guarantees. Trust
lives in the app. Sits beside the composite-site model
(`2026-07-15-composite-site-namespace-manifest-design.md`); complements, does not
replace, sneakernet view-and-share (`2026-07-13-sneakerweb-view-share-design.md`).

---

## 1. Thesis (locked with rabble, 2026-07-15/16)

**Web = reach. App = truth.** Want something confirmable? Run the Riot app (p2p,
signed). The web view is the poster on the wall: see it anywhere, no install, no
key, no crypto surface for the reader. Every page links `riot://open?namespace=…`
so a serious reader opens the real verified thing.

**Many mirrors is the censorship answer, not a hardened center.** Anything that
re-centralizes is the bug: a canonical domain (block/seize/surveil point), a
gateway everyone pulls through (coercion target), a page that phones home (origin
coupling). A *malicious* mirror is a mini-honeypot (sees its visitors, can
tamper) — many-mirrors **dilutes** this (no mirror sees everyone; readers rotate;
tampering only fools casual readers), it does not eliminate it. Stated honestly,
not hidden. The web view never claims to be authoritative, so no one is fooled.

## 2. Three properties every dump must have (BUILT)

1. **Self-contained** — CSP baked into `<head>` as `<meta http-equiv>`, CSS/JS
   inline. A mirror is a complete folder; drop it on S3/Pages/IPFS/USB and the
   fences travel with it (a header-only CSP evaporates off `server.py`).
2. **Origin-agnostic** — no hardcoded domain, relative links, identity = E-pubkey
   in content not the URL. Same folder renders at any domain and at `file://`.
3. **Zero phone-home** — `connect-src 'none'`; no analytics; no external fetch.
   A dump that watches its readers *is* a honeypot.

Shipped: `server.py --dump DIR` renders each route to `<DIR>/<route>/index.html`.
Verified in headless chromium from `file://`: 0 CSP violations, renders at an
arbitrary path, 0 external fetch refs.

## 3. CSP is the load-bearing fence (BUILT)

```
default-src 'none'; style-src 'sha256-<skin>'; script-src 'sha256-<filter>';
connect-src 'none'; base-uri 'none'; form-action 'none'
```

`default-src 'none'` neutralizes the one real danger of rich owner content: CSS
`url()`/`@font-face`/`@import` and any JS can't fetch, so they can't leak a
reader's IP. This is what lets untrusted-authored CSS/JS be safe — the fence, not
trust in the author.

## 4. CSS / JS authorship

- **Owner-published CSS is the goal** (full stylesheet, not a token menu) — a site
  becomes unmistakably itself. Rides the signed manifest (owner-signed in E), so a
  mirror serves it but can't alter it. Fenced by `default-src 'none'`.
- **Skins are the seam (BUILT).** A skin is just a stylesheet;
  `content_security_policy(skin)` derives `style-src` from whichever is active, so
  swapping the stylesheet keeps the page self-fencing with no code change. This is
  exactly the slot owner-CSS plugs into once the manifest lands. Two shipped
  defaults: `newsprint` (sober newspaper) and `zine` (riso protest-poster),
  `--skin` selectable.
- **JS: vendored-in only, never external CDN.** External-at-runtime breaks privacy
  (reader-IP leak) + censorship (CDN choke point) + supply chain. Bundle inline,
  hash-pin `script-src 'sha256-…'` (never `'self'` — a hostile mirror owns its
  origin). Shipped: a self-contained client-side entry filter, `connect-src 'none'`
  so it filters but can't phone home; progressive enhancement (no-JS readers see
  no dead controls).
- **Owner-published *JS*** (vs viewer/app-shell JS) is higher risk — a compromised
  owner key runs code in readers' browsers (can't exfil under connect-src none, can
  deface). Deferred until a real need.

## 5. Trust-tier legibility (design = security)

Editorial (E, owned, verified) vs open-wire (W, communal, unverified) must be
visibly different so an anonymous tip never masquerades as editorial (composite
spec §6). In the newswire mockups this is the center features column vs the
monospace open-wire rail + a quiet green ✓ vs an "open · unverified" tag. The
aesthetic and the security requirement are the same decision.

## 6. What is built vs mocked

| | State |
|---|---|
| `--dump` static tree, mirror-anywhere | **built** (`ab69b0e`) |
| CSP baked into `<head>` | **built** (`e54706c`) |
| vendored hash-pinned client filter | **built** (`f42c0ed`) |
| two default skins + per-skin CSP (owner-CSS seam) | **built** (`19e7bc3`) |
| two-column indymedia newswire layout (features + open-wire, bylines, categories) | **mocked** — needs E/W namespace content (composite Unit 1/2) |
| owner-published CSS end-to-end (e.g. a collective's own brand) | **mocked** — needs the signed manifest (composite Unit 2) |

All built work reskins the hash-locked conference *incident* fixture; it is not
yet the two-column newswire, because that needs real editorial + open-wire content.

## 7. Next unlocks (in dependency order)

1. Composite **Unit 1** (owned-namespace admission) + **Unit 2** (signed manifest)
   → real E editorial + W open-wire content, and owner-published CSS/JS via the
   manifest, dropping into the skin seam already built.
2. Then: the two-column newswire render profile (the mockup made real).
3. Freshness on the cache path (stale-mirror rollback) — deferred; app is the
   backstop, so low urgency for the reach layer.
