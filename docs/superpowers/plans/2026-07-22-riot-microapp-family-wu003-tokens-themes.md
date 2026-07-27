# WU-003 — Semantic Tokens + 6 Theme Presets + Drift/Contrast Contract (web `_shared`) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: metaswarm orchestrated-execution. Steps use `- [ ]`. Parent: `2026-07-22-riot-microapp-family-master-plan.md`. Spec: `docs/superpowers/specs/2026-07-22-riot-microapp-family-design.md` §"Visual family" + §"Personal themes / Presets".

**Goal:** Replace the old soft-rounded `_shared/tokens.css` with Riot's paper/ink role-token system and all six theme presets (light + dark), plus a Node contract that validates the role set, the six themes, and WCAG AA / 3:1 contrast — the canonical source every later app WU copies from.

**Architecture:** Web-only. `fixtures/apps/_shared/tokens.css` is the authored source of truth (spec §"Canonical shared source") and the preview foundation (`scripts/apps/miniapp-preview-host.mjs:185` injects `/apps/_shared/tokens.css`). Packed apps read their **own** copy of `tokens.css`, so changing `_shared` is **non-breaking** for the eight shipped apps (they keep old vars until their per-app WU re-copies + repacks). No Rust, no native, no repack, no app-dir edits here.

**Tech Stack:** vanilla CSS custom properties + `@media (prefers-color-scheme: dark)` + `[data-riot-theme]` attribute selectors; Node `node:test` contract.

**Scope boundary (do NOT exceed):**
- Modify: `fixtures/apps/_shared/tokens.css`, `scripts/apps/miniapp-contracts.mjs` (migrate old-token assertions to role names).
- Create: `scripts/apps/test/theme-tokens.test.mjs` (new contract).
- **Never** touch: any `fixtures/apps/<app>/` source dir, any `*.cbor`, `pack_starter.rs`, Rust, or native code. Do NOT re-copy tokens into app dirs or repack — that changes app IDs and is each app's own Slice-4 WU.
- Font `@font-face` at `/.riot/fonts/*` is **WU-005** — here declare font-family **stacks with system fallbacks only**, no `@font-face`.
- The `data-riot-theme` **injection** (host→WebView) is **WU-006**; here the CSS only *reacts* to the attribute. Default (no attribute) = Night Garden.

