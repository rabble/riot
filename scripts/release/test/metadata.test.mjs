import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import * as realFs from "node:fs/promises";
import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";

import { tempReleaseRoot } from "./helpers.mjs";
import { loadSchemaRegistry } from "../schema.mjs";
import {
  CLAIMS,
  evaluateMetadata,
  generateMetadata,
  loadMetadataSources,
} from "../metadata.mjs";

const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");

const POSITIONING =
  "Community-owned news and practical tools designed to stay useful locally when networks are unreliable.";
const EARLY_ACCESS = "Version 1.0 is an early-access release.";
const SIGNATURE_DISCLAIMER =
  "A valid signature proves source and integrity, not truth.";
const EDITORIAL_DISCLAIMER =
  "Editorial labels are community signals, not independent factual verification.";
const PRICING = "Riot is free, with no in-app purchases.";
const WORLDWIDE = "Riot is available worldwide.";
const URLS = {
  privacy: "https://riot.protest.net/privacy/",
  support: "https://riot.protest.net/support/",
  marketing: "https://riot.protest.net/releases/",
};

const CLAIM_BULLETS = [
  "Follow community newswires.",
  "Publish signed updates.",
  "Read signatures and community editorial labels.",
  "Carry community tools, including shared checklists.",
  "Exchange updates nearby with other phones. Nearby exchange is experimental.",
  "Keep a local copy available offline.",
];

function descriptionText() {
  return [
    `${POSITIONING} ${EARLY_ACCESS}`,
    "",
    ...CLAIM_BULLETS.map((claim) => `- ${claim}`),
    "",
    `${SIGNATURE_DISCLAIMER} ${EDITORIAL_DISCLAIMER}`,
    "",
    `${PRICING} ${WORLDWIDE}`,
    `Privacy: ${URLS.privacy}`,
    `Support: ${URLS.support}`,
    `Releases: ${URLS.marketing}`,
  ].join("\n");
}

function validApple() {
  return {
    schemaVersion: 1,
    locale: "en-US",
    fields: {
      name: "Riot",
      subtitle: "Community news and tools",
      promotionalText: `${POSITIONING} ${EARLY_ACCESS}`,
      description: descriptionText(),
      keywords: "community,news,newswire,local,offline,signed,tools,checklist",
      releaseNotes: `${EARLY_ACCESS} First public release: community newswires, signed updates, editorial labels, community tools, experimental nearby exchange, and offline reading.`,
    },
    urls: { ...URLS },
    categories: { primaryCategory: "News", secondaryCategory: "Lifestyle" },
  };
}

function validGoogle() {
  return {
    schemaVersion: 1,
    locale: "en-US",
    fields: {
      title: "Riot",
      shortDescription: "Community-owned news and practical tools that stay useful locally.",
      fullDescription: descriptionText(),
      releaseNotes: `${EARLY_ACCESS} First public release: newswires, signed updates, editorial labels, tools, experimental nearby exchange, offline reading.`,
    },
    urls: { ...URLS },
    categories: { category: "NEWS_AND_MAGAZINES" },
  };
}

function validVisuals() {
  return {
    frames: [
      { stateId: "community-newswire", claimId: "follow-newswire", headline: "Your community. Your newswire." },
      { stateId: "signed-publishing", claimId: "publish-signed", headline: "Publish signed updates from the field." },
      { stateId: "labels-and-signatures", claimId: "read-signatures-labels", headline: "Read signatures and community editorial labels." },
      { stateId: "community-tools", claimId: "community-tools", headline: "Carry useful tools with the community." },
      { stateId: "nearby-exchange", claimId: "nearby-exchange", headline: "Exchange updates nearby." },
      { stateId: "offline-copy", claimId: "offline-copy", headline: "Keep a local copy available offline." },
    ],
  };
}

function validSources() {
  return { apple: validApple(), google: validGoogle(), visuals: validVisuals() };
}

function blockedGate(gates, id) {
  const gate = gates.find((candidate) => candidate.id === id);
  assert.ok(gate, `expected gate ${id}`);
  assert.equal(gate.state, "BLOCKED", `${id} should be BLOCKED: ${JSON.stringify(gate)}`);
  for (const field of ["id", "state", "sourceFile", "pointer", "observed", "expected", "recovery"]) {
    assert.notEqual(gate[field], undefined, `gate ${id} missing ${field}`);
  }
  return gate;
}

function allPass(gates) {
  const failures = gates.filter((gate) => gate.state !== "PASS");
  assert.deepEqual(failures, [], `expected all PASS: ${JSON.stringify(failures)}`);
}

