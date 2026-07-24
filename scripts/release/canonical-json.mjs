// Deterministic canonical JSON: the single serialization every durable release
// record and SHA-256 content digest is built on. Object keys are sorted so the
// bytes depend only on the value, never on construction order, and anything
// JSON cannot represent deterministically (undefined, functions, symbols,
// bigint, non-finite numbers) fails closed instead of being silently dropped or
// coerced.

function canonicalValue(value) {
  if (value === null) {
    return 'null';
  }

  const type = typeof value;

  if (type === 'string') {
    return JSON.stringify(value);
  }

  if (type === 'boolean') {
    return value ? 'true' : 'false';
  }

  if (type === 'number') {
    if (!Number.isFinite(value)) {
      throw new Error(`canonical JSON requires finite numbers, received ${value}`);
    }
    return JSON.stringify(value);
  }

  if (Array.isArray(value)) {
    return `[${value.map((element) => canonicalValue(element)).join(',')}]`;
  }

  if (type === 'object') {
    const keys = Object.keys(value).sort();
    const members = keys.map((key) => {
      const serialized = canonicalValue(value[key]);
      return `${JSON.stringify(key)}:${serialized}`;
    });
    return `{${members.join(',')}}`;
  }

  throw new Error(`canonical JSON has no representation for unsupported value of type ${type}`);
}

/**
 * Serialize a value to canonical JSON with recursively sorted object keys and no
 * insignificant whitespace. Throws on any value JSON cannot represent
 * deterministically.
 */
export function canonicalize(value) {
  return canonicalValue(value);
}
