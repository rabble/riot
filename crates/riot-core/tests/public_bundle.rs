//! G1 evidence: RiotEvidenceBundleV1 — deterministic framing, hard global
//! and cumulative ceilings, frozen fatal precedence, sibling isolation, and
//! sanitized structured diagnostics.

use minicbor::Encoder;
use riot_core::import::{
    decode_bundle, encode_bundle, BundleDecodeOutcome, BundleEncodeError, DiagnosticCode,
    ItemComponent, ItemStatus, RejectionCode, BUNDLE_CODEC_ID, BUNDLE_MAGIC,
    MAX_AUTH_BYTES_PER_BUNDLE, MAX_CAPABILITY_BYTES, MAX_ITEM_PAYLOAD_BYTES,
};
use riot_core::model::{Certainty, Severity, Urgency};
use riot_core::willow::{
    create_signed_alert_with, generate_communal_author_with, snapshot_from_unix_seconds,
    AlertDraft, ClockSnapshot, ClockSource, EntropySource, EvidenceAuthor, OsEntropy,
    SignedWillowEntry, WillowError,
};

// ---------- helpers ----------

struct CountingEntropy(u8);

impl EntropySource for CountingEntropy {
    fn fill(&mut self, buf: &mut [u8]) -> Result<(), WillowError> {
        for b in buf.iter_mut() {
            self.0 = self.0.wrapping_add(1);
            *b = self.0;
        }
        Ok(())
    }
}

struct FixedClock(ClockSnapshot);

impl ClockSource for FixedClock {
    fn snapshot(&self) -> Result<ClockSnapshot, WillowError> {
        Ok(self.0)
    }
}

fn snapshot() -> ClockSnapshot {
    snapshot_from_unix_seconds(1_783_000_000, 60).expect("valid instant")
}

fn draft(headline: &str) -> AlertDraft {
    AlertDraft {
        valid_from: None,
        expires_at: 1_800_000_000,
        language: "en".into(),
        urgency: Urgency::Immediate,
        severity: Severity::Severe,
        certainty: Certainty::Observed,
        headline: headline.into(),
        description: "Use the north route via Alder.".into(),
        affected_area_claim: None,
        source_claims: vec!["Two field observers".into()],
        ai_assisted: false,
    }
}

/// Deterministic author: fixed namespace secret seeds looped to a communal
/// key, fixed subspace secret. Stable across runs for the golden fixture.
fn deterministic_author() -> EvidenceAuthor {
    use willow25::entry::NamespaceSecret;
    let mut seed = *b"riot-golden-namespace-secret-01!";
    let namespace_id = loop {
        let candidate = NamespaceSecret::from_bytes(&seed).corresponding_namespace_id();
        if candidate.is_communal() {
            break candidate;
        }
        seed[31] = seed[31].wrapping_add(1);
    };
    EvidenceAuthor::from_parts_for_tests(namespace_id, b"riot-golden-subspace-secret-01!!")
}

fn random_signed_alert(headline: &str) -> SignedWillowEntry {
    let author = generate_communal_author_with(&mut OsEntropy).expect("entropy");
    create_signed_alert_with(
        &author,
        &mut OsEntropy,
        &FixedClock(snapshot()),
        draft(headline),
    )
    .expect("signs")
    .signed
}

/// Hand-frames arbitrary component bytes into the outer document — the
/// test-side hostile framer (release code has no unvalidated export path).
type RawParts<'a> = (&'a [u8], &'a [u8], &'a [u8], &'a [u8]);

fn frame_raw(items: &[RawParts<'_>]) -> Vec<u8> {
    let mut buffer: Vec<u8> = Vec::new();
    buffer.extend_from_slice(BUNDLE_MAGIC);
    let mut e = Encoder::new(&mut buffer);
    let r: Result<_, minicbor::encode::Error<core::convert::Infallible>> = (|| {
        e.map(2)?;
        e.u8(0)?.str(BUNDLE_CODEC_ID)?;
        e.u8(1)?.array(items.len() as u64)?;
        for (entry, cap, sig, payload) in items {
            e.map(4)?;
            e.u8(0)?.bytes(entry)?;
            e.u8(1)?.bytes(cap)?;
            e.u8(2)?.bytes(sig)?;
            e.u8(3)?.bytes(payload)?;
        }
        Ok(())
    })();
    r.expect("framing");
    buffer
}

fn parts(item: &SignedWillowEntry) -> RawParts<'_> {
    (
        &item.entry_bytes,
        &item.capability_bytes,
        &item.signature,
        &item.payload_bytes,
    )
}

fn expect_rejected(bytes: &[u8], code: RejectionCode) {
    match decode_bundle(bytes) {
        BundleDecodeOutcome::Rejected(rejection) => assert_eq!(
            rejection.code, code,
            "wrong rejection code (detail: {})",
            rejection.detail
        ),
        BundleDecodeOutcome::Decoded(_) => panic!("expected global rejection {code:?}"),
    }
}

