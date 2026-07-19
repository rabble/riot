// WU-006A — independent TypeScript reproduction of the anchor protocol wire format.
//
// This module is the second, INDEPENDENT implementation of the canonical anchor
// records described by `fixtures/anchor/protocol-v1-vectors.json`. It never asks
// Rust for expected bytes: it re-encodes every record from the vector's semantic
// fields, re-frames and re-hashes every digest/preimage with a vendored BLAKE3,
// re-derives every HMAC input, and verifies every ed25519 signature with Node's
// built-in crypto. Agreement with the Rust-emitted fixture is the cross-language
// conformance proof.
//
// Everything here is dependency-free (Node built-ins only) so the existing
// `node --test scripts/web/test/*.test.mjs` glob runs it through Node's built-in
// type stripping with no package.json / package-lock.json change.

import { createPublicKey, verify as nodeVerify } from "node:crypto";

// ===========================================================================
// Vendored BLAKE3 (pure TypeScript, 32-byte output).
//
// Implements the BLAKE3 tree hash for the default (unkeyed) mode. Self-tested
// against the official test vectors in the companion `.test.mjs`.
// ===========================================================================

const BLAKE3_IV = Uint32Array.from([
  0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c,
  0x1f83d9ab, 0x5be0cd19,
]);
const BLAKE3_MSG_PERMUTATION = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8];
const CHUNK_START = 1 << 0;
const CHUNK_END = 1 << 1;
const PARENT = 1 << 2;
const ROOT = 1 << 3;
const BLAKE3_BLOCK_LEN = 64;
const BLAKE3_CHUNK_LEN = 1024;

function rotr32(x: number, n: number): number {
  return ((x >>> n) | (x << (32 - n))) >>> 0;
}

function g(v: Uint32Array, a: number, b: number, c: number, d: number, mx: number, my: number): void {
  v[a] = (v[a] + v[b] + mx) >>> 0;
  v[d] = rotr32(v[d] ^ v[a], 16);
  v[c] = (v[c] + v[d]) >>> 0;
  v[b] = rotr32(v[b] ^ v[c], 12);
  v[a] = (v[a] + v[b] + my) >>> 0;
  v[d] = rotr32(v[d] ^ v[a], 8);
  v[c] = (v[c] + v[d]) >>> 0;
  v[b] = rotr32(v[b] ^ v[c], 7);
}

function blake3Compress(
  cv: Uint32Array,
  blockWords: Uint32Array,
  counter: bigint,
  blockLen: number,
  flags: number,
): Uint32Array {
  const v = new Uint32Array(16);
  v.set(cv.subarray(0, 8), 0);
  v.set(BLAKE3_IV.subarray(0, 4), 8);
  v[12] = Number(counter & 0xffffffffn) >>> 0;
  v[13] = Number((counter >> 32n) & 0xffffffffn) >>> 0;
  v[14] = blockLen >>> 0;
  v[15] = flags >>> 0;

  let m = blockWords;
  for (let round = 0; round < 7; round++) {
    g(v, 0, 4, 8, 12, m[0], m[1]);
    g(v, 1, 5, 9, 13, m[2], m[3]);
    g(v, 2, 6, 10, 14, m[4], m[5]);
    g(v, 3, 7, 11, 15, m[6], m[7]);
    g(v, 0, 5, 10, 15, m[8], m[9]);
    g(v, 1, 6, 11, 12, m[10], m[11]);
    g(v, 2, 7, 8, 13, m[12], m[13]);
    g(v, 3, 4, 9, 14, m[14], m[15]);
    if (round < 6) {
      const permuted = new Uint32Array(16);
      for (let i = 0; i < 16; i++) permuted[i] = m[BLAKE3_MSG_PERMUTATION[i]];
      m = permuted;
    }
  }
  const out = new Uint32Array(16);
  for (let i = 0; i < 8; i++) {
    out[i] = (v[i] ^ v[i + 8]) >>> 0;
    out[i + 8] = (v[i + 8] ^ cv[i]) >>> 0;
  }
  return out;
}

function wordsFromBlock(block: Uint8Array): Uint32Array {
  // 64-byte little-endian block → 16 words (short blocks are zero-padded).
  const padded = new Uint8Array(BLAKE3_BLOCK_LEN);
  padded.set(block.subarray(0, BLAKE3_BLOCK_LEN));
  const words = new Uint32Array(16);
  const view = new DataView(padded.buffer, padded.byteOffset, BLAKE3_BLOCK_LEN);
  for (let i = 0; i < 16; i++) words[i] = view.getUint32(i * 4, true);
  return words;
}

interface Blake3Output {
  inputCv: Uint32Array;
  blockWords: Uint32Array;
  counter: bigint;
  blockLen: number;
  flags: number;
}

function outputChainingValue(o: Blake3Output): Uint32Array {
  return blake3Compress(o.inputCv, o.blockWords, o.counter, o.blockLen, o.flags).subarray(0, 8);
}

function outputRootBytes(o: Blake3Output, length: number): Uint8Array {
  const out = new Uint8Array(length);
  let counter = 0n;
  let filled = 0;
  while (filled < length) {
    const words = blake3Compress(o.inputCv, o.blockWords, counter, o.blockLen, o.flags | ROOT);
    for (let i = 0; i < 16 && filled < length; i++) {
      let w = words[i] >>> 0;
      for (let b = 0; b < 4 && filled < length; b++) {
        out[filled++] = w & 0xff;
        w >>>= 8;
      }
    }
    counter += 1n;
  }
  return out;
}

