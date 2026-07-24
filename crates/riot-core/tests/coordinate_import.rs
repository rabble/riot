//! Public proof that signed Coordinate items ride the ordinary evidence
//! pipeline: they admit through the same import boundary as newswire records,
//! are retained, and are rediscovered by the Coordinate ledger prefix scan (the
//! design's silent 5th registration site). A durable reopen confirms the scan
//! survives persistence, and a coordinate item is proven NOT to fall through to
//! alert admission.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use minicbor::Encoder;
use riot_core::coordinate::{
    create_signed_coordinate_item, inspect_coordinate_record, is_coordinate_prefix,
    load_ledger_records, CoordinateItemV1, CoordinateKind, CoordinatePayload, CoordinateStoreError,
    VerifiedCoordinateRecord,
};
use riot_core::import::{
    decode_bundle, encode_bundle, BundleDecodeOutcome, DiagnosticCode, ItemStatus, BUNDLE_CODEC_ID,
    BUNDLE_MAGIC,
};
use riot_core::newswire::{
    create_signed_news_post, create_signed_space_descriptor, inspect_news_record, NewsPostV1,
    SpaceDescriptorV1,
};
use riot_core::session::{CommitOutcome, EvidenceStore, ImportContext, RiotSession};
use riot_core::store::{DatabaseConfig, RiotDatabase};
use riot_core::willow::{
    generate_communal_author_for_namespace, generate_space_organizer_author, EvidenceAuthor, Path,
    SignedWillowEntry,
};