fn expect_item_diagnostic(
    bytes: &[u8],
    index: usize,
    code: DiagnosticCode,
    component: ItemComponent,
) {
    match decode_bundle(bytes) {
        BundleDecodeOutcome::Decoded(decoded) => match &decoded.items[index].status {
            ItemStatus::Invalid(diagnostic) => {
                assert_eq!(diagnostic.code, code);
                assert_eq!(diagnostic.component, component);
            }
            ItemStatus::Valid(_) => panic!("item {index} unexpectedly valid"),
        },
        BundleDecodeOutcome::Rejected(rejection) => {
            panic!("expected item diagnostic, got global rejection {rejection:?}")
        }
    }
}

// ---------- deterministic bytes and golden fixture ----------

#[test]
fn public_bundle_golden_one_item_bytes_are_frozen() {
    let author = deterministic_author();
    let signed = create_signed_alert_with(
        &author,
        &mut CountingEntropy(0),
        &FixedClock(snapshot()),
        draft("Bridge at 4th St closed"),
    )
    .expect("signs")
    .signed;
    let encoded = encode_bundle(std::slice::from_ref(&signed)).expect("encodes");

    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/willow/bundle-golden-1.riot-evidence"
    );
    if std::env::var("RIOT_BLESS").as_deref() == Ok("1") {
        std::fs::write(path, &encoded).expect("bless golden bundle");
    }
    let frozen = std::fs::read(path).expect("golden bundle present — bless once and commit");
    assert_eq!(encoded, frozen, "deterministic bundle bytes drifted");

    match decode_bundle(&frozen) {
        BundleDecodeOutcome::Decoded(decoded) => {
            assert_eq!(decoded.items.len(), 1);
            assert!(matches!(decoded.items[0].status, ItemStatus::Valid(_)));
        }
        BundleDecodeOutcome::Rejected(r) => panic!("golden bundle rejected: {r:?}"),
    }
}

#[test]
fn public_bundle_roundtrip_and_reencode_identity() {
    let items = vec![random_signed_alert("alpha"), random_signed_alert("beta")];
    let encoded = encode_bundle(&items).expect("encodes");
    assert_eq!(&encoded[..6], BUNDLE_MAGIC);

    let BundleDecodeOutcome::Decoded(decoded) = decode_bundle(&encoded) else {
        panic!("valid bundle rejected");
    };
    assert_eq!(decoded.items.len(), 2);
    for (before, item) in items.iter().zip(decoded.items.iter()) {
        assert!(matches!(item.status, ItemStatus::Valid(_)));
        assert_eq!(item.frame.entry_bytes(), before.entry_bytes);
        assert_eq!(item.frame.payload_bytes(), before.payload_bytes);
    }
}

// ---------- global ceilings and fatal precedence ----------

#[test]
fn public_bundle_accepts_64_rejects_65_entries() {
    // 64 distinct real entries encode and fully verify.
    let items: Vec<SignedWillowEntry> = (0..64)
        .map(|i| random_signed_alert(&format!("h{i}")))
        .collect();
    let encoded = encode_bundle(&items).expect("64 entries encode");
    let BundleDecodeOutcome::Decoded(decoded) = decode_bundle(&encoded) else {
        panic!("64-entry bundle rejected");
    };
    assert_eq!(decoded.items.len(), 64);
    assert!(decoded
        .items
        .iter()
        .all(|i| matches!(i.status, ItemStatus::Valid(_))));

    // 65 rejects on encode, and a hand-framed 65 rejects on decode.
    let mut sixty_five = items.clone();
    sixty_five.push(random_signed_alert("h64"));
    assert!(encode_bundle(&sixty_five).is_err());
    let raw: Vec<RawParts<'_>> = sixty_five.iter().map(parts).collect();
    expect_rejected(&frame_raw(&raw), RejectionCode::TooManyEntries);
}

#[test]
fn public_bundle_rejects_oversized_before_any_parsing() {
    // 8 MiB + 1 of arbitrary bytes: size precedes even the magic check.
    let oversized = vec![0x41u8; 8 * 1024 * 1024 + 1];
    expect_rejected(&oversized, RejectionCode::TooLarge);

    // Exactly at the ceiling with wrong magic: size passes, magic fires —
    // proving the precedence order between the two.
    let at_limit = vec![0x41u8; 8 * 1024 * 1024];
    expect_rejected(&at_limit, RejectionCode::WrongMagic);
}

#[test]
fn public_bundle_rejects_wrong_magic_and_codec() {
    let one = random_signed_alert("solo");
    let good = encode_bundle(std::slice::from_ref(&one)).expect("encodes");

    let mut wrong_magic = good.clone();
    wrong_magic[0] = b'X';
    expect_rejected(&wrong_magic, RejectionCode::WrongMagic);

    // Unknown codec id: structurally canonical, semantically unsupported.
    let mut buffer: Vec<u8> = Vec::new();
    buffer.extend_from_slice(BUNDLE_MAGIC);
    let mut e = Encoder::new(&mut buffer);
    e.map(2).unwrap();
    e.u8(0).unwrap().str("org.riot.evidence-bundle/9").unwrap();
    e.u8(1).unwrap().array(0).unwrap();
    expect_rejected(&buffer, RejectionCode::UnsupportedCodec);
}

