// Canonical JSON is the serialization every durable release record and content
// digest is built on, so its output must be byte-identical regardless of how a
// value was constructed and must fail closed on anything JSON cannot represent
// deterministically.
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { canonicalize } from '../canonical-json.mjs';

test('object keys are emitted in sorted order regardless of insertion order', () => {
  const a = canonicalize({ b: 1, a: 2, c: 3 });
  const b = canonicalize({ c: 3, a: 2, b: 1 });
  assert.equal(a, '{"a":2,"b":1,"c":3}');
  assert.equal(a, b);
});

test('nested objects are sorted recursively', () => {
  const out = canonicalize({ z: { y: 1, x: 2 }, a: [{ q: 1, p: 2 }] });
  assert.equal(out, '{"a":[{"p":2,"q":1}],"z":{"x":2,"y":1}}');
});

test('array element order is preserved', () => {
  assert.equal(canonicalize([3, 1, 2]), '[3,1,2]');
});

test('output carries no insignificant whitespace', () => {
  assert.equal(canonicalize({ a: 1, b: [1, 2] }), '{"a":1,"b":[1,2]}');
});

test('scalars serialize like JSON', () => {
  assert.equal(canonicalize('hi'), '"hi"');
  assert.equal(canonicalize(true), 'true');
  assert.equal(canonicalize(false), 'false');
  assert.equal(canonicalize(null), 'null');
  assert.equal(canonicalize(42), '42');
});

test('strings with special characters are JSON-escaped', () => {
  assert.equal(canonicalize('a"b\n'), '"a\\"b\\n"');
});

test('non-finite numbers fail closed', () => {
  assert.throws(() => canonicalize(Number.NaN), /finite/);
  assert.throws(() => canonicalize(Number.POSITIVE_INFINITY), /finite/);
});

test('undefined values fail closed rather than being dropped', () => {
  assert.throws(() => canonicalize(undefined), /unsupported/i);
  assert.throws(() => canonicalize({ a: undefined }), /unsupported/i);
});

test('functions and symbols fail closed', () => {
  assert.throws(() => canonicalize(() => 1), /unsupported/i);
  assert.throws(() => canonicalize(Symbol('x')), /unsupported/i);
});

test('bigint fails closed because it has no canonical JSON form', () => {
  assert.throws(() => canonicalize(10n), /unsupported/i);
});
