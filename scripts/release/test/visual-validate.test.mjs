import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import * as realFs from "node:fs/promises";
import { join } from "node:path";

import { tempReleaseRoot } from "./helpers.mjs";
import { validateVisuals } from "../visual-validate.mjs";
import { renderDrafts } from "../visual-render.mjs";

const repositoryRoot = new URL("../../..", import.meta.url).pathname;
const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");

async function realVisuals() {
  return JSON.parse(
    await realFs.readFile(join(repositoryRoot, "release", "source", "visuals.json"), "utf8"),
  );
}

// Render the 30-cell draft tree once; tests share it. Provenance-mutation
// tests always start from the captured original bytes.
let sharedTree = null;

async function renderedTree() {
  if (sharedTree) return sharedTree;
  const root = await tempReleaseRoot();
  const visuals = await realVisuals();
  const provenance = await renderDrafts({
    visuals,
    fixtureRevision: "riot-1.0-synthetic-v1",
    fixtureSha256: "930a9c5aa06dea920b0502dbd72b6b2bf00d1b4cb9405b99e69e00d035640469",
    fontsDirectory: join(repositoryRoot, "apps", "ios", "Riot", "Resources", "Fonts"),
    outputDirectory: join(root, "release", "generated"),
    fs: realFs,
    sha256,
  });
  const provenanceBytes = await realFs.readFile(
    join(root, "release", "generated", "visual-draft-provenance.json"),
    "utf8",
  );
  sharedTree = { root, visuals, provenance, provenanceBytes };
  return sharedTree;
}

function gate(gates, id) {
  const found = gates.find((candidate) => candidate.id === id);
  assert.ok(found, `expected gate ${id}`);
  return found;
}

test("a freshly rendered draft tree passes validation", async () => {
  const { root, visuals } = await renderedTree();
  const gates = await validateVisuals({
    visuals,
    repositoryRoot: root,
    fs: realFs,
    sha256,
  });
  const failures = gates.filter((candidate) => candidate.state === "BLOCKED");
  assert.deepEqual(failures, [], `expected no BLOCKED: ${JSON.stringify(failures)}`);
});

test("missing and extra cells fail provenance completeness", async () => {
  const { root, visuals, provenanceBytes } = await renderedTree();
  const provenancePath = join(root, "release", "generated", "visual-draft-provenance.json");
  const provenance = JSON.parse(provenanceBytes);

  const missing = structuredClone(provenance);
  missing.cells = missing.cells.filter((cell) => cell.device !== "mac" || cell.stateId !== "offline-copy");
  await realFs.writeFile(provenancePath, JSON.stringify(missing));
  let gates = await validateVisuals({ visuals, repositoryRoot: root, fs: realFs, sha256 });
  assert.equal(gate(gates, "visual.provenance").state, "BLOCKED");

  const extra = structuredClone(provenance);
  extra.cells = [...extra.cells, { ...extra.cells[0], file: "release/generated/visuals/draft/iphone/frame-9-bogus.png" }];
  await realFs.writeFile(provenancePath, JSON.stringify(extra));
  gates = await validateVisuals({ visuals, repositoryRoot: root, fs: realFs, sha256 });
  assert.equal(gate(gates, "visual.provenance").state, "BLOCKED");
  await realFs.writeFile(provenancePath, provenanceBytes);
});

test("a tampered digest fails validation", async () => {
  const { root, visuals, provenanceBytes } = await renderedTree();
  const provenancePath = join(root, "release", "generated", "visual-draft-provenance.json");
  const provenance = JSON.parse(provenanceBytes);
  provenance.cells[0].sha256 = "0".repeat(64);
  await realFs.writeFile(provenancePath, JSON.stringify(provenance));
  const gates = await validateVisuals({ visuals, repositoryRoot: root, fs: realFs, sha256 });
  assert.equal(gate(gates, "visual.provenance").state, "BLOCKED");
  await realFs.writeFile(provenancePath, provenanceBytes);
});