#[test]
fn public_bundle_rejects_malformed_outer_frames() {
    let one = random_signed_alert("solo");
    let (entry, cap, sig, payload) = parts(&one);

    // Unknown item key: replace key 3 with key 5.
    let mut unknown_key: Vec<u8> = Vec::new();
    unknown_key.extend_from_slice(BUNDLE_MAGIC);
    let mut e = Encoder::new(&mut unknown_key);
    e.map(2).unwrap();
    e.u8(0).unwrap().str(BUNDLE_CODEC_ID).unwrap();
    e.u8(1).unwrap().array(1).unwrap();
    e.map(4).unwrap();
    e.u8(0).unwrap().bytes(entry).unwrap();
    e.u8(1).unwrap().bytes(cap).unwrap();
    e.u8(2).unwrap().bytes(sig).unwrap();
    e.u8(5).unwrap().bytes(payload).unwrap();
    expect_rejected(&unknown_key, RejectionCode::MalformedFrame);

    // Indefinite-length items array.
    let mut indefinite: Vec<u8> = Vec::new();
    indefinite.extend_from_slice(BUNDLE_MAGIC);
    indefinite.push(0xa2); // map(2)
    indefinite.push(0x00);
    {
        let mut e = Encoder::new(&mut indefinite);
        e.str(BUNDLE_CODEC_ID).unwrap();
    }
    indefinite.push(0x01);
    indefinite.push(0x9f); // indefinite array
    indefinite.push(0xff); // break
    expect_rejected(&indefinite, RejectionCode::MalformedFrame);

    // Trailing bytes after a valid document.
    let good = encode_bundle(std::slice::from_ref(&one)).expect("encodes");
    let mut trailing = good.clone();
    trailing.push(0x00);
    expect_rejected(&trailing, RejectionCode::MalformedFrame);

    // Duplicate outer keys (0 twice).
    let mut dup: Vec<u8> = Vec::new();
    dup.extend_from_slice(BUNDLE_MAGIC);
    let mut e = Encoder::new(&mut dup);
    e.map(2).unwrap();
    e.u8(0).unwrap().str(BUNDLE_CODEC_ID).unwrap();
    e.u8(0).unwrap().str(BUNDLE_CODEC_ID).unwrap();
    expect_rejected(&dup, RejectionCode::MalformedFrame);
}

#[test]
fn public_bundle_rejects_non_shortest_outer_integers() {
    // Widen the first outer key 0x00 to 0x18 0x00: parses equal, re-framing
    // proves non-canonical.
    let one = random_signed_alert("solo");
    let good = encode_bundle(std::slice::from_ref(&one)).expect("encodes");
    let body_start = BUNDLE_MAGIC.len();
    assert_eq!(good[body_start], 0xa2);
    assert_eq!(good[body_start + 1], 0x00);
    let mut widened = Vec::with_capacity(good.len() + 1);
    widened.extend_from_slice(&good[..=body_start]);
    widened.extend_from_slice(&[0x18, 0x00]);
    widened.extend_from_slice(&good[body_start + 2..]);
    expect_rejected(&widened, RejectionCode::NonCanonicalFrame);
}

#[test]
fn public_bundle_enforces_cumulative_authorization_budget() {
    // 60 items, each with 40 KiB of fake capability bytes: item 0 alone is
    // under every per-item cap, but the running total crosses 2 MiB at item
    // ~52 during parse — before any expensive verification.
    let one = random_signed_alert("solo");
    let big_cap = vec![0xCC; 40 * 1024];
    let items: Vec<RawParts<'_>> = (0..60)
        .map(|_| {
            (
                one.entry_bytes.as_slice(),
                big_cap.as_slice(),
                one.signature.as_slice(),
                one.payload_bytes.as_slice(),
            )
        })
        .collect();
    expect_rejected(
        &frame_raw(&items),
        RejectionCode::AuthorizationBudgetExceeded,
    );
}

#[test]
fn public_bundle_rejects_duplicate_entry_ids_globally() {
    let one = random_signed_alert("solo");
    let p = parts(&one);
    expect_rejected(&frame_raw(&[p, p]), RejectionCode::DuplicateEntryId);
}

// ---------- item-scoped diagnostics and sibling isolation ----------

#[test]
fn public_bundle_flags_signature_length_63_and_65() {
    let one = random_signed_alert("solo");
    let (entry, cap, sig, payload) = parts(&one);

    let short = &sig[..63];
    let bytes = frame_raw(&[(entry, cap, short, payload)]);
    expect_item_diagnostic(
        &bytes,
        0,
        DiagnosticCode::BadSignatureLength,
        ItemComponent::Signature,
    );

    let mut long = sig.to_vec();
    long.push(0);
    let bytes = frame_raw(&[(entry, cap, &long, payload)]);
    expect_item_diagnostic(
        &bytes,
        0,
        DiagnosticCode::BadSignatureLength,
        ItemComponent::Signature,
    );
}

