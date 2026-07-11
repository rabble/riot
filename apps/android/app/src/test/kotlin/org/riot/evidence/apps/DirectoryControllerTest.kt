package org.riot.evidence.apps

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.riot_ffi.DirectoryListing
import uniffi.riot_ffi.InstalledAppRecord
import uniffi.riot_ffi.PublicSpace

private fun listing(
    appId: ByteArray,
    trustedInSpaces: List<ByteArray> = emptyList(),
    bundlePresent: Boolean = true,
    builtIn: Boolean = false,
    installed: Boolean = false,
    endorsingMet: List<ByteArray> = emptyList(),
    endorsingUnmet: UInt = 0u,
) = DirectoryListing(
    appId = appId,
    name = "Checklist",
    description = "Keep a shared checklist.",
    version = "1.0.0",
    authorSigningKeyId = ByteArray(32),
    permissions = listOf("Keep its own notes in this space"),
    bundlePresent = bundlePresent,
    builtIn = builtIn,
    installed = installed,
    carrierSubspaceId = null,
    trustedInSpaces = trustedInSpaces,
    endorsingMetSubspaces = endorsingMet,
    endorsingUnmetCount = endorsingUnmet,
    supersededBy = null,
)

private fun sampleApp(idHex: String) = InstalledApp(
    InstalledAppRecord(
        appId = idHex, appIdBytes = hexToBytes(idHex), name = "Checklist", description = "d",
        version = "1.0.0", entryPoint = "index.html",
        permissions = listOf("Keep its own notes in this space"),
    ),
    DecodedAppBundle("index.html", listOf(AppResource("index.html", "text/html", ByteArray(1)))),
)

private fun hexToBytes(hex: String) = ByteArray(hex.length / 2) {
    hex.substring(it * 2, it * 2 + 2).toInt(16).toByte()
}

private class FakePort(private val rows: List<DirectoryListing> = emptyList()) : DirectoryPort {
    val endorsed = mutableListOf<Triple<ByteArray, String, Boolean>>()
    val shared = mutableListOf<Pair<ByteArray, PublicSpace>>()
    override fun listings(): List<DirectoryListing> = rows
    override fun endorse(appId: ByteArray, note: String, retract: Boolean) {
        endorsed += Triple(appId, note, retract)
    }
    override fun share(appId: ByteArray, space: PublicSpace) {
        shared += appId to space
    }
}

private class FakeInstalledApps : InstalledAppsAccess {
    var installCount = 0
    val installedBytes = mutableListOf<Pair<ByteArray, ByteArray>>()
    val byHex = mutableMapOf<String, InstalledApp>()

    /** Ids passed to [getFromDirectory], in order. */
    val gotFromDirectory = mutableListOf<ByteArray>()

    /** When set, [getFromDirectory] fails the way a rejecting Rust side would. */
    var directoryFailure: Exception? = null

    override fun install(manifestBytes: ByteArray, bundleBytes: ByteArray): InstalledApp {
        installCount++
        installedBytes += manifestBytes to bundleBytes
        return sampleApp("00".repeat(32))
    }
    override fun find(appIdHex: String): InstalledApp? = byHex[appIdHex]
    override fun getFromDirectory(appIdBytes: ByteArray): InstalledApp {
        gotFromDirectory += appIdBytes
        directoryFailure?.let { throw it }
        return sampleApp(DirectoryController.hex(appIdBytes))
    }
}

private class FakeStarter(private val bytes: Pair<ByteArray, ByteArray>?) : StarterCatalog {
    var reads = 0
    override fun read(): Pair<ByteArray, ByteArray>? {
        reads++
        return bytes
    }
}

class DirectoryControllerTest {
    @Test
    fun starterInstallsExactlyOnceAcrossManyRenders() {
        val installed = FakeInstalledApps()
        val starter = FakeStarter(byteArrayOf(1) to byteArrayOf(2))
        val controller = DirectoryController(FakePort(), installed, starter)

        controller.ensureStarterInstalled()
        controller.ensureStarterInstalled()
        controller.ensureStarterInstalled()

        assertEquals(1, installed.installCount)
    }

    @Test
    fun unreadableStarterAssetsNeitherInstallNorThrow() {
        val installed = FakeInstalledApps()
        val controller = DirectoryController(FakePort(), installed, FakeStarter(null))

        controller.ensureStarterInstalled()
        controller.ensureStarterInstalled()

        assertEquals(0, installed.installCount)
    }

    @Test
    fun installedForMatchesByLowercaseHexOfAppIdBytes() {
        val installed = FakeInstalledApps()
        val app = sampleApp("abcd")
        installed.byHex["abcd"] = app
        val controller = DirectoryController(FakePort(), installed, FakeStarter(null))

        val match = controller.installedFor(listing(byteArrayOf(0xAB.toByte(), 0xCD.toByte())))
        assertSame(app, match)
        assertNull(controller.installedFor(listing(byteArrayOf(0x00, 0x01))))
    }