**Verified anchors (origin/main incl. #94):** current `_shared/tokens.css` uses old vars `--paper #f4efe5 / --accent #2457d6 / --font-rounded`; `miniapp-contracts.mjs:11` reads `_shared/tokens.css`, `:44` asserts `color-scheme: light dark`, `:95-103` extract dark `--paper/--surface/--accent/--focus-ring` and assert 3:1; `miniapp-preview-host.mjs:185` injects `/apps/_shared/tokens.css` as foundation. App `index.html` links its own `tokens.css` then `style.css`.

**Spec preset values (authoritative — design-gate approved). Slugs are kebab-case of the theme name.**

Neutral roles:

| Role | var | Light | Dark |
| --- | --- | --- | --- |
| Paper | `--riot-paper` | `#ECE7DB` | `#17160F` |
| Surface | `--riot-surface` | `#F8F4E9` | `#242219` |
| Ink | `--riot-ink` | `#17160F` | `#F5F0E4` |
| Soft ink | `--riot-ink-soft` | `#5F594F` | `#C8C1B4` |
| Line | `--riot-line` | `#17160F` | `#8D8679` |

Theme accent roles — light (`structure/on-structure · action/on-action · quiet/on-quiet · signal/on-signal · focus`):

| slug | structure/on | action/on | quiet/on | signal/on | focus |
| --- | --- | --- | --- | --- | --- |
| `night-garden` (default) | `#642B58`/`#FFFFFF` | `#D1216E`/`#FFFFFF` | `#AEB8A0`/`#17160F` | `#E9F056`/`#17160F` | `#642B58` |
| `repair-picnic` | `#351E28`/`#FFFFFF` | `#FF5C34`/`#17160F` | `#AEB8A0`/`#17160F` | `#D3B86A`/`#17160F` | `#351E28` |
| `living-network` | `#006B62`/`#FFFFFF` | `#D1216E`/`#FFFFFF` | `#B8DFBD`/`#17160F` | `#D5A83F`/`#17160F` | `#006B62` |
| `deep-amaranth` | `#642B58`/`#FFFFFF` | `#D1216E`/`#FFFFFF` | `#C9AEC0`/`#17160F` | `#E8B4CE`/`#17160F` | `#642B58` |
| `signal-chartreuse` | `#C8E63C`/`#17160F` | `#D1216E`/`#FFFFFF` | `#DDE8A3`/`#17160F` | `#F0A7C7`/`#17160F` | `#D1216E` |
| `burnt-tomato` | `#E94B35`/`#17160F` | `#D1216E`/`#FFFFFF` | `#F1B2A5`/`#17160F` | `#E5B54B`/`#17160F` | `#E94B35` |

Theme accent roles — dark:

| slug | structure/on | action/on | quiet/on | signal/on | focus |
| --- | --- | --- | --- | --- | --- |
| `night-garden` | `#B982AD`/`#17160F` | `#E45A96`/`#17160F` | `#7F8975`/`#17160F` | `#E9F056`/`#17160F` | `#E9F056` |
| `repair-picnic` | `#B07A91`/`#17160F` | `#FF7A57`/`#17160F` | `#909A84`/`#17160F` | `#D7BF72`/`#17160F` | `#FF7A57` |
| `living-network` | `#4FB5A9`/`#17160F` | `#E45A96`/`#17160F` | `#8FC99B`/`#17160F` | `#E0B94C`/`#17160F` | `#8FC99B` |
| `deep-amaranth` | `#B982AD`/`#17160F` | `#E45A96`/`#17160F` | `#96778E`/`#17160F` | `#D993B2`/`#17160F` | `#B982AD` |
| `signal-chartreuse` | `#D8ED5A`/`#17160F` | `#E45A96`/`#17160F` | `#9DAA67`/`#17160F` | `#E79BBD`/`#17160F` | `#D8ED5A` |
| `burnt-tomato` | `#F06A50`/`#17160F` | `#E45A96`/`#17160F` | `#B9786B`/`#17160F` | `#E0B94C`/`#17160F` | `#E0B94C` |

Role var names (exact, spec L503-518): `--riot-paper --riot-surface --riot-ink --riot-ink-soft --riot-line --riot-structure --riot-on-structure --riot-action --riot-on-action --riot-quiet --riot-on-quiet --riot-signal --riot-on-signal --riot-focus`.

---

## Task 1: Rewrite `_shared/tokens.css` to the role system + 6 presets

**Files:** Modify `fixtures/apps/_shared/tokens.css`.

- [ ] **Step 1: Write the failing test** — create `scripts/apps/test/theme-tokens.test.mjs`:

```js
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");
const css = readFileSync(path.join(repoRoot, "fixtures/apps/_shared/tokens.css"), "utf8");

const ROLES = [
  "--riot-paper","--riot-surface","--riot-ink","--riot-ink-soft","--riot-line",
  "--riot-structure","--riot-on-structure","--riot-action","--riot-on-action",
  "--riot-quiet","--riot-on-quiet","--riot-signal","--riot-on-signal","--riot-focus",
];
const THEMES = ["night-garden","repair-picnic","living-network","deep-amaranth","signal-chartreuse","burnt-tomato"];

test("opts into light+dark controls", () => {
  assert.match(css, /color-scheme\s*:\s*light\s+dark/);
});

test("every role var is defined at :root (default Night Garden)", () => {
  for (const r of ROLES) assert.match(css, new RegExp(`${r}\\s*:`), `missing ${r}`);
});

test("all six theme slugs have a light attribute selector", () => {
  for (const t of THEMES) {
    assert.match(css, new RegExp(`\\[data-riot-theme="${t}"\\]`), `missing selector for ${t}`);
  }
});

test("a dark block redefines the roles", () => {
  // Split at the dark @media open; the dark region begins there. (Do NOT use a
  // greedy end-anchored `\}\s*$` regex — it over-captures the trailing
  // reduced-motion block; splitting on the marker is robust to rule reorder.)
  const darkStart = css.indexOf("@media (prefers-color-scheme: dark)");
  assert.ok(darkStart > 0, "no dark media block");
  const dark = css.slice(darkStart);
  for (const r of ["--riot-paper","--riot-ink","--riot-action","--riot-focus"]) {
    assert.match(dark, new RegExp(`${r}\\s*:`), `dark missing ${r}`);
  }
});

test("no legacy soft-rounded tokens remain in canonical source", () => {
  assert.doesNotMatch(css, /--font-rounded|--accent\s*:/, "legacy tokens must be gone from _shared");
});
```

