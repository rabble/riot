package org.riot.evidence

import java.security.KeyFactory
import java.security.Signature
import java.security.spec.X509EncodedKeySpec
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * WU-006B — independent Kotlin reproduction of the anchor protocol wire format.
 *
 * The FOURTH, INDEPENDENT implementation (after Rust, TypeScript, and Swift) of the
 * canonical anchor records pinned by `fixtures/anchor/protocol-v1-vectors.json`. It
 * never asks Rust for expected bytes: it re-encodes every record from the vector's
 * semantic fields with its own canonical positional-CBOR encoder, re-frames and
 * re-hashes every digest/preimage with a vendored pure-Kotlin BLAKE3 (anchored by the
 * fixture's BLAKE3 known-answer tests), re-derives every HMAC input, and verifies every
 * ed25519 signature with the host JDK's built-in Ed25519 (JDK 15+, no gradle
 * dependency added). Byte-for-byte agreement with the Rust-emitted fixture is the
 * cross-language conformance proof.
 *
 * This runs on the host JVM (`testDebugUnitTest`); it never touches `libriot_ffi`.
 */
@Suppress("UNCHECKED_CAST")
class AnchorProtocolVectorTest {

    // =======================================================================
    // Vendored BLAKE3 (pure Kotlin, 32-byte output, default unkeyed mode).
    // =======================================================================
    private object Blake3 {
        private val IV = intArrayOf(
            0x6a09e667.toInt(), 0xbb67ae85.toInt(), 0x3c6ef372, 0xa54ff53a.toInt(),
            0x510e527f, 0x9b05688c.toInt(), 0x1f83d9ab, 0x5be0cd19,
        )
        private val MSG_PERMUTATION = intArrayOf(2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8)
        private const val CHUNK_START = 1 shl 0
        private const val CHUNK_END = 1 shl 1
        private const val PARENT = 1 shl 2
        private const val ROOT = 1 shl 3
        private const val BLOCK_LEN = 64
        private const val CHUNK_LEN = 1024

        private fun rotr32(x: Int, n: Int): Int = (x ushr n) or (x shl (32 - n))

        private fun g(v: IntArray, a: Int, b: Int, c: Int, d: Int, mx: Int, my: Int) {
            v[a] = v[a] + v[b] + mx
            v[d] = rotr32(v[d] xor v[a], 16)
            v[c] = v[c] + v[d]
            v[b] = rotr32(v[b] xor v[c], 12)
            v[a] = v[a] + v[b] + my
            v[d] = rotr32(v[d] xor v[a], 8)
            v[c] = v[c] + v[d]
            v[b] = rotr32(v[b] xor v[c], 7)
        }

        private fun compress(cv: IntArray, blockWords: IntArray, counter: Long, blockLen: Int, flags: Int): IntArray {
            val v = IntArray(16)
            for (i in 0 until 8) v[i] = cv[i]
            for (i in 0 until 4) v[8 + i] = IV[i]
            v[12] = (counter and 0xffffffffL).toInt()
            v[13] = ((counter ushr 32) and 0xffffffffL).toInt()
            v[14] = blockLen
            v[15] = flags

            var m = blockWords
            for (round in 0 until 7) {
                g(v, 0, 4, 8, 12, m[0], m[1])
                g(v, 1, 5, 9, 13, m[2], m[3])
                g(v, 2, 6, 10, 14, m[4], m[5])
                g(v, 3, 7, 11, 15, m[6], m[7])
                g(v, 0, 5, 10, 15, m[8], m[9])
                g(v, 1, 6, 11, 12, m[10], m[11])
                g(v, 2, 7, 8, 13, m[12], m[13])
                g(v, 3, 4, 9, 14, m[14], m[15])
                if (round < 6) {
                    val permuted = IntArray(16)
                    for (i in 0 until 16) permuted[i] = m[MSG_PERMUTATION[i]]
                    m = permuted
                }
            }
            val out = IntArray(16)
            for (i in 0 until 8) {
                out[i] = v[i] xor v[i + 8]
                out[i + 8] = v[i + 8] xor cv[i]
            }
            return out
        }

        private fun wordsFromBlock(block: ByteArray, start: Int, len: Int): IntArray {
            val padded = ByteArray(BLOCK_LEN)
            for (i in 0 until len) padded[i] = block[start + i]
            val words = IntArray(16)
            for (i in 0 until 16) {
                val o = i * 4
                words[i] = (padded[o].toInt() and 0xff) or
                    ((padded[o + 1].toInt() and 0xff) shl 8) or
                    ((padded[o + 2].toInt() and 0xff) shl 16) or
                    ((padded[o + 3].toInt() and 0xff) shl 24)
            }
            return words
        }

        private class Output(
            val inputCv: IntArray,
            val blockWords: IntArray,
            val counter: Long,
            val blockLen: Int,
            val flags: Int,
        )

        private fun chainingValue(o: Output): IntArray =
            compress(o.inputCv, o.blockWords, o.counter, o.blockLen, o.flags).copyOfRange(0, 8)

        private fun rootBytes(o: Output, length: Int): ByteArray {
            val out = ByteArray(length)
            var counter = 0L
            var filled = 0
            while (filled < length) {
                val words = compress(o.inputCv, o.blockWords, counter, o.blockLen, o.flags or ROOT)
                var i = 0
                while (i < 16 && filled < length) {
                    var w = words[i]
                    var b = 0
                    while (b < 4 && filled < length) {
                        out[filled] = (w and 0xff).toByte()
                        w = w ushr 8
                        filled++
                        b++
                    }
                    i++
                }
                counter += 1
            }
            return out
        }

        private fun chunkToOutput(bytes: ByteArray, from: Int, to: Int, chunkCounter: Long): Output {
            var cv = IV
            val len = to - from
            val blockCount = if (len == 0) 1 else (len + BLOCK_LEN - 1) / BLOCK_LEN
            for (i in 0 until blockCount) {
                val start = from + i * BLOCK_LEN
                val end = minOf(start + BLOCK_LEN, to)
                val blockLen = end - start
                val words = wordsFromBlock(bytes, start, blockLen)
                var flags = 0
                if (i == 0) flags = flags or CHUNK_START
                val isLast = i == blockCount - 1
                if (isLast) {
                    return Output(cv, words, chunkCounter, blockLen, flags or CHUNK_END)
                }
                cv = compress(cv, words, chunkCounter, BLOCK_LEN, flags).copyOfRange(0, 8)
            }
            error("blake3: empty chunk loop")
        }

        private fun parentOutput(left: IntArray, right: IntArray): Output {
            val block = IntArray(16)
            for (i in 0 until 8) {
                block[i] = left[i]
                block[8 + i] = right[i]
            }
            return Output(IV, block, 0, BLOCK_LEN, PARENT)
        }

