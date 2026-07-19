package org.riot.evidence

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.riot_ffi.FollowedSiteRow

/**
 * Pure screening + row-projection tests for the follow-a-site flow. No device,
 * no FFI verify — those live in the core `follow_site` tests. This pins the
 * scheme/length screen and the honest row mapping (crucially: a transport-blocked
 * row offers NO refresh and carries NO fetch URL). Mirrors the iOS
 * `FollowSiteModelTests` so both platforms make the same honest decisions.
 */
class FollowSiteTest {

    private fun row(
        root: String = "r",
        title: String = "Bay Area IMC",
        state: String = "pending-first-sync",
        transportBlocked: Boolean = false,
        fetchUrl: String? = "https://mirror.example/site.bundle",
    ): FollowedSiteRow = FollowedSiteRow(
        root = root,
        title = title,
        state = state,
        transportBlocked = transportBlocked,
        fetchUrl = fetchUrl,
    )

    @Test
    fun screenAcceptsASiteTicketAndTrims() {
        assertEquals("riot://site/v1/abc123", FollowSiteModel.screen("  riot://site/v1/abc123  "))
    }

    @Test
    fun screenRejectsAForeignScheme() {
        val error = assertThrows(FollowSiteScreenException::class.java) {
            FollowSiteModel.screen("riot://newswire/join/v1/xyz")
        }
        assertEquals(FollowSiteError.NOT_A_SITE_TICKET, error.reason)
    }

    @Test
    fun screenRejectsAnOversizePayload() {
        val huge = "riot://site/v1/" + "a".repeat(5000)
        val error = assertThrows(FollowSiteScreenException::class.java) {
            FollowSiteModel.screen(huge)
        }
        assertEquals(FollowSiteError.TOO_LONG, error.reason)
    }

    @Test
    fun hexBytesDecodesA32ByteRoot() {
        val hex = "0a".repeat(32)
        assertArrayEquals(ByteArray(32) { 0x0a }, FollowSiteModel.hexBytes(hex))
    }

    @Test
    fun hexBytesRejectsWrongLengthOrNonHex() {
        assertNull(FollowSiteModel.hexBytes("0a".repeat(31)))
        assertNull(FollowSiteModel.hexBytes("zz".repeat(32)))
    }

    @Test
    fun displayShowsRefreshForAnAvailableSiteWithUrl() {
        val display = FollowedSiteDisplay.from(row(transportBlocked = false))
        assertTrue(display.canRefresh)
        assertEquals("https://mirror.example/site.bundle", display.fetchUrl)
    }

    @Test
    fun displayHoldsBackRefreshForATransportBlockedSite() {
        // Even though a URL is present, a blocked row must not offer a fetch and
        // must carry no URL locally — no clearnet IP can leak to a mirror.
        val display = FollowedSiteDisplay.from(
            row(transportBlocked = true, fetchUrl = "https://leak.example/site.bundle"),
        )
        assertFalse(display.canRefresh)
        assertNull(display.fetchUrl)
    }

    @Test
    fun displayHasNoRefreshWhenTheTicketCarriedNoUrl() {
        val display = FollowedSiteDisplay.from(row(fetchUrl = null))
        assertFalse(display.canRefresh)
    }

    @Test
    fun stateLabelsAreHumanReadable() {
        assertEquals("Up to date", FollowedSiteDisplay.label("available"))
        assertEquals("Waiting for first sync", FollowedSiteDisplay.label("pending-first-sync"))
        assertEquals("Requires Tor — unavailable", FollowedSiteDisplay.label("transport-blocked"))
        assertEquals("Needs attention", FollowedSiteDisplay.label("degraded"))
        assertEquals("novel-token", FollowedSiteDisplay.label("novel-token"))
    }

    @Test
    fun importedSummaryPluralizes() {
        assertEquals("Imported 1 record", FollowedSiteDisplay.importedSummary(1))
        assertEquals("Imported 3 records", FollowedSiteDisplay.importedSummary(3))
        assertEquals("Imported 0 records", FollowedSiteDisplay.importedSummary(0))
    }
}