test("metadata-carrying PNGs fail the EXIF gate", async () => {
  const { root, visuals } = await renderedTree();
  const sharp = (await import("sharp")).default;
  const target = join(root, "release", "generated", "visuals", "draft", "iphone", "frame-1-community-newswire.png");
  // Rewrite as a valid PNG carrying an EXIF block.
  const withMeta = await sharp(target)
    .withMetadata({ exif: { IFD0: { Copyright: "someone" } } })
    .png()
    .toBuffer();
  const savedBytes = await realFs.readFile(target);
  await realFs.writeFile(target, withMeta);
  const gates = await validateVisuals({ visuals, repositoryRoot: root, fs: realFs, sha256 });
  assert.equal(gate(gates, "visual.metadata").state, "BLOCKED");
  await realFs.writeFile(target, savedBytes);
});

test("alpha-bearing screenshots fail the alpha gate", async () => {
  const { root, visuals } = await renderedTree();
  const sharp = (await import("sharp")).default;
  const target = join(root, "release", "generated", "visuals", "draft", "mac", "frame-6-offline-copy.png");
  const rgba = await sharp(target).ensureAlpha().png().toBuffer();
  const savedBytes = await realFs.readFile(target);
  await realFs.writeFile(target, rgba);
  const gates = await validateVisuals({ visuals, repositoryRoot: root, fs: realFs, sha256 });
  assert.equal(gate(gates, "visual.alpha").state, "BLOCKED");
  await realFs.writeFile(target, savedBytes);
});

test("outputs on final visual paths fail the draft boundary guard", async () => {
  const { root, visuals } = await renderedTree();
  const sharp = (await import("sharp")).default;
  await realFs.mkdir(join(root, "release", "generated", "visuals", "iphone"), { recursive: true });
  await sharp({ create: { width: 8, height: 8, channels: 3, background: "#17160f" } })
    .png()
    .toFile(join(root, "release", "generated", "visuals", "iphone", "frame-1-community-newswire.png"));
  const gates = await validateVisuals({ visuals, repositoryRoot: root, fs: realFs, sha256 });
  assert.equal(gate(gates, "visual.draft-boundary").state, "BLOCKED");
  await realFs.rm(join(root, "release", "generated", "visuals", "iphone"), { recursive: true, force: true });
});

test("headline-band contrast below 4.5:1 fails", async () => {
  const { root, visuals } = await renderedTree();
  const sharp = (await import("sharp")).default;
  // Repaint a draft with a low-contrast band (mid-gray text on mid-gray band).
  const width = 1320; const height = 2868;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}">`
    + `<rect width="${width}" height="${height}" fill="#eae6da"/>`
    + `<rect y="${height - 600}" width="${width}" height="600" fill="#77776f"/>`
    + `<text x="140" y="${height - 200}" font-size="90" fill="#888880">Low contrast</text></svg>`;
  const lowTarget = join(root, "release", "generated", "visuals", "draft", "iphone", "frame-2-signed-publishing.png");
  const savedLow = await realFs.readFile(lowTarget);
  await realFs.writeFile(lowTarget, await sharp(Buffer.from(svg)).png().toBuffer());
  const gates = await validateVisuals({ visuals, repositoryRoot: root, fs: realFs, sha256 });
  assert.equal(gate(gates, "visual.contrast").state, "BLOCKED");
  await realFs.writeFile(lowTarget, savedLow);
});

test("prohibited tokens in visuals source strings fail closed", async () => {
  const visuals = await realVisuals();
  const classes = [
    ["person/name", "Person: Ana"],
    ["email", "ana@example.com"],
    ["phone", "+64 21 555 0100"],
    ["address", "123 Main Street"],
    ["coordinates", "37.7749,-122.4194"],
    ["device token", "ExponentPushToken[fixture]"],
    ["private", "private community"],
    ["production URL", "https://riot.protest.net.evil.example"],
    ["IP address", "203.0.113.1"],
    ["npub", "npub1t985dmat80n6xlrnhsjzzrlhfkcmmemul47n3mz9lws70lrxs0pqwzdyaw"],
    ["nsec", "nsec1tu92893lv55urd4almqhfnrv48ls2uwas5hxg6eashq9jhnt45ts9en3zd"],
    ["note", "note1m99r7nwc0wdrkzldrqan96gklg5usqspq7z9696j6unf0ljnpxjspqfw99"],
  ];
  for (const [label, token] of classes) {
    const mutated = structuredClone(visuals);
    mutated.frames[0].supportingCopy = token;
    const gates = await validateVisuals({
      visuals: mutated,
      repositoryRoot: repositoryRoot,
      fs: realFs,
      sha256,
      skipRenderChecks: true,
    });
    assert.equal(gate(gates, "visual.prohibited-data").state, "BLOCKED", label);
  }
});

