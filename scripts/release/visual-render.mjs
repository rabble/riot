import { join } from "node:path";

import { tmpdir } from "node:os";

import { canonicalJson } from "./canonical-json.mjs";
import { buildFontsConf } from "./render-environment.mjs";
import { buildMatrix, layoutCell, validateLayout } from "./visual-model.mjs";

// librsvg resolves fonts through fontconfig at first rasterization, so the
// hermetic font environment must be live before sharp is loaded.
async function prepareFonts({ fontsDirectory, fs }) {
  const workDirectory = await fs.mkdtemp(join(tmpdir(), "riot-render-fonts-"));
  const { env } = await buildFontsConf({ fontsDirectory, workDirectory, fs });
  for (const [key, value] of Object.entries(env)) {
    process.env[key] = value;
  }
}

function escapeXml(text) {
  return text
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

export function buildBaseSvg(visuals, cell) {
  const ink = visuals.colors.ink;
  const paper = visuals.colors.paper;
  const centerX = cell.width / 2;
  const centerY = cell.height / 2;
  const labelSize = Math.round(Math.min(cell.width, cell.height) * 0.028);
  const noteSize = Math.round(labelSize * 0.6);
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${cell.width}" height="${cell.height}">`
    + `<rect width="${cell.width}" height="${cell.height}" fill="${paper}"/>`
    + `<text x="${centerX}" y="${centerY}" font-family="${visuals.fonts.label.family}" font-weight="bold" font-size="${labelSize}" fill="${ink}" text-anchor="middle">${escapeXml(cell.stateId)}</text>`
    + `<text x="${centerX}" y="${centerY + labelSize + noteSize}" font-family="${visuals.fonts.body.family}" font-size="${noteSize}" fill="${ink}" text-anchor="middle">Synthetic draft base — not a product screenshot</text>`
    + `</svg>`;
}

export function buildOverlaySvg(visuals, cell, layout) {
  const ink = visuals.colors.ink;
  const paper = visuals.colors.paper;
  const textX = layout.inset;
  const firstBaseline = layout.bandY + layout.paddingPx + Math.round(layout.lineHeightPx * 0.85);
  const headlineLines = layout.lines
    .map((line, index) => {
      const y = firstBaseline + index * layout.lineHeightPx;
      return `<text x="${textX}" y="${y}" font-family="${visuals.fonts.headline.family}" font-size="${layout.fontSizePx}" fill="${paper}">${escapeXml(line)}</text>`;
    })
    .join("");
  const labelText = layout.labelText ?? cell.stateId;
  const label = `<text x="${layout.inset}" y="${Math.round(layout.inset * 0.9)}" font-family="${visuals.fonts.label.family}" font-weight="bold" font-size="${layout.labelSizePx}" fill="${ink}">${escapeXml(labelText)}</text>`;
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${cell.width}" height="${cell.height}">`
    + `<rect x="0" y="${layout.bandY}" width="${cell.width}" height="${layout.bandHeight}" fill="${ink}"/>`
    + headlineLines
    + label
    + `</svg>`;
}

async function resolveSharp(sharp) {
  if (sharp) return sharp;
  return (await import("sharp")).default;
}

export async function renderDrafts({
  visuals,
  fixtureRevision,
  fixtureSha256,
  fontsDirectory,
  outputDirectory,
  fs,
  sha256,
  sharp,
}) {
  if (typeof sha256 !== "function") {
    throw new TypeError("a sha256 dependency is required");
  }
  if (!fontsDirectory) {
    throw new TypeError("a fonts directory is required");
  }
  await prepareFonts({ fontsDirectory, fs });
  const sharpAdapter = await resolveSharp(sharp);
  const cells = buildMatrix(visuals);
  const provenanceCells = [];
  for (const cell of cells) {
    const layout = layoutCell(visuals, cell);
    const issues = validateLayout(visuals, cell, layout);
    if (issues.length > 0) {
      throw new TypeError(`layout invalid for ${cell.device}/${cell.stateId}: ${JSON.stringify(issues)}`);
    }
    const base = Buffer.from(buildBaseSvg(visuals, cell));
    const overlay = Buffer.from(buildOverlaySvg(visuals, cell, layout));
    const png = await sharpAdapter(base).composite([{ input: overlay }]).flatten({ background: visuals.colors.paper }).removeAlpha().png().toBuffer();
    const target = join(outputDirectory, ...cell.file.split("/").slice(2));
    await fs.mkdir(join(target, ".."), { recursive: true });
    await fs.writeFile(target, png);
    provenanceCells.push({
      device: cell.device,
      frame: cell.frame,
      stateId: cell.stateId,
      claimId: cell.claimId,
      variant: cell.variant,
      width: cell.width,
      height: cell.height,
      file: cell.file,
      sha256: sha256(png),
    });
  }
  const provenance = {
    schemaVersion: 1,
    platform: `${process.platform}-${process.arch}`,
    locale: visuals.locale,
    appearance: visuals.appearance,
    fixtureRevision,
    fixtureSha256,
    templateVersion: visuals.templateVersion,
    cells: provenanceCells,
  };
  // Human-review approval survives regeneration while the rendered cells and
  // template inputs are unchanged; any drift drops it for re-review.
  const provenancePath = join(outputDirectory, "visual-draft-provenance.json");
  try {
    const existing = JSON.parse(await fs.readFile(provenancePath, "utf8"));
    const { approval, ...unsignedExisting } = existing;
    if (approval && canonicalJson(unsignedExisting) === canonicalJson(provenance)) {
      provenance.approval = approval;
    }
  } catch {
    // No prior provenance (or unreadable): nothing to preserve.
  }
  await fs.mkdir(join(outputDirectory), { recursive: true });
  await fs.writeFile(provenancePath, canonicalJson(provenance));
  return Object.freeze({ cells: Object.freeze(provenanceCells), provenance: Object.freeze(provenance) });
}

const MAC_ICON_SIZES = [
  ["icon_16x16.png", 16],
  ["icon_16x16@2x.png", 32],
  ["icon_32x32.png", 32],
  ["icon_32x32@2x.png", 64],
  ["icon_128x128.png", 128],
  ["icon_128x128@2x.png", 256],
  ["icon_256x256.png", 256],
  ["icon_256x256@2x.png", 512],
  ["icon_512x512.png", 512],
  ["icon_512x512@2x.png", 1024],
];

const ANDROID_LEGACY_SIZES = [
  ["mipmap-mdpi", 48],
  ["mipmap-hdpi", 72],
  ["mipmap-xhdpi", 96],
  ["mipmap-xxhdpi", 144],
  ["mipmap-xxxhdpi", 192],
];

function macIconsetContents() {
  const entries = [
    ["16x16", "1x", "icon_16x16.png"],
    ["16x16", "2x", "icon_16x16@2x.png"],
    ["32x32", "1x", "icon_32x32.png"],
    ["32x32", "2x", "icon_32x32@2x.png"],
    ["128x128", "1x", "icon_128x128.png"],
    ["128x128", "2x", "icon_128x128@2x.png"],
    ["256x256", "1x", "icon_256x256.png"],
    ["256x256", "2x", "icon_256x256@2x.png"],
    ["512x512", "1x", "icon_512x512.png"],
    ["512x512", "2x", "icon_512x512@2x.png"],
  ];
  return `${JSON.stringify({
    images: entries.map(([size, scale, filename]) => ({ size, idiom: "mac", filename, scale })),
    info: { author: "xcode", version: 1 },
  }, null, 2)}\n`;
}

export async function renderIcons({ masterPath, outputDirectory, fs, sha256, sharp, visuals, fontsDirectory }) {
  if (typeof sha256 !== "function") {
    throw new TypeError("a sha256 dependency is required");
  }
  if (!visuals) {
    throw new TypeError("a visuals source is required");
  }
  if (fontsDirectory) {
    await prepareFonts({ fontsDirectory, fs });
  }
  const sharpAdapter = await resolveSharp(sharp);
  const icons = [];
  const writePng = async (relative, pipeline) => {
    const bytes = await pipeline.png().toBuffer();
    const target = join(outputDirectory, ...relative.split("/").slice(2));
    await fs.mkdir(join(target, ".."), { recursive: true });
    await fs.writeFile(target, bytes);
    icons.push({ file: relative, sha256: sha256(bytes) });
    return bytes;
  };

  // macOS iconset: opaque, no alpha.
  for (const [name, size] of MAC_ICON_SIZES) {
    await writePng(
      `release/generated/icons/macos/AppIcon.appiconset/${name}`,
      sharpAdapter(masterPath).resize(size, size).flatten({ background: "#eae6da" }),
    );
  }
  const contentsTarget = join(outputDirectory, "icons", "macos", "AppIcon.appiconset", "Contents.json");
  await fs.mkdir(join(contentsTarget, ".."), { recursive: true });
  await fs.writeFile(contentsTarget, macIconsetContents());
  icons.push({
    file: "release/generated/icons/macos/AppIcon.appiconset/Contents.json",
    sha256: sha256(await fs.readFile(contentsTarget)),
  });

  // Android legacy launcher: opaque.
  for (const [density, size] of ANDROID_LEGACY_SIZES) {
    await writePng(
      `release/generated/icons/android/${density}/ic_launcher.png`,
      sharpAdapter(masterPath).resize(size, size).flatten({ background: "#eae6da" }),
    );
  }

  // Android adaptive icon: foreground keeps alpha with the glyph inside the
  // 66 percent safe zone; background is opaque paper.
  const glyph = await sharpAdapter(masterPath)
    .resize(Math.round(432 * 0.66), Math.round(432 * 0.66))
    .png()
    .toBuffer();
  await writePng(
    "release/generated/icons/android/adaptive/foreground.png",
    sharpAdapter({ create: { width: 432, height: 432, channels: 4, background: { r: 0, g: 0, b: 0, alpha: 0 } } })
      .composite([{ input: glyph, gravity: "center" }]),
  );
  await writePng(
    "release/generated/icons/android/adaptive/background.png",
    sharpAdapter({ create: { width: 432, height: 432, channels: 3, background: "#eae6da" } }),
  );

  // Play store icon: full-bleed opaque 512, emitted at both store-shaped paths.
  const playBytes = await writePng(
    "release/generated/icons/android/play-icon-512.png",
    sharpAdapter(masterPath).resize(512, 512).flatten({ background: "#eae6da" }),
  );
  const playCopy = join(outputDirectory, "visuals", "google", "play-icon-512.png");
  await fs.mkdir(join(playCopy, ".."), { recursive: true });
  await fs.writeFile(playCopy, playBytes);
  icons.push({ file: "release/generated/visuals/google/play-icon-512.png", sha256: sha256(playBytes) });

  // Feature graphic: branded band composition, no screenshot content.
  const ink = visuals.colors.ink;
  const paper = visuals.colors.paper;
  const featureSvg = `<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="500">`
    + `<rect width="1024" height="500" fill="${paper}"/>`
    + `<rect y="330" width="1024" height="170" fill="${ink}"/>`
    + `<text x="512" y="240" font-family="Anton" font-size="210" fill="${ink}" text-anchor="middle">RIOT</text>`
    + `<text x="512" y="430" font-family="Space Mono" font-weight="bold" font-size="44" fill="${paper}" text-anchor="middle">Your community. Your newswire.</text>`
    + `</svg>`;
  await writePng(
    "release/generated/visuals/google/feature-graphic-1024x500.png",
    sharpAdapter(Buffer.from(featureSvg)).flatten({ background: paper }),
  );

  return Object.freeze({ icons: Object.freeze(icons) });
}
