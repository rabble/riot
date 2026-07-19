// WU-006B — independent Swift reproduction of the anchor protocol wire format.
//
// This is the THIRD, INDEPENDENT implementation (after Rust and TypeScript) of the
// canonical anchor records pinned by `fixtures/anchor/protocol-v1-vectors.json`.
// It never asks Rust for expected bytes: it re-encodes every record from the
// vector's semantic fields with its own canonical positional-CBOR encoder, re-frames
// and re-hashes every digest/preimage with a vendored pure-Swift BLAKE3 (anchored by
// the fixture's BLAKE3 known-answer tests), re-derives every HMAC input, and verifies
// every ed25519 signature with CryptoKit's Curve25519. Byte-for-byte agreement with
// the Rust-emitted fixture is the cross-language conformance proof.
//
// Nothing here calls `riot-ffi` — the proof is deliberately Rust-free.

import CryptoKit
import Foundation
import XCTest

// ===========================================================================
// Vendored BLAKE3 (pure Swift, 32-byte output, default unkeyed mode).
// Self-tested against the official BLAKE3 vectors carried by the fixture.
// ===========================================================================

private enum Blake3 {
    static let IV: [UInt32] = [
        0x6a09_e667, 0xbb67_ae85, 0x3c6e_f372, 0xa54f_f53a,
        0x510e_527f, 0x9b05_688c, 0x1f83_d9ab, 0x5be0_cd19,
    ]
    static let MSG_PERMUTATION = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8]
    static let CHUNK_START: UInt32 = 1 << 0
    static let CHUNK_END: UInt32 = 1 << 1
    static let PARENT: UInt32 = 1 << 2
    static let ROOT: UInt32 = 1 << 3
    static let BLOCK_LEN = 64
    static let CHUNK_LEN = 1024

    @inline(__always) static func rotr32(_ x: UInt32, _ n: UInt32) -> UInt32 {
        (x >> n) | (x << (32 - n))
    }

    @inline(__always) static func g(_ v: inout [UInt32], _ a: Int, _ b: Int, _ c: Int, _ d: Int, _ mx: UInt32, _ my: UInt32) {
        v[a] = v[a] &+ v[b] &+ mx
        v[d] = rotr32(v[d] ^ v[a], 16)
        v[c] = v[c] &+ v[d]
        v[b] = rotr32(v[b] ^ v[c], 12)
        v[a] = v[a] &+ v[b] &+ my
        v[d] = rotr32(v[d] ^ v[a], 8)
        v[c] = v[c] &+ v[d]
        v[b] = rotr32(v[b] ^ v[c], 7)
    }

    static func compress(_ cv: [UInt32], _ blockWords: [UInt32], _ counter: UInt64, _ blockLen: UInt32, _ flags: UInt32) -> [UInt32] {
        var v = [UInt32](repeating: 0, count: 16)
        for i in 0..<8 { v[i] = cv[i] }
        for i in 0..<4 { v[8 + i] = IV[i] }
        v[12] = UInt32(truncatingIfNeeded: counter)
        v[13] = UInt32(truncatingIfNeeded: counter >> 32)
        v[14] = blockLen
        v[15] = flags

        var m = blockWords
        for round in 0..<7 {
            g(&v, 0, 4, 8, 12, m[0], m[1])
            g(&v, 1, 5, 9, 13, m[2], m[3])
            g(&v, 2, 6, 10, 14, m[4], m[5])
            g(&v, 3, 7, 11, 15, m[6], m[7])
            g(&v, 0, 5, 10, 15, m[8], m[9])
            g(&v, 1, 6, 11, 12, m[10], m[11])
            g(&v, 2, 7, 8, 13, m[12], m[13])
            g(&v, 3, 4, 9, 14, m[14], m[15])
            if round < 6 {
                var permuted = [UInt32](repeating: 0, count: 16)
                for i in 0..<16 { permuted[i] = m[MSG_PERMUTATION[i]] }
                m = permuted
            }
        }
        var out = [UInt32](repeating: 0, count: 16)
        for i in 0..<8 {
            out[i] = v[i] ^ v[i + 8]
            out[i + 8] = v[i + 8] ^ cv[i]
        }
        return out
    }

    static func wordsFromBlock(_ block: ArraySlice<UInt8>) -> [UInt32] {
        var padded = [UInt8](repeating: 0, count: BLOCK_LEN)
        var idx = 0
        for b in block { if idx < BLOCK_LEN { padded[idx] = b; idx += 1 } }
        var words = [UInt32](repeating: 0, count: 16)
        for i in 0..<16 {
            let o = i * 4
            words[i] = UInt32(padded[o]) | (UInt32(padded[o + 1]) << 8) | (UInt32(padded[o + 2]) << 16) | (UInt32(padded[o + 3]) << 24)
        }
        return words
    }

    struct Output {
        var inputCv: [UInt32]
        var blockWords: [UInt32]
        var counter: UInt64
        var blockLen: UInt32
        var flags: UInt32
    }

    static func chainingValue(_ o: Output) -> [UInt32] {
        Array(compress(o.inputCv, o.blockWords, o.counter, o.blockLen, o.flags).prefix(8))
    }

    static func rootBytes(_ o: Output, _ length: Int) -> [UInt8] {
        var out = [UInt8](repeating: 0, count: length)
        var counter: UInt64 = 0
        var filled = 0
        while filled < length {
            let words = compress(o.inputCv, o.blockWords, counter, o.blockLen, o.flags | ROOT)
            var i = 0
            while i < 16 && filled < length {
                var w = words[i]
                var b = 0
                while b < 4 && filled < length {
                    out[filled] = UInt8(w & 0xff)
                    w >>= 8
                    filled += 1
                    b += 1
                }
                i += 1
            }
            counter += 1
        }
        return out
    }

    static func chunkToOutput(_ bytes: ArraySlice<UInt8>, _ chunkCounter: UInt64) -> Output {
        var cv = IV
        let len = bytes.count
        let blockCount = len == 0 ? 1 : (len + BLOCK_LEN - 1) / BLOCK_LEN
        let base = bytes.startIndex
        for i in 0..<blockCount {
            let start = base + i * BLOCK_LEN
            let end = Swift.min(start + BLOCK_LEN, bytes.endIndex)
            let block = bytes[start..<end]
            let blockLen = UInt32(block.count)
            let words = wordsFromBlock(block)
            var flags: UInt32 = 0
            if i == 0 { flags |= CHUNK_START }
            let isLast = i == blockCount - 1
            if isLast {
                return Output(inputCv: cv, blockWords: words, counter: chunkCounter, blockLen: blockLen, flags: flags | CHUNK_END)
            }
            cv = Array(compress(cv, words, chunkCounter, UInt32(BLOCK_LEN), flags).prefix(8))
        }
        fatalError("blake3: empty chunk loop")
    }

    static func parentOutput(_ left: [UInt32], _ right: [UInt32]) -> Output {
        var block = [UInt32](repeating: 0, count: 16)
        for i in 0..<8 { block[i] = left[i]; block[8 + i] = right[i] }
        return Output(inputCv: IV, blockWords: block, counter: 0, blockLen: UInt32(BLOCK_LEN), flags: PARENT)
    }

    static func leftLen(_ contentLen: Int) -> Int {
        let fullChunks = (contentLen - 1) / CHUNK_LEN
        var p = 1
        while p * 2 <= fullChunks { p *= 2 }
        return p * CHUNK_LEN
    }

    static func hashRange(_ bytes: ArraySlice<UInt8>, _ chunkCounter: UInt64) -> Output {
        if bytes.count <= CHUNK_LEN {
            return chunkToOutput(bytes, chunkCounter)
        }
        let ll = leftLen(bytes.count)
        let mid = bytes.startIndex + ll
        let left = hashRange(bytes[bytes.startIndex..<mid], chunkCounter)
        let right = hashRange(bytes[mid..<bytes.endIndex], chunkCounter + UInt64(ll / CHUNK_LEN))
        return parentOutput(chainingValue(left), chainingValue(right))
    }

    static func hash(_ input: [UInt8]) -> [UInt8] {
        rootBytes(hashRange(input[input.startIndex..<input.endIndex], 0), 32)
    }
}