    @Test
    fun aCarriedAppWhosePagesHaveArrivedCanBeTakenUpAndThenOpened() {
        val installed = FakeInstalledApps()
        val controller = DirectoryController(FakePort(), installed, FakeStarter(null))
        val id = ByteArray(32) { 0x5A }
        val row = listing(id, bundlePresent = true)

        // Not held yet, but all here: the row offers a way in.
        assertTrue(controller.canGet(row))

        val app = controller.get(row)

        // Rust was asked for this exact app by its raw id, and what came back
        // is a real installed tool with a bundle to serve — the app is now
        // openable once an organizer reviews it.
        assertTrue(installed.gotFromDirectory.single().contentEquals(id))
        assertEquals("5a".repeat(32), app.record.appId)
        assertEquals("index.html", app.bundle.entryPoint)
    }

    @Test
    fun anAppStillInFlightOffersNoWayIn() {
        val installed = FakeInstalledApps()
        val controller = DirectoryController(FakePort(), installed, FakeStarter(null))

        // The listing has arrived but its pages have not.
        assertFalse(controller.canGet(listing(ByteArray(32), bundlePresent = false)))
        // Nor is there anything to take up for an app already held.
        installed.byHex["ab".repeat(32)] = sampleApp("ab".repeat(32))
        assertFalse(controller.canGet(listing(ByteArray(32) { 0xAB.toByte() })))
        assertTrue(installed.gotFromDirectory.isEmpty())
    }

    @Test
    fun aRejectedGetSurfacesInPlainLanguage() {
        val installed = FakeInstalledApps()
        installed.directoryFailure = IllegalStateException("AppRejected")
        val controller = DirectoryController(FakePort(), installed, FakeStarter(null))

        val error = try {
            controller.get(listing(ByteArray(32), bundlePresent = true))
            null
        } catch (failure: IllegalStateException) {
            failure
        }

        val message = requireNotNull(error).message.orEmpty()
        assertEquals(
            "Checklist hasn't finished arriving from your group. " +
                "Try again the next time you're together.",
            message,
        )
        // No jargon leaks out of the FFI, and the cause is kept for the log.
        assertFalse(message.contains("AppRejected"))
        assertEquals("AppRejected", error.cause?.message)
    }

    @Test
    fun trustedInCurrentSpaceComparesRawIdBytesToTheSpacesHex() {
        val controller = DirectoryController(FakePort(), FakeInstalledApps(), FakeStarter(null))
        val nsBytes = ByteArray(32) { 0x11 }
        val space = PublicSpace(namespaceId = "11".repeat(32), title = "Berlin", isPublic = true)

        assertTrue(controller.trustedInCurrentSpace(listing(byteArrayOf(1), listOf(nsBytes)), space))
        assertFalse(
            controller.trustedInCurrentSpace(
                listing(byteArrayOf(1), listOf(ByteArray(32) { 0x22 })),
                space,
            ),
        )
        assertFalse(controller.trustedInCurrentSpace(listing(byteArrayOf(1), listOf(nsBytes)), null))
    }

    @Test
    fun recommendIsOfferedOnlyWhenTrustedInCurrentSpace() {
        val controller = DirectoryController(FakePort(), FakeInstalledApps(), FakeStarter(null))
        val nsBytes = ByteArray(32) { 0x11 }
        val space = PublicSpace(namespaceId = "11".repeat(32), title = "Berlin", isPublic = true)

        assertTrue(controller.canRecommend(listing(byteArrayOf(1), listOf(nsBytes)), space))
        assertFalse(controller.canRecommend(listing(byteArrayOf(1)), space))
        assertFalse(controller.canRecommend(listing(byteArrayOf(1), listOf(nsBytes)), null))
    }

    @Test
    fun recommendEndorsesWithoutRetracting() {
        val port = FakePort()
        val controller = DirectoryController(port, FakeInstalledApps(), FakeStarter(null))
        val id = byteArrayOf(9, 9)

        controller.recommend(listing(id), "we use this every action")

        assertEquals(1, port.endorsed.size)
        val (appId, note, retract) = port.endorsed.single()
        assertTrue(appId.contentEquals(id))
        assertEquals("we use this every action", note)
        assertFalse(retract)
    }

    @Test
    fun shareDelegatesAppIdAndSpaceToThePort() {
        val port = FakePort()
        val controller = DirectoryController(port, FakeInstalledApps(), FakeStarter(null))
        val id = byteArrayOf(7, 7)
        val space = PublicSpace(namespaceId = "22".repeat(32), title = "Jail Support", isPublic = true)

        controller.share(listing(id), space)

        assertEquals(1, port.shared.size)
        val (appId, target) = port.shared.single()
        assertTrue(appId.contentEquals(id))
        assertSame(space, target)
    }
}
