import { join } from "node:path";

import { canonicalJson } from "./canonical-json.mjs";
import { releaseDiagnosticError, validateSource } from "./schema.mjs";

export const POSITIONING_SENTENCE =
  "Community-owned news and practical tools designed to stay useful locally when networks are unreliable.";
export const EARLY_ACCESS_SENTENCE = "Version 1.0 is an early-access release.";
export const SIGNATURE_DISCLAIMER =
  "A valid signature proves source and integrity, not truth.";
export const EDITORIAL_DISCLAIMER =
  "Editorial labels are community signals, not independent factual verification.";
export const PRICING_SENTENCE = "Riot is free, with no in-app purchases.";
export const WORLDWIDE_SENTENCE = "Riot is available worldwide.";
export const NEARBY_QUALIFICATION = "Nearby exchange is experimental.";

export const CANONICAL_URLS = Object.freeze({
  privacy: "https://riot.protest.net/privacy/",
  support: "https://riot.protest.net/support/",
  marketing: "https://riot.protest.net/releases/",
});

// The only capability claims store copy may make. Each claimId maps to the
// exact description bullet that carries it; screenshot headlines reference
// claims by these IDs in release/source/visuals.json.
export const CLAIMS = Object.freeze({
  "follow-newswire": "Follow community newswires.",
  "publish-signed": "Publish signed updates.",
  "read-signatures-labels": "Read signatures and community editorial labels.",
  "community-tools": "Carry community tools, including shared checklists.",
  "nearby-exchange":
    "Exchange updates nearby with other phones. Nearby exchange is experimental.",
  "offline-copy": "Keep a local copy available offline.",
});

export const APPLE_FIELD_LIMITS = Object.freeze({
  name: 30,
  subtitle: 30,
  promotionalText: 170,
  description: 4000,
  keywords: 100,
  releaseNotes: 4000,
});

export const GOOGLE_FIELD_LIMITS = Object.freeze({
  title: 30,
  shortDescription: 80,
  fullDescription: 4000,
  releaseNotes: 500,
});

const APPLE_FIELD_FILES = Object.freeze({
  name: "name.txt",
  subtitle: "subtitle.txt",
  promotionalText: "promotional-text.txt",
  description: "description.txt",
  keywords: "keywords.txt",
  releaseNotes: "release-notes.txt",
});

const GOOGLE_FIELD_FILES = Object.freeze({
  title: "title.txt",
  shortDescription: "short-description.txt",
  fullDescription: "full-description.txt",
  releaseNotes: "release-notes.txt",
});

const APPLE_PRIMARY_CATEGORIES = Object.freeze(["News"]);
const APPLE_SECONDARY_CATEGORIES = Object.freeze([
  "Lifestyle",
  "Productivity",
  "Social Networking",
  "Utilities",
]);
const GOOGLE_CATEGORIES = Object.freeze(["NEWS_AND_MAGAZINES"]);

const APPLE_SOURCE = "release/source/apple/en-US.json";
const GOOGLE_SOURCE = "release/source/google/en-US.json";
const VISUALS_SOURCE = "release/source/visuals.json";
const CONSISTENCY_SOURCE = "release/source/apple/en-US.json + release/source/google/en-US.json";

// Case-insensitive prohibited vocabulary. Word-boundary regexes avoid
// false positives inside unrelated words.
const PROHIBITED_PATTERNS = Object.freeze([
  /\bbest\b/i,
  /\bfastest\b/i,
  /\bleading\b/i,
  /number one/i,
  /#1\b/,
  /\banonymous\b/i,
  /\buntraceable\b/i,
  /fully private/i,
  /no logs/i,
  /\$/,
  /\bsale\b/i,
  /\bdiscount\b/i,
  /military-grade/i,
  /\bunhackable\b/i,
  /secure against/i,
  /\b(?:npub|nsec|note)1[023456789acdefghjklmnpqrstuvwxyz]+\b/i,
]);

function gate(id, state, sourceFile, pointer, observed, expected, recovery) {
  return { id, state, sourceFile, pointer, observed, expected, recovery };
}

function pass(id, sourceFile, pointer, observed, expected) {
  return gate(id, "PASS", sourceFile, pointer, observed, expected, "No action required.");
}

function codePoints(value) {
  return Array.from(value).length;
}