- [ ] **Step 2: Run to verify fail** — `node --test scripts/apps/test/theme-tokens.test.mjs` → FAIL (roles/selectors absent, legacy tokens present).

- [ ] **Step 3: Implement** — replace the ENTIRE contents of `fixtures/apps/_shared/tokens.css` with:

```css
/* Riot microapp family — canonical semantic tokens + 6 personal themes.
   Source of truth (spec §"Canonical shared source"); each app WU copies this
   into its own dir before repacking. Default (no data-riot-theme) = Night
   Garden, so first paint never flashes another person's theme. Fonts are
   declared as stacks with system fallbacks; the self-hosted TTF pack + @font-face
   at /.riot/fonts/* land in WU-005. */

:root {
  color-scheme: light dark;

  /* neutral roles — light */
  --riot-paper: #ECE7DB;
  --riot-surface: #F8F4E9;
  --riot-ink: #17160F;
  --riot-ink-soft: #5F594F;
  --riot-line: #17160F;

  /* Night Garden (default) — light */
  --riot-structure: #642B58;
  --riot-on-structure: #FFFFFF;
  --riot-action: #D1216E;
  --riot-on-action: #FFFFFF;
  --riot-quiet: #AEB8A0;
  --riot-on-quiet: #17160F;
  --riot-signal: #E9F056;
  --riot-on-signal: #17160F;
  --riot-focus: #642B58;

  /* typography — WU-005 adds the self-hosted @font-face; stacks fall back to system */
  --riot-display: "Anton", Impact, "Arial Narrow", sans-serif;
  --riot-body: "Work Sans", system-ui, -apple-system, sans-serif;
  --riot-mono: "Space Mono", ui-monospace, "SFMono-Regular", monospace;

  /* one spacing scale */
  --riot-space-1: 4px;
  --riot-space-2: 8px;
  --riot-space-3: 12px;
  --riot-space-4: 16px;
  --riot-space-5: 24px;
  --riot-space-6: 32px;

  /* square / lightly-eased corners, hard 2px rules, deliberate offset shadow */
  --riot-radius: 2px;
  --riot-rule: 2px;
  --riot-shadow: 3px 3px 0 var(--riot-line);
}

:root[data-riot-theme="night-garden"] {
  --riot-structure: #642B58; --riot-on-structure: #FFFFFF;
  --riot-action: #D1216E; --riot-on-action: #FFFFFF;
  --riot-quiet: #AEB8A0; --riot-on-quiet: #17160F;
  --riot-signal: #E9F056; --riot-on-signal: #17160F;
  --riot-focus: #642B58;
}
:root[data-riot-theme="repair-picnic"] {
  --riot-structure: #351E28; --riot-on-structure: #FFFFFF;
  --riot-action: #FF5C34; --riot-on-action: #17160F;
  --riot-quiet: #AEB8A0; --riot-on-quiet: #17160F;
  --riot-signal: #D3B86A; --riot-on-signal: #17160F;
  --riot-focus: #351E28;
}
:root[data-riot-theme="living-network"] {
  --riot-structure: #006B62; --riot-on-structure: #FFFFFF;
  --riot-action: #D1216E; --riot-on-action: #FFFFFF;
  --riot-quiet: #B8DFBD; --riot-on-quiet: #17160F;
  --riot-signal: #D5A83F; --riot-on-signal: #17160F;
  --riot-focus: #006B62;
}
:root[data-riot-theme="deep-amaranth"] {
  --riot-structure: #642B58; --riot-on-structure: #FFFFFF;
  --riot-action: #D1216E; --riot-on-action: #FFFFFF;
  --riot-quiet: #C9AEC0; --riot-on-quiet: #17160F;
  --riot-signal: #E8B4CE; --riot-on-signal: #17160F;
  --riot-focus: #642B58;
}
:root[data-riot-theme="signal-chartreuse"] {
  --riot-structure: #C8E63C; --riot-on-structure: #17160F;
  --riot-action: #D1216E; --riot-on-action: #FFFFFF;
  --riot-quiet: #DDE8A3; --riot-on-quiet: #17160F;
  --riot-signal: #F0A7C7; --riot-on-signal: #17160F;
  --riot-focus: #D1216E;
}
:root[data-riot-theme="burnt-tomato"] {
  --riot-structure: #E94B35; --riot-on-structure: #17160F;
  --riot-action: #D1216E; --riot-on-action: #FFFFFF;
  --riot-quiet: #F1B2A5; --riot-on-quiet: #17160F;
  --riot-signal: #E5B54B; --riot-on-signal: #17160F;
  --riot-focus: #E94B35;
}

@media (prefers-color-scheme: dark) {
  :root {
    /* neutral roles — dark */
    --riot-paper: #17160F;
    --riot-surface: #242219;
    --riot-ink: #F5F0E4;
    --riot-ink-soft: #C8C1B4;
    --riot-line: #8D8679;

    /* Night Garden (default) — dark */
    --riot-structure: #B982AD; --riot-on-structure: #17160F;
    --riot-action: #E45A96; --riot-on-action: #17160F;
    --riot-quiet: #7F8975; --riot-on-quiet: #17160F;
    --riot-signal: #E9F056; --riot-on-signal: #17160F;
    --riot-focus: #E9F056;
  }
  :root[data-riot-theme="night-garden"] {
    --riot-structure: #B982AD; --riot-on-structure: #17160F;
    --riot-action: #E45A96; --riot-on-action: #17160F;
    --riot-quiet: #7F8975; --riot-on-quiet: #17160F;
    --riot-signal: #E9F056; --riot-on-signal: #17160F;
    --riot-focus: #E9F056;
  }
  :root[data-riot-theme="repair-picnic"] {
    --riot-structure: #B07A91; --riot-on-structure: #17160F;
    --riot-action: #FF7A57; --riot-on-action: #17160F;
    --riot-quiet: #909A84; --riot-on-quiet: #17160F;
    --riot-signal: #D7BF72; --riot-on-signal: #17160F;
    --riot-focus: #FF7A57;
  }
  :root[data-riot-theme="living-network"] {
    --riot-structure: #4FB5A9; --riot-on-structure: #17160F;
    --riot-action: #E45A96; --riot-on-action: #17160F;
    --riot-quiet: #8FC99B; --riot-on-quiet: #17160F;
    --riot-signal: #E0B94C; --riot-on-signal: #17160F;
    --riot-focus: #8FC99B;
  }
  :root[data-riot-theme="deep-amaranth"] {
    --riot-structure: #B982AD; --riot-on-structure: #17160F;
    --riot-action: #E45A96; --riot-on-action: #17160F;
    --riot-quiet: #96778E; --riot-on-quiet: #17160F;
    --riot-signal: #D993B2; --riot-on-signal: #17160F;
    --riot-focus: #B982AD;
  }
  :root[data-riot-theme="signal-chartreuse"] {
    --riot-structure: #D8ED5A; --riot-on-structure: #17160F;
    --riot-action: #E45A96; --riot-on-action: #17160F;
    --riot-quiet: #9DAA67; --riot-on-quiet: #17160F;
    --riot-signal: #E79BBD; --riot-on-signal: #17160F;
    --riot-focus: #D8ED5A;
  }
  :root[data-riot-theme="burnt-tomato"] {
    --riot-structure: #F06A50; --riot-on-structure: #17160F;
    --riot-action: #E45A96; --riot-on-action: #17160F;
    --riot-quiet: #B9786B; --riot-on-quiet: #17160F;
    --riot-signal: #E0B94C; --riot-on-signal: #17160F;
    --riot-focus: #E0B94C;
  }
}

*, *::before, *::after { box-sizing: border-box; }
html, body { min-height: 100%; }
body {
  margin: 0;
  padding: 0;
  background: var(--riot-paper);
  color: var(--riot-ink);
  font-family: var(--riot-body);
  line-height: 1.5;
  -webkit-font-smoothing: antialiased;
}
button, input, select, textarea { font: inherit; color: inherit; }
button, input, select, textarea, [role="button"] { min-height: 44px; }
:focus-visible { outline: 3px solid var(--riot-focus); outline-offset: 3px; }
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    scroll-behavior: auto !important;
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
  }
}
```