test("band geometry violations in rendered pixels fail", async () => {
  const { root, visuals } = await renderedTree();
  const sharp = (await import("sharp")).default;
  // Band covering 40% of a portrait draft violates the 24% rule.
  const width = 1080; const height = 2400;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}">`
    + `<rect width="${width}" height="${height}" fill="#eae6da"/>`
    + `<rect y="${Math.floor(height * 0.6)}" width="${width}" height="${Math.ceil(height * 0.4)}" fill="#17160f"/></svg>`;
  const bandTarget = join(root, "release", "generated", "visuals", "draft", "android-phone", "frame-3-labels-and-signatures.png");
  const savedBand = await realFs.readFile(bandTarget);
  await realFs.writeFile(bandTarget, await sharp(Buffer.from(svg)).png().toBuffer());
  const gates = await validateVisuals({ visuals, repositoryRoot: root, fs: realFs, sha256 });
  assert.equal(gate(gates, "visual.band-geometry").state, "BLOCKED");
  await realFs.writeFile(bandTarget, savedBand);
});

test("validateVisuals requires a sha256 dependency", async () => {
  await assert.rejects(validateVisuals({ visuals: await realVisuals(), repositoryRoot: ".", fs: realFs }), /sha256 dependency is required/);
});

test("unreadable provenance fails the provenance gate", async () => {
  const { root, visuals } = await renderedTree();
  const provenancePath = join(root, "release", "generated", "visual-draft-provenance.json");
  const original = await realFs.readFile(provenancePath, "utf8");
  await realFs.writeFile(provenancePath, "{ not json");
  const gates = await validateVisuals({ visuals, repositoryRoot: root, fs: realFs, sha256 });
  assert.equal(gate(gates, "visual.provenance").state, "BLOCKED");
  await realFs.writeFile(provenancePath, original);
});

test("an approval block with a mismatched manifest digest fails", async () => {
  const { root, visuals, provenanceBytes } = await renderedTree();
  const provenancePath = join(root, "release", "generated", "visual-draft-provenance.json");
  const provenance = JSON.parse(provenanceBytes);
  provenance.approval = {
    manifestSha256: "0".repeat(64),
    reviewer: "test",
    reviewedAt: "2026-07-27T00:00:00.000Z",
    nativeSizeResult: "PASS",
    thumbnailResult: "PASS",
  };
  await realFs.writeFile(provenancePath, JSON.stringify(provenance));
  const gates = await validateVisuals({ visuals, repositoryRoot: root, fs: realFs, sha256 });
  assert.equal(gate(gates, "visual.provenance").state, "BLOCKED");
  await realFs.writeFile(provenancePath, provenanceBytes);
});

test("a missing draft PNG fails the pixel gates fail-closed", async () => {
  const { root, visuals } = await renderedTree();
  const target = join(root, "release", "generated", "visuals", "draft", "android-tablet", "frame-6-offline-copy.png");
  const saved = await realFs.readFile(target);
  await realFs.rm(target);
  const gates = await validateVisuals({ visuals, repositoryRoot: root, fs: realFs, sha256 });
  assert.equal(gate(gates, "visual.metadata").state, "BLOCKED");
  await realFs.writeFile(target, saved);
});

test("an approval block with a correct manifest digest passes", async () => {
  const { root, visuals, provenanceBytes } = await renderedTree();
  const { canonicalJson } = await import("../canonical-json.mjs");
  const provenancePath = join(root, "release", "generated", "visual-draft-provenance.json");
  const provenance = JSON.parse(provenanceBytes);
  provenance.approval = {
    manifestSha256: sha256(canonicalJson(provenance)),
    reviewer: "test",
    reviewedAt: "2026-07-27T00:00:00.000Z",
    nativeSizeResult: "PASS",
    thumbnailResult: "PASS",
  };
  await realFs.writeFile(provenancePath, JSON.stringify(provenance));
  const gates = await validateVisuals({ visuals, repositoryRoot: root, fs: realFs, sha256 });
  assert.equal(gate(gates, "visual.provenance").state, "PASS");
  await realFs.writeFile(provenancePath, provenanceBytes);
});