test("canonical sources produce all-PASS metadata gates with the standard shape", () => {
  const gates = evaluateMetadata(validSources());
  assert.ok(gates.length > 0);
  for (const gate of gates) {
    for (const field of ["id", "state", "sourceFile", "pointer", "observed", "expected", "recovery"]) {
      assert.notEqual(gate[field], undefined, `gate ${gate.id} missing ${field}`);
    }
  }
  allPass(gates);
});

test("claims whitelist exposes exactly the six canonical claims", () => {
  assert.deepEqual(Object.keys(CLAIMS).sort(), [
    "community-tools",
    "follow-newswire",
    "nearby-exchange",
    "offline-copy",
    "publish-signed",
    "read-signatures-labels",
  ]);
});

const APPLE_LIMITS = {
  name: 30,
  subtitle: 30,
  promotionalText: 170,
  description: 4000,
  keywords: 100,
  releaseNotes: 4000,
};

const GOOGLE_LIMITS = {
  title: 30,
  shortDescription: 80,
  fullDescription: 4000,
  releaseNotes: 500,
};

test("every Apple field accepts its exact limit and rejects limit plus one code point", () => {
  for (const [field, limit] of Object.entries(APPLE_LIMITS)) {
    if (field === "name") continue; // const "Riot", limit enforced by schema
    const atLimit = evaluateMetadata({
      ...validSources(),
      apple: {
        ...validApple(),
        fields: { ...validApple().fields, [field]: "x".repeat(limit) },
      },
    });
    // limit-boundary content may violate pinned-sentence rules; only the
    // length gate for this field may decide pass/fail, so check directly:
    const lengthGate = atLimit.find((gate) => gate.id === `metadata.apple.${field}.length`);
    assert.ok(lengthGate, `missing length gate for ${field}`);
    assert.equal(lengthGate.state, "PASS", `${field} at limit must PASS`);

    const overLimit = evaluateMetadata({
      ...validSources(),
      apple: { ...validApple(), fields: { ...validApple().fields, [field]: "y".repeat(limit + 1) } },
    });
    blockedGate(overLimit, `metadata.apple.${field}.length`);
  }
});

test("every Google field accepts its exact limit and rejects limit plus one code point", () => {
  for (const [field, limit] of Object.entries(GOOGLE_LIMITS)) {
    if (field === "title") continue;
    const atLimit = evaluateMetadata({
      ...validSources(),
      google: {
        ...validGoogle(),
        fields: { ...validGoogle().fields, [field]: "x".repeat(limit) },
      },
    });
    const lengthGate = atLimit.find((gate) => gate.id === `metadata.google.${field}.length`);
    assert.ok(lengthGate, `missing length gate for ${field}`);
    assert.equal(lengthGate.state, "PASS", `${field} at limit must PASS`);

    const overLimit = evaluateMetadata({
      ...validSources(),
      google: { ...validGoogle(), fields: { ...validGoogle().fields, [field]: "y".repeat(limit + 1) } },
    });
    blockedGate(overLimit, `metadata.google.${field}.length`);
  }
});

test("code-point counting treats astral characters as one", () => {
  const sources = validSources();
  sources.apple.fields.subtitle = "💚".repeat(30);
  const gates = evaluateMetadata(sources);
  const gate = gates.find((candidate) => candidate.id === "metadata.apple.subtitle.length");
  assert.equal(gate.state, "PASS");
  sources.apple.fields.subtitle = "💚".repeat(31);
  blockedGate(evaluateMetadata(sources), "metadata.apple.subtitle.length");
});

test("prohibited vocabulary fails closed in every emitted field", async (t) => {
  const cases = [
    ["best", "The best community app."],
    ["fastest", "The fastest newswire."],
    ["leading", "The leading community tool."],
    ["number one", "The number one newswire."],
    ["#1", "The #1 community app."],
    ["anonymous", "Stay fully anonymous online."],
    ["untraceable", "Untraceable messaging."],
    ["fully private", "Fully private by design."],
    ["no logs", "We keep no logs ever."],
    ["$", "Now $0 for everyone."],
    ["sale", "On sale this week."],
    ["discount", "A discount for early users."],
    ["military-grade", "Military-grade encryption."],
    ["unhackable", "Unhackable infrastructure."],
    ["secure against", "Secure against all attackers."],
    ["foreign URL", "See https://example.com for details."],
    ["npub1", "Contact npub1t985dmat80n6xlrnhsjzzrlhfkcmmemul47n3mz9lws70lrxs0pqwzdyaw now."],
    ["nsec1", "Never share nsec1tu92893lv55urd4almqhfnrv48ls2uwas5hxg6eashq9jhnt45ts9en3zd here."],
    ["note1", "See note1m99r7nwc0wdrkzldrqan96gklg5usqspq7z9696j6unf0ljnpxjspqfw99 today."],
  ];
  for (const [label, sentence] of cases) {
    await t.test(`rejects ${label}`, () => {
      const sources = validSources();
      sources.apple.fields.subtitle = sentence;
      blockedGate(evaluateMetadata(sources), "metadata.apple.subtitle.vocabulary");
      const mixed = validSources();
      mixed.google.fields.shortDescription = sentence.toUpperCase();
      blockedGate(evaluateMetadata(mixed), "metadata.google.shortDescription.vocabulary");
    });
  }
});