// ===========================================================================
// Hex / byte helpers.
// ===========================================================================

private func hexToBytes(_ hex: String) -> [UInt8] {
    precondition(hex.count % 2 == 0, "odd-length hex")
    var out = [UInt8]()
    out.reserveCapacity(hex.count / 2)
    var idx = hex.startIndex
    while idx < hex.endIndex {
        let next = hex.index(idx, offsetBy: 2)
        out.append(UInt8(hex[idx..<next], radix: 16)!)
        idx = next
    }
    return out
}

private func bytesToHex(_ bytes: [UInt8]) -> String {
    var s = ""
    s.reserveCapacity(bytes.count * 2)
    for b in bytes { s += String(format: "%02x", b) }
    return s
}

private func utf8(_ s: String) -> [UInt8] { Array(s.utf8) }

private func concat(_ parts: [UInt8]...) -> [UInt8] {
    var out = [UInt8]()
    for p in parts { out.append(contentsOf: p) }
    return out
}

private func u16be(_ n: Int) -> [UInt8] { [UInt8((n >> 8) & 0xff), UInt8(n & 0xff)] }
private func u32be(_ n: Int) -> [UInt8] {
    [UInt8((n >> 24) & 0xff), UInt8((n >> 16) & 0xff), UInt8((n >> 8) & 0xff), UInt8(n & 0xff)]
}
private func u64be(_ n: UInt64) -> [UInt8] {
    var out = [UInt8](repeating: 0, count: 8)
    var v = n
    var i = 7
    while i >= 0 { out[i] = UInt8(v & 0xff); v >>= 8; i -= 1 }
    return out
}

private func compareBytes(_ a: [UInt8], _ b: [UInt8]) -> Int {
    let n = Swift.min(a.count, b.count)
    for i in 0..<n where a[i] != b[i] { return Int(a[i]) - Int(b[i]) }
    return a.count - b.count
}

// JSON accessors: the vector document arrives as nested Any via JSONSerialization.
private typealias Fields = [String: Any]

private func D(_ v: Any?) -> Fields { v as? Fields ?? [:] }
private func A(_ v: Any?) -> [Any] { v as? [Any] ?? [] }
private func S(_ v: Any?) -> String {
    if let s = v as? String { return s }
    if let n = v as? NSNumber { return n.stringValue }
    fatalError("expected string, got \(String(describing: v))")
}
private func U(_ v: Any?) -> UInt64 {
    if let s = v as? String { return UInt64(s)! }
    if let n = v as? NSNumber { return n.uint64Value }
    fatalError("expected uint, got \(String(describing: v))")
}
private func BOOL(_ v: Any?) -> Bool {
    if let b = v as? Bool { return b }
    if let n = v as? NSNumber { return n.boolValue }
    fatalError("expected bool")
}
private func isNull(_ v: Any?) -> Bool { v == nil || v is NSNull }
private func BX(_ v: Any?) -> [UInt8] { hexToBytes(S(v)) }

// ===========================================================================
// Canonical positional-CBOR encoder (minimal ints, definite lengths, sorted
// sets — never maps, never indefinite lengths).
// ===========================================================================

private final class Writer {
    private(set) var bytes = [UInt8]()

    private func head(_ major: UInt8, _ n: UInt64) {
        let m = major << 5
        if n < 24 { bytes.append(m | UInt8(n)) }
        else if n < 0x100 { bytes.append(m | 24); bytes.append(UInt8(n)) }
        else if n < 0x1_0000 { bytes.append(m | 25); pushBE(n, 2) }
        else if n < 0x1_0000_0000 { bytes.append(m | 26); pushBE(n, 4) }
        else { bytes.append(m | 27); pushBE(n, 8) }
    }

    private func pushBE(_ n: UInt64, _ len: Int) {
        var i = len - 1
        while i >= 0 { bytes.append(UInt8((n >> (8 * UInt64(i))) & 0xff)); i -= 1 }
    }

    @discardableResult func uint(_ n: UInt64) -> Writer { head(0, n); return self }
    @discardableResult func bstr(_ u8: [UInt8]) -> Writer { head(2, UInt64(u8.count)); bytes.append(contentsOf: u8); return self }
    @discardableResult func tstr(_ s: String) -> Writer { let u8 = utf8(s); head(3, UInt64(u8.count)); bytes.append(contentsOf: u8); return self }
    @discardableResult func bool(_ b: Bool) -> Writer { bytes.append(b ? 0xf5 : 0xf4); return self }
    @discardableResult func nul() -> Writer { bytes.append(0xf6); return self }
    @discardableResult func arr(_ n: Int) -> Writer { head(4, UInt64(n)); return self }
    @discardableResult func raw(_ u8: [UInt8]) -> Writer { bytes.append(contentsOf: u8); return self }
}

private func encodeSet(_ w: Writer, _ elements: [[UInt8]]) {
    let sorted = elements.sorted { compareBytes($0, $1) < 0 }
    for i in 1..<max(1, sorted.count) where i < sorted.count {
        if compareBytes(sorted[i - 1], sorted[i]) == 0 { fatalError("duplicate set member") }
    }
    w.arr(sorted.count)
    for e in sorted { w.raw(e) }
}

private func encByteSet(_ hexes: [Any]) -> [[UInt8]] { hexes.map { Writer().bstr(hexToBytes(S($0))).bytes } }
private func encTextSet(_ items: [Any]) -> [[UInt8]] { items.map { Writer().tstr(S($0)).bytes } }
private func encUintSet(_ items: [Any]) -> [[UInt8]] { items.map { Writer().uint(U($0)).bytes } }

// ===========================================================================
// Per-record encoders — each mirrors the crate's `encode_canonical` positionally.
// ===========================================================================

private let COMMUNITY_LISTING_SCHEMA = "riot/community-listing/1"

