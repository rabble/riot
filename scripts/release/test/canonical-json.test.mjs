import assert from "node:assert/strict";
import test from "node:test";

import { canonicalJson } from "../canonical-json.mjs";

test("canonicalJson sorts keys recursively and preserves array order", () => {
  assert.equal(
    canonicalJson({ z: [{ b: 2, a: 1 }], a: "Riot" }),
    '{"a":"Riot","z":[{"a":1,"b":2}]}\n',
  );
});

test("canonicalJson emits UTF-8 text with exactly one terminal newline", () => {
  assert.equal(canonicalJson({ message: "Kia ora 🌿\n" }), '{"message":"Kia ora 🌿\\n"}\n');
});

test("canonicalJson rejects unsafe values and non-plain objects", () => {
  for (const value of [
    { value: undefined },
    { value: Number.NaN },
    { value: Number.POSITIVE_INFINITY },
    new Date(),
    Object.create(null),
  ]) {
    assert.throws(() => canonicalJson(value), /unsupported|finite|plain object/);
  }
});

test("canonicalJson rejects cycles", () => {
  const value = {};
  value.self = value;
  assert.throws(() => canonicalJson(value), /cyclic/);
});
