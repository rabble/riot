// Fail-closed JSON Schema 2020-12 gate for every durable release record. Schemas
// are compiled in Ajv strict mode so a malformed or ambiguous contract is a
// build error, never a silently-permissive pass; validators return structured
// errors, and `assertValid` raises a labeled error for call sites that treat any
// deviation as fatal.
import Ajv2020 from 'ajv/dist/2020.js';

import { canonicalize } from './canonical-json.mjs';

function newAjv() {
  return new Ajv2020({ allErrors: true, strict: true });
}

/**
 * Compile a schema into a validator returning `{ valid, errors }`. Throws if the
 * schema itself is malformed.
 */
export function createValidator(schema) {
  const validator = newAjv().compile(schema);
  return (data) => {
    const valid = validator(data);
    // Ajv populates `errors` with a non-empty array on every failed validation,
    // so an invalid result always has a concrete list to copy.
    return { valid, errors: valid ? [] : [...validator.errors] };
  };
}

/**
 * Validate `data` against `schema`, returning the data on success and throwing a
 * `label`-prefixed error carrying the canonical error list on failure.
 */
export function assertValid(schema, data, label) {
  const { valid, errors } = createValidator(schema)(data);
  if (!valid) {
    throw new Error(`${label} failed schema validation: ${canonicalize(errors)}`);
  }
  return data;
}
