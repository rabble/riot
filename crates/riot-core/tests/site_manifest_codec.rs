//! Composite-site Unit 2 — canonical CBOR codec for the owner-signed site
//! manifest record (`O:/manifest`). Mirrors the newswire/model.rs golden-vector
//! discipline: deterministic encode, byte-identical decode (`prove_canonical`),
//! definite lengths, strictly-ordered integer keys, a closed failure vocabulary,
//! open `role`/`rule`/`display`/transport enums (unknown -> `Unknown` variant),
//! and a CLOSED `layout`/`require` enum (unknown value -> reject).

use minicbor::Encoder;
use riot_core::site::{
    decode_site_manifest, encode_site_manifest, RequireTransport, SiteDisplay, SiteLayout,
    SiteManifestError, SiteManifestV1, SiteMemberV1, SiteRole, SiteRule, SiteTransport,
    TransportPolicyV1, SITE_MANIFEST_SCHEMA,
};

fn site_manifest() -> SiteManifestV1 {
    SiteManifestV1 {
        root: [0x40; 32],
        members: vec![
            SiteMemberV1 {
                ns: [0x40; 32],
                role: SiteRole::Masthead,
                rule: SiteRule::OwnedWrite,
                display: SiteDisplay::FrontArticles,
            },
            SiteMemberV1 {
                ns: [0x41; 32],
                role: SiteRole::Comments,
                rule: SiteRule::CommunalOpen,
                display: SiteDisplay::UnderArticles,
            },
            SiteMemberV1 {
                ns: [0x42; 32],
                role: SiteRole::OpenWire,
                rule: SiteRule::CommunalOpen,
                display: SiteDisplay::WireColumn,
            },
        ],
        moderation_path: vec![b"mod".to_vec()],
        transport_policy: TransportPolicyV1 {
            allow: vec![SiteTransport::Iroh, SiteTransport::Arti],
            require: RequireTransport::None,
        },
        version: 1,
        layout: SiteLayout::SiteDefault,
    }
}

#[test]
fn manifest_round_trips_byte_identically() {
    let manifest = site_manifest();
    let bytes = encode_site_manifest(&manifest).expect("encode");
    let decoded = decode_site_manifest(&bytes).expect("decode");
    assert_eq!(decoded, manifest);
    // Re-encoding the decoded value reproduces the exact bytes.
    assert_eq!(encode_site_manifest(&decoded).expect("re-encode"), bytes);
}

#[test]
fn open_enums_decode_unknown_values_to_unknown_variant() {
    // role/rule/display and the transport allow-list are OPEN: a future value
    // decodes to `Unknown(raw)`, never a hard error (forward compatibility).
    let mut manifest = site_manifest();
    manifest.members[0].role = SiteRole::Unknown(99);
    manifest.members[0].rule = SiteRule::Unknown(88);
    manifest.members[0].display = SiteDisplay::Unknown(77);
    manifest.transport_policy.allow = vec![SiteTransport::Unknown(66)];
    let bytes = encode_site_manifest(&manifest).expect("encode");
    let decoded = decode_site_manifest(&bytes).expect("decode");
    assert_eq!(decoded, manifest);
}

#[test]
fn closed_layout_enum_rejects_unknown_value() {
    // `layout` is CLOSED: core resolves it to a section order the shells render
    // verbatim, so an unknown value is a hard reject (no free-form render blob).
    let manifest = site_manifest();
    let bytes = encode_site_manifest(&manifest).expect("encode");
    // `layout` (key 6) is the highest key, so it is the LAST top-level pair; its
    // uint value is the final byte. Flip it to an unknown layout code.
    let mut hostile = bytes.clone();
    *hostile.last_mut().expect("non-empty") = 0x0a;
    assert_eq!(
        decode_site_manifest(&hostile),
        Err(SiteManifestError::InvalidEnum("layout"))
    );
}

#[test]
fn closed_require_enum_rejects_unknown_value() {
    // `require` is CLOSED and ordered (none < arti); an unknown transport-floor
    // token fails closed rather than being read as `none`.
    let manifest = site_manifest();
    let bytes = encode_site_manifest(&manifest).expect("encode");
    let mut hostile = bytes.clone();
    // Anchor on the transport_policy pair: top-level key 4 (0x04) followed by
    // its map(2) header (0xA2). With allow = [iroh, arti] the bytes are
    // 04 A2 00 82 00 01 01 <require>, so the require value sits at anchor+7.
    let anchor = hostile
        .windows(2)
        .position(|window| window == [0x04, 0xA2])
        .expect("transport_policy pair present");
    hostile[anchor + 7] = 0x09; // unknown require code
    assert_eq!(
        decode_site_manifest(&hostile),
        Err(SiteManifestError::InvalidEnum("require"))
    );
}

#[test]
fn hostile_encodings_are_rejected() {
    let canonical = encode_site_manifest(&site_manifest()).expect("encode");

    let mut trailing = canonical.clone();
    trailing.push(0);

    let mut indefinite_map = vec![0xbf];
    indefinite_map.extend_from_slice(&canonical[1..]);
    indefinite_map.push(0xff);

    let mut wrong_schema = canonical.clone();
    let schema_position = wrong_schema
        .windows(SITE_MANIFEST_SCHEMA.len())
        .position(|window| window == SITE_MANIFEST_SCHEMA.as_bytes())
        .expect("schema present");
    wrong_schema[schema_position] = b'x';

    // A map whose keys are misordered (root before schema) must be rejected.
    let misordered = {
        let mut bytes = Vec::new();
        let mut encoder = Encoder::new(&mut bytes);
        encoder.map(2).unwrap();
        encoder.u8(1).unwrap().bytes(&[0x40; 32]).unwrap();
        encoder.u8(0).unwrap().str(SITE_MANIFEST_SCHEMA).unwrap();
        bytes
    };

    let cases = [
        (trailing, SiteManifestError::TrailingBytes),
        (indefinite_map, SiteManifestError::NonCanonical),
        (wrong_schema, SiteManifestError::WrongSchema),
        (misordered, SiteManifestError::DuplicateOrMisorderedKey(0)),
    ];
    for (bytes, expected) in cases {
        assert_eq!(decode_site_manifest(&bytes), Err(expected));
    }
}

#[test]
fn oversized_membership_is_rejected() {
    let mut manifest = site_manifest();
    manifest.members = (0..(riot_core::site::MAX_SITE_MEMBERS + 1))
        .map(|index| SiteMemberV1 {
            ns: [index as u8; 32],
            role: SiteRole::Comments,
            rule: SiteRule::CommunalOpen,
            display: SiteDisplay::UnderArticles,
        })
        .collect();
    assert_eq!(
        encode_site_manifest(&manifest),
        Err(SiteManifestError::TooManyEntries("members"))
    );
}
