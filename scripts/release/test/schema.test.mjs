// Every durable release record is gated by a JSON Schema 2020-12 contract. The
// validator must fail closed: unknown properties, missing required fields, and
// wrong types are rejected, and a malformed schema is a build error rather than
// a silently-permissive pass.
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { createValidator, assertValid } from '../schema.mjs';

const productSchema = {
  $schema: 'https://json-schema.org/draft/2020-12/schema',
  type: 'object',
  additionalProperties: false,
  required: ['name', 'version'],
  properties: {
    name: { type: 'string', minLength: 1 },
    version: { type: 'string', pattern: '^[0-9]+\\.[0-9]+$' },
    platforms: { type: 'array', items: { type: 'string' } },
  },
};

test('a conforming object validates', () => {
  const validate = createValidator(productSchema);
  const result = validate({ name: 'Riot', version: '1.0', platforms: ['ios'] });
  assert.equal(result.valid, true);
  assert.deepEqual(result.errors, []);
});

test('an unknown property is rejected', () => {
  const validate = createValidator(productSchema);
  const result = validate({ name: 'Riot', version: '1.0', surprise: true });
  assert.equal(result.valid, false);
  assert.ok(result.errors.length > 0);
});

test('a missing required field is rejected', () => {
  const validate = createValidator(productSchema);
  const result = validate({ name: 'Riot' });
  assert.equal(result.valid, false);
});

test('a wrong-typed field is rejected', () => {
  const validate = createValidator(productSchema);
  const result = validate({ name: 'Riot', version: 10 });
  assert.equal(result.valid, false);
});

test('a pattern violation is rejected', () => {
  const validate = createValidator(productSchema);
  const result = validate({ name: 'Riot', version: 'one' });
  assert.equal(result.valid, false);
});

test('the 2020-12 dialect is in force (prefixItems is honored)', () => {
  const tupleSchema = {
    $schema: 'https://json-schema.org/draft/2020-12/schema',
    type: 'array',
    prefixItems: [{ type: 'string' }, { type: 'number' }],
    minItems: 2,
    maxItems: 2,
  };
  const validate = createValidator(tupleSchema);
  assert.equal(validate(['a', 1]).valid, true);
  assert.equal(validate(['a', 1, 'extra']).valid, false);
  assert.equal(validate([1, 'a']).valid, false);
});

test('a malformed schema fails closed at compile time', () => {
  assert.throws(() => createValidator({ type: 'not-a-real-type' }));
});

test('assertValid returns the data on success and throws with the label on failure', () => {
  const data = { name: 'Riot', version: '1.0' };
  assert.deepEqual(assertValid(productSchema, data, 'product'), data);
  assert.throws(
    () => assertValid(productSchema, { name: 'Riot' }, 'product'),
    /product/,
  );
});