static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(1);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        let sequence = NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "riot-coordinate-import-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create test directory");
        Self(path)
    }

    fn database(&self) -> PathBuf {
        self.0.join("riot.sqlite")
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Fixture {
    descriptor_id: [u8; 32],
    namespace_id: [u8; 32],
    /// descriptor + one newswire post (a FOREIGN prefix) + one coordinate item.
    signed: Vec<SignedWillowEntry>,
    items: Vec<VerifiedCoordinateRecord>,
}

fn fixture(label: &str) -> Fixture {
    let organizer = generate_space_organizer_author().expect("organizer");
    let namespace_id = *organizer.namespace_id().as_bytes();
    let member = generate_communal_author_for_namespace(namespace_id).expect("member");
    let descriptor_record = create_signed_space_descriptor(
        &organizer,
        SpaceDescriptorV1 {
            namespace_id,
            name: format!("{label} Room"),
            summary: "A community working ledger.".into(),
            languages: vec!["en".into()],
            geographic_tags: vec![label.into()],
            topic_tags: vec!["local".into()],
            editorial_roster: vec![*member.subspace_id().as_bytes()],
            predecessor: None,
            successor: None,
        },
    )
    .expect("descriptor");
    let descriptor_id = descriptor_record.entry_id;
    let verified = inspect_news_record(&descriptor_record.signed).expect("verified descriptor");

    // A newswire post — a DIFFERENT reserved prefix in the same room, present to
    // prove the coordinate scan skips foreign prefixes.
    let post = create_signed_news_post(
        &member,
        &verified,
        NewsPostV1 {
            space_descriptor_entry_id: descriptor_id,
            headline: format!("{label} post"),
            body: "A discuss-channel post, not a ledger item.".into(),
            language: "en".into(),
            event_time_unix_seconds: None,
            expires_at_unix_seconds: None,
            coarse_location: None,
            source_claims: vec![],
            operational_profile: None,
            ai_assisted: false,
        },
    )
    .expect("post");

    let item = create_signed_coordinate_item(
        &member,
        &verified,
        CoordinateItemV1 {
            space_descriptor_entry_id: descriptor_id,
            kind: CoordinateKind::Need,
            title: format!("{label} needs a hand"),
            body: "Two hours on Saturday.".into(),
            language: "en".into(),
            category_tags: vec!["help".into()],
            coarse_location: None,
            capacity: None,
            needed_by_unix_seconds: None,
            expires_at_unix_seconds: None,
            contact_instructions: "Ask at the door".into(),
            source_claims: vec![],
            ai_assisted: false,
        },
    )
    .expect("coordinate item");

    let items = vec![inspect_coordinate_record(&item.signed).expect("inspect item")];
    Fixture {
        descriptor_id,
        namespace_id,
        signed: vec![descriptor_record.signed, post.signed, item.signed],
        items,
    }
}

fn commit(store: &EvidenceStore, signed: &[SignedWillowEntry]) -> CommitOutcome {
    let bundle = encode_bundle(signed).expect("encode Coordinate bundle");
    let preview = store
        .inspect(&bundle, ImportContext::new("coordinate-test"))
        .expect("inspect")
        .expect_preview();
    assert_eq!(
        preview.eligible_count().expect("eligible count"),
        signed.len()
    );
    preview.plan_all().expect("plan").commit().expect("commit")
}

#[test]
fn coordinate_prefix_reservation_requires_two_exact_raw_components() {
    assert!(is_coordinate_prefix(
        &Path::from_slices(&[b"coordinate", b"v1"]).expect("bare prefix")
    ));
    assert!(is_coordinate_prefix(
        &Path::from_slices(&[b"coordinate", b"v1", b"malformed"]).expect("reserved descendant")
    ));
    assert!(!is_coordinate_prefix(
        &Path::from_slices(&[b"coordinate"]).expect("one component")
    ));
    assert!(!is_coordinate_prefix(
        &Path::from_slices(&[b"coordinate", b"V1"]).expect("wrong raw version")
    ));
}

#[test]
fn coordinate_item_imports_retains_and_the_scan_finds_it_skipping_the_newswire_post() {
    let fixture = fixture("Harbor");
    let expected_ids = fixture
        .signed
        .iter()
        .map(|signed| riot_core::willow::entry_id(&signed.entry_bytes))
        .collect::<BTreeSet<_>>();
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");

    let outcome = commit(&store, &fixture.signed);
    assert!(matches!(
        outcome,
        CommitOutcome::Committed(ref receipt) if receipt.dispositions.len() == 3
    ));
    assert_eq!(
        store
            .live_entry_ids()
            .expect("live ids")
            .into_iter()
            .collect::<BTreeSet<_>>(),
        expected_ids
    );

    // The coordinate scan returns EXACTLY the ledger item — the newswire post
    // under `newswire/v1/<id>/posts` is a foreign prefix and is skipped.
    let scanned = load_ledger_records(&store, fixture.descriptor_id).expect("load ledger");
    assert_eq!(scanned, fixture.items);
    assert_eq!(scanned.len(), 1);
    assert!(matches!(scanned[0].payload(), CoordinatePayload::Item(_)));
    assert_eq!(scanned[0].namespace_id(), fixture.namespace_id);

    // Idempotent re-offer is a no-op.
    assert!(matches!(
        commit(&store, &fixture.signed),
        CommitOutcome::NoChanges(_)
    ));
}

#[test]
fn sqlite_reopen_preserves_the_coordinate_ledger_scan() {
    let directory = TestDir::new();
    let database_path = directory.database();
    let fixture = fixture("North");

    let database = RiotDatabase::open(&database_path, DatabaseConfig::default()).expect("database");
    let session = RiotSession::open_sqlite(database).expect("session");
    let store = session.create_store().expect("store");
    assert!(matches!(
        commit(&store, &fixture.signed),
        CommitOutcome::Committed(_)
    ));
    drop(store);
    drop(session);

    let database = RiotDatabase::open(&database_path, DatabaseConfig::default()).expect("reopen");
    let session = RiotSession::open_sqlite(database).expect("reopen session");
    let store = session.create_store().expect("reopen store");
    // The full SignedWillowEntry (cap + signature) persisted durably, so the
    // scan reprojects the ledger item verbatim after a reopen.
    assert_eq!(
        load_ledger_records(&store, fixture.descriptor_id).expect("ledger after reopen"),
        fixture.items
    );
}

#[test]
fn empty_store_has_a_typed_descriptor_not_found_error() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    assert_eq!(
        load_ledger_records(&store, [0xA5; 32]),
        Err(CoordinateStoreError::DescriptorNotFound)
    );
}