- [ ] **Step 4: Run to verify pass** — `node --test scripts/apps/test/theme-tokens.test.mjs` → all 5 structural tests PASS.

- [ ] **Step 5: Commit**

```bash
git add fixtures/apps/_shared/tokens.css scripts/apps/test/theme-tokens.test.mjs
git commit -m "feat(apps): canonical role tokens + six theme presets in _shared"
```

---

## Task 2: WCAG AA + 3:1 contrast contract across all six themes, light + dark

**Files:** Modify `scripts/apps/test/theme-tokens.test.mjs` (add contrast cases).

- [ ] **Step 1: Write the failing test** — append. It parses each theme's roles in light and dark and enforces the spec's contract ("Text/background pairs meet WCAG AA. Focus indicators and non-text control boundaries meet at least 3:1"):

```js
function srgb(hex) {
  const n = parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((c) => {
    const s = c / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  });
}
function lum([r, g, b]) { return 0.2126 * r + 0.7152 * g + 0.0722 * b; }
function ratio(a, b) {
  const [la, lb] = [lum(srgb(a)), lum(srgb(b))].sort((x, y) => y - x);
  return (la + 0.05) / (lb + 0.05);
}

// The spec's fixed values per theme/scheme (mirror the plan tables verbatim).
// Each entry: paper, ink, and the four fill/on pairs + focus.
const LIGHT = {
  "night-garden": { paper:"#ECE7DB", surface:"#F8F4E9", ink:"#17160F", focus:"#642B58",
    pairs:[["#642B58","#FFFFFF"],["#D1216E","#FFFFFF"],["#AEB8A0","#17160F"],["#E9F056","#17160F"]] },
  "repair-picnic": { paper:"#ECE7DB", surface:"#F8F4E9", ink:"#17160F", focus:"#351E28",
    pairs:[["#351E28","#FFFFFF"],["#FF5C34","#17160F"],["#AEB8A0","#17160F"],["#D3B86A","#17160F"]] },
  "living-network": { paper:"#ECE7DB", surface:"#F8F4E9", ink:"#17160F", focus:"#006B62",
    pairs:[["#006B62","#FFFFFF"],["#D1216E","#FFFFFF"],["#B8DFBD","#17160F"],["#D5A83F","#17160F"]] },
  "deep-amaranth": { paper:"#ECE7DB", surface:"#F8F4E9", ink:"#17160F", focus:"#642B58",
    pairs:[["#642B58","#FFFFFF"],["#D1216E","#FFFFFF"],["#C9AEC0","#17160F"],["#E8B4CE","#17160F"]] },
  "signal-chartreuse": { paper:"#ECE7DB", surface:"#F8F4E9", ink:"#17160F", focus:"#D1216E",
    pairs:[["#C8E63C","#17160F"],["#D1216E","#FFFFFF"],["#DDE8A3","#17160F"],["#F0A7C7","#17160F"]] },
  "burnt-tomato": { paper:"#ECE7DB", surface:"#F8F4E9", ink:"#17160F", focus:"#E94B35",
    pairs:[["#E94B35","#17160F"],["#D1216E","#FFFFFF"],["#F1B2A5","#17160F"],["#E5B54B","#17160F"]] },
};
const DARK = {
  "night-garden": { paper:"#17160F", surface:"#242219", ink:"#F5F0E4", focus:"#E9F056",
    pairs:[["#B982AD","#17160F"],["#E45A96","#17160F"],["#7F8975","#17160F"],["#E9F056","#17160F"]] },
  "repair-picnic": { paper:"#17160F", surface:"#242219", ink:"#F5F0E4", focus:"#FF7A57",
    pairs:[["#B07A91","#17160F"],["#FF7A57","#17160F"],["#909A84","#17160F"],["#D7BF72","#17160F"]] },
  "living-network": { paper:"#17160F", surface:"#242219", ink:"#F5F0E4", focus:"#8FC99B",
    pairs:[["#4FB5A9","#17160F"],["#E45A96","#17160F"],["#8FC99B","#17160F"],["#E0B94C","#17160F"]] },
  "deep-amaranth": { paper:"#17160F", surface:"#242219", ink:"#F5F0E4", focus:"#B982AD",
    pairs:[["#B982AD","#17160F"],["#E45A96","#17160F"],["#96778E","#17160F"],["#D993B2","#17160F"]] },
  "signal-chartreuse": { paper:"#17160F", surface:"#242219", ink:"#F5F0E4", focus:"#D8ED5A",
    pairs:[["#D8ED5A","#17160F"],["#E45A96","#17160F"],["#9DAA67","#17160F"],["#E79BBD","#17160F"]] },
  "burnt-tomato": { paper:"#17160F", surface:"#242219", ink:"#F5F0E4", focus:"#E0B94C",
    pairs:[["#F06A50","#17160F"],["#E45A96","#17160F"],["#B9786B","#17160F"],["#E0B94C","#17160F"]] },
};

for (const [scheme, table] of [["light", LIGHT], ["dark", DARK]]) {
  for (const [slug, t] of Object.entries(table)) {
    test(`${slug}/${scheme}: ink on paper & surface >= AA 4.5`, () => {
      assert.ok(ratio(t.ink, t.paper) >= 4.5, `ink/paper ${ratio(t.ink,t.paper).toFixed(2)}`);
      assert.ok(ratio(t.ink, t.surface) >= 4.5, `ink/surface ${ratio(t.ink,t.surface).toFixed(2)}`);
    });
    test(`${slug}/${scheme}: each on/fill text pair >= AA 4.5`, () => {
      for (const [fill, on] of t.pairs) {
        assert.ok(ratio(on, fill) >= 4.5, `on ${on} / fill ${fill} = ${ratio(on,fill).toFixed(2)}`);
      }
    });
    test(`${slug}/${scheme}: focus ring >= 3:1 vs paper and surface`, () => {
      assert.ok(ratio(t.focus, t.paper) >= 3, `focus/paper ${ratio(t.focus,t.paper).toFixed(2)}`);
      assert.ok(ratio(t.focus, t.surface) >= 3, `focus/surface ${ratio(t.focus,t.surface).toFixed(2)}`);
    });
  }
}

// ---- Pin every hex in tokens.css to the spec table above ----
// Without this the LIGHT/DARK tables are self-referential: a typo in
// tokens.css (e.g. --riot-action: #D1216F) would pass every other test. This
// parses the ACTUAL file and asserts each theme block equals the table, and
// that the attribute-less :root default IS Night Garden. Split on the dark
// marker (robust to rule reordering); theme blocks are flat so `[^}]*` is safe.
const darkStart = css.indexOf("@media (prefers-color-scheme: dark)");
const lightCss = css.slice(0, darkStart);
const darkCss = css.slice(darkStart);
function blockBody(scopeCss, selector) {
  const esc = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return scopeCss.match(new RegExp(esc + "\\s*\\{([^}]*)\\}"))?.[1] ?? "";
}
function role(body, name) {
  return body.match(new RegExp(name + ":\\s*(#[0-9a-fA-F]{6})"))?.[1]?.toUpperCase();
}
const ROLE_OF = { structure: "--riot-structure", action: "--riot-action", quiet: "--riot-quiet", signal: "--riot-signal" };
const ON_OF = { structure: "--riot-on-structure", action: "--riot-on-action", quiet: "--riot-on-quiet", signal: "--riot-on-signal" };
const ORDER = ["structure", "action", "quiet", "signal"]; // matches pairs[] order in LIGHT/DARK

for (const [scheme, scopeCss, table] of [["light", lightCss, LIGHT], ["dark", darkCss, DARK]]) {
  for (const [slug, t] of Object.entries(table)) {
    test(`${slug}/${scheme}: file hexes equal the spec table`, () => {
      const body = blockBody(scopeCss, `:root[data-riot-theme="${slug}"]`);
      assert.ok(body, `missing ${slug} ${scheme} selector block`);
      assert.equal(role(body, "--riot-focus"), t.focus.toUpperCase(), `${slug} ${scheme} focus`);
      ORDER.forEach((r, i) => {
        assert.equal(role(body, ROLE_OF[r]), t.pairs[i][0].toUpperCase(), `${slug} ${scheme} ${r} fill`);
        assert.equal(role(body, ON_OF[r]), t.pairs[i][1].toUpperCase(), `${slug} ${scheme} on-${r}`);
      });
    });
  }
  test(`${scheme}: attribute-less :root default equals Night Garden`, () => {
    const def = blockBody(scopeCss, ":root");
    const ng = table["night-garden"];
    assert.equal(role(def, "--riot-paper"), ng.paper.toUpperCase(), `${scheme} default paper (neutral)`);
    assert.equal(role(def, "--riot-focus"), ng.focus.toUpperCase(), `${scheme} default focus == night-garden`);
    ORDER.forEach((r, i) => {
      assert.equal(role(def, ROLE_OF[r]), ng.pairs[i][0].toUpperCase(), `${scheme} default ${r} == night-garden`);
    });
  });
}
```

