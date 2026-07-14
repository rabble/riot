//! WU1 G1 evidence: deterministic signed-alert payload codec.
//! Naming: every test is prefixed `public_` per the sprint's filter contract.

use minicbor::Encoder;
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
        description: "Use the north route via Alder. Medics staging at the school gym.".to_string(),
        affected_area_claim: Some("Downtown between 3rd and 5th".to_string()),
        source_claims: vec!["Confirmed by two field observers at 14:20".to_string()],
        ai_assisted: false,
    }
}

fn encoded_with_enum_values(urgency: u8, severity: u8, certainty: u8) -> Vec<u8> {
    let mut bytes = encode_alert(&canonical_alert()).expect("canonical alert encodes");
    let marker = [0x07, 0x00, 0x08, 0x01, 0x09, 0x00];
    let start = bytes
        .windows(marker.len())
        .position(|window| window == marker)
        .expect("canonical enum fields are adjacent");
    bytes[start + 1] = urgency;
    bytes[start + 3] = severity;
    bytes[start + 5] = certainty;
    bytes
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RawFault {
    None,
    Null(u8),
    TruncateValue(u8),
    InvalidUtf8(u8),
    IndefiniteSources,
    NullSourceClaim,
    EmptyText(u8),
}

fn raw_alert(skip_key: Option<u8>, fault: RawFault) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = Encoder::new(&mut bytes);
    encoder.map(15 - u64::from(skip_key.is_some())).unwrap();
    for key in 0..=14u8 {
        if skip_key == Some(key) {
            continue;
        }
        encoder.u8(key).unwrap();
        if fault == RawFault::TruncateValue(key) {
            return encoder.into_writer().to_vec();
        }
        if fault == RawFault::Null(key) {
            encoder.null().unwrap();
            continue;
        }
        if fault == RawFault::InvalidUtf8(key) {
            encoder.writer_mut().extend_from_slice(&[0x61, 0xff]);
            continue;
        }
        match key {
            0 => {
                encoder.str(riot_core::model::ALERT_SCHEMA).unwrap();
            }
            1 => {
                encoder.bytes(b"riot-obj-0000001").unwrap();
            }
            2 => {
                encoder.bytes(b"riot-rev-0000001").unwrap();
            }
            3 => {
                encoder.u64(1_783_000_000).unwrap();
            }
            4 => {
                encoder.u64(1_783_000_100).unwrap();
            }
            5 => {
                encoder.u64(1_783_086_400).unwrap();
            }
            6 => {
                encoder.str("en").unwrap();
            }
            7 => {
                encoder.u8(0).unwrap();
            }
            8 => {
                encoder.u8(1).unwrap();
            }
            9 => {
                encoder.u8(0).unwrap();
            }
            10 => {
                encoder
                    .str(if fault == RawFault::EmptyText(10) {
                        ""
                    } else {
                        "Bridge closed"
                    })
                    .unwrap();
            }
            11 => {
                encoder.str("Use another route").unwrap();
            }
            12 => {
                encoder.str("Downtown").unwrap();
            }
            13 if fault == RawFault::IndefiniteSources => {
                encoder.begin_array().unwrap().end().unwrap();
            }
            13 => {
                encoder.array(1).unwrap();
                if fault == RawFault::NullSourceClaim {
                    encoder.null().unwrap();
                } else {
                    encoder.str("field report").unwrap();
                }
            }
            14 => {
                encoder.bool(false).unwrap();
            }
            _ => unreachable!(),
        }
    }
    bytes
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

    let frozen = std::fs::read(path)
        .expect("golden vector missing — run once with RIOT_BLESS=1 to freeze it, then commit");
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
fn public_alert_decode_rejects_duplicate_key() {
    // Duplicate the final pair (key 14): same key twice violates ascending
    // order and must produce the distinct misordered/duplicate code.
    let valid = encode_alert(&canonical_alert()).expect("encode");
    let mut tampered = valid.clone();
    assert_eq!(tampered[0] & 0xe0, 0xa0);
    tampered[0] += 1;
    tampered.extend_from_slice(&[0x0e, 0xf4]); // key 14 again, value false
    assert!(matches!(
        decode_alert(&tampered),
        Err(AlertError::DuplicateOrMisorderedKey(14))
    ));
}

#[test]
fn public_alert_decode_rejects_non_shortest_integer_encoding() {
    // Re-encode key 0 (0x00) as the two-byte form 0x18 0x00. The strict
    // canonicality proof must reject the widened-but-equal document.
    let valid = encode_alert(&canonical_alert()).expect("encode");
    assert_eq!(valid[1], 0x00, "first key is 0");
    let mut widened = Vec::with_capacity(valid.len() + 1);
    widened.push(valid[0]);
    widened.extend_from_slice(&[0x18, 0x00]);
    widened.extend_from_slice(&valid[2..]);
    assert!(decode_alert(&widened).is_err());
}

#[test]
fn public_alert_golden_json_projection_matches_cbor() {
    // The JSON projection is diagnostic only; CBOR remains the signed form.
    // It must agree with the frozen CBOR fixture's hash.
    let json_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/objects/alert-golden-1.json"
    );
    let cbor_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/objects/alert-golden-1.cbor"
    );
    let doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(json_path).expect("json projection"))
            .expect("valid json");
    let cbor = std::fs::read(cbor_path).expect("cbor fixture");

    use sha2::Digest;
    let actual_hash: String = sha2::Sha256::digest(&cbor)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(doc["cbor_sha256"].as_str(), Some(actual_hash.as_str()));
    assert_eq!(doc["headline"].as_str(), Some("Bridge at 4th St closed"));
    assert_eq!(doc["schema"].as_str(), Some("org.riot.alert/1"));
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

