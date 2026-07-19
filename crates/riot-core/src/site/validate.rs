//! Owner-signed site-manifest validation — **independent of admission**.
//!
//! Admission (Unit 1, `import::bundle::admissible_capability`) proves an entry
//! was authored under *some* valid cap chain rooted at the followed site. That
//! is NOT sufficient for the manifest: a *delegated* owned cap whose granted
//! area happens to cover `/manifest` passes admission (admission never inspects
//! `delegations()`) yet must be REFUSED as the manifest signer — otherwise a
//! delegated editor could publish a manifest and seize the site. This module
//! re-derives the signer requirement from scratch:
//!
//! - the signing cap `is_owned()` **and** carries **zero delegations**, **and**
//! - the cap's `granted_namespace()` equals the entry namespace, **and**
//! - `manifest.root` equals that owned namespace id (invariant 2, root
//!   self-attests), **and**
//! - `manifest.root` equals the **followed site root** (invariant 3, consumed
//!   from Unit 1's admission context — never re-derived here), **and**
//! - the willow25 cryptographic chain check (`verify_entry`) authorises the
//!   entry (owned genesis signed by the namespace secret + receiver signature).
//!
//! Note the signer binding is on `granted_namespace()` / the namespace id, NOT
//! `receiver()`: an owned cap's receiver is the owner's *subspace* author key,
//! distinct from the namespace key that is the site identity. Admission binds
//! the followed root to `genesis().namespace_key()`, so the manifest matches it.
//!
//! Member classification (invariant 1) is intrinsic to key structure: each
//! member's declared `rule` class must agree with its namespace marker bit
//! (`NamespaceId::is_owned()` / `is_communal()`), else that single member is
//! dropped to `Unverified` — the rest of the site still resolves.

use willow25::prelude::*;

use super::manifest::{
    decode_site_manifest, SiteManifestError, SiteManifestV1, SiteMemberV1, SiteRule,
};
use crate::willow::site_paths::MANIFEST_COMPONENT;
use crate::willow::{
    decode_capability_canonic, decode_entry_canonic, verify_entry, AuthorisationToken,
    SignedWillowEntry,
};

/// Largest accepted canonical entry encoding for a manifest record.
const MAX_MANIFEST_ENTRY_BYTES: usize = 4_096;
/// Largest accepted canonical write-capability encoding.
const MAX_MANIFEST_CAPABILITY_BYTES: usize = 65_536;

/// Closed failure vocabulary for manifest signer validation. Distinct from the
/// codec's [`SiteManifestError`] — these are trust failures, not decode faults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SiteManifestValidationError {
    /// A wire component exceeded its byte ceiling.
    ComponentTooLarge,
    /// The entry bytes are not a canonical willow entry.
    CanonicalEntryInvalid,
    /// The capability bytes are not a canonical willow capability.
    CanonicalCapabilityInvalid,
    /// The entry path is not exactly the reserved `/manifest`.
    NotManifestPath,
    /// The entry namespace is not an owned namespace.
    NamespaceNotOwned,
    /// The signing cap is communal (not owned).
    SignerNotOwned,
    /// The signing cap carries delegations — the KEYSTONE refusal that admission
    /// does not make. A delegated cap can pass admission but never sign the
    /// manifest.
    SignerDelegated,
    /// The cap's granted namespace does not equal the entry namespace.
    SignerNamespaceMismatch,
    /// The cap's granted area does not include the entry (does-not-authorise).
    CapabilityDoesNotAuthorise,
    /// `manifest.root` does not equal the hosting owned namespace id (inv 2).
    RootMismatch,
    /// `manifest.root` does not equal the followed site root (inv 3).
    SiteIdentityMismatch,
    /// The receiver signature over the entry does not verify.
    SignatureInvalid,
    /// The manifest payload failed the strict codec.
    ManifestDecode(SiteManifestError),
}

impl std::fmt::Display for SiteManifestValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for SiteManifestValidationError {}

/// Whether a member's declared rule class agrees with its namespace key
/// structure. `Unverified` never disappears a member — it is rendered as an
/// accountable "this section couldn't be verified" state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberClassification {
    /// Declared rule class matches the namespace marker bit.
    Verified,
    /// Declared rule class contradicts the namespace marker bit (or the rule is
    /// an unknown class that cannot be checked against a structure).
    Unverified,
}

/// One member with its resolved classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassifiedMember {
    pub member: SiteMemberV1,
    pub classification: MemberClassification,
}

