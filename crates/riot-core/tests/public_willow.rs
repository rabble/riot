//! WU1 G1 evidence: communal Willow authority, WILLIAM3 digests, canonical
//! entry/capability bytes, and the fixed alert path layout.

use riot_core::model::encode_alert;
use riot_core::willow::{
    alert_path, authorise_entry, build_alert_entry, decode_capability_canonic,
    decode_entry_canonic, encode_capability, encode_entry, generate_communal_author,
    verify_entry, william3_digest, EntryFacts, WillowError,
};
use willow25::entry::Entrylike;
use willow25::groupings::{Coordinatelike, Keylike};

const OBJECT_ID: [u8; 16] = *b"riot-obj-0000001";
const REVISION_ID: [u8; 16] = *b"riot-rev-0000001";
const WILLOW_TS_MICROS: u64 = 836_179_200_000_000; // an arbitrary fixed TAI/J2000 instant

fn canonical_payload() -> Vec<u8> {
    // Reuse the frozen alert golden vector as the payload under test.
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/objects/alert-golden-1.cbor"
    );
    std::fs::read(path).expect("alert golden vector present")
}

#[test]
fn public_william3_matches_frozen_vector_fixture() {
    // The conformance suite proves these vectors through bab_rs directly
    // (with independent willow-go provenance). This test proves willow25's
    // PayloadDigest path produces the same digests, closing the loop
    // between the digest dependency and the entry construction path.
    let fixture = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/willow/william3-vectors.json"
    );
    let raw = std::fs::read_to_string(fixture).expect("vectors fixture present");
    let doc: serde_json::Value = serde_json::from_str(&raw).expect("valid JSON");

    for vector in doc["vectors"].as_array().expect("vectors") {
        let name = vector["name"].as_str().expect("name");
        let input = vector["input"].clone();
        let bytes: Vec<u8> = match input["kind"].as_str().expect("kind") {
            "empty" => Vec::new(),
            "ascii" => input["value"].as_str().unwrap().as_bytes().to_vec(),
            "repeat" => vec![
                input["byte"].as_u64().unwrap() as u8;
                input["count"].as_u64().unwrap() as usize
            ],
            "pattern-mod-251" => (0..input["count"].as_u64().unwrap() as u32)
                .map(|i| (i % 251) as u8)
                .collect(),
            "file" => std::fs::read(format!(
                "{}/../../{}",
                env!("CARGO_MANIFEST_DIR"),
                input["path"].as_str().unwrap()
            ))
            .expect("fixture file"),
            other => panic!("unknown kind {other}"),
        };
        let expected = vector["digest_hex"].as_str().expect("digest");
        assert_eq!(
            hex(&william3_digest(&bytes)),
            expected,
            "willow25 digest path diverged from frozen vector `{name}`"
        );
    }
}

#[test]
fn public_communal_author_authorises_own_subspace() {
    let author = generate_communal_author();
    assert!(
        author.namespace_id().is_communal(),
        "evidence namespace must be communal (LSB of byte 31 is zero)"
    );

    let payload = canonical_payload();
    let entry = build_alert_entry(&author, &OBJECT_ID, &REVISION_ID, WILLOW_TS_MICROS, &payload)
        .expect("entry builds");
    let authorised = authorise_entry(&author, entry.clone()).expect("own-subspace write authorises");

    assert!(verify_entry(&entry, authorised.authorisation_token()));
}

#[test]
fn public_cross_subspace_denial() {
    let author = generate_communal_author();
    let intruder = generate_communal_author();
    let payload = canonical_payload();

    let own_entry =
        build_alert_entry(&author, &OBJECT_ID, &REVISION_ID, WILLOW_TS_MICROS, &payload)
            .expect("entry builds");
    let own_token = authorise_entry(&author, own_entry.clone())
        .expect("own write authorises")
        .authorisation_token()
        .clone();

    // The intruder writes into the author's subspace of the same namespace.
    let forged = build_alert_entry(&author, &OBJECT_ID, &REVISION_ID, WILLOW_TS_MICROS, &payload)
        .expect("entry builds");
    assert!(
        matches!(
            authorise_entry(&intruder, forged),
            Err(WillowError::DoesNotAuthorise)
        ),
        "intruder secret must not mint a token for another subspace"
    );

    // The author's token must not authorise an entry in the intruder's subspace.
    let cross_entry = build_alert_entry(
        &intruder,
        &OBJECT_ID,
        &REVISION_ID,
        WILLOW_TS_MICROS,
        &payload,
    )
    .expect("entry builds");
    assert!(
        !verify_entry(&cross_entry, &own_token),
        "capability area must deny a different subspace before signature checks"
    );
}

#[test]
fn public_alert_entry_binds_path_and_payload() {
    let author = generate_communal_author();
    let payload = canonical_payload();
    let entry = build_alert_entry(&author, &OBJECT_ID, &REVISION_ID, WILLOW_TS_MICROS, &payload)
        .expect("entry builds");

    let expected = alert_path(&OBJECT_ID, &REVISION_ID).expect("path builds");
    assert_eq!(entry.path(), &expected);
    assert_eq!(entry.path().component_count(), 4);

    assert_eq!(entry.payload_length(), payload.len() as u64);
    assert_eq!(
        entry.payload_digest_bytes(),
        william3_digest(&payload),
        "entry digest must be corrected WILLIAM3 of the exact payload bytes"
    );
    assert_eq!(u64::from(entry.timestamp()), WILLOW_TS_MICROS);
}

#[test]
fn public_entry_and_capability_canonical_bytes_roundtrip() {
    let author = generate_communal_author();
    let payload = canonical_payload();
    let entry = build_alert_entry(&author, &OBJECT_ID, &REVISION_ID, WILLOW_TS_MICROS, &payload)
        .expect("entry builds");

    let entry_bytes = encode_entry(&entry);
    let decoded = decode_entry_canonic(&entry_bytes).expect("canonical entry decodes");
    assert_eq!(&decoded, &entry);

    let mut trailing = entry_bytes.clone();
    trailing.push(0x00);
    assert!(
        decode_entry_canonic(&trailing).is_err(),
        "trailing bytes must be rejected"
    );

    let capability = author.write_capability();
    let cap_bytes = encode_capability(&capability);
    let decoded_cap = decode_capability_canonic(&cap_bytes).expect("canonical capability decodes");
    assert_eq!(&decoded_cap, &capability);

    let mut cap_trailing = cap_bytes.clone();
    cap_trailing.push(0x00);
    assert!(decode_capability_canonic(&cap_trailing).is_err());
}

#[test]
fn public_alert_payload_is_the_signed_bytes() {
    // The Willow payload under authority tests must be the exact canonical
    // alert encoding, proving model and willow layers agree on bytes.
    let payload = canonical_payload();
    let reencoded = encode_alert(&riot_core::model::decode_alert(&payload).expect("decodes"))
        .expect("re-encodes");
    assert_eq!(payload, reencoded);
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
