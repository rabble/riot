import { join } from "node:path";

import { canonicalJson } from "./canonical-json.mjs";
import { contrastRatio, relativeLuminance } from "./image-inspector.mjs";
import { buildMatrix, layoutCell } from "./visual-model.mjs";

const VISUALS_SOURCE = "release/source/visuals.json";
const PROVENANCE_FILE = "release/generated/visual-draft-provenance.json";
const FINAL_VISUAL_DIRS = ["iphone", "ipad", "mac", "android-phone", "android-tablet"];

const PROHIBITED_PATTERNS = [
  /person: ana/i,
  /ana@example\.com/i,
  /\+64 21 555 0100/i,
  /123 main street/i,
  /37\.7749,-122\.4194/i,
  /exponentpushtoken\[fixture\]/i,
  /private community/i,
  /\b(?:npub|nsec|note)1[023456789acdefghjklmnpqrstuvwxyz]+\b/i,
  /\b(?:\d{1,3}\.){3}\d{1,3}\b/,
  /https?:\/\/(?!riot\.protest\.net\/)/i,
];

function gate(id, state, sourceFile, pointer, observed, expected, recovery) {
  return { id, state, sourceFile, pointer, observed, expected, recovery };
}

function scanProhibited(value, path, hits) {
  if (typeof value === "string") {
    for (const pattern of PROHIBITED_PATTERNS) {
      if (pattern.test(value)) hits.push(`${path}: ${value.slice(0, 48)}`);
    }
    return;
  }
  if (Array.isArray(value)) {
    value.forEach((child, index) => scanProhibited(child, `${path}/${index}`, hits));
    return;
  }
  if (value && typeof value === "object") {
    for (const [key, child] of Object.entries(value)) {
      scanProhibited(child, `${path}/${key}`, hits);
    }
  }
}

async function pathExists(fs, path) {
  try {
    await fs.readdir(path);
    return true;
  } catch {
    return false;
  }
}

function hexToRgb(hex) {
  return [1, 3, 5].map((index) => parseInt(hex.slice(index, index + 2), 16));
}

function colorDistance(first, second) {
  return Math.abs(first[0] - second[0]) + Math.abs(first[1] - second[1]) + Math.abs(first[2] - second[2]);
}

async function analyzeBand(sharp, path, cell, layout, ink, paper) {
  // Pixel verification anchored to the deterministic layout: the renderer
  // places the band at layout.bandY, so validation checks the region the
  // model declares plus the row just above it (oversized-band guard).
  const scanWidth = 320;
  const { data, info } = await sharp(path)
    .resize({ width: scanWidth })
    .raw()
    .toBuffer({ resolveWithObject: true });
  const { width, height, channels } = info;
  const scaleY = height / cell.height;
  const bandTop = Math.max(0, Math.round(layout.bandY * scaleY));
  const bottomOffset = (height - 1) * width * channels;
  const bottomColor = [data[bottomOffset], data[bottomOffset + 1], data[bottomOffset + 2]];

  const textLuminanceFloor = relativeLuminance(paper) * 0.75;
  let bandPx = 0;
  let textPx = 0;
  for (let row = bandTop; row < height; row += 1) {
    const base = row * width * channels;
    for (let x = 0; x < width; x += 1) {
      const offset = base + x * channels;
      const pixel = [data[offset], data[offset + 1], data[offset + 2]];
      if (colorDistance(pixel, ink) <= 24) bandPx += 1;
      else if (relativeLuminance(pixel) >= textLuminanceFloor) textPx += 1;
    }
  }
  const regionPixels = (height - bandTop) * width;
  let aboveInkShare = 0;
  if (bandTop > 0) {
    const base = (bandTop - 1) * width * channels;
    let count = 0;
    for (let x = 0; x < width; x += 1) {
      const offset = base + x * channels;
      if (colorDistance([data[offset], data[offset + 1], data[offset + 2]], ink) <= 24) count += 1;
    }
    aboveInkShare = count / width;
  }
  return { bottomColor, bandPx, textPx, regionPixels, aboveInkShare };
}

