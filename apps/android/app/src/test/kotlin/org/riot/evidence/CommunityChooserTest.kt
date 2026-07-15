package org.riot.evidence

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.riot_ffi.CommunityRelationship
import uniffi.riot_ffi.CommunityRow

/**
 * Unit 3C — the Android chooser's pure rendering + returning-last-available
 * logic, host-JVM (the FFI does not load off-device; the switch itself is
 * proven at the FFI level, Risk 10). Mirrors the iOS `CommunityChooserTests`
 * so both platforms render the same plain language.
 */
class CommunityChooserTest {

    private fun row(
        namespaceId: String,
        title: String = "C",
        relationship: CommunityRelationship = CommunityRelationship.ORGANIZER,
        available: Boolean = true,
        archived: Boolean = false,
        quarantined: Boolean = false,
        recentActivityUnixSeconds: ULong? = null,
        syncFreshnessUnixSeconds: ULong? = null,
    ): CommunityRow = CommunityRow(
        namespaceId = namespaceId,
        title = title,
        relationship = relationship,
        descriptorEntryId = null,
        recentActivityUnixSeconds = recentActivityUnixSeconds,
        syncFreshnessUnixSeconds = syncFreshnessUnixSeconds,
        archived = archived,
        quarantined = quarantined,
        available = available,
    )

    @Test
    fun relationshipsRenderInPlainLanguage() {
        assertEquals("Organizer", CommunityRelationship.ORGANIZER.plainLabel())
        assertEquals("Member", CommunityRelationship.MEMBER.plainLabel())
        assertEquals("Public reader", CommunityRelationship.PUBLIC_READER.plainLabel())
    }

    @Test
    fun recentActivityAndSyncFreshnessAreHumanPhrases() {
        val now = 1_000_000L
        fun at(secondsAgo: Long): ULong = (now - secondsAgo).toULong()
        assertEquals("No activity yet", CommunityRelativeTime.recentActivity(null, now))
        assertEquals("Not synced yet", CommunityRelativeTime.syncFreshness(null, now))
        assertEquals("Active just now", CommunityRelativeTime.recentActivity(at(10), now))
        assertEquals("Active 1 minute ago", CommunityRelativeTime.recentActivity(at(60), now))
        assertEquals("Active 2 minutes ago", CommunityRelativeTime.recentActivity(at(120), now))
        assertEquals("Active 1 hour ago", CommunityRelativeTime.recentActivity(at(3_600), now))
        assertEquals("Synced 2 hours ago", CommunityRelativeTime.syncFreshness(at(7_200), now))
        assertEquals("Active 1 day ago", CommunityRelativeTime.recentActivity(at(86_400), now))
    }

    @Test
    fun aChooserRowLeadsWithNameAndRelationshipNeverTheNamespaceId() {
        val now = 1_000_000L
        val ns = "a".repeat(64)
        val chooserRow = CommunityChooserRow.from(
            row(
                namespaceId = ns,
                title = "Queers of Aotearoa",
                relationship = CommunityRelationship.MEMBER,
                recentActivityUnixSeconds = (now - 3_600).toULong(),
            ),
            now,
        )
        assertEquals("Queers of Aotearoa", chooserRow.name)
        assertEquals("Member", chooserRow.relationshipLabel)
        assertEquals("Active 1 hour ago", chooserRow.recentActivity)
        assertEquals("Not synced yet", chooserRow.syncFreshness)
        for (visible in listOf(chooserRow.name, chooserRow.relationshipLabel, chooserRow.recentActivity, chooserRow.syncFreshness)) {
            assertFalse("a technical id leaked into '$visible'", visible.contains(ns))
        }
    }

    @Test
    fun returningOpensTheLastAvailableCommunityDirectly() {
        val active = row("ns-a", title = "A", available = true)
        val outcome = CommunityReturnOutcome.decide(active, listOf(active, row("ns-b")))
        assertEquals(CommunityReturnOutcome.OpenCommunity("ns-a"), outcome)
    }

    @Test
    fun anUnavailableLastCommunityRecoversInPlace() {
        val active = row("ns-a", title = "Fire Watch", available = false)
        val outcome = CommunityReturnOutcome.decide(active, listOf(active))
        assertEquals(CommunityReturnOutcome.Unavailable("Fire Watch"), outcome)
    }

    @Test
    fun noActiveButHeldShowsTheChooserAndNoneIsNoCommunity() {
        assertEquals(
            CommunityReturnOutcome.Chooser,
            CommunityReturnOutcome.decide(null, listOf(row("ns-a"), row("ns-b"))),
        )
        assertEquals(
            CommunityReturnOutcome.NoCommunity,
            CommunityReturnOutcome.decide(null, emptyList()),
        )
        assertEquals(
            CommunityReturnOutcome.NoCommunity,
            CommunityReturnOutcome.decide(null, listOf(row("ns-a", archived = true))),
        )
    }

    // Unit 3D — manual multi-community join: the "pending first sync" state and the
    // provisional label. (The native join/decode themselves are FFI and assumed
    // off-device — Risk 10, same as 1E/2C; this proves the shared derivation that
    // mirrors iOS, so both platforms render the joined-not-yet-synced state alike.)

    @Test
    fun aFreshlyJoinedMemberCommunityIsPendingFirstSync() {
        val joined = row("ns-b", title = "New community · ns-b", relationship = CommunityRelationship.MEMBER)
        assertTrue(CommunityChooserRow.isPendingFirstSync(joined))
        assertTrue(CommunityChooserRow.from(joined, nowUnixSeconds = 1_000_000L).pendingFirstSync)
    }

    @Test
    fun anOrganizerOrAnActiveCommunityIsNotPendingFirstSync() {
        // An organizer's own space has its descriptor locally — never pending.
        assertFalse(
            CommunityChooserRow.isPendingFirstSync(row("ns-a", relationship = CommunityRelationship.ORGANIZER)),
        )
        // Any recorded activity clears the pending state.
        assertFalse(
            CommunityChooserRow.isPendingFirstSync(
                row("ns-b", relationship = CommunityRelationship.MEMBER, recentActivityUnixSeconds = 5uL),
            ),
        )
        // A sync exchange clears it too.
        assertFalse(
            CommunityChooserRow.isPendingFirstSync(
                row("ns-c", relationship = CommunityRelationship.MEMBER, syncFreshnessUnixSeconds = 5uL),
            ),
        )
    }

    @Test
    fun theProvisionalTitleLeadsWithoutAFullTechnicalId() {
        val ns = "abcdef0123456789".repeat(4)
        val title = CommunityShareJoin.provisionalTitle(ns)
        assertEquals("New community · abcdef", title)
        assertFalse("the full namespace id is never the label", title.contains(ns))
    }
}