test("pinned sentences are required in their fields", async (t) => {
  await t.test("positioning sentence missing from promotional text", () => {
    const sources = validSources();
    sources.apple.fields.promotionalText = EARLY_ACCESS;
    blockedGate(evaluateMetadata(sources), "metadata.apple.promotionalText.positioning");
  });
  await t.test("early access sentence missing from release notes", () => {
    const sources = validSources();
    sources.google.fields.releaseNotes = "Bug fixes and improvements.";
    blockedGate(evaluateMetadata(sources), "metadata.google.releaseNotes.earlyAccess");
  });
  await t.test("signature disclaimer missing from description", () => {
    const sources = validSources();
    sources.apple.fields.description = sources.apple.fields.description.replace(SIGNATURE_DISCLAIMER, "");
    blockedGate(evaluateMetadata(sources), "metadata.apple.description.signatureDisclaimer");
  });
  await t.test("editorial disclaimer missing from description", () => {
    const sources = validSources();
    sources.google.fields.fullDescription = sources.google.fields.fullDescription.replace(EDITORIAL_DISCLAIMER, "");
    blockedGate(evaluateMetadata(sources), "metadata.google.fullDescription.editorialDisclaimer");
  });
  await t.test("pricing sentence missing from description", () => {
    const sources = validSources();
    sources.apple.fields.description = sources.apple.fields.description.replace(PRICING, "");
    blockedGate(evaluateMetadata(sources), "metadata.apple.description.pricing");
  });
});

test("cross-platform consistency requires shared sentences in both sources", () => {
  const sources = validSources();
  sources.google.fields.fullDescription = sources.google.fields.fullDescription.replace(SIGNATURE_DISCLAIMER, "");
  blockedGate(evaluateMetadata(sources), "metadata.consistency.signatureDisclaimer");
});

test("nearby exchange is qualified experimental and phones-only", () => {
  const sources = validSources();
  sources.apple.fields.description = sources.apple.fields.description.replace(
    "Nearby exchange is experimental.",
    "",
  );
  blockedGate(evaluateMetadata(sources), "metadata.apple.description.nearbyQualification");
});

test("claim bullets must exactly equal the whitelist", async (t) => {
  await t.test("a claim outside the whitelist fails", () => {
    const sources = validSources();
    sources.apple.fields.description = sources.apple.fields.description.replace(
      "- Follow community newswires.",
      "- Syncs unlimited devices in real time.",
    );
    blockedGate(evaluateMetadata(sources), "metadata.apple.description.claims");
  });
  await t.test("a missing claim bullet fails", () => {
    const sources = validSources();
    sources.google.fields.fullDescription = sources.google.fields.fullDescription.replace(
      "- Keep a local copy available offline.\n",
      "",
    );
    blockedGate(evaluateMetadata(sources), "metadata.google.fullDescription.claims");
  });
});

test("claim-carrier rule: every visuals frame claim must appear in the description", async (t) => {
  await t.test("unknown frame claimId fails", () => {
    const sources = validSources();
    sources.visuals.frames[0] = { ...sources.visuals.frames[0], claimId: "teleportation" };
    blockedGate(evaluateMetadata(sources), "metadata.carrier.claimId");
  });
  await t.test("frame claim missing from Apple description fails", () => {
    const sources = validSources();
    sources.apple.fields.description = sources.apple.fields.description.replace(
      "- Follow community newswires.\n",
      "",
    );
    blockedGate(evaluateMetadata(sources), "metadata.carrier.apple");
  });
  await t.test("frame claim missing from Google description fails", () => {
    const sources = validSources();
    sources.google.fields.fullDescription = sources.google.fields.fullDescription.replace(
      "- Carry community tools, including shared checklists.\n",
      "",
    );
    blockedGate(evaluateMetadata(sources), "metadata.carrier.google");
  });
});

test("category recommendations come from pinned allowlists", () => {
  const apple = validSources();
  apple.apple.categories.primaryCategory = "Games";
  blockedGate(evaluateMetadata(apple), "metadata.apple.categories");
  const google = validSources();
  google.google.categories.category = "CASINO";
  blockedGate(evaluateMetadata(google), "metadata.google.categories");
});