        private fun leftLen(contentLen: Int): Int {
            val fullChunks = (contentLen - 1) / CHUNK_LEN
            var p = 1
            while (p * 2 <= fullChunks) p *= 2
            return p * CHUNK_LEN
        }

        private fun hashRange(bytes: ByteArray, from: Int, to: Int, chunkCounter: Long): Output {
            if (to - from <= CHUNK_LEN) {
                return chunkToOutput(bytes, from, to, chunkCounter)
            }
            val ll = leftLen(to - from)
            val mid = from + ll
            val left = hashRange(bytes, from, mid, chunkCounter)
            val right = hashRange(bytes, mid, to, chunkCounter + (ll / CHUNK_LEN).toLong())
            return parentOutput(chainingValue(left), chainingValue(right))
        }

        fun hash(input: ByteArray): ByteArray = rootBytes(hashRange(input, 0, input.size, 0), 32)
    }

    // =======================================================================
    // Hex / byte helpers.
    // =======================================================================
    private fun hexToBytes(hex: String): ByteArray {
        require(hex.length % 2 == 0) { "odd-length hex" }
        val out = ByteArray(hex.length / 2)
        for (i in out.indices) {
            out[i] = ((hex[i * 2].digitToInt(16) shl 4) or hex[i * 2 + 1].digitToInt(16)).toByte()
        }
        return out
    }

    private fun bytesToHex(bytes: ByteArray): String {
        val sb = StringBuilder(bytes.size * 2)
        for (b in bytes) {
            val v = b.toInt() and 0xff
            sb.append("0123456789abcdef"[v ushr 4])
            sb.append("0123456789abcdef"[v and 0x0f])
        }
        return sb.toString()
    }

    private fun utf8(s: String): ByteArray = s.toByteArray(Charsets.UTF_8)

    private fun concat(vararg parts: ByteArray): ByteArray {
        val total = parts.sumOf { it.size }
        val out = ByteArray(total)
        var off = 0
        for (p in parts) {
            System.arraycopy(p, 0, out, off, p.size)
            off += p.size
        }
        return out
    }

    private fun u16be(n: Int): ByteArray = byteArrayOf(((n ushr 8) and 0xff).toByte(), (n and 0xff).toByte())
    private fun u32be(n: Int): ByteArray = byteArrayOf(
        ((n ushr 24) and 0xff).toByte(), ((n ushr 16) and 0xff).toByte(),
        ((n ushr 8) and 0xff).toByte(), (n and 0xff).toByte(),
    )
    private fun u64be(n: Long): ByteArray {
        val out = ByteArray(8)
        var v = n
        var i = 7
        while (i >= 0) {
            out[i] = (v and 0xff).toByte()
            v = v ushr 8
            i--
        }
        return out
    }

    private fun compareBytes(a: ByteArray, b: ByteArray): Int {
        val n = minOf(a.size, b.size)
        for (i in 0 until n) {
            val ai = a[i].toInt() and 0xff
            val bi = b[i].toInt() and 0xff
            if (ai != bi) return ai - bi
        }
        return a.size - b.size
    }

    // JSON accessors (the vector document arrives as nested Any? from the parser).
    private fun D(v: Any?): Map<String, Any?> = (v as? Map<String, Any?>) ?: emptyMap()
    private fun L(v: Any?): List<Any?> = (v as? List<Any?>) ?: emptyList()
    private fun S(v: Any?): String = when (v) {
        is String -> v
        else -> error("expected string, got $v")
    }
    private fun U(v: Any?): Long = S(v).toLong()
    private fun BOOL(v: Any?): Boolean = v as Boolean
    private fun isNull(v: Any?): Boolean = v == null
    private fun BX(v: Any?): ByteArray = hexToBytes(S(v))

    // =======================================================================
    // Canonical positional-CBOR encoder.
    // =======================================================================
    private inner class Writer {
        val buf = ArrayList<Byte>(256)

        private fun head(major: Int, n: Long) {
            val m = major shl 5
            when {
                n < 24 -> buf.add((m or n.toInt()).toByte())
                n < 0x100 -> { buf.add((m or 24).toByte()); buf.add(n.toByte()) }
                n < 0x10000 -> { buf.add((m or 25).toByte()); pushBE(n, 2) }
                n < 0x100000000L -> { buf.add((m or 26).toByte()); pushBE(n, 4) }
                else -> { buf.add((m or 27).toByte()); pushBE(n, 8) }
            }
        }

        private fun pushBE(n: Long, len: Int) {
            var i = len - 1
            while (i >= 0) {
                buf.add(((n ushr (8 * i)) and 0xff).toByte())
                i--
            }
        }

        fun uint(n: Long): Writer { head(0, n); return this }
        fun bstr(u8: ByteArray): Writer { head(2, u8.size.toLong()); for (b in u8) buf.add(b); return this }
        fun tstr(s: String): Writer { val u8 = utf8(s); head(3, u8.size.toLong()); for (b in u8) buf.add(b); return this }
        fun bool(b: Boolean): Writer { buf.add(if (b) 0xf5.toByte() else 0xf4.toByte()); return this }
        fun nul(): Writer { buf.add(0xf6.toByte()); return this }
        fun arr(n: Int): Writer { head(4, n.toLong()); return this }
        fun raw(u8: ByteArray): Writer { for (b in u8) buf.add(b); return this }
        fun out(): ByteArray = buf.toByteArray()
    }

    private fun encodeSet(w: Writer, elements: List<ByteArray>) {
        val sorted = elements.sortedWith { a, b -> compareBytes(a, b) }
        for (i in 1 until sorted.size) {
            if (compareBytes(sorted[i - 1], sorted[i]) == 0) error("duplicate set member")
        }
        w.arr(sorted.size)
        for (e in sorted) w.raw(e)
    }

    private fun encByteSet(hexes: List<Any?>): List<ByteArray> = hexes.map { Writer().bstr(hexToBytes(S(it))).out() }
    private fun encTextSet(items: List<Any?>): List<ByteArray> = items.map { Writer().tstr(S(it)).out() }
    private fun encUintSet(items: List<Any?>): List<ByteArray> = items.map { Writer().uint(U(it)).out() }

    // =======================================================================
    // Per-record encoders — each mirrors the crate's `encode_canonical` positionally.
    // =======================================================================
    private val communityListingSchema = "riot/community-listing/1"

    private fun encOperatorKey(f: Map<String, Any?>): ByteArray =
        Writer().arr(3).uint(1).tstr("ed25519").bstr(BX(f["public_key"])).out()

    private fun encPublicSiteTicketCore(f: Map<String, Any?>): ByteArray =
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
            .out()

