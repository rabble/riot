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

const homeLinks = home.match(/href="\/protocols\/"/g) ?? [];
assert.equal(homeLinks.length, 2, "homepage must contain exactly two quiet /protocols/ links");
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