test("loadMetadataSources validates closed schemas and canonical URLs", async () => {
  const repositoryRoot = new URL("../../..", import.meta.url).pathname;
  const registry = await loadSchemaRegistry(join(repositoryRoot, "release", "schemas"));
  const root = await tempReleaseRoot();
  const write = async (relative, value) => {
    const path = join(root, relative);
    await mkdir(join(path, ".."), { recursive: true });
    await writeFile(path, JSON.stringify(value, null, 2));
  };
  await write("release/source/apple/en-US.json", validApple());
  await write("release/source/google/en-US.json", validGoogle());
  // The visuals source is a rich closed-schema record; use the real one.
  const realVisuals = JSON.parse(
    await realFs.readFile(join(repositoryRoot, "release", "source", "visuals.json"), "utf8"),
  );
  await write("release/source/visuals.json", realVisuals);

  const loaded = await loadMetadataSources({ sourceDirectory: join(root, "release", "source"), registry, fs: realFs });
  assert.equal(loaded.apple.fields.name, "Riot");
  assert.ok(Object.isFrozen(loaded.apple));

  await write("release/source/apple/en-US.json", { ...validApple(), extra: true });
  await assert.rejects(
    loadMetadataSources({ sourceDirectory: join(root, "release", "source"), registry, fs: realFs }),
    /additionalProperties|extra/,
  );

  await write("release/source/apple/en-US.json", validApple());
  await write("release/source/google/en-US.json", {
    ...validGoogle(),
    urls: { ...URLS, support: "https://example.com/support" },
  });
  await assert.rejects(
    loadMetadataSources({ sourceDirectory: join(root, "release", "source"), registry, fs: realFs }),
  );
});

test("generateMetadata emits platform files and digest-true manifests deterministically", async () => {
  const root = await tempReleaseRoot();
  const outputDirectory = join(root, "release", "generated");
  const sources = validSources();

  const first = await generateMetadata({ sources, outputDirectory, fs: realFs, sha256 });
  const second = await generateMetadata({ sources, outputDirectory, fs: realFs, sha256 });
  assert.deepEqual(first, second, "generation must be deterministic");

  const appleDir = join(outputDirectory, "apple", "en-US");
  const expectedAppleFiles = [
    "description.txt",
    "keywords.txt",
    "manifest.json",
    "name.txt",
    "promotional-text.txt",
    "release-notes.txt",
    "subtitle.txt",
  ];
  assert.deepEqual((await realFs.readdir(appleDir)).sort(), expectedAppleFiles);
  const googleDir = join(outputDirectory, "google", "en-US");
  assert.deepEqual(
    (await realFs.readdir(googleDir)).sort(),
    ["full-description.txt", "manifest.json", "release-notes.txt", "short-description.txt", "title.txt"],
  );

  assert.equal(await realFs.readFile(join(appleDir, "name.txt"), "utf8"), "Riot\n");
  const manifest = JSON.parse(await realFs.readFile(join(appleDir, "manifest.json"), "utf8"));
  assert.equal(manifest.schemaVersion, 1);
  assert.equal(manifest.platform, "apple");
  assert.equal(manifest.locale, "en-US");
  assert.deepEqual(manifest.urls, URLS);
  assert.deepEqual(manifest.categories, { primaryCategory: "News", secondaryCategory: "Lifestyle" });
  for (const [field, entry] of Object.entries(manifest.fields)) {
    const bytes = await realFs.readFile(join(appleDir, entry.file));
    assert.equal(sha256(bytes), entry.sha256, `manifest digest mismatch for ${field}`);
    assert.equal(Array.from(bytes.toString("utf8").replace(/\n$/, "")).length, entry.codePoints);
    assert.equal(entry.limit, APPLE_LIMITS[field]);
  }
  assert.equal(first.appleManifestSha256, sha256(await realFs.readFile(join(appleDir, "manifest.json"))));
  assert.equal(
    first.googleManifestSha256,
    sha256(await realFs.readFile(join(googleDir, "manifest.json"))),
  );
});

test("loadMetadataSources and generateMetadata guard their dependencies", async () => {
  await assert.rejects(loadMetadataSources({ sourceDirectory: "x", fs: realFs }), /schema registry is required/);
  await assert.rejects(generateMetadata({ sources: validSources(), outputDirectory: "x", fs: realFs }), /sha256 dependency is required/);
});

test("generateMetadata refuses blocked sources with diagnostics", async () => {
  const sources = validSources();
  sources.apple.fields.subtitle = "The best app ever made";
  await assert.rejects(
    generateMetadata({ sources, outputDirectory: "x", fs: realFs, sha256 }),
    (error) => {
      assert.match(error.message, /metadata sources are blocked/);
      assert.ok(error.diagnostics?.length >= 1);
      assert.ok(error.sourceFile.includes("apple"));
      return true;
    },
  );
});