    private fun encRootSignedTicketCoreEnvelope(f: Map<String, Any?>): ByteArray =
        Writer().arr(3).uint(2).raw(encPublicSiteTicketCore(D(f["core"]))).bstr(BX(f["root_signature"])).out()

    private fun encListingDelegateGrant(f: Map<String, Any?>): ByteArray =
        Writer().arr(6)
            .bstr(BX(f["root_id"]))
            .bstr(BX(f["delegate_key"]))
            .bstr(BX(f["terminal_capability_digest"]))
            .uint(U(f["listing_epoch"]))
            .uint(U(f["issued_unix_seconds"]))
            .uint(U(f["expiry_unix_seconds"]))
            .out()

    private fun encCommunityListing(f: Map<String, Any?>): ByteArray {
        require(S(f["schema"]) == communityListingSchema) { "unexpected schema" }
        val w = Writer().arr(19)
            .tstr(communityListingSchema)
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
        encodeSet(w, encByteSet(L(f["topic_tags"])))
        encodeSet(w, encTextSet(L(f["languages"])))
        if (isNull(f["region"])) w.nul() else w.bstr(BX(f["region"]))
        w.uint(U(f["issued_unix_seconds"])).uint(U(f["expiry_unix_seconds"]))
        if (isNull(f["steward_name"])) w.nul() else w.tstr(S(f["steward_name"]))
        return w.out()
    }

    private fun encAdmittedListingEnvelope(f: Map<String, Any?>): ByteArray {
        val w = Writer().arr(4).uint(1)
            .bstr(BX(f["signed_listing_entry_bytes"]))
            .bstr(BX(f["capability_chain_bytes"]))
        if (isNull(f["delegate_grant_bytes"])) w.nul() else w.bstr(BX(f["delegate_grant_bytes"]))
        return w.out()
    }

    private fun encLimitValue(w: Writer, v: Any?) {
        if (v is List<*>) w.arr(2).uint(U(v[0])).uint(U(v[1])) else w.uint(U(v))
    }

    private fun encAnchorLimitProfile(f: Map<String, Any?>): ByteArray {
        val w = Writer().arr(3).uint(1).uint(U(f["profile_epoch"]))
        val entries = L(f["entries"])
        w.arr(entries.size)
        for (e in entries) {
            val ed = D(e)
            w.arr(3).uint(U(ed["id"]))
            encLimitValue(w, ed["effective"])
            encLimitValue(w, ed["absolute"])
        }
        return w.out()
    }

    private fun encDescriptorFloor(f: Map<String, Any?>): ByteArray =
        Writer().arr(4)
            .bstr(BX(f["anchor_id"]))
            .uint(U(f["descriptor_epoch"]))
            .bstr(BX(f["descriptor_digest"]))
            .raw(encOperatorKey(D(f["operator_verification_key"])))
            .out()

    private fun encRoleSet(w: Writer, roles: List<Any?>) = encodeSet(w, encTextSet(roles))

    private fun encAnchorDescriptorBody(f: Map<String, Any?>): ByteArray {
        val w = Writer().arr(19).uint(1)
            .bstr(BX(f["anchor_id"]))
            .bstr(BX(f["genesis_operator_public_key"]))
            .bstr(BX(f["genesis_random_256_bits"]))
            .raw(encOperatorKey(D(f["current_operator_verification_key"])))
            .bstr(BX(f["current_operator_key_id"]))
            .uint(U(f["descriptor_epoch"]))
        if (isNull(f["previous_descriptor_digest"])) w.nul() else w.bstr(BX(f["previous_descriptor_digest"]))
        w.bstr(BX(f["current_iroh_endpoint_id"]))
            .tstr(S(f["https_origin"]))
            .tstr(S(f["operator_display_label"]))
            .tstr(S(f["self_reported_failure_domain_label"]))
        encodeSet(w, encUintSet(L(f["supported_control_versions"])))
        encodeSet(w, encUintSet(L(f["supported_sync_versions"])))
        encRoleSet(w, L(f["enabled_roles"]))
        w.bstr(BX(f["limit_profile_digest"]))
        if (isNull(f["predecessor_operator_verification_key"])) w.nul()
        else w.raw(encOperatorKey(D(f["predecessor_operator_verification_key"])))
        w.uint(U(f["issued_at"])).uint(U(f["expires_at"]))
        return w.out()
    }

    private fun encDescriptorEnvelope(f: Map<String, Any?>): ByteArray {
        val w = Writer().arr(4).uint(1).raw(encAnchorDescriptorBody(D(f["body"]))).bstr(BX(f["current_signature"]))
        if (isNull(f["predecessor_signature"])) w.nul() else w.bstr(BX(f["predecessor_signature"]))
        return w.out()
    }

    private fun encNamespaceResult(w: Writer, r: Map<String, Any?>) {
        w.arr(3).bstr(BX(r["namespace_id"])).bstr(BX(r["snapshot_digest"])).uint(U(r["entry_count"]))
    }

    private fun encHostingReceiptBody(f: Map<String, Any?>): ByteArray {
        val w = Writer().arr(16).uint(1)
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
        val results = L(f["ordered_namespace_results"])
        w.arr(results.size)
        for (r in results) encNamespaceResult(w, D(r))
        w.tstr(S(f["status"])).uint(U(f["accepted_at"])).uint(U(f["reported_retention_through"])).bstr(BX(f["limit_profile_digest"]))
        return w.out()
    }

    private fun encHostingReceiptEnvelope(f: Map<String, Any?>): ByteArray =
        Writer().arr(3).uint(1).raw(encHostingReceiptBody(D(f["body"]))).bstr(BX(f["operator_signature"])).out()

    private fun encListingReceiptBody(f: Map<String, Any?>): ByteArray =
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
            .out()

    private fun encListingReceiptEnvelope(f: Map<String, Any?>): ByteArray =
        Writer().arr(3).uint(1).raw(encListingReceiptBody(D(f["body"]))).bstr(BX(f["operator_signature"])).out()

    private fun encWorkChallengeBody(f: Map<String, Any?>): ByteArray =
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
            .out()

    private fun encWorkChallengeEnvelope(f: Map<String, Any?>): ByteArray =
        Writer().arr(3).uint(1).raw(encWorkChallengeBody(D(f["body"]))).bstr(BX(f["operator_signature"])).out()

    private fun encWorkStamp(f: Map<String, Any?>): ByteArray =
        Writer().arr(4).uint(1).bstr(BX(f["challenge_envelope_bytes"])).uint(U(f["counter"])).bstr(BX(f["proof_bytes"])).out()