private func encOperatorKey(_ f: Fields) -> [UInt8] {
    Writer().arr(3).uint(1).tstr("ed25519").bstr(BX(f["public_key"])).bytes
}

private func encPublicSiteTicketCore(_ f: Fields) -> [UInt8] {
    Writer().arr(12)
        .bstr(BX(f["root_id"]))
        .bstr(BX(f["o_namespace_id"]))
        .bstr(BX(f["c_namespace_id"]))
        .bstr(BX(f["w_namespace_id"]))
        .bstr(BX(f["manifest_digest"]))
        .uint(U(f["manifest_version"]))
        .uint(U(f["min_sync_version"]))
        .tstr(S(f["manifest_required_transport"]))
        .tstr(S(f["transport_floor"]))
        .uint(U(f["transport_epoch"]))
        .uint(U(f["issued_unix_seconds"]))
        .uint(U(f["expiry_unix_seconds"]))
        .bytes
}

private func encRootSignedTicketCoreEnvelope(_ f: Fields) -> [UInt8] {
    Writer().arr(3).uint(2).raw(encPublicSiteTicketCore(D(f["core"]))).bstr(BX(f["root_signature"])).bytes
}

private func encListingDelegateGrant(_ f: Fields) -> [UInt8] {
    Writer().arr(6)
        .bstr(BX(f["root_id"]))
        .bstr(BX(f["delegate_key"]))
        .bstr(BX(f["terminal_capability_digest"]))
        .uint(U(f["listing_epoch"]))
        .uint(U(f["issued_unix_seconds"]))
        .uint(U(f["expiry_unix_seconds"]))
        .bytes
}

private func encCommunityListing(_ f: Fields) -> [UInt8] {
    precondition(S(f["schema"]) == COMMUNITY_LISTING_SCHEMA, "unexpected schema")
    let w = Writer().arr(18)
        .tstr(COMMUNITY_LISTING_SCHEMA)
        .bstr(BX(f["root_id"]))
        .bstr(BX(f["o_namespace_id"]))
        .bstr(BX(f["c_namespace_id"]))
        .bstr(BX(f["w_namespace_id"]))
        .bstr(BX(f["manifest_digest"]))
        .uint(U(f["manifest_version"]))
        .bstr(BX(f["ticket_core_bytes"]))
        .uint(U(f["listing_epoch"]))
        .uint(U(f["listing_revision"]))
        .bool(BOOL(f["listed"]))
        .tstr(S(f["title"]))
        .tstr(S(f["summary"]))
    encodeSet(w, encByteSet(A(f["topic_tags"])))
    encodeSet(w, encTextSet(A(f["languages"])))
    if isNull(f["region"]) { w.nul() } else { w.bstr(BX(f["region"])) }
    w.uint(U(f["issued_unix_seconds"])).uint(U(f["expiry_unix_seconds"]))
    return w.bytes
}

private func encAdmittedListingEnvelope(_ f: Fields) -> [UInt8] {
    let w = Writer().arr(4).uint(1)
        .bstr(BX(f["signed_listing_entry_bytes"]))
        .bstr(BX(f["capability_chain_bytes"]))
    if isNull(f["delegate_grant_bytes"]) { w.nul() } else { w.bstr(BX(f["delegate_grant_bytes"])) }
    return w.bytes
}

private func encLimitValue(_ w: Writer, _ v: Any?) {
    if let arr = v as? [Any] { w.arr(2).uint(U(arr[0])).uint(U(arr[1])) } else { w.uint(U(v)) }
}

private func encAnchorLimitProfile(_ f: Fields) -> [UInt8] {
    let w = Writer().arr(3).uint(1).uint(U(f["profile_epoch"]))
    let entries = A(f["entries"])
    w.arr(entries.count)
    for e in entries {
        let ed = D(e)
        w.arr(3).uint(U(ed["id"]))
        encLimitValue(w, ed["effective"])
        encLimitValue(w, ed["absolute"])
    }
    return w.bytes
}

private func encDescriptorFloor(_ f: Fields) -> [UInt8] {
    Writer().arr(4)
        .bstr(BX(f["anchor_id"]))
        .uint(U(f["descriptor_epoch"]))
        .bstr(BX(f["descriptor_digest"]))
        .raw(encOperatorKey(D(f["operator_verification_key"])))
        .bytes
}

private func encRoleSet(_ w: Writer, _ roles: [Any]) { encodeSet(w, encTextSet(roles)) }

private func encAnchorDescriptorBody(_ f: Fields) -> [UInt8] {
    let w = Writer().arr(19).uint(1)
        .bstr(BX(f["anchor_id"]))
        .bstr(BX(f["genesis_operator_public_key"]))
        .bstr(BX(f["genesis_random_256_bits"]))
        .raw(encOperatorKey(D(f["current_operator_verification_key"])))
        .bstr(BX(f["current_operator_key_id"]))
        .uint(U(f["descriptor_epoch"]))
    if isNull(f["previous_descriptor_digest"]) { w.nul() } else { w.bstr(BX(f["previous_descriptor_digest"])) }
    w.bstr(BX(f["current_iroh_endpoint_id"]))
        .tstr(S(f["https_origin"]))
        .tstr(S(f["operator_display_label"]))
        .tstr(S(f["self_reported_failure_domain_label"]))
    encodeSet(w, encUintSet(A(f["supported_control_versions"])))
    encodeSet(w, encUintSet(A(f["supported_sync_versions"])))
    encRoleSet(w, A(f["enabled_roles"]))
    w.bstr(BX(f["limit_profile_digest"]))
    if isNull(f["predecessor_operator_verification_key"]) { w.nul() } else { w.raw(encOperatorKey(D(f["predecessor_operator_verification_key"]))) }
    w.uint(U(f["issued_at"])).uint(U(f["expires_at"]))
    return w.bytes
}

private func encDescriptorEnvelope(_ f: Fields) -> [UInt8] {
    let w = Writer().arr(4).uint(1).raw(encAnchorDescriptorBody(D(f["body"]))).bstr(BX(f["current_signature"]))
    if isNull(f["predecessor_signature"]) { w.nul() } else { w.bstr(BX(f["predecessor_signature"])) }
    return w.bytes
}

private func encNamespaceResult(_ w: Writer, _ r: Fields) {
    w.arr(3).bstr(BX(r["namespace_id"])).bstr(BX(r["snapshot_digest"])).uint(U(r["entry_count"]))
}

private func encHostingReceiptBody(_ f: Fields) -> [UInt8] {
    let w = Writer().arr(16).uint(1)
        .bstr(BX(f["anchor_id"]))
        .bstr(BX(f["operator_key_id"]))
        .uint(U(f["descriptor_epoch"]))
        .bstr(BX(f["descriptor_digest"]))
        .bstr(BX(f["hosting_operation_id"]))
        .bstr(BX(f["full_site_root"]))
        .bstr(BX(f["manifest_digest"]))
        .uint(U(f["manifest_version"]))
        .uint(U(f["base_site_generation"]))
        .uint(U(f["committed_site_generation"]))
    let results = A(f["ordered_namespace_results"])
    w.arr(results.count)
    for r in results { encNamespaceResult(w, D(r)) }
    w.tstr(S(f["status"])).uint(U(f["accepted_at"])).uint(U(f["reported_retention_through"])).bstr(BX(f["limit_profile_digest"]))
    return w.bytes
}

