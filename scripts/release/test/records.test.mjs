import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

import { canonicalJson } from "../canonical-json.mjs";
import { createRecord, verifyRecord } from "../records.mjs";
import { loadSchemaRegistry } from "../schema.mjs";
import { fixedClock } from "./helpers.mjs";

const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");
const fullDigest = "a".repeat(64);
const repositoryRoot = dirname(dirname(dirname(dirname(fileURLToPath(import.meta.url)))));
const registry = await loadSchemaRegistry(join(repositoryRoot, "release", "schemas"));
const validProduct = {
  schemaVersion: 1,
  name: "riot.protest.net",
  version: "1.0",
  price: "free",
  availability: "worldwide",
  releaseChannel: "public-early-access",
  appleBundleId: "net.protest.riot",
  androidApplicationId: "net.protest.riot",
  urls: {
    privacy: { url: "https://riot.protest.net/privacy/", evidencePath: "marketing/privacy/index.html", evidenceState: "current" },
    support: { url: "https://riot.protest.net/support/", evidencePath: "marketing/support/index.html", evidenceState: "missing" },
    marketing: { url: "https://riot.protest.net/releases/", evidencePath: "marketing/releases/index.html", evidenceState: "current" },
  },
};

async function validRecord(payload = validProduct, schema = "riot.release.product.v1") {
  return createRecord({
    schema,
    payload,
    clock: fixedClock,
    sha256,
    registry,
  });
}

test("createRecord binds schemaVersion, schema, timestamp, payload, and digest", async () => {
  const record = await validRecord();
  assert.equal(record.schemaVersion, 1);
  assert.equal(record.schema, "riot.release.product.v1");
  assert.equal(record.createdAt, "2026-07-24T00:00:00.000Z");
  assert.match(record.digest, /^[0-9a-f]{64}$/);
  assert.deepEqual(await verifyRecord(record, { sha256, registry }), record.payload);
  assert(Object.isFrozen(record.payload));
});

test("verifyRecord rejects malformed wrappers fail closed", async () => {
  const record = await validRecord();
  for (const mutation of [
    (copy) => delete copy.payload,
    (copy) => { copy.surprise = true; },
    (copy) => { copy.schemaVersion = 2; },
    (copy) => { copy.schema = "riot.release.unknown.v1"; },
    (copy) => { copy.createdAt = "2026-07-24"; },
    (copy) => { copy.digest = "A".repeat(64); },
    (copy) => { copy.digest = "a".repeat(63); },
    (copy) => { copy.payload.name = "changed"; },
  ]) {
    const copy = structuredClone(record);
    mutation(copy);
    await assert.rejects(() => verifyRecord(copy, { sha256, registry }), /record|schema|timestamp|digest|field|validation/);
  }
});

test("records require full lowercase SHA-256 evidence references", async () => {
  const claims = (evidenceDigest) => ({
    schemaVersion: 1,
    claims: [{
      id: "read",
      text: "Read.",
      platforms: ["ios"],
      evidenceByPlatform: {
        ios: {
          codePaths: ["crates/riot-core/src/newswire/store.rs"],
          journeyIds: ["ios:read"],
          candidateJourney: {
            candidateId: "candidate-ios",
            journeyId: "ios:read",
            result: "PASS",
            evidenceDigest,
          },
        },
      },
      state: "human-action",
    }],
  });
  await assert.doesNotReject(() => validRecord(claims(fullDigest), "riot.release.claims.v1"));
  for (const evidenceDigest of ["a".repeat(63), "A".repeat(64), "g".repeat(64)]) {
    await assert.rejects(() => validRecord(claims(evidenceDigest), "riot.release.claims.v1"), /evidenceDigest|validation/);
  }
});

test("record schema labels cannot self-certify arbitrary payloads on create or verify", async () => {
  await assert.rejects(
    () => validRecord({ name: "riot.protest.net" }),
    /product validation failed/,
  );
  const record = await validRecord();
  const forged = { ...structuredClone(record), payload: { name: "riot.protest.net" } };
  forged.digest = sha256(canonicalJson({
    schemaVersion: forged.schemaVersion,
    schema: forged.schema,
    createdAt: forged.createdAt,
    payload: forged.payload,
  }));
  await assert.rejects(
    () => verifyRecord(forged, { sha256, registry }),
    /product validation failed/,
  );
});

test("records reject invalid hash adapters and non-object wrappers", async () => {
  await assert.rejects(
    () => createRecord({
      schema: "riot.release.product.v1",
      payload: validProduct,
      clock: fixedClock,
      sha256: () => "short",
      registry,
    }),
    /invalid digest/,
  );
  for (const record of [null, "record", []]) {
    await assert.rejects(() => verifyRecord(record, { sha256, registry }), /record must be an object/);
  }
  await assert.rejects(() => verifyRecord({}, {}), /sha256/);
});

test("records reject calendar-invalid RFC3339-shaped timestamps", async () => {
  const record = await validRecord();
  for (const createdAt of [
    "2026-99-99T00:00:00.000Z",
    "2026-02-29T00:00:00.000Z",
    "2026-04-31T00:00:00.000Z",
    "2026-01-01T24:00:00.000Z",
    "2026-01-01T00:00:00Z",
  ]) {
    const invalid = { ...record, createdAt };
    invalid.digest = sha256(canonicalJson({
      schemaVersion: invalid.schemaVersion,
      schema: invalid.schema,
      createdAt: invalid.createdAt,
      payload: invalid.payload,
    }));
    await assert.rejects(
      () => verifyRecord(invalid, { sha256, registry }),
      /timestamp/,
      createdAt,
    );
  }
  const validLeapDay = { ...record, createdAt: "2028-02-29T23:59:59.999Z" };
  validLeapDay.digest = sha256(canonicalJson({
    schemaVersion: validLeapDay.schemaVersion,
    schema: validLeapDay.schema,
    createdAt: validLeapDay.createdAt,
    payload: validLeapDay.payload,
  }));
  await assert.doesNotReject(() => verifyRecord(validLeapDay, { sha256, registry }));
});

test("createRecord requires injected clock and hash dependencies", async () => {
  await assert.rejects(
    () => createRecord({ schema: "riot.release.product.v1", payload: validProduct, clock: null, sha256, registry }),
    /clock/,
  );
  await assert.rejects(
    () => createRecord({ schema: "riot.release.product.v1", payload: validProduct, clock: fixedClock, registry }),
    /sha256/,
  );
  await assert.rejects(
    () => createRecord({ schema: "riot.release.product.v1", payload: validProduct, clock: fixedClock, sha256 }),
    /registry/,
  );
  await assert.rejects(
    () => createRecord({
      schema: "riot.release.product.v1",
      payload: validProduct,
      clock: () => ({ toISOString: () => "2026-02-29T00:00:00.000Z" }),
      sha256,
      registry,
    }),
    /timestamp/,
  );
  const record = await validRecord();
  await assert.rejects(() => verifyRecord(record, { sha256 }), /registry/);
});