function checkLength(gates, platform, sourceFile, field, value, limit) {
  const length = codePoints(value);
  const id = `metadata.${platform}.${field}.length`;
  if (length > limit) {
    gates.push(gate(
      id,
      "BLOCKED",
      sourceFile,
      `/fields/${field}`,
      length,
      `at most ${limit} code points`,
      `Shorten the ${platform} ${field} field to fit the store limit.`,
    ));
  } else {
    gates.push(pass(id, sourceFile, `/fields/${field}`, length, `at most ${limit} code points`));
  }
}

function checkVocabulary(gates, platform, sourceFile, field, value) {
  const id = `metadata.${platform}.${field}.vocabulary`;
  const withoutCanonicalUrls = Object.values(CANONICAL_URLS)
    .reduce((text, url) => text.split(url).join(""), value);
  const urlMatch = withoutCanonicalUrls.match(/https?:\/\/\S+/i);
  const pattern = PROHIBITED_PATTERNS.find((candidate) => candidate.test(value));
  if (urlMatch || pattern) {
    gates.push(gate(
      id,
      "BLOCKED",
      sourceFile,
      `/fields/${field}`,
      (urlMatch ? urlMatch[0] : value.match(pattern)[0]),
      "no rankings, superlatives, absolute privacy claims, price anchors, unverifiable security claims, non-canonical URLs, or operational identifiers",
      `Remove the prohibited vocabulary from the ${platform} ${field} field.`,
    ));
  } else {
    gates.push(pass(
      id,
      sourceFile,
      `/fields/${field}`,
      "clean",
      "no prohibited vocabulary",
    ));
  }
}

function checkContains(gates, id, sourceFile, pointer, value, needle, expected, recovery) {
  if (value.includes(needle)) {
    gates.push(pass(id, sourceFile, pointer, "present", expected));
  } else {
    gates.push(gate(id, "BLOCKED", sourceFile, pointer, "missing", expected, recovery));
  }
}

function descriptionBullets(description) {
  return description.split("\n").filter((line) => line.startsWith("- ")).map((line) => line.slice(2));
}

function checkClaims(gates, platform, sourceFile, field, description) {
  const id = `metadata.${platform}.${field}.claims`;
  const expectedBullets = Object.values(CLAIMS).sort();
  const actual = descriptionBullets(description).sort();
  if (JSON.stringify(actual) === JSON.stringify(expectedBullets)) {
    gates.push(pass(id, sourceFile, `/fields/${field}`, "exact whitelist", "exactly the six canonical claim bullets"));
  } else {
    gates.push(gate(
      id,
      "BLOCKED",
      sourceFile,
      `/fields/${field}`,
      JSON.stringify(actual),
      "exactly the six canonical claim bullets",
      `Replace the ${platform} ${field} bullet list with the canonical claims whitelist.`,
    ));
  }
}

function checkCategories(gates, platform, sourceFile, categories, allowlist, entries) {
  const id = `metadata.${platform}.categories`;
  const invalid = entries.filter(([, value]) => !allowlist.includes(value));
  if (invalid.length > 0) {
    gates.push(gate(
      id,
      "BLOCKED",
      sourceFile,
      "/categories",
      invalid.map(([key, value]) => `${key}=${value}`).join(", "),
      `one of: ${allowlist.join(", ")}`,
      `Choose ${platform} categories from the pinned store allowlist.`,
    ));
  } else {
    gates.push(pass(id, sourceFile, "/categories", entries.map(([, value]) => value).join(", "), `one of: ${allowlist.join(", ")}`));
  }
}

