//! G1 evidence: communal Willow authority, one-snapshot clock separation,
//! fallible author/entry factories, canonical component bytes, digest
//! domains, and the fixed alert path layout.

use riot_core::model::{encode_alert, Certainty, Severity, Urgency};
use riot_core::willow::{
    alert_path, authorise_entry, build_alert_entry, create_signed_alert, decode_capability_canonic,
    decode_entry_canonic, encode_capability, encode_entry, entry_id, evidence_digest,
    generate_communal_author, snapshot_from_unix_seconds, verify_entry, william3_digest,
    AlertDraft, ClockSnapshot, ClockSource, EntropySource, EntryFacts, EvidenceAuthor,
    NamespaceKind, OsEntropy, WillowError,
};
use willow25::entry::Entrylike;
use willow25::groupings::{Coordinatelike, Keylike};

const OBJECT_ID: [u8; 16] = *b"riot-obj-0000001";
const REVISION_ID: [u8; 16] = *b"riot-rev-0000001";
const WILLOW_TS_MICROS: u64 = 836_179_200_000_000;

fn author() -> EvidenceAuthor {
    generate_communal_author(&mut OsEntropy).expect("os entropy available")
}

fn canonical_payload() -> Vec<u8> {
    std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/objects/alert-golden-1.cbor"
    ))
    .expect("alert golden vector present")
}

/// Test-only failing entropy: fails after `works_for` successful fills.
struct FailingEntropy {
    works_for: usize,
}

