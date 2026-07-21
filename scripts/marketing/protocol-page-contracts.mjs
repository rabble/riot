import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

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
const readAll = async (obj) => {
  const entries = Object.entries(obj);
  const values = await Promise.all(entries.map(([, p]) => read(p)));
  return Object.fromEntries(entries.map(([k], i) => [k, values[i]]));
};
const {
  home, publicHome, protocols, publicProtocols,
  about, publicAbout, privacy, publicPrivacy,
  "open-source": openSource, publicOpenSource, community, publicCommunity,
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
assert.match(home, /class="device-scene"[\s\S]*class="phone-frame main"[\s\S]*\/assets\/screenshots\/spaces\.png/i);

// ---------- reframed homepage (offline-guides design, 2026-07-20) ----------
// The homepage tells the paired story in eight ordered sections. Order is the
// contract: hero → communities → partners → builders → paths → privacy →
// status → learn. Extra sections (how, evidence, builder) may interleave but
// never reorder the eight.
{
  const orderedMarkers = [
    "Community infrastructure that travels with people",
    '<section id="communities">',
    '<section id="screens">',
    '<section id="partners"',
    '<section id="builders">',
    '<section id="paths">',
    '<section id="privacy">',
    '<section id="status"',
    '<section id="learn">',
  ];
  let cursor = -1;
  for (const marker of orderedMarkers) {
    const at = home.indexOf(marker);
    assert.ok(at > cursor, `homepage section out of order or missing: ${marker}`);
    cursor = at;
  }
}
// Visible prototype label in the hero, per the design's honesty rules.
assert.match(home, /class="hero-stamp">Prototype/i, "hero must carry a visible Prototype label");
// Before/after comparison with per-row scope labels.
assert.match(home, /<table class="contrast">[\s\S]*Participants carry community state[\s\S]*Nearby exchange can continue locally[\s\S]*Willow data carries community continuity/i, "hero must contain the conventional-vs-Riot comparison");
// Claims are labeled where they first appear, not only in a closing block.
for (const phrase of [
  "not yet a live sync server",
  "Plaintext by design",
  "pseudonymity is not anonymity",
  "planned separately",
  "Physical two-iPhone Bluetooth remains unverified",
  "Working in the prototype",
  "Direction being built or still unverified",
]) {
  assert.ok(home.includes(phrase), `homepage missing honest boundary: ${phrase}`);
}
// Voice rule: no startup/platform "ecosystem" jargon anywhere in the copy.
assert.doesNotMatch(home, /ecosystem/i, "homepage must not use 'ecosystem' jargon");
// The reframed homepage is fully static — the old reveal script is gone.
assert.doesNotMatch(home, /<script/i, "homepage must not contain JavaScript");
// Both guides are reachable from the homepage.
for (const guidePath of ["/why-riot/", "/guide/"]) {
  assert.ok(home.includes(`href="${guidePath}"`), `homepage must link to ${guidePath}`);
}
for (const name of ["apps", "compose", "checklist"]) {
  assert.match(home, new RegExp(`class="phone-frame thumb"[\\s\\S]*?/assets/screenshots/${name}\\.png`, "i"), `missing ${name} supporting phone`);
}
assert.match(home, /Real app screens[\s\S]*iPhone simulator build/i);
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

// --- Unified footer nav across all pages -------------------------------------
// Every page's footer must link to every OTHER site page so a visitor on any
// route can reach all the others. A page may omit its own self-link (a "Home"
// link on the home page adds nothing). The link set is the single source of
// truth for "all pages".
const allSitePaths = ["/", "/why-riot/", "/guide/", "/about/", "/privacy/", "/open-source/", "/community/", "/releases/", "/protocols/"];
const pageOwnPath = { home: "/", protocols: "/protocols/", about: "/about/", privacy: "/privacy/", "open-source": "/open-source/", community: "/community/", releases: "/releases/", "why-riot": "/why-riot/", guide: "/guide/" };
const pageContents = { home, protocols, about, privacy, "open-source": openSource, community, releases, "why-riot": whyRiot, guide };
for (const [pageName, content] of Object.entries(pageContents)) {
  for (const sitePath of allSitePaths) {
    if (sitePath === pageOwnPath[pageName]) continue;
    assert.ok(
      content.includes(`href="${sitePath}"`),
      `${pageName} page footer must link to ${sitePath}`,
    );
  }
}

// --- Site-wide top nav -------------------------------------------------------
// Every page must carry the cross-page site nav (sticky top bar) so the sibling
// pages are reachable and always visible. Footer-only nav was a discoverability
// failure; this guard prevents regression. The nav uses `.sitenav-links` inside
// a sticky `.sitenav` so the important cross-page links never scroll away.
for (const [pageName, content] of Object.entries(pageContents)) {
  assert.match(content, /class="sitenav-links"/, `${pageName} page must include the site-wide sticky nav (sitenav-links)`);
  assert.match(content, /\.sitenav\s*\{[^}]*position:\s*sticky/i, `${pageName} page must make the sitenav sticky`);
  const navMatch = content.match(/<nav[^>]*class="sitenav"[^>]*>[\s\S]*?<\/nav>/i);
  assert.ok(navMatch, `${pageName} page must have a sitenav <nav> element`);
  const navBlock = navMatch[0];
  for (const sitePath of allSitePaths) {
    if (sitePath === pageOwnPath[pageName]) continue;
    assert.ok(
      navBlock.includes(`href="${sitePath}"`),
      `${pageName} page sticky nav must link to ${sitePath}`,
    );
  }
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

// --- Guide pages: paired-story depths + honest boundaries --------------------
// Why Riot is one story at three depths; each depth is labeled, and the
// privacy/status boundaries appear in the page, not only in a closing block.
for (const marker of [
  "for communities and organizers",
  "for partners, funders, and journalists",
  "for builders and protocol readers",
]) {
  assert.ok(whyRiot.toLowerCase().includes(marker), `why-riot missing audience depth label: ${marker}`);
}
for (const phrase of [
  "Privacy through control, not secrecy",
  "Plaintext by design",
  "not an anonymity guarantee",
  "cannot promise anonymity",
  "not yet a live sync server",
  "One update, different paths",
  "Working in the prototype",
  "Direction being built or still unverified",
  "does not use Meadowcap as a confidentiality boundary",
]) {
  assert.ok(whyRiot.includes(phrase), `why-riot missing honest boundary: ${phrase}`);
}
assert.doesNotMatch(whyRiot, /ecosystem/i, "why-riot must not use 'ecosystem' jargon");

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
for (const sitePath of allSitePaths) {
  assert.ok(sitemap.includes(`<loc>https://riot-protest-net-marketing.protestnet.workers.dev${sitePath}</loc>`), `sitemap must list ${sitePath}`);
}
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

console.log("protocol marketing contracts: PASS");
