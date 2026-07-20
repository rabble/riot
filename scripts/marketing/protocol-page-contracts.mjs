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

const read = async (path) => readFile(path, "utf8");
const [home, publicHome, protocols, publicProtocols, swiftStory, swiftPresentation] = await Promise.all([
  read(paths.home),
  read(paths.publicHome),
  read(paths.protocols),
  read(paths.publicProtocols),
  read(paths.swiftStory),
  read(paths.swiftPresentation),
]);

assert.equal(home, publicHome, "homepage source and public mirror must be byte-identical");
assert.equal(protocols, publicProtocols, "protocol page source and public mirror must be byte-identical");

for (const name of ["spaces", "apps", "compose", "checklist"]) {
  const [sourceAsset, publicAsset] = await Promise.all([
    readFile(resolve(root, `marketing/assets/screenshots/${name}.png`)),
    readFile(resolve(root, `marketing/public/assets/screenshots/${name}.png`)),
  ]);
  assert.deepEqual(sourceAsset, publicAsset, `${name} screenshot source and public mirror must be byte-identical`);
}

assert.doesNotMatch(home, /hero-mesh|mesh-edges|mesh-nodes/, "approved Hero C must replace the abstract mesh");
assert.match(home, /\.hero-grid\s*\{[^}]*align-items:\s*start/i, "desktop hero copy and devices must be top-aligned");
assert.match(home, /class="device-scene"[\s\S]*class="phone-frame main"[\s\S]*\/assets\/screenshots\/spaces\.png/i);
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
assert.match(home, /<nav class="topnav">[\s\S]*href="#builder"[^>]*>Builder/i);
assert.match(home, /<footer[\s\S]*Built by[\s\S]*@rabble/i);

const homeLinks = home.match(/href="\/protocols\/"/g) ?? [];
assert.equal(homeLinks.length, 4, "homepage must contain four visible and secondary /protocols/ paths");
assert.match(home, /<nav class="topnav">[\s\S]*href="\/protocols\/"[^>]*>Protocols</i);
assert.match(home, /class="protocol-callout reveal"[\s\S]*Where does Riot fit\?[\s\S]*Compare Riot, Willow, and neighboring protocols/i);
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
assert.doesNotMatch(protocols, /(?:plausible|google-analytics|googletagmanager|segment\.com|mixpanel)/i, "protocol page must not include analytics");

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
