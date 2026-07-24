package org.riot.evidence

import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.io.DataInputStream
import java.io.DataOutputStream
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assert.assertThrows
import org.junit.Test

class PersistedProfileCodecTest {
    @Test
    fun roundTripPreservesFullIdentityFreshnessAndAiDisclosure() {
        val entryId = "01".repeat(32)
        val namespaceId = "02".repeat(32)
        val signerId = "03".repeat(32)
        val profile = PersistedProfile(
            space = PersistedSpace(namespaceId, "Berlin Mutual Aid"),
            alerts = listOf(
                PersistedAlert(
                    entryId = entryId,
                    namespaceId = namespaceId,
                    signerId = signerId,
                    headline = "Water available at the courtyard",
                    createdAt = 1_720_000_000,
                    validFrom = 1_720_000_010,
                    expiresAt = 1_720_003_600,
                    aiAssisted = true,
                    bundleBytes = byteArrayOf(0x01, 0x02, 0x7f),
                ),
            ),
            identityState = PersistedIdentityState(ByteArray(32) { 7 }, ByteArray(112) { 9 }),
        )

        val encoded = PersistedProfileCodec.encode(profile)
        val restored = PersistedProfileCodec.decode(encoded)

        assertEquals(entryId, restored.alerts.single().entryId)
        assertEquals(namespaceId, restored.alerts.single().namespaceId)
        assertEquals(signerId, restored.alerts.single().signerId)
        assertEquals(1_720_000_000L, restored.alerts.single().createdAt)
        assertEquals(1_720_000_010L, restored.alerts.single().validFrom)
        assertEquals(1_720_003_600L, restored.alerts.single().expiresAt)
        assertTrue(restored.alerts.single().aiAssisted)
        assertArrayEquals(profile.alerts.single().bundleBytes, restored.alerts.single().bundleBytes)
        assertArrayEquals(ByteArray(32) { 7 }, restored.identityState!!.wrappingKey)
        assertArrayEquals(ByteArray(112) { 9 }, restored.identityState!!.sealedIdentity)
        encoded.fill(0)
        restored.identityState!!.wrappingKey.fill(0)
    }

    @Test
    fun roundTripPreservesInstalledAppsAndAppData() {
        val appId = "ab".repeat(32)
        val profile = PersistedProfile(
            space = PersistedSpace("02".repeat(32), "Berlin Mutual Aid"),
            alerts = emptyList(),
            identityState = null,
            installedApps = listOf(
                PersistedApp(
                    appId = appId,
                    manifestBytes = byteArrayOf(0x10, 0x11, 0x12),
                    bundleBytes = byteArrayOf(0x20, 0x21, 0x7f),
                    trusted = true,
                ),
                PersistedApp(
                    appId = "cd".repeat(32),
                    manifestBytes = byteArrayOf(0x30),
                    bundleBytes = byteArrayOf(0x40, 0x41),
                    trusted = false,
                ),
            ),
            appData = listOf(
                PersistedAppData(appId, "items", byteArrayOf(0x50, 0x51, 0x00, 0x7f)),
                PersistedAppData(appId, "done", byteArrayOf(0x60)),
            ),
        )

        val restored = PersistedProfileCodec.decode(PersistedProfileCodec.encode(profile))

        assertEquals(2, restored.installedApps.size)
        assertEquals(appId, restored.installedApps.first().appId)
        assertArrayEquals(byteArrayOf(0x10, 0x11, 0x12), restored.installedApps.first().manifestBytes)
        assertArrayEquals(byteArrayOf(0x20, 0x21, 0x7f), restored.installedApps.first().bundleBytes)
        assertTrue(restored.installedApps.first().trusted)
        assertFalse(restored.installedApps[1].trusted)
        assertEquals(listOf("items", "done"), restored.appData.map { it.key })
        assertArrayEquals(byteArrayOf(0x50, 0x51, 0x00, 0x7f), restored.appData.first().bundleBytes)
        assertEquals(PersistedProfileCodec.encodedSize(profile), PersistedProfileCodec.encode(profile).size)
    }

