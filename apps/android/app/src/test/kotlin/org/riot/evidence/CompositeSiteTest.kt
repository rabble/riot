package org.riot.evidence

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.riot_ffi.ResolvedCompositeSite
import uniffi.riot_ffi.ResolvedSiteItem
import uniffi.riot_ffi.SiteDegradation
import uniffi.riot_ffi.SiteItemTreatment
import uniffi.riot_ffi.SiteTrustTier

/**
 * WU-006 Tasks 1-3 (Android parity) — the composite-site moderation READ model.
 * Pure-Kotlin twin of iOS `CompositeSiteReadModelTests`
 * (apps/ios/RiotTests/NewswireSurfaceTests.swift). The security property under
 * test: a `.moderationLoading` verdict HOLDS the whole content surface (the
 * banner is a control, not decoration), and moderated items render as
 * accountable placeholders. Runs on the host JVM against the generated
 * record/enum types only — no native library needed (mirrors
 * `RiotControllerNewswireTest`'s honesty note).
 */
class CompositeSiteTest {
    private fun hexRepeat(unit: String, count: Int) = unit.repeat(count)

    private fun item(
        id: String,
        treatment: SiteItemTreatment,
        tier: SiteTrustTier = SiteTrustTier.OPEN_WIRE,
    ) = ResolvedSiteItem(
        entryId = id,
        authorSubspace = hexRepeat("ab", 32),
        trustTier = tier,
        treatment = treatment,
    )

    private fun resolved(
        degradation: SiteDegradation,
        items: List<ResolvedSiteItem> = emptyList(),
    ) = ResolvedCompositeSite(
        root = hexRepeat("cd", 32),
        degradation = degradation,
        transportStatus = "available",
        items = items,
        writerCapExpired = false,
    )

    // MARK: - Hold / banner (security property)

    /** THE security property: moderation-loading holds the surface. The whole
     *  content column is gated (`isContentHeld`) and the banner explains why —
     *  a present item is NEVER shown as trustworthy-clean under a hold. */
    @Test
    fun moderationLoadingHoldsTheWholeSurface() {
        val model = CompositeSiteReadModel.from(
            resolved(SiteDegradation.MODERATION_LOADING, items = listOf(item(hexRepeat("11", 32), SiteItemTreatment.ORDINARY))),
        )
        assertTrue("moderation-loading must HOLD the content surface", model.isContentHeld)
        assertEquals(CompositeContentHold.Held(SiteDegradation.MODERATION_LOADING), model.hold)
        assertNotNull("the hold must be explained by a banner", model.bannerMessage)
        assertEquals(1, model.items.size)
    }

    /** A fully-current site shows content: not held, no banner. */
    @Test
    fun currentSiteShowsContentWithoutABanner() {
        val model = CompositeSiteReadModel.from(
            resolved(SiteDegradation.NONE, items = listOf(item(hexRepeat("11", 32), SiteItemTreatment.ORDINARY))),
        )
        assertFalse(model.isContentHeld)
        assertEquals(CompositeContentHold.Shown, model.hold)
        assertNull(model.bannerMessage)
    }

    /** A tombstoned or hidden item renders as an accountable placeholder
     *  treatment, never a silent disappearance — even on a current site. */
    @Test
    fun moderatedItemsRenderAsAccountablePlaceholders() {
        val model = CompositeSiteReadModel.from(
            resolved(
                SiteDegradation.NONE,
                items = listOf(
                    item(hexRepeat("11", 32), SiteItemTreatment.TOMBSTONED),
                    item(hexRepeat("22", 32), SiteItemTreatment.HIDDEN),
                    item(hexRepeat("33", 32), SiteItemTreatment.ORDINARY),
                ),
            ),
        )
        assertEquals(
            listOf(NewswirePostDisplay.TOMBSTONED, NewswirePostDisplay.HIDDEN_INTERSTITIAL, NewswirePostDisplay.ORDINARY),
            model.items.map { it.display },
        )
    }

