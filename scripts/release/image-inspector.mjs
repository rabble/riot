// Pixel-level inspection for rendered release visuals. Sharp is an injected
// adapter so error branches and metadata paths stay unit-testable.

export function relativeLuminance([r, g, b]) {
  const channel = (value) => {
    const srgb = value / 255;
    return srgb <= 0.04045 ? srgb / 12.92 : ((srgb + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
}

export function contrastRatio(first, second) {
  const [lighter, darker] = [relativeLuminance(first), relativeLuminance(second)].sort((a, b) => b - a);
  // Truncate, never round up: a true 4.496 must fail a 4.5 floor.
  return Math.floor(((lighter + 0.05) / (darker + 0.05)) * 100) / 100;
}

function quantize(value) {
  return Math.round(value / 8) * 8;
}

export async function inspectPng({ sharp, path, regions, fs }) {
  if (typeof sharp !== "function") {
    throw new TypeError("a sharp adapter is required");
  }
  const readFile = fs?.readFile ?? (await import("node:fs/promises")).readFile;
  const image = sharp(path);
  const metadata = await image.metadata();
  const bytes = await readFile(path);
  const hasMetadata = bytes.includes("eXIf") || metadata.exif !== undefined || metadata.xmp !== undefined;
  const result = {
    width: metadata.width,
    height: metadata.height,
    hasAlpha: metadata.hasAlpha === true,
    hasMetadata,
  };
  if (regions) {
    result.regions = {};
    for (const [name, region] of Object.entries(regions)) {
      const { data, info } = await sharp(path)
        .extract(region)
        .raw()
        .toBuffer({ resolveWithObject: true });
      const counts = new Map();
      for (let offset = 0; offset < data.length; offset += info.channels) {
        const key = [quantize(data[offset]), quantize(data[offset + 1]), quantize(data[offset + 2])].join(",");
        counts.set(key, (counts.get(key) ?? 0) + 1);
      }
      const colors = [...counts.entries()]
        .sort((a, b) => b[1] - a[1])
        .slice(0, 4)
        .map(([key, count]) => ({ color: key.split(",").map(Number), count }));
      result.regions[name] = { colors };
    }
  }
  return result;
}
