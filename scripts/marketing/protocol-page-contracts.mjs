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
};

const read = async (path) => readFile(path, "utf8");
const [home, publicHome, protocols, publicProtocols] = await Promise.all([
  read(paths.home),
  read(paths.publicHome),
  read(paths.protocols),
  read(paths.publicProtocols),
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
assert.match(home, /Riot and the Willow implementation inside it are being built by[\s\S]{0,180}>@rabble</i, "credit Riot's Willow implementation to @rabble");
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

console.log("protocol marketing contracts: PASS");