private func encHostingReceiptEnvelope(_ f: Fields) -> [UInt8] {
    Writer().arr(3).uint(1).raw(encHostingReceiptBody(D(f["body"]))).bstr(BX(f["operator_signature"])).bytes
}

private func encListingReceiptBody(_ f: Fields) -> [UInt8] {
    Writer().arr(13).uint(1)
        .bstr(BX(f["anchor_id"]))
        .bstr(BX(f["operator_key_id"]))
        .uint(U(f["descriptor_epoch"]))
        .bstr(BX(f["descriptor_digest"]))
        .bstr(BX(f["listing_digest"]))
        .bstr(BX(f["full_site_root"]))
        .uint(U(f["accepted_listing_epoch"]))
        .uint(U(f["accepted_listing_revision"]))
        .uint(U(f["feed_coordinate"]))
        .uint(U(f["accepted_at"]))
        .uint(U(f["expires_at"]))
        .bstr(BX(f["request_idempotency_key"]))
        .bytes
}

private func encListingReceiptEnvelope(_ f: Fields) -> [UInt8] {
    Writer().arr(3).uint(1).raw(encListingReceiptBody(D(f["body"]))).bstr(BX(f["operator_signature"])).bytes
}

private func encWorkChallengeBody(_ f: Fields) -> [UInt8] {
    Writer().arr(14).uint(1)
        .bstr(BX(f["anchor_id"]))
        .bstr(BX(f["operator_key_id"]))
        .uint(U(f["descriptor_epoch"]))
        .bstr(BX(f["descriptor_digest"]))
        .tstr(S(f["operation_kind"]))
        .bstr(BX(f["idempotency_key"]))
        .bstr(BX(f["work_target_digest"]))
        .bstr(BX(f["community_root"]))
        .bstr(BX(f["random_challenge"]))
        .uint(U(f["policy_epoch"]))
        .uint(U(f["difficulty"]))
        .uint(U(f["issued_at"]))
        .uint(U(f["expires_at"]))
        .bytes
}

private func encWorkChallengeEnvelope(_ f: Fields) -> [UInt8] {
    Writer().arr(3).uint(1).raw(encWorkChallengeBody(D(f["body"]))).bstr(BX(f["operator_signature"])).bytes
}

private func encWorkStamp(_ f: Fields) -> [UInt8] {
    Writer().arr(4).uint(1).bstr(BX(f["challenge_envelope_bytes"])).uint(U(f["counter"])).bstr(BX(f["proof_bytes"])).bytes
}

private func encReplicaPrepareChallenge(_ f: Fields) -> [UInt8] {
    Writer().arr(7).uint(1)
        .bstr(BX(f["destination_anchor_id"]))
        .bstr(BX(f["random_256_bit_nonce"]))
        .bstr(BX(f["prepare_idempotency_key"]))
        .bstr(BX(f["full_site_root"]))
        .uint(U(f["issued_at"]))
        .uint(U(f["expires_at"]))
        .bytes
}

private func encReplicaSourceAttestationBody(_ f: Fields) -> [UInt8] {
    let w = Writer().arr(16).uint(1)
        .bstr(BX(f["source_anchor_id"]))
        .bstr(BX(f["source_current_operator_key_id"]))
        .uint(U(f["source_current_descriptor_epoch"]))
        .bstr(BX(f["source_current_descriptor_digest"]))
        .bstr(BX(f["destination_anchor_id"]))
        .bstr(BX(f["peer_transcript_digest"]))
        .bstr(BX(f["destination_prepare_nonce"]))
        .bstr(BX(f["prepare_idempotency_key"]))
        .bstr(BX(f["full_site_root"]))
        .arr(2)
        .bstr(BX(f["manifest_digest"]))
        .uint(U(f["manifest_version"]))
        .bstr(BX(f["root_signed_ticket_core_digest"]))
        .uint(U(f["source_site_generation"]))
    let digests = A(f["ordered_namespace_snapshot_digests"])
    w.arr(digests.count)
    for d in digests { w.bstr(hexToBytes(S(d))) }
    w.uint(U(f["issued_at"])).uint(U(f["expires_at"]))
    return w.bytes
}

private func encReplicaSourceAttestationEnvelope(_ f: Fields) -> [UInt8] {
    Writer().arr(3).uint(1).raw(encReplicaSourceAttestationBody(D(f["body"]))).bstr(BX(f["operator_signature"])).bytes
}

private func encSnapshotCursorBody(_ f: Fields) -> [UInt8] {
    let w = Writer().arr(8).uint(1)
        .bstr(BX(f["checkpoint_digest"]))
        .uint(U(f["snapshot_generation_id"]))
        .uint(U(f["next_ordinal"]))
    if isNull(f["previous_root"]) { w.nul() } else { w.bstr(BX(f["previous_root"])) }
    w.uint(U(f["issued_at"])).uint(U(f["expires_at"])).uint(U(f["cursor_secret_epoch"]))
    return w.bytes
}

private func encSnapshotCursor(_ f: Fields) -> [UInt8] {
    Writer().arr(3).uint(1).raw(encSnapshotCursorBody(D(f["body"]))).bstr(BX(f["cursor_tag"])).bytes
}

private func encBootstrapDescriptor(_ d: Fields) -> [UInt8] {
    let w = Writer().arr(4).uint(1).raw(encDescriptorFloor(D(d["floor"]))).tstr(S(d["https_origin"]))
    encRoleSet(w, A(d["roles"]))
    return w.bytes
}

private func encAnchorBootstrap(_ f: Fields) -> [UInt8] {
    let descriptors = A(f["descriptors"])
    let w = Writer().arr(2).uint(1).arr(descriptors.count)
    for d in descriptors { w.raw(encBootstrapDescriptor(D(d))) }
    return w.bytes
}

private func encDescribeSemanticBody() -> [UInt8] { Writer().arr(1).uint(1).bytes }

private func encControlRequest(_ f: Fields) -> [UInt8] {
    precondition(S(D(f["semantic"])["kind"]) == "describe", "control request encoder only supports describe")
    return Writer().arr(4).uint(1)
        .tstr(S(f["operation_kind"]))
        .bstr(BX(f["idempotency_key"]))
        .raw(encDescribeSemanticBody())
        .bytes
}

private func encNotHostedRefusal() -> [UInt8] {
    Writer().arr(5).tstr("not_hosted").tstr("listing").bool(true).nul().arr(1).tstr("none").bytes
}

