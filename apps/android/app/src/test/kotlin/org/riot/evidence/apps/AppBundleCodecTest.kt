package org.riot.evidence.apps

import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test

class AppBundleCodecTest {
    private fun sample() = DecodedAppBundle(
        entryPoint = "index.html",
        resources = listOf(
            AppResource("app.js", "text/javascript", "riot.watch();".toByteArray()),
            AppResource("index.html", "text/html", "<!doctype html>".toByteArray()),
        ),
    )

    @Test
    fun decodesTheCanonicalEncodingItProduces() {
        val encoded = AppBundleCodec.encode(sample())
        assertEquals(sample(), AppBundleCodec.decode(encoded))
    }

    @Test
    fun rejectsTrailingBytes() {
        val encoded = AppBundleCodec.encode(sample()) + byteArrayOf(0)
        assertThrows(AppBundleCodecException::class.java) { AppBundleCodec.decode(encoded) }
    }

    @Test
    fun rejectsEveryNonCanonicalSingleByteFlipOrDecodesEqual() {
        // Mirror of apps_codec_hostile.rs's byte-flip property: a flipped
        // byte either fails decode or (if it flipped inside resource bytes)
        // decodes to a *different* bundle — never the same bundle from
        // different bytes.
        val encoded = AppBundleCodec.encode(sample())
        for (index in encoded.indices) {
            val mutated = encoded.copyOf().also { it[index] = (it[index].toInt() xor 0x01).toByte() }
            val decoded = runCatching { AppBundleCodec.decode(mutated) }.getOrNull() ?: continue
            assertEquals(mutated.toList(), AppBundleCodec.encode(decoded).toList())
        }
    }

    @Test
    fun rejectsMissingEntryPointResource() {
        val bundle = DecodedAppBundle("missing.html", sample().resources)
        assertThrows(AppBundleCodecException::class.java) { AppBundleCodec.encode(bundle) }
    }

    @Test
    fun rejectsOversizedResourceCountHeaderWithoutAllocating() {
        // map(2), key 0, "a" entry point, key 1, array claiming 2^32 items.
        val forged = byteArrayOf(
            0xA2.toByte(), 0x00, 0x61, 0x61, 0x01,
            0x9A.toByte(), 0xFF.toByte(), 0xFF.toByte(), 0xFF.toByte(), 0xFF.toByte(),
        )
        assertThrows(AppBundleCodecException::class.java) { AppBundleCodec.decode(forged) }
    }
}
