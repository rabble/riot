import assert from "node:assert/strict";
import { createServer } from "node:http";
import { readFile, mkdir, stat, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, extname, join, normalize, resolve } from "node:path";
import { chromium } from "@playwright/test";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "../..");
const paths = {
  home: resolve(root, "marketing/index.html"),
  publicHome: resolve(root, "marketing/public/index.html"),
  protocols: resolve(root, "marketing/protocols/index.html"),
  publicProtocols: resolve(root, "marketing/public/protocols/index.html"),
  swiftStory: resolve(root, "apps/ios/Riot/CommunityShell.swift"),
  swiftPresentation: resolve(root, "apps/ios/Riot/ConferenceShellView.swift"),
};
// Secondary pages (source + byte-identical public mirror). Each is dependency-free
// and reuses the protocols-page design system (system fonts, no runtime media/scripts).
const secondaryPages = ["about", "privacy", "open-source", "community", "releases"];
const secondary = Object.fromEntries(
  secondaryPages.flatMap((name) => [
    [name, resolve(root, `marketing/${name}/index.html`)],
    [`public${name[0].toUpperCase()}${name.slice(1)}`, resolve(root, `marketing/public/${name}/index.html`)],
  ]),
);
// Guide routes (offline-guides reframe, 2026-07-20 design): the paired-story
// entry points. Text-first for now — the deterministic screenshot/Willow-art
// pipeline ships separately; prose carries every material fact.
const guides = {
  whyRiot: resolve(root, "marketing/why-riot/index.html"),
  publicWhyRiot: resolve(root, "marketing/public/why-riot/index.html"),
  guide: resolve(root, "marketing/guide/index.html"),
  publicGuide: resolve(root, "marketing/public/guide/index.html"),
};
const allPaths = { ...paths, ...secondary, ...guides };