- [ ] **Step 2: Run to verify.** `node --test scripts/apps/test/theme-tokens.test.mjs`. The contrast cases pass (verified: all 12 theme/scheme combinations satisfy AA + 3:1). The new "file hexes equal the spec table" + "default equals Night Garden" cases pass because Task 1's CSS carries exactly these values. **If a `file hexes` case fails, a hex in `tokens.css` was mistyped — fix the CSS to match the spec table (do NOT edit the table). If a `contrast` case fails, STOP and report the exact theme/scheme/pair + ratio — that is a spec defect to surface upward, never a silent threshold relaxation.**

- [ ] **Step 3:** No implementation change if all pass (the CSS from Task 1 already carries these exact values; the test pins them). If a large-text exception is genuinely needed for one pair, that requires a spec amendment — surface it, do not encode a lower threshold unilaterally.

- [ ] **Step 4: Run to verify pass** — full `node --test scripts/apps/test/theme-tokens.test.mjs` green (structural + all contrast cases).

- [ ] **Step 5: Commit**

```bash
git add scripts/apps/test/theme-tokens.test.mjs
git commit -m "test(apps): enforce WCAG AA + 3:1 focus contrast for all six themes light+dark"
```

---

## Task 3: Migrate the existing contract off legacy token names

**Files:** Modify `scripts/apps/miniapp-contracts.mjs`.