export async function validateVisuals({ visuals, repositoryRoot, fs, sha256, sharp, skipRenderChecks = false }) {
  if (typeof sha256 !== "function") {
    throw new TypeError("a sha256 dependency is required");
  }
  const gates = [];

  // Prohibited-data scan over the visuals source (and only cost: source scan).
  const hits = [];
  scanProhibited(visuals, "", hits);
  gates.push(hits.length > 0
    ? gate("visual.prohibited-data", "BLOCKED", VISUALS_SOURCE, "/", hits.join("; "), "no person, contact, location, device-credential, private-data, production-network, or operational identifier patterns", "Remove the prohibited content from the visuals source.")
    : gate("visual.prohibited-data", "PASS", VISUALS_SOURCE, "/", "clean", "no prohibited content patterns", "No action required."));

  if (skipRenderChecks) return gates;

  const sharpAdapter = sharp ?? (await import("sharp")).default;

  // Draft/final boundary: no WU-002 output may land on final visual paths.
  const stray = [];
  for (const device of FINAL_VISUAL_DIRS) {
    const finalDir = join(repositoryRoot, "release", "generated", "visuals", device);
    if (await pathExists(fs, finalDir)) {
      stray.push(...(await fs.readdir(finalDir)).map((name) => `visuals/${device}/${name}`));
    }
  }
  gates.push(stray.length > 0
    ? gate("visual.draft-boundary", "BLOCKED", PROVENANCE_FILE, "/", stray.join(", "), "WU-002 outputs only under visuals/draft/", "Remove non-draft outputs from the final visual paths; final captures arrive in WU-005/WU-006.")
    : gate("visual.draft-boundary", "PASS", PROVENANCE_FILE, "/", "clean", "WU-002 outputs only under visuals/draft/", "No action required."));

  // Provenance completeness and digest truth.
  const cells = buildMatrix(visuals);
  const provenancePath = join(repositoryRoot, PROVENANCE_FILE);
  let provenanceError = null;
  try {
    const provenance = JSON.parse(await fs.readFile(provenancePath, "utf8"));
    const expectedFiles = cells.map((cell) => cell.file).sort();
    const actualFiles = (provenance.cells ?? []).map((cell) => cell.file).sort();
    if (JSON.stringify(expectedFiles) !== JSON.stringify(actualFiles)) {
      provenanceError = `cell inventory mismatch: expected ${expectedFiles.length} pinned files, found ${actualFiles.length}`;
    } else {
      for (const cell of provenance.cells) {
        const bytes = await fs.readFile(join(repositoryRoot, ...cell.file.split("/").slice(0)));
        if (sha256(bytes) !== cell.sha256) {
          provenanceError = `digest mismatch for ${cell.file}`;
          break;
        }
      }
    }
    if (!provenanceError && provenance.approval) {
      const { approval, ...unsigned } = provenance;
      const derived = sha256(canonicalJson(unsigned));
      if (derived !== approval.manifestSha256) {
        provenanceError = `approval manifest digest mismatch: ${approval.manifestSha256} != ${derived}`;
      }
    }
  } catch (error) {
    provenanceError = `provenance unreadable: ${error.message}`;
  }
  gates.push(provenanceError
    ? gate("visual.provenance", "BLOCKED", PROVENANCE_FILE, "/cells", provenanceError, "30 pinned cells with re-derivable digests and a consistent approval digest", "Regenerate the draft visuals and provenance with release:generate.")
    : gate("visual.provenance", "PASS", PROVENANCE_FILE, "/cells", "30 cells, digests verified", "30 pinned cells with re-derivable digests", "No action required."));

  // Pixel-level gates over the draft tree.
  const ink = hexToRgb(visuals.colors.ink);
  const paper = hexToRgb(visuals.colors.paper);
  if (contrastRatio(ink, paper) < visuals.headline.minContrast) {
    throw new TypeError(`canonical band pair fails its own contrast rule: ${contrastRatio(ink, paper)}:1`);
  }
  const metadataBad = [];
  const alphaBad = [];
  const bandBad = [];
  const contrastBad = [];
  for (const cell of cells) {
    const path = join(repositoryRoot, cell.file);
    let bytes;
    try {
      bytes = await fs.readFile(path);
    } catch {
      metadataBad.push(`${cell.file} (missing)`);
      continue;
    }
    if (bytes.includes("eXIf") || bytes.includes("Exif")) metadataBad.push(cell.file);
    try {
      const metadata = await sharpAdapter(path).metadata();
      if (metadata.hasAlpha === true) alphaBad.push(cell.file);
      const layout = layoutCell(visuals, cell);
      const { bottomColor, bandPx, textPx, regionPixels, aboveInkShare } = await analyzeBand(
        sharpAdapter,
        path,
        cell,
        layout,
        ink,
        paper,
      );
      if (colorDistance(bottomColor, ink) > 24) {
        contrastBad.push(`${cell.file} (band color ${bottomColor.join(",")})`);
      } else if (aboveInkShare >= 0.15) {
        bandBad.push(`${cell.file} (band extends above the layout region)`);
      } else if (regionPixels === 0 || bandPx < regionPixels * 0.5 || textPx < regionPixels * 0.01) {
        contrastBad.push(`${cell.file} (band ${bandPx}/${regionPixels}, text ${textPx})`);
      }
    } catch (error) {
      metadataBad.push(`${cell.file} (unreadable: ${error.message})`);
    }
  }
  gates.push(metadataBad.length > 0
    ? gate("visual.metadata", "BLOCKED", PROVENANCE_FILE, "/", metadataBad.join(", "), "EXIF-free PNGs", "Strip image metadata from the named PNGs.")
    : gate("visual.metadata", "PASS", PROVENANCE_FILE, "/", "clean", "EXIF-free PNGs", "No action required."));
  gates.push(alphaBad.length > 0
    ? gate("visual.alpha", "BLOCKED", PROVENANCE_FILE, "/", alphaBad.join(", "), "RGB screenshots without alpha", "Flatten the named screenshots to opaque RGB.")
    : gate("visual.alpha", "PASS", PROVENANCE_FILE, "/", "clean", "RGB screenshots without alpha", "No action required."));
  gates.push(bandBad.length > 0
    ? gate("visual.band-geometry", "BLOCKED", PROVENANCE_FILE, "/", bandBad.join(", "), "an opaque band within the pinned height fraction", "Correct the band geometry in the named drafts.")
    : gate("visual.band-geometry", "PASS", PROVENANCE_FILE, "/", "within fractions", "an opaque band within the pinned height fraction", "No action required."));
  gates.push(contrastBad.length > 0
    ? gate("visual.contrast", "BLOCKED", PROVENANCE_FILE, "/", contrastBad.join(", "), `headline contrast at least ${visuals.headline.minContrast}:1`, "Raise headline/band contrast in the named drafts.")
    : gate("visual.contrast", "PASS", PROVENANCE_FILE, "/", "contrast ok", `headline contrast at least ${visuals.headline.minContrast}:1`, "No action required."));

  return gates;
}
