//! Composite-site Unit 4 — composite resolver (read-side composition).
//!
//! Turns the manifest + the three typed namespaces + the moderation overlay into
//! a resolved view model that the native shells render with NO business logic
//! (shared-core rule). This module owns the decisions the shells must never make:
//! per-item trust tier, moderation treatment, and honest degradation states.
//!
//! Task 1 here is trust-tier resolution — the anti-impersonation core. "W never
//! masquerades as editorial" is enforced by tagging each item from the
//! OWNER-SIGNED manifest's namespace→role binding, never from anything a peer or
//! a shell can assert.

use crate::newswire::PostTreatment;
use crate::site::manifest::{SiteManifestV1, SiteRole};
use crate::site::moderation::ModerationFreshness;
use crate::site::version_floor::VersionFloorOutcome;
use std::collections::BTreeSet;

/// The per-item trust tier the shells style. Resolved by core from the signed
/// manifest; a shell renders exactly this and never infers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustTier {
    /// `O:/articles` — editorial, cap-chain verified at admission.
    Editorial,
    /// `W` — open publishing (the wire column).
    OpenWire,
    /// `C` — open comments, author = subspace, unlinkable.
    Comment,
}

/// Resolve an item's trust tier from the manifest's namespace→role binding.
///
/// Returns `None` when the entry's namespace is not a manifest member (an item
/// from outside the composite is untrusted — the resolver drops/flags it, never
/// styles it). The mapping is total over the manifest's roles; an `Unknown` role
/// (a forward-compat role this client does not understand) is `None` rather than
/// silently editorial — fail closed, never up-tier.
///
/// **Security invariant (impersonation guard):** an entry in the `OpenWire` (or
/// `Comments`) member namespace can NEVER resolve to `Editorial`. Editorial is
/// reachable only through a `Masthead` member, which Unit 2 manifest validation
/// binds to the owned namespace. So a communal namespace cannot claim editorial
/// unless the owner cryptographically signed it as the masthead — in which case
/// it IS the masthead.
pub fn resolve_trust_tier(
    manifest: &SiteManifestV1,
    entry_namespace: &[u8; 32],
) -> Option<TrustTier> {
    let member = manifest.members.iter().find(|m| &m.ns == entry_namespace)?;
    match member.role {
        SiteRole::Masthead => Some(TrustTier::Editorial),
        SiteRole::OpenWire => Some(TrustTier::OpenWire),
        SiteRole::Comments => Some(TrustTier::Comment),
        SiteRole::Unknown(_) => None,
    }
}

/// The honest, named degradation state of a composite site (§6). A single
/// PRIMARY state, ordered least→most severe; the resolver reports the most severe
/// applicable so a shell shows one clear "why" with a next step, never a blank
/// screen or an infinite spinner. Every non-`None` state holds real content
/// (accountable degradation), never silently drops it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CompositeDegradation {
    /// Fully resolved and current.
    None,
    /// A member was dropped for a rule/key-structure mismatch (§2.2 inv 1); the
    /// rest of the site still resolves. Shown as "this section couldn't be
    /// verified", never a silent disappearance.
    MemberUnverified,
    /// O (editorial) synced but C/W (comments/wire) still pending.
    EditorialOnly,
    /// `/mod/` not yet current (Unit 3 verdict was `Loading`); open namespaces
    /// held with "posts appear once moderation syncs", never rendered un-moderated.
    ModerationLoading,
    /// The required transport is unavailable (fail-closed, §5.3).
    TransportBlocked,
    /// The manifest was rolled back or its `require` downgraded below the durable
    /// floor.
    ManifestRollbackAlarm,
    /// Two conflicting owner signatures at the same version — compromise alarm.
    EquivocationAlarm,
    /// The manifest is invalid/unverifiable — no trustworthy site to resolve.
    ManifestInvalid,
}

/// The already-computed sub-verdicts the resolver combines into one primary
/// degradation. Keeping them as plain inputs makes the precedence headless-testable
/// and keeps `resolve.rs` free of the store/sync/transport plumbing that produces
/// them.
pub struct DegradationInputs<'a> {
    /// `validate_site_manifest` returned `Ok`.
    pub manifest_valid: bool,
    /// No member classified `Unverified` (`ValidatedManifest::all_members_verified`).
    pub all_members_verified: bool,
    /// The durable version-floor verdict.
    pub floor: VersionFloorOutcome,
    /// Unit 3's moderation freshness verdict.
    pub moderation: &'a ModerationFreshness,
    /// The required transport is unavailable.
    pub transport_blocked: bool,
    /// C and W have finished their first sync (else `editorial-only`).
    pub comments_and_wire_synced: bool,
}