private func encControlResponse(_ f: Fields) -> [UInt8] {
    let w = Writer().arr(3).uint(1).tstr(S(f["kind"]))
    let outcome = D(f["outcome"])
    switch S(outcome["type"]) {
    case "refused":
        precondition(S(D(outcome["refusal"])["code"]) == "not_hosted", "only not_hosted refusal supported")
        w.arr(2).tstr("refused").raw(encNotHostedRefusal())
    case "success":
        let success = D(outcome["success"])
        precondition(S(success["kind"]) == "submit_listing", "only submit_listing success supported")
        let payload = Writer().arr(2).uint(1).raw(encListingReceiptEnvelope(D(success["listing_receipt"]))).bytes
        w.arr(2).tstr("success").raw(payload)
    default:
        fatalError("unknown outcome")
    }
    return w.bytes
}

private let RECORD_ENCODERS: [String: (Fields) -> [UInt8]] = [
    "OperatorVerificationKeyV1": encOperatorKey,
    "PublicSiteTicketV2Core": encPublicSiteTicketCore,
    "RootSignedTicketCoreEnvelopeV2": encRootSignedTicketCoreEnvelope,
    "ListingDelegateGrantV1": encListingDelegateGrant,
    "CommunityListingV1": encCommunityListing,
    "AdmittedListingEnvelopeV1": encAdmittedListingEnvelope,
    "AnchorLimitProfileV1": encAnchorLimitProfile,
    "DescriptorFloor": encDescriptorFloor,
    "AnchorDescriptorBodyV1": encAnchorDescriptorBody,
    "DescriptorEnvelopeV1": encDescriptorEnvelope,
    "HostingReceiptBodyV1": encHostingReceiptBody,
    "HostingReceiptV1": encHostingReceiptEnvelope,
    "ListingReceiptBodyV1": encListingReceiptBody,
    "ListingReceiptV1": encListingReceiptEnvelope,
    "WorkChallengeBodyV1": encWorkChallengeBody,
    "WorkChallengeV1": encWorkChallengeEnvelope,
    "WorkStampV1": encWorkStamp,
    "ReplicaPrepareChallengeV1": encReplicaPrepareChallenge,
    "ReplicaSourceAttestationBodyV1": encReplicaSourceAttestationBody,
    "ReplicaSourceAttestationV1": encReplicaSourceAttestationEnvelope,
    "SnapshotCursorBodyV1": encSnapshotCursorBody,
    "SnapshotCursorV1": encSnapshotCursor,
    "AnchorBootstrapV1": encAnchorBootstrap,
    "ControlRequestV1": encControlRequest,
    "ControlResponseV1": encControlResponse,
]

private func encodeRecord(_ record: String, _ fields: Fields) -> [UInt8] {
    guard let enc = RECORD_ENCODERS[record] else { fatalError("no Swift encoder for \(record)") }
    return enc(fields)
}

// ===========================================================================
// Domain-separation labels (Swift's own independent copy).
// ===========================================================================

private enum Labels {
    static let operatorKeyId = "riot/anchor-operator-key-id/v1"
    static let anchorId = "riot/anchor-id/v1"
    static let workProof = "riot/anchor-work-proof/v1"
    static let syncSnapshot = "riot/sync-snapshot/v2"
    static let namespaceToken = "riot/namespace-token/v1"
    static let snapshotCursor = "riot/directory-snapshot-cursor/v1"
    static let peerProof = "riot/anchor-peer-proof/v1"
}

private func digestV1Preimage(_ labelAscii: String, _ message: [UInt8]) -> [UInt8] {
    let label = utf8(labelAscii)
    return concat(u16be(label.count), label, u64be(UInt64(message.count)), message)
}

private func operatorKeyIdPreimage(_ canonical: [UInt8]) -> [UInt8] { concat(utf8(Labels.operatorKeyId), canonical) }
private func anchorIdPreimage(_ pk: [UInt8], _ rand: [UInt8]) -> [UInt8] { concat(utf8(Labels.anchorId), pk, rand) }
private func workProofPreimage(_ challengeDigest: [UInt8], _ counter: UInt64) -> [UInt8] { concat(utf8(Labels.workProof), challengeDigest, u64be(counter)) }

private func syncSnapshotDigest(_ nsId: [UInt8], _ entryCount: UInt64, _ logicalBytes: UInt64, _ sortedIds: [[UInt8]]) -> [UInt8] {
    var parts = concat(utf8(Labels.syncSnapshot), u32be(nsId.count), nsId, u64be(entryCount), u64be(logicalBytes))
    for id in sortedIds { parts.append(contentsOf: u32be(id.count)); parts.append(contentsOf: id) }
    return Blake3.hash(parts)
}

private func namespaceTokenHmacInput(_ opId: [UInt8], _ nsId: [UInt8], _ expiry: UInt64, _ epoch: Int) -> [UInt8] {
    let label = utf8(Labels.namespaceToken)
    return concat(u16be(23), label, u16be(opId.count), opId, u16be(nsId.count), nsId, u64be(expiry), u32be(epoch))
}

private func snapshotCursorHmacInput(_ canonical: [UInt8]) -> [UInt8] {
    let label = utf8(Labels.snapshotCursor)
    return concat(u16be(33), label, u64be(UInt64(canonical.count)), canonical)
}

private func peerProofPreimage(_ role: String, _ transcriptDigest: [UInt8]) -> [UInt8] {
    let label = utf8(Labels.peerProof)
    let roleBytes = utf8(role)
    return concat(u16be(25), label, u16be(roleBytes.count), roleBytes, transcriptDigest)
}

// ===========================================================================
// ed25519 verification via CryptoKit (raw 32-byte key).
// ===========================================================================

private func ed25519Verify(_ publicKey: [UInt8], _ message: [UInt8], _ signature: [UInt8]) -> Bool {
    guard publicKey.count == 32, signature.count == 64 else { return false }
    guard let key = try? Curve25519.Signing.PublicKey(rawRepresentation: Data(publicKey)) else { return false }
    return key.isValidSignature(Data(signature), for: Data(message))
}

// ===========================================================================
// Canonical-CBOR validator + a CommunityListing set-order decoder, used to
// prove alternate-grammar encodings are rejected independently of Rust.
// ===========================================================================

private struct DecodeError: Error { let message: String }

private final class Reader {
    private let bytes: [UInt8]
    var pos: Int
    init(_ bytes: [UInt8], _ pos: Int = 0) { self.bytes = bytes; self.pos = pos }

    private func byte() throws -> UInt8 {
        guard pos < bytes.count else { throw DecodeError(message: "unexpected end of input") }
        let b = bytes[pos]; pos += 1; return b
    }

    func remaining() -> Int { bytes.count - pos }

