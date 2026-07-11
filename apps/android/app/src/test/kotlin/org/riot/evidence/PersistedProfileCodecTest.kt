package org.riot.evidence

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
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
        )

        val restored = PersistedProfileCodec.decode(PersistedProfileCodec.encode(profile))

        assertEquals(entryId, restored.alerts.single().entryId)
        assertEquals(namespaceId, restored.alerts.single().namespaceId)
        assertEquals(signerId, restored.alerts.single().signerId)
        assertEquals(1_720_000_000L, restored.alerts.single().createdAt)
        assertEquals(1_720_000_010L, restored.alerts.single().validFrom)
        assertEquals(1_720_003_600L, restored.alerts.single().expiresAt)
        assertTrue(restored.alerts.single().aiAssisted)
        assertArrayEquals(profile.alerts.single().bundleBytes, restored.alerts.single().bundleBytes)
    }

    @Test
    fun totalProfileSizeIsBoundedEvenWhenIndividualBundlesAreValid() {
        val profile = PersistedProfile(
            PersistedSpace("02".repeat(32), "Berlin Mutual Aid"),
            List(3) { index ->
                PersistedAlert(
                    entryId = "%02x".format(index).repeat(32),
                    namespaceId = "02".repeat(32),
                    signerId = "03".repeat(32),
                    headline = "Bounded alert $index",
                    createdAt = 1,
                    validFrom = null,
                    expiresAt = 2,
                    aiAssisted = false,
                    bundleBytes = ByteArray(2 * 1024 * 1024),
                )
            },
        )

        assertThrows(IllegalArgumentException::class.java) {
            PersistedProfileCodec.encode(profile)
        }
    }
}