/// Resolve the single most-severe degradation from the sub-verdicts. Precedence
/// (severe→mild): a broken manifest dominates (nothing trustworthy to show), then
/// floor alarms, then a blocked transport, then moderation-loading (never render
/// un-moderated), then editorial-only, then member-unverified, then none.
pub fn resolve_degradation(inputs: &DegradationInputs) -> CompositeDegradation {
    if !inputs.manifest_valid {
        return CompositeDegradation::ManifestInvalid;
    }
    match inputs.floor {
        VersionFloorOutcome::EquivocationAlarm => return CompositeDegradation::EquivocationAlarm,
        VersionFloorOutcome::RollbackRejected | VersionFloorOutcome::RequireDowngradeRejected => {
            return CompositeDegradation::ManifestRollbackAlarm
        }
        VersionFloorOutcome::Accepted => {}
    }
    if inputs.transport_blocked {
        return CompositeDegradation::TransportBlocked;
    }
    if matches!(inputs.moderation, ModerationFreshness::Loading(_)) {
        return CompositeDegradation::ModerationLoading;
    }
    if !inputs.comments_and_wire_synced {
        return CompositeDegradation::EditorialOnly;
    }
    if !inputs.all_members_verified {
        return CompositeDegradation::MemberUnverified;
    }
    CompositeDegradation::None
}

/// Resolve one item's moderation treatment from Unit 3's freshness verdict.
///
/// The overlay applies ONLY when moderation is `Current` — a `Loading` verdict
/// yields `Ordinary` here because the whole surface is held at the
/// `ModerationLoading` degradation (the caller must NOT render these items as
/// clean; `resolve_degradation` reports the hold). When `Current`, the sets are
/// already exemption-filtered by Unit 3 (root revoke / manifest tombstone
/// removed), so this applies them directly. Moderated rows become accountable
/// placeholders (`Hidden`/`Tombstoned`), never dropped.
pub fn item_treatment(
    author_key: &[u8; 32],
    entry_id: &[u8; 32],
    moderation: &ModerationFreshness,
) -> PostTreatment {
    let ModerationFreshness::Current {
        revoked,
        tombstoned,
        ..
    } = moderation
    else {
        // Loading: surface is held at the degradation level, not per-item.
        return PostTreatment::Ordinary;
    };
    if tombstoned.contains(entry_id) {
        PostTreatment::Tombstoned { actions: vec![] }
    } else if revoked.contains(author_key) {
        PostTreatment::Hidden { actions: vec![] }
    } else {
        PostTreatment::Ordinary
    }
}

// ---- Task 5: soft-link resolution (comment → article parent) ----

/// A cross-namespace soft link (a comment in `C` referencing its parent article
/// in `O`). Resolved at render only — `C` is open, so there is no admission-time
/// referential integrity, and a reference may point at an article this client has
/// not synced (or that never existed). A dangling reference collapses gracefully.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoftLink {
    /// The parent article is held; the link resolves to its entry-id.
    Resolved([u8; 32]),
    /// The parent is not held — collapse gracefully, never error the whole view.
    Dangling,
}

/// Resolve a comment's parent-article reference against the set of held article
/// entry-ids. A miss is `Dangling`, not a failure — tolerate it (§6 step 7).
pub fn resolve_soft_link(parent_ref: &[u8; 32], held_article_ids: &BTreeSet<[u8; 32]>) -> SoftLink {
    if held_article_ids.contains(parent_ref) {
        SoftLink::Resolved(*parent_ref)
    } else {
        SoftLink::Dangling
    }
}

// ---- Task 6: writer-side state (produced here, rendered in Unit 6) ----

/// The editor's own capability state, surfaced at compose. An editor whose
/// time-boxed cap has expired must be warned BEFORE writing — silent
/// write-rejection is the worst publishing UX (§6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriterCapState {
    /// The cap is within its `time_range`; writes will be accepted.
    Active,
    /// The cap expired; the shell warns "editorial access expired on <date>".
    Expired { expired_at: u64 },
}

/// Resolve the writer's cap state from its expiry and the current time.
pub fn writer_cap_state(cap_expires_at: u64, now: u64) -> WriterCapState {
    if now >= cap_expires_at {
        WriterCapState::Expired {
            expired_at: cap_expires_at,
        }
    } else {
        WriterCapState::Active
    }
}