const read = async (path) => readFile(path, "utf8");
const allSitePaths = ["/", "/why-riot/", "/guide/", "/about/", "/privacy/", "/open-source/", "/community/", "/releases/", "/protocols/"];
const primaryNavPaths = ["/", "/why-riot/", "/guide/", "/about/", "/open-source/", "/community/", "/releases/", "/protocols/"];
const normalizeLocalRoute = (href, { allowExternal = false } = {}) => {
  const url = new URL(href, "https://local.invalid");
  if (!allowExternal && url.origin !== "https://local.invalid") return null;
  let path = url.pathname.replace(/\/index\.html$/i, "/");
  if (path !== "/" && !path.endsWith("/")) path += "/";
  return path;
};
const hrefsFrom = (html) => [...html.matchAll(/\bhref=(?:"([^"]*)"|'([^']*)')/gi)].map((match) => match[1] ?? match[2]);
const localRoutesFrom = (html) => hrefsFrom(html).map(normalizeLocalRoute).filter(Boolean);
const block = (html, pattern, label) => {
  const match = html.match(pattern);
  assert.ok(match, `missing ${label}`);
  return match[0];
};
const assertExactRoutes = (actual, expected, label, { ordered = false } = {}) => {
  assert.deepEqual(new Set(actual), new Set(expected), `${label} local route set drift`);
  if (ordered) assert.deepEqual(actual, expected, `${label} local route order drift`);
};
const visibleText = (html) => html.replace(/<style\b[\s\S]*?<\/style>/gi, " ").replace(/<script\b[\s\S]*?<\/script>/gi, " ").replace(/<[^>]+>/g, " ").replace(/&(?:nbsp|mdash|ndash|amp|lt|gt);/g, " ").replace(/\s+/g, " ").trim();
const expectEnoent = async (path) => {
  await assert.rejects(stat(path), (error) => error?.code === "ENOENT", `${path} must not exist`);
};
const readAll = async (obj) => {
  const entries = Object.entries(obj);
  const values = await Promise.all(entries.map(([, p]) => read(p)));
  return Object.fromEntries(entries.map(([k], i) => [k, values[i]]));
};
const {
  home, publicHome, protocols, publicProtocols,
  about, publicAbout, privacy, publicPrivacy,
  "open-source": openSource, "publicOpen-source": publicOpenSource, community, publicCommunity,
  releases, publicReleases, whyRiot, publicWhyRiot, guide, publicGuide,
} = await readAll(allPaths);
// Paired explainer (#92): the iOS story/presentation sources must match the
// five-beat copy the website claims. Read them so later assertions can pin both sides.
const [swiftStory, swiftPresentation] = await Promise.all([
  read(paths.swiftStory),
  read(paths.swiftPresentation),
]);

assert.equal(home, publicHome, "homepage source and public mirror must be byte-identical");
assert.equal(protocols, publicProtocols, "protocol page source and public mirror must be byte-identical");
for (const name of secondaryPages) {
  const src = allPaths[name];
  const mirror = allPaths[`public${name[0].toUpperCase()}${name.slice(1)}`];
  assert.equal(await read(src), await read(mirror), `${name} page source and public mirror must be byte-identical`);
}
assert.equal(whyRiot, publicWhyRiot, "why-riot page source and public mirror must be byte-identical");
assert.equal(guide, publicGuide, "guide page source and public mirror must be byte-identical");

for (const name of [
  "spaces", "apps", "compose", "checklist",
  "app-home", "app-events", "app-decisions", "app-dispatches", "app-photos", "app-checklist",
]) {
  const [sourceAsset, publicAsset] = await Promise.all([
    readFile(resolve(root, `marketing/assets/screenshots/${name}.png`)),
    readFile(resolve(root, `marketing/public/assets/screenshots/${name}.png`)),
  ]);
  assert.deepEqual(sourceAsset, publicAsset, `${name} screenshot source and public mirror must be byte-identical`);
}

assert.doesNotMatch(home, /hero-mesh|mesh-edges|mesh-nodes/, "approved Hero C must replace the abstract mesh");
assert.match(home, /\.hero-grid\s*\{[^}]*align-items:\s*start/i, "desktop hero copy and devices must be top-aligned");
assert.match(home, /class="device-scene"[\s\S]*class="win-frame main"[\s\S]*\/assets\/screenshots\/app-events\.png/i);

// ---------- human-capacity homepage (2026-07-22) ----------------------------
assert.match(home, /<h1[^>]*>Community tools that travel with people\.<\/h1>/i, "homepage needs its distinct human-capacity headline");
assert.doesNotMatch(home, /<h1[^>]*>People are the infrastructure\.<\/h1>/i, "homepage must not duplicate Why Riot");
assert.match(home, /Riot is a home for public conversation, community decisions, shared tools, and collective\s+memory(?:—|&mdash;)carried by the people who make it matter\./i, "homepage needs the approved support line");
assert.match(home, /class="hero-actions"[\s\S]*href="\/why-riot\/"[^>]*>Why Riot exists/i, "homepage hero needs a prominent Why Riot action");
assert.match(home, /festival|community meal|neighborhood publication|cooperative decision/i, "homepage must show value in ordinary community life");
assert.match(home, /class="hero-stamp">Prototype/i, "hero must carry a visible Prototype label");
assert.doesNotMatch(home, /<script/i, "homepage must remain script-free");
assert.doesNotMatch(home, /ecosystem/i, "homepage must not use 'ecosystem' jargon");
for (const guidePath of ["/why-riot/", "/guide/"]) assert.ok(home.includes(`href="${guidePath}"`), `homepage must link to ${guidePath}`);
for (const name of ["app-decisions", "app-dispatches", "app-photos"]) {
  assert.match(home, new RegExp(`class="win-frame thumb"[\\s\\S]*?/assets/screenshots/${name}\\.png`, "i"), `missing ${name} supporting screen`);
}
assert.match(home, /Real screens[\s\S]*Riot desktop build/i);
assert.match(home, /More than a social feed[\s\S]*Communities carry their own tools[\s\S]*checklists, alerts, decisions, events/i);
assert.match(home, /@media\s*\(max-width:\s*860px\)[\s\S]*\.device-scene/i, "Hero C needs a mobile device composition");

assert.match(home, /<section\s+id="builder"\s+class="builder-card"[\s\S]*Built by @rabble/i, "homepage needs a visible builder section");
assert.match(home, />@rabble<\/a> is building Riot using the Willow libraries/i, "credit @rabble with Riot while crediting Willow as a dependency");
assert.doesNotMatch(home, /(?:built|building)[^.<]{0,120}Willow implementation|Willow implementation[^.<]{0,120}(?:built|building)/i, "do not claim @rabble implemented Willow");
assert.doesNotMatch(home, /Evan Henshaw(?:-Plath| Plath)/i, "the marketing site must not publish @rabble's legal name");
assert.match(home, /2017[\s\S]*Linksunten[\s\S]*2026[\s\S]*complete ban/i, "distinguish the two Indymedia government actions");
for (const href of [
  "https://www.cjr.org/business_of_news/local-news-indymedia-network-25-anniversary.php",
  "https://www.nos.social/team/rabble",
  "https://theanarchistlibrary.org/mirror/c/cg/crimethinc-german-government-shuts-down-indymedia.bare.html",
  "https://www.heise.de/en/news/Acute-threat-Interior-ministers-demand-complete-ban-of-Indymedia-11350956.html",
]) {
  assert.ok(home.includes(`href="${href}"`), `missing builder source: ${href}`);
}
assert.match(home, /class="section-toc"[\s\S]*href="#builder"[^>]*>Builder/i, "homepage section TOC must link to #builder");
assert.match(home, /<footer[\s\S]*Built by[\s\S]*@rabble/i);

const homeLinks = home.match(/href="\/protocols\/"/g) ?? [];
// The rebuild surfaces /protocols/ in more than the original four placements (sitenav,
// what-is, explore cluster) — that is intentional and good for discoverability. The guard
// is now "at least four" plus the specific required placements below, which preserve the
// original editorial intent (topnav, callout, technically-curious panel, footer).
assert.ok(homeLinks.length >= 4, `homepage must contain at least four /protocols/ paths (found ${homeLinks.length})`);
assert.match(home, /class="sitenav-links"[\s\S]*href="\/protocols\/"[^>]*>Protocol field guide</i, "homepage sitenav must link to the protocol field guide");
assert.match(home, /class="protocol-callout"[\s\S]*Where does Riot fit\?[\s\S]*Compare Riot, Willow, and neighboring protocols/i);
assert.match(home, /For the technically curious[\s\S]*Compare Riot with adjacent protocols/i);
assert.match(home, /<footer[\s\S]*Protocol field guide/i);

for (const landmark of ["<main", "<nav", "<h1", "<footer"]) {
  assert.ok(protocols.includes(landmark), `protocol page must include ${landmark}`);
}
assert.match(protocols, /<table[\s\S]*<caption>/i, "comparison table needs a caption");
assert.match(protocols, /aria-label="Protocol comparison table"/i);
assert.match(protocols, /@media\s*\(prefers-reduced-motion:\s*reduce\)/i);
assert.match(protocols, /Checked\s+13 July 2026/i);

const requiredProfiles = [
  ["riot-willow", "Riot + Willow"],
  ["at-protocol", "AT Protocol"],
  ["activitypub", "ActivityPub"],
  ["dfos", "DFOS"],
  ["nostr", "Nostr"],
  ["farcaster", "Farcaster"],
  ["bitchat", "Bitchat"],
  ["secure-scuttlebutt", "Secure Scuttlebutt"],
  ["briar", "Briar"],
  ["matrix", "Matrix"],
];
for (const [id, name] of requiredProfiles) {
  assert.match(protocols, new RegExp(`id="${id}"[\\s\\S]{0,500}<h3[^>]*>${name.replace("+", "\\+")}`, "i"), `${name} needs a deep-dive profile`);
}

for (const phrase of [
  "private encrypted groups are not shipped",
  "public spaces are not confidential",
  "two physical iPhones",
  "durable multi-community storage",
  "not an audited app store",
  "relay operator can read what it stores",
]) {
  assert.ok(protocols.toLowerCase().includes(phrase.toLowerCase()), `missing honest boundary: ${phrase}`);
}

for (const section of [
  "These systems are not the same layer",
  "Orientation by situation",
  "Why Willow?",
  "One checklist change, end to end",
  "What Riot does not provide yet",
  "Primary source ledger",
]) {
  assert.ok(protocols.includes(section), `missing required section: ${section}`);
}

const sourceOrigins = [
  "willowprotocol.org",
  "atproto.com",
  "w3.org/TR/activitypub",
  "protocol.dfos.com",
  "github.com/nostr-protocol/nips",
  "docs.farcaster.xyz",
  "github.com/permissionlesstech/bitchat",
  "ssbc.github.io",
  "briarproject.org",
  "spec.matrix.org",
];
for (const origin of sourceOrigins) {
  assert.ok(protocols.includes(origin), `missing primary source for ${origin}`);
}

assert.doesNotMatch(protocols, /<(?:script|iframe|img|audio|video|source)\b[^>]*\bsrc=/i, "protocol page must not fetch runtime media or scripts");
assert.doesNotMatch(protocols, /@import\s+url|url\(\s*["']?https?:/i, "protocol page must not fetch remote CSS assets");
// Match analytics only as a loader (script tag, src/href reference, or loader URL) —
// not the vendor names appearing in body prose like the privacy page's "no Google Analytics".
assert.doesNotMatch(protocols, /<(?:script|link|img|iframe)[^>]+(?:plausible|google-analytics|googletagmanager|segment\.com|mixpanel|hotjar|clarity)/i, "protocol page must not load analytics");
assert.doesNotMatch(protocols, /(?:plausible|google-analytics|googletagmanager|segment\.com|mixpanel|hotjar|clarity)\.[a-z0-9-]+\/(?:[a-z0-9-]+\.js|analytics|track|beacon)/i, "protocol page must not reference an analytics endpoint");

// --- Exact footer and primary-navigation contracts --------------------------
const pageContents = { home, protocols, about, privacy, "open-source": openSource, community, releases, "why-riot": whyRiot, guide };
const mirrorContents = { home: publicHome, protocols: publicProtocols, about: publicAbout, privacy: publicPrivacy, "open-source": publicOpenSource, community: publicCommunity, releases: publicReleases, "why-riot": publicWhyRiot, guide: publicGuide };
for (const [pageName, content] of Object.entries({ ...Object.fromEntries(Object.entries(pageContents).map(([name, html]) => [`source:${name}`, html])), ...Object.fromEntries(Object.entries(mirrorContents).map(([name, html]) => [`mirror:${name}`, html])) })) {
  assert.match(content, /\.sitenav\s*\{[^}]*position:\s*sticky/i, `${pageName} page must make the sitenav sticky`);
  const footer = block(content, /<footer\b[^>]*>[\s\S]*?<\/footer>/i, `${pageName} footer`);
  assertExactRoutes(localRoutesFrom(footer), allSitePaths, `${pageName} footer`);
  const nav = block(content, /<div\s+class="sitenav-links">[\s\S]*?<\/div>/i, `${pageName} sitenav-links`);
  assertExactRoutes(localRoutesFrom(nav), primaryNavPaths, `${pageName} primary nav`, { ordered: true });
  assert.doesNotMatch(nav, /href="\/privacy\/"/i, `${pageName} primary nav must not include Privacy`);
}

// --- Favicons ----------------------------------------------------------------
// Every page carries the same inline SVG data-URI favicon so there are no binary
// assets and no remote fetches.
for (const [pageName, content] of Object.entries(pageContents)) {
  assert.match(content, /<link rel="icon" href="data:image\/svg\+xml/, `${pageName} page must declare an inline SVG favicon`);
}

// --- Secondary pages: structure + dependency-free ----------------------------
// The four secondary pages follow the protocols-page rule: no runtime
// media/scripts, no remote CSS, no analytics, and the accessibility landmarks
// every page shares.
for (const [pageName, content] of Object.entries({ about, privacy, "open-source": openSource, community, releases, "why-riot": whyRiot, guide })) {
  for (const landmark of ["<main", "<nav", "<h1", "<footer"]) {
    assert.ok(content.includes(landmark), `${pageName} page must include ${landmark}`);
  }
  assert.match(content, /class="skip-link"/, `${pageName} page must include a skip-link`);
  assert.doesNotMatch(content, /<(?:script|iframe|img|audio|video|source)\b[^>]*\bsrc=/i, `${pageName} page must not fetch runtime media or scripts`);
  assert.doesNotMatch(content, /@import\s+url|url\(\s*["']?https?:/i, `${pageName} page must not fetch remote CSS assets`);
  assert.doesNotMatch(content, /<(?:script|link|img|iframe)[^>]+(?:plausible|google-analytics|googletagmanager|segment\.com|mixpanel|hotjar|clarity)/i, `${pageName} page must not load analytics`);
  assert.doesNotMatch(content, /(?:plausible|google-analytics|googletagmanager|segment\.com|mixpanel|hotjar|clarity)\.[a-z0-9-]+\/(?:[a-z0-9-]+\.js|analytics|track|beacon)/i, `${pageName} page must not reference an analytics endpoint`);
}

// --- Static resource, navigation, and claim safety --------------------------
const allEditorialCopies = { ...Object.fromEntries(Object.entries(pageContents).map(([name, html]) => [`source:${name}`, html])), ...Object.fromEntries(Object.entries(mirrorContents).map(([name, html]) => [`mirror:${name}`, html])) };
const forbiddenHtml = [
  [/javascript:/i, "javascript URL"], [/\son[a-z]+\s*=/i, "inline event handler"], [/\sping\s*=/i, "ping attribute"],
  [/<meta\b[^>]*http-equiv\s*=\s*["']?refresh/i, "meta refresh"], [/<form\b/i, "form"], [/<base\b/i, "base element"], [/\ssrcdoc\s*=/i, "srcdoc"],
  [/<(?:use|image|feImage)\b[^>]*(?:href|xlink:href)\s*=\s*["'](?:https?:)?\/\//i, "remote SVG reference"],
  [/<(?:script|link|img|iframe|audio|video|source|object|embed)\b[^>]*(?:src|srcset|href|data|poster)\s*=\s*["'](?:https?:)?\/\//i, "remote runtime resource"],
  [/@import\s+url|url\(\s*["']?(?:https?:)?\/\//i, "remote CSS resource"],
  [/<(?:script|link|img|iframe)[^>]+(?:plausible|google-analytics|googletagmanager|segment\.com|mixpanel|hotjar|clarity)/i, "analytics loader"],
];
const unsafeRawUrl = /[\\\x00-\x1f\x7f]|%(?![0-9a-f]{2})/i;
const isLocalResource = (value) => !unsafeRawUrl.test(value) && !value.startsWith("//") && !/^[a-z][a-z0-9+.-]*:/i.test(value);
const validateSrcset = (value, label) => {
  const candidates = value.split(",").map((candidate) => candidate.trim());
  assert.ok(candidates.length && candidates.every(Boolean), `${label} malformed srcset`);
  for (const candidate of candidates) {
    const parts = candidate.split(/\s+/);
    assert.ok(parts.length <= 2 && isLocalResource(parts[0]), `${label} unsafe srcset candidate`);
    if (parts[1]) assert.match(parts[1], /^(?:(?:[1-9]\d*)w|(?:\d+(?:\.\d+)?)x)$/, `${label} malformed srcset descriptor`);
  }
};
assert.throws(() => validateSrcset("/safe.png 2q", "self-test"), /malformed srcset descriptor/);
assert.throws(() => validateSrcset("/safe.png 1wgarbage", "self-test"), /malformed srcset descriptor/);
assert.throws(() => validateSrcset("/safe.png 1x extra", "self-test"), /unsafe srcset candidate/);
const decodeSvgData = (value) => {
  assert.match(value, /^data:image\/svg\+xml(?:;charset=[^;,]+)?(?:;base64)?,/i, "only SVG favicon data URLs are allowed");
  const comma = value.indexOf(",");
  const metadata = value.slice(0, comma);
  const payload = value.slice(comma + 1);
  let svg;
  try {
    if (/;base64$/i.test(metadata)) {
      assert.match(payload, /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/, "favicon SVG base64 must be canonical");
      const bytes = Buffer.from(payload, "base64");
      assert.equal(bytes.toString("base64"), payload, "favicon SVG base64 must round-trip");
      svg = bytes.toString("utf8");
    } else svg = decodeURIComponent(payload);
  } catch { assert.fail("favicon SVG data URL must decode"); }
  for (const [pattern, label] of [...forbiddenHtml, [/<script\b/i, "SVG script"], [/<foreignObject\b/i, "SVG foreignObject"]]) assert.doesNotMatch(svg, pattern, `unsafe favicon ${label}`);
  for (const ref of svg.matchAll(/\b(?:href|xlink:href)=(?:"([^"]*)"|'([^']*)')/gi)) {
    const value = ref[1] ?? ref[2];
    assert.ok(isLocalResource(value) || value.startsWith("#"), `unsafe favicon SVG reference: ${value}`);
  }
  for (const cssUrl of svg.matchAll(/url\(\s*["']?([^"')]+)["']?\s*\)/gi)) assert.ok(isLocalResource(cssUrl[1].trim()), `unsafe favicon SVG CSS URL: ${cssUrl[1]}`);
};
for (const [pageName, content] of Object.entries(allEditorialCopies)) {
  for (const [pattern, label] of forbiddenHtml) assert.doesNotMatch(content, pattern, `${pageName} contains ${label}`);
  for (const tag of content.matchAll(/<(script|link|img|iframe|audio|video|source|object|embed)\b([^>]*)>/gi)) {
    const [, element, attrs] = tag;
    for (const attr of attrs.matchAll(/\b(src|srcset|href|data|poster)=(?:"([^"]*)"|'([^']*)')/gi)) {
      const name = attr[1].toLowerCase(); const value = attr[2] ?? attr[3];
      if (name === "srcset") validateSrcset(value, pageName);
      else if (value.startsWith("data:")) {
        assert.ok(element.toLowerCase() === "link" && name === "href" && /\brel="icon"/i.test(attrs), `${pageName} active data URL`);
        decodeSvgData(value);
      } else assert.ok(isLocalResource(value), `${pageName} unsafe ${element}[${name}]: ${value}`);
    }
  }
  for (const ref of content.matchAll(/<(?:use|image|feImage)\b[^>]*(?:href|xlink:href)=(?:"([^"]*)"|'([^']*)')/gi)) {
    const value = ref[1] ?? ref[2];
    assert.ok(isLocalResource(value), `${pageName} unsafe SVG reference: ${value}`);
  }
  for (const match of content.matchAll(/url\(\s*["']?([^"')]+)["']?\s*\)/gi)) {
    const value = match[1].trim();
    assert.ok(/^data:font\/woff2;base64,/i.test(value) || isLocalResource(value), `${pageName} unsafe CSS url()`);
  }
  for (const anchor of content.matchAll(/<a\b([^>]*)>/gi)) {
    if (/\btarget="_blank"/i.test(anchor[1])) assert.match(anchor[1], /\brel="[^"]*noopener[^"]*"/i, `${pageName} blank-target link needs noopener`);
  }
}

const productDocs = { README: await read(resolve(root, "README.md")), productBrief: await read(resolve(root, "docs/product/product-brief.md")) };
const claimAuditInputs = { ...pageContents, ...productDocs };
const forbiddenClaims = [
  /\buncensorable\b/i, /\bunstoppable\b/i, /impossible to shut down/i, /cannot be shut down/i,
  /nothing (?:anyone|anybody|someone) can (?:seize|pressure|switch off)/i, /\balways available\b/i,
  /works? from zero signal/i, /nothing (?:gets|is) (?:silently )?lost/i, /\bpreserves? everything\b/i,
  /\bguaranteed (?:delivery|discovery|synchroni[sz]ation|persistence|recovery|availability)\b/i,
  /\banonymous by default\b/i, /\bprivate by default\b/i, /\bproduction[- ]ready\b/i,
  /\bfield[- ]proven\b/i, /\boperating at scale\b/i, /\bno company that can revoke access\b/i,
  /\ba raid on one address cannot take a community's data\b/i,
];
for (const [name, content] of Object.entries(claimAuditInputs)) for (const pattern of forbiddenClaims) assert.doesNotMatch(content, pattern, `${name} contains unsafe absolute claim ${pattern}`);
for (const [name, content] of Object.entries(productDocs)) {
  assert.match(content, /Private groups[\s\S]{0,200}Direction, not shipped/i, `${name} must mark private groups unshipped`);
  assert.doesNotMatch(content, /no server to (?:raid|seize)/i, `${name} must qualify seizure resistance`);
  assert.match(content, /participant-held copies/i, `${name} must name participant-held copies`);
  assert.match(content, /does not guarantee/i, `${name} must state the missing guarantee`);
}

// --- Why Riot: human-capacity narrative + deterministic status --------------
assert.match(whyRiot, /<link rel="canonical" href="\/why-riot\/">/i);
assert.match(whyRiot, /<h1>People are the infrastructure\.<\/h1>/i);
assert.match(whyRiot, /Every day, people make a community through meals, meetings, stories, decisions, celebrations, care, and shared work\./i);
for (const heading of ["A community is something people do", "Tools for the commons", "The future is a practice", "More than one path", "Honest boundaries", "Build it with us"]) {
  assert.ok(whyRiot.includes(heading), `why-riot missing section: ${heading}`);
}
assert.match(whyRiot, /<svg\b[^>]*(?:aria-hidden="true"|role="img")[\s\S]*?<\/svg>/i, "Why Riot needs a code-native accessible illustration");
assert.doesNotMatch(whyRiot, /<script\b/i, "Why Riot must be script-free");
assert.doesNotMatch(whyRiot, /ecosystem/i, "why-riot must not use 'ecosystem' jargon");

const exactStatusText = { prototype: "Available in the prototype", local: "Tested locally", development: "In development", direction: "Direction, not shipped" };
const allowedStatusText = new Set(Object.values(exactStatusText));
for (const [, text] of home.matchAll(/<span\s+class="chip[^"]*"[^>]*>([^<]+)<\/span>/gi)) {
  assert.ok(allowedStatusText.has(text.trim()), `homepage status label must use the approved taxonomy: ${text.trim()}`);
}
const toolsBlock = block(whyRiot, /<section\b[^>]*id="tools"[^>]*>[\s\S]*?<\/section>/i, "Why Riot tools section");
const capabilityArticle = (capability) => block(toolsBlock, new RegExp(`<article\\b[^>]*data-capability="${capability}"[^>]*>[\\s\\S]*?<\\/article>`, "i"), `${capability} capability`);
const chipMatches = (html) => [...html.matchAll(/<span\s+class="chip"\s+data-status="(prototype|local|development|direction)">([^<]+)<\/span>/gi)].map(([, status, text]) => ({ status, text: text.trim() }));
for (const [capability, phrase] of [
  ["publish", "signed public updates and community media"],
  ["meet", "meeting artifacts, polls, discussion, decisions, and a shared record"],
  ["coordinate", "checklist, supply board, roll call, and quick poll"],
]) {
  const article = capabilityArticle(capability);
  assert.ok(article.toLowerCase().includes(phrase), `${capability} exact claim drift`);
  assert.deepEqual(chipMatches(article), [{ status: "prototype", text: exactStatusText.prototype }], `${capability} status drift`);
}
assert.match(capabilityArticle("meet"), /not live audio or video/i);
const carry = capabilityArticle("carry");
assert.equal(chipMatches(carry).length, 6, "Carry must have exactly six path statuses");
const carryExpected = new Map([["local-state", "prototype"], ["bundle-file", "prototype"], ["community-reference", "prototype"], ["nearby", "local"], ["gateway", "prototype"], ["anchors", "development"]]);
const carryRows = [...carry.matchAll(/<li\b[^>]*data-carry-path="([^"]+)"[^>]*>([\s\S]*?)<\/li>/gi)];
assert.equal(carryRows.length, carryExpected.size, "Carry path count drift");
for (const [, path, row] of carryRows) {
  assert.ok(carryExpected.has(path), `unexpected Carry path: ${path}`);
  assert.deepEqual(chipMatches(row), [{ status: carryExpected.get(path), text: exactStatusText[carryExpected.get(path)] }], `${path} status drift`);
}
assert.match(carry, /export and import a Riot bundle file/i);
assert.match(carry, /share a community reference by link or QR/i);
assert.match(carry, /onboarding\/reference, not proof that content moved by radio/i);
assert.match(whyRiot, /A community should be able to leave a provider without leaving one another\.[\s\S]*Direction, not shipped/i);
for (const phrase of ["plaintext", "readable", "copyable", "private encrypted groups are not shipped", "pseudonymity is not anonymity", "signature proves control of a key", "functioning device", "compatible peer or transport"]) {
  assert.ok(whyRiot.toLowerCase().includes(phrase.toLowerCase()), `why-riot missing boundary: ${phrase}`);
}
const boundaries = block(whyRiot, /<section\b[^>]*id="boundaries"[^>]*>[\s\S]*?<\/section>/i, "Why Riot boundaries");
for (const href of ["/privacy/", "/protocols/", "https://signal.org/"]) assert.ok(boundaries.includes(`href="${href}"`), `Why Riot boundaries must link ${href}`);
assert.match(whyRiot, /Rebecca Solnit[\s\S]*A Paradise Built in Hell[\s\S]*penguinrandomhouse\.com/i);
for (const href of ["/guide/", "/community/", "/releases/", "https://github.com/rabble/riot"]) assert.ok(whyRiot.includes(`href="${href}"`), `Why Riot invitation must link ${href}`);

// --- Privacy: public-first factual reference --------------------------------
assert.match(privacy, /<link rel="canonical" href="\/privacy\/">/i);
const privacyMarkers = ["Public means public", "What local-first changes—and what it does not", "This website", "Where to go next"];
let privacyCursor = -1;
for (const marker of privacyMarkers) { const at = privacy.indexOf(marker); assert.ok(at > privacyCursor, `Privacy section missing or out of order: ${marker}`); privacyCursor = at; }
for (const phrase of ["plaintext", "readable", "copyable", "no confidential public-read boundary", "private encrypted groups", "Cloudflare", "request metadata"]) assert.ok(privacy.toLowerCase().includes(phrase.toLowerCase()), `Privacy missing: ${phrase}`);
for (const href of ["/why-riot/", "/protocols/", "https://signal.org/"]) assert.ok(privacy.includes(`href="${href}"`), `Privacy must link ${href}`);
assert.doesNotMatch(privacy, /<script\b/i, "Privacy must be script-free");

// Using Riot is a task manual for the current app: linked contents, per-task
// offline/connection notes, platform labels, and an explicit not-yet list.
for (const phrase of [
  "Back to contents",
  "Works offline",
  "Needs a connection or permission",
  "What is not available yet",
  "Troubleshooting",
]) {
  assert.ok(guide.includes(phrase), `guide missing manual structure: ${phrase}`);
}
for (const platform of ["iOS", "macOS", "Android"]) {
  assert.ok(guide.includes(platform), `guide missing platform notes for ${platform}`);
}

// --- Sitemap + robots --------------------------------------------------------
// Static crawl helpers live in the deployment mirror (no source copy; they are
// deployment artifacts). Sitemap must list exactly the site's pages.
const sitemap = await read(resolve(root, "marketing/public/sitemap.xml"));
const robots = await read(resolve(root, "marketing/public/robots.txt"));
const sitemapUrls = [...sitemap.matchAll(/<loc>([^<]+)<\/loc>/gi)].map(([, href]) => new URL(href));
for (const url of sitemapUrls) assert.equal(url.origin, "https://riot-protest-net-marketing.protestnet.workers.dev", "sitemap origin drift");
const sitemapPaths = sitemapUrls.map((url) => normalizeLocalRoute(url.href, { allowExternal: true })).filter(Boolean);
assertExactRoutes(sitemapPaths, allSitePaths, "sitemap", { ordered: true });
const marketingReadme = await read(resolve(root, "marketing/README.md"));
const routesSection = block(marketingReadme, /## Routes\s*([\s\S]*?)(?=\n##\s|$)/, "marketing README Routes section");
const readmeRoutes = [...routesSection.matchAll(/^- `([^`]+)`/gm)].map(([, href]) => normalizeLocalRoute(href));
assertExactRoutes(readmeRoutes, allSitePaths, "marketing README route inventory", { ordered: true });
await expectEnoent(resolve(root, "marketing/resilience"));
await expectEnoent(resolve(root, "marketing/public/resilience"));
for (const content of [...Object.values(pageContents), sitemap, marketingReadme]) assert.ok(!localRoutesFrom(content).includes("/resilience/"), "site must not link /resilience/");
assert.match(robots, /User-agent: \*/i, "robots.txt must allow all user-agents");
assert.match(robots, /Sitemap:\s+https:\/\/riot-protest-net-marketing\.protestnet\.workers\.dev\/sitemap\.xml/i, "robots.txt must reference the sitemap");

// ---------- paired five-beat story (app + website must agree) ----------
// The homepage "How it works" section opens with an ordered five-beat primer
// that mirrors OnboardingExplainerStory in the app, so a person who meets Riot
// on the web or in first run hears the same trust boundaries in the same order.
const pairedStory = [
  {
    title: "No central account or publishing server",
    required: ["cryptographic key", "single place Riot must publish"],
  },
  {
    title: "Publishing moves peer to peer",
    required: ["does not mean anonymous", "observe connections"],
  },
  {
    title: "Many mirrors, not one site",
    required: [
      "display altered text",
      "false attribution",
      "accepts as the claimed author",
    ],
  },
  {
    title: "Signed records, checked in the app",
    required: ["independently synced record", "not whether its claims are true"],
  },
  {
    title: "Web for reach; the app for provenance",
    required: ["instead of trusting what a mirror displayed"],
  },
];

const howSection = home.match(/<section id="how">([\s\S]*?)<\/section>/)?.[1] ?? "";
assert.match(howSection, /<h2>One story, wherever you meet Riot<\/h2>/, "how-section heading must introduce the paired story");

const primer = howSection.match(/<ol class="story-beats"[^>]*>([\s\S]*?)<\/ol>/)?.[1] ?? "";
const htmlBeats = [...primer.matchAll(/<li class="story-beat">([\s\S]*?)<\/li>/g)].map((m) => m[1]);
assert.equal(htmlBeats.length, 5, "primer must contain five semantic list items");

const swiftBeats = [...swiftStory.matchAll(/OnboardingExplainerPoint\(\s*title: "([^"]+)",\s*body: "([^"]+)"/g)].map(([, title, body]) => ({ title, body }));
assert.equal(swiftBeats.length, 5, "Swift story must contain five explicit points");

for (const [index, { title, required }] of pairedStory.entries()) {
  const htmlBeat = htmlBeats[index];
  const swiftBeat = swiftBeats[index];
  assert.equal(swiftBeat.title, title, `Swift beat ${index + 1} title drift`);
  assert.match(
    htmlBeat,
    new RegExp(`<h3>${title.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}</h3>`),
    `homepage beat ${index + 1} heading drift`,
  );
  assert.match(htmlBeat, /<p>[\s\S]+<\/p>/, `homepage beat ${index + 1} needs body copy`);
  for (const phrase of required) {
    assert.ok(swiftBeat.body.includes(phrase), `Swift beat ${index + 1} missing: ${phrase}`);
    assert.ok(htmlBeat.includes(phrase), `homepage beat ${index + 1} missing: ${phrase}`);
  }
}

// The paired story exists to retire these unsafe claims. They must not return
// anywhere in the native presentation or the homepage.
for (const rejected of [
  "safe to read from",
  "cannot alter it",
  "app is proof",
  "proof stays in the app",
  "No servers, no accounts",
]) {
  assert.ok(!swiftStory.includes(rejected), `unsafe Swift story claim: ${rejected}`);
  assert.ok(!swiftPresentation.includes(rejected), `unsafe Swift presentation claim: ${rejected}`);
  assert.ok(!home.includes(rejected), `unsafe homepage claim: ${rejected}`);
}

// The primer deliberately does not use progressive enhancement: it must render
// identically with JavaScript disabled. The existing workflow may keep .reveal.
assert.match(
  home,
  /<div class="workflow-head[^"]*"[^>]*>[\s\S]*<h3>What you do<\/h3>[\s\S]*What actually happens, screen by screen/,
  "the workflow divider must separate the primer from the screen-by-screen steps",
);
assert.doesNotMatch(
  home,
  /<ol class="story-beats[^"]*\breveal\b/,
  "paired primer must remain visible without JavaScript",
);

// ---------- browser-authoritative local artifact checks ---------------------
const publicRoot = resolve(root, "marketing/public");
const contentTypes = { ".html": "text/html; charset=utf-8", ".png": "image/png", ".xml": "application/xml; charset=utf-8", ".txt": "text/plain; charset=utf-8" };
const server = createServer(async (request, response) => {
  try {
    const pathname = decodeURIComponent(new URL(request.url, "http://local.invalid").pathname);
    const relative = pathname === "/" ? "index.html" : pathname.endsWith("/") ? `${pathname.slice(1)}index.html` : pathname.slice(1);
    const safeRelative = normalize(relative);
    if (safeRelative.startsWith("..") || safeRelative.includes("\0")) { response.writeHead(400).end("bad path"); return; }
    const file = join(publicRoot, safeRelative);
    const body = await readFile(file);
    response.writeHead(200, { "content-type": contentTypes[extname(file)] ?? "application/octet-stream", "cache-control": "no-store" });
    response.end(body);
  } catch (error) {
    response.writeHead(error?.code === "ENOENT" ? 404 : 500, { "content-type": "text/plain; charset=utf-8" });
    response.end(error?.code === "ENOENT" ? "not found" : "server error");
  }
});
await new Promise((resolveListen, rejectListen) => { server.once("error", rejectListen); server.listen(0, "127.0.0.1", resolveListen); });
const address = server.address();
assert.ok(address && typeof address === "object");
const previewOrigin = `http://127.0.0.1:${address.port}`;
const browserEvidence = { origin: previewOrigin, routes: [], resilience: null };
let browser;
try {
  browser = await chromium.launch({ headless: true });
  for (const route of allSitePaths) {
    const context = await browser.newContext();
    const page = await context.newPage();
    const requests = [];
    const responses = [];
    const responseTasks = [];
    page.on("request", (request) => requests.push(request.url()));
    page.on("response", (response) => {
      responseTasks.push(response.allHeaders().then((headers) => {
        responses.push({ url: response.url(), status: response.status(), headers: Object.entries(headers).sort(([a], [b]) => a.localeCompare(b)) });
      }));
    });
    try {
      const cookiesBefore = await context.cookies();
      assert.deepEqual(cookiesBefore, [], `${route} cookie jar must start empty`);
      const navigation = await page.goto(`${previewOrigin}${route}`, { waitUntil: "networkidle" });
      assert.equal(navigation?.status(), 200, `${route} must return 200`);
      assert.equal(navigation?.request().redirectedFrom(), null, `${route} must not redirect`);
      await page.evaluate(async () => { for (let y = 0; y < document.documentElement.scrollHeight; y += Math.max(innerHeight, 400)) { scrollTo(0, y); await new Promise((done) => requestAnimationFrame(() => done())); } scrollTo(0, document.documentElement.scrollHeight); });
      await page.waitForLoadState("networkidle");
      await Promise.all(responseTasks);
      const dom = await page.evaluate(() => ({
        cookie: document.cookie,
        resources: performance.getEntriesByType("resource").map((entry) => entry.name).sort(),
        anchors: [...document.querySelectorAll("a[href]")].map((anchor) => ({ raw: anchor.getAttribute("href"), resolved: anchor.href })),
        resourceUrls: [...document.querySelectorAll("script[src],link[href],img[src],iframe[src],audio[src],video[src],video[poster],source[src],object[data],embed[src],svg use[href],svg image[href],svg feImage[href],svg use[xlink\\:href],svg image[xlink\\:href],svg feImage[xlink\\:href]")].flatMap((element) => [element.src, element.href?.baseVal ?? element.href, element.getAttribute("xlink:href"), element.data, element.poster].filter((value) => typeof value === "string" && value)),
      }));
      assert.equal(dom.cookie, "", `${route} document.cookie must be empty`);
      const cookiesAfter = await context.cookies();
      assert.deepEqual(cookiesAfter, [], `${route} cookie jar must remain empty`);
      for (const url of requests) assert.equal(new URL(url).origin, previewOrigin, `${route} made off-origin request: ${url}`);
      for (const response of responses) assert.ok(!response.headers.some(([name]) => name.toLowerCase() === "set-cookie"), `${route} response set a cookie`);
      for (const anchor of dom.anchors) {
        if (anchor.raw?.startsWith("#")) continue;
        assert.ok(["http:", "https:"].includes(new URL(anchor.resolved).protocol), `${route} unsafe anchor: ${anchor.raw}`);
      }
      for (const url of [...dom.resources, ...dom.resourceUrls]) {
        if (url.startsWith("data:image/svg+xml")) continue;
        assert.equal(new URL(url, previewOrigin).origin, previewOrigin, `${route} resolved off-origin resource: ${url}`);
      }
      browserEvidence.routes.push({ route, cookiesBefore, cookiesAfter, documentCookie: dom.cookie, requests: [...new Set(requests)].sort(), responses: responses.sort((a, b) => a.url.localeCompare(b.url)), resources: dom.resources });
    } finally { await context.close(); }
  }
  const response = await fetch(`${previewOrigin}/resilience/`, { redirect: "manual" });
  browserEvidence.resilience = { status: response.status, location: response.headers.get("location") };
  assert.equal(response.status, 404, "/resilience/ must return a direct 404");
  assert.equal(response.headers.get("location"), null, "/resilience/ must not redirect");
} finally {
  if (browser) await browser.close();
  await new Promise((resolveClose, rejectClose) => server.close((error) => error ? rejectClose(error) : resolveClose()));
}
const evidenceDir = "/tmp/visual-review/riot-human-capacity";
await mkdir(evidenceDir, { recursive: true });
await writeFile(resolve(evidenceDir, "browser-evidence.json"), `${JSON.stringify(browserEvidence, null, 2)}\n`);

console.log("protocol marketing contracts: PASS");
