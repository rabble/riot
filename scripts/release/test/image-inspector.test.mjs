import { test } from "node:test";
import assert from "node:assert/strict";
import * as realFs from "node:fs/promises";
import { join } from "node:path";
import { spawn } from "node:child_process";

import { tempReleaseRoot } from "./helpers.mjs";
import { inspectPng, relativeLuminance, contrastRatio } from "../image-inspector.mjs";

const repositoryRoot = new URL("../../..", import.meta.url).pathname;

test("contrast ratio math matches WCAG references", () => {
  // Black on white is 21:1; identical colors are 1:1.
  assert.equal(contrastRatio([0, 0, 0], [255, 255, 255]), 21);
  assert.equal(contrastRatio([23, 22, 15], [23, 22, 15]), 1);
  // Canonical band pair must clear 4.5:1.
  const ink = [0x17, 0x16, 0x0f];
  const paper = [0xea, 0xe6, 0xda];
  assert.ok(contrastRatio(ink, paper) >= 4.5, `ink/paper ${contrastRatio(ink, paper)}`);
  assert.ok(relativeLuminance([255, 255, 255]) > relativeLuminance([0, 0, 0]));
});

test("inspectPng reports geometry, alpha, and metadata absence", async () => {
  const sharp = (await import("sharp")).default;
  const root = await tempReleaseRoot();
  const opaque = join(root, "opaque.png");
  await sharp({ create: { width: 40, height: 20, channels: 3, background: "#17160f" } }).png().toFile(opaque);
  const inspected = await inspectPng({ sharp, path: opaque });
  assert.deepEqual([inspected.width, inspected.height], [40, 20]);
  assert.equal(inspected.hasAlpha, false);
  assert.equal(inspected.hasMetadata, false);

  const withMeta = join(root, "meta.png");
  await sharp({ create: { width: 8, height: 8, channels: 3, background: "#eae6da" } })
    .withMetadata({ exif: { IFD0: { Copyright: "x" } } })
    .png()
    .toFile(withMeta);
  const flagged = await inspectPng({ sharp, path: withMeta });
  assert.equal(flagged.hasMetadata, true);
});

test("inspectPng samples region colors for contrast checks", async () => {
  const sharp = (await import("sharp")).default;
  const root = await tempReleaseRoot();
  const target = join(root, "band.png");
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">`
    + `<rect width="100" height="100" fill="#eae6da"/>`
    + `<rect y="76" width="100" height="24" fill="#17160f"/>`
    + `<text x="8" y="94" font-size="12" fill="#eae6da">Hi</text></svg>`;
  await sharp(Buffer.from(svg)).png().toFile(target);
  const inspected = await inspectPng({
    sharp,
    path: target,
    regions: { band: { left: 0, top: 76, width: 100, height: 24 } },
  });
  assert.ok(inspected.regions.band, "band region sampled");
  assert.ok(inspected.regions.band.colors.length > 0);
});


test("inspectPng requires a sharp adapter", async () => {
  await assert.rejects(inspectPng({ path: "x.png" }), /sharp adapter is required/);
});

test("luminance handles the dark-srgb linearization branch", () => {
  assert.ok(relativeLuminance([10, 10, 10]) < relativeLuminance([40, 40, 40]));
  assert.ok(contrastRatio([5, 5, 5], [250, 250, 250]) > 15);
});

test("inspectPng accepts an injected fs adapter", async () => {
  const sharp = (await import("sharp")).default;
  const root = await tempReleaseRoot();
  const target = join(root, "injected.png");
  await sharp({ create: { width: 6, height: 6, channels: 3, background: "#17160f" } }).png().toFile(target);
  const inspected = await inspectPng({ sharp, path: target, fs: realFs });
  assert.equal(inspected.width, 6);
});
