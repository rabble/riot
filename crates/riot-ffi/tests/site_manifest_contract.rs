//! Composite-site Unit 2 Task 5 — FFI contract for the resolved (validated)
//! site-manifest record. Builds a genuinely owner-signed manifest wire with the
//! core primitives and drives `resolve_site_manifest` across the FFI surface:
//! a valid manifest resolves `Valid` with per-member classification; a
//! rule/key-structure mismatch resolves `MemberUnverified`; a tampered signature
//! resolves to the `ManifestInvalid` degraded STATE (not a thrown error); and
//! malformed fixed-width inputs are `InvalidInput`.
//!
//! This is the Rust-level FFI smoke test (the checksum-abort guard on iOS +
//! Android is the coordinator's native rebuild, which lands with the binding
//! regen in the same commit as the new `uniffi::Record`s).

use riot_core::site::manifest::{
    encode_site_manifest, RequireTransport, SiteDisplay, SiteLayout, SiteManifestV1, SiteMemberV1,
    SiteRole, SiteRule, TransportPolicyV1,
};
use riot_core::willow::{
    encode_capability, encode_entry, Entry, OwnedMasthead, Path, MANIFEST_COMPONENT,
};
use riot_ffi::{resolve_site_manifest, ManifestValidationStatus, MobileError};

/// A communal namespace id (marker bit = communal).
fn communal_ns() -> [u8; 32] {
    *riot_core::willow::generate_space_organizer_author()
        .expect("communal author")
        .namespace_id()
        .as_bytes()
}

/// Sign `manifest` (whose `root` must equal a fresh masthead) and return the
/// wire triple plus the site root.
fn owner_signed_wire(build: impl FnOnce([u8; 32]) -> SiteManifestV1) -> (WireInputs, [u8; 32]) {
    let masthead = OwnedMasthead::generate().expect("masthead");
    let root = *masthead.namespace_id().as_bytes();
    let manifest = build(root);
    let payload = encode_site_manifest(&manifest).expect("encode manifest");
    let entry = Entry::builder()
        .namespace_id(masthead.namespace_id().clone())
        .subspace_id(masthead.owner_subspace_id())
        .path(Path::from_slices(&[MANIFEST_COMPONENT]).expect("manifest path"))
        .timestamp(1_000u64)
        .payload(&payload)
        .build();
    let authorised = masthead
        .authorise_owner_entry(entry)
        .expect("owner authorises");
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    (
        WireInputs {
            entry_bytes: encode_entry(authorised.entry()),
            capability_bytes: encode_capability(token.capability()),
            signature: signature.to_bytes().to_vec(),
            payload_bytes: payload,
        },
        root,
    )
}

struct WireInputs {
    entry_bytes: Vec<u8>,
    capability_bytes: Vec<u8>,
    signature: Vec<u8>,
    payload_bytes: Vec<u8>,
}

fn three_member_manifest(
    root: [u8; 32],
    communal_a: [u8; 32],
    communal_b: [u8; 32],
) -> SiteManifestV1 {
    SiteManifestV1 {
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
            allow: vec![
                riot_core::site::manifest::SiteTransport::Iroh,
                riot_core::site::manifest::SiteTransport::Arti,
            ],
            require: RequireTransport::None,
        },
        version: 9,
        layout: SiteLayout::SiteDefault,
        sections: vec![],
    }
}

#[test]
fn valid_manifest_resolves_with_classified_members() {
    let (communal_a, communal_b) = (communal_ns(), communal_ns());
    let (wire, root) =
        owner_signed_wire(|root| three_member_manifest(root, communal_a, communal_b));

    let resolved = resolve_site_manifest(
        wire.entry_bytes,
        wire.capability_bytes,
        wire.signature,
        wire.payload_bytes,
        root.to_vec(),
    )
    .expect("resolve");

    assert_eq!(resolved.status, ManifestValidationStatus::Valid);
    assert_eq!(resolved.version, 9);
    assert_eq!(resolved.root, hex(&root));
    assert_eq!(resolved.members.len(), 3);
    assert!(resolved.members.iter().all(|m| m.verified));
    assert_eq!(resolved.members[0].role, "masthead");
    assert_eq!(resolved.members[0].rule, "owned-write");
    assert_eq!(resolved.members[2].display, "wire-column");
    assert_eq!(resolved.allow_transports, vec!["iroh", "arti"]);
    assert_eq!(resolved.require_transport, "none");
    assert_eq!(resolved.moderation_path, vec![hex(b"mod")]);
    assert!(resolved.invalid_reason.is_none());
}

