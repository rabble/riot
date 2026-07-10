//! G1 evidence: RiotEvidenceBundleV1 — deterministic framing, hard global
//! and cumulative ceilings, frozen fatal precedence, sibling isolation, and
//! sanitized structured diagnostics.

use minicbor::Encoder;
use riot_core::import::{
    decode_bundle, encode_bundle, BundleDecodeOutcome, DiagnosticCode, ItemComponent, ItemStatus,
    RejectionCode, BUNDLE_CODEC_ID, BUNDLE_MAGIC,
};
use riot_core::model::{Certainty, Severity, Urgency};
use riot_core::willow::{
    create_signed_alert, generate_communal_author, snapshot_from_unix_seconds, AlertDraft,
    ClockSnapshot, ClockSource, EntropySource, EvidenceAuthor, OsEntropy, SignedWillowEntry,
    WillowError,
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
    let author = generate_communal_author(&mut OsEntropy).expect("entropy");
    create_signed_alert(
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
    let signed = create_signed_alert(
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
fn public_bundle_flags_unsupported_schema() {
    // A real authorised entry over a payload that is not a Riot alert:
    // crypto verifies, schema fails.
    use riot_core::willow::{authorise_entry, build_alert_entry, encode_capability, encode_entry};
    let author = generate_communal_author(&mut OsEntropy).expect("entropy");
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
