import { test } from "node:test";
import assert from "node:assert/strict";
import * as realFs from "node:fs/promises";
import { join } from "node:path";

import {
  ANTON_CAP_RATIO,
  buildMatrix,
  layoutCell,
  readFontMetrics,
  validateLayout,
} from "../visual-model.mjs";

const repositoryRoot = new URL("../../..", import.meta.url).pathname;

async function realVisuals() {
  return JSON.parse(
    await realFs.readFile(join(repositoryRoot, "release", "source", "visuals.json"), "utf8"),
  );
}

const DEVICE_DIMS = {
  iphone: [1320, 2868],
  ipad: [2752, 2064],
  mac: [2880, 1800],
  "android-phone": [1080, 2400],
  "android-tablet": [1920, 1200],
};

const STATE_ORDER = [
  "community-newswire",
  "signed-publishing",
  "labels-and-signatures",
  "community-tools",
  "nearby-exchange",
  "offline-copy",
];

test("matrix has exactly 30 cells with pinned device geometries and frame order", async () => {
  const cells = buildMatrix(await realVisuals());
  assert.equal(cells.length, 30);
  for (const deviceId of Object.keys(DEVICE_DIMS)) {
    const deviceCells = cells.filter((cell) => cell.device === deviceId);
    assert.deepEqual(deviceCells.map((cell) => cell.stateId), STATE_ORDER, `${deviceId} frame order`);
    for (const cell of deviceCells) {
      assert.equal(cell.width, DEVICE_DIMS[deviceId][0]);
      assert.equal(cell.height, DEVICE_DIMS[deviceId][1]);
      assert.match(cell.file, new RegExp(`^release/generated/visuals/draft/${deviceId}/frame-\\d+-${cell.stateId}\\.png$`));
    }
  }
});

test("frame 5 selects nearby on phones and the join fallback elsewhere", async () => {
  const cells = buildMatrix(await realVisuals());
  const frame5 = cells.filter((cell) => cell.stateId === "nearby-exchange");
  const variant = Object.fromEntries(frame5.map((cell) => [cell.device, cell.variant]));
  assert.deepEqual(variant, {
    iphone: "nearby",
    ipad: "join",
    mac: "join",
    "android-phone": "nearby",
    "android-tablet": "join",
  });
});

test("every cell carries its frame claimId", async () => {
  const cells = buildMatrix(await realVisuals());
  assert.equal(cells.find((cell) => cell.stateId === "nearby-exchange" && cell.device === "mac").claimId, "nearby-exchange");
  assert.equal(cells[0].claimId, "follow-newswire");
});

test("readFontMetrics parses sCapHeight and unitsPerEm from the checked-in fonts", async () => {
  const fonts = {
    "Anton-Regular.ttf": { family: "Anton" },
    "SpaceMono-Bold.ttf": { family: "Space Mono" },
    "WorkSans-Variable.ttf": { family: "Work Sans" },
  };
  for (const [file] of Object.entries(fonts)) {
    const bytes = await realFs.readFile(join(repositoryRoot, "apps", "ios", "Riot", "Resources", "Fonts", file));
    const metrics = readFontMetrics(bytes);
    assert.ok(Number.isInteger(metrics.unitsPerEm) && metrics.unitsPerEm > 0, `${file} unitsPerEm`);
    assert.ok(Number.isInteger(metrics.sCapHeight) && metrics.sCapHeight > 0, `${file} sCapHeight`);
    assert.ok(metrics.sCapHeight <= metrics.unitsPerEm, `${file} sane ratio`);
  }
  assert.throws(() => readFontMetrics(Buffer.from("not a font")), /sfnt|table/);
});

test("the pinned Anton cap ratio matches the real font metrics", async () => {
  const bytes = await realFs.readFile(
    join(repositoryRoot, "apps", "ios", "Riot", "Resources", "Fonts", "Anton-Regular.ttf"),
  );
  const { unitsPerEm, sCapHeight } = readFontMetrics(bytes);
  const actual = sCapHeight / unitsPerEm;
  assert.ok(
    Math.abs(actual - ANTON_CAP_RATIO) / actual < 0.05,
    `pinned ratio ${ANTON_CAP_RATIO} vs actual ${actual.toFixed(3)}`,
  );
});

test("layout respects band fraction boundaries at 24 percent portrait and 28 percent landscape", async () => {
  const visuals = await realVisuals();
  const cells = buildMatrix(visuals);
  const phone = cells.find((cell) => cell.device === "iphone" && cell.stateId === "community-newswire");
  const tablet = cells.find((cell) => cell.device === "android-tablet" && cell.stateId === "community-newswire");

  const phoneLayout = layoutCell(visuals, phone);
  assert.ok(phoneLayout.bandHeight <= Math.floor(0.24 * phone.height), "portrait band within 24%");
  const tabletLayout = layoutCell(visuals, tablet);
  assert.ok(tabletLayout.bandHeight <= Math.floor(0.28 * tablet.height), "landscape band within 28%");

  // Forcing the band one pixel over the fraction must fail validation.
  const overPortrait = { ...phoneLayout, bandHeight: Math.floor(0.24 * phone.height) + 1 };
  assert.ok(
    validateLayout(visuals, phone, overPortrait).some((issue) => issue.rule === "band-fraction"),
    "portrait band +1px must fail",
  );
  const overLandscape = { ...tabletLayout, bandHeight: Math.floor(0.28 * tablet.height) + 1 };
  assert.ok(
    validateLayout(visuals, tablet, overLandscape).some((issue) => issue.rule === "band-fraction"),
    "landscape band +1px must fail",
  );
});

