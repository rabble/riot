//! WU1 G1 evidence: deterministic signed-alert payload codec.
//! Naming: every test is prefixed `public_` per the sprint's filter contract.

use riot_core::model::{
    decode_alert, encode_alert, AlertError, AlertPayload, Certainty, Severity, Urgency,
};

fn canonical_alert() -> AlertPayload {
    AlertPayload {
        object_id: *b"riot-obj-0000001",
        revision_id: *b"riot-rev-0000001",
        created_at: 1_783_000_000,
        valid_from: Some(1_783_000_100),
        expires_at: 1_783_086_400,
        language: "en".to_string(),
        urgency: Urgency::Immediate,
        severity: Severity::Severe,
        certainty: Certainty::Observed,
        headline: "Bridge at 4th St closed".to_string(),
        description: "Use the north route via Alder. Medics staging at the school gym."
            .to_string(),
        affected_area_claim: Some("Downtown between 3rd and 5th".to_string()),
        source_claims: vec!["Confirmed by two field observers at 14:20".to_string()],
        ai_assisted: false,
    }
}

#[test]
fn public_alert_roundtrip_is_deterministic() {
    let alert = canonical_alert();
    let first = encode_alert(&alert).expect("encode");
    let second = encode_alert(&alert).expect("encode twice");
    assert_eq!(first, second, "same payload must produce identical bytes");

    let decoded = decode_alert(&first).expect("decode");
    assert_eq!(decoded, alert, "decode must invert encode exactly");
}

#[test]
fn public_alert_golden_vector_matches_frozen_bytes() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/objects/alert-golden-1.cbor"
    );
    let encoded = encode_alert(&canonical_alert()).expect("encode");

    if std::env::var("RIOT_BLESS").as_deref() == Ok("1") {
        std::fs::write(path, &encoded).expect("bless golden vector");
    }

    let frozen = std::fs::read(path).expect(
        "golden vector missing — run once with RIOT_BLESS=1 to freeze it, then commit",
    );
    assert_eq!(
        encoded, frozen,
        "canonical alert bytes diverged from the frozen golden vector"
    );
    let decoded = decode_alert(&frozen).expect("frozen vector must decode");
    assert_eq!(decoded, canonical_alert());
}

#[test]
fn public_alert_optional_fields_absent_changes_encoding_deterministically() {
    let mut alert = canonical_alert();
    alert.valid_from = None;
    alert.affected_area_claim = None;
    let bytes = encode_alert(&alert).expect("encode without optionals");
    let with_optionals = encode_alert(&canonical_alert()).expect("encode with optionals");
    assert_ne!(bytes, with_optionals);
    assert_eq!(decode_alert(&bytes).expect("decode"), alert);
}

#[test]
fn public_alert_rejects_expiry_not_after_created() {
    let mut alert = canonical_alert();
    alert.expires_at = alert.created_at;
    assert!(matches!(
        encode_alert(&alert),
        Err(AlertError::ExpiryNotAfterCreated)
    ));
}

#[test]
fn public_alert_rejects_empty_source_claims() {
    let mut alert = canonical_alert();
    alert.source_claims.clear();
    assert!(matches!(
        encode_alert(&alert),
        Err(AlertError::MissingSourceClaim)
    ));

    let mut blank = canonical_alert();
    blank.source_claims = vec!["   ".to_string()];
    assert!(matches!(
        encode_alert(&blank),
        Err(AlertError::MissingSourceClaim)
    ));
}

#[test]
fn public_alert_rejects_oversized_fields() {
    let mut alert = canonical_alert();
    alert.headline = "h".repeat(513);
    assert!(matches!(
        encode_alert(&alert),
        Err(AlertError::FieldTooLarge("headline"))
    ));

    let mut alert = canonical_alert();
    alert.description = "d".repeat(65_537);
    assert!(matches!(
        encode_alert(&alert),
        Err(AlertError::FieldTooLarge("description"))
    ));
}

#[test]
fn public_alert_decode_rejects_unknown_key() {
    // A map that copies the canonical alert but appends an unknown key 99.
    // Built with raw minicbor so the strict decoder is exercised from bytes.
    let valid = encode_alert(&canonical_alert()).expect("encode");
    // The canonical encoding is a definite map; bump its length nibble and
    // append `99: 0` (0x18 0x63 0x00) to smuggle in an unknown key.
    let mut tampered = valid.clone();
    assert_eq!(tampered[0] & 0xe0, 0xa0, "expected a definite CBOR map");
    tampered[0] += 1; // one more map pair
    tampered.extend_from_slice(&[0x18, 0x63, 0x00]);
    assert!(matches!(
        decode_alert(&tampered),
        Err(AlertError::UnknownKey(99))
    ));
}

#[test]
fn public_alert_decode_rejects_truncated_input() {
    let valid = encode_alert(&canonical_alert()).expect("encode");
    let truncated = &valid[..valid.len() - 7];
    assert!(decode_alert(truncated).is_err());
}

#[test]
fn public_alert_decode_rejects_oversized_input() {
    let oversized = vec![0u8; 1_048_577]; // payload_bytes ceiling + 1
    assert!(matches!(
        decode_alert(&oversized),
        Err(AlertError::InputTooLarge)
    ));
}

#[test]
fn public_alert_decode_rejects_noncanonical_bytes() {
    // Same logical content, indefinite-length map: must be rejected because
    // only the canonical encoding is acceptable on the wire.
    let valid = encode_alert(&canonical_alert()).expect("encode");
    let mut indefinite = Vec::with_capacity(valid.len() + 1);
    indefinite.push(0xbf); // indefinite map header
    indefinite.extend_from_slice(&valid[1..]);
    indefinite.push(0xff); // break
    assert!(decode_alert(&indefinite).is_err());
}