#[test]
fn public_bundle_flags_noncanonical_component_bytes() {
    let one = random_signed_alert("solo");
    let (entry, cap, sig, payload) = parts(&one);

    let mut bad_entry = entry.to_vec();
    bad_entry.push(0x00);
    let bytes = frame_raw(&[(&bad_entry, cap, sig, payload)]);
    expect_item_diagnostic(
        &bytes,
        0,
        DiagnosticCode::NonCanonicalEntry,
        ItemComponent::Entry,
    );

    let mut bad_cap = cap.to_vec();
    bad_cap.push(0x00);
    let bytes = frame_raw(&[(entry, &bad_cap, sig, payload)]);
    expect_item_diagnostic(
        &bytes,
        0,
        DiagnosticCode::NonCanonicalCapability,
        ItemComponent::Capability,
    );
}

#[test]
fn public_bundle_flags_payload_mismatches() {
    let one = random_signed_alert("solo");
    let two = random_signed_alert("twin"); // same headline length: equal payload size
    let (entry, cap, sig, payload) = parts(&one);

    // Wrong length.
    let truncated = &payload[..payload.len() - 1];
    let bytes = frame_raw(&[(entry, cap, sig, truncated)]);
    expect_item_diagnostic(
        &bytes,
        0,
        DiagnosticCode::PayloadLengthMismatch,
        ItemComponent::Payload,
    );

    // Right length (same draft shape, different ids → same length), wrong digest.
    assert_eq!(payload.len(), two.payload_bytes.len());
    let bytes = frame_raw(&[(entry, cap, sig, &two.payload_bytes)]);
    expect_item_diagnostic(
        &bytes,
        0,
        DiagnosticCode::PayloadDigestMismatch,
        ItemComponent::Payload,
    );
}

#[test]
fn public_bundle_flags_invalid_authorization_and_isolates_siblings() {
    let good = random_signed_alert("good");
    let bad = random_signed_alert("bad");
    let mut forged_sig = bad.signature;
    forged_sig[10] ^= 0x01;

    let bytes = frame_raw(&[
        parts(&good),
        (
            &bad.entry_bytes,
            &bad.capability_bytes,
            &forged_sig,
            &bad.payload_bytes,
        ),
    ]);
    let BundleDecodeOutcome::Decoded(decoded) = decode_bundle(&bytes) else {
        panic!("item failure must not reject the artifact");
    };
    assert!(
        matches!(decoded.items[0].status, ItemStatus::Valid(_)),
        "valid sibling must stay valid"
    );
    match &decoded.items[1].status {
        ItemStatus::Invalid(d) => {
            assert_eq!(d.code, DiagnosticCode::DoesNotAuthorise);
            assert_eq!(d.component, ItemComponent::Authorization);
        }
        ItemStatus::Valid(_) => panic!("forged signature accepted"),
    }
}

#[test]
fn public_bundle_precedence_noncanonical_beats_unsupported_codec() {
    // A document that is BOTH non-canonical (non-shortest outer key 0) AND
    // carries an unsupported codec must report NonCanonicalFrame — canonical
    // framing is judged before the codec value.
    let one = random_signed_alert("solo");
    let (entry, cap, sig, payload) = parts(&one);

    let mut buffer: Vec<u8> = Vec::new();
    buffer.extend_from_slice(BUNDLE_MAGIC);
    buffer.push(0xa2); // map(2)
    buffer.extend_from_slice(&[0x18, 0x00]); // non-shortest key 0
    {
        let mut e = Encoder::new(&mut buffer);
        e.str("org.riot.evidence-bundle/9").unwrap(); // unsupported codec
    }
    buffer.push(0x01);
    buffer.push(0x81); // array(1)
    {
        let mut e = Encoder::new(&mut buffer);
        e.map(4).unwrap();
        e.u8(0).unwrap().bytes(entry).unwrap();
        e.u8(1).unwrap().bytes(cap).unwrap();
        e.u8(2).unwrap().bytes(sig).unwrap();
        e.u8(3).unwrap().bytes(payload).unwrap();
    }
    expect_rejected(&buffer, RejectionCode::NonCanonicalFrame);
}

#[test]
fn public_bundle_precedence_codec_beats_limits() {
    // Canonical framing, unsupported codec, AND 65 entries: codec wins over
    // the entry-count limit.
    let one = random_signed_alert("solo");
    let (entry, cap, sig, payload) = parts(&one);
    let mut buffer: Vec<u8> = Vec::new();
    buffer.extend_from_slice(BUNDLE_MAGIC);
    let mut e = Encoder::new(&mut buffer);
    e.map(2).unwrap();
    e.u8(0).unwrap().str("org.riot.evidence-bundle/9").unwrap();
    e.u8(1).unwrap().array(65).unwrap();
    for _ in 0..65 {
        e.map(4).unwrap();
        e.u8(0).unwrap().bytes(entry).unwrap();
        e.u8(1).unwrap().bytes(cap).unwrap();
        e.u8(2).unwrap().bytes(sig).unwrap();
        e.u8(3).unwrap().bytes(payload).unwrap();
    }
    expect_rejected(&buffer, RejectionCode::UnsupportedCodec);
}