- [ ] **Step 1: See it fail at the FIRST legacy assertion, not the dark block.** `miniapp-contracts.mjs` reads `_shared/tokens.css` into `tokens` at ~`:37`, then asserts against legacy names in THREE places: the presence loop `:38-43` (`--paper --surface --ink --muted --line --accent --radius --shadow --space-1 --font-rounded`), the focus checks `:49-50` (`--focus-ring:` and `:focus-visible … var(--focus-ring)`), and the dark-token block `:95-103` (`--paper/--surface/--accent/--focus-ring`). After Task 1 renames everything to `--riot-*`, `node scripts/apps/miniapp-contracts.mjs` throws at **`:42`** (`--paper`) — long before the dark block. ALL THREE spots must migrate.

- [ ] **Step 2a: Migrate the presence loop (`:38-43`)** to the role names that actually exist in the new `_shared` (drop `--muted`→`--riot-ink-soft`, `--accent`→`--riot-action`, `--font-rounded`→`--riot-body`; keep the rest as `--riot-*`):

```js
for (const token of [
  "--riot-paper", "--riot-surface", "--riot-ink", "--riot-ink-soft", "--riot-line",
  "--riot-structure", "--riot-action", "--riot-quiet", "--riot-signal", "--riot-focus",
  "--riot-radius", "--riot-shadow", "--riot-space-1", "--riot-body",
]) {
  assert.match(tokens, new RegExp(`${token}\\s*:`), `shared tokens must define ${token}`);
}
```

