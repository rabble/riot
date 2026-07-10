//! WU1 G1 evidence: RiotEvidenceBundleV1 deterministic codec with visible
//! magic, hard ceilings, and full cryptographic re-verification on decode.

use riot_core::import::{
    bundle_digest, decode_bundle, encode_bundle, entry_digest, object_digest, BundleError,
    BundleItem, BUNDLE_MAGIC,
};
use riot_core::willow::{authorise_entry, build_alert_entry, generate_communal_author};

fn payload() -> Vec<u8> {
    std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/objects/alert-golden-1.cbor"
    ))
    .expect("alert golden vector present")
}

fn item(seed: u8) -> BundleItem {
    let author = generate_communal_author();
    let object_id = [seed; 16];
    let revision_id = [seed.wrapping_add(1); 16];
    let bytes = payload();
    let entry = build_alert_entry(&author, &object_id, &revision_id, 836_179_200_000_000, &bytes)
        .expect("entry builds");
    let authorised = authorise_entry(&author, entry).expect("authorises");
    BundleItem::new(authorised, bytes).expect("item builds")
}

#[test]
fn public_bundle_roundtrip_preserves_items_and_magic() {
    let items = vec![item(1), item(2)];
    let encoded = encode_bundle(&items).expect("encode");

    assert_eq!(&encoded[..6], BUNDLE_MAGIC, "visible RIOTE1 magic prefix");

    let decoded = decode_bundle(&encoded).expect("decode");
    assert_eq!(decoded.len(), 2);
    for (before, after) in items.iter().zip(decoded.iter()) {
        assert_eq!(before.entry_bytes(), after.entry_bytes());
        assert_eq!(before.capability_bytes(), after.capability_bytes());
        assert_eq!(before.signature_bytes(), after.signature_bytes());
        assert_eq!(before.payload(), after.payload());
    }

    // Determinism: encoding the decoded items reproduces the exact bytes.
    assert_eq!(encode_bundle(&decoded).expect("re-encode"), encoded);
}

#[test]
fn public_bundle_rejects_wrong_magic() {
    let encoded = encode_bundle(&[item(3)]).expect("encode");
    let mut wrong = encoded.clone();
    wrong[0] = b'X';
    assert!(matches!(
        decode_bundle(&wrong),
        Err(BundleError::WrongMagic)
    ));
}

#[test]
fn public_bundle_rejects_too_many_entries() {
    let one = item(4);
    let items = vec![one; 65];
    assert!(matches!(
        encode_bundle(&items),
        Err(BundleError::TooManyEntries)
    ));
}

#[test]
fn public_bundle_rejects_tampered_signature() {
    let items = vec![item(5)];
    let mut encoded = encode_bundle(&items).expect("encode");
    // Locate the 64-byte signature by its known content and corrupt one byte.
    let sig = items[0].signature_bytes();
    let pos = encoded
        .windows(sig.len())
        .position(|w| w == sig)
        .expect("signature bytes present in encoding");
    encoded[pos] ^= 0x01;
    assert!(matches!(
        decode_bundle(&encoded),
        Err(BundleError::DoesNotAuthorise) | Err(BundleError::Malformed)
    ));
}

#[test]
fn public_bundle_rejects_payload_digest_mismatch() {
    let good = item(6);
    let mut tampered_payload = good.payload().to_vec();
    let last = tampered_payload.len() - 1;
    tampered_payload[last] ^= 0xff;

    // Re-frame the same entry/authorisation with different payload bytes.
    let forged = BundleItem::from_raw_parts(
        good.entry_bytes().to_vec(),
        good.capability_bytes().to_vec(),
        good.signature_bytes().to_vec(),
        tampered_payload,
    );
    let encoded = encode_bundle_unchecked_for_tests(&forged);
    assert!(matches!(
        decode_bundle(&encoded),
        Err(BundleError::PayloadDigestMismatch)
    ));
}

// Encodes a single raw item without encode-side validation, exercising the
// decoder's own checks. Uses the public raw framing helper.
fn encode_bundle_unchecked_for_tests(item: &BundleItem) -> Vec<u8> {
    riot_core::import::encode_bundle_raw(std::slice::from_ref(item))
}

#[test]
fn public_bundle_digest_vocabulary_is_stable() {
    let one = item(7);
    let encoded = encode_bundle(std::slice::from_ref(&one)).expect("encode");

    let bd1 = bundle_digest(&encoded);
    let bd2 = bundle_digest(&encoded);
    assert_eq!(bd1, bd2);
    assert_eq!(bd1.len(), 32);

    let ed = entry_digest(
        one.entry_bytes(),
        one.capability_bytes(),
        one.signature_bytes(),
    );
    assert_eq!(ed.len(), 32);
    // Domain separation: reordering the same bytes changes the digest.
    let ed_swapped = entry_digest(
        one.capability_bytes(),
        one.entry_bytes(),
        one.signature_bytes(),
    );
    assert_ne!(ed, ed_swapped);

    let od = object_digest(one.payload());
    assert_eq!(od, object_digest(&payload()));
}

#[test]
fn public_bundle_rejects_trailing_bytes() {
    let mut encoded = encode_bundle(&[item(8)]).expect("encode");
    encoded.push(0x00);
    assert!(matches!(
        decode_bundle(&encoded),
        Err(BundleError::TrailingBytes)
    ));
}