#[test]
fn public_bundle_rejects_invalid_utf8_codec() {
    // Invalid UTF-8 where the codec text should be is a malformed outer
    // frame (minicbor's str() rejects non-UTF-8).
    let mut buffer: Vec<u8> = Vec::new();
    buffer.extend_from_slice(BUNDLE_MAGIC);
    buffer.push(0xa2); // map(2)
    buffer.push(0x00); // key 0
    buffer.push(0x62); // text string len 2
    buffer.push(0xff);
    buffer.push(0xfe); // invalid UTF-8
    buffer.push(0x01);
    buffer.push(0x80); // array(0)
    expect_rejected(&buffer, RejectionCode::MalformedFrame);
}

#[test]
fn public_bundle_rejects_indefinite_byte_string_field() {
    // An item field encoded as an indefinite/chunked byte string is malformed.
    let one = random_signed_alert("solo");
    let (entry, cap, sig, _payload) = parts(&one);
    let mut buffer: Vec<u8> = Vec::new();
    buffer.extend_from_slice(BUNDLE_MAGIC);
    {
        let mut e = Encoder::new(&mut buffer);
        e.map(2).unwrap();
        e.u8(0).unwrap().str(BUNDLE_CODEC_ID).unwrap();
        e.u8(1).unwrap().array(1).unwrap();
        e.map(4).unwrap();
        e.u8(0).unwrap().bytes(entry).unwrap();
        e.u8(1).unwrap().bytes(cap).unwrap();
        e.u8(2).unwrap().bytes(sig).unwrap();
        e.u8(3).unwrap(); // key 3; value written raw below
    }
    buffer.push(0x5f); // indefinite byte string
    buffer.push(0x41);
    buffer.push(0x00);
    buffer.push(0xff); // break
    expect_rejected(&buffer, RejectionCode::MalformedFrame);
}

#[test]
fn public_bundle_flags_entry_bytes_over_4kib() {
    // Entry bytes above the frozen 4 KiB Entry ceiling reject globally,
    // distinct from the 64 KiB capability limit.
    let one = random_signed_alert("solo");
    let (_entry, cap, sig, payload) = parts(&one);
    let oversized_entry = vec![0u8; 4097];
    let bytes = frame_raw(&[(&oversized_entry, cap, sig, payload)]);
    expect_rejected(&bytes, RejectionCode::EntryBytesExceeded);
}

#[test]
fn public_bundle_accepts_valid_within_size_and_rejects_one_over() {
    let one = random_signed_alert("solo");
    let encoded = encode_bundle(std::slice::from_ref(&one)).expect("encodes");
    assert!(encoded.len() <= 8 * 1024 * 1024);
    let BundleDecodeOutcome::Decoded(_) = decode_bundle(&encoded) else {
        panic!("valid bundle within size limit rejected");
    };
    let mut oversized = encoded.clone();
    oversized.resize(8 * 1024 * 1024 + 1, 0);
    expect_rejected(&oversized, RejectionCode::TooLarge);
}

