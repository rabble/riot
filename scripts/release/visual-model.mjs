// Pure visual-model: the six-frame/five-device matrix, overlay geometry, and
// layout validation. No I/O except the pure TTF metrics parser.

// Anton's sCapHeight/unitsPerEm ratio, verified against the checked-in TTF by
// the visual-model tests (tolerance 5 percent).
export const ANTON_CAP_RATIO = 0.86;

const HEADLINE_CHAR_WIDTH_RATIO = 0.42;
const LINE_HEIGHT_RATIO = 1.1;
const BAND_PADDING_RATIO = 0.25;
const LABEL_SIZE_RATIO = 0.33;

export function buildMatrix(visuals) {
  const cells = [];
  for (const device of visuals.devices) {
    visuals.frames.forEach((frame, index) => {
      const isNearbyFrame = frame.stateId === "nearby-exchange";
      cells.push({
        device: device.id,
        frame: index + 1,
        stateId: frame.stateId,
        claimId: frame.claimId,
        headline: frame.headline,
        supportingCopy: frame.supportingCopy,
        variant: isNearbyFrame ? (device.nearbyVariant ? "nearby" : "join") : "standard",
        width: device.width,
        height: device.height,
        orientation: device.orientation,
        file: `release/generated/visuals/draft/${device.id}/frame-${index + 1}-${frame.stateId}.png`,
      });
    });
  }
  return cells;
}

export function readFontMetrics(bytes) {
  if (bytes.length < 12) throw new TypeError("not an sfnt font: too small");
  const numTables = bytes.readUInt16BE(4);
  const tables = {};
  for (let index = 0; index < numTables; index += 1) {
    const record = 12 + index * 16;
    if (record + 16 > bytes.length) throw new TypeError("not an sfnt font: truncated table directory");
    const tag = bytes.toString("latin1", record, record + 4);
    tables[tag] = { offset: bytes.readUInt32BE(record + 8), length: bytes.readUInt32BE(record + 12) };
  }
  if (!tables.head || !tables["OS/2"]) {
    throw new TypeError("not an sfnt font: missing head or OS/2 table");
  }
  if (tables.head.offset + 20 > bytes.length) {
    throw new TypeError("not an sfnt font: truncated head table");
  }
  const unitsPerEm = bytes.readUInt16BE(tables.head.offset + 18);
  const os2 = tables["OS/2"].offset;
  if (os2 + 90 > bytes.length) throw new TypeError("not an sfnt font: truncated OS/2 table");
  const sCapHeight = bytes.readInt16BE(os2 + 88);
  if (sCapHeight <= 0) throw new TypeError("font has no positive sCapHeight metric");
  return { unitsPerEm, sCapHeight };
}

function wrapHeadline(headline, maxCharsPerLine, maxLines) {
  const words = headline.split(" ");
  const lines = [];
  let current = "";
  for (const word of words) {
    const candidate = current === "" ? word : `${current} ${word}`;
    if (Array.from(candidate).length <= maxCharsPerLine) {
      current = candidate;
    } else {
      if (current !== "") lines.push(current);
      current = word;
    }
  }
  lines.push(current);
  return { lines, overflow: lines.length > maxLines };
}

function isPhoneClass(deviceId) {
  return deviceId === "iphone" || deviceId === "android-phone";
}

export function layoutCell(visuals, cell) {
  const { band, headline, thumbnail } = visuals;
  const minDim = Math.min(cell.width, cell.height);
  const inset = Math.ceil(band.safeInsetFraction * minDim);
  const phone = isPhoneClass(cell.device);
  const classMinCap = phone ? headline.minCapHeightPhonePx : headline.minCapHeightLargePx;
  const thumbnailMinCap = Math.ceil((thumbnail.minCapHeightPx * cell.width) / thumbnail.widthPx);
  const targetCap = Math.max(classMinCap, thumbnailMinCap);
  const fontSizePx = Math.ceil(targetCap / ANTON_CAP_RATIO);
  const capHeightPx = Math.round(fontSizePx * ANTON_CAP_RATIO);
  const availableWidth = cell.width - 2 * inset;
  const maxCharsPerLine = Math.max(1, Math.floor(availableWidth / (HEADLINE_CHAR_WIDTH_RATIO * fontSizePx)));
  const { lines, overflow } = wrapHeadline(cell.headline, maxCharsPerLine, headline.maxLines);
  const lineHeightPx = Math.round(fontSizePx * LINE_HEIGHT_RATIO);
  const paddingPx = Math.round(fontSizePx * BAND_PADDING_RATIO);
  const bandHeight = 2 * paddingPx + lines.length * lineHeightPx;
  return {
    bandY: cell.height - bandHeight,
    bandHeight,
    inset,
    fontSizePx,
    capHeightPx,
    lineHeightPx,
    paddingPx,
    lines,
    overflow,
    labelSizePx: Math.round(fontSizePx * LABEL_SIZE_RATIO),
    labelText: `frame ${cell.frame}/6 ${cell.stateId}${cell.variant === "join" ? " (join fallback)" : ""} - synthetic fixture`,
  };
}

export function validateLayout(visuals, cell, layout) {
  const issues = [];
  const { band, headline, thumbnail } = visuals;
  const text = layout.headline ?? (layout.lines ?? []).join("\n");
  const lineCount = text.split("\n").length;
  if (lineCount > headline.maxLines || layout.overflow === true) {
    issues.push({ rule: "headline-lines", observed: lineCount, expected: `at most ${headline.maxLines}` });
  }
  if (Array.from(text.replace(/\n/g, "")).length > headline.maxCodePoints) {
    issues.push({ rule: "headline-length", observed: Array.from(text.replace(/\n/g, "")).length, expected: `at most ${headline.maxCodePoints} code points` });
  }
  if (cell === null) return issues;

  const maxFraction = cell.orientation === "portrait" ? band.maxFractionPortrait : band.maxFractionLandscape;
  if (layout.bandHeight > Math.floor(maxFraction * cell.height)) {
    issues.push({ rule: "band-fraction", observed: layout.bandHeight, expected: `at most ${Math.floor(maxFraction * cell.height)}px (${maxFraction} of ${cell.height})` });
  }
  const minInset = Math.ceil(band.safeInsetFraction * Math.min(cell.width, cell.height));
  if (layout.inset < minInset) {
    issues.push({ rule: "safe-inset", observed: layout.inset, expected: `at least ${minInset}px` });
  }
  const phone = isPhoneClass(cell.device);
  const minCap = phone ? headline.minCapHeightPhonePx : headline.minCapHeightLargePx;
  if (layout.capHeightPx < minCap) {
    issues.push({ rule: "cap-height", observed: layout.capHeightPx, expected: `at least ${minCap}px` });
  }
  const thumbCap = layout.capHeightPx * (thumbnail.widthPx / cell.width);
  if (thumbCap < thumbnail.minCapHeightPx) {
    issues.push({ rule: "thumbnail-cap-height", observed: Number(thumbCap.toFixed(1)), expected: `at least ${thumbnail.minCapHeightPx}px at ${thumbnail.widthPx}px wide` });
  }
  return issues;
}