    @Test
    fun versionTwoProfileDecodesWithEmptyAppFields() {
        // A v2 snapshot (identity, no app sections) written before Task 5.
        val v2 = ByteArrayOutputStream().use { bytes ->
            DataOutputStream(bytes).use { output ->
                output.writeInt(0x52494f54)
                output.writeInt(2)
                output.writeInt(64)
                output.write("02".repeat(32).toByteArray())
                output.writeInt(11)
                output.write("Aid v2 space".take(11).toByteArray())
                output.writeInt(0)
                output.writeBoolean(true)
                output.write(ByteArray(32) { 7 })
                output.write(ByteArray(112) { 9 })
            }
            bytes.toByteArray()
        }

        val migrated = PersistedProfileCodec.decode(v2)

        assertTrue(migrated.alerts.isEmpty())
        assertTrue(migrated.installedApps.isEmpty())
        assertTrue(migrated.appData.isEmpty())
        assertArrayEquals(ByteArray(32) { 7 }, migrated.identityState!!.wrappingKey)
        migrated.identityState!!.wrappingKey.fill(0)
    }

    @Test
    fun tooManyInstalledAppsIsRejectedBeforeStreamAllocation() {
        val profile = PersistedProfile(
            space = PersistedSpace("02".repeat(32), "Bounded"),
            alerts = emptyList(),
            installedApps = List(PersistedProfileCodec.MAX_INSTALLED_APPS + 1) { index ->
                PersistedApp("%02x".format(index).repeat(32), byteArrayOf(1), byteArrayOf(2), false)
            },
        )

        var streamAllocated = false
        assertThrows(IllegalArgumentException::class.java) {
            PersistedProfileCodec.encodeWithHooksForTest(
                profile,
                onStreamAllocated = { streamAllocated = true },
            )
        }
        assertFalse("oversize app list must reject before stream allocation", streamAllocated)
    }

    @Test
    fun legacyVersionOneProfileMigratesWithoutIdentityState() {
        val legacy = ByteArrayOutputStream().use { bytes ->
            DataOutputStream(bytes).use { output ->
                output.writeInt(0x52494f54)
                output.writeInt(1)
                output.writeInt(64)
                output.write("02".repeat(32).toByteArray())
                output.writeInt(16)
                output.write("Legacy aid space".toByteArray())
                output.writeInt(0)
            }
            bytes.toByteArray()
        }

        val migrated = PersistedProfileCodec.decode(legacy)

        assertEquals("Legacy aid space", migrated.space.title)
        assertTrue(migrated.alerts.isEmpty())
        assertEquals(null, migrated.identityState)
    }

    @Test
    fun totalProfileSizeIsBoundedEvenWhenIndividualBundlesAreValid() {
        val sharedTwoMiBBundle = ByteArray(2 * 1024 * 1024)
        val profile = PersistedProfile(
            PersistedSpace("02".repeat(32), "Berlin Mutual Aid"),
            List(256) { index ->
                PersistedAlert(
                    entryId = "%02x".format(index).repeat(32),
                    namespaceId = "02".repeat(32),
                    signerId = "03".repeat(32),
                    headline = "Bounded alert $index",
                    createdAt = 1,
                    validFrom = null,
                    expiresAt = 2,
                    aiAssisted = false,
                    bundleBytes = sharedTwoMiBBundle,
                )
            },
        )

        var streamAllocated = false
        assertThrows(IllegalArgumentException::class.java) {
            PersistedProfileCodec.encodeWithHooksForTest(
                profile,
                onStreamAllocated = { streamAllocated = true },
            )
        }
        assertFalse("oversize must reject before stream allocation", streamAllocated)
    }

