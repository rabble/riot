// Durable release records are content-addressed: a record's digest is the
// SHA-256 of its canonical form, references between records must carry the full
// untruncated digest, and every record is stamped with an injected RFC 3339 UTC
// timestamp so test runs are deterministic and a bad clock cannot forge a
// plausible-looking time.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';

import { contentDigest, createRecord, assertFullDigest } from '../records.mjs';
import { canonicalize } from '../canonical-json.mjs';

const FIXED_NOW = '2026-07-24T00:00:00Z';

test('contentDigest is the SHA-256 hex of the canonical form', () => {
  const value = { b: 1, a: 2 };
  const expected = createHash('sha256').update(canonicalize(value)).digest('hex');
  assert.equal(contentDigest(value), expected);
  assert.match(contentDigest(value), /^[0-9a-f]{64}$/);
});

test('contentDigest is stable across key insertion order', () => {
  assert.equal(contentDigest({ a: 1, b: 2 }), contentDigest({ b: 2, a: 1 }));
});

test('createRecord stamps schema version 1, the injected timestamp, and a body digest', () => {
  const record = createRecord(
    { recordType: 'product', body: { name: 'Riot' } },
    { now: () => FIXED_NOW },
  );
  assert.equal(record.recordType, 'product');
  assert.equal(record.schemaVersion, 1);
  assert.equal(record.createdAt, FIXED_NOW);
  assert.deepEqual(record.body, { name: 'Riot' });
  assert.equal(record.digest, contentDigest({ name: 'Riot' }));
});

test('createRecord is deterministic for a fixed clock', () => {
  const opts = { now: () => FIXED_NOW };
  const a = createRecord({ recordType: 'product', body: { name: 'Riot' } }, opts);
  const b = createRecord({ recordType: 'product', body: { name: 'Riot' } }, opts);
  assert.deepEqual(a, b);
});

test('createRecord rejects a non-UTC or non-RFC-3339 timestamp', () => {
  assert.throws(
    () => createRecord({ recordType: 'x', body: {} }, { now: () => '2026-07-24 00:00:00' }),
    /RFC 3339/,
  );
  assert.throws(
    () => createRecord({ recordType: 'x', body: {} }, { now: () => '2026-07-24T00:00:00+05:00' }),
    /UTC/,
  );
});

test('createRecord requires a recordType', () => {
  assert.throws(() => createRecord({ body: {} }, { now: () => FIXED_NOW }), /recordType/);
});

test('assertFullDigest accepts a 64-char lowercase hex digest', () => {
  const digest = contentDigest({ a: 1 });
  assert.equal(assertFullDigest(digest), digest);
});

test('assertFullDigest rejects truncated, uppercase, or non-hex references', () => {
  const digest = contentDigest({ a: 1 });
  assert.throws(() => assertFullDigest(digest.slice(0, 32)), /full/i);
  assert.throws(() => assertFullDigest(digest.toUpperCase()), /full/i);
  assert.throws(() => assertFullDigest(`${digest.slice(0, 63)}z`), /full/i);
  assert.throws(() => assertFullDigest(''), /full/i);
});