    /** Ordinary treatment is never rendered as a placeholder. */
    @Test
    fun ordinaryTreatmentIsNotAPlaceholder() {
        val row = CompositeItemRow.of(item(hexRepeat("33", 32), SiteItemTreatment.ORDINARY))
        assertEquals(NewswirePostDisplay.ORDINARY, row.display)
        assertNotEquals(NewswirePostDisplay.TOMBSTONED, row.display)
        assertNotEquals(NewswirePostDisplay.HIDDEN_INTERSTITIAL, row.display)
    }

    /** An invalid manifest also holds the surface (a more-severe honest state),
     *  with a banner — content is never shown as trustworthy without a valid
     *  manifest. */
    @Test
    fun invalidManifestHoldsTheSurface() {
        val model = CompositeSiteReadModel.from(resolved(SiteDegradation.MANIFEST_INVALID))
        assertTrue(model.isContentHeld)
        assertEquals(CompositeContentHold.Held(SiteDegradation.MANIFEST_INVALID), model.hold)
        assertNotNull(model.bannerMessage)
    }

    /** Every other held state also holds the surface with an honest banner. */
    @Test
    fun everyHeldDegradationHoldsWithABanner() {
        for (degradation in listOf(
            SiteDegradation.MODERATION_LOADING,
            SiteDegradation.MANIFEST_INVALID,
            SiteDegradation.TRANSPORT_BLOCKED,
            SiteDegradation.MANIFEST_ROLLBACK_ALARM,
            SiteDegradation.EQUIVOCATION_ALARM,
        )) {
            val model = CompositeSiteReadModel.from(resolved(degradation))
            assertTrue("$degradation must hold the surface", model.isContentHeld)
            assertNotNull("$degradation must explain itself", model.bannerMessage)
        }
    }

    /** transportBlocked's banner is the honest fail-closed string — this site
     *  requires a connection that isn't available, never a vague error. */
    @Test
    fun transportBlockedBannerIsTheHonestFailClosedString() {
        val model = CompositeSiteReadModel.from(resolved(SiteDegradation.TRANSPORT_BLOCKED))
        assertEquals(
            "This site requires a connection that isn't available right now.",
            model.bannerMessage,
        )
    }

    /** The mild states show content WITH a notice — held is reserved for states
     *  where content must not be trusted, so a milder degradation never
     *  needlessly blanks the surface. */
    @Test
    fun mildDegradationShowsContentWithANotice() {
        for (degradation in listOf(SiteDegradation.EDITORIAL_ONLY, SiteDegradation.MEMBER_UNVERIFIED)) {
            val model = CompositeSiteReadModel.from(
                resolved(degradation, items = listOf(item(hexRepeat("11", 32), SiteItemTreatment.ORDINARY))),
            )
            assertFalse("$degradation should show content, not hold it", model.isContentHeld)
            assertNotNull("$degradation still explains itself", model.bannerMessage)
        }
    }

    // MARK: - Trust-tier visual style (anti-impersonation)
    //
    // `CompositeSiteTierStyle` is a SECURITY-relevant UI type, not decoration: an
    // open-wire or comment item must never be able to wear editorial's badge or
    // tint, so `forTier(_:)` is required to produce visually DISTINCT values per
    // tier — twin of iOS `CompositeSiteTierStyle`.

    @Test
    fun editorialAndOpenWireProduceDistinctTierStyles() {
        val editorial = CompositeSiteTierStyle.forTier(SiteTrustTier.EDITORIAL)
        val openWire = CompositeSiteTierStyle.forTier(SiteTrustTier.OPEN_WIRE)

        assertNotEquals("open-wire must not be styled like editorial", editorial, openWire)
        assertNotEquals(
            "an open-wire item must not carry the editorial badge symbol",
            editorial.badgeSymbol, openWire.badgeSymbol,
        )
        assertNotEquals(
            "an open-wire item must not carry the editorial tint",
            editorial.tintToken, openWire.tintToken,
        )
    }