function chunkToOutput(bytes: Uint8Array, chunkCounter: bigint): Blake3Output {
  let cv = Uint32Array.from(BLAKE3_IV);
  const blockCount = bytes.length === 0 ? 1 : Math.ceil(bytes.length / BLAKE3_BLOCK_LEN);
  for (let i = 0; i < blockCount; i++) {
    const start = i * BLAKE3_BLOCK_LEN;
    const block = bytes.subarray(start, start + BLAKE3_BLOCK_LEN);
    const blockLen = block.length;
    const words = wordsFromBlock(block);
    let flags = 0;
    if (i === 0) flags |= CHUNK_START;
    const isLast = i === blockCount - 1;
    if (isLast) {
      return { inputCv: cv, blockWords: words, counter: chunkCounter, blockLen, flags: flags | CHUNK_END };
    }
    cv = blake3Compress(cv, words, chunkCounter, BLAKE3_BLOCK_LEN, flags).subarray(0, 8);
  }
  // Unreachable (blockCount >= 1 always returns from the last block).
  throw new Error("blake3: empty chunk loop");
}

function parentOutput(leftCv: Uint32Array, rightCv: Uint32Array): Blake3Output {
  const block = new Uint32Array(16);
  block.set(leftCv.subarray(0, 8), 0);
  block.set(rightCv.subarray(0, 8), 8);
  return { inputCv: Uint32Array.from(BLAKE3_IV), blockWords: block, counter: 0n, blockLen: BLAKE3_BLOCK_LEN, flags: PARENT };
}

function leftLen(contentLen: number): number {
  const fullChunks = Math.floor((contentLen - 1) / BLAKE3_CHUNK_LEN);
  let p = 1;
  while (p * 2 <= fullChunks) p *= 2;
  return p * BLAKE3_CHUNK_LEN;
}

function hashRange(bytes: Uint8Array, chunkCounter: bigint): Blake3Output {
  if (bytes.length <= BLAKE3_CHUNK_LEN) {
    return chunkToOutput(bytes, chunkCounter);
  }
  const ll = leftLen(bytes.length);
  const left = hashRange(bytes.subarray(0, ll), chunkCounter);
  const right = hashRange(bytes.subarray(ll), chunkCounter + BigInt(ll / BLAKE3_CHUNK_LEN));
  return parentOutput(outputChainingValue(left), outputChainingValue(right));
}

export function blake3(input: Uint8Array): Uint8Array {
  return outputRootBytes(hashRange(input, 0n), 32);
}

// ===========================================================================
// Hex / bytes helpers.
// ===========================================================================

