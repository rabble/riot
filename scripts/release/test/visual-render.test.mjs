import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import * as realFs from "node:fs/promises";
import { join } from "node:path";

import { tempReleaseRoot } from "./helpers.mjs";
import { renderDrafts, renderIcons, buildOverlaySvg, buildBaseSvg } from "../visual-render.mjs";
import { buildMatrix } from "../visual-model.mjs";

const repositoryRoot = new URL("../../..", import.meta.url).pathname;
const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");

async function realVisuals() {
  return JSON.parse(
    await realFs.readFile(join(repositoryRoot, "release", "source", "visuals.json"), "utf8"),
  );
}

function fakeSharp() {
  const calls = [];
  const pipeline = {
    composite(inputs) { calls.push(["composite", inputs]); return pipeline; },
    png(options) { calls.push(["png", options]); return pipeline; },
    resize(width, height, options) { calls.push(["resize", [width, height, options]]); return pipeline; },
    flatten(options) { calls.push(["flatten", options]); return pipeline; },
    removeAlpha() { calls.push(["removeAlpha"]); return pipeline; },
    async toBuffer() { calls.push(["toBuffer"]); return Buffer.from("fake-png"); },
    async toFile(path) { calls.push(["toFile", path]); },
    async metadata() { return { width: 100, height: 100, hasAlpha: false }; },
  };
  const factory = (input) => { calls.push(["create", input]); return pipeline; };
  return { factory, calls };
}

