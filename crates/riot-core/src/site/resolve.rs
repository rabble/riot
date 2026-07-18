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

use crate::site::manifest::{SiteManifestV1, SiteRole};

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
}
