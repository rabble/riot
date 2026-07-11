package org.riot.evidence.apps

import java.io.ByteArrayOutputStream

/**
 * Minimal canonical CBOR primitives — the exact subset used by the app
 * bundle format. Definite lengths only, minimal-length heads, strict
 * reads. This is a serving mirror of `crates/riot-core/src/apps/bundle.rs`,
 * not a general CBOR library.
 */
internal object CborMajor {
    const val UINT = 0
    const val BYTES = 2
    const val TEXT = 3
    const val ARRAY = 4
    const val MAP = 5
}

/** Emits definite-length, minimal-head CBOR items. */
internal class CborWriter {
    private val out = ByteArrayOutputStream()

    fun head(major: Int, value: Long): CborWriter {
        val mt = major shl 5
        when {
            value < 24L -> out.write(mt or value.toInt())
            value < 0x100L -> {
                out.write(mt or 24)
                writeBigEndian(value, 1)
            }
            value < 0x10000L -> {
                out.write(mt or 25)
                writeBigEndian(value, 2)
            }
            value < 0x100000000L -> {
                out.write(mt or 26)
                writeBigEndian(value, 4)
            }
            else -> {
                out.write(mt or 27)
                writeBigEndian(value, 8)
            }
        }
        return this
    }

    private fun writeBigEndian(value: Long, bytes: Int) {
        for (shift in (bytes - 1) downTo 0) {
            out.write(((value ushr (shift * 8)) and 0xFF).toInt())
        }
    }

    fun uint(value: Long): CborWriter = head(CborMajor.UINT, value)

    fun map(entries: Int): CborWriter = head(CborMajor.MAP, entries.toLong())

    fun array(items: Int): CborWriter = head(CborMajor.ARRAY, items.toLong())

    fun text(value: String): CborWriter {
        val encoded = value.toByteArray(Charsets.UTF_8)
        head(CborMajor.TEXT, encoded.size.toLong())
        out.write(encoded)
        return this
    }

    fun bytes(value: ByteArray): CborWriter {
        head(CborMajor.BYTES, value.size.toLong())
        out.write(value)
        return this
    }

    fun toByteArray(): ByteArray = out.toByteArray()
}

/**
 * Strict cursor reader. Rejects indefinite/reserved length forms and reads
 * bounds *before* consuming input sized from them, so a forged length can
 * never trigger an oversized allocation. Non-minimal or otherwise
 * non-canonical encodings survive the read but are rejected by the codec's
 * re-encode proof.
 */
internal class CborReader(private val input: ByteArray) {
    var position: Int = 0
        private set

    val size: Int get() = input.size

    private fun readByte(): Int {
        if (position >= input.size) {
            throw AppBundleCodecException("unexpected end of CBOR input")
        }
        return input[position++].toInt() and 0xFF
    }

    private fun readUIntBytes(count: Int): Long {
        var value = 0L
        repeat(count) { value = (value shl 8) or readByte().toLong() }
        return value
    }

    /** Reads one item head; rejects indefinite (31) and reserved (28-30). */
    private fun readHead(): Pair<Int, Long> {
        val initial = readByte()
        val major = initial ushr 5
        val info = initial and 0x1F
        val argument = when {
            info < 24 -> info.toLong()
            info == 24 -> readUIntBytes(1)
            info == 25 -> readUIntBytes(2)
            info == 26 -> readUIntBytes(4)
            info == 27 -> readUIntBytes(8)
            else -> throw AppBundleCodecException("indefinite or reserved CBOR length")
        }
        // 8-byte forms with the top bit set overflow a signed Long; treat as
        // out of range so downstream bounds checks stay meaningful.
        if (argument < 0L) {
            throw AppBundleCodecException("CBOR argument out of range")
        }
        return major to argument
    }

    fun expectMap(entries: Int) {
        val (major, argument) = readHead()
        if (major != CborMajor.MAP || argument != entries.toLong()) {
            throw AppBundleCodecException("expected map($entries)")
        }
    }

    fun readArrayHeader(): Long {
        val (major, argument) = readHead()
        if (major != CborMajor.ARRAY) {
            throw AppBundleCodecException("expected array")
        }
        return argument
    }

    fun expectUInt(value: Long) {
        val (major, argument) = readHead()
        if (major != CborMajor.UINT || argument != value) {
            throw AppBundleCodecException("expected uint $value")
        }
    }

    fun readText(maxBytes: Int): String {
        val (major, argument) = readHead()
        if (major != CborMajor.TEXT) {
            throw AppBundleCodecException("expected text")
        }
        if (argument == 0L || argument > maxBytes.toLong()) {
            throw AppBundleCodecException("text length out of bounds")
        }
        val length = argument.toInt()
        if (position + length > input.size) {
            throw AppBundleCodecException("text overruns CBOR input")
        }
        val text = String(input, position, length, Charsets.UTF_8)
        position += length
        return text
    }

    fun readBytes(maxBytes: Int): ByteArray {
        val (major, argument) = readHead()
        if (major != CborMajor.BYTES) {
            throw AppBundleCodecException("expected bytes")
        }
        if (argument > maxBytes.toLong()) {
            throw AppBundleCodecException("byte string length out of bounds")
        }
        val length = argument.toInt()
        if (position + length > input.size) {
            throw AppBundleCodecException("byte string overruns CBOR input")
        }
        val value = input.copyOfRange(position, position + length)
        position += length
        return value
    }
}