#[test]
fn public_bundle_whole_outcome_debug_never_leaks_payload_bytes() {
    // Formatting the ENTIRE decoded outcome (not just ItemStatus) must not
    // expose payload bytes, ascii or decimal.
    use riot_core::willow::{authorise_entry, build_alert_entry, encode_capability, encode_entry};
    let author = generate_communal_author_with(&mut OsEntropy).expect("entropy");
    let marker: &[u8] = b"HOSTILE-MARKER-PAYLOAD-BYTES-0102030405";
    let entry = build_alert_entry(&author, &[1u8; 16], &[2u8; 16], 836_179_200_000_000, marker)
        .expect("builds");
    let authorised = authorise_entry(&author, entry).expect("authorises");
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    let bytes = frame_raw(&[(
        &encode_entry(authorised.entry()),
        &encode_capability(token.capability()),
        &signature.to_bytes(),
        marker,
    )]);

    let rendered = format!("{:?}", decode_bundle(&bytes));
    assert!(!rendered.contains("HOSTILE-MARKER"), "ascii marker leaked");
    let decimal: String = marker
        .iter()
        .map(|b| b.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    assert!(!rendered.contains(&decimal), "decimal byte sequence leaked");
}

#[test]
fn public_bundle_flags_unsupported_schema() {
    // A real authorised entry over a payload that is not a Riot alert:
    // crypto verifies, schema fails.
    use riot_core::willow::{authorise_entry, build_alert_entry, encode_capability, encode_entry};
    let author = generate_communal_author_with(&mut OsEntropy).expect("entropy");
    let payload = b"HOSTILE-MARKER-not-an-alert".to_vec();
    let entry = build_alert_entry(
        &author,
        &[9u8; 16],
        &[10u8; 16],
        836_179_200_000_000,
        &payload,
    )
    .expect("builds");
    let authorised = authorise_entry(&author, entry).expect("authorises");
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();

    let bytes = frame_raw(&[(
        &encode_entry(authorised.entry()),
        &encode_capability(token.capability()),
        &signature.to_bytes(),
        &payload,
    )]);
    expect_item_diagnostic(
        &bytes,
        0,
        DiagnosticCode::UnsupportedSchema,
        ItemComponent::Schema,
    );

    // Sanitization: the hostile payload marker never appears in the
    // diagnostic's debug rendering.
    let BundleDecodeOutcome::Decoded(decoded) = decode_bundle(&bytes) else {
        panic!("decoded expected");
    };
    let rendered = format!("{:?}", decoded.items[0].status);
    assert!(
        !rendered.contains("HOSTILE-MARKER"),
        "diagnostics must not embed untrusted bytes"
    );
}

#[test]
fn public_bundle_empty_bundle_and_all_frame_accessors_round_trip() {
    let empty = encode_bundle(&[]).expect("zero entries are valid");
    let BundleDecodeOutcome::Decoded(decoded) = decode_bundle(&empty) else {
        panic!("empty canonical bundle rejected");
    };
    assert!(decoded.items.is_empty());

    let one = random_signed_alert("accessors");
    let encoded = encode_bundle(std::slice::from_ref(&one)).unwrap();
    let BundleDecodeOutcome::Decoded(decoded) = decode_bundle(&encoded) else {
        panic!("valid bundle rejected");
    };
    let frame = &decoded.items[0].frame;
    assert_eq!(frame.capability_bytes(), one.capability_bytes);
    assert_eq!(frame.signature_bytes(), one.signature);
}

#[test]
fn public_bundle_encode_reports_each_preflight_and_framed_size_error() {
    let one = random_signed_alert("encode failures");

    let mut invalid = one.clone();
    invalid.signature[0] ^= 1;
    assert!(matches!(
        encode_bundle(&[invalid]),
        Err(BundleEncodeError::InvalidItem(_))
    ));

    let mut excessive_authorization = one.clone();
    excessive_authorization.capability_bytes = vec![0; MAX_AUTH_BYTES_PER_BUNDLE + 1];
    assert_eq!(
        encode_bundle(&[excessive_authorization]),
        Err(BundleEncodeError::AuthorizationBudgetExceeded)
    );

    use riot_core::apps::entry::app_data_path;
    use riot_core::willow::{authorise_entry, encode_capability, encode_entry, Entry};
    let author = deterministic_author();
    let payload = vec![0x5a; MAX_ITEM_PAYLOAD_BYTES];
    let mut large_items = Vec::new();
    for index in 0..9u8 {
        let path = app_data_path(&[0x55; 32], &format!("large/{index}")).unwrap();
        let entry = Entry::builder()
            .namespace_id(author.namespace_id().clone())
            .subspace_id(author.subspace_id())
            .path(path)
            .timestamp(index as u64)
            .payload(&payload)
            .build();
        let authorised = authorise_entry(&author, entry).unwrap();
        let token = authorised.authorisation_token();
        let signature: ed25519_dalek::Signature = token.signature().clone().into();
        large_items.push(SignedWillowEntry {
            entry_bytes: encode_entry(authorised.entry()),
            capability_bytes: encode_capability(token.capability()),
            signature: signature.to_bytes(),
            payload_bytes: payload.clone(),
        });
    }
    assert_eq!(
        encode_bundle(&large_items),
        Err(BundleEncodeError::BundleTooLarge)
    );
}

#[test]
fn public_bundle_capability_and_payload_ceilings_accept_exact_reject_one_over() {
    let one = random_signed_alert("field ceilings");
    let (entry, _cap, sig, payload) = parts(&one);

    let capability_at_limit = vec![0; MAX_CAPABILITY_BYTES];
    assert!(matches!(
        decode_bundle(&frame_raw(&[(entry, &capability_at_limit, sig, payload)])),
        BundleDecodeOutcome::Decoded(_)
    ));
    let capability_over = vec![0; MAX_CAPABILITY_BYTES + 1];
    expect_rejected(
        &frame_raw(&[(entry, &capability_over, sig, payload)]),
        RejectionCode::CapabilityBytesExceeded,
    );

    let payload_at_limit = vec![0; MAX_ITEM_PAYLOAD_BYTES];
    assert!(matches!(
        decode_bundle(&frame_raw(&[(
            entry,
            one.capability_bytes.as_slice(),
            sig,
            &payload_at_limit
        )])),
        BundleDecodeOutcome::Decoded(_)
    ));
    let payload_over = vec![0; MAX_ITEM_PAYLOAD_BYTES + 1];
    expect_rejected(
        &frame_raw(&[(entry, one.capability_bytes.as_slice(), sig, &payload_over)]),
        RejectionCode::PayloadBytesExceeded,
    );
}

#[test]
fn public_bundle_rejects_every_remaining_outer_shape_error() {
    let bodies: Vec<Vec<u8>> = vec![
        vec![],
        vec![0xa1, 0x00, 0x60],
        vec![0xa2, 0x02],
        vec![0xa2, 0x00, 0x60, 0x01, 0x9a, 0x00, 0x80, 0x00, 0x01],
        vec![0xa2, 0x00, 0x60, 0x01, 0x81, 0xbf, 0xff],
        vec![0xa2, 0x00, 0x60, 0x01, 0x81, 0xa3],
    ];
    for body in bodies {
        let mut bytes = BUNDLE_MAGIC.to_vec();
        bytes.extend_from_slice(&body);
        expect_rejected(&bytes, RejectionCode::MalformedFrame);
    }
}

#[test]
fn public_bundle_rejects_non_byte_values_in_each_early_item_field() {
    let one = random_signed_alert("wrong field types");
    let fields = [
        one.entry_bytes.as_slice(),
        one.capability_bytes.as_slice(),
        one.signature.as_slice(),
        one.payload_bytes.as_slice(),
    ];
    for wrong_index in 0..3usize {
        let mut bytes = BUNDLE_MAGIC.to_vec();
        let mut encoder = Encoder::new(&mut bytes);
        encoder.map(2).unwrap();
        encoder.u8(0).unwrap().str(BUNDLE_CODEC_ID).unwrap();
        encoder.u8(1).unwrap().array(1).unwrap();
        encoder.map(4).unwrap();
        for (index, field) in fields.iter().enumerate() {
            encoder.u8(index as u8).unwrap();
            if index == wrong_index {
                encoder.u8(7).unwrap();
            } else {
                encoder.bytes(field).unwrap();
            }
        }
        expect_rejected(&bytes, RejectionCode::MalformedFrame);
    }
}

// ---------- owned-namespace admission (Unit 1 Task 1) ----------

/// Encoded component bytes for one hand-framed item, owned so `frame_raw`'s
/// borrowed `RawParts` can reference them.
struct OwnedFrameParts {
    entry: Vec<u8>,
    capability: Vec<u8>,
    signature: [u8; 64],
    payload: Vec<u8>,
}

impl OwnedFrameParts {
    fn as_parts(&self) -> RawParts<'_> {
        (
            &self.entry,
            &self.capability,
            &self.signature,
            &self.payload,
        )
    }
}