test("overlay SVG uses plain font-family, ink band, and escaped text", async () => {
  const visuals = await realVisuals();
  const cell = buildMatrix(visuals)[0];
  const svg = buildOverlaySvg(visuals, cell, { bandHeight: 600, inset: 140, fontSizePx: 90, lines: ["Your community.", "Your newswire."] });
  assert.match(svg, /font-family="Anton"/);
  assert.match(svg, /font-family="Space Mono"/);
  assert.doesNotMatch(svg, /@font-face/);
  assert.match(svg, /#17160f/);
  const hostile = buildOverlaySvg(visuals, { ...cell, width: 100 }, { bandHeight: 10, inset: 1, fontSizePx: 5, lines: ["<script>alert(1)</script>"] });
  assert.doesNotMatch(hostile, /<script>/);
  assert.match(hostile, /&lt;script&gt;/);
});

test("base SVG is clearly synthetic and carries the fixture state id", async () => {
  const visuals = await realVisuals();
  const cell = buildMatrix(visuals)[0];
  const svg = buildBaseSvg(visuals, cell);
  assert.match(svg, /#eae6da/);
  assert.match(svg, /community-newswire/);
  assert.match(svg, /synthetic draft/i);
  assert.match(svg, new RegExp(`width="${cell.width}"`));
});

test("renderDrafts is deterministic, strips metadata, and writes only draft paths", async () => {
  const visuals = await realVisuals();
  const root = await tempReleaseRoot();
  const outputDirectory = join(root, "release", "generated");
  const fixtureSha256 = "930a9c5aa06dea920b0502dbd72b6b2bf00d1b4cb9405b99e69e00d035640469";
  const args = {
    visuals,
    fixtureRevision: "riot-1.0-synthetic-v1",
    fixtureSha256,
    fontsDirectory: join(repositoryRoot, "apps", "ios", "Riot", "Resources", "Fonts"),
    outputDirectory,
    fs: realFs,
    sha256,
  };
  const first = await renderDrafts(args);
  const second = await renderDrafts(args);
  assert.deepEqual(first, second, "draft rendering must be deterministic");
  assert.equal(first.cells.length, 30);
  for (const cell of first.cells) {
    assert.match(cell.file, /^release\/generated\/visuals\/draft\//);
    const bytes = await realFs.readFile(join(root, cell.file));
    assert.equal(sha256(bytes), cell.sha256, `${cell.file} provenance digest`);
    // PNG magic + no EXIF: draft pipeline never writes metadata chunks.
    assert.deepEqual([...bytes.subarray(0, 8)], [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
    assert.equal(bytes.includes("Exif"), false, `${cell.file} must not carry EXIF`);
    assert.equal(bytes.includes("eXIf"), false, `${cell.file} must not carry eXIf chunk`);
  }
});

test("renderDrafts fails closed when the sharp adapter fails", async () => {
  const visuals = await realVisuals();
  const root = await tempReleaseRoot();
  const { factory } = fakeSharp();
  const failing = (input) => {
    const pipeline = factory(input);
    pipeline.toBuffer = async () => { throw new Error("composite exploded"); };
    return pipeline;
  };
  await assert.rejects(
    renderDrafts({
      visuals,
      fixtureRevision: "riot-1.0-synthetic-v1",
      fixtureSha256: "0".repeat(64),
      fontsDirectory: join(repositoryRoot, "apps", "ios", "Riot", "Resources", "Fonts"),
      outputDirectory: join(root, "out"),
      fs: realFs,
      sha256,
      sharp: failing,
    }),
    /composite exploded/,
  );
});

test("renderIcons derives every icon from the 1024 master with the alpha table", async () => {
  const root = await tempReleaseRoot();
  const result = await renderIcons({
    masterPath: join(repositoryRoot, "apps", "ios", "Riot", "Assets.xcassets", "AppIcon.appiconset", "AppIcon-1024.png"),
    outputDirectory: join(root, "release", "generated"),
    fs: realFs,
    sha256,
    fontsDirectory: join(repositoryRoot, "apps", "ios", "Riot", "Resources", "Fonts"),
    visuals: await realVisuals(),
  });
  const files = result.icons.map((icon) => icon.file).sort();
  for (const expected of [
    "release/generated/icons/macos/AppIcon.appiconset/Contents.json",
    "release/generated/icons/android/play-icon-512.png",
    "release/generated/icons/android/adaptive/foreground.png",
    "release/generated/icons/android/adaptive/background.png",
    "release/generated/icons/android/mipmap-mdpi/ic_launcher.png",
    "release/generated/icons/android/mipmap-xxxhdpi/ic_launcher.png",
    "release/generated/visuals/google/play-icon-512.png",
    "release/generated/visuals/google/feature-graphic-1024x500.png",
  ]) {
    assert.ok(files.includes(expected), `missing ${expected}`);
  }
  const macIcons = result.icons.filter((icon) => icon.file.includes("macos") && icon.file.endsWith(".png"));
  assert.equal(macIcons.length, 10, "10 macOS icon PNGs");
  for (const icon of result.icons) {
    const bytes = await realFs.readFile(join(root, icon.file));
    assert.equal(sha256(bytes), icon.sha256, `${icon.file} digest`);
  }
  // The Play icon is a byte copy at both output paths.
  const a = await realFs.readFile(join(root, "release/generated/icons/android/play-icon-512.png"));
  const b = await realFs.readFile(join(root, "release/generated/visuals/google/play-icon-512.png"));
  assert.deepEqual(a, b);
  // Adaptive foreground keeps alpha; opaque artifacts do not.
  const sharp = (await import("sharp")).default;
  const foreground = await sharp(join(root, "release/generated/icons/android/adaptive/foreground.png")).metadata();
  assert.equal(foreground.hasAlpha, true);
  const playIcon = await sharp(join(root, "release/generated/icons/android/play-icon-512.png")).metadata();
  assert.equal(playIcon.hasAlpha, false);
  const feature = await sharp(join(root, "release/generated/visuals/google/feature-graphic-1024x500.png")).metadata();
  assert.deepEqual([feature.width, feature.height], [1024, 500]);
});

test("renderDrafts and renderIcons guard their dependencies", async () => {
  const visuals = await realVisuals();
  await assert.rejects(
    renderDrafts({ visuals, fontsDirectory: "x", outputDirectory: "y", fs: realFs }),
    /sha256 dependency is required/,
  );
  await assert.rejects(
    renderDrafts({ visuals, fixtureRevision: "r", fixtureSha256: "0".repeat(64), outputDirectory: "y", fs: realFs, sha256 }),
    /fonts directory is required/,
  );
  await assert.rejects(
    renderIcons({ masterPath: "x", outputDirectory: "y", fs: realFs }),
    /sha256 dependency is required/,
  );
  await assert.rejects(
    renderIcons({ masterPath: "x", outputDirectory: "y", fs: realFs, sha256 }),
    /visuals source is required/,
  );
});

test("renderDrafts fails closed on an invalid layout", async () => {
  const visuals = await realVisuals();
  visuals.frames[0].headline = "x".repeat(43);
  await assert.rejects(
    renderDrafts({
      visuals,
      fixtureRevision: "r",
      fixtureSha256: "0".repeat(64),
      fontsDirectory: join(repositoryRoot, "apps", "ios", "Riot", "Resources", "Fonts"),
      outputDirectory: "y",
      fs: realFs,
      sha256,
    }),
    /layout invalid/,
  );
});

test("approval survives regeneration while cells are unchanged, drops on drift", async () => {
  const visuals = await realVisuals();
  const root = await tempReleaseRoot();
  const outputDirectory = join(root, "release", "generated");
  const args = {
    visuals,
    fixtureRevision: "riot-1.0-synthetic-v1",
    fixtureSha256: "0".repeat(64),
    fontsDirectory: join(repositoryRoot, "apps", "ios", "Riot", "Resources", "Fonts"),
    outputDirectory,
    fs: realFs,
    sha256,
  };
  await renderDrafts(args);
  const provenancePath = join(outputDirectory, "visual-draft-provenance.json");
  const provenance = JSON.parse(await realFs.readFile(provenancePath, "utf8"));
  provenance.approval = {
    manifestSha256: "1".repeat(64),
    reviewer: "test",
    reviewedAt: "2026-07-27T00:00:00.000Z",
    nativeSizeResult: "PASS",
    thumbnailResult: "PASS",
  };
  await realFs.writeFile(provenancePath, JSON.stringify(provenance));

  await renderDrafts(args);
  const preserved = JSON.parse(await realFs.readFile(provenancePath, "utf8"));
  assert.deepEqual(preserved.approval, provenance.approval, "approval must survive unchanged regeneration");

  const drifted = structuredClone(visuals);
  drifted.frames[0].supportingCopy = "Changed copy.";
  await renderDrafts({ ...args, visuals: drifted });
  // supportingCopy does not alter the rendered bytes; force a real drift
  // through the fixture revision instead.
  await renderDrafts({ ...args, fixtureRevision: "riot-1.0-synthetic-v2" });
  const dropped = JSON.parse(await realFs.readFile(provenancePath, "utf8"));
  assert.equal(dropped.approval, undefined, "approval must drop when inputs drift");
});