    @Test
    fun exactPreflightMatchesEncodedBytes() {
        val profile = profileWithIdentity()

        val encoded = PersistedProfileCodec.encode(profile)

        assertEquals(PersistedProfileCodec.encodedSize(profile), encoded.size)
        encoded.fill(0)
        profile.identityState!!.wrappingKey.fill(0)
    }

    @Test
    fun returnedPlaintextIsWipedWhenPostCopyStepFails() {
        val profile = profileWithIdentity()
        lateinit var returnedPlaintext: ByteArray

        assertThrows(IllegalStateException::class.java) {
            PersistedProfileCodec.encodeWithHooksForTest(
                profile,
                afterCopy = {
                    returnedPlaintext = it
                    throw IllegalStateException("injected post-copy failure")
                },
            )
        }

        assertArrayEquals(ByteArray(returnedPlaintext.size), returnedPlaintext)
        profile.identityState!!.wrappingKey.fill(0)
    }

    @Test
    fun decodedWrappingKeyIsWipedOnTruncatedSealedIdentity() {
        val profile = profileWithIdentity()
        val encoded = PersistedProfileCodec.encode(profile)
        lateinit var cleared: ByteArray

        assertThrows(IllegalArgumentException::class.java) {
            PersistedProfileCodec.decodeWithIdentityKeyObserverForTest(
                encoded.copyOf(encoded.size - 1),
                onRejectedIdentityKey = { cleared = it },
            )
        }

        assertArrayEquals(ByteArray(PersistedProfileCodec.WRAPPING_KEY_BYTES), cleared)
        encoded.fill(0)
        profile.identityState!!.wrappingKey.fill(0)
    }

    @Test
    fun partiallyReadWrappingKeyIsWipedOnEof() {
        val profile = profileWithIdentity()
        val encoded = PersistedProfileCodec.encode(profile)
        val truncatedMidKey = encoded.copyOf(encoded.size - PersistedProfileCodec.SEALED_IDENTITY_BYTES - 16)
        lateinit var cleared: ByteArray

        assertThrows(IllegalArgumentException::class.java) {
            PersistedProfileCodec.decodeWithIdentityKeyObserverForTest(
                truncatedMidKey,
                onRejectedIdentityKey = { cleared = it },
            )
        }

        assertArrayEquals(ByteArray(PersistedProfileCodec.WRAPPING_KEY_BYTES), cleared)
        encoded.fill(0)
        truncatedMidKey.fill(0)
        profile.identityState!!.wrappingKey.fill(0)
    }

    @Test
    fun decodedWrappingKeyIsWipedOnTrailingBytes() {
        val profile = profileWithIdentity()
        val encoded = PersistedProfileCodec.encode(profile)
        val withTrailing = encoded + 0x7f
        lateinit var cleared: ByteArray

        assertThrows(IllegalArgumentException::class.java) {
            PersistedProfileCodec.decodeWithIdentityKeyObserverForTest(
                withTrailing,
                onRejectedIdentityKey = { cleared = it },
            )
        }

        assertArrayEquals(ByteArray(PersistedProfileCodec.WRAPPING_KEY_BYTES), cleared)
        encoded.fill(0)
        withTrailing.fill(0)
        profile.identityState!!.wrappingKey.fill(0)
    }

    // MARK: - Starter-catalog generation (WU-001N)

    @Test
    fun generationTwoRoundTripsAsVersionFour() {
        val profile = generationProfile(2)

        val encoded = PersistedProfileCodec.encode(profile)
        val restored = PersistedProfileCodec.decode(encoded)

        assertEquals(4, headerVersion(encoded))
        assertEquals(2, restored.starterCatalogGeneration)
    }

    @Test
    fun explicitGenerationOneRoundTripsAsVersionFour() {
        val profile = generationProfile(1)

        val encoded = PersistedProfileCodec.encode(profile)
        val restored = PersistedProfileCodec.decode(encoded)

        assertEquals(4, headerVersion(encoded))
        assertEquals(1, restored.starterCatalogGeneration)
    }