#[test]
fn public_alert_error_display_is_stable_for_every_variant() {
    let cases = [
        (AlertError::ExpiryNotAfterCreated, "ExpiryNotAfterCreated"),
        (AlertError::MissingSourceClaim, "MissingSourceClaim"),
        (AlertError::TooManySourceClaims, "TooManySourceClaims"),
        (
            AlertError::FieldEmpty("headline"),
            "FieldEmpty(\"headline\")",
        ),
        (
            AlertError::FieldTooLarge("description"),
            "FieldTooLarge(\"description\")",
        ),
        (AlertError::InputTooLarge, "InputTooLarge"),
        (AlertError::UnknownKey(99), "UnknownKey(99)"),
        (
            AlertError::DuplicateOrMisorderedKey(14),
            "DuplicateOrMisorderedKey(14)",
        ),
        (AlertError::MissingKey(5), "MissingKey(5)"),
        (AlertError::WrongSchema, "WrongSchema"),
        (
            AlertError::InvalidEnum("urgency"),
            "InvalidEnum(\"urgency\")",
        ),
        (AlertError::NonCanonical, "NonCanonical"),
        (AlertError::TrailingBytes, "TrailingBytes"),
        (AlertError::Malformed, "Malformed"),
    ];

    for (error, expected) in cases {
        assert_eq!(error.to_string(), expected);
    }
}

