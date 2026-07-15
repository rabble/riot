//! Newswire share/join reference FFI contract.
//!
//! Two things must hold across the UniFFI boundary:
//!   1. The deterministic encoders (`newswire_descriptor_content_digest`,
//!      `newswire_encode_share_reference`, `newswire_decode_share_reference`)
//!      reproduce the committed cross-platform golden vector byte-for-byte —
//!      this is the same vector the iOS and Android tests assert against, so a
//!      match here proves all three platforms encode identical records.
//!   2. The real share flow (`newswire_share_reference` on a held descriptor)
//!      binds the descriptor's own content digest and round-trips through the
//!      canonical string.

use riot_ffi::{
    newswire_decode_share_reference, newswire_descriptor_content_digest,
    newswire_encode_share_reference, open_local_profile, NewswireSpaceInput,
};

fn golden() -> serde_json::Value {
    let path = format!(
        "{}/../../fixtures/newswire/newswire-golden-1.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let raw = std::fs::read_to_string(path).expect("read golden fixture");
    serde_json::from_str(&raw).expect("valid golden JSON")
}

fn str_vec(value: &serde_json::Value) -> Vec<String> {
    value
        .as_array()
        .expect("array")
        .iter()
        .map(|item| item.as_str().expect("string").to_string())
        .collect()
}

/// The golden descriptor input, reconstructed from the committed fixture fields.
fn golden_space_input(doc: &serde_json::Value) -> (NewswireSpaceInput, String) {
    let d = &doc["descriptor"];
    let input = NewswireSpaceInput {
        name: d["name"].as_str().expect("name").into(),
        summary: d["summary"].as_str().expect("summary").into(),
        languages: str_vec(&d["languages"]),
        geographic_tags: str_vec(&d["geographic_tags"]),
        topic_tags: str_vec(&d["topic_tags"]),
        editorial_roster: str_vec(&d["editorial_roster_hex"]),
    };
    (
        input,
        d["namespace_id_hex"].as_str().expect("namespace").into(),
    )
}

#[test]
fn ffi_reproduces_the_golden_descriptor_content_digest() {
    let doc = golden();
    let (input, namespace_id) = golden_space_input(&doc);
    let digest = newswire_descriptor_content_digest(input, namespace_id).expect("digest");
    assert_eq!(
        digest,
        doc["descriptor"]["content_digest_hex"]
            .as_str()
            .expect("digest hex")
    );
}

#[test]
fn ffi_reproduces_the_golden_share_reference_string() {
    let doc = golden();
    let share = &doc["share_reference"];
    let encoded = newswire_encode_share_reference(
        share["namespace_id_hex"]
            .as_str()
            .expect("namespace")
            .into(),
        share["descriptor_entry_id_hex"]
            .as_str()
            .expect("entry id")
            .into(),
        share["content_digest_hex"].as_str().expect("digest").into(),
    )
    .expect("encode");
    assert_eq!(encoded, share["encoded"].as_str().expect("encoded"));

    let decoded = newswire_decode_share_reference(encoded).expect("decode");
    assert_eq!(
        decoded.namespace_id,
        share["namespace_id_hex"].as_str().expect("namespace")
    );
    assert_eq!(
        decoded.descriptor_entry_id,
        share["descriptor_entry_id_hex"].as_str().expect("entry id")
    );
    assert_eq!(
        decoded.content_digest,
        share["content_digest_hex"].as_str().expect("digest")
    );
    assert_eq!(decoded.encoded, share["encoded"].as_str().expect("encoded"));
}

#[test]
fn ffi_decode_rejects_a_malformed_reference() {
    assert!(newswire_decode_share_reference("https://example.com/nope".into()).is_err());
    assert!(newswire_decode_share_reference("riot://newswire/join/v1/abc".into()).is_err());
}

/// The real share flow: a profile creates a space, then mints a reference for
/// it. The reference binds that descriptor's own content digest, and the encoded
/// string round-trips through decode.
#[test]
fn held_descriptor_produces_a_verifiable_share_reference() {
    let profile = open_local_profile().expect("profile");
    let space = profile
        .create_newswire_space(NewswireSpaceInput {
            name: "Riverside Commons".into(),
            summary: "A community newswire.".into(),
            languages: vec!["en".into()],
            geographic_tags: vec!["riverside".into()],
            topic_tags: vec!["local".into()],
            editorial_roster: vec![],
        })
        .expect("create space");

    let reference = profile
        .newswire_share_reference(space.entry_id.clone())
        .expect("share reference");
    assert_eq!(reference.descriptor_entry_id, space.entry_id);
    assert_eq!(reference.content_digest.len(), 64);
    assert!(reference.encoded.starts_with("riot://newswire/join/v1/"));

    // The encoded string decodes back to the same coordinates.
    let decoded = newswire_decode_share_reference(reference.encoded.clone()).expect("decode");
    assert_eq!(decoded.namespace_id, reference.namespace_id);
    assert_eq!(decoded.descriptor_entry_id, reference.descriptor_entry_id);
    assert_eq!(decoded.content_digest, reference.content_digest);

    // A different community must not collide on the content digest.
    let other = profile
        .create_newswire_space(NewswireSpaceInput {
            name: "Harbor Commons".into(),
            summary: "A different community newswire.".into(),
            languages: vec!["en".into()],
            geographic_tags: vec!["harbor".into()],
            topic_tags: vec!["local".into()],
            editorial_roster: vec![],
        })
        .expect("create other space");
    let other_reference = profile
        .newswire_share_reference(other.entry_id.clone())
        .expect("other reference");
    assert_ne!(other_reference.content_digest, reference.content_digest);
    assert_ne!(
        other_reference.descriptor_entry_id,
        reference.descriptor_entry_id
    );
}