    @Test
    fun editorialAndCommentProduceDistinctTierStyles() {
        val editorial = CompositeSiteTierStyle.forTier(SiteTrustTier.EDITORIAL)
        val comment = CompositeSiteTierStyle.forTier(SiteTrustTier.COMMENT)

        assertNotEquals("a comment must not be styled like editorial", editorial, comment)
        assertNotEquals(
            "a comment must not carry the editorial badge symbol",
            editorial.badgeSymbol, comment.badgeSymbol,
        )
        assertNotEquals(
            "a comment must not carry the editorial tint",
            editorial.tintToken, comment.tintToken,
        )
    }

    @Test
    fun allThreeTrustTiersProduceDistinctTierStyles() {
        val styles = listOf(
            CompositeSiteTierStyle.forTier(SiteTrustTier.EDITORIAL),
            CompositeSiteTierStyle.forTier(SiteTrustTier.OPEN_WIRE),
            CompositeSiteTierStyle.forTier(SiteTrustTier.COMMENT),
        )
        assertEquals("each trust tier must have a visually distinct style", 3, styles.toSet().size)
        assertEquals("each trust tier must have a distinct badge symbol", 3, styles.map { it.badgeSymbol }.toSet().size)
        assertEquals("each trust tier must have a distinct tint", 3, styles.map { it.tintToken }.toSet().size)
    }

    @Test
    fun tierStylesHaveNonEmptyBadgeAndTint() {
        for (tier in listOf(SiteTrustTier.EDITORIAL, SiteTrustTier.OPEN_WIRE, SiteTrustTier.COMMENT)) {
            val style = CompositeSiteTierStyle.forTier(tier)
            assertTrue("$tier must have a badge symbol", style.badgeSymbol.isNotEmpty())
            assertTrue("$tier must have a tint", style.tintToken.isNotEmpty())
        }
    }
}

/**
 * WU-006 Task 8a (Android parity) — the §9.3 mandatory seizure disclosure gate
 * for owned-site (masthead) creation. Twin of iOS `OwnedSiteCreationTests`.
 * Minting an owned masthead puts the site's owner key on this phone; a captor
 * who seizes the unlocked device gains that same power — impersonation of the
 * site plus revocation of the real editors, not merely the loss of a key.
 */
class OwnedSiteCreationTest {
    @Test
    fun disclosureBodyNamesImpersonationAsASeizureConsequence() {
        val body = SiteSeizureDisclosure.BODY.lowercase()
        assertTrue(
            "the disclosure must name impersonation of the site as a seizure consequence",
            body.contains("impersonat"),
        )
    }

    @Test
    fun disclosureBodyNamesEditorRevocationAsASeizureConsequence() {
        val body = SiteSeizureDisclosure.BODY.lowercase()
        assertTrue("the disclosure must name revocation as a seizure consequence", body.contains("revoke"))
        assertTrue("the disclosure must name the real editors as who gets revoked", body.contains("editor"))
    }

    @Test
    fun disclosureBodyDoesNotReduceTheRiskToKeyLossAlone() {
        val body = SiteSeizureDisclosure.BODY.lowercase()
        assertTrue(
            "the disclosure must describe a takeover of the site, not just key loss",
            body.contains("take over") || body.contains("takeover"),
        )
    }

    @Test
    fun disclosureHasATitleAndAnAcknowledgementLabel() {
        assertTrue(SiteSeizureDisclosure.TITLE.isNotEmpty())
        assertTrue(SiteSeizureDisclosure.ACKNOWLEDGEMENT_LABEL.isNotEmpty())
    }

    @Test
    fun gateCanMintIsFalseBeforeAcknowledgement() {
        val gate = OwnedSiteCreationGate()
        assertFalse(gate.disclosureAcknowledged)
        assertFalse(gate.canMint)
    }

    @Test
    fun gateCanMintIsTrueOnlyAfterAcknowledgement() {
        val gate = OwnedSiteCreationGate(disclosureAcknowledged = true)
        assertTrue(gate.canMint)
    }

    @Test
    fun gateCanMintReturnsFalseAgainIfAcknowledgementIsRevoked() {
        val acknowledged = OwnedSiteCreationGate(disclosureAcknowledged = true)
        assertTrue(acknowledged.canMint)
        val revoked = acknowledged.copy(disclosureAcknowledged = false)
        assertFalse(revoked.canMint)
    }
}