function evaluatePlatform(gates, platform, sourceFile, source, limits, descriptionField) {
  const { fields } = source;
  for (const [field, limit] of Object.entries(limits)) {
    checkLength(gates, platform, sourceFile, field, fields[field], limit);
    checkVocabulary(gates, platform, sourceFile, field, fields[field]);
  }

  const description = fields[descriptionField];
  checkContains(
    gates,
    `metadata.${platform}.${descriptionField}.signatureDisclaimer`,
    sourceFile,
    `/fields/${descriptionField}`,
    description,
    SIGNATURE_DISCLAIMER,
    "the exact signature disclaimer",
    "Add the canonical signature disclaimer to the description.",
  );
  checkContains(
    gates,
    `metadata.${platform}.${descriptionField}.editorialDisclaimer`,
    sourceFile,
    `/fields/${descriptionField}`,
    description,
    EDITORIAL_DISCLAIMER,
    "the exact editorial-label disclaimer",
    "Add the canonical editorial-label disclaimer to the description.",
  );
  checkContains(
    gates,
    `metadata.${platform}.${descriptionField}.pricing`,
    sourceFile,
    `/fields/${descriptionField}`,
    description,
    PRICING_SENTENCE,
    "the exact free/no-in-app-purchases sentence",
    "Add the canonical pricing sentence to the description.",
  );
  checkContains(
    gates,
    `metadata.${platform}.${descriptionField}.nearbyQualification`,
    sourceFile,
    `/fields/${descriptionField}`,
    description,
    NEARBY_QUALIFICATION,
    "the experimental phones-only Nearby qualification",
    "Qualify Nearby exchange as experimental in the description.",
  );
  checkContains(
    gates,
    `metadata.${platform}.releaseNotes.earlyAccess`,
    sourceFile,
    "/fields/releaseNotes",
    fields.releaseNotes,
    EARLY_ACCESS_SENTENCE,
    "the exact 1.0 early-access sentence",
    "Lead the release notes with the canonical early-access sentence.",
  );
  checkClaims(gates, platform, sourceFile, descriptionField, description);
}

function checkConsistency(gates, appleDescription, googleDescription) {
  const shared = {
    positioning: POSITIONING_SENTENCE,
    earlyAccess: EARLY_ACCESS_SENTENCE,
    signatureDisclaimer: SIGNATURE_DISCLAIMER,
    editorialDisclaimer: EDITORIAL_DISCLAIMER,
    pricing: PRICING_SENTENCE,
  };
  for (const [name, sentence] of Object.entries(shared)) {
    const id = `metadata.consistency.${name}`;
    if (appleDescription.includes(sentence) && googleDescription.includes(sentence)) {
      gates.push(pass(id, CONSISTENCY_SOURCE, "/fields", "shared", "the sentence in both platform descriptions"));
    } else {
      gates.push(gate(
        id,
        "BLOCKED",
        CONSISTENCY_SOURCE,
        "/fields",
        "divergent",
        "the exact shared sentence in both platform descriptions",
        "Align both platform descriptions on the canonical sentence.",
      ));
    }
  }
}

function checkCarrier(gates, sources) {
  const frames = sources.visuals.frames;
  const unknown = frames.filter((frame) => !(frame.claimId in CLAIMS));
  if (unknown.length > 0) {
    gates.push(gate(
      "metadata.carrier.claimId",
      "BLOCKED",
      VISUALS_SOURCE,
      "/frames",
      unknown.map((frame) => frame.claimId).join(", "),
      `claim IDs from: ${Object.keys(CLAIMS).join(", ")}`,
      "Point every visuals frame at a claims-whitelist ID.",
    ));
  } else {
    gates.push(pass("metadata.carrier.claimId", VISUALS_SOURCE, "/frames", "all known", "claims-whitelist IDs"));
  }

  for (const platform of ["apple", "google"]) {
    const sourceFile = platform === "apple" ? APPLE_SOURCE : GOOGLE_SOURCE;
    const descriptionField = platform === "apple" ? "description" : "fullDescription";
    const description = sources[platform].fields[descriptionField];
    const missing = frames
      .filter((frame) => frame.claimId in CLAIMS)
      .filter((frame) => !descriptionBullets(description).includes(CLAIMS[frame.claimId]));
    const id = `metadata.carrier.${platform}`;
    if (missing.length > 0) {
      gates.push(gate(
        id,
        "BLOCKED",
        sourceFile,
        `/fields/${descriptionField}`,
        missing.map((frame) => frame.claimId).join(", "),
        "every screenshot-headline claim carried in the description bullets",
        `Add the missing claim bullets to the ${platform} description.`,
      ));
    } else {
      gates.push(pass(id, sourceFile, `/fields/${descriptionField}`, "all carried", "every screenshot-headline claim carried in the description bullets"));
    }
  }
}