    @Test
    fun nullGenerationEncodesAsVersionThree() {
        val profile = generationProfile(null)

        val encoded = PersistedProfileCodec.encode(profile)
        val restored = PersistedProfileCodec.decode(encoded)

        assertEquals(3, headerVersion(encoded))
        assertEquals(null, restored.starterCatalogGeneration)
    }

    @Test
    fun nullGenerationV3ProfileReEncodesByteForByteIdentically() {
        val profile = generationProfile(null)

        val first = PersistedProfileCodec.encode(profile)
        val second = PersistedProfileCodec.encode(PersistedProfileCodec.decode(first))

        assertEquals(3, headerVersion(first))
        assertArrayEquals(first, second)
    }

    @Test
    fun invalidGenerationIsRejectedBeforeStreamAllocation() {
        for (invalid in listOf(0, 3)) {
            val profile = generationProfile(invalid)
            var streamAllocated = false
            assertThrows(IllegalArgumentException::class.java) {
                PersistedProfileCodec.encodeWithHooksForTest(
                    profile,
                    onStreamAllocated = { streamAllocated = true },
                )
            }
            assertFalse(
                "invalid generation $invalid must reject before stream allocation",
                streamAllocated,
            )
            // The shared preflight is the production seam WU-002c consumes; it must
            // reject the same input directly, not only through encode().
            assertThrows(IllegalArgumentException::class.java) {
                PersistedProfileCodec.encodedSize(profile)
            }
        }
    }

    @Test
    fun malformedVersionFourStreamWithInvalidGenerationIsRejectedOnDecode() {
        for (invalid in listOf(0, 3)) {
            assertThrows(IllegalArgumentException::class.java) {
                PersistedProfileCodec.decode(versionFourStreamWithGeneration(invalid))
            }
        }
        // Sanity: a well-formed v4 stream with a legal generation decodes.
        assertEquals(2, PersistedProfileCodec.decode(versionFourStreamWithGeneration(2)).starterCatalogGeneration)
    }

    @Test
    fun preflightEqualsEncodedSizeForBothWireVersions() {
        val v3 = generationProfile(null)
        val v4 = generationProfile(2)

        assertEquals(PersistedProfileCodec.encodedSize(v3), PersistedProfileCodec.encode(v3).size)
        assertEquals(PersistedProfileCodec.encodedSize(v4), PersistedProfileCodec.encode(v4).size)
    }

    @Test
    fun v3ProfileAtExactLimitEncodesAndOnePastIsRejectedBeforeAllocation() {
        val exact = exactLimitProfile(null)
        assertEquals(PersistedProfileCodec.MAX_ENCODED_BYTES, PersistedProfileCodec.encodedSize(exact))

        val encoded = PersistedProfileCodec.encode(exact)
        assertEquals(PersistedProfileCodec.MAX_ENCODED_BYTES, encoded.size)
        assertEquals(3, headerVersion(encoded))
        encoded.fill(0)

        val oversize = oneBytePastLimitProfile(null)
        var streamAllocated = false
        assertThrows(IllegalArgumentException::class.java) {
            PersistedProfileCodec.encodeWithHooksForTest(
                oversize,
                onStreamAllocated = { streamAllocated = true },
            )
        }
        assertFalse("v3 one-past-limit must reject before stream allocation", streamAllocated)
        assertThrows(IllegalArgumentException::class.java) {
            PersistedProfileCodec.encodedSize(oversize)
        }
    }