impl EntropySource for FailingEntropy {
    fn fill(&mut self, buf: &mut [u8]) -> Result<(), WillowError> {
        if self.works_for == 0 {
            return Err(WillowError::EntropyUnavailable);
        }
        self.works_for -= 1;
        // Deterministic non-zero bytes; test-only.
        for (i, b) in buf.iter_mut().enumerate() {
            *b = (i as u8).wrapping_add(7);
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

struct BrokenClock;

impl ClockSource for BrokenClock {
    fn snapshot(&self) -> Result<ClockSnapshot, WillowError> {
        Err(WillowError::ClockUnavailable)
    }
}

fn draft() -> AlertDraft {
    AlertDraft {
        valid_from: None,
        expires_at: 1_800_000_000,
        language: "en".into(),
        urgency: Urgency::Immediate,
        severity: Severity::Severe,
        certainty: Certainty::Observed,
        headline: "Bridge at 4th St closed".into(),
        description: "Use the north route via Alder.".into(),
        affected_area_claim: None,
        source_claims: vec!["Two field observers".into()],
        ai_assisted: false,
    }
}

// ---------- identity and entropy ----------

#[test]
fn public_author_identity_exposes_full_ids_and_communal_kind() {
    let author = author();
    let identity = author.identity();
    assert_eq!(identity.namespace_kind, NamespaceKind::Communal);
    assert_eq!(identity.namespace_id.len(), 32);
    assert_eq!(identity.subspace_id.len(), 32);
    assert_eq!(
        identity.signing_key_id, identity.subspace_id,
        "signing key id is the same public identity as the subspace"
    );
    assert_eq!(
        identity.namespace_id[31] % 2,
        0,
        "communal namespace has an even final byte"
    );
    assert!(author.namespace_id().is_communal());
}

#[test]
fn public_author_generation_fails_closed_without_entropy() {
    let result = generate_communal_author(&mut FailingEntropy { works_for: 0 });
    assert!(
        matches!(result, Err(WillowError::EntropyUnavailable)),
        "entropy failure must return ENTROPY_UNAVAILABLE and construct no author"
    );
}

// ---------- clock ----------

#[test]
fn public_clock_snapshot_separates_utc_and_tai_views() {
    // 2026-07-10T00:00:00Z-ish: both views come from one reading and differ
    // by the TAI/J2000 conversion, not by a second clock read.
    let snapshot = snapshot_from_unix_seconds(1_783_000_000, 60).expect("valid instant");
    assert_eq!(snapshot.unix_seconds, 1_783_000_000);
    assert_eq!(snapshot.uncertainty_seconds, 60);
    // J2000 is 2000-01-01; unix 1_783_000_000 is ~2026. The TAI micros value
    // must land within ±1 day of (unix - J2000_unix) seconds in micros.
    // The exact epoch/leap-second arithmetic is delegated to the pinned
    // willow25 + hifitime conversion; this asserts the value is in the
    // right ~26-year range (±2 days of a naive J2000 subtraction), i.e.
    // seconds were not confused with micros or a second clock read.
    let j2000_unix_approx = 946_728_000u64;
    let expected_micros = (1_783_000_000u64 - j2000_unix_approx) * 1_000_000;
    let two_days_micros = 2 * 86_400_000_000u64;
    assert!(
        snapshot.tai_j2000_micros.abs_diff(expected_micros) < two_days_micros,
        "TAI/J2000 micros far from expectation: {}",
        snapshot.tai_j2000_micros
    );
}

#[test]
fn public_clock_rejects_pre_epoch_and_out_of_range() {
    // Before Unix epoch.
    assert!(matches!(
        snapshot_from_unix_seconds(-1, 0),
        Err(WillowError::ClockUnavailable)
    ));
    // After Unix epoch but before J2000: the Willow timestamp cannot
    // represent it, so the conversion must fail closed.
    assert!(matches!(
        snapshot_from_unix_seconds(0, 0),
        Err(WillowError::ClockUnavailable)
    ));
}

// ---------- signed alert entry factory ----------

#[test]
fn public_signed_alert_uses_one_snapshot_for_both_time_views() {
    let author = author();
    let snapshot = snapshot_from_unix_seconds(1_783_000_000, 60).expect("valid instant");
    let signed = create_signed_alert(&author, &mut OsEntropy, &FixedClock(snapshot), draft())
        .expect("factory succeeds");

    assert_eq!(signed.payload.created_at, snapshot.unix_seconds);
    assert_eq!(signed.snapshot, snapshot);

    let entry = decode_entry_canonic(&signed.signed.entry_bytes).expect("entry decodes");
    assert_eq!(u64::from(entry.timestamp()), snapshot.tai_j2000_micros);
    assert_eq!(
        entry.path(),
        &alert_path(&signed.object_id, &signed.revision_id).unwrap()
    );

    // The payload bytes are the canonical alert encoding of the payload.
    assert_eq!(
        encode_alert(&signed.payload).expect("re-encode"),
        signed.signed.payload_bytes
    );
    // Entry digest commits to those exact bytes.
    assert_eq!(
        entry.payload_digest_bytes(),
        william3_digest(&signed.signed.payload_bytes)
    );
}

#[test]
fn public_signed_alert_fails_closed_on_clock_and_entropy() {
    let author = author();
    assert!(matches!(
        create_signed_alert(&author, &mut OsEntropy, &BrokenClock, draft()),
        Err(WillowError::ClockUnavailable)
    ));
    assert!(matches!(
        create_signed_alert(
            &author,
            &mut FailingEntropy { works_for: 1 },
            &BrokenClock,
            draft()
        ),
        Err(WillowError::EntropyUnavailable) | Err(WillowError::ClockUnavailable)
    ));
    // Draft validity failure surfaces as InvalidAlert: expiry before the
    // snapshot-derived created_at.
    let snapshot = snapshot_from_unix_seconds(1_783_000_000, 60).unwrap();
    let mut bad = draft();
    bad.expires_at = 1_000; // long before created_at
    assert!(matches!(
        create_signed_alert(&author, &mut OsEntropy, &FixedClock(snapshot), bad),
        Err(WillowError::InvalidAlert(_))
    ));
}

// ---------- authority ----------

#[test]
fn public_communal_author_authorises_own_subspace() {
    let author = author();
    let payload = canonical_payload();
    let entry = build_alert_entry(
        &author,
        &OBJECT_ID,
        &REVISION_ID,
        WILLOW_TS_MICROS,
        &payload,
    )
    .expect("entry builds");
    let authorised =
        authorise_entry(&author, entry.clone()).expect("own-subspace write authorises");
    assert!(verify_entry(&entry, authorised.authorisation_token()));
}

#[test]
fn public_cross_subspace_denial_within_one_namespace() {
    // Two subspaces under the SAME communal namespace: this is the area
    // restriction Meadowcap must enforce. Two independently generated
    // namespaces would not prove it.
    let alice = author();
    let bob = EvidenceAuthor::from_parts_for_tests(
        alice.namespace_id().clone(),
        b"bob-secret-key-32-bytes-long!!!!",
    );
    assert_eq!(
        alice.namespace_id(),
        bob.namespace_id(),
        "both authors share one namespace"
    );
    assert_ne!(alice.subspace_id(), bob.subspace_id());

    let payload = canonical_payload();

    // Bob cannot mint a token for an entry in Alice's subspace.
    let alices_entry =
        build_alert_entry(&alice, &OBJECT_ID, &REVISION_ID, WILLOW_TS_MICROS, &payload)
            .expect("entry builds");
    assert!(matches!(
        authorise_entry(&bob, alices_entry.clone()),
        Err(WillowError::DoesNotAuthorise)
    ));

    // Alice's token does not verify an entry in Bob's subspace, same namespace.
    let alices_token = authorise_entry(&alice, alices_entry)
        .expect("alice authorises her own entry")
        .authorisation_token()
        .clone();
    let bobs_entry = build_alert_entry(&bob, &OBJECT_ID, &REVISION_ID, WILLOW_TS_MICROS, &payload)
        .expect("entry builds");
    assert!(
        !verify_entry(&bobs_entry, &alices_token),
        "capability area must deny a different subspace of the same namespace"
    );
}

// ---------- canonical bytes and digests ----------

#[test]
fn public_entry_and_capability_canonical_bytes_roundtrip() {
    let author = author();
    let payload = canonical_payload();
    let entry = build_alert_entry(
        &author,
        &OBJECT_ID,
        &REVISION_ID,
        WILLOW_TS_MICROS,
        &payload,
    )
    .expect("entry builds");

    let entry_bytes = encode_entry(&entry);
    let decoded = decode_entry_canonic(&entry_bytes).expect("canonical entry decodes");
    assert_eq!(&decoded, &entry);

    let mut trailing = entry_bytes.clone();
    trailing.push(0x00);
    assert!(decode_entry_canonic(&trailing).is_err());

    let capability = author.write_capability();
    let cap_bytes = encode_capability(&capability);
    let decoded_cap = decode_capability_canonic(&cap_bytes).expect("canonical capability decodes");
    assert_eq!(&decoded_cap, &capability);

    let mut cap_trailing = cap_bytes.clone();
    cap_trailing.push(0x00);
    assert!(decode_capability_canonic(&cap_trailing).is_err());
}

#[test]
fn public_entry_id_is_value_identity_not_proof_identity() {
    let alice = author();
    let payload = canonical_payload();
    let entry = build_alert_entry(&alice, &OBJECT_ID, &REVISION_ID, WILLOW_TS_MICROS, &payload)
        .expect("entry builds");
    let entry_bytes = encode_entry(&entry);
    let token = authorise_entry(&alice, entry)
        .expect("authorises")
        .authorisation_token()
        .clone();
    let cap_bytes = encode_capability(token.capability());
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    let sig = signature.to_bytes();

    let id = entry_id(&entry_bytes);
    let proof = evidence_digest(&entry_bytes, &cap_bytes, &sig);
    assert_ne!(id, proof, "value identity must differ from proof identity");
    // entry_id depends only on entry bytes.
    assert_eq!(id, entry_id(&entry_bytes));
    // evidence digest changes when the signature changes.
    let mut other_sig = sig;
    other_sig[0] ^= 1;
    assert_ne!(proof, evidence_digest(&entry_bytes, &cap_bytes, &other_sig));
}

#[test]
fn public_alert_entry_binds_path_and_payload() {
    let author = author();
    let payload = canonical_payload();
    let entry = build_alert_entry(
        &author,
        &OBJECT_ID,
        &REVISION_ID,
        WILLOW_TS_MICROS,
        &payload,
    )
    .expect("entry builds");

    let expected = alert_path(&OBJECT_ID, &REVISION_ID).expect("path builds");
    assert_eq!(entry.path(), &expected);
    assert_eq!(entry.path().component_count(), 4);
    assert_eq!(entry.payload_length(), payload.len() as u64);
    assert_eq!(entry.payload_digest_bytes(), william3_digest(&payload));
    assert_eq!(u64::from(entry.timestamp()), WILLOW_TS_MICROS);
}

#[test]
fn public_william3_matches_frozen_vector_fixture() {
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

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