/// A valid, canonical Riot alert payload (reused so a non-alert-path owned
/// entry still satisfies the schema check — admission is schema-only, and any
/// path that is not a reserved prefix must carry a decodable alert).
fn alert_payload_bytes() -> Vec<u8> {
    random_signed_alert("owned editorial payload").payload_bytes
}

/// Frame an entry authorised by `cap`/`secret` into hand-framed component bytes.
fn owned_frame(
    entry: riot_core::willow::Entry,
    cap: &willow25::prelude::WriteCapability,
    secret: &willow25::prelude::SubspaceSecret,
    payload: Vec<u8>,
) -> OwnedFrameParts {
    use riot_core::willow::{encode_capability, encode_entry};
    let authorised = entry
        .into_authorised_entry(cap, secret)
        .expect("cap authorises the entry");
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    OwnedFrameParts {
        entry: encode_entry(authorised.entry()),
        capability: encode_capability(token.capability()),
        signature: signature.to_bytes(),
        payload,
    }
}

fn owned_entry_at(
    namespace: &willow25::prelude::NamespaceId,
    subspace: willow25::prelude::SubspaceId,
    path: &[&[u8]],
    payload: &[u8],
) -> riot_core::willow::Entry {
    use riot_core::willow::{Entry, Path};
    Entry::builder()
        .namespace_id(namespace.clone())
        .subspace_id(subspace)
        .path(Path::from_slices(path).expect("path"))
        .timestamp(1_000_000u64)
        .payload(payload)
        .build()
}

#[test]
fn public_bundle_admits_valid_owned_editorial_and_owner_entries() {
    use riot_core::willow::{OwnedMasthead, ARTICLES_COMPONENT};
    use willow25::prelude::{Area, Path, SubspaceSecret, TimeRange};

    let masthead = OwnedMasthead::generate().expect("masthead");

    // --- Editor-delegated entry under /articles/news ---
    let editor = SubspaceSecret::from_bytes(&[0x2b; 32]);
    let editor_id = editor.corresponding_subspace_id();
    let area = Area::new(
        Some(editor_id.clone()),
        Path::from_slices(&[ARTICLES_COMPONENT, b"news"]).expect("path"),
        TimeRange::new(0u64.into(), Some(u64::MAX.into())),
    );
    let editor_cap = masthead
        .delegate_section(editor_id.clone(), area)
        .expect("delegate section");

    let article_payload = alert_payload_bytes();
    let article = owned_entry_at(
        masthead.namespace_id(),
        editor_id,
        &[ARTICLES_COMPONENT, b"news", b"post-1"],
        &article_payload,
    );
    let editorial = owned_frame(article, &editor_cap, &editor, article_payload.clone());

    let bytes = frame_raw(&[editorial.as_parts()]);
    let BundleDecodeOutcome::Decoded(decoded) = decode_bundle(&bytes) else {
        panic!("valid owned editorial entry must not reject the artifact");
    };
    assert!(
        matches!(decoded.items[0].status, ItemStatus::Valid(_)),
        "valid owned editorial (delegated) entry must be ADMITTED"
    );

    // --- Owner-authored entry under the owner's own subspace ---
    let owner_payload = alert_payload_bytes();
    let owner_entry = owned_entry_at(
        masthead.namespace_id(),
        masthead.owner_subspace_id(),
        &[ARTICLES_COMPONENT, b"news", b"owner-post"],
        &owner_payload,
    );
    let owner_authorised = masthead
        .authorise_owner_entry(owner_entry)
        .expect("owner authorises");
    let owner_token = owner_authorised.authorisation_token();
    let owner_sig: ed25519_dalek::Signature = owner_token.signature().clone().into();
    let owner_parts = OwnedFrameParts {
        entry: riot_core::willow::encode_entry(owner_authorised.entry()),
        capability: riot_core::willow::encode_capability(owner_token.capability()),
        signature: owner_sig.to_bytes(),
        payload: owner_payload,
    };
    let bytes = frame_raw(&[owner_parts.as_parts()]);
    let BundleDecodeOutcome::Decoded(decoded) = decode_bundle(&bytes) else {
        panic!("valid owner-authored owned entry must not reject the artifact");
    };
    assert!(
        matches!(decoded.items[0].status, ItemStatus::Valid(_)),
        "valid owner-authored owned entry must be ADMITTED"
    );
}

