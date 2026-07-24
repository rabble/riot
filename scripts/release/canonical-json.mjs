function compareCodePoints(left, right) {
  const a = Array.from(left, (character) => character.codePointAt(0));
  const b = Array.from(right, (character) => character.codePointAt(0));
  for (let index = 0; index < Math.min(a.length, b.length); index += 1) {
    if (a[index] !== b[index]) return a[index] - b[index];
  }
  return a.length - b.length;
}

function normalize(value, ancestors) {
  if (value === null || typeof value === "string" || typeof value === "boolean") return value;
  if (typeof value === "number") {
    if (!Number.isFinite(value)) throw new TypeError("numbers must be finite");
    return value;
  }
  if (typeof value !== "object") throw new TypeError(`unsupported JSON value: ${typeof value}`);
  if (ancestors.has(value)) throw new TypeError("cyclic values are unsupported");

  ancestors.add(value);
  try {
    if (Array.isArray(value)) return value.map((item) => normalize(item, ancestors));
    if (Object.getPrototypeOf(value) !== Object.prototype) {
      throw new TypeError("canonical JSON requires a plain object");
    }
    return Object.fromEntries(
      Object.keys(value)
        .sort(compareCodePoints)
        .map((key) => [key, normalize(value[key], ancestors)]),
    );
  } finally {
    ancestors.delete(value);
  }
}

export function canonicalJson(value) {
  return `${JSON.stringify(normalize(value, new Set()))}\n`;
}