export function evaluateMetadata(sources) {
  const gates = [];
  evaluatePlatform(gates, "apple", APPLE_SOURCE, sources.apple, APPLE_FIELD_LIMITS, "description");
  evaluatePlatform(gates, "google", GOOGLE_SOURCE, sources.google, GOOGLE_FIELD_LIMITS, "fullDescription");
  if (sources.apple.fields.promotionalText.includes(POSITIONING_SENTENCE)) {
    gates.push(pass(
      "metadata.apple.promotionalText.positioning",
      APPLE_SOURCE,
      "/fields/promotionalText",
      "present",
      "the exact positioning sentence",
    ));
  } else {
    gates.push(gate(
      "metadata.apple.promotionalText.positioning",
      "BLOCKED",
      APPLE_SOURCE,
      "/fields/promotionalText",
      "missing",
      "the exact positioning sentence",
      "Lead the promotional text with the canonical positioning sentence.",
    ));
  }
  checkConsistency(gates, sources.apple.fields.description, sources.google.fields.fullDescription);
  checkCategories(
    gates,
    "apple",
    APPLE_SOURCE,
    sources.apple.categories,
    [...APPLE_PRIMARY_CATEGORIES, ...APPLE_SECONDARY_CATEGORIES],
    [["primaryCategory", sources.apple.categories.primaryCategory], ["secondaryCategory", sources.apple.categories.secondaryCategory]],
  );
  checkCategories(
    gates,
    "google",
    GOOGLE_SOURCE,
    sources.google.categories,
    GOOGLE_CATEGORIES,
    [["category", sources.google.categories.category]],
  );
  checkCarrier(gates, sources);
  return gates;
}

async function readJson(fs, path) {
  return JSON.parse(await fs.readFile(path, "utf8"));
}

export async function loadMetadataSources({ sourceDirectory, registry, fs }) {
  if (!registry || typeof registry.ajv?.getSchema !== "function") {
    throw new TypeError("a schema registry is required");
  }
  const apple = validateSource(registry, "apple-metadata", await readJson(fs, join(sourceDirectory, "apple", "en-US.json")));
  const google = validateSource(registry, "google-metadata", await readJson(fs, join(sourceDirectory, "google", "en-US.json")));
  const visuals = validateSource(registry, "visuals", await readJson(fs, join(sourceDirectory, "visuals.json")));
  return Object.freeze({ apple, google, visuals });
}

function buildManifest(platform, source, limits, fieldFiles, emitted) {
  const fields = {};
  for (const [field, file] of Object.entries(fieldFiles)) {
    fields[field] = {
      file,
      codePoints: codePoints(emitted[field]),
      limit: limits[field],
      sha256: emitted[`__digest:${field}`],
    };
  }
  return {
    schemaVersion: 1,
    platform,
    locale: "en-US",
    urls: { ...source.urls },
    categories: { ...source.categories },
    fields,
  };
}

async function writePlatform({ fs, sha256, directory, source, limits, fieldFiles }) {
  const staging = `${directory}.staging`;
  await fs.rm(staging, { recursive: true, force: true });
  await fs.mkdir(staging, { recursive: true });
  const emitted = {};
  for (const [field, file] of Object.entries(fieldFiles)) {
    const content = source.fields[field];
    emitted[field] = content;
    const bytes = `${content}\n`;
    emitted[`__digest:${field}`] = sha256(bytes);
    await fs.writeFile(join(staging, file), bytes);
  }
  const manifest = buildManifest(
    directory.includes("apple") ? "apple" : "google",
    source,
    limits,
    fieldFiles,
    emitted,
  );
  const manifestBytes = canonicalJson(manifest);
  await fs.writeFile(join(staging, "manifest.json"), manifestBytes);
  await fs.rm(directory, { recursive: true, force: true });
  await fs.rename(staging, directory);
  return sha256(manifestBytes);
}

export async function generateMetadata({ sources, outputDirectory, fs, sha256 }) {
  if (typeof sha256 !== "function") {
    throw new TypeError("a sha256 dependency is required");
  }
  const failures = evaluateMetadata(sources).filter((gate) => gate.state === "BLOCKED");
  if (failures.length > 0) {
    const first = failures[0];
    throw releaseDiagnosticError(`metadata sources are blocked: ${first.id}`, {
      sourceFile: first.sourceFile,
      pointer: first.pointer,
      observed: first.observed,
      expected: first.expected,
    });
  }
  const appleManifestSha256 = await writePlatform({
    fs,
    sha256,
    directory: join(outputDirectory, "apple", "en-US"),
    source: sources.apple,
    limits: APPLE_FIELD_LIMITS,
    fieldFiles: APPLE_FIELD_FILES,
  });
  const googleManifestSha256 = await writePlatform({
    fs,
    sha256,
    directory: join(outputDirectory, "google", "en-US"),
    source: sources.google,
    limits: GOOGLE_FIELD_LIMITS,
    fieldFiles: GOOGLE_FIELD_FILES,
  });
  return Object.freeze({ appleManifestSha256, googleManifestSha256 });
}