/// A coordinate item's reserved path must NEVER fall through to alert admission:
/// the item admits as a coordinate record, and a coordinate path carrying
/// non-decodable bytes is rejected outright, not rescued as an alert.
#[test]
fn coordinate_prefix_never_falls_through_to_alert_admission() {
    let fixture = fixture("Guard");
    // The genuine item admits and carries a coordinate path.
    let item_signed = &fixture.signed[2];
    let BundleDecodeOutcome::Decoded(decoded) =
        decode_bundle(&encode_bundle(std::slice::from_ref(item_signed)).expect("bundle"))
    else {
        panic!("decode");
    };
    assert!(matches!(decoded.items[0].status, ItemStatus::Valid(_)));

    // A malformed coordinate/v1 path carrying alert-shaped bytes must be
    // UnsupportedSchema — the producer bundle codec would reject it, so it is
    // framed raw to prove the RECEIVING classifier never rescues it as an alert.
    let author = generate_communal_author_for_namespace(fixture.namespace_id).expect("author");
    let malformed_path =
        Path::from_slices(&[b"coordinate", b"v1", b"not-a-family"]).expect("reserved path");
    let payload = riot_core::model::encode_alert(&riot_core::model::AlertPayload {
        object_id: [3; 16],
        revision_id: [4; 16],
        created_at: 1_000,
        valid_from: None,
        expires_at: 2_000,
        language: "en".into(),
        urgency: riot_core::model::Urgency::Immediate,
        severity: riot_core::model::Severity::Severe,
        certainty: riot_core::model::Certainty::Observed,
        headline: "Valid alert bytes".into(),
        description: "These bytes must not rescue a reserved Coordinate path.".into(),
        affected_area_claim: None,
        source_claims: vec!["fixture".into()],
        ai_assisted: false,
    })
    .expect("alert payload");
    let signed = sign_raw(&author, malformed_path, 100, payload);
    let BundleDecodeOutcome::Decoded(decoded) = decode_bundle(&frame_raw(&[signed])) else {
        panic!("decode");
    };
    assert!(matches!(
        decoded.items[0].status,
        ItemStatus::Invalid(ref diagnostic) if diagnostic.code == DiagnosticCode::UnsupportedSchema
    ));
}

fn frame_raw(items: &[SignedWillowEntry]) -> Vec<u8> {
    let mut buffer = BUNDLE_MAGIC.to_vec();
    let mut encoder = Encoder::new(&mut buffer);
    encoder.map(2).unwrap();
    encoder.u8(0).unwrap().str(BUNDLE_CODEC_ID).unwrap();
    encoder.u8(1).unwrap().array(items.len() as u64).unwrap();
    for item in items {
        encoder.map(4).unwrap();
        encoder.u8(0).unwrap().bytes(&item.entry_bytes).unwrap();
        encoder
            .u8(1)
            .unwrap()
            .bytes(&item.capability_bytes)
            .unwrap();
        encoder.u8(2).unwrap().bytes(&item.signature).unwrap();
        encoder.u8(3).unwrap().bytes(&item.payload_bytes).unwrap();
    }
    buffer
}

fn sign_raw(
    author: &EvidenceAuthor,
    path: Path,
    timestamp: u64,
    payload: Vec<u8>,
) -> SignedWillowEntry {
    use riot_core::willow::{authorise_entry, encode_capability, encode_entry, Entry};
    let entry = Entry::builder()
        .namespace_id(author.namespace_id().clone())
        .subspace_id(author.subspace_id())
        .path(path)
        .timestamp(timestamp)
        .payload(&payload)
        .build();
    let authorised = authorise_entry(author, entry).expect("authorise raw entry");
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    SignedWillowEntry {
        entry_bytes: encode_entry(authorised.entry()),
        capability_bytes: encode_capability(token.capability()),
        signature: signature.to_bytes(),
        payload_bytes: payload,
    }
}
