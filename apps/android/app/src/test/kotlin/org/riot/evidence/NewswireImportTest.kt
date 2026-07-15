package org.riot.evidence

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.riot_ffi.NewswireShareReference

/**
 * Unit 1E — newswire merge & share, Android half.
 *
 * The committed cross-platform golden vector (`newswire-golden-1.json`, bundled
 * into this module's unit-test resources — the same bytes Rust and iOS assert
 * against) is the byte-identity anchor.
 *
 * Honesty note on what this UNIT test can and cannot prove. The app module's
 * `testDebugUnitTest` runs on a host JVM with only device-ABI `.so`s on
 * `jniLibs`, so it CANNOT load `libriot_ffi` — no unit test in this repo calls a
 * native FFI function, only the generated record/enum types. So this test proves
 * two things without native:
 *   1. Android reproduces the deterministic share-reference STRING format from
 *      the same coordinates, matching the committed `encoded` exactly.
 *   2. The generated `NewswireShareReference` FFI record is constructible on
 *      Kotlin from those coordinates (the record shape crossed the binding).
 * The descriptor's canonical-CBOR/WILLIAM3 byte-identity is proven Rust<->iOS
 * (both run the real native encoder against this same fixture); reproducing it on
 * Android would require running the encoder on a device (instrumentation), which
 * is out of scope for a unit test.
 */
class NewswireImportTest {
    private val golden: String by lazy {
        val stream = javaClass.getResourceAsStream("/newswire-golden-1.json")
            ?: error("newswire-golden-1.json is not on the unit-test resources classpath")
        stream.bufferedReader().use { it.readText() }
    }

    /** First string value for `key` in the fixture. Coordinates shared between the
     *  descriptor and share_reference blocks are equal by construction, so the
     *  first match is authoritative. Avoids a JSON dependency the unit-test
     *  classpath does not carry. */
    private fun field(key: String): String {
        val pattern = "\"" + Regex.escape(key) + "\"\\s*:\\s*\"([^\"]*)\""
        return Regex(pattern).find(golden)?.groupValues?.get(1)
            ?: error("missing field $key in golden fixture")
    }

    private val prefix = "riot://newswire/join/v1/"

    @Test
    fun goldenShareReferenceStringIsReproducibleOnAndroid() {
        val namespaceId = field("namespace_id_hex")
        val entryId = field("descriptor_entry_id_hex")
        val digest = field("content_digest_hex")
        val encoded = field("encoded")

        assertEquals("$prefix$namespaceId/$entryId/$digest", encoded)
    }

    @Test
    fun generatedShareReferenceRecordIsConstructibleFromGoldenCoordinates() {
        val reference = NewswireShareReference(
            namespaceId = field("namespace_id_hex"),
            descriptorEntryId = field("descriptor_entry_id_hex"),
            contentDigest = field("content_digest_hex"),
            encoded = field("encoded"),
        )
        assertEquals(
            "$prefix${reference.namespaceId}/${reference.descriptorEntryId}/${reference.contentDigest}",
            reference.encoded,
        )
        assertTrue(reference.encoded.startsWith(prefix))
    }

    @Test
    fun encodedReferenceDecodesToItsThreeHexCoordinates() {
        val encoded = field("encoded")
        val body = encoded.removePrefix(prefix)
        val parts = body.split("/")

        assertEquals(3, parts.size)
        parts.forEach { part ->
            assertEquals(64, part.length)
            assertTrue(part.all { it.isDigit() || it in 'a'..'f' })
        }
        assertEquals(field("namespace_id_hex"), parts[0])
        assertEquals(field("descriptor_entry_id_hex"), parts[1])
        assertEquals(field("content_digest_hex"), parts[2])
    }
}