test("a canonical band pair below the contrast floor is a hard error", async () => {
  const visuals = await realVisuals();
  visuals.colors.ink = "#77776f";
  visuals.colors.paper = "#888880";
  await assert.rejects(
    validateVisuals({ visuals, repositoryRoot: ".", fs: realFs, sha256 }),
    /contrast rule/,
  );
});

test("an ink band with dark text fails the contrast gate", async () => {
  const { root, visuals } = await renderedTree();
  const sharp = (await import("sharp")).default;
  const width = 2752; const height = 2064;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}">`
    + `<rect width="${width}" height="${height}" fill="#eae6da"/>`
    + `<rect y="${height - 225}" width="${width}" height="225" fill="#17160f"/>`
    + `<text x="140" y="${height - 80}" font-size="100" fill="#2a2823">Dark text</text></svg>`;
  const darkTarget = join(root, "release", "generated", "visuals", "draft", "ipad", "frame-2-signed-publishing.png");
  const savedDark = await realFs.readFile(darkTarget);
  await realFs.writeFile(darkTarget, await sharp(Buffer.from(svg)).png().toBuffer());
  const gates = await validateVisuals({ visuals, repositoryRoot: root, fs: realFs, sha256 });
  assert.equal(gate(gates, "visual.contrast").state, "BLOCKED");
  await realFs.writeFile(darkTarget, savedDark);
});

test("a corrupt PNG is metadata-fail-closed inside the pixel checks", async () => {
  const { root, visuals } = await renderedTree();
  const target = join(root, "release", "generated", "visuals", "draft", "ipad", "frame-4-community-tools.png");
  const saved = await realFs.readFile(target);
  await realFs.writeFile(target, Buffer.from("definitely not a png"));
  const gates = await validateVisuals({ visuals, repositoryRoot: root, fs: realFs, sha256 });
  assert.equal(gate(gates, "visual.metadata").state, "BLOCKED");
  await realFs.writeFile(target, saved);
});

test("provenance without a cells array fails completeness", async () => {
  const { root, visuals, provenanceBytes } = await renderedTree();
  const provenancePath = join(root, "release", "generated", "visual-draft-provenance.json");
  const provenance = JSON.parse(provenanceBytes);
  delete provenance.cells;
  await realFs.writeFile(provenancePath, JSON.stringify(provenance));
  const gates = await validateVisuals({ visuals, repositoryRoot: root, fs: realFs, sha256 });
  assert.equal(gate(gates, "visual.provenance").state, "BLOCKED");
  await realFs.writeFile(provenancePath, provenanceBytes);
});

test("clean visuals pass the prohibited gate on the skipped path", async () => {
  const gates = await validateVisuals({
    visuals: await realVisuals(),
    repositoryRoot: repositoryRoot,
    fs: realFs,
    sha256,
    skipRenderChecks: true,
  });
  assert.equal(gate(gates, "visual.prohibited-data").state, "PASS");
});

test("prohibited content inside provenance fails the full-path gate", async () => {
  const { root, visuals, provenanceBytes } = await renderedTree();
  const provenancePath = join(root, "release", "generated", "visual-draft-provenance.json");
  const provenance = JSON.parse(provenanceBytes);
  provenance.fixtureNote = "123 Main Street";
  await realFs.writeFile(provenancePath, JSON.stringify(provenance));
  const gates = await validateVisuals({ visuals, repositoryRoot: root, fs: realFs, sha256 });
  assert.equal(gate(gates, "visual.prohibited-data").state, "BLOCKED");
  await realFs.writeFile(provenancePath, provenanceBytes);
});

test("provenance without an icons key still validates its cells", async () => {
  const { root, visuals, provenanceBytes } = await renderedTree();
  const provenancePath = join(root, "release", "generated", "visual-draft-provenance.json");
  const provenance = JSON.parse(provenanceBytes);
  delete provenance.icons;
  await realFs.writeFile(provenancePath, JSON.stringify(provenance));
  const gates = await validateVisuals({ visuals, repositoryRoot: root, fs: realFs, sha256 });
  assert.equal(gate(gates, "visual.provenance").state, "PASS");
  await realFs.writeFile(provenancePath, provenanceBytes);
});