export function hexToBytes(hex: string): Uint8Array {
  if (hex.length % 2 !== 0) throw new Error(`odd-length hex: ${hex.length}`);
  const out = new Uint8Array(hex.length / 2);
  for (let i = 0; i < out.length; i++) {
    out[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  }
  return out;
}

export function bytesToHex(bytes: Uint8Array): string {
  let s = "";
  for (const b of bytes) s += b.toString(16).padStart(2, "0");
  return s;
}

function utf8(s: string): Uint8Array {
  return new TextEncoder().encode(s);
}

function concatBytes(...parts: Uint8Array[]): Uint8Array {
  const total = parts.reduce((n, p) => n + p.length, 0);
  const out = new Uint8Array(total);
  let off = 0;
  for (const p of parts) {
    out.set(p, off);
    off += p.length;
  }
  return out;
}

function u16be(n: number): Uint8Array {
  return Uint8Array.from([(n >>> 8) & 0xff, n & 0xff]);
}
function u32be(n: number): Uint8Array {
  return Uint8Array.from([(n >>> 24) & 0xff, (n >>> 16) & 0xff, (n >>> 8) & 0xff, n & 0xff]);
}
function u64be(n: bigint): Uint8Array {
  const out = new Uint8Array(8);
  let v = n;
  for (let i = 7; i >= 0; i--) {
    out[i] = Number(v & 0xffn);
    v >>= 8n;
  }
  return out;
}

function bytesEqual(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
  return true;
}

function compareBytes(a: Uint8Array, b: Uint8Array): number {
  const n = Math.min(a.length, b.length);
  for (let i = 0; i < n; i++) {
    if (a[i] !== b[i]) return a[i] - b[i];
  }
  return a.length - b.length;
}

// ===========================================================================
// Canonical positional-CBOR encoder (minimal ints, definite lengths, sorted
// sets — never maps, never indefinite lengths). This is the independent codec.
// ===========================================================================

class Writer {
  private readonly bytes: number[] = [];

  private head(major: number, n: bigint): void {
    const m = major << 5;
    if (n < 24n) this.bytes.push(m | Number(n));
    else if (n < 0x100n) this.bytes.push(m | 24, Number(n));
    else if (n < 0x10000n) {
      this.bytes.push(m | 25);
      this.pushBE(n, 2);
    } else if (n < 0x100000000n) {
      this.bytes.push(m | 26);
      this.pushBE(n, 4);
    } else {
      this.bytes.push(m | 27);
      this.pushBE(n, 8);
    }
  }

  private pushBE(n: bigint, len: number): void {
    for (let i = len - 1; i >= 0; i--) this.bytes.push(Number((n >> (8n * BigInt(i))) & 0xffn));
  }

  uint(n: bigint | number | string): this {
    this.head(0, BigInt(n));
    return this;
  }
  bstr(u8: Uint8Array): this {
    this.head(2, BigInt(u8.length));
    for (const b of u8) this.bytes.push(b);
    return this;
  }
  tstr(s: string): this {
    const u8 = utf8(s);
    this.head(3, BigInt(u8.length));
    for (const b of u8) this.bytes.push(b);
    return this;
  }
  bool(b: boolean): this {
    this.bytes.push(b ? 0xf5 : 0xf4);
    return this;
  }
  nul(): this {
    this.bytes.push(0xf6);
    return this;
  }
  arr(n: number): this {
    this.head(4, BigInt(n));
    return this;
  }
  raw(u8: Uint8Array): this {
    for (const b of u8) this.bytes.push(b);
    return this;
  }
  out(): Uint8Array {
    return Uint8Array.from(this.bytes);
  }
}

/** Encode a sorted canonical set from pre-encoded element bytes. */
function encodeSet(w: Writer, elements: Uint8Array[]): void {
  const sorted = [...elements].sort(compareBytes);
  for (let i = 1; i < sorted.length; i++) {
    if (compareBytes(sorted[i - 1], sorted[i]) === 0) {
      throw new Error("duplicate set member");
    }
  }
  w.arr(sorted.length);
  for (const e of sorted) w.raw(e);
}

function encByteSet(hexes: string[]): Uint8Array[] {
  return hexes.map((h) => new Writer().bstr(hexToBytes(h)).out());
}
function encTextSet(items: string[]): Uint8Array[] {
  return items.map((s) => new Writer().tstr(s).out());
}
function encUintSet(items: string[]): Uint8Array[] {
  return items.map((s) => new Writer().uint(s).out());
}

// ---------------------------------------------------------------------------
// Field accessors (all wire integers arrive as decimal strings).
// ---------------------------------------------------------------------------

type Fields = Record<string, any>;

const bx = (f: any): Uint8Array => hexToBytes(f as string);

export const COMMUNITY_LISTING_SCHEMA = "riot/community-listing/1";

// ---------------------------------------------------------------------------
// Per-record encoders. Each mirrors the crate's `encode_canonical` positionally.
// ---------------------------------------------------------------------------

function encOperatorKey(f: Fields): Uint8Array {
  return new Writer().arr(3).uint(1).tstr("ed25519").bstr(bx(f.public_key)).out();
}

function encPublicSiteTicketCore(f: Fields): Uint8Array {
  return new Writer()
    .arr(12)
    .bstr(bx(f.root_id))
    .bstr(bx(f.o_namespace_id))
    .bstr(bx(f.c_namespace_id))
    .bstr(bx(f.w_namespace_id))
    .bstr(bx(f.manifest_digest))
    .uint(f.manifest_version)
    .uint(f.min_sync_version)
    .tstr(f.manifest_required_transport)
    .tstr(f.transport_floor)
    .uint(f.transport_epoch)
    .uint(f.issued_unix_seconds)
    .uint(f.expiry_unix_seconds)
    .out();
}

function encRootSignedTicketCoreEnvelope(f: Fields): Uint8Array {
  return new Writer()
    .arr(3)
    .uint(2)
    .raw(encPublicSiteTicketCore(f.core))
    .bstr(bx(f.root_signature))
    .out();
}

function encListingDelegateGrant(f: Fields): Uint8Array {
  return new Writer()
    .arr(6)
    .bstr(bx(f.root_id))
    .bstr(bx(f.delegate_key))
    .bstr(bx(f.terminal_capability_digest))
    .uint(f.listing_epoch)
    .uint(f.issued_unix_seconds)
    .uint(f.expiry_unix_seconds)
    .out();
}

function encCommunityListing(f: Fields): Uint8Array {
  if (f.schema !== COMMUNITY_LISTING_SCHEMA) throw new Error(`unexpected schema ${f.schema}`);
  const w = new Writer()
    .arr(18)
    .tstr(COMMUNITY_LISTING_SCHEMA)
    .bstr(bx(f.root_id))
    .bstr(bx(f.o_namespace_id))
    .bstr(bx(f.c_namespace_id))
    .bstr(bx(f.w_namespace_id))
    .bstr(bx(f.manifest_digest))
    .uint(f.manifest_version)
    .bstr(bx(f.ticket_core_bytes))
    .uint(f.listing_epoch)
    .uint(f.listing_revision)
    .bool(f.listed)
    .tstr(f.title)
    .tstr(f.summary);
  encodeSet(w, encByteSet(f.topic_tags));
  encodeSet(w, encTextSet(f.languages));
  if (f.region === null) w.nul();
  else w.bstr(bx(f.region));
  w.uint(f.issued_unix_seconds).uint(f.expiry_unix_seconds);
  return w.out();
}

function encAdmittedListingEnvelope(f: Fields): Uint8Array {
  const w = new Writer()
    .arr(4)
    .uint(1)
    .bstr(bx(f.signed_listing_entry_bytes))
    .bstr(bx(f.capability_chain_bytes));
  if (f.delegate_grant_bytes === null) w.nul();
  else w.bstr(bx(f.delegate_grant_bytes));
  return w.out();
}

function encLimitValue(w: Writer, v: any): void {
  if (Array.isArray(v)) w.arr(2).uint(v[0]).uint(v[1]);
  else w.uint(v);
}

function encAnchorLimitProfile(f: Fields): Uint8Array {
  const w = new Writer().arr(3).uint(1).uint(f.profile_epoch);
  w.arr(f.entries.length);
  for (const e of f.entries) {
    w.arr(3).uint(e.id);
    encLimitValue(w, e.effective);
    encLimitValue(w, e.absolute);
  }
  return w.out();
}

function encDescriptorFloor(f: Fields): Uint8Array {
  return new Writer()
    .arr(4)
    .bstr(bx(f.anchor_id))
    .uint(f.descriptor_epoch)
    .bstr(bx(f.descriptor_digest))
    .raw(encOperatorKey(f.operator_verification_key))
    .out();
}

function encRoleSet(w: Writer, roles: string[]): void {
  encodeSet(w, encTextSet(roles));
}

function encAnchorDescriptorBody(f: Fields): Uint8Array {
  const w = new Writer()
    .arr(19)
    .uint(1)
    .bstr(bx(f.anchor_id))
    .bstr(bx(f.genesis_operator_public_key))
    .bstr(bx(f.genesis_random_256_bits))
    .raw(encOperatorKey(f.current_operator_verification_key))
    .bstr(bx(f.current_operator_key_id))
    .uint(f.descriptor_epoch);
  if (f.previous_descriptor_digest === null) w.nul();
  else w.bstr(bx(f.previous_descriptor_digest));
  w.bstr(bx(f.current_iroh_endpoint_id))
    .tstr(f.https_origin)
    .tstr(f.operator_display_label)
    .tstr(f.self_reported_failure_domain_label);
  encodeSet(w, encUintSet(f.supported_control_versions));
  encodeSet(w, encUintSet(f.supported_sync_versions));
  encRoleSet(w, f.enabled_roles);
  w.bstr(bx(f.limit_profile_digest));
  if (f.predecessor_operator_verification_key === null) w.nul();
  else w.raw(encOperatorKey(f.predecessor_operator_verification_key));
  w.uint(f.issued_at).uint(f.expires_at);
  return w.out();
}

function encDescriptorEnvelope(f: Fields): Uint8Array {
  const w = new Writer().arr(4).uint(1).raw(encAnchorDescriptorBody(f.body)).bstr(bx(f.current_signature));
  if (f.predecessor_signature === null) w.nul();
  else w.bstr(bx(f.predecessor_signature));
  return w.out();
}

function encNamespaceResult(w: Writer, r: Fields): void {
  w.arr(3).bstr(bx(r.namespace_id)).bstr(bx(r.snapshot_digest)).uint(r.entry_count);
}

function encHostingReceiptBody(f: Fields): Uint8Array {
  const w = new Writer()
    .arr(16)
    .uint(1)
    .bstr(bx(f.anchor_id))
    .bstr(bx(f.operator_key_id))
    .uint(f.descriptor_epoch)
    .bstr(bx(f.descriptor_digest))
    .bstr(bx(f.hosting_operation_id))
    .bstr(bx(f.full_site_root))
    .bstr(bx(f.manifest_digest))
    .uint(f.manifest_version)
    .uint(f.base_site_generation)
    .uint(f.committed_site_generation);
  w.arr(3);
  for (const r of f.ordered_namespace_results) encNamespaceResult(w, r);
  w.tstr(f.status).uint(f.accepted_at).uint(f.reported_retention_through).bstr(bx(f.limit_profile_digest));
  return w.out();
}

function encHostingReceiptEnvelope(f: Fields): Uint8Array {
  return new Writer().arr(3).uint(1).raw(encHostingReceiptBody(f.body)).bstr(bx(f.operator_signature)).out();
}

function encListingReceiptBody(f: Fields): Uint8Array {
  return new Writer()
    .arr(13)
    .uint(1)
    .bstr(bx(f.anchor_id))
    .bstr(bx(f.operator_key_id))
    .uint(f.descriptor_epoch)
    .bstr(bx(f.descriptor_digest))
    .bstr(bx(f.listing_digest))
    .bstr(bx(f.full_site_root))
    .uint(f.accepted_listing_epoch)
    .uint(f.accepted_listing_revision)
    .uint(f.feed_coordinate)
    .uint(f.accepted_at)
    .uint(f.expires_at)
    .bstr(bx(f.request_idempotency_key))
    .out();
}

function encListingReceiptEnvelope(f: Fields): Uint8Array {
  return new Writer().arr(3).uint(1).raw(encListingReceiptBody(f.body)).bstr(bx(f.operator_signature)).out();
}

function encWorkChallengeBody(f: Fields): Uint8Array {
  return new Writer()
    .arr(14)
    .uint(1)
    .bstr(bx(f.anchor_id))
    .bstr(bx(f.operator_key_id))
    .uint(f.descriptor_epoch)
    .bstr(bx(f.descriptor_digest))
    .tstr(f.operation_kind)
    .bstr(bx(f.idempotency_key))
    .bstr(bx(f.work_target_digest))
    .bstr(bx(f.community_root))
    .bstr(bx(f.random_challenge))
    .uint(f.policy_epoch)
    .uint(f.difficulty)
    .uint(f.issued_at)
    .uint(f.expires_at)
    .out();
}

function encWorkChallengeEnvelope(f: Fields): Uint8Array {
  return new Writer().arr(3).uint(1).raw(encWorkChallengeBody(f.body)).bstr(bx(f.operator_signature)).out();
}

function encWorkStamp(f: Fields): Uint8Array {
  return new Writer()
    .arr(4)
    .uint(1)
    .bstr(bx(f.challenge_envelope_bytes))
    .uint(f.counter)
    .bstr(bx(f.proof_bytes))
    .out();
}

function encReplicaPrepareChallenge(f: Fields): Uint8Array {
  return new Writer()
    .arr(7)
    .uint(1)
    .bstr(bx(f.destination_anchor_id))
    .bstr(bx(f.random_256_bit_nonce))
    .bstr(bx(f.prepare_idempotency_key))
    .bstr(bx(f.full_site_root))
    .uint(f.issued_at)
    .uint(f.expires_at)
    .out();
}

function encReplicaSourceAttestationBody(f: Fields): Uint8Array {
  const w = new Writer()
    .arr(16)
    .uint(1)
    .bstr(bx(f.source_anchor_id))
    .bstr(bx(f.source_current_operator_key_id))
    .uint(f.source_current_descriptor_epoch)
    .bstr(bx(f.source_current_descriptor_digest))
    .bstr(bx(f.destination_anchor_id))
    .bstr(bx(f.peer_transcript_digest))
    .bstr(bx(f.destination_prepare_nonce))
    .bstr(bx(f.prepare_idempotency_key))
    .bstr(bx(f.full_site_root))
    .arr(2)
    .bstr(bx(f.manifest_digest))
    .uint(f.manifest_version)
    .bstr(bx(f.root_signed_ticket_core_digest))
    .uint(f.source_site_generation);
  w.arr(3);
  for (const d of f.ordered_namespace_snapshot_digests) w.bstr(bx(d));
  w.uint(f.issued_at).uint(f.expires_at);
  return w.out();
}

function encReplicaSourceAttestationEnvelope(f: Fields): Uint8Array {
  return new Writer().arr(3).uint(1).raw(encReplicaSourceAttestationBody(f.body)).bstr(bx(f.operator_signature)).out();
}

function encSnapshotCursorBody(f: Fields): Uint8Array {
  const w = new Writer()
    .arr(8)
    .uint(1)
    .bstr(bx(f.checkpoint_digest))
    .uint(f.snapshot_generation_id)
    .uint(f.next_ordinal);
  if (f.previous_root === null) w.nul();
  else w.bstr(bx(f.previous_root));
  w.uint(f.issued_at).uint(f.expires_at).uint(f.cursor_secret_epoch);
  return w.out();
}

function encSnapshotCursor(f: Fields): Uint8Array {
  return new Writer().arr(3).uint(1).raw(encSnapshotCursorBody(f.body)).bstr(bx(f.cursor_tag)).out();
}

function encBootstrapDescriptor(d: Fields): Uint8Array {
  const w = new Writer().arr(4).uint(1).raw(encDescriptorFloor(d.floor)).tstr(d.https_origin);
  encRoleSet(w, d.roles);
  return w.out();
}

function encAnchorBootstrap(f: Fields): Uint8Array {
  const w = new Writer().arr(2).uint(1).arr(f.descriptors.length);
  for (const d of f.descriptors) w.raw(encBootstrapDescriptor(d));
  return w.out();
}

/** The describe operation's semantic body: `[1, 1]`. */
function encDescribeSemanticBody(): Uint8Array {
  return new Writer().arr(1).uint(1).out();
}

function encControlRequest(f: Fields): Uint8Array {
  if (f.semantic.kind !== "describe") {
    throw new Error(`control request encoder only supports 'describe', got ${f.semantic.kind}`);
  }
  return new Writer()
    .arr(4)
    .uint(1)
    .tstr(f.operation_kind)
    .bstr(bx(f.idempotency_key))
    .raw(encDescribeSemanticBody())
    .out();
}

/** The NotHosted refusal: `["not_hosted", "listing", true, null, ["none"]]`. */
function encNotHostedRefusal(): Uint8Array {
  return new Writer().arr(5).tstr("not_hosted").tstr("listing").bool(true).nul().arr(1).tstr("none").out();
}

function encControlResponse(f: Fields): Uint8Array {
  const w = new Writer().arr(3).uint(1).tstr(f.kind);
  if (f.outcome.type === "refused") {
    if (f.outcome.refusal.code !== "not_hosted") {
      throw new Error(`control response encoder only supports the not_hosted refusal, got ${f.outcome.refusal.code}`);
    }
    w.arr(2).tstr("refused").raw(encNotHostedRefusal());
  } else if (f.outcome.type === "success") {
    if (f.outcome.success.kind !== "submit_listing") {
      throw new Error(`control response encoder only supports the submit_listing success, got ${f.outcome.success.kind}`);
    }
    // submit_listing success payload = [1, listing_receipt].
    const payload = new Writer().arr(2).uint(1).raw(encListingReceiptEnvelope(f.outcome.success.listing_receipt)).out();
    w.arr(2).tstr("success").raw(payload);
  } else {
    throw new Error(`unknown outcome ${f.outcome.type}`);
  }
  return w.out();
}

const RECORD_ENCODERS: Record<string, (f: Fields) => Uint8Array> = {
  OperatorVerificationKeyV1: encOperatorKey,
  PublicSiteTicketV2Core: encPublicSiteTicketCore,
  RootSignedTicketCoreEnvelopeV2: encRootSignedTicketCoreEnvelope,
  ListingDelegateGrantV1: encListingDelegateGrant,
  CommunityListingV1: encCommunityListing,
  AdmittedListingEnvelopeV1: encAdmittedListingEnvelope,
  AnchorLimitProfileV1: encAnchorLimitProfile,
  DescriptorFloor: encDescriptorFloor,
  AnchorDescriptorBodyV1: encAnchorDescriptorBody,
  DescriptorEnvelopeV1: encDescriptorEnvelope,
  HostingReceiptBodyV1: encHostingReceiptBody,
  HostingReceiptV1: encHostingReceiptEnvelope,
  ListingReceiptBodyV1: encListingReceiptBody,
  ListingReceiptV1: encListingReceiptEnvelope,
  WorkChallengeBodyV1: encWorkChallengeBody,
  WorkChallengeV1: encWorkChallengeEnvelope,
  WorkStampV1: encWorkStamp,
  ReplicaPrepareChallengeV1: encReplicaPrepareChallenge,
  ReplicaSourceAttestationBodyV1: encReplicaSourceAttestationBody,
  ReplicaSourceAttestationV1: encReplicaSourceAttestationEnvelope,
  SnapshotCursorBodyV1: encSnapshotCursorBody,
  SnapshotCursorV1: encSnapshotCursor,
  AnchorBootstrapV1: encAnchorBootstrap,
  ControlRequestV1: encControlRequest,
  ControlResponseV1: encControlResponse,
};

export function encodeRecord(record: string, fields: Fields): Uint8Array {
  const enc = RECORD_ENCODERS[record];
  if (!enc) throw new Error(`no TS encoder for record ${record}`);
  return enc(fields);
}

// ===========================================================================
// Domain-separation labels (TS's own independent copy; cross-checked against the
// fixture's label_ascii strings).
// ===========================================================================

const LABELS = {
  operator_key_id: "riot/anchor-operator-key-id/v1",
  anchor_id: "riot/anchor-id/v1",
  work_proof: "riot/anchor-work-proof/v1",
  sync_snapshot: "riot/sync-snapshot/v2",
  namespace_token: "riot/namespace-token/v1",
  snapshot_cursor: "riot/directory-snapshot-cursor/v1",
  peer_proof: "riot/anchor-peer-proof/v1",
} as const;

// ===========================================================================
// Digest / preimage derivations.
// ===========================================================================

export function digestV1Preimage(labelAscii: string, message: Uint8Array): Uint8Array {
  const label = utf8(labelAscii);
  return concatBytes(u16be(label.length), label, u64be(BigInt(message.length)), message);
}

export function digestV1(labelAscii: string, message: Uint8Array): Uint8Array {
  return blake3(digestV1Preimage(labelAscii, message));
}

function operatorKeyIdPreimage(canonical: Uint8Array): Uint8Array {
  return concatBytes(utf8(LABELS.operator_key_id), canonical);
}
function anchorIdPreimage(genesisPk: Uint8Array, genesisRand: Uint8Array): Uint8Array {
  return concatBytes(utf8(LABELS.anchor_id), genesisPk, genesisRand);
}
function workProofPreimage(challengeDigest: Uint8Array, counter: bigint): Uint8Array {
  return concatBytes(utf8(LABELS.work_proof), challengeDigest, u64be(counter));
}
function syncSnapshotDigest(nsId: Uint8Array, entryCount: bigint, logicalBytes: bigint, sortedIds: Uint8Array[]): Uint8Array {
  const parts: Uint8Array[] = [utf8(LABELS.sync_snapshot), u32be(nsId.length), nsId, u64be(entryCount), u64be(logicalBytes)];
  for (const id of sortedIds) {
    parts.push(u32be(id.length), id);
  }
  return blake3(concatBytes(...parts));
}
function namespaceTokenHmacInput(opId: Uint8Array, nsId: Uint8Array, expiry: bigint, epoch: number): Uint8Array {
  const label = utf8(LABELS.namespace_token);
  return concatBytes(u16be(23), label, u16be(opId.length), opId, u16be(nsId.length), nsId, u64be(expiry), u32be(epoch));
}
function snapshotCursorHmacInput(canonical: Uint8Array): Uint8Array {
  const label = utf8(LABELS.snapshot_cursor);
  return concatBytes(u16be(33), label, u64be(BigInt(canonical.length)), canonical);
}
function peerProofPreimage(role: string, transcriptDigest: Uint8Array): Uint8Array {
  const label = utf8(LABELS.peer_proof);
  const roleBytes = utf8(role);
  return concatBytes(u16be(25), label, u16be(roleBytes.length), roleBytes, transcriptDigest);
}

// ===========================================================================
// ed25519 verification (Node built-in crypto, raw 32-byte key).
// ===========================================================================

const ED25519_SPKI_PREFIX = hexToBytes("302a300506032b6570032100");

export function ed25519Verify(publicKey: Uint8Array, message: Uint8Array, signature: Uint8Array): boolean {
  if (publicKey.length !== 32 || signature.length !== 64) return false;
  const der = concatBytes(ED25519_SPKI_PREFIX, publicKey);
  let key;
  try {
    key = createPublicKey({ key: Buffer.from(der), format: "der", type: "spki" });
  } catch {
    return false;
  }
  return nodeVerify(null, Buffer.from(message), key, Buffer.from(signature));
}

// ===========================================================================
// Generic canonical-CBOR validator + a CommunityListing set-order decoder, used
// to prove alternate-grammar encodings are rejected independently of Rust.
// ===========================================================================

class Reader {
  private readonly bytes: Uint8Array;
  public pos: number;

  constructor(bytes: Uint8Array, pos = 0) {
    this.bytes = bytes;
    this.pos = pos;
  }

  private byte(): number {
    if (this.pos >= this.bytes.length) throw new Error("unexpected end of input");
    return this.bytes[this.pos++];
  }

  remaining(): number {
    return this.bytes.length - this.pos;
  }

  /** Read a definite-length head, enforcing minimal integer encoding. */
  private head(): { major: number; value: bigint } {
    const ib = this.byte();
    const major = ib >> 5;
    const ai = ib & 0x1f;
    if (ai < 24) return { major, value: BigInt(ai) };
    if (ai === 24) {
      const v = this.byte();
      if (v < 24) throw new Error("non-minimal integer");
      return { major, value: BigInt(v) };
    }
    if (ai === 25) {
      const v = (BigInt(this.byte()) << 8n) | BigInt(this.byte());
      if (v < 0x100n) throw new Error("non-minimal integer");
      return { major, value: v };
    }
    if (ai === 26) {
      let v = 0n;
      for (let i = 0; i < 4; i++) v = (v << 8n) | BigInt(this.byte());
      if (v < 0x10000n) throw new Error("non-minimal integer");
      return { major, value: v };
    }
    if (ai === 27) {
      let v = 0n;
      for (let i = 0; i < 8; i++) v = (v << 8n) | BigInt(this.byte());
      if (v < 0x100000000n) throw new Error("non-minimal integer");
      return { major, value: v };
    }
    throw new Error("indefinite or reserved length");
  }

  /** Recursively validate one canonical item; the protocol forbids maps/tags/floats. */
  validateItem(): void {
    const startByte = this.bytes[this.pos];
    const major = startByte >> 5;
    if (major === 5) throw new Error("maps are never canonical in this protocol");
    if (major === 6) throw new Error("tags are never canonical in this protocol");
    if (major === 7) {
      const ai = startByte & 0x1f;
      // Allow only false/true/null (20/21/22); reject floats & other simples.
      if (ai !== 20 && ai !== 21 && ai !== 22) throw new Error("unsupported simple/float value");
      this.pos++;
      return;
    }
    const { major: m, value } = this.head();
    if (m === 0 || m === 1) return; // (neg ints unused, but accept minimal form)
    if (m === 2 || m === 3) {
      const len = Number(value);
      for (let i = 0; i < len; i++) this.byte();
      return;
    }
    if (m === 4) {
      const len = Number(value);
      for (let i = 0; i < len; i++) this.validateItem();
      return;
    }
    throw new Error(`unexpected major type ${m}`);
  }

  // -- typed readers for the CommunityListing set-order decoder --------------
  arrayHead(): number {
    const { major, value } = this.head();
    if (major !== 4) throw new Error("expected array");
    return Number(value);
  }
  uintHead(): bigint {
    const { major, value } = this.head();
    if (major !== 0) throw new Error("expected uint");
    return value;
  }
  readBytes(): Uint8Array {
    const { major, value } = this.head();
    if (major !== 2) throw new Error("expected byte string");
    const len = Number(value);
    const out = this.bytes.subarray(this.pos, this.pos + len);
    this.pos += len;
    return out;
  }
  readText(): string {
    const { major, value } = this.head();
    if (major !== 3) throw new Error("expected text string");
    const len = Number(value);
    const out = this.bytes.subarray(this.pos, this.pos + len);
    this.pos += len;
    return new TextDecoder().decode(out);
  }
  readBool(): boolean {
    const b = this.byte();
    if (b === 0xf5) return true;
    if (b === 0xf4) return false;
    throw new Error("expected bool");
  }
  peekNull(): boolean {
    return this.bytes[this.pos] === 0xf6;
  }
  readNull(): void {
    if (this.byte() !== 0xf6) throw new Error("expected null");
  }
}

/** Assert `bytes` is a single canonical CBOR item with no trailing bytes. */
export function assertCanonical(bytes: Uint8Array): void {
  const r = new Reader(bytes);
  r.validateItem();
  if (r.remaining() !== 0) throw new Error("trailing bytes after canonical item");
}

/** Decode a CommunityListingV1 enforcing sorted, duplicate-free topic/language sets. */
export function decodeCommunityListingCanonical(bytes: Uint8Array): void {
  const r = new Reader(bytes);
  if (r.arrayHead() !== 18) throw new Error("expected 18-element listing");
  if (r.readText() !== COMMUNITY_LISTING_SCHEMA) throw new Error("bad schema");
  for (let i = 0; i < 5; i++) r.readBytes(); // root_id, o/c/w ns, manifest_digest
  r.uintHead(); // manifest_version
  r.readBytes(); // ticket_core_bytes
  r.uintHead(); // listing_epoch
  r.uintHead(); // listing_revision
  r.readBool(); // listed
  r.readText(); // title
  r.readText(); // summary
  readSortedSet(r, "byte", "topic_tags");
  readSortedSet(r, "text", "languages");
  // region / issued / expiry: only the sets carry order rules we assert here.
}

function readSortedSet(r: Reader, kind: "byte" | "text", name: string): void {
  const count = r.arrayHead();
  let previous: Uint8Array | null = null;
  for (let i = 0; i < count; i++) {
    const element = kind === "byte" ? new Writer().bstr(r.readBytes()).out() : new Writer().tstr(r.readText()).out();
    if (previous !== null) {
      const cmp = compareBytes(element, previous);
      if (cmp === 0) throw new Error(`duplicate ${name} member`);
      if (cmp < 0) throw new Error(`unsorted ${name} set`);
    }
    previous = element;
  }
}

// ===========================================================================
// The vector-document verifier.
// ===========================================================================

export interface VerifyReport {
  records: number;
  digests: number;
  signatures: number;
  hmacInputs: number;
  alternateGrammar: number;
  peerRoles: string[];
  sentinelsChecked: number;
}

function expectEqualBytes(actual: Uint8Array, expectedHex: string, context: string): void {
  if (!bytesEqual(actual, hexToBytes(expectedHex))) {
    throw new Error(`${context}: expected ${expectedHex}, got ${bytesToHex(actual)}`);
  }
}

/** Resolve the exact message bytes a record-attached signature covers, by
 * re-encoding the relevant sub-record from THIS vector (never trusting Rust). */
function signatureMessage(record: string, fields: Fields, message: string): Uint8Array {
  switch (message) {
    case "ticket_core_canonical":
      return encPublicSiteTicketCore(fields.core);
    case "grant_canonical":
      return encListingDelegateGrant(fields);
    case "body_canonical":
      return encBodyForEnvelope(record, fields.body);
    case "blake3(body_canonical)":
      return blake3(encBodyForEnvelope(record, fields.body));
    default:
      throw new Error(`unknown signature message source ${message}`);
  }
}

function encBodyForEnvelope(record: string, body: Fields): Uint8Array {
  switch (record) {
    case "DescriptorEnvelopeV1":
      return encAnchorDescriptorBody(body);
    case "HostingReceiptV1":
      return encHostingReceiptBody(body);
    case "ListingReceiptV1":
      return encListingReceiptBody(body);
    case "WorkChallengeV1":
      return encWorkChallengeBody(body);
    case "ReplicaSourceAttestationV1":
      return encReplicaSourceAttestationBody(body);
    default:
      throw new Error(`no body encoder for envelope ${record}`);
  }
}

function verifyRecordVector(v: Fields, report: VerifyReport): void {
  // 1. Independent byte reproduction from semantic fields.
  const canonical = encodeRecord(v.record, v.fields);
  expectEqualBytes(canonical, v.canonical_hex, `record ${v.id} (${v.record})`);
  report.records++;

  // 2. digest attachments.
  for (const d of v.digests ?? []) {
    verifyDigest(d, canonical, v, report);
  }

  // 3. signature attachments (+ one-bit tamper must fail).
  for (const sgn of v.signatures ?? []) {
    const pk = hexToBytes(sgn.public_key_hex);
    const message = signatureMessage(v.record, v.fields, sgn.message);
    const preimage = concatBytes(utf8(sgn.domain_ascii), message);
    expectEqualBytes(preimage, sgn.preimage_hex, `signature ${sgn.name} preimage of ${v.id}`);
    const sig = hexToBytes(sgn.signature_hex);
    if (!ed25519Verify(pk, preimage, sig)) throw new Error(`signature ${sgn.name} of ${v.id} failed to verify`);
    const tampered = new Uint8Array(preimage);
    tampered[0] ^= 0x01;
    if (ed25519Verify(pk, tampered, sig)) throw new Error(`tampered signature ${sgn.name} of ${v.id} verified`);
    report.signatures++;
  }
}

function verifyDigest(d: Fields, canonical: Uint8Array, v: Fields, report: VerifyReport): void {
  switch (d.algo) {
    case "digest_v1": {
      let message: Uint8Array;
      if (d.message === "canonical") {
        message = canonical;
      } else {
        // control_digest_body: re-derive it (describe carries no work stamp).
        message = deriveControlDigestBody(v.fields);
        expectEqualBytes(message, d.message_hex, `control digest body of ${v.id}`);
      }
      const preimage = digestV1Preimage(d.label_ascii, message);
      expectEqualBytes(preimage, d.preimage_hex, `digest ${d.name} preimage`);
      const value = blake3(preimage);
      expectEqualBytes(value, d.value_hex, `digest ${d.name} value`);
      // one-bit mutation of the message must change the digest.
      const flipped = new Uint8Array(message);
      flipped[0] ^= 0x01;
      if (bytesEqual(blake3(digestV1Preimage(d.label_ascii, flipped)), hexToBytes(d.value_hex))) {
        throw new Error(`digest ${d.name} did not change under a one-bit mutation`);
      }
      report.digests++;
      break;
    }
    case "operator_key_id": {
      const preimage = operatorKeyIdPreimage(canonical);
      expectEqualBytes(preimage, d.preimage_hex, `${d.name} preimage`);
      expectEqualBytes(blake3(preimage), d.value_hex, `${d.name} value`);
      report.digests++;
      break;
    }
    case "anchor_id": {
      const preimage = anchorIdPreimage(bx(d.inputs.genesis_operator_public_key), bx(d.inputs.genesis_random_256_bits));
      expectEqualBytes(preimage, d.preimage_hex, `${d.name} preimage`);
      expectEqualBytes(blake3(preimage), d.value_hex, `${d.name} value`);
      report.digests++;
      break;
    }
    case "work_proof": {
      const preimage = workProofPreimage(bx(d.inputs.work_challenge_digest), BigInt(d.inputs.counter));
      expectEqualBytes(preimage, d.preimage_hex, `${d.name} preimage`);
      expectEqualBytes(blake3(preimage), d.value_hex, `${d.name} value`);
      report.digests++;
      break;
    }
    case "snapshot_cursor_hmac_input": {
      const input = snapshotCursorHmacInput(canonical);
      expectEqualBytes(input, d.preimage_hex, `${d.name}`);
      report.hmacInputs++;
      break;
    }
    default:
      throw new Error(`unknown digest algo ${d.algo}`);
  }
}

function deriveControlDigestBody(fields: Fields): Uint8Array {
  if (fields.semantic.kind !== "describe") throw new Error("only describe control digest body supported");
  return new Writer().arr(3).uint(1).tstr("describe").raw(encDescribeSemanticBody()).out();
}

function verifyDigestVector(d: Fields, report: VerifyReport): void {
  switch (d.algo) {
    case "digest_v1_over_message": {
      const message = hexToBytes(d.inputs.message_hex);
      const preimage = digestV1Preimage(d.label_ascii, message);
      expectEqualBytes(preimage, d.preimage_hex, `${d.id} preimage`);
      expectEqualBytes(blake3(preimage), d.value_hex, `${d.id} value`);
      report.digests++;
      break;
    }
    case "sync_snapshot": {
      const sortedIds = (d.inputs.sorted_entry_ids as string[]).map(hexToBytes);
      const value = syncSnapshotDigest(
        bx(d.inputs.namespace_id),
        BigInt(d.inputs.entry_count),
        BigInt(d.inputs.logical_bytes),
        sortedIds,
      );
      expectEqualBytes(value, d.value_hex, `${d.id}`);
      report.digests++;
      break;
    }
    case "namespace_token_hmac_input": {
      const input = namespaceTokenHmacInput(
        bx(d.inputs.operation_id),
        bx(d.inputs.namespace_id),
        BigInt(d.inputs.operation_expiry_unix_seconds),
        Number(d.inputs.token_secret_epoch),
      );
      expectEqualBytes(input, d.preimage_hex, `${d.id}`);
      report.hmacInputs++;
      break;
    }
    case "peer_proof_preimage": {
      const role = d.inputs.role as string;
      const preimage = peerProofPreimage(role, bx(d.inputs.peer_transcript_digest));
      expectEqualBytes(preimage, d.preimage_hex, `${d.id} preimage`);
      if (d.signature) {
        const ok = ed25519Verify(bx(d.signature.public_key_hex), preimage, bx(d.signature.signature_hex));
        if (!ok) throw new Error(`${d.id} peer-proof signature failed to verify`);
      }
      report.peerRoles.push(role);
      break;
    }
    default:
      throw new Error(`unknown digest vector algo ${d.algo}`);
  }
}

// Sentinel token tables — TS's independent copy, cross-checked against the fixture.
const SENTINELS: Record<string, string[]> = {
  transport_floor: ["require_none", "require_arti"],
  enabled_role: ["directory", "gossip", "host", "mirror"],
  control_operation_kind: [
    "describe",
    "get_work_challenge",
    "prepare_host",
    "commit_host",
    "submit_listing",
    "prepare_replica",
    "pull_directory_feed",
    "pull_directory_snapshot",
    "get_operation",
  ],
  hosting_status: ["committed"],
  control_outcome: ["success", "refused"],
};

function verifySentinels(doc: Fields, report: VerifyReport): void {
  for (const [name, expected] of Object.entries(SENTINELS)) {
    const actual = doc.sentinels?.[name];
    if (!Array.isArray(actual) || actual.length !== expected.length || actual.some((t: string, i: number) => t !== expected[i])) {
      throw new Error(`sentinel table ${name} disagrees with the fixture`);
    }
    report.sentinelsChecked++;
  }
}

function verifyAlternateGrammar(doc: Fields, report: VerifyReport): void {
  for (const g of doc.alternate_grammar ?? []) {
    const hostile = hexToBytes(g.hostile_hex);
    let rejected = false;
    try {
      assertCanonical(hostile);
    } catch {
      rejected = true;
    }
    if (!rejected && g.record === "CommunityListingV1") {
      try {
        decodeCommunityListingCanonical(hostile);
      } catch {
        rejected = true;
      }
    }
    if (!rejected) throw new Error(`alternate grammar '${g.desc}' was not rejected`);
    report.alternateGrammar++;
  }
}

/** Verify an entire parsed vector document. Throws on the first disagreement. */
export function verifyVectorDocument(doc: Fields): VerifyReport {
  if (doc.protocol !== "riot-anchor-protocol") throw new Error("unexpected protocol tag");
  const report: VerifyReport = {
    records: 0,
    digests: 0,
    signatures: 0,
    hmacInputs: 0,
    alternateGrammar: 0,
    peerRoles: [],
    sentinelsChecked: 0,
  };
  for (const v of doc.vectors) verifyRecordVector(v, report);
  for (const d of doc.digest_vectors) verifyDigestVector(d, report);
  verifySentinels(doc, report);
  verifyAlternateGrammar(doc, report);

  // Peer roles must cover both the initiator and responder domains.
  if (!report.peerRoles.includes("initiator") || !report.peerRoles.includes("responder")) {
    throw new Error(`peer roles incomplete: ${report.peerRoles.join(",")}`);
  }
  return report;
}