    private fun encReplicaPrepareChallenge(f: Map<String, Any?>): ByteArray =
        Writer().arr(7).uint(1)
            .bstr(BX(f["destination_anchor_id"]))
            .bstr(BX(f["random_256_bit_nonce"]))
            .bstr(BX(f["prepare_idempotency_key"]))
            .bstr(BX(f["full_site_root"]))
            .uint(U(f["issued_at"]))
            .uint(U(f["expires_at"]))
            .out()

    private fun encReplicaSourceAttestationBody(f: Map<String, Any?>): ByteArray {
        val w = Writer().arr(16).uint(1)
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
        val digests = L(f["ordered_namespace_snapshot_digests"])
        w.arr(digests.size)
        for (d in digests) w.bstr(hexToBytes(S(d)))
        w.uint(U(f["issued_at"])).uint(U(f["expires_at"]))
        return w.out()
    }

    private fun encReplicaSourceAttestationEnvelope(f: Map<String, Any?>): ByteArray =
        Writer().arr(3).uint(1).raw(encReplicaSourceAttestationBody(D(f["body"]))).bstr(BX(f["operator_signature"])).out()

    private fun encSnapshotCursorBody(f: Map<String, Any?>): ByteArray {
        val w = Writer().arr(8).uint(1)
            .bstr(BX(f["checkpoint_digest"]))
            .uint(U(f["snapshot_generation_id"]))
            .uint(U(f["next_ordinal"]))
        if (isNull(f["previous_root"])) w.nul() else w.bstr(BX(f["previous_root"]))
        w.uint(U(f["issued_at"])).uint(U(f["expires_at"])).uint(U(f["cursor_secret_epoch"]))
        return w.out()
    }

    private fun encSnapshotCursor(f: Map<String, Any?>): ByteArray =
        Writer().arr(3).uint(1).raw(encSnapshotCursorBody(D(f["body"]))).bstr(BX(f["cursor_tag"])).out()

    private fun encBootstrapDescriptor(d: Map<String, Any?>): ByteArray {
        val w = Writer().arr(4).uint(1).raw(encDescriptorFloor(D(d["floor"]))).tstr(S(d["https_origin"]))
        encRoleSet(w, L(d["roles"]))
        return w.out()
    }

    private fun encAnchorBootstrap(f: Map<String, Any?>): ByteArray {
        val descriptors = L(f["descriptors"])
        val w = Writer().arr(2).uint(1).arr(descriptors.size)
        for (d in descriptors) w.raw(encBootstrapDescriptor(D(d)))
        return w.out()
    }

    private fun encDescribeSemanticBody(): ByteArray = Writer().arr(1).uint(1).out()

    private fun encControlRequest(f: Map<String, Any?>): ByteArray {
        require(S(D(f["semantic"])["kind"]) == "describe") { "control request encoder only supports describe" }
        return Writer().arr(4).uint(1)
            .tstr(S(f["operation_kind"]))
            .bstr(BX(f["idempotency_key"]))
            .raw(encDescribeSemanticBody())
            .out()
    }

    private fun encNotHostedRefusal(): ByteArray =
        Writer().arr(5).tstr("not_hosted").tstr("listing").bool(true).nul().arr(1).tstr("none").out()

    private fun encControlResponse(f: Map<String, Any?>): ByteArray {
        val w = Writer().arr(3).uint(1).tstr(S(f["kind"]))
        val outcome = D(f["outcome"])
        when (S(outcome["type"])) {
            "refused" -> {
                require(S(D(outcome["refusal"])["code"]) == "not_hosted") { "only not_hosted refusal supported" }
                w.arr(2).tstr("refused").raw(encNotHostedRefusal())
            }
            "success" -> {
                val success = D(outcome["success"])
                require(S(success["kind"]) == "submit_listing") { "only submit_listing success supported" }
                val payload = Writer().arr(2).uint(1).raw(encListingReceiptEnvelope(D(success["listing_receipt"]))).out()
                w.arr(2).tstr("success").raw(payload)
            }
            else -> error("unknown outcome")
        }
        return w.out()
    }

    private fun encodeRecord(record: String, fields: Map<String, Any?>): ByteArray = when (record) {
        "OperatorVerificationKeyV1" -> encOperatorKey(fields)
        "PublicSiteTicketV2Core" -> encPublicSiteTicketCore(fields)
        "RootSignedTicketCoreEnvelopeV2" -> encRootSignedTicketCoreEnvelope(fields)
        "ListingDelegateGrantV1" -> encListingDelegateGrant(fields)
        "CommunityListingV1" -> encCommunityListing(fields)
        "AdmittedListingEnvelopeV1" -> encAdmittedListingEnvelope(fields)
        "AnchorLimitProfileV1" -> encAnchorLimitProfile(fields)
        "DescriptorFloor" -> encDescriptorFloor(fields)
        "AnchorDescriptorBodyV1" -> encAnchorDescriptorBody(fields)
        "DescriptorEnvelopeV1" -> encDescriptorEnvelope(fields)
        "HostingReceiptBodyV1" -> encHostingReceiptBody(fields)
        "HostingReceiptV1" -> encHostingReceiptEnvelope(fields)
        "ListingReceiptBodyV1" -> encListingReceiptBody(fields)
        "ListingReceiptV1" -> encListingReceiptEnvelope(fields)
        "WorkChallengeBodyV1" -> encWorkChallengeBody(fields)
        "WorkChallengeV1" -> encWorkChallengeEnvelope(fields)
        "WorkStampV1" -> encWorkStamp(fields)
        "ReplicaPrepareChallengeV1" -> encReplicaPrepareChallenge(fields)
        "ReplicaSourceAttestationBodyV1" -> encReplicaSourceAttestationBody(fields)
        "ReplicaSourceAttestationV1" -> encReplicaSourceAttestationEnvelope(fields)
        "SnapshotCursorBodyV1" -> encSnapshotCursorBody(fields)
        "SnapshotCursorV1" -> encSnapshotCursor(fields)
        "AnchorBootstrapV1" -> encAnchorBootstrap(fields)
        "ControlRequestV1" -> encControlRequest(fields)
        "ControlResponseV1" -> encControlResponse(fields)
        else -> error("no Kotlin encoder for record $record")
    }

    // =======================================================================
    // Domain-separation labels (Kotlin's own independent copy).
    // =======================================================================
    private val labelOperatorKeyId = "riot/anchor-operator-key-id/v1"
    private val labelAnchorId = "riot/anchor-id/v1"
    private val labelWorkProof = "riot/anchor-work-proof/v1"
    private val labelSyncSnapshot = "riot/sync-snapshot/v2"
    private val labelNamespaceToken = "riot/namespace-token/v1"
    private val labelSnapshotCursor = "riot/directory-snapshot-cursor/v1"
    private val labelPeerProof = "riot/anchor-peer-proof/v1"

