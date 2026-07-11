package org.riot.evidence

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class AppPersistenceTest {
    private val space = PersistedSpace("02".repeat(32), "Aid")
    private val appA = "aa".repeat(32)
    private val appB = "bb".repeat(32)

    @Test
    fun recordInstalledAppAppendsAndCopiesBytes() {
        val manifest = byteArrayOf(1, 2, 3)
        val bundle = byteArrayOf(4, 5, 6)

        val updated = recordInstalledApp(base(), appA, manifest, bundle)

        val stored = updated.installedApps.single()
        assertEquals(appA, stored.appId)
        assertArrayEquals(manifest, stored.manifestBytes)
        assertArrayEquals(bundle, stored.bundleBytes)
        assertFalse(stored.trusted)
        // Defensive copy: mutating the caller's array must not change the record.
        manifest[0] = 9
        assertArrayEquals(byteArrayOf(1, 2, 3), stored.manifestBytes)
    }

    @Test
    fun recordInstalledAppReplacesSameIdAndKeepsTrust() {
        val trusted = base(
            installedApps = listOf(PersistedApp(appA, byteArrayOf(1), byteArrayOf(2), trusted = true)),
        )

        val reinstalled = recordInstalledApp(trusted, appA, byteArrayOf(7), byteArrayOf(8))

        val stored = reinstalled.installedApps.single()
        assertArrayEquals(byteArrayOf(7), stored.manifestBytes)
        assertTrue("re-installing a trusted app keeps its trust", stored.trusted)
    }

    @Test
    fun recordAppTrustFlipsOnlyMatchingApp() {
        val profile = base(
            installedApps = listOf(
                PersistedApp(appA, byteArrayOf(1), byteArrayOf(2), trusted = false),
                PersistedApp(appB, byteArrayOf(3), byteArrayOf(4), trusted = false),
            ),
        )

        val trusted = recordAppTrust(profile, appA)

        assertTrue(trusted.installedApps.first { it.appId == appA }.trusted)
        assertFalse(trusted.installedApps.first { it.appId == appB }.trusted)
    }

    @Test
    fun recordAppDataKeepsLatestPerKey() {
        val first = recordAppData(base(), appA, "items", byteArrayOf(1))
        val second = recordAppData(first, appA, "done", byteArrayOf(2))
        val superseding = recordAppData(second, appA, "items", byteArrayOf(3))

        assertEquals(listOf("done", "items"), superseding.appData.map { it.key })
        assertArrayEquals(byteArrayOf(3), superseding.appData.first { it.key == "items" }.bundleBytes)
        // Same key under a different app is a distinct entry.
        val other = recordAppData(superseding, appB, "items", byteArrayOf(4))
        assertEquals(3, other.appData.size)
    }

    @Test
    fun recordAppDataStaysBoundedByEvictingOldest() {
        val max = PersistedProfileCodec.MAX_APP_DATA_ENTRIES
        var profile = base()
        // One more distinct key than the ceiling allows.
        for (i in 0..max) {
            profile = recordAppData(profile, appA, "key$i", byteArrayOf(i.toByte()))
        }

        assertEquals(max, profile.appData.size)
        // Oldest ("key0") evicted; newest retained.
        assertFalse(profile.appData.any { it.key == "key0" })
        assertTrue(profile.appData.any { it.key == "key$max" })
    }

    @Test
    fun restoreInstallsTrustsAndReplaysInOrderWithoutReputting() {
        val profile = base(
            installedApps = listOf(
                PersistedApp(appA, byteArrayOf(1), byteArrayOf(2), trusted = true),
                PersistedApp(appB, byteArrayOf(3), byteArrayOf(4), trusted = false),
            ),
            appData = listOf(
                PersistedAppData(appA, "items", byteArrayOf(0x10)),
                PersistedAppData(appA, "done", byteArrayOf(0x11)),
            ),
        )
        val port = RecordingRestorePort()

        restoreApps(profile, port)

        // Both apps re-installed; only the trusted one re-trusted.
        assertEquals(listOf(appA, appB), port.installed)
        assertEquals(listOf(appA), port.trusted)
        // App data comes back only via replay of the committed bundles — the
        // port has no `put`, so a re-put is not even expressible.
        assertEquals(2, port.replayed.size)
        assertArrayEquals(byteArrayOf(0x10), port.replayed.first())
        assertArrayEquals(byteArrayOf(0x11), port.replayed[1])
    }

    private fun base(
        installedApps: List<PersistedApp> = emptyList(),
        appData: List<PersistedAppData> = emptyList(),
    ) = PersistedProfile(space, emptyList(), null, installedApps, appData)

    private class RecordingRestorePort : AppRestorePort {
        val installed = mutableListOf<String>()
        val trusted = mutableListOf<String>()
        val replayed = mutableListOf<ByteArray>()

        override fun install(appId: String, manifestBytes: ByteArray, bundleBytes: ByteArray) {
            installed += appId
        }

        override fun trust(appId: String) {
            trusted += appId
        }

        override fun replayAppData(bundleBytes: ByteArray) {
            replayed += bundleBytes
        }
    }
}
