import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import test from "node:test";

import { createRecord, verifyRecord } from "../records.mjs";
import { fixedClock } from "./helpers.mjs";

const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");
const fullDigest = "a".repeat(64);

async function validRecord(payload = { name: "riot.protest.net" }) {
  return createRecord({
    schema: "riot.release.product.v1",
    payload,
    clock: fixedClock,
    sha256,
  });
}

test("createRecord binds schemaVersion, schema, timestamp, payload, and digest", async () => {
  const record = await validRecord();
  assert.equal(record.schemaVersion, 1);
  assert.equal(record.schema, "riot.release.product.v1");
  assert.equal(record.createdAt, "2026-07-24T00:00:00.000Z");
  assert.match(record.digest, /^[0-9a-f]{64}$/);
  assert.deepEqual(await verifyRecord(record, { sha256 }), record.payload);
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
    await assert.rejects(() => verifyRecord(copy, { sha256 }), /record|schema|timestamp|digest|field/);
  }
});

test("records require full lowercase SHA-256 evidence references", async () => {
  await assert.doesNotReject(() => validRecord({ evidenceDigest: fullDigest }));
  for (const evidenceDigest of ["a".repeat(63), "A".repeat(64), "g".repeat(64)]) {
    await assert.rejects(() => validRecord({ evidenceDigest }), /evidenceDigest/);
  }
});

test("createRecord requires injected clock and hash dependencies", async () => {
  await assert.rejects(
    () => createRecord({ schema: "riot.release.product.v1", payload: {}, clock: null, sha256 }),
    /clock/,
  );
  await assert.rejects(
    () => createRecord({ schema: "riot.release.product.v1", payload: {}, clock: fixedClock }),
    /sha256/,
  );
});