    private fun digestV1Preimage(labelAscii: String, message: ByteArray): ByteArray {
        val label = utf8(labelAscii)
        return concat(u16be(label.size), label, u64be(message.size.toLong()), message)
    }

    private fun operatorKeyIdPreimage(canonical: ByteArray): ByteArray = concat(utf8(labelOperatorKeyId), canonical)
    private fun anchorIdPreimage(pk: ByteArray, rand: ByteArray): ByteArray = concat(utf8(labelAnchorId), pk, rand)
    private fun workProofPreimage(challengeDigest: ByteArray, counter: Long): ByteArray =
        concat(utf8(labelWorkProof), challengeDigest, u64be(counter))

    private fun syncSnapshotDigest(nsId: ByteArray, entryCount: Long, logicalBytes: Long, sortedIds: List<ByteArray>): ByteArray {
        val parts = ArrayList<ByteArray>()
        parts.add(utf8(labelSyncSnapshot)); parts.add(u32be(nsId.size)); parts.add(nsId)
        parts.add(u64be(entryCount)); parts.add(u64be(logicalBytes))
        for (id in sortedIds) { parts.add(u32be(id.size)); parts.add(id) }
        return Blake3.hash(concat(*parts.toTypedArray()))
    }

    private fun namespaceTokenHmacInput(opId: ByteArray, nsId: ByteArray, expiry: Long, epoch: Int): ByteArray {
        val label = utf8(labelNamespaceToken)
        return concat(u16be(23), label, u16be(opId.size), opId, u16be(nsId.size), nsId, u64be(expiry), u32be(epoch))
    }

    private fun snapshotCursorHmacInput(canonical: ByteArray): ByteArray {
        val label = utf8(labelSnapshotCursor)
        return concat(u16be(33), label, u64be(canonical.size.toLong()), canonical)
    }

    private fun peerProofPreimage(role: String, transcriptDigest: ByteArray): ByteArray {
        val label = utf8(labelPeerProof)
        val roleBytes = utf8(role)
        return concat(u16be(25), label, u16be(roleBytes.size), roleBytes, transcriptDigest)
    }

    // =======================================================================
    // ed25519 verification via the host JDK's built-in Ed25519 (raw 32-byte key).
    // =======================================================================
    private val ed25519SpkiPrefix = hexToBytes("302a300506032b6570032100")

    private fun ed25519Verify(publicKey: ByteArray, message: ByteArray, signature: ByteArray): Boolean {
        if (publicKey.size != 32 || signature.size != 64) return false
        return try {
            val der = concat(ed25519SpkiPrefix, publicKey)
            val key = KeyFactory.getInstance("Ed25519").generatePublic(X509EncodedKeySpec(der))
            val verifier = Signature.getInstance("Ed25519")
            verifier.initVerify(key)
            verifier.update(message)
            verifier.verify(signature)
        } catch (_: Exception) {
            false
        }
    }

    // =======================================================================
    // Canonical-CBOR validator + a CommunityListing set-order decoder.
    // =======================================================================
    private class DecodeException(message: String) : Exception(message)

    private inner class Reader(val bytes: ByteArray, var pos: Int = 0) {
        private fun byte(): Int {
            if (pos >= bytes.size) throw DecodeException("unexpected end of input")
            return (bytes[pos++].toInt() and 0xff)
        }

        fun remaining(): Int = bytes.size - pos

        private fun head(): Pair<Int, Long> {
            val ib = byte()
            val major = ib ushr 5
            val ai = ib and 0x1f
            if (ai < 24) return Pair(major, ai.toLong())
            if (ai == 24) {
                val v = byte()
                if (v < 24) throw DecodeException("non-minimal integer")
                return Pair(major, v.toLong())
            }
            if (ai == 25) {
                val v = ((byte().toLong()) shl 8) or byte().toLong()
                if (v < 0x100) throw DecodeException("non-minimal integer")
                return Pair(major, v)
            }
            if (ai == 26) {
                var v = 0L
                for (i in 0 until 4) v = (v shl 8) or byte().toLong()
                if (v < 0x10000) throw DecodeException("non-minimal integer")
                return Pair(major, v)
            }
            if (ai == 27) {
                var v = 0L
                for (i in 0 until 8) v = (v shl 8) or byte().toLong()
                if (v < 0x100000000L) throw DecodeException("non-minimal integer")
                return Pair(major, v)
            }
            throw DecodeException("indefinite or reserved length")
        }

        fun validateItem() {
            if (pos >= bytes.size) throw DecodeException("unexpected end of input")
            val startByte = bytes[pos].toInt() and 0xff
            val major = startByte ushr 5
            if (major == 5) throw DecodeException("maps are never canonical in this protocol")
            if (major == 6) throw DecodeException("tags are never canonical in this protocol")
            if (major == 7) {
                val ai = startByte and 0x1f
                if (ai != 20 && ai != 21 && ai != 22) throw DecodeException("unsupported simple/float value")
                pos++
                return
            }
            val (m, value) = head()
            if (m == 0 || m == 1) return
            if (m == 2 || m == 3) {
                val len = value.toInt()
                for (i in 0 until len) byte()
                return
            }
            if (m == 4) {
                val len = value.toInt()
                for (i in 0 until len) validateItem()
                return
            }
            throw DecodeException("unexpected major type $m")
        }

        fun arrayHead(): Int {
            val (major, value) = head()
            if (major != 4) throw DecodeException("expected array")
            return value.toInt()
        }
        fun uintHead(): Long {
            val (major, value) = head()
            if (major != 0) throw DecodeException("expected uint")
            return value
        }
        fun readBytes(): ByteArray {
            val (major, value) = head()
            if (major != 2) throw DecodeException("expected byte string")
            val len = value.toInt()
            val out = bytes.copyOfRange(pos, pos + len); pos += len
            return out
        }
        fun readText(): String {
            val (major, value) = head()
            if (major != 3) throw DecodeException("expected text string")
            val len = value.toInt()
            val out = bytes.copyOfRange(pos, pos + len); pos += len
            return String(out, Charsets.UTF_8)
        }
        fun readBool(): Boolean {
            val b = byte()
            if (b == 0xf5) return true
            if (b == 0xf4) return false
            throw DecodeException("expected bool")
        }
    }

    private fun assertCanonical(bytes: ByteArray) {
        val r = Reader(bytes)
        r.validateItem()
        if (r.remaining() != 0) throw DecodeException("trailing bytes after canonical item")
    }

    private fun decodeCommunityListingCanonical(bytes: ByteArray) {
        val r = Reader(bytes)
        if (r.arrayHead() != 19) throw DecodeException("expected 19-element listing")
        if (r.readText() != communityListingSchema) throw DecodeException("bad schema")
        for (i in 0 until 5) r.readBytes()
        r.uintHead()
        r.readBytes()
        r.uintHead()
        r.uintHead()
        r.readBool()
        r.readText()
        r.readText()
        readSortedSet(r, "byte", "topic_tags")
        readSortedSet(r, "text", "languages")
    }

