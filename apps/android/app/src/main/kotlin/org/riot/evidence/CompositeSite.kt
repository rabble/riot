package org.riot.evidence

import uniffi.riot_ffi.ResolvedCompositeSite
import uniffi.riot_ffi.ResolvedSiteItem
import uniffi.riot_ffi.SiteDegradation
import uniffi.riot_ffi.SiteItemTreatment
import uniffi.riot_ffi.SiteTrustTier

/**
 * WU-006 Tasks 1-3 (Android parity) — composite-site moderation READ surface.
 *
 * Pure-Kotlin TWIN of iOS `NewswireEditorial.swift`'s "Composite-site
 * moderation read surface (fix #4, read path)" section
 * (`CompositeContentHold`, `CompositeItemRow`, `CompositeSiteReadModel`,
 * `CompositeSiteTierStyle`). Core owns every decision — trust tier, treatment,
 * and degradation — this file only re-shapes core's `ResolvedCompositeSite`
 * for rendering. No native call is made here; `CompositeSiteTest` exercises
 * this on the host JVM against the generated record/enum types only (same
 * honesty note as `RiotControllerNewswireTest`).
 */

/**
 * How the composite site's content surface is presented, resolved from core's
 * single [SiteDegradation] verdict. `Held` is a SECURITY control, not
 * decoration: when core reports `MODERATION_LOADING` the moderation list is
 * not yet current, so the whole content surface is HELD — items are NEVER
 * rendered as trustworthy-clean, because showing not-yet-moderated content as
 * if it were moderated is the exact failure the freshness gate exists to
 * prevent. A more-severe honest degradation (invalid manifest,
 * rollback/equivocation alarm, blocked transport) holds the surface too; the
 * mild states (editorial-only, member-unverified) show content with a notice.
 */
sealed class CompositeContentHold {
    /** Content renders normally (possibly with a mild informational banner). */
    object Shown : CompositeContentHold()

    /** Content is gated behind the banner — not presented as trustworthy. */
    data class Held(val reason: SiteDegradation) : CompositeContentHold()
}

/** Map a composite-site item treatment to the shared post-display treatment —
 *  the same accountable-placeholder rendering a moderated newswire post gets. */
fun NewswirePostDisplay.Companion.fromSite(treatment: SiteItemTreatment): NewswirePostDisplay =
    when (treatment) {
        SiteItemTreatment.ORDINARY -> NewswirePostDisplay.ORDINARY
        SiteItemTreatment.HIDDEN -> NewswirePostDisplay.HIDDEN_INTERSTITIAL
        SiteItemTreatment.TOMBSTONED -> NewswirePostDisplay.TOMBSTONED
    }

/**
 * One accountable row on the composite read surface. The resolved item
 * carries no body (this is the moderation/trust view, not the article
 * reader), so a row is identity + trust tier + moderation treatment. A
 * Hidden/Tombstoned item is an accountable placeholder (its [display]), never
 * a silent disappearance.
 */
data class CompositeItemRow(
    val id: String,
    val authorTag: String,
    val tier: SiteTrustTier,
    val display: NewswirePostDisplay,
) {
    companion object {
        fun of(item: ResolvedSiteItem) = CompositeItemRow(
            id = item.entryId,
            authorTag = item.authorSubspace.take(8),
            tier = item.trustTier,
            display = NewswirePostDisplay.fromSite(item.treatment),
        )
    }
}

/**
 * The read-model for a resolved composite site: the hold decision, the honest
 * banner copy, and the accountable rows. Pure over core's
 * [ResolvedCompositeSite] — the view renders exactly this and makes no trust
 * decision of its own.
 */
data class CompositeSiteReadModel(
    val root: String,
    val hold: CompositeContentHold,
    val bannerMessage: String?,
    val items: List<CompositeItemRow>,
) {
    /** Whether the content surface is HELD (gated, not trustworthy). The view
     *  MUST honour this — under a hold it never renders the rows as clean
     *  content. */
    val isContentHeld: Boolean
        get() = hold is CompositeContentHold.Held

    companion object {
        fun from(resolved: ResolvedCompositeSite) = CompositeSiteReadModel(
            root = resolved.root,
            hold = holdFor(resolved.degradation),
            bannerMessage = banner(resolved.degradation),
            items = resolved.items.map(CompositeItemRow::of),
        )

        /** The hold decision. Every state where content must NOT be trusted
         *  holds the surface; the two mild states still show content with a
         *  notice. */
        internal fun holdFor(degradation: SiteDegradation): CompositeContentHold =
            when (degradation) {
                SiteDegradation.NONE, SiteDegradation.MEMBER_UNVERIFIED, SiteDegradation.EDITORIAL_ONLY ->
                    CompositeContentHold.Shown
                SiteDegradation.MODERATION_LOADING, SiteDegradation.MANIFEST_INVALID,
                SiteDegradation.TRANSPORT_BLOCKED, SiteDegradation.MANIFEST_ROLLBACK_ALARM,
                SiteDegradation.EQUIVOCATION_ALARM ->
                    CompositeContentHold.Held(degradation)
            }

        /** Honest, plain-language banner copy per degradation. `null` only
         *  when fully current — every degraded state names its own "why". */
        internal fun banner(degradation: SiteDegradation): String? =
            when (degradation) {
                SiteDegradation.NONE -> null
                SiteDegradation.MODERATION_LOADING ->
                    "Moderation loading — posts stay held until this site's moderation list catches up."
                SiteDegradation.MANIFEST_INVALID ->
                    "This site couldn't be verified. Its content is held until a valid signature syncs."
                SiteDegradation.MEMBER_UNVERIFIED ->
                    "A section of this site couldn't be verified."
                SiteDegradation.EDITORIAL_ONLY ->
                    "Comments and the open wire are still syncing."
                SiteDegradation.TRANSPORT_BLOCKED ->
                    "This site requires a connection that isn't available right now."
                SiteDegradation.MANIFEST_ROLLBACK_ALARM ->
                    "This site's configuration looks rolled back — content is held."
                SiteDegradation.EQUIVOCATION_ALARM ->
                    "This site has conflicting owner signatures — content is held."
            }
    }
}

