// Content-addressed durable records. A record's digest is the SHA-256 of its
// canonical body, timestamps are injected (never read from the ambient clock) so
// runs are reproducible, and references between records must carry the full
// untruncated lowercase-hex digest — a truncated or re-cased reference is a
// forge attempt and fails closed.
import { createHash } from 'node:crypto';

import { canonicalize } from './canonical-json.mjs';

const FULL_DIGEST = /^[0-9a-f]{64}$/;
// RFC 3339 UTC: a `Z`-terminated instant with no numeric offset.
const RFC3339_UTC = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/;

/** SHA-256 hex digest of a value's canonical JSON form. */
export function contentDigest(value) {
  return createHash('sha256').update(canonicalize(value)).digest('hex');
}

/**
 * Assert a reference is a full 64-char lowercase-hex digest, returning it on
 * success. Truncated, upper-cased, or non-hex references fail closed.
 */
export function assertFullDigest(reference) {
  if (typeof reference !== 'string' || !FULL_DIGEST.test(reference)) {
    throw new Error(`reference is not a full 64-char lowercase-hex digest: ${reference}`);
  }
  return reference;
}

function assertRfc3339Utc(timestamp) {
  // A single fail-closed check: only a Z-terminated RFC 3339 instant is a valid
  // UTC timestamp, so a space-separated form or a numeric offset is rejected.
  if (typeof timestamp !== 'string' || !RFC3339_UTC.test(timestamp)) {
    throw new Error(`timestamp is not an RFC 3339 UTC instant: ${timestamp}`);
  }
  return timestamp;
}

/**
 * Build a durable record. `now` is an injected clock returning an RFC 3339 UTC
 * timestamp; the record carries schema version 1, that timestamp, the body, and
 * the body's content digest.
 */
export function createRecord({ recordType, body }, { now }) {
  if (typeof recordType !== 'string' || recordType.length === 0) {
    throw new Error('record requires a non-empty recordType');
  }
  const createdAt = assertRfc3339Utc(now());
  return {
    recordType,
    schemaVersion: 1,
    createdAt,
    body,
    digest: contentDigest(body),
  };
}