    private fun readSortedSet(r: Reader, kind: String, name: String) {
        val count = r.arrayHead()
        var previous: ByteArray? = null
        for (i in 0 until count) {
            val element = if (kind == "byte") Writer().bstr(r.readBytes()).out() else Writer().tstr(r.readText()).out()
            val prev = previous
            if (prev != null) {
                val cmp = compareBytes(element, prev)
                if (cmp == 0) throw DecodeException("duplicate $name member")
                if (cmp < 0) throw DecodeException("unsorted $name set")
            }
            previous = element
        }
    }

    // =======================================================================
    // The vector-document verifier.
    // =======================================================================
    private class VerifyReport {
        var records = 0
        var digests = 0
        var signatures = 0
        var hmacInputs = 0
        var alternateGrammar = 0
        val peerRoles = ArrayList<String>()
        var sentinelsChecked = 0
    }

    private fun expectEqualBytes(actual: ByteArray, expectedHex: String, context: String) {
        if (!actual.contentEquals(hexToBytes(expectedHex))) {
            error("$context: expected $expectedHex, got ${bytesToHex(actual)}")
        }
    }

    private fun encBodyForEnvelope(record: String, body: Map<String, Any?>): ByteArray = when (record) {
        "DescriptorEnvelopeV1" -> encAnchorDescriptorBody(body)
        "HostingReceiptV1" -> encHostingReceiptBody(body)
        "ListingReceiptV1" -> encListingReceiptBody(body)
        "WorkChallengeV1" -> encWorkChallengeBody(body)
        "ReplicaSourceAttestationV1" -> encReplicaSourceAttestationBody(body)
        else -> error("no body encoder for envelope $record")
    }

    private fun signatureMessage(record: String, fields: Map<String, Any?>, message: String): ByteArray = when (message) {
        "ticket_core_canonical" -> encPublicSiteTicketCore(D(fields["core"]))
        "grant_canonical" -> encListingDelegateGrant(fields)
        "body_canonical" -> encBodyForEnvelope(record, D(fields["body"]))
        "blake3(body_canonical)" -> Blake3.hash(encBodyForEnvelope(record, D(fields["body"])))
        else -> error("unknown signature message source $message")
    }

    private fun deriveControlDigestBody(fields: Map<String, Any?>): ByteArray {
        require(S(D(fields["semantic"])["kind"]) == "describe") { "only describe control digest body supported" }
        return Writer().arr(3).uint(1).tstr("describe").raw(encDescribeSemanticBody()).out()
    }

    private fun verifyDigest(d: Map<String, Any?>, canonical: ByteArray, v: Map<String, Any?>, report: VerifyReport) {
        when (S(d["algo"])) {
            "digest_v1" -> {
                val message: ByteArray
                if (S(d["message"]) == "canonical") {
                    message = canonical
                } else {
                    message = deriveControlDigestBody(D(v["fields"]))
                    expectEqualBytes(message, S(d["message_hex"]), "control digest body")
                }
                val preimage = digestV1Preimage(S(d["label_ascii"]), message)
                expectEqualBytes(preimage, S(d["preimage_hex"]), "digest ${S(d["name"])} preimage")
                expectEqualBytes(Blake3.hash(preimage), S(d["value_hex"]), "digest ${S(d["name"])} value")
                val flipped = message.copyOf(); flipped[0] = (flipped[0].toInt() xor 0x01).toByte()
                if (Blake3.hash(digestV1Preimage(S(d["label_ascii"]), flipped)).contentEquals(hexToBytes(S(d["value_hex"])))) {
                    error("digest did not change under a one-bit mutation")
                }
                report.digests++
            }
            "operator_key_id" -> {
                val preimage = operatorKeyIdPreimage(canonical)
                expectEqualBytes(preimage, S(d["preimage_hex"]), "operator_key_id preimage")
                expectEqualBytes(Blake3.hash(preimage), S(d["value_hex"]), "operator_key_id value")
                report.digests++
            }
            "anchor_id" -> {
                val inputs = D(d["inputs"])
                val preimage = anchorIdPreimage(BX(inputs["genesis_operator_public_key"]), BX(inputs["genesis_random_256_bits"]))
                expectEqualBytes(preimage, S(d["preimage_hex"]), "anchor_id preimage")
                expectEqualBytes(Blake3.hash(preimage), S(d["value_hex"]), "anchor_id value")
                report.digests++
            }
            "work_proof" -> {
                val inputs = D(d["inputs"])
                val preimage = workProofPreimage(BX(inputs["work_challenge_digest"]), U(inputs["counter"]))
                expectEqualBytes(preimage, S(d["preimage_hex"]), "work_proof preimage")
                expectEqualBytes(Blake3.hash(preimage), S(d["value_hex"]), "work_proof value")
                report.digests++
            }
            "snapshot_cursor_hmac_input" -> {
                val input = snapshotCursorHmacInput(canonical)
                expectEqualBytes(input, S(d["preimage_hex"]), "snapshot_cursor_hmac_input")
                report.hmacInputs++
            }
            else -> error("unknown digest algo ${S(d["algo"])}")
        }
    }

    private fun verifyRecordVector(v: Map<String, Any?>, report: VerifyReport) {
        val canonical = encodeRecord(S(v["record"]), D(v["fields"]))
        expectEqualBytes(canonical, S(v["canonical_hex"]), "record ${S(v["id"])} (${S(v["record"])})")
        report.records++

        for (d in L(v["digests"])) verifyDigest(D(d), canonical, v, report)

        for (sgnAny in L(v["signatures"])) {
            val sgn = D(sgnAny)
            val pk = hexToBytes(S(sgn["public_key_hex"]))
            val message = signatureMessage(S(v["record"]), D(v["fields"]), S(sgn["message"]))
            val preimage = concat(utf8(S(sgn["domain_ascii"])), message)
            expectEqualBytes(preimage, S(sgn["preimage_hex"]), "signature ${S(sgn["name"])} preimage of ${S(v["id"])}")
            val sig = hexToBytes(S(sgn["signature_hex"]))
            if (!ed25519Verify(pk, preimage, sig)) error("signature ${S(sgn["name"])} of ${S(v["id"])} failed to verify")
            val tampered = preimage.copyOf(); tampered[0] = (tampered[0].toInt() xor 0x01).toByte()
            if (ed25519Verify(pk, tampered, sig)) error("tampered signature ${S(sgn["name"])} verified")
            report.signatures++
        }
    }