    private func head() throws -> (major: Int, value: UInt64) {
        let ib = try byte()
        let major = Int(ib >> 5)
        let ai = Int(ib & 0x1f)
        if ai < 24 { return (major, UInt64(ai)) }
        if ai == 24 {
            let v = try byte()
            if v < 24 { throw DecodeError(message: "non-minimal integer") }
            return (major, UInt64(v))
        }
        if ai == 25 {
            let v = (UInt64(try byte()) << 8) | UInt64(try byte())
            if v < 0x100 { throw DecodeError(message: "non-minimal integer") }
            return (major, v)
        }
        if ai == 26 {
            var v: UInt64 = 0
            for _ in 0..<4 { v = (v << 8) | UInt64(try byte()) }
            if v < 0x1_0000 { throw DecodeError(message: "non-minimal integer") }
            return (major, v)
        }
        if ai == 27 {
            var v: UInt64 = 0
            for _ in 0..<8 { v = (v << 8) | UInt64(try byte()) }
            if v < 0x1_0000_0000 { throw DecodeError(message: "non-minimal integer") }
            return (major, v)
        }
        throw DecodeError(message: "indefinite or reserved length")
    }

    func validateItem() throws {
        guard pos < bytes.count else { throw DecodeError(message: "unexpected end of input") }
        let startByte = bytes[pos]
        let major = Int(startByte >> 5)
        if major == 5 { throw DecodeError(message: "maps are never canonical in this protocol") }
        if major == 6 { throw DecodeError(message: "tags are never canonical in this protocol") }
        if major == 7 {
            let ai = Int(startByte & 0x1f)
            if ai != 20 && ai != 21 && ai != 22 { throw DecodeError(message: "unsupported simple/float value") }
            pos += 1
            return
        }
        let (m, value) = try head()
        if m == 0 || m == 1 { return }
        if m == 2 || m == 3 {
            let len = Int(value)
            for _ in 0..<len { _ = try byte() }
            return
        }
        if m == 4 {
            let len = Int(value)
            for _ in 0..<len { try validateItem() }
            return
        }
        throw DecodeError(message: "unexpected major type \(m)")
    }

    func arrayHead() throws -> Int {
        let (major, value) = try head()
        if major != 4 { throw DecodeError(message: "expected array") }
        return Int(value)
    }
    func uintHead() throws -> UInt64 {
        let (major, value) = try head()
        if major != 0 { throw DecodeError(message: "expected uint") }
        return value
    }
    func readBytes() throws -> [UInt8] {
        let (major, value) = try head()
        if major != 2 { throw DecodeError(message: "expected byte string") }
        let len = Int(value)
        let out = Array(bytes[pos..<pos + len]); pos += len
        return out
    }
    func readText() throws -> String {
        let (major, value) = try head()
        if major != 3 { throw DecodeError(message: "expected text string") }
        let len = Int(value)
        let out = Array(bytes[pos..<pos + len]); pos += len
        return String(decoding: out, as: UTF8.self)
    }
    func readBool() throws -> Bool {
        let b = try byte()
        if b == 0xf5 { return true }
        if b == 0xf4 { return false }
        throw DecodeError(message: "expected bool")
    }
}

private func assertCanonical(_ bytes: [UInt8]) throws {
    let r = Reader(bytes)
    try r.validateItem()
    if r.remaining() != 0 { throw DecodeError(message: "trailing bytes after canonical item") }
}

private func decodeCommunityListingCanonical(_ bytes: [UInt8]) throws {
    let r = Reader(bytes)
    if try r.arrayHead() != 18 { throw DecodeError(message: "expected 18-element listing") }
    if try r.readText() != COMMUNITY_LISTING_SCHEMA { throw DecodeError(message: "bad schema") }
    for _ in 0..<5 { _ = try r.readBytes() }
    _ = try r.uintHead()
    _ = try r.readBytes()
    _ = try r.uintHead()
    _ = try r.uintHead()
    _ = try r.readBool()
    _ = try r.readText()
    _ = try r.readText()
    try readSortedSet(r, "byte", "topic_tags")
    try readSortedSet(r, "text", "languages")
}

private func readSortedSet(_ r: Reader, _ kind: String, _ name: String) throws {
    let count = try r.arrayHead()
    var previous: [UInt8]? = nil
    for _ in 0..<count {
        let element: [UInt8] = kind == "byte" ? Writer().bstr(try r.readBytes()).bytes : Writer().tstr(try r.readText()).bytes
        if let prev = previous {
            let cmp = compareBytes(element, prev)
            if cmp == 0 { throw DecodeError(message: "duplicate \(name) member") }
            if cmp < 0 { throw DecodeError(message: "unsorted \(name) set") }
        }
        previous = element
    }
}

// ===========================================================================
// The vector-document verifier.
// ===========================================================================

private struct VerifyReport {
    var records = 0
    var digests = 0
    var signatures = 0
    var hmacInputs = 0
    var alternateGrammar = 0
    var peerRoles: [String] = []
    var sentinelsChecked = 0
}

private struct ConformanceError: Error { let message: String }

private func expectEqualBytes(_ actual: [UInt8], _ expectedHex: String, _ context: String) throws {
    if actual != hexToBytes(expectedHex) {
        throw ConformanceError(message: "\(context): expected \(expectedHex), got \(bytesToHex(actual))")
    }
}

private func encBodyForEnvelope(_ record: String, _ body: Fields) -> [UInt8] {
    switch record {
    case "DescriptorEnvelopeV1": return encAnchorDescriptorBody(body)
    case "HostingReceiptV1": return encHostingReceiptBody(body)
    case "ListingReceiptV1": return encListingReceiptBody(body)
    case "WorkChallengeV1": return encWorkChallengeBody(body)
    case "ReplicaSourceAttestationV1": return encReplicaSourceAttestationBody(body)
    default: fatalError("no body encoder for envelope \(record)")
    }
}

private func signatureMessage(_ record: String, _ fields: Fields, _ message: String) -> [UInt8] {
    switch message {
    case "ticket_core_canonical": return encPublicSiteTicketCore(D(fields["core"]))
    case "grant_canonical": return encListingDelegateGrant(fields)
    case "body_canonical": return encBodyForEnvelope(record, D(fields["body"]))
    case "blake3(body_canonical)": return Blake3.hash(encBodyForEnvelope(record, D(fields["body"])))
    default: fatalError("unknown signature message source \(message)")
    }
}

private func deriveControlDigestBody(_ fields: Fields) -> [UInt8] {
    precondition(S(D(fields["semantic"])["kind"]) == "describe", "only describe control digest body supported")
    return Writer().arr(3).uint(1).tstr("describe").raw(encDescribeSemanticBody()).bytes
}

