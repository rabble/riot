import { canonicalJson } from "./canonical-json.mjs";

const RECORD_FIELDS = ["createdAt", "digest", "payload", "schema", "schemaVersion"];
const FIXED_SCHEMAS = new Set([
  "riot.release.accessibility.v1",
  "riot.release.account-gates.v1",
  "riot.release.claims.v1",
  "riot.release.export-compliance.v1",
  "riot.release.network-matrix.v1",
  "riot.release.policy.v1",
  "riot.release.privacy.v1",
  "riot.release.product.v1",
  "riot.release.review-instructions.v1",
  "riot.release.toolchains.v1",
]);
const SHA256 = /^[0-9a-f]{64}$/;
const RFC3339_UTC = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/;

function assertDependencies(clock, sha256) {
  if (typeof clock !== "function") throw new TypeError("clock dependency is required");
  if (typeof sha256 !== "function") throw new TypeError("sha256 dependency is required");
}

function assertSchema(schema) {
  if (!FIXED_SCHEMAS.has(schema)) throw new TypeError(`unknown fixed schema identifier: ${schema}`);
}

function assertDigestReferences(value, path = "") {
  if (Array.isArray(value)) {
    value.forEach((item, index) => assertDigestReferences(item, `${path}/${index}`));
    return;
  }
  if (value === null || typeof value !== "object") return;
  for (const [key, child] of Object.entries(value)) {
    const childPath = `${path}/${key}`;
    if ((key.endsWith("Digest") || key.endsWith("Sha256")) && !SHA256.test(child)) {
      throw new TypeError(`${childPath} must be a full lowercase SHA-256 digest`);
    }
    assertDigestReferences(child, childPath);
  }
}

function deepFreeze(value) {
  if (value && typeof value === "object" && !Object.isFrozen(value)) {
    Object.freeze(value);
    for (const child of Object.values(value)) deepFreeze(child);
  }
  return value;
}

function body(record) {
  return {
    schemaVersion: record.schemaVersion,
    schema: record.schema,
    createdAt: record.createdAt,
    payload: record.payload,
  };
}

export async function createRecord({ schema, payload, clock, sha256 }) {
  assertDependencies(clock, sha256);
  assertSchema(schema);
  assertDigestReferences(payload);
  const createdAt = clock().toISOString();
  const recordBody = { schemaVersion: 1, schema, createdAt, payload: structuredClone(payload) };
  const digest = await sha256(canonicalJson(recordBody));
  if (!SHA256.test(digest)) throw new TypeError("sha256 dependency returned an invalid digest");
  return deepFreeze({ ...recordBody, digest });
}

export async function verifyRecord(record, { sha256 }) {
  assertDependencies(() => new Date(), sha256);
  if (record === null || typeof record !== "object" || Array.isArray(record)) {
    throw new TypeError("record must be an object");
  }
  const fields = Object.keys(record).sort();
  if (fields.length !== RECORD_FIELDS.length || fields.some((field, index) => field !== RECORD_FIELDS[index])) {
    throw new TypeError("record has missing or unknown wrapper field");
  }
  if (record.schemaVersion !== 1) throw new TypeError("record schemaVersion must be 1");
  assertSchema(record.schema);
  if (!RFC3339_UTC.test(record.createdAt) || Number.isNaN(Date.parse(record.createdAt))) {
    throw new TypeError("record timestamp must be RFC3339 UTC");
  }
  if (!SHA256.test(record.digest)) throw new TypeError("record digest must be full lowercase SHA-256");
  assertDigestReferences(record.payload);
  const expected = await sha256(canonicalJson(body(record)));
  if (expected !== record.digest) throw new TypeError("record digest mismatch");
  return deepFreeze(structuredClone(record.payload));
}