    private fun verifyDigestVector(d: Map<String, Any?>, report: VerifyReport) {
        when (S(d["algo"])) {
            "digest_v1_over_message" -> {
                val inputs = D(d["inputs"])
                val message = hexToBytes(S(inputs["message_hex"]))
                val preimage = digestV1Preimage(S(d["label_ascii"]), message)
                expectEqualBytes(preimage, S(d["preimage_hex"]), "${S(d["id"])} preimage")
                expectEqualBytes(Blake3.hash(preimage), S(d["value_hex"]), "${S(d["id"])} value")
                report.digests++
            }
            "sync_snapshot" -> {
                val inputs = D(d["inputs"])
                val sortedIds = L(inputs["sorted_entry_ids"]).map { hexToBytes(S(it)) }
                val value = syncSnapshotDigest(BX(inputs["namespace_id"]), U(inputs["entry_count"]), U(inputs["logical_bytes"]), sortedIds)
                expectEqualBytes(value, S(d["value_hex"]), "${S(d["id"])}")
                report.digests++
            }
            "namespace_token_hmac_input" -> {
                val inputs = D(d["inputs"])
                val input = namespaceTokenHmacInput(
                    BX(inputs["operation_id"]), BX(inputs["namespace_id"]),
                    U(inputs["operation_expiry_unix_seconds"]), U(inputs["token_secret_epoch"]).toInt(),
                )
                expectEqualBytes(input, S(d["preimage_hex"]), "${S(d["id"])}")
                report.hmacInputs++
            }
            "peer_proof_preimage" -> {
                val inputs = D(d["inputs"])
                val role = S(inputs["role"])
                val preimage = peerProofPreimage(role, BX(inputs["peer_transcript_digest"]))
                expectEqualBytes(preimage, S(d["preimage_hex"]), "${S(d["id"])} preimage")
                val sig = d["signature"]
                if (sig is Map<*, *>) {
                    val sm = D(sig)
                    if (!ed25519Verify(hexToBytes(S(sm["public_key_hex"])), preimage, hexToBytes(S(sm["signature_hex"])))) {
                        error("${S(d["id"])} peer-proof signature failed to verify")
                    }
                }
                report.peerRoles.add(role)
            }
            else -> error("unknown digest vector algo ${S(d["algo"])}")
        }
    }

    // Sentinel token tables — Kotlin's independent copy, cross-checked against the fixture.
    private val sentinels: Map<String, List<String>> = mapOf(
        "transport_floor" to listOf("require_none", "require_arti"),
        "enabled_role" to listOf("directory", "gossip", "host", "mirror"),
        "control_operation_kind" to listOf(
            "describe", "get_work_challenge", "prepare_host", "commit_host", "submit_listing",
            "prepare_replica", "pull_directory_feed", "pull_directory_snapshot", "get_operation",
        ),
        "hosting_status" to listOf("committed"),
        "control_outcome" to listOf("success", "refused"),
    )

    private fun verifySentinels(doc: Map<String, Any?>, report: VerifyReport) {
        val docSentinels = D(doc["sentinels"])
        for ((name, expected) in sentinels) {
            val actual = L(docSentinels[name]).map { S(it) }
            if (actual != expected) error("sentinel table $name disagrees with the fixture")
            report.sentinelsChecked++
        }
    }

    private fun verifyAlternateGrammar(doc: Map<String, Any?>, report: VerifyReport) {
        for (gAny in L(doc["alternate_grammar"])) {
            val g = D(gAny)
            val hostile = hexToBytes(S(g["hostile_hex"]))
            var rejected = false
            try { assertCanonical(hostile) } catch (_: Exception) { rejected = true }
            if (!rejected && S(g["record"]) == "CommunityListingV1") {
                try { decodeCommunityListingCanonical(hostile) } catch (_: Exception) { rejected = true }
            }
            if (!rejected) error("alternate grammar '${S(g["desc"])}' was not rejected")
            report.alternateGrammar++
        }
    }

    private fun verifyVectorDocument(doc: Map<String, Any?>): VerifyReport {
        if (S(doc["protocol"]) != "riot-anchor-protocol") error("unexpected protocol tag")
        val report = VerifyReport()
        for (v in L(doc["vectors"])) verifyRecordVector(D(v), report)
        for (d in L(doc["digest_vectors"])) verifyDigestVector(D(d), report)
        verifySentinels(doc, report)
        verifyAlternateGrammar(doc, report)
        if (!report.peerRoles.contains("initiator") || !report.peerRoles.contains("responder")) {
            error("peer roles incomplete: ${report.peerRoles.joinToString(",")}")
        }
        return report
    }

    // =======================================================================
    // Minimal dependency-free JSON parser (avoids adding a gradle dependency and
    // sidesteps the host-JVM org.json stub). Numbers are kept as their literal
    // string; the wire integers this test consumes are all quoted strings anyway.
    // =======================================================================
    private class JsonParser(private val s: String) {
        private var i = 0

        fun parse(): Any? {
            skipWs()
            val v = parseValue()
            skipWs()
            if (i != s.length) error("trailing JSON at $i")
            return v
        }

        private fun skipWs() {
            while (i < s.length && s[i].isWhitespace()) i++
        }

        private fun parseValue(): Any? {
            skipWs()
            return when (val c = s[i]) {
                '{' -> parseObject()
                '[' -> parseArray()
                '"' -> parseString()
                't' -> { expect("true"); true }
                'f' -> { expect("false"); false }
                'n' -> { expect("null"); null }
                else -> if (c == '-' || c in '0'..'9') parseNumber() else error("unexpected char '$c' at $i")
            }
        }

        private fun expect(word: String) {
            if (!s.regionMatches(i, word, 0, word.length)) error("expected '$word' at $i")
            i += word.length
        }

        private fun parseObject(): Map<String, Any?> {
            val map = LinkedHashMap<String, Any?>()
            i++ // {
            skipWs()
            if (s[i] == '}') { i++; return map }
            while (true) {
                skipWs()
                val key = parseString()
                skipWs()
                if (s[i] != ':') error("expected ':' at $i")
                i++
                val value = parseValue()
                map[key] = value
                skipWs()
                when (s[i]) {
                    ',' -> { i++; continue }
                    '}' -> { i++; break }
                    else -> error("expected ',' or '}' at $i")
                }
            }
            return map
        }

        private fun parseArray(): List<Any?> {
            val list = ArrayList<Any?>()
            i++ // [
            skipWs()
            if (s[i] == ']') { i++; return list }
            while (true) {
                val value = parseValue()
                list.add(value)
                skipWs()
                when (s[i]) {
                    ',' -> { i++; continue }
                    ']' -> { i++; break }
                    else -> error("expected ',' or ']' at $i")
                }
            }
            return list
        }