private func verifyDigest(_ d: Fields, _ canonical: [UInt8], _ v: Fields, _ report: inout VerifyReport) throws {
    switch S(d["algo"]) {
    case "digest_v1":
        var message: [UInt8]
        if S(d["message"]) == "canonical" {
            message = canonical
        } else {
            message = deriveControlDigestBody(D(v["fields"]))
            try expectEqualBytes(message, S(d["message_hex"]), "control digest body")
        }
        let preimage = digestV1Preimage(S(d["label_ascii"]), message)
        try expectEqualBytes(preimage, S(d["preimage_hex"]), "digest \(S(d["name"])) preimage")
        try expectEqualBytes(Blake3.hash(preimage), S(d["value_hex"]), "digest \(S(d["name"])) value")
        var flipped = message; flipped[0] ^= 0x01
        if Blake3.hash(digestV1Preimage(S(d["label_ascii"]), flipped)) == hexToBytes(S(d["value_hex"])) {
            throw ConformanceError(message: "digest did not change under a one-bit mutation")
        }
        report.digests += 1
    case "operator_key_id":
        let preimage = operatorKeyIdPreimage(canonical)
        try expectEqualBytes(preimage, S(d["preimage_hex"]), "operator_key_id preimage")
        try expectEqualBytes(Blake3.hash(preimage), S(d["value_hex"]), "operator_key_id value")
        report.digests += 1
    case "anchor_id":
        let inputs = D(d["inputs"])
        let preimage = anchorIdPreimage(BX(inputs["genesis_operator_public_key"]), BX(inputs["genesis_random_256_bits"]))
        try expectEqualBytes(preimage, S(d["preimage_hex"]), "anchor_id preimage")
        try expectEqualBytes(Blake3.hash(preimage), S(d["value_hex"]), "anchor_id value")
        report.digests += 1
    case "work_proof":
        let inputs = D(d["inputs"])
        let preimage = workProofPreimage(BX(inputs["work_challenge_digest"]), U(inputs["counter"]))
        try expectEqualBytes(preimage, S(d["preimage_hex"]), "work_proof preimage")
        try expectEqualBytes(Blake3.hash(preimage), S(d["value_hex"]), "work_proof value")
        report.digests += 1
    case "snapshot_cursor_hmac_input":
        let input = snapshotCursorHmacInput(canonical)
        try expectEqualBytes(input, S(d["preimage_hex"]), "snapshot_cursor_hmac_input")
        report.hmacInputs += 1
    default:
        throw ConformanceError(message: "unknown digest algo \(S(d["algo"]))")
    }
}

private func verifyRecordVector(_ v: Fields, _ report: inout VerifyReport) throws {
    let canonical = encodeRecord(S(v["record"]), D(v["fields"]))
    try expectEqualBytes(canonical, S(v["canonical_hex"]), "record \(S(v["id"])) (\(S(v["record"])))")
    report.records += 1

    for d in A(v["digests"]) { try verifyDigest(D(d), canonical, v, &report) }

    for sgnAny in A(v["signatures"]) {
        let sgn = D(sgnAny)
        let pk = hexToBytes(S(sgn["public_key_hex"]))
        let message = signatureMessage(S(v["record"]), D(v["fields"]), S(sgn["message"]))
        let preimage = concat(utf8(S(sgn["domain_ascii"])), message)
        try expectEqualBytes(preimage, S(sgn["preimage_hex"]), "signature \(S(sgn["name"])) preimage of \(S(v["id"]))")
        let sig = hexToBytes(S(sgn["signature_hex"]))
        if !ed25519Verify(pk, preimage, sig) { throw ConformanceError(message: "signature \(S(sgn["name"])) of \(S(v["id"])) failed to verify") }
        var tampered = preimage; tampered[0] ^= 0x01
        if ed25519Verify(pk, tampered, sig) { throw ConformanceError(message: "tampered signature \(S(sgn["name"])) verified") }
        report.signatures += 1
    }
}

private func verifyDigestVector(_ d: Fields, _ report: inout VerifyReport) throws {
    switch S(d["algo"]) {
    case "digest_v1_over_message":
        let inputs = D(d["inputs"])
        let message = hexToBytes(S(inputs["message_hex"]))
        let preimage = digestV1Preimage(S(d["label_ascii"]), message)
        try expectEqualBytes(preimage, S(d["preimage_hex"]), "\(S(d["id"])) preimage")
        try expectEqualBytes(Blake3.hash(preimage), S(d["value_hex"]), "\(S(d["id"])) value")
        report.digests += 1
    case "sync_snapshot":
        let inputs = D(d["inputs"])
        let sortedIds = A(inputs["sorted_entry_ids"]).map { hexToBytes(S($0)) }
        let value = syncSnapshotDigest(BX(inputs["namespace_id"]), U(inputs["entry_count"]), U(inputs["logical_bytes"]), sortedIds)
        try expectEqualBytes(value, S(d["value_hex"]), "\(S(d["id"]))")
        report.digests += 1
    case "namespace_token_hmac_input":
        let inputs = D(d["inputs"])
        let input = namespaceTokenHmacInput(BX(inputs["operation_id"]), BX(inputs["namespace_id"]), U(inputs["operation_expiry_unix_seconds"]), Int(U(inputs["token_secret_epoch"])))
        try expectEqualBytes(input, S(d["preimage_hex"]), "\(S(d["id"]))")
        report.hmacInputs += 1
    case "peer_proof_preimage":
        let inputs = D(d["inputs"])
        let role = S(inputs["role"])
        let preimage = peerProofPreimage(role, BX(inputs["peer_transcript_digest"]))
        try expectEqualBytes(preimage, S(d["preimage_hex"]), "\(S(d["id"])) preimage")
        if let sig = d["signature"] as? Fields {
            if !ed25519Verify(hexToBytes(S(sig["public_key_hex"])), preimage, hexToBytes(S(sig["signature_hex"]))) {
                throw ConformanceError(message: "\(S(d["id"])) peer-proof signature failed to verify")
            }
        }
        report.peerRoles.append(role)
    default:
        throw ConformanceError(message: "unknown digest vector algo \(S(d["algo"]))")
    }
}

// Sentinel token tables — Swift's independent copy, cross-checked against the fixture.
private let SENTINELS: [String: [String]] = [
    "transport_floor": ["require_none", "require_arti"],
    "enabled_role": ["directory", "gossip", "host", "mirror"],
    "control_operation_kind": ["describe", "get_work_challenge", "prepare_host", "commit_host", "submit_listing", "prepare_replica", "pull_directory_feed", "pull_directory_snapshot", "get_operation"],
    "hosting_status": ["committed"],
    "control_outcome": ["success", "refused"],
]

private func verifySentinels(_ doc: Fields, _ report: inout VerifyReport) throws {
    let sentinels = D(doc["sentinels"])
    for (name, expected) in SENTINELS {
        let actual = A(sentinels[name]).map { S($0) }
        if actual != expected { throw ConformanceError(message: "sentinel table \(name) disagrees with the fixture") }
        report.sentinelsChecked += 1
    }
}

private func verifyAlternateGrammar(_ doc: Fields, _ report: inout VerifyReport) throws {
    for gAny in A(doc["alternate_grammar"]) {
        let g = D(gAny)
        let hostile = hexToBytes(S(g["hostile_hex"]))
        var rejected = false
        do { try assertCanonical(hostile) } catch { rejected = true }
        if !rejected && S(g["record"]) == "CommunityListingV1" {
            do { try decodeCommunityListingCanonical(hostile) } catch { rejected = true }
        }
        if !rejected { throw ConformanceError(message: "alternate grammar '\(S(g["desc"]))' was not rejected") }
        report.alternateGrammar += 1
    }
}