/** A short, stable label for the item's owner-resolved trust tier. */
val SiteTrustTier.label: String
    get() = when (this) {
        SiteTrustTier.EDITORIAL -> "Editorial"
        SiteTrustTier.OPEN_WIRE -> "Open wire"
        SiteTrustTier.COMMENT -> "Comment"
    }

/**
 * The badge identity for one resolved item's trust tier — a SECURITY-relevant
 * type, not decoration: an open-wire or comment item must never be able to
 * wear editorial's badge or tint, so [forTier] is required to produce a
 * DISTINCT [badgeSymbol] AND a distinct [tintToken] per tier (an open-wire
 * item can never be confused for an editorial one at a glance). [tintToken]
 * names a theme color function rather than holding a resolved color directly,
 * so this type stays a plain, host-JVM-testable value class independent of
 * Compose/Android `Color`; a render layer resolves it at draw time. (Named
 * `forTier` rather than `for` — `for` is a hard keyword in Kotlin, unlike
 * Swift's backtickable `for`.)
 */
data class CompositeSiteTierStyle(
    /** Stable per-tier token: doubles as the accessibility/testTag suffix. */
    val token: String,
    val badgeSymbol: String,
    val tintToken: String,
) {
    companion object {
        fun forTier(tier: SiteTrustTier): CompositeSiteTierStyle =
            when (tier) {
                SiteTrustTier.EDITORIAL ->
                    CompositeSiteTierStyle(token = "editorial", badgeSymbol = "verified", tintToken = "pink")
                SiteTrustTier.OPEN_WIRE ->
                    CompositeSiteTierStyle(token = "open-wire", badgeSymbol = "wifi_tethering", tintToken = "blue")
                SiteTrustTier.COMMENT ->
                    CompositeSiteTierStyle(token = "comment", badgeSymbol = "chat_bubble", tintToken = "inkSoft")
            }
    }
}

// MARK: - §9.3 mandatory seizure disclosure (WU-006 Task 8a)

/**
 * Creating an owned site mints an owned masthead namespace: the owner
 * capability lives on this phone from that moment on. §9.3 requires that,
 * before that mint happens, the person is shown a BLOCKING disclosure that
 * says plainly what device seizure means for this site — not "you might lose
 * a key," but "a captor who takes this phone unlocked can impersonate the
 * site under your name and revoke the real editors." An activist deciding
 * whether to mint a site needs that sentence, not a euphemism for it. Copy is
 * VERBATIM the iOS `SiteSeizureDisclosure` (WU-006 §5 parity).
 */
object SiteSeizureDisclosure {
    const val TITLE = "If this phone is seized, this site can be taken over"

    /** Deliberately does not stop at "you could lose the key" — it names the
     *  two concrete powers a seizer gains: impersonating the site, and
     *  revoking the real editors. Losing a key is recoverable with a backup;
     *  this is not a backup problem, it is a takeover risk with no undo. */
    const val BODY =
        "Creating this site puts its owner key on this phone. Anyone who " +
            "seizes this phone while it's unlocked — a captor, police, a border " +
            "agent — doesn't just cost you a key. They can impersonate this site " +
            "under your name and revoke the real editors, take over the site, and " +
            "lock you and your team out of it. There is no undo once that " +
            "happens. Only mint this site if you have a plan for keeping this " +
            "phone out of the wrong hands."

    const val ACKNOWLEDGEMENT_LABEL =
        "I understand a phone seizure can hand a captor control of this site"
}

/**
 * Pure gate: the mint action stays disabled until the disclosure above has
 * been explicitly acknowledged. Deterministic and UI-independent so the
 * safety invariant is provable without a live view. Twin of iOS
 * `OwnedSiteCreationGate`.
 */
data class OwnedSiteCreationGate(val disclosureAcknowledged: Boolean = false) {
    /** False until the disclosure is acknowledged; flips back to false if the
     *  acknowledgement is withdrawn (e.g. the toggle is turned off again
     *  before confirming), so the gate can never be primed and left armed. */
    val canMint: Boolean
        get() = disclosureAcknowledged
}