        private fun parseString(): String {
            if (s[i] != '"') error("expected string at $i")
            i++
            val sb = StringBuilder()
            while (true) {
                val c = s[i++]
                when (c) {
                    '"' -> break
                    '\\' -> {
                        when (val e = s[i++]) {
                            '"' -> sb.append('"')
                            '\\' -> sb.append('\\')
                            '/' -> sb.append('/')
                            'b' -> sb.append('\b')
                            'f' -> sb.append('\u000C')
                            'n' -> sb.append('\n')
                            'r' -> sb.append('\r')
                            't' -> sb.append('\t')
                            'u' -> {
                                val hex = s.substring(i, i + 4)
                                i += 4
                                sb.append(hex.toInt(16).toChar())
                            }
                            else -> error("bad escape '\\$e' at $i")
                        }
                    }
                    else -> sb.append(c)
                }
            }
            return sb.toString()
        }

        private fun parseNumber(): String {
            val start = i
            if (s[i] == '-') i++
            while (i < s.length && (s[i] in '0'..'9' || s[i] == '.' || s[i] == 'e' || s[i] == 'E' || s[i] == '+' || s[i] == '-')) i++
            return s.substring(start, i)
        }
    }

    private fun loadResourceText(name: String): String {
        val stream = javaClass.getResourceAsStream(name)
            ?: error("$name is not on the unit-test resources classpath")
        return stream.bufferedReader().use { it.readText() }
    }

    private fun loadResourceBytes(name: String): ByteArray {
        val stream = javaClass.getResourceAsStream(name)
            ?: error("$name is not on the unit-test resources classpath")
        return stream.use { it.readBytes() }
    }

    private val document: Map<String, Any?> by lazy {
        D(JsonParser(loadResourceText("/protocol-v1-vectors.json")).parse())
    }

    // =======================================================================
    // Tests.
    // =======================================================================
    private fun referenceInput(n: Int): ByteArray {
        val out = ByteArray(n)
        for (i in 0 until n) out[i] = (i % 251).toByte()
        return out
    }

    @Test
    fun blake3MatchesOfficialEmptyVector() {
        assertEquals(
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262",
            bytesToHex(Blake3.hash(referenceInput(0))),
        )
    }

    @Test
    fun blake3MatchesFixtureKnownAnswerTests() {
        val kats = L(document["blake3_kats"])
        assertTrue("expected >=6 BLAKE3 KATs", kats.size >= 6)
        var sawMultiChunk = false
        for (katAny in kats) {
            val kat = D(katAny)
            val n = U(kat["input_len"]).toInt()
            assertEquals("blake3 of $n bytes", S(kat["hash_hex"]), bytesToHex(Blake3.hash(referenceInput(n))))
            if (n > 1024) sawMultiChunk = true
        }
        assertTrue("BLAKE3 multi-chunk tree path must be exercised", sawMultiChunk)
    }

    @Test
    fun everyVectorIsReproducedAndVerifiedIndependently() {
        val report = verifyVectorDocument(document)
        assertTrue("expected >=25 record vectors, got ${report.records}", report.records >= 25)
        assertTrue("expected >=10 digest checks, got ${report.digests}", report.digests >= 10)
        assertTrue("expected >=6 signature checks, got ${report.signatures}", report.signatures >= 6)
        assertTrue("expected >=2 HMAC-input checks, got ${report.hmacInputs}", report.hmacInputs >= 2)
        assertTrue("expected >=5 alt-grammar rejections, got ${report.alternateGrammar}", report.alternateGrammar >= 5)
        assertEquals(listOf("initiator", "responder"), report.peerRoles.sorted())
        assertTrue(report.sentinelsChecked >= 5)
    }

    @Test
    fun oneBitMutationIsDetected() {
        val v = L(document["vectors"]).map { D(it) }.first { S(it["id"]) == "public_site_ticket_core" }
        val canonical = encodeRecord(S(v["record"]), D(v["fields"]))
        assertEquals(S(v["canonical_hex"]), bytesToHex(canonical))
        val mutated = canonical.copyOf(); mutated[5] = (mutated[5].toInt() xor 0x01).toByte()
        assertNotEquals(bytesToHex(canonical), bytesToHex(mutated))
        assertNotEquals(bytesToHex(mutated), bytesToHex(encodeRecord(S(v["record"]), D(v["fields"]))))
    }

    @Test
    fun alternateGrammarEncodingsAreRejected() {
        assertThrows { assertCanonical(hexToBytes("9f0102ff")) } // indefinite-length array
        assertThrows { assertCanonical(hexToBytes("1801")) }      // non-minimal integer
        assertThrows { assertCanonical(hexToBytes("a10102")) }    // a map
        assertThrows { assertCanonical(hexToBytes("0100")) }      // trailing bytes

        for (gAny in L(document["alternate_grammar"])) {
            val g = D(gAny)
            val hostile = hexToBytes(S(g["hostile_hex"]))
            var rejected = false
            try { assertCanonical(hostile) } catch (_: Exception) { rejected = true }
            if (!rejected && S(g["record"]) == "CommunityListingV1") {
                assertThrows { decodeCommunityListingCanonical(hostile) }
                rejected = true
            }
            assertTrue("alt-grammar '${S(g["desc"])}' should be rejected", rejected)
        }
    }

    @Test
    fun developmentBootstrapResourceParsesButIsNotReleaseEligible() {
        val cbor = loadResourceBytes("/bootstrap-development-v1.cbor")
        val vector = L(document["vectors"]).map { D(it) }.first { S(it["record"]) == "AnchorBootstrapV1" }

        // The checked-in .cbor is byte-identical to the independently re-encoded record.
        assertEquals(S(vector["canonical_hex"]), bytesToHex(cbor))
        assertEquals(bytesToHex(cbor), bytesToHex(encodeRecord("AnchorBootstrapV1", D(vector["fields"]))))

        val descriptors = L(D(vector["fields"])["descriptors"]).map { D(it) }
        assertTrue(descriptors.size >= 3)
        val operatorKeys = descriptors.map { S(D(D(it["floor"])["operator_verification_key"])["public_key"]) }.toSet()
        assertTrue("expected >=2 distinct operators", operatorKeys.size >= 2)

        val allDev = descriptors.all { S(it["https_origin"]).contains(".dev.invalid") }
        val releaseEligible = descriptors.size >= 3 && operatorKeys.size >= 2 && !allDev
        assertFalse("development bootstrap must not be release-eligible", releaseEligible)
        assertTrue(allDev)
    }

    private fun assertThrows(block: () -> Unit) {
        var threw = false
        try { block() } catch (_: Exception) { threw = true }
        assertTrue("expected an exception", threw)
    }
}