- [ ] **Step 2b: Migrate the focus checks (`:49-50`)**:

```js
assert.match(tokens, /--riot-focus\s*:/, "tokens must define a dedicated focus color");
assert.match(tokens, /:focus-visible\s*\{[^}]*outline\s*:\s*3px\s+solid\s+var\(--riot-focus\)/s, "keyboard focus must use a visible 3px focus ring");
```

(Leave `:44-48,51` unchanged — `color-scheme: light dark`, box-sizing, body-margin, `font: inherit`, `min-height: 44px`, reduced-motion are all still present in the new file.)

- [ ] **Step 2c: Migrate the dark-token smoke block (`:95-103`)** to role names, using the repo's proven non-greedy split (NOT a greedy end-anchored regex — that over-captures the trailing reduced-motion block). The full six-theme matrix lives in `theme-tokens.test.mjs`; keep a Night-Garden-dark smoke check here so this file stays self-consistent:

```js
// _shared now carries the role-token system; the full six-theme WCAG matrix is
// in scripts/apps/test/theme-tokens.test.mjs. Smoke-check dark Night Garden here.
const darkStart = tokens.indexOf("@media (prefers-color-scheme: dark)");
const darkTokens = darkStart >= 0 ? tokens.slice(darkStart) : "";
const darkPaper = darkTokens.match(/--riot-paper:\s*(#[0-9a-f]{6})/i)?.[1];
const darkAction = darkTokens.match(/--riot-action:\s*(#[0-9a-f]{6})/i)?.[1];
const darkFocus = darkTokens.match(/--riot-focus:\s*(#[0-9a-f]{6})/i)?.[1];
assert(darkPaper && darkAction && darkFocus, "dark mode must define riot paper, action, and focus roles");
assert(contrast(darkFocus, darkPaper) >= 3, "dark focus ring must have at least 3:1 contrast against paper");
```