#[test]
fn public_bundle_rejects_communal_marker_bit_forgery_over_owned_namespace() {
    // The load-bearing invariant: `NamespaceId::is_owned()` is only the LSB
    // marker bit. A COMMUNAL genesis capability is unconditionally valid and
    // can NAME an owned namespace id — willow25 would happily verify it. Only
    // the `capability.is_owned()` gate stops the forgery. Build the hostile
    // cap the raw willow25 way, exactly as a malicious peer would.
    use willow25::prelude::{NamespaceSecret, SubspaceSecret, WriteCapability};

    // An owned namespace id the attacker does NOT control the root secret for.
    let mut owned_seed = [0x44; 32];
    let owned_namespace = loop {
        let candidate = NamespaceSecret::from_bytes(&owned_seed).corresponding_namespace_id();
        if candidate.is_owned() {
            break candidate;
        }
        owned_seed[0] = owned_seed[0].wrapping_add(1);
    };

    let attacker = SubspaceSecret::from_bytes(&[0x99; 32]);
    let attacker_id = attacker.corresponding_subspace_id();
    // A communal genesis cap that NAMES the owned namespace — is_owned() == false.
    let forged = WriteCapability::new_communal(owned_namespace.clone(), attacker_id.clone());
    assert!(
        !forged.is_owned(),
        "a communal genesis cap is never is_owned(), even naming an owned namespace"
    );

    let payload = alert_payload_bytes();
    let entry = owned_entry_at(
        &owned_namespace,
        attacker_id,
        &[riot_core::willow::ARTICLES_COMPONENT, b"news", b"forged"],
        &payload,
    );
    let forged_parts = owned_frame(entry, &forged, &attacker, payload);

    let bytes = frame_raw(&[forged_parts.as_parts()]);
    expect_item_diagnostic(
        &bytes,
        0,
        DiagnosticCode::UnsupportedCapability,
        ItemComponent::Authorization,
    );
}

#[test]
fn public_bundle_rejects_each_unsupported_capability_form() {
    use riot_core::willow::{encode_capability, encode_entry, Entry, Path};
    use willow25::prelude::{Area, NamespaceSecret, SubspaceSecret, WriteCapability};

    let author = deterministic_author();
    let valid = random_signed_alert("capability forms");

    let mut owned_seed = [0x33; 32];
    let (owned_secret, owned_namespace) = loop {
        let secret = NamespaceSecret::from_bytes(&owned_seed);
        let namespace = secret.corresponding_namespace_id();
        if namespace.is_owned() {
            break (secret, namespace);
        }
        owned_seed[0] = owned_seed[0].wrapping_add(1);
    };
    let owned = WriteCapability::new_owned(&owned_secret, author.subspace_id());
    expect_item_diagnostic(
        &frame_raw(&[(
            &valid.entry_bytes,
            &encode_capability(&owned),
            &valid.signature,
            &valid.payload_bytes,
        )]),
        0,
        DiagnosticCode::UnsupportedCapability,
        ItemComponent::Authorization,
    );

    let mut delegated = author.write_capability();
    let signer = SubspaceSecret::from_bytes(b"riot-golden-subspace-secret-01!!");
    let receiver = SubspaceSecret::from_bytes(&[0x77; 32]).corresponding_subspace_id();
    delegated.delegate(
        &signer,
        Area::new_subspace_area(author.subspace_id()),
        receiver,
    );
    expect_item_diagnostic(
        &frame_raw(&[(
            &valid.entry_bytes,
            &encode_capability(&delegated),
            &valid.signature,
            &valid.payload_bytes,
        )]),
        0,
        DiagnosticCode::UnsupportedCapability,
        ItemComponent::Authorization,
    );

    let path = Path::from_slices(&[b"objects", b"alert", &[1; 16], &[2; 16]]).unwrap();
    let foreign_entry = Entry::builder()
        .namespace_id(owned_namespace)
        .subspace_id(author.subspace_id())
        .path(path)
        .timestamp(1u64)
        .payload(&valid.payload_bytes)
        .build();
    let communal = author.write_capability();
    expect_item_diagnostic(
        &frame_raw(&[(
            &encode_entry(&foreign_entry),
            &encode_capability(&communal),
            &[0; 64],
            &valid.payload_bytes,
        )]),
        0,
        DiagnosticCode::UnsupportedCapability,
        ItemComponent::Authorization,
    );
}
