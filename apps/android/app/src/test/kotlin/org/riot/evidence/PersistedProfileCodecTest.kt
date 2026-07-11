package org.riot.evidence

import java.io.ByteArrayOutputStream
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
        assertEquals(PersistedProfileCodec.encodedSizeForTest(profile), PersistedProfileCodec.encode(profile).size)
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

        assertEquals(PersistedProfileCodec.encodedSizeForTest(profile), encoded.size)
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

    private fun profileWithIdentity() = PersistedProfile(
        PersistedSpace("02".repeat(32), "Bounded identity"),
        emptyList(),
        PersistedIdentityState(ByteArray(32) { 0x22 }, ByteArray(112) { 0x44 }),
    )
}