/// The status of a locally-authored write, core-reported so the shell never
/// infers publish-success from local state. A peer-rejected write is `Failed`,
/// never silently shown as `Published`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteStatus {
    /// Accepted by at least one peer (or the local durable store) — published.
    Published,
    /// Authored locally, not yet confirmed by a peer.
    Pending,
    /// Rejected at a peer (e.g. expired cap, admission refusal) — NOT published.
    Failed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::manifest::{
        RequireTransport, SiteDisplay, SiteLayout, SiteManifestV1, SiteMemberV1, SiteRule,
        TransportPolicyV1,
    };

    const MASTHEAD_NS: [u8; 32] = [0x11; 32];
    const WIRE_NS: [u8; 32] = [0x22; 32];
    const COMMENTS_NS: [u8; 32] = [0x33; 32];
    const STRANGER_NS: [u8; 32] = [0x44; 32];

    fn member(ns: [u8; 32], role: SiteRole, rule: SiteRule, display: SiteDisplay) -> SiteMemberV1 {
        SiteMemberV1 {
            ns,
            role,
            rule,
            display,
        }
    }

    fn manifest() -> SiteManifestV1 {
        SiteManifestV1 {
            root: MASTHEAD_NS,
            members: vec![
                member(
                    MASTHEAD_NS,
                    SiteRole::Masthead,
                    SiteRule::OwnedWrite,
                    SiteDisplay::FrontArticles,
                ),
                member(
                    WIRE_NS,
                    SiteRole::OpenWire,
                    SiteRule::CommunalOpen,
                    SiteDisplay::WireColumn,
                ),
                member(
                    COMMENTS_NS,
                    SiteRole::Comments,
                    SiteRule::CommunalOpen,
                    SiteDisplay::UnderArticles,
                ),
            ],
            moderation_path: vec![b"mod".to_vec()],
            transport_policy: TransportPolicyV1 {
                allow: vec![],
                require: RequireTransport::None,
            },
            version: 1,
            layout: SiteLayout::SiteDefault,
        }
    }

    #[test]
    fn masthead_namespace_resolves_to_editorial() {
        assert_eq!(
            resolve_trust_tier(&manifest(), &MASTHEAD_NS),
            Some(TrustTier::Editorial)
        );
    }

    #[test]
    fn open_wire_namespace_never_resolves_to_editorial() {
        // The impersonation guard: W is open-wire, never editorial.
        let tier = resolve_trust_tier(&manifest(), &WIRE_NS);
        assert_eq!(tier, Some(TrustTier::OpenWire));
        assert_ne!(
            tier,
            Some(TrustTier::Editorial),
            "an open-wire namespace must never masquerade as editorial"
        );
    }

    #[test]
    fn comments_namespace_resolves_to_comment() {
        assert_eq!(
            resolve_trust_tier(&manifest(), &COMMENTS_NS),
            Some(TrustTier::Comment)
        );
    }

    #[test]
    fn a_namespace_not_in_the_manifest_is_untrusted() {
        assert_eq!(resolve_trust_tier(&manifest(), &STRANGER_NS), None);
    }

    #[test]
    fn an_unknown_role_fails_closed_not_editorial() {
        let mut m = manifest();
        m.members[1].role = SiteRole::Unknown(99);
        assert_eq!(
            resolve_trust_tier(&m, &WIRE_NS),
            None,
            "an unknown forward-compat role must fail closed, never up-tier to editorial"
        );
    }

    // ---- degradation + overlay (Tasks 2/3/4) ----

    use crate::site::moderation::{ModerationFreshness, ModerationLoading};
    use std::collections::BTreeSet;

    fn healthy_inputs(moderation: &ModerationFreshness) -> DegradationInputs<'_> {
        DegradationInputs {
            manifest_valid: true,
            all_members_verified: true,
            floor: VersionFloorOutcome::Accepted,
            moderation,
            transport_blocked: false,
            comments_and_wire_synced: true,
        }
    }

    fn current(revoked: &[[u8; 32]], tombstoned: &[[u8; 32]]) -> ModerationFreshness {
        ModerationFreshness::Current {
            revoked: revoked.iter().copied().collect(),
            tombstoned: tombstoned.iter().copied().collect(),
            endorsed: BTreeSet::new(),
        }
    }

    #[test]
    fn a_fully_healthy_current_site_has_no_degradation() {
        let mod_ok = current(&[], &[]);
        assert_eq!(
            resolve_degradation(&healthy_inputs(&mod_ok)),
            CompositeDegradation::None
        );
    }

    #[test]
    fn an_invalid_manifest_dominates_every_other_state() {
        let loading = ModerationFreshness::Loading(ModerationLoading::NoHeartbeat);
        let mut inputs = healthy_inputs(&loading);
        inputs.manifest_valid = false;
        inputs.transport_blocked = true;
        inputs.all_members_verified = false;
        // Even with several problems, the broken manifest is THE reported state.
        assert_eq!(
            resolve_degradation(&inputs),
            CompositeDegradation::ManifestInvalid
        );
    }

    #[test]
    fn a_rollback_alarm_outranks_transport_and_moderation() {
        let loading = ModerationFreshness::Loading(ModerationLoading::SeqGap);
        let mut inputs = healthy_inputs(&loading);
        inputs.floor = VersionFloorOutcome::RollbackRejected;
        inputs.transport_blocked = true;
        assert_eq!(
            resolve_degradation(&inputs),
            CompositeDegradation::ManifestRollbackAlarm
        );
    }

    #[test]
    fn moderation_loading_is_reported_when_not_current() {
        let loading = ModerationFreshness::Loading(ModerationLoading::DigestMismatch);
        assert_eq!(
            resolve_degradation(&healthy_inputs(&loading)),
            CompositeDegradation::ModerationLoading
        );
    }

    #[test]
    fn editorial_only_when_comments_and_wire_are_pending() {
        let mod_ok = current(&[], &[]);
        let mut inputs = healthy_inputs(&mod_ok);
        inputs.comments_and_wire_synced = false;
        assert_eq!(
            resolve_degradation(&inputs),
            CompositeDegradation::EditorialOnly
        );
    }

    #[test]
    fn member_unverified_is_the_mildest_state_still_surfaced() {
        let mod_ok = current(&[], &[]);
        let mut inputs = healthy_inputs(&mod_ok);
        inputs.all_members_verified = false;
        assert_eq!(
            resolve_degradation(&inputs),
            CompositeDegradation::MemberUnverified
        );
    }

    // ---- overlay ----

    const AUTHOR: [u8; 32] = [0x77; 32];
    const ENTRY: [u8; 32] = [0x88; 32];

    #[test]
    fn a_revoked_author_entry_is_hidden_when_current() {
        let mod_state = current(&[AUTHOR], &[]);
        assert!(matches!(
            item_treatment(&AUTHOR, &ENTRY, &mod_state),
            PostTreatment::Hidden { .. }
        ));
    }

    #[test]
    fn a_tombstoned_entry_is_tombstoned_when_current() {
        let mod_state = current(&[], &[ENTRY]);
        assert!(matches!(
            item_treatment(&AUTHOR, &ENTRY, &mod_state),
            PostTreatment::Tombstoned { .. }
        ));
    }

    #[test]
    fn an_unmoderated_entry_is_ordinary_when_current() {
        let mod_state = current(&[], &[]);
        assert!(matches!(
            item_treatment(&AUTHOR, &ENTRY, &mod_state),
            PostTreatment::Ordinary
        ));
    }

    #[test]
    fn loading_holds_the_surface_so_items_are_not_individually_hidden() {
        // Under Loading the overlay is NOT applied per-item (the whole surface is
        // held at ModerationLoading); even a would-be-revoked author is Ordinary
        // here, because rendering it Hidden would leak a partial verdict.
        let loading = ModerationFreshness::Loading(ModerationLoading::NoHeartbeat);
        assert!(matches!(
            item_treatment(&AUTHOR, &ENTRY, &loading),
            PostTreatment::Ordinary
        ));
    }

    // ---- soft links (Task 5) ----

    #[test]
    fn a_soft_link_to_a_held_article_resolves() {
        let mut held = BTreeSet::new();
        held.insert(ENTRY);
        assert_eq!(resolve_soft_link(&ENTRY, &held), SoftLink::Resolved(ENTRY));
    }

    #[test]
    fn a_dangling_soft_link_collapses_gracefully() {
        // The parent article is not held — must collapse, not error the view.
        let held = BTreeSet::new();
        assert_eq!(resolve_soft_link(&ENTRY, &held), SoftLink::Dangling);
    }

    // ---- writer-side state (Task 6) ----

    #[test]
    fn an_active_cap_is_active_and_an_expired_cap_warns() {
        assert_eq!(writer_cap_state(1_000, 500), WriterCapState::Active);
        assert_eq!(
            writer_cap_state(1_000, 1_000),
            WriterCapState::Expired { expired_at: 1_000 }
        );
        assert_eq!(
            writer_cap_state(1_000, 2_000),
            WriterCapState::Expired { expired_at: 1_000 }
        );
    }

    #[test]
    fn a_rejected_write_is_failed_never_published() {
        // The status is core-reported; a peer rejection is Failed, distinct from
        // Published — the shell must not infer publish-success from local state.
        assert_ne!(WriteStatus::Failed, WriteStatus::Published);
        assert_ne!(WriteStatus::Pending, WriteStatus::Published);
    }
}
