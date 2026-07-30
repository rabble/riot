import { canonicalJson } from "./canonical-json.mjs";
import { isExactUtcTimestamp, validateSource } from "./schema.mjs";

const RECORD_FIELDS = ["createdAt", "digest", "payload", "schema", "schemaVersion"];
const FIXED_SCHEMAS = new Map([
  ["riot.release.accessibility.v1", "accessibility"],
  ["riot.release.account-gates.v1", "account-gates"],
  ["riot.release.claims.v1", "claims"],
  ["riot.release.export-compliance.v1", "export-compliance"],
  ["riot.release.network-matrix.v1", "network-matrix"],
  ["riot.release.policy.v1", "policy"],
  ["riot.release.privacy.v1", "privacy"],
  ["riot.release.product.v1", "product"],
  ["riot.release.review-instructions.v1", "review-instructions"],
  ["riot.release.toolchains.v1", "toolchains"],
]);
const SHA256 = /^[0-9a-f]{64}$/;

function assertRegistry(registry) {
  if (typeof registry?.ajv?.getSchema !== "function") {
    throw new TypeError("validated schema registry dependency is required");
  }
}

function assertDependencies(clock, sha256, registry) {
  if (typeof clock !== "function") throw new TypeError("clock dependency is required");
  if (typeof sha256 !== "function") throw new TypeError("sha256 dependency is required");
  assertRegistry(registry);
}

function assertSchema(schema) {
  const name = FIXED_SCHEMAS.get(schema);
  if (!name) throw new TypeError(`unknown fixed schema identifier: ${schema}`);
  return name;
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

export async function createRecord({ schema, payload, clock, sha256, registry }) {
  assertDependencies(clock, sha256, registry);
  const schemaName = assertSchema(schema);
  const validatedPayload = validateSource(registry, schemaName, payload);
  const createdAt = clock().toISOString();
  if (!isExactUtcTimestamp(createdAt)) throw new TypeError("record timestamp must be exact RFC3339 UTC");
  const recordBody = { schemaVersion: 1, schema, createdAt, payload: structuredClone(validatedPayload) };
  const digest = await sha256(canonicalJson(recordBody));
  if (!SHA256.test(digest)) throw new TypeError("sha256 dependency returned an invalid digest");
  return deepFreeze({ ...recordBody, digest });
}

export async function verifyRecord(record, { sha256, registry }) {
  if (typeof sha256 !== "function") throw new TypeError("sha256 dependency is required");
  assertRegistry(registry);
  if (record === null || typeof record !== "object" || Array.isArray(record)) {
    throw new TypeError("record must be an object");
  }
  const fields = Object.keys(record).sort();
  if (fields.length !== RECORD_FIELDS.length || fields.some((field, index) => field !== RECORD_FIELDS[index])) {
    throw new TypeError("record has missing or unknown wrapper field");
  }
  if (record.schemaVersion !== 1) throw new TypeError("record schemaVersion must be 1");
  const schemaName = assertSchema(record.schema);
  if (!isExactUtcTimestamp(record.createdAt)) {
    throw new TypeError("record timestamp must be RFC3339 UTC");
  }
  if (!SHA256.test(record.digest)) throw new TypeError("record digest must be full lowercase SHA-256");
  const validatedPayload = validateSource(registry, schemaName, record.payload);
  const expected = await sha256(canonicalJson(body(record)));
  if (expected !== record.digest) throw new TypeError("record digest mismatch");
  return deepFreeze(structuredClone(validatedPayload));
}