    @Test
    fun v4ProfileAtExactLimitEncodesAndOnePastIsRejectedBeforeAllocation() {
        val exact = exactLimitProfile(2)
        assertEquals(PersistedProfileCodec.MAX_ENCODED_BYTES, PersistedProfileCodec.encodedSize(exact))

        val encoded = PersistedProfileCodec.encode(exact)
        assertEquals(PersistedProfileCodec.MAX_ENCODED_BYTES, encoded.size)
        assertEquals(4, headerVersion(encoded))
        encoded.fill(0)

        val oversize = oneBytePastLimitProfile(2)
        var streamAllocated = false
        assertThrows(IllegalArgumentException::class.java) {
            PersistedProfileCodec.encodeWithHooksForTest(
                oversize,
                onStreamAllocated = { streamAllocated = true },
            )
        }
        assertFalse("v4 one-past-limit must reject before stream allocation", streamAllocated)
        assertThrows(IllegalArgumentException::class.java) {
            PersistedProfileCodec.encodedSize(oversize)
        }
    }

    private fun profileWithIdentity() = PersistedProfile(
        PersistedSpace("02".repeat(32), "Bounded identity"),
        emptyList(),
        PersistedIdentityState(ByteArray(32) { 0x22 }, ByteArray(112) { 0x44 }),
    )

    /** The header version an encoded profile declares (second int after MAGIC). */
    private fun headerVersion(encoded: ByteArray): Int =
        DataInputStream(ByteArrayInputStream(encoded)).use { input ->
            input.readInt() // magic
            input.readInt() // version
        }

    private fun generationProfile(generation: Int?) = PersistedProfile(
        space = PersistedSpace("02".repeat(32), "Generation space"),
        alerts = emptyList(),
        identityState = null,
        installedApps = emptyList(),
        appData = emptyList(),
        starterCatalogGeneration = generation,
    )

    /**
     * A boundary profile carrying two legal app-data bundles (so no single
     * length-prefixed field exceeds its 2 MiB limit) plus the optional
     * generation marker. Payload byte counts are the only tuned variable, so a
     * caller can hit an exact total via [PersistedProfileCodec.encodedSize].
     */
    private fun boundaryProfile(payload1: Int, payload2: Int, generation: Int?) = PersistedProfile(
        space = PersistedSpace("02".repeat(32), "Boundary"),
        alerts = emptyList(),
        identityState = null,
        installedApps = emptyList(),
        appData = listOf(
            PersistedAppData("aa".repeat(32), "one", ByteArray(payload1)),
            PersistedAppData("bb".repeat(32), "two", ByteArray(payload2)),
        ),
        starterCatalogGeneration = generation,
    )

    /** A profile whose exact encoded size equals the codec's hard byte ceiling. */
    private fun exactLimitProfile(generation: Int?): PersistedProfile {
        val overhead = PersistedProfileCodec.encodedSize(boundaryProfile(0, 0, generation))
        val budget = PersistedProfileCodec.MAX_ENCODED_BYTES - overhead
        val first = budget / 2
        return boundaryProfile(first, budget - first, generation)
    }

    /** The same shape as [exactLimitProfile] but exactly one byte over the ceiling. */
    private fun oneBytePastLimitProfile(generation: Int?): PersistedProfile {
        val overhead = PersistedProfileCodec.encodedSize(boundaryProfile(0, 0, generation))
        val budget = PersistedProfileCodec.MAX_ENCODED_BYTES - overhead + 1
        val first = budget / 2
        return boundaryProfile(first, budget - first, generation)
    }

    /**
     * A hand-crafted v4 stream (magic, version 4, minimal body, trailing 32-bit
     * generation) so the decoder's generation-membership check can be exercised
     * independently of the encoder.
     */
    private fun versionFourStreamWithGeneration(generation: Int): ByteArray =
        ByteArrayOutputStream().use { bytes ->
            DataOutputStream(bytes).use { output ->
                output.writeInt(0x52494f54) // magic
                output.writeInt(4) // version 4
                output.writeInt(64)
                output.write("02".repeat(32).toByteArray())
                output.writeInt(8)
                output.write("Boundary".toByteArray())
                output.writeInt(0) // alerts
                output.writeBoolean(false) // identity
                output.writeInt(0) // installed apps
                output.writeInt(0) // app data
                output.writeInt(generation) // trailing generation marker
            }
            bytes.toByteArray()
        }
}
