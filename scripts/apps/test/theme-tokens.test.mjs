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
const NEUTRALS = {
  light: {
    "--riot-paper": "#ECE7DB", "--riot-surface": "#F8F4E9",
    "--riot-ink": "#17160F", "--riot-ink-soft": "#5F594F", "--riot-line": "#17160F",
  },
  dark: {
    "--riot-paper": "#17160F", "--riot-surface": "#242219",
    "--riot-ink": "#F5F0E4", "--riot-ink-soft": "#C8C1B4", "--riot-line": "#8D8679",
  },
};

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
    for (const [name, expected] of Object.entries(NEUTRALS[scheme])) {
      assert.equal(role(def, name), expected, `${scheme} default ${name}`);
    }
    assert.equal(role(def, "--riot-focus"), ng.focus.toUpperCase(), `${scheme} default focus == night-garden`);
    ORDER.forEach((r, i) => {
      assert.equal(role(def, ROLE_OF[r]), ng.pairs[i][0].toUpperCase(), `${scheme} default ${r} == night-garden`);
    });
  });
}

test("every CSS hex literal belongs to the approved theme palette", () => {
  const approved = new Set([
    ...Object.values(NEUTRALS).flatMap((roles) => Object.values(roles)),
    ...[LIGHT, DARK].flatMap((table) => Object.values(table).flatMap((theme) => [
      theme.focus,
      ...theme.pairs.flat(),
    ])),
  ].map((hex) => hex.toUpperCase()));
  const actual = [...css.matchAll(/#[0-9a-fA-F]{6}\b/g)].map(([hex]) => hex.toUpperCase());
  assert.ok(actual.length > 0, "tokens.css must contain literal palette values");
  for (const hex of actual) assert.ok(approved.has(hex), `unapproved color ${hex}`);
});
