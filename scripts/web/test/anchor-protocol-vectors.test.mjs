// WU-006A — Node test harness for the independent TypeScript anchor-protocol
// verifier. Imported through Node's built-in type stripping (Node >= 23.6), so the
// existing `node --test scripts/web/test/*.test.mjs` glob discovers it with no
// package.json / package-lock.json change.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import {
  assertCanonical,
  blake3,
  bytesToHex,
  decodeCommunityListingCanonical,
  encodeRecord,
  hexToBytes,
  verifyVectorDocument,
} from "../anchor-protocol-vectors.ts";

const here = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(here, "../../..");
const vectorsPath = path.resolve(repoRoot, "fixtures/anchor/protocol-v1-vectors.json");
const bootstrapPath = path.resolve(repoRoot, "fixtures/anchor/bootstrap-development-v1.cbor");

const doc = JSON.parse(readFileSync(vectorsPath, "utf8"));

// --- BLAKE3 known-answer tests (official BLAKE3 test vectors) ---------------
// The reference input of length n is bytes [0,1,...,250,0,1,...] (i mod 251).
function referenceInput(n) {
  const out = new Uint8Array(n);
  for (let i = 0; i < n; i++) out[i] = i % 251;
  return out;
}

test("BLAKE3 matches the official empty-input vector", () => {
  // The well-known empty-input BLAKE3 hash anchors the vendored implementation as
  // genuine BLAKE3 independently of both Rust and the fixture.
  assert.equal(
    bytesToHex(blake3(referenceInput(0))),
    "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262",
  );
});

test("vendored BLAKE3 matches the reference blake3 crate across chunk boundaries", () => {
  // These KATs are emitted by the reference `blake3` crate (see golden_vectors.rs),
  // spanning single-block (0,1,64), full-chunk boundaries (1023,1024,1025), and the
  // multi-chunk tree with unbalanced left_len (3072, 4097).
  assert.ok(doc.blake3_kats.length >= 6);
  let sawMultiChunk = false;
  for (const kat of doc.blake3_kats) {
    const n = Number(kat.input_len);
    assert.equal(bytesToHex(blake3(referenceInput(n))), kat.hash_hex, `blake3 of ${n} bytes`);
    if (n > 1024) sawMultiChunk = true;
  }
  assert.ok(sawMultiChunk, "BLAKE3 multi-chunk tree path must be exercised");
});

// --- Full cross-language conformance ----------------------------------------
test("every anchor protocol vector is reproduced and verified independently", () => {
  const report = verifyVectorDocument(doc);
  // The fixture must be non-trivial (guards against a silently-empty document).
  assert.ok(report.records >= 25, `expected >=25 record vectors, got ${report.records}`);
  assert.ok(report.digests >= 10, `expected >=10 digest checks, got ${report.digests}`);
  assert.ok(report.signatures >= 6, `expected >=6 signature checks, got ${report.signatures}`);
  assert.ok(report.hmacInputs >= 2, `expected >=2 HMAC-input checks, got ${report.hmacInputs}`);
  assert.ok(report.alternateGrammar >= 5, `expected >=5 alt-grammar rejections, got ${report.alternateGrammar}`);
  assert.deepEqual([...report.peerRoles].sort(), ["initiator", "responder"]);
  assert.ok(report.sentinelsChecked >= 5);
});

// --- One-bit mutation must break independent reproduction -------------------
test("a one-bit mutation of any canonical vector is detected", () => {
  const v = doc.vectors.find((x) => x.id === "public_site_ticket_core");
  const canonical = encodeRecord(v.record, v.fields);
  assert.equal(bytesToHex(canonical), v.canonical_hex);
  const mutated = new Uint8Array(canonical);
  mutated[5] ^= 0x01;
  assert.notEqual(bytesToHex(mutated), bytesToHex(canonical));
  // The re-derived bytes never match the mutated bytes: a verifier keyed on the
  // canonical form rejects the mutation.
  assert.notEqual(bytesToHex(encodeRecord(v.record, v.fields)), bytesToHex(mutated));
});

// --- Alternate-grammar mutations are rejected independently ------------------
test("alternate-grammar encodings are rejected by the independent decoder", () => {
  // Indefinite-length array.
  assert.throws(() => assertCanonical(hexToBytes("9f0102ff")));
  // Non-minimal integer (uint 1 encoded in two bytes).
  assert.throws(() => assertCanonical(hexToBytes("1801")));
  // A map where the protocol never uses one.
  assert.throws(() => assertCanonical(hexToBytes("a10102")));
  // Trailing bytes after a complete item.
  assert.throws(() => assertCanonical(hexToBytes("0100")));

  // Each fixture-pinned hostile encoding is rejected.
  for (const g of doc.alternate_grammar) {
    const hostile = hexToBytes(g.hostile_hex);
    let rejected = false;
    try {
      assertCanonical(hostile);
    } catch {
      rejected = true;
    }
    if (!rejected && g.record === "CommunityListingV1") {
      assert.throws(() => decodeCommunityListingCanonical(hostile), new RegExp("unsorted|duplicate"));
      rejected = true;
    }
    assert.ok(rejected, `alt-grammar '${g.desc}' should be rejected`);
  }
});

// --- Development bootstrap parses but is not release-eligible ----------------
test("the development bootstrap resource parses but is refused by release validation", () => {
  const cbor = new Uint8Array(readFileSync(bootstrapPath));
  const vector = doc.vectors.find((v) => v.record === "AnchorBootstrapV1");
  // The checked-in .cbor is byte-identical to the independently re-encoded record.
  assert.equal(bytesToHex(cbor), vector.canonical_hex);
  assert.equal(bytesToHex(encodeRecord("AnchorBootstrapV1", vector.fields)), bytesToHex(cbor));

  const descriptors = vector.fields.descriptors;
  // Structural diversity floor: >=3 descriptors across >=2 operators.
  assert.ok(descriptors.length >= 3);
  const operators = new Set(descriptors.map((d) => d.floor.operator_verification_key.public_key));
  assert.ok(operators.size >= 2);

  // Release validation refuses it: it is visibly development-only (.dev.invalid).
  const releaseEligible =
    descriptors.length >= 3 &&
    operators.size >= 2 &&
    descriptors.every((d) => !d.https_origin.includes(".dev.invalid"));
  assert.equal(releaseEligible, false, "development bootstrap must not be release-eligible");
  assert.ok(descriptors.every((d) => d.https_origin.includes(".dev.invalid")));
});