/// A manifest that passed signer validation, with per-member classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedManifest {
    pub manifest: SiteManifestV1,
    pub members: Vec<ClassifiedMember>,
}

impl ValidatedManifest {
    /// True iff every member classified `Verified`.
    pub fn all_members_verified(&self) -> bool {
        self.members
            .iter()
            .all(|m| m.classification == MemberClassification::Verified)
    }
}

/// Derive a member's classification from its namespace key structure (invariant
/// 1). The manifest only *references* the rule; the marker bit is intrinsic and
/// authoritative. A declared class that contradicts the bit — or an unknown
/// rule class that maps to no bit — is `Unverified`.
fn classify_member(member: &SiteMemberV1) -> MemberClassification {
    let namespace = NamespaceId::from_bytes(&member.ns);
    let agrees = match member.rule {
        SiteRule::OwnedWrite => namespace.is_owned(),
        SiteRule::CommunalOpen => namespace.is_communal(),
        SiteRule::Unknown(_) => false,
    };
    if agrees {
        MemberClassification::Verified
    } else {
        MemberClassification::Unverified
    }
}

/// Validate an owner-signed site manifest, INDEPENDENT of admission.
///
/// `followed_site_root` is the site the user follows (from Unit 1's admission
/// context). Invariant 3 is consumed here — `manifest.root` must equal it — but
/// never re-derived; a different owned root is a different site.
pub fn validate_site_manifest(
    signed: &SignedWillowEntry,
    followed_site_root: &[u8; 32],
) -> Result<ValidatedManifest, SiteManifestValidationError> {
    if signed.entry_bytes.len() > MAX_MANIFEST_ENTRY_BYTES
        || signed.capability_bytes.len() > MAX_MANIFEST_CAPABILITY_BYTES
        || signed.payload_bytes.len() > super::manifest::MAX_SITE_MANIFEST_BYTES
    {
        return Err(SiteManifestValidationError::ComponentTooLarge);
    }

    let entry = decode_entry_canonic(&signed.entry_bytes)
        .map_err(|_| SiteManifestValidationError::CanonicalEntryInvalid)?;
    let capability = decode_capability_canonic(&signed.capability_bytes)
        .map_err(|_| SiteManifestValidationError::CanonicalCapabilityInvalid)?;

    // Reserved path: exactly the single component `/manifest`, nothing under it.
    let mut components = entry.path().components();
    let is_manifest_path = components
        .next()
        .is_some_and(|first| first.as_ref() == MANIFEST_COMPONENT)
        && components.next().is_none();
    if !is_manifest_path {
        return Err(SiteManifestValidationError::NotManifestPath);
    }

    if !entry.namespace_id().is_owned() {
        return Err(SiteManifestValidationError::NamespaceNotOwned);
    }

    // Signer validation, INDEPENDENT of admission. Admission (`admissible_
    // capability`) checks `is_owned()` + root binding but NOT `delegations()`:
    // the zero-delegation requirement is what refuses a delegated signer.
    if !capability.is_owned() {
        return Err(SiteManifestValidationError::SignerNotOwned);
    }
    if !capability.delegations().is_empty() {
        return Err(SiteManifestValidationError::SignerDelegated);
    }
    if capability.granted_namespace() != entry.namespace_id() {
        return Err(SiteManifestValidationError::SignerNamespaceMismatch);
    }
    if !capability.includes(&entry) {
        return Err(SiteManifestValidationError::CapabilityDoesNotAuthorise);
    }

    let manifest = decode_site_manifest(&signed.payload_bytes)
        .map_err(SiteManifestValidationError::ManifestDecode)?;

    // Invariant 2: root self-attests to the hosting owned namespace.
    if &manifest.root != entry.namespace_id().as_bytes() {
        return Err(SiteManifestValidationError::RootMismatch);
    }
    // Invariant 3 (consumed): site identity binds the followed root.
    if &manifest.root != followed_site_root {
        return Err(SiteManifestValidationError::SiteIdentityMismatch);
    }

    // Cryptographic chain check last (the expensive step): owned genesis signed
    // by the namespace secret + the receiver signature over the entry.
    let token = AuthorisationToken::new(capability, signed.signature.into());
    if !verify_entry(&entry, &token) {
        return Err(SiteManifestValidationError::SignatureInvalid);
    }

    let members = manifest
        .members
        .iter()
        .cloned()
        .map(|member| ClassifiedMember {
            classification: classify_member(&member),
            member,
        })
        .collect();

    Ok(ValidatedManifest { manifest, members })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::bundle::admissible_capability;
    use crate::site::manifest::{
        encode_site_manifest, RequireTransport, SiteDisplay, SiteLayout, SiteRole,
        TransportPolicyV1,
    };
    use crate::willow::{encode_capability, encode_entry, OwnedMasthead, OwnedRoot};

    fn manifest_path() -> Path {
        Path::from_slices(&[MANIFEST_COMPONENT]).expect("manifest path")
    }

    /// A communal namespace id (marker bit = communal).
    fn communal_ns() -> [u8; 32] {
        *crate::willow::generate_space_organizer_author()
            .expect("communal author")
            .namespace_id()
            .as_bytes()
    }

    /// Build the manifest CBOR for a site rooted at `root`, with an owned
    /// masthead member plus two communal members (all correctly classified).
    fn manifest_bytes_for(root: [u8; 32], communal_a: [u8; 32], communal_b: [u8; 32]) -> Vec<u8> {
        let manifest = SiteManifestV1 {
            root,
            members: vec![
                SiteMemberV1 {
                    ns: root,
                    role: SiteRole::Masthead,
                    rule: SiteRule::OwnedWrite,
                    display: SiteDisplay::FrontArticles,
                },
                SiteMemberV1 {
                    ns: communal_a,
                    role: SiteRole::Comments,
                    rule: SiteRule::CommunalOpen,
                    display: SiteDisplay::UnderArticles,
                },
                SiteMemberV1 {
                    ns: communal_b,
                    role: SiteRole::OpenWire,
                    rule: SiteRule::CommunalOpen,
                    display: SiteDisplay::WireColumn,
                },
            ],
            moderation_path: vec![b"mod".to_vec()],
            transport_policy: TransportPolicyV1 {
                allow: vec![],
                require: RequireTransport::None,
            },
            version: 1,
            layout: SiteLayout::SiteDefault,
            sections: vec![],
        };
        encode_site_manifest(&manifest).expect("encode manifest")
    }

    fn signed_from(authorised: &AuthorisedEntry, payload_bytes: Vec<u8>) -> SignedWillowEntry {
        let token = authorised.authorisation_token();
        let signature: ed25519_dalek::Signature = token.signature().clone().into();
        SignedWillowEntry {
            entry_bytes: encode_entry(authorised.entry()),
            capability_bytes: encode_capability(token.capability()),
            signature: signature.to_bytes(),
            payload_bytes,
        }
    }

    /// A legitimately owner-signed, self-consistent 3-member manifest and its
    /// site root (owned masthead + two communal members, all correct).
    fn owner_signed_manifest() -> (SignedWillowEntry, [u8; 32]) {
        let masthead = OwnedMasthead::generate().expect("masthead");
        let root = *masthead.namespace_id().as_bytes();
        let payload = manifest_bytes_for(root, communal_ns(), communal_ns());
        let entry = Entry::builder()
            .namespace_id(masthead.namespace_id().clone())
            .subspace_id(masthead.owner_subspace_id())
            .path(manifest_path())
            .timestamp(1_000u64)
            .payload(&payload)
            .build();
        let authorised = masthead
            .authorise_owner_entry(entry)
            .expect("owner authorises manifest");
        (signed_from(&authorised, payload), root)
    }

    #[test]
    fn owner_signed_manifest_validates_and_classifies_all_members_verified() {
        let (signed, root) = owner_signed_manifest();
        let validated = validate_site_manifest(&signed, &root).expect("validates");
        assert_eq!(validated.manifest.root, root);
        assert!(validated.all_members_verified());
        assert_eq!(validated.members.len(), 3);
    }

    #[test]
    fn root_not_equal_owner_is_rejected() {
        // Payload claims a foreign root; the hosting namespace is the masthead.
        let masthead = OwnedMasthead::generate().expect("masthead");
        let root = *masthead.namespace_id().as_bytes();
        let foreign = [0xAB; 32];
        let payload = manifest_bytes_for(foreign, communal_ns(), communal_ns());
        let entry = Entry::builder()
            .namespace_id(masthead.namespace_id().clone())
            .subspace_id(masthead.owner_subspace_id())
            .path(manifest_path())
            .timestamp(1_000u64)
            .payload(&payload)
            .build();
        let authorised = masthead.authorise_owner_entry(entry).expect("authorise");
        let signed = signed_from(&authorised, payload);
        assert_eq!(
            validate_site_manifest(&signed, &root),
            Err(SiteManifestValidationError::RootMismatch)
        );
    }

    #[test]
    fn site_identity_mismatch_is_rejected() {
        // A perfectly valid manifest for site O, but the follower follows a
        // DIFFERENT root — invariant 3 refuses it (never silently this site).
        let (communal_a, communal_b) = (communal_ns(), communal_ns());
        let masthead = OwnedMasthead::generate().expect("masthead");
        let root = *masthead.namespace_id().as_bytes();
        let payload = manifest_bytes_for(root, communal_a, communal_b);
        let entry = Entry::builder()
            .namespace_id(masthead.namespace_id().clone())
            .subspace_id(masthead.owner_subspace_id())
            .path(manifest_path())
            .timestamp(1_000u64)
            .payload(&payload)
            .build();
        let authorised = masthead.authorise_owner_entry(entry).expect("authorise");
        let signed = signed_from(&authorised, payload);
        let other_root = [0x11; 32];
        assert_eq!(
            validate_site_manifest(&signed, &other_root),
            Err(SiteManifestValidationError::SiteIdentityMismatch)
        );
    }

    #[test]
    fn delegated_cap_passes_admission_but_is_refused_as_manifest_signer() {
        // THE KEYSTONE. A delegated owned cap with an area covering `/manifest`:
        //   * passes Unit 1 admission (`admissible_capability`), AND
        //   * genuinely authorises the entry (`verify_entry`),
        // yet manifest validation MUST refuse it (zero-delegation requirement).
        let root_secret = OwnedRoot::generate().expect("owned root");
        let root = *root_secret.namespace_id().as_bytes();

        let owner_subspace = SubspaceSecret::from_bytes(&[1u8; 32]);
        let owner_subspace_id = owner_subspace.corresponding_subspace_id();
        let editor = SubspaceSecret::from_bytes(&[2u8; 32]);
        let editor_id = editor.corresponding_subspace_id();

        // Owner delegates a FULL-area cap to the editor (raw willow25 — the
        // friendly `delegate_section` refuses non-`/articles/` areas; a hostile
        // or careless owner is not bound by that belt).
        let mut delegated =
            WriteCapability::new_owned(root_secret.namespace_secret_ref(), owner_subspace_id);
        delegated
            .try_delegate(&owner_subspace, Area::full(), editor_id.clone())
            .expect("delegate full area");
        assert!(delegated.is_owned());
        assert_eq!(delegated.delegations().len(), 1);

        let payload = manifest_bytes_for(root, communal_ns(), communal_ns());
        let entry = Entry::builder()
            .namespace_id(root_secret.namespace_id().clone())
            .subspace_id(editor_id)
            .path(manifest_path())
            .timestamp(1_000u64)
            .payload(&payload)
            .build();
        let authorised = entry
            .clone()
            .into_authorised_entry(&delegated, &editor)
            .expect("editor authorises with full-area delegated cap");
        let signed = signed_from(&authorised, payload);

        // Direction 1: admission ACCEPTS this cap+entry for the followed site.
        assert!(
            admissible_capability(&delegated, entry.namespace_id(), Some(&root)),
            "admission must accept the delegated full-area cap"
        );
        let token = authorised.authorisation_token();
        assert!(
            verify_entry(&entry, token),
            "the entry is genuinely authorised by the delegated cap"
        );

        // Direction 2: manifest validation REFUSES it — the keystone.
        assert_eq!(
            validate_site_manifest(&signed, &root),
            Err(SiteManifestValidationError::SignerDelegated)
        );
    }

    #[test]
    fn bad_signature_is_rejected() {
        let (mut signed, root) = owner_signed_manifest();
        // Flip a signature byte: all cheaper checks pass, crypto fails.
        signed.signature[0] ^= 0xFF;
        assert_eq!(
            validate_site_manifest(&signed, &root),
            Err(SiteManifestValidationError::SignatureInvalid)
        );
    }

    #[test]
    fn communal_signer_is_rejected() {
        // A communal cap naming the owned namespace: not owned → SignerNotOwned.
        let masthead = OwnedMasthead::generate().expect("masthead");
        let root = *masthead.namespace_id().as_bytes();
        let author = SubspaceSecret::from_bytes(&[7u8; 32]);
        let author_id = author.corresponding_subspace_id();
        let communal_cap =
            WriteCapability::new_communal(masthead.namespace_id().clone(), author_id.clone());
        let payload = manifest_bytes_for(root, communal_ns(), communal_ns());
        let entry = Entry::builder()
            .namespace_id(masthead.namespace_id().clone())
            .subspace_id(author_id)
            .path(manifest_path())
            .timestamp(1_000u64)
            .payload(&payload)
            .build();
        let authorised = entry
            .into_authorised_entry(&communal_cap, &author)
            .expect("communal authorises in its own subspace");
        let signed = signed_from(&authorised, payload);
        assert_eq!(
            validate_site_manifest(&signed, &root),
            Err(SiteManifestValidationError::SignerNotOwned)
        );
    }

    #[test]
    fn rule_key_structure_mismatch_classifies_only_that_member_unverified() {
        // Declare a COMMUNAL namespace as `owned-write` and an OWNED one as
        // `communal-open`: both offending members drop to Unverified; the site
        // still validates and the correctly-declared member stays Verified.
        let masthead = OwnedMasthead::generate().expect("masthead");
        let root = *masthead.namespace_id().as_bytes();
        let owned_member = *OwnedRoot::generate()
            .expect("owned")
            .namespace_id()
            .as_bytes();
        let communal_member = communal_ns();

        let manifest = SiteManifestV1 {
            root,
            members: vec![
                // Correct: owned root declared owned-write.
                SiteMemberV1 {
                    ns: root,
                    role: SiteRole::Masthead,
                    rule: SiteRule::OwnedWrite,
                    display: SiteDisplay::FrontArticles,
                },
                // WRONG: a communal namespace relabelled owned-write.
                SiteMemberV1 {
                    ns: communal_member,
                    role: SiteRole::Comments,
                    rule: SiteRule::OwnedWrite,
                    display: SiteDisplay::UnderArticles,
                },
                // WRONG: an owned namespace relabelled communal-open.
                SiteMemberV1 {
                    ns: owned_member,
                    role: SiteRole::OpenWire,
                    rule: SiteRule::CommunalOpen,
                    display: SiteDisplay::WireColumn,
                },
            ],
            moderation_path: vec![b"mod".to_vec()],
            transport_policy: TransportPolicyV1 {
                allow: vec![],
                require: RequireTransport::None,
            },
            version: 1,
            layout: SiteLayout::SiteDefault,
            sections: vec![],
        };
        let payload = encode_site_manifest(&manifest).expect("encode");
        let entry = Entry::builder()
            .namespace_id(masthead.namespace_id().clone())
            .subspace_id(masthead.owner_subspace_id())
            .path(manifest_path())
            .timestamp(1_000u64)
            .payload(&payload)
            .build();
        let authorised = masthead.authorise_owner_entry(entry).expect("authorise");
        let signed = signed_from(&authorised, payload);

        let validated = validate_site_manifest(&signed, &root).expect("site still validates");
        assert_eq!(
            validated.members[0].classification,
            MemberClassification::Verified
        );
        assert_eq!(
            validated.members[1].classification,
            MemberClassification::Unverified,
            "communal namespace relabelled owned-write is unverified"
        );
        assert_eq!(
            validated.members[2].classification,
            MemberClassification::Unverified,
            "owned namespace relabelled communal-open is unverified"
        );
        assert!(!validated.all_members_verified());
    }

    #[test]
    fn unknown_rule_class_is_unverified() {
        let member = SiteMemberV1 {
            ns: communal_ns(),
            role: SiteRole::Unknown(9),
            rule: SiteRule::Unknown(9),
            display: SiteDisplay::Unknown(9),
        };
        assert_eq!(classify_member(&member), MemberClassification::Unverified);
    }

    #[test]
    fn non_manifest_path_is_rejected() {
        let masthead = OwnedMasthead::generate().expect("masthead");
        let root = *masthead.namespace_id().as_bytes();
        let payload = manifest_bytes_for(root, communal_ns(), communal_ns());
        let entry = Entry::builder()
            .namespace_id(masthead.namespace_id().clone())
            .subspace_id(masthead.owner_subspace_id())
            .path(Path::from_slices(&[crate::willow::ARTICLES_COMPONENT, b"x"]).expect("path"))
            .timestamp(1_000u64)
            .payload(&payload)
            .build();
        let authorised = masthead.authorise_owner_entry(entry).expect("authorise");
        let signed = signed_from(&authorised, payload);
        assert_eq!(
            validate_site_manifest(&signed, &root),
            Err(SiteManifestValidationError::NotManifestPath)
        );
    }
}