Leave the per-app checks (`:58-73`: per-app `riot-app.json/index.html/tokens.css/style.css/app.js` presence, tokens-before-style order, `riot.watch/whoami/profile/ensureSeeded`, no `innerHTML=`, no `fetch(`) unchanged — the eight apps still carry their own copies this WU. Do NOT add an "app copy == canonical `_shared`" drift assertion yet: app copies intentionally diverge from `_shared` until each app's Slice-4 WU re-copies + repacks (drift check lands per-app in WU-007..014).

- [ ] **Step 3: Run to verify pass** — `node scripts/apps/miniapp-contracts.mjs` exits 0 (no throw at `:42` or elsewhere; the preview-host boot still validates the eight apps with their own copies). Watch: the new `_shared` foundation adds a `body { background/color/font-family }` reset that the preview injects ahead of each app's own CSS — confirm no app preview visibly breaks (this only affects preview, never packed bundles). If an app's own `body` styling already sets these, it wins by later cascade; if a regression appears, it belongs to that app's later WU, not here.

- [ ] **Step 4: Commit**

```bash
git add scripts/apps/miniapp-contracts.mjs
git commit -m "chore(apps): migrate shared-token contract to riot role names"
```

---

## Task 4: Full web gate

- [ ] **Step 1:** `node --test scripts/apps/test/theme-tokens.test.mjs` → all green.
- [ ] **Step 2:** `node scripts/apps/miniapp-contracts.mjs` → exit 0.
- [ ] **Step 3:** If `package.json` has an apps test script (`test:apps:contracts` / `test:apps:unit`), run it to confirm no wider web breakage: `npm run test:apps:contracts` (skip browser/Playwright — that's WU-016).
- [ ] **Step 4:** Confirm no app-dir or `.cbor` bytes changed: `git status --porcelain` shows only `fixtures/apps/_shared/tokens.css`, `scripts/apps/miniapp-contracts.mjs`, `scripts/apps/test/theme-tokens.test.mjs`.

---

## Definition of Done

- `_shared/tokens.css` defines all 14 role vars at `:root` (Night Garden default), six `[data-riot-theme]` light selectors, and a dark `@media` block with `:root` default + all six selectors; legacy `--accent`/`--font-rounded` gone from canonical source.
- New `theme-tokens.test.mjs` proves: role completeness, six-theme presence, dark redefinition, **exact-hex equality between `tokens.css` and the spec table** (parses each theme block from the file — a mistyped hex fails), **attribute-less `:root` default == Night Garden** (light + dark), and **WCAG AA (≥4.5) for ink/paper+surface and every on/fill pair + ≥3:1 focus** across all six themes in light AND dark.
- `miniapp-contracts.mjs` migrated to role names in ALL three spots (presence loop, focus checks, dark smoke block), still green; eight apps untouched (keep their own copies).
- No app-dir/`.cbor`/Rust/native changes; no repack.

## Explicitly deferred

- Self-hosted TTF `@font-face` at `/.riot/fonts/*` + per-ID CSP `font-src 'self'` + normalization vectors → **WU-005**.
- `data-riot-theme` host→WebView injection, 8-ID allowlist, fail-closed → **WU-006**.
- Re-copying tokens into each app dir + repack + "app copy == canonical" drift audit → each app's **WU-007..014**.
- `appearanceProfileID` + theme picker + native preference store → **WU-004**.