#[test]
fn public_alert_text_fields_accept_exact_limits_and_reject_empty_or_over_limit() {
    let mut exact = canonical_alert();
    exact.language = "l".repeat(35);
    exact.headline = "h".repeat(512);
    exact.description = "d".repeat(65_536);
    exact.affected_area_claim = Some("a".repeat(2_048));
    exact.source_claims = vec!["s".repeat(1_024)];
    assert!(encode_alert(&exact).is_ok());

    for (field, expected) in [
        ("language", AlertError::FieldEmpty("language")),
        ("headline", AlertError::FieldEmpty("headline")),
        ("description", AlertError::FieldEmpty("description")),
        (
            "affected_area_claim",
            AlertError::FieldEmpty("affected_area_claim"),
        ),
    ] {
        let mut alert = canonical_alert();
        match field {
            "language" => alert.language.clear(),
            "headline" => alert.headline = "   ".into(),
            "description" => alert.description.clear(),
            "affected_area_claim" => alert.affected_area_claim = Some(" ".into()),
            _ => unreachable!(),
        }
        assert_eq!(encode_alert(&alert), Err(expected));
    }

    let mut short_language = canonical_alert();
    short_language.language = "x".into();
    assert_eq!(
        encode_alert(&short_language),
        Err(AlertError::FieldEmpty("language"))
    );

    for (field, expected) in [
        ("language", AlertError::FieldTooLarge("language")),
        (
            "affected_area_claim",
            AlertError::FieldTooLarge("affected_area_claim"),
        ),
        ("source_claim", AlertError::FieldTooLarge("source_claim")),
    ] {
        let mut alert = canonical_alert();
        match field {
            "language" => alert.language = "l".repeat(36),
            "affected_area_claim" => alert.affected_area_claim = Some("a".repeat(2_049)),
            "source_claim" => alert.source_claims = vec!["s".repeat(1_025)],
            _ => unreachable!(),
        }
        assert_eq!(encode_alert(&alert), Err(expected));
    }
}

#[test]
fn public_alert_source_count_and_expiry_boundaries_are_enforced() {
    let mut exact = canonical_alert();
    exact.source_claims = (0..16).map(|index| format!("source {index}")).collect();
    assert!(encode_alert(&exact).is_ok());

    let mut over = canonical_alert();
    over.source_claims = (0..17).map(|index| format!("source {index}")).collect();
    assert_eq!(encode_alert(&over), Err(AlertError::TooManySourceClaims));

    let mut before_creation = canonical_alert();
    before_creation.expires_at = before_creation.created_at - 1;
    assert_eq!(
        encode_alert(&before_creation),
        Err(AlertError::ExpiryNotAfterCreated)
    );

    let mut optional_validity = canonical_alert();
    optional_validity.valid_from = Some(optional_validity.expires_at + 1);
    assert!(encode_alert(&optional_validity).is_ok());
}

#[test]
fn public_alert_closed_enums_accept_all_known_values_and_reject_unknown_values() {
    let urgency = [
        Urgency::Immediate,
        Urgency::Expected,
        Urgency::Future,
        Urgency::Past,
        Urgency::Unknown,
    ];
    let severity = [
        Severity::Extreme,
        Severity::Severe,
        Severity::Moderate,
        Severity::Minor,
        Severity::Unknown,
    ];
    let certainty = [
        Certainty::Observed,
        Certainty::Likely,
        Certainty::Possible,
        Certainty::Unlikely,
        Certainty::Unknown,
    ];

    for raw in 0..=4 {
        let decoded = decode_alert(&encoded_with_enum_values(raw, raw, raw)).expect("known enums");
        assert_eq!(decoded.urgency, urgency[raw as usize]);
        assert_eq!(decoded.severity, severity[raw as usize]);
        assert_eq!(decoded.certainty, certainty[raw as usize]);
    }

    assert_eq!(
        decode_alert(&encoded_with_enum_values(5, 1, 0)),
        Err(AlertError::InvalidEnum("urgency"))
    );
    assert_eq!(
        decode_alert(&encoded_with_enum_values(0, 5, 0)),
        Err(AlertError::InvalidEnum("severity"))
    );
    assert_eq!(
        decode_alert(&encoded_with_enum_values(0, 1, 5)),
        Err(AlertError::InvalidEnum("certainty"))
    );
}

