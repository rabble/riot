package org.riot.evidence.apps

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.riot_ffi.AppDataItem
import uniffi.riot_ffi.AppPairBytes
import uniffi.riot_ffi.AppRuntimeSessionInterface
import uniffi.riot_ffi.DirectoryListing
import uniffi.riot_ffi.InstalledAppRecord
import uniffi.riot_ffi.PublicSpace

/**
 * The Rust app-runtime session, faked. Only the calls the controller makes are
 * given behaviour; the rest exist because the interface declares them.
 *
 * Trust is held as real state rather than a call log, so a revoke that never
 * reached Rust cannot pass by having merely been "called".
 */
private class FakeAppRuntimeSession(
    private val organizer: Boolean = true,
) : AppRuntimeSessionInterface {
    val trusted = mutableSetOf<String>()

    /** Every trust/untrust in order, so the Rust-first ordering can be asserted. */
    val calls = mutableListOf<String>()

    override fun `isOrganizer`(): Boolean = organizer

    override fun `isAppTrusted`(appId: String): Boolean = appId in trusted

    override fun `trustApp`(appId: String) {
        calls += "trust:$appId"
        trusted += appId
    }

    override fun `untrustApp`(appId: String) {
        calls += "untrust:$appId"
        trusted -= appId
    }

    override fun `installApp`(manifestBytes: ByteArray, bundleBytes: ByteArray): InstalledAppRecord =
        throw UnsupportedOperationException("not exercised here")

    override fun `installFromDirectory`(appId: ByteArray): InstalledAppRecord =
        throw UnsupportedOperationException("not exercised here")

    override fun `appPairBytes`(appId: ByteArray): AppPairBytes =
        throw UnsupportedOperationException("not exercised here")

    override fun `appDataGet`(appId: String, key: String): ByteArray? = null
    override fun `appDataList`(appId: String, prefix: String): List<AppDataItem> = emptyList()
    override fun `appDataPut`(appId: String, key: String, value: ByteArray) = Unit
    override fun `appDataPutWithReceipt`(appId: String, key: String, value: ByteArray): ByteArray =
        ByteArray(0)

    override fun `appDisplayName`(): String = "member-00000000"
    override fun `canOrganize`(): Boolean = true
    override fun `directoryListings`(): List<DirectoryListing> = emptyList()
    override fun `endorseApp`(appId: ByteArray, note: String, retract: Boolean) = Unit
    override fun `replayAppDataBundle`(bytes: ByteArray) = Unit
    override fun `retractEndorsement`(appId: ByteArray) = Unit
    override fun `shareApp`(appId: ByteArray, space: PublicSpace) = Unit
}

private fun app(idHex: String = "5a".repeat(32)) = InstalledApp(
    InstalledAppRecord(
        appId = idHex,
        appIdBytes = ByteArray(32) { 0x5A },
        name = "Checklist",
        description = "Keep a shared checklist.",
        version = "1.0.0",
        entryPoint = "index.html",
        permissions = listOf("Keep its own notes in this space"),
    ),
    DecodedAppBundle("index.html", listOf(AppResource("index.html", "text/html", ByteArray(1)))),
)

class RiotAppsControllerTest {
    /**
     * Turning an app off has to reach BOTH sides: Rust (which gates the launch)
     * and the persistence hook (because Rust's trust is in-memory, so `restore()`
     * would re-trust the app on the next launch if the decision never landed).
     */
    @Test
    fun untrustRevokesInRustAndRecordsTheDecisionForTheNextLaunch() {
        val session = FakeAppRuntimeSession()
        val untrusted = mutableListOf<String>()
        val controller = RiotAppsController(session, onUntrusted = { untrusted += it })
        val app = app()

        session.trustApp(app.record.appId)
        assertTrue(controller.isTrusted(app))

        controller.untrust(app)

        assertFalse("Rust no longer trusts it, so the launch gate closes", controller.isTrusted(app))
        assertEquals(
            "the revoke is recorded so restore() does not re-trust it",
            listOf(app.record.appId),
            untrusted,
        )
    }

    /**
     * Rust first, persistence second — the same order [RiotAppsController.trust]
     * uses. A revoke Rust refuses must never be written down as though it had
     * happened.
     */
    @Test
    fun aRevokeRustRefusesIsNeverRecorded() {
        val session = object : AppRuntimeSessionInterface by FakeAppRuntimeSession() {
            override fun `untrustApp`(appId: String) = throw IllegalStateException("NotSpaceOrganizer")
        }
        val untrusted = mutableListOf<String>()
        val controller = RiotAppsController(session, onUntrusted = { untrusted += it })

        val failed = runCatching { controller.untrust(app()) }

        assertTrue("the core's refusal reaches the caller", failed.isFailure)
        assertTrue("and nothing was persisted", untrusted.isEmpty())
    }

    /**
     * The revoke affordance is organizer-gated exactly like the approve one: a
     * member sees no "Turn off" control, because they could not have granted it
     * either. `isOrganizer()` is the signal the surface draws on.
     */
    @Test
    fun onlyAnOrganizerIsOfferedTheTurnOffControl() {
        assertTrue(RiotAppsController(FakeAppRuntimeSession(organizer = true)).isOrganizer())
        assertFalse(RiotAppsController(FakeAppRuntimeSession(organizer = false)).isOrganizer())
    }

    /** Trust still works the way it did — the revoke is its mirror, not its replacement. */
    @Test
    fun trustStillReachesRustAndIsRecorded() {
        val session = FakeAppRuntimeSession()
        val trusted = mutableListOf<String>()
        val controller = RiotAppsController(session, onTrusted = { trusted += it })
        val app = app()

        controller.trust(app)

        assertTrue(controller.isTrusted(app))
        assertEquals(listOf(app.record.appId), trusted)
        assertEquals(listOf("trust:${app.record.appId}"), session.calls)
    }
}