test("layout enforces the 5 percent safe inset on every edge", async () => {
  const visuals = await realVisuals();
  const cell = buildMatrix(visuals)[0];
  const layout = layoutCell(visuals, cell);
  const minInset = Math.ceil(0.05 * Math.min(cell.width, cell.height));
  assert.ok(layout.inset >= minInset, `inset ${layout.inset} >= ${minInset}`);
  const shrunk = { ...layout, inset: minInset - 1 };
  assert.ok(validateLayout(visuals, cell, shrunk).some((issue) => issue.rule === "safe-inset"));
});

test("headline limits reject four lines or 43 code points", async () => {
  const visuals = await realVisuals();
  assert.ok(
    validateLayout(visuals, null, { headline: "one\ntwo\nthree\nfour", bandHeight: 10, inset: 10, fontSizePx: 10, capHeightPx: 100 })
      .some((issue) => issue.rule === "headline-lines"),
  );
  assert.ok(
    validateLayout(visuals, null, { headline: "x".repeat(43), bandHeight: 10, inset: 10, fontSizePx: 10, capHeightPx: 100 })
      .some((issue) => issue.rule === "headline-length"),
  );
  assert.ok(
    validateLayout(visuals, null, { headline: "fine headline", bandHeight: 10, inset: 10, fontSizePx: 10, capHeightPx: 100 })
      .every((issue) => issue.rule !== "headline-lines" && issue.rule !== "headline-length"),
  );
});

test("cap-height minima use real font metrics at layout time", async () => {
  const visuals = await realVisuals();
  const cells = buildMatrix(visuals);
  const phone = cells.find((cell) => cell.device === "android-phone");
  const mac = cells.find((cell) => cell.device === "mac");
  const phoneLayout = layoutCell(visuals, phone);
  const macLayout = layoutCell(visuals, mac);
  assert.ok(phoneLayout.capHeightPx >= 56, `phone cap height ${phoneLayout.capHeightPx}`);
  assert.ok(macLayout.capHeightPx >= 72, `mac cap height ${macLayout.capHeightPx}`);
  const undersized = { ...phoneLayout, capHeightPx: 55 };
  assert.ok(validateLayout(visuals, phone, undersized).some((issue) => issue.rule === "cap-height"));
  const undersizedMac = { ...macLayout, capHeightPx: 71 };
  assert.ok(validateLayout(visuals, mac, undersizedMac).some((issue) => issue.rule === "cap-height"));
});

test("320px thumbnail geometry keeps the headline legible", async () => {
  const visuals = await realVisuals();
  const cells = buildMatrix(visuals);
  for (const cell of cells) {
    const layout = layoutCell(visuals, cell);
    const scale = 320 / cell.width;
    const thumbCap = layout.capHeightPx * scale;
    const issues = validateLayout(visuals, cell, layout);
    if (thumbCap < 14) {
      assert.ok(issues.some((issue) => issue.rule === "thumbnail-cap-height"), `${cell.device}/${cell.stateId}`);
    } else {
      assert.ok(issues.every((issue) => issue.rule !== "thumbnail-cap-height"), `${cell.device}/${cell.stateId}`);
    }
  }
  // Every pinned device must pass at its canonical layout.
  for (const cell of cells) {
    assert.ok(
      validateLayout(visuals, cell, layoutCell(visuals, cell)).every((issue) => issue.rule !== "thumbnail-cap-height"),
      `${cell.device}/${cell.stateId} thumbnail`,
    );
  }
});

test("readFontMetrics rejects truncated and table-less sfnt data", () => {
  const truncated = Buffer.alloc(13);
  truncated.writeUInt16BE(1, 4);
  assert.throws(() => readFontMetrics(truncated), /truncated table directory/);
  const noTables = Buffer.alloc(12);
  assert.throws(() => readFontMetrics(noTables), /missing head or OS\/2/);
});

test("readFontMetrics rejects a truncated OS/2 table and a non-positive sCapHeight", () => {
  const build = (sCapHeight, os2Length) => {
    const buffer = Buffer.alloc(256);
    buffer.writeUInt16BE(2, 4);
    buffer.write("head", 12, "latin1");
    buffer.writeUInt32BE(100, 12 + 8);
    buffer.writeUInt32BE(54, 12 + 12);
    buffer.write("OS/2", 28, "latin1");
    buffer.writeUInt32BE(160, 28 + 8);
    buffer.writeUInt32BE(os2Length, 28 + 12);
    buffer.writeUInt16BE(1000, 118);
    buffer.writeInt16BE(sCapHeight, 160 + 88);
    return buffer;
  };
  assert.throws(() => readFontMetrics(build(700, 10).subarray(0, 200)), /truncated OS\/2/);
  const truncatedHead = build(700, 96);
  truncatedHead.writeUInt32BE(4000, 12 + 8);
  assert.throws(() => readFontMetrics(truncatedHead), /truncated head/);
  assert.throws(() => readFontMetrics(build(0, 96)), /sCapHeight/);
});

test("validateLayout tolerates a layout without headline text fields", () => {
  const issues = validateLayout(
    { band: { maxFractionPortrait: 0.24, maxFractionLandscape: 0.28, safeInsetFraction: 0.05 }, headline: { maxLines: 3, maxCodePoints: 42, minContrast: 4.5, minCapHeightPhonePx: 56, minCapHeightLargePx: 72 }, thumbnail: { widthPx: 320, minCapHeightPx: 14 } },
    null,
    { bandHeight: 10, inset: 10, fontSizePx: 10, capHeightPx: 100 },
  );
  assert.deepEqual(issues, []);
});