#[test]
fn public_alert_decoder_enforces_map_source_schema_and_trailing_boundaries() {
    assert_eq!(decode_alert(&[]), Err(AlertError::Malformed));
    assert_eq!(decode_alert(&[0xa0]), Err(AlertError::Malformed));

    let mut too_many_pairs = Vec::new();
    Encoder::new(&mut too_many_pairs).map(129).unwrap();
    assert_eq!(decode_alert(&too_many_pairs), Err(AlertError::Malformed));

    let canonical = encode_alert(&canonical_alert()).unwrap();
    let source_array = canonical
        .windows(2)
        .position(|window| window == [0x0d, 0x81])
        .expect("one-element source array");

    let mut no_sources = canonical.clone();
    no_sources[source_array + 1] = 0x80;
    assert_eq!(
        decode_alert(&no_sources),
        Err(AlertError::MissingSourceClaim)
    );

    let mut too_many_sources = canonical.clone();
    too_many_sources[source_array + 1] = 0x91;
    assert_eq!(
        decode_alert(&too_many_sources),
        Err(AlertError::TooManySourceClaims)
    );

    let mut trailing = canonical.clone();
    trailing.push(0);
    assert_eq!(decode_alert(&trailing), Err(AlertError::TrailingBytes));

    let mut wrong_schema = canonical;
    let schema_start = wrong_schema
        .windows(riot_core::model::ALERT_SCHEMA.len())
        .position(|window| window == riot_core::model::ALERT_SCHEMA.as_bytes())
        .unwrap();
    wrong_schema[schema_start] = b'x';
    assert_eq!(decode_alert(&wrong_schema), Err(AlertError::WrongSchema));
}

#[test]
fn public_alert_decoder_rejects_wrong_type_and_over_limit_text_before_allocation() {
    let canonical = encode_alert(&canonical_alert()).unwrap();
    let language = canonical
        .windows(5)
        .position(|window| window == [0x06, 0x62, b'e', b'n', 0x07])
        .expect("language field followed by urgency key");

    let mut wrong_type = canonical.clone();
    wrong_type[language + 1] = 0x42;
    assert_eq!(decode_alert(&wrong_type), Err(AlertError::Malformed));

    let mut overlong = canonical[..language + 1].to_vec();
    Encoder::new(&mut overlong).str(&"l".repeat(36)).unwrap();
    overlong.extend_from_slice(&canonical[language + 4..]);
    assert_eq!(
        decode_alert(&overlong),
        Err(AlertError::FieldTooLarge("language"))
    );
}

#[test]
fn public_alert_decoder_maps_every_field_type_failure_without_panicking() {
    let expected = [
        (0, AlertError::Malformed),
        (1, AlertError::Malformed),
        (2, AlertError::Malformed),
        (3, AlertError::Malformed),
        (4, AlertError::Malformed),
        (5, AlertError::Malformed),
        (6, AlertError::Malformed),
        (7, AlertError::Malformed),
        (8, AlertError::Malformed),
        (9, AlertError::Malformed),
        (10, AlertError::Malformed),
        (11, AlertError::Malformed),
        (12, AlertError::Malformed),
        (13, AlertError::Malformed),
        (14, AlertError::Malformed),
    ];
    for (key, error) in expected {
        assert_eq!(
            decode_alert(&raw_alert(None, RawFault::Null(key))),
            Err(error)
        );
    }

    let mut missing_key_bytes = Vec::new();
    Encoder::new(&mut missing_key_bytes).map(13).unwrap();
    assert_eq!(decode_alert(&missing_key_bytes), Err(AlertError::Malformed));

    assert_eq!(
        decode_alert(&raw_alert(None, RawFault::TruncateValue(6))),
        Err(AlertError::Malformed)
    );
    assert_eq!(
        decode_alert(&raw_alert(None, RawFault::InvalidUtf8(6))),
        Err(AlertError::Malformed)
    );
    assert_eq!(
        decode_alert(&raw_alert(None, RawFault::IndefiniteSources)),
        Err(AlertError::NonCanonical)
    );
    assert_eq!(
        decode_alert(&raw_alert(None, RawFault::NullSourceClaim)),
        Err(AlertError::Malformed)
    );
    assert_eq!(
        decode_alert(&raw_alert(None, RawFault::EmptyText(10))),
        Err(AlertError::FieldEmpty("headline"))
    );
}

#[test]
fn public_alert_decoder_reports_every_required_missing_key() {
    for key in [1, 2, 3, 5, 6, 7, 8, 9, 10, 11, 13, 14] {
        assert_eq!(
            decode_alert(&raw_alert(Some(key), RawFault::None)),
            Err(AlertError::MissingKey(key as u64))
        );
    }
}