#[test]
fn rule_key_structure_mismatch_resolves_member_unverified() {
    let communal = communal_ns();
    let (wire, root) = owner_signed_wire(|root| SiteManifestV1 {
        root,
        members: vec![
            SiteMemberV1 {
                ns: root,
                role: SiteRole::Masthead,
                rule: SiteRule::OwnedWrite,
                display: SiteDisplay::FrontArticles,
            },
            // A communal namespace relabelled owned-write — must be unverified.
            SiteMemberV1 {
                ns: communal,
                role: SiteRole::Comments,
                rule: SiteRule::OwnedWrite,
                display: SiteDisplay::UnderArticles,
            },
        ],
        moderation_path: vec![b"mod".to_vec()],
        transport_policy: TransportPolicyV1 {
            allow: vec![],
            require: RequireTransport::Arti,
        },
        version: 3,
        layout: SiteLayout::SiteDefault,
        sections: vec![],
    });

    let resolved = resolve_site_manifest(
        wire.entry_bytes,
        wire.capability_bytes,
        wire.signature,
        wire.payload_bytes,
        root.to_vec(),
    )
    .expect("resolve");

    assert_eq!(resolved.status, ManifestValidationStatus::MemberUnverified);
    assert!(
        resolved.members[0].verified,
        "owned masthead stays verified"
    );
    assert!(
        !resolved.members[1].verified,
        "communal ns relabelled owned-write is unverified"
    );
    assert_eq!(resolved.require_transport, "arti");
}

#[test]
fn tampered_signature_resolves_to_manifest_invalid_state() {
    let (communal_a, communal_b) = (communal_ns(), communal_ns());
    let (mut wire, root) =
        owner_signed_wire(|root| three_member_manifest(root, communal_a, communal_b));
    wire.signature[0] ^= 0xFF;

    let resolved = resolve_site_manifest(
        wire.entry_bytes,
        wire.capability_bytes,
        wire.signature,
        wire.payload_bytes,
        root.to_vec(),
    )
    .expect("resolve returns a state, not an error");

    assert_eq!(resolved.status, ManifestValidationStatus::ManifestInvalid);
    assert!(resolved.members.is_empty());
    assert!(resolved.invalid_reason.is_some());
}

#[test]
fn wrong_length_fixed_inputs_are_invalid_input() {
    let (communal_a, communal_b) = (communal_ns(), communal_ns());
    let (wire, root) =
        owner_signed_wire(|root| three_member_manifest(root, communal_a, communal_b));

    // Signature must be exactly 64 bytes.
    assert!(matches!(
        resolve_site_manifest(
            wire.entry_bytes.clone(),
            wire.capability_bytes.clone(),
            vec![0u8; 10],
            wire.payload_bytes.clone(),
            root.to_vec(),
        ),
        Err(MobileError::InvalidInput)
    ));

    // Followed root must be exactly 32 bytes.
    assert!(matches!(
        resolve_site_manifest(
            wire.entry_bytes,
            wire.capability_bytes,
            wire.signature,
            wire.payload_bytes,
            vec![0u8; 5],
        ),
        Err(MobileError::InvalidInput)
    ));
}

/// Local lowercase-hex mirror of the FFI encoding (for asserting hex fields).
fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        value.push(HEX[(byte >> 4) as usize] as char);
        value.push(HEX[(byte & 0x0f) as usize] as char);
    }
    value
}