private func verifyVectorDocument(_ doc: Fields) throws -> VerifyReport {
    if S(doc["protocol"]) != "riot-anchor-protocol" { throw ConformanceError(message: "unexpected protocol tag") }
    var report = VerifyReport()
    for v in A(doc["vectors"]) { try verifyRecordVector(D(v), &report) }
    for d in A(doc["digest_vectors"]) { try verifyDigestVector(D(d), &report) }
    try verifySentinels(doc, &report)
    try verifyAlternateGrammar(doc, &report)
    if !report.peerRoles.contains("initiator") || !report.peerRoles.contains("responder") {
        throw ConformanceError(message: "peer roles incomplete: \(report.peerRoles.joined(separator: ","))")
    }
    return report
}

// ===========================================================================
// XCTest harness.
// ===========================================================================

final class AnchorProtocolVectorTests: XCTestCase {
    private var testBundle: Bundle { Bundle(for: type(of: self)) }

    private func loadDocument() throws -> Fields {
        let url = try XCTUnwrap(
            testBundle.url(forResource: "protocol-v1-vectors", withExtension: "json"),
            "protocol-v1-vectors.json is not bundled into the RiotTests resources"
        )
        let data = try Data(contentsOf: url)
        let obj = try JSONSerialization.jsonObject(with: data)
        return try XCTUnwrap(obj as? Fields, "vector document did not parse as an object")
    }

    // BLAKE3 KATs anchor the vendored hash to genuine BLAKE3, independent of the fixture.
    private func referenceInput(_ n: Int) -> [UInt8] { (0..<n).map { UInt8($0 % 251) } }

    func testBlake3MatchesOfficialEmptyVector() {
        XCTAssertEqual(
            bytesToHex(Blake3.hash(referenceInput(0))),
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
        )
    }

    func testBlake3MatchesFixtureKnownAnswerTests() throws {
        let doc = try loadDocument()
        let kats = A(doc["blake3_kats"])
        XCTAssertGreaterThanOrEqual(kats.count, 6)
        var sawMultiChunk = false
        for katAny in kats {
            let kat = D(katAny)
            let n = Int(U(kat["input_len"]))
            XCTAssertEqual(bytesToHex(Blake3.hash(referenceInput(n))), S(kat["hash_hex"]), "blake3 of \(n) bytes")
            if n > 1024 { sawMultiChunk = true }
        }
        XCTAssertTrue(sawMultiChunk, "BLAKE3 multi-chunk tree path must be exercised")
    }

    func testEveryVectorIsReproducedAndVerifiedIndependently() throws {
        let doc = try loadDocument()
        let report = try verifyVectorDocument(doc)
        XCTAssertGreaterThanOrEqual(report.records, 25, "expected >=25 record vectors, got \(report.records)")
        XCTAssertGreaterThanOrEqual(report.digests, 10, "expected >=10 digest checks, got \(report.digests)")
        XCTAssertGreaterThanOrEqual(report.signatures, 6, "expected >=6 signature checks, got \(report.signatures)")
        XCTAssertGreaterThanOrEqual(report.hmacInputs, 2, "expected >=2 HMAC-input checks, got \(report.hmacInputs)")
        XCTAssertGreaterThanOrEqual(report.alternateGrammar, 5, "expected >=5 alt-grammar rejections, got \(report.alternateGrammar)")
        XCTAssertEqual(report.peerRoles.sorted(), ["initiator", "responder"])
        XCTAssertGreaterThanOrEqual(report.sentinelsChecked, 5)
    }

    func testOneBitMutationIsDetected() throws {
        let doc = try loadDocument()
        let v = try XCTUnwrap(A(doc["vectors"]).map { D($0) }.first { S($0["id"]) == "public_site_ticket_core" })
        let canonical = encodeRecord(S(v["record"]), D(v["fields"]))
        XCTAssertEqual(bytesToHex(canonical), S(v["canonical_hex"]))
        var mutated = canonical; mutated[5] ^= 0x01
        XCTAssertNotEqual(bytesToHex(mutated), bytesToHex(canonical))
        // Re-encoding from the semantic fields rejects the mutation.
        XCTAssertNotEqual(bytesToHex(encodeRecord(S(v["record"]), D(v["fields"]))), bytesToHex(mutated))
    }

    func testAlternateGrammarEncodingsAreRejected() throws {
        // Hand-rolled hostile forms rejected by the independent decoder.
        XCTAssertThrowsError(try assertCanonical(hexToBytes("9f0102ff")))  // indefinite-length array
        XCTAssertThrowsError(try assertCanonical(hexToBytes("1801")))       // non-minimal integer
        XCTAssertThrowsError(try assertCanonical(hexToBytes("a10102")))     // a map
        XCTAssertThrowsError(try assertCanonical(hexToBytes("0100")))       // trailing bytes

        let doc = try loadDocument()
        for gAny in A(doc["alternate_grammar"]) {
            let g = D(gAny)
            let hostile = hexToBytes(S(g["hostile_hex"]))
            var rejected = false
            do { try assertCanonical(hostile) } catch { rejected = true }
            if !rejected && S(g["record"]) == "CommunityListingV1" {
                XCTAssertThrowsError(try decodeCommunityListingCanonical(hostile))
                rejected = true
            }
            XCTAssertTrue(rejected, "alt-grammar '\(S(g["desc"]))' should be rejected")
        }
    }

    func testDevelopmentBootstrapResourceParsesButIsNotReleaseEligible() throws {
        let doc = try loadDocument()
        let url = try XCTUnwrap(
            testBundle.url(forResource: "bootstrap-development-v1", withExtension: "cbor"),
            "bootstrap-development-v1.cbor is not bundled into the RiotTests resources"
        )
        let cbor = [UInt8](try Data(contentsOf: url))
        let vector = try XCTUnwrap(A(doc["vectors"]).map { D($0) }.first { S($0["record"]) == "AnchorBootstrapV1" })

        // The checked-in .cbor is byte-identical to the independently re-encoded record.
        XCTAssertEqual(bytesToHex(cbor), S(vector["canonical_hex"]))
        XCTAssertEqual(bytesToHex(encodeRecord("AnchorBootstrapV1", D(vector["fields"]))), bytesToHex(cbor))

        let descriptors = A(D(vector["fields"])["descriptors"]).map { D($0) }
        XCTAssertGreaterThanOrEqual(descriptors.count, 3)
        // Distinct operator keys across the descriptors (>=2 operators).
        let operatorKeys = Set(descriptors.map { S(D(D($0["floor"])["operator_verification_key"])["public_key"]) })
        XCTAssertGreaterThanOrEqual(operatorKeys.count, 2)

        // Release validation refuses it: every origin is a visibly development-only .dev.invalid host.
        let allDev = descriptors.allSatisfy { S($0["https_origin"]).contains(".dev.invalid") }
        let releaseEligible = descriptors.count >= 3 && operatorKeys.count >= 2 && !allDev
        XCTAssertFalse(releaseEligible, "development bootstrap must not be release-eligible")
        XCTAssertTrue(allDev)
    }
}
