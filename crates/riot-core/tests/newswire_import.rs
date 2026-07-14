//! Public proof that signed Newswire records use the ordinary evidence pipeline.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use minicbor::Encoder;
use riot_core::import::{
    decode_bundle, encode_bundle, BundleDecodeOutcome, DiagnosticCode, ItemStatus, BUNDLE_CODEC_ID,
    BUNDLE_MAGIC,
};
use riot_core::model::{encode_alert, AlertPayload, Certainty, Severity, Urgency};
use riot_core::newswire::{
    create_signed_editorial_action, create_signed_news_post, create_signed_space_descriptor,
    inspect_news_record, is_newswire_prefix, load_space_descriptor, load_space_records,
    newswire_path, project, project_space, EditorialActionKind, EditorialActionV1, NewsPostV1,
    NewswirePathKind, NewswireStoreError, ProjectionClockV1, SpaceDescriptorV1,
    VerifiedNewswireRecord,
};
use riot_core::session::{CommitOutcome, EvidenceStore, ImportContext, RiotSession};
use riot_core::store::{DatabaseConfig, RiotDatabase};
use riot_core::willow::{
    authorise_entry, encode_capability, encode_entry, generate_communal_author,
    generate_communal_author_for_namespace, generate_space_organizer_author, william3_digest,
    Entry, EvidenceAuthor, Path, SignedWillowEntry,
};

static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(1);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        let sequence = NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "riot-newswire-import-{}-{sequence}",
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
    signed: Vec<SignedWillowEntry>,
    descriptor: VerifiedNewswireRecord,
    records: Vec<VerifiedNewswireRecord>,
}

fn fixture(label: &str) -> Fixture {
    let organizer = generate_space_organizer_author().expect("organizer");
    let namespace_id = *organizer.namespace_id().as_bytes();
    let editor = generate_communal_author_for_namespace(namespace_id).expect("editor");
    let descriptor_record = create_signed_space_descriptor(
        &organizer,
        SpaceDescriptorV1 {
            namespace_id,
            name: format!("{label} Newswire"),
            summary: "Human-published neighborhood reporting.".into(),
            languages: vec!["en".into()],
            geographic_tags: vec![label.into()],
            topic_tags: vec!["local".into()],
            editorial_roster: vec![*editor.subspace_id().as_bytes()],
            predecessor: None,
            successor: None,
        },
    )
    .expect("descriptor");
    let descriptor = inspect_news_record(&descriptor_record.signed).expect("inspect descriptor");
    let first = create_signed_news_post(
        &editor,
        &descriptor,
        NewsPostV1 {
            space_descriptor_entry_id: descriptor_record.entry_id,
            headline: format!("{label} first report"),
            body: "People reopened the north pier.".into(),
            language: "en".into(),
            event_time_unix_seconds: None,
            expires_at_unix_seconds: None,
            coarse_location: Some("north pier".into()),
            source_claims: vec!["eyewitness".into()],
            operational_profile: None,
            ai_assisted: false,
        },
    )
    .expect("first post");
    let second = create_signed_news_post(
        &editor,
        &descriptor,
        NewsPostV1 {
            space_descriptor_entry_id: descriptor_record.entry_id,
            headline: format!("{label} second report"),
            body: "The community kitchen is serving dinner.".into(),
            language: "en".into(),
            event_time_unix_seconds: None,
            expires_at_unix_seconds: None,
            coarse_location: Some("market hall".into()),
            source_claims: vec!["organizer".into()],
            operational_profile: None,
            ai_assisted: false,
        },
    )
    .expect("second post");
    let action = create_signed_editorial_action(
        &editor,
        &descriptor,
        EditorialActionV1 {
            space_descriptor_entry_id: descriptor_record.entry_id,
            target_entry_id: first.entry_id,
            kind: EditorialActionKind::Feature,
            reason: None,
            correction_text: None,
        },
    )
    .expect("action");
    let mut records = vec![
        inspect_news_record(&first.signed).expect("inspect first"),
        inspect_news_record(&second.signed).expect("inspect second"),
        inspect_news_record(&action.signed).expect("inspect action"),
    ];
    records.sort_by_key(|record| (record.tai_j2000_micros(), record.entry_id()));
    Fixture {
        descriptor_id: descriptor_record.entry_id,
        namespace_id,
        signed: vec![
            descriptor_record.signed,
            first.signed,
            second.signed,
            action.signed,
        ],
        descriptor,
        records,
    }
}

fn commit(store: &EvidenceStore, signed: &[SignedWillowEntry]) -> CommitOutcome {
    let bundle = encode_bundle(signed).expect("encode Newswire bundle");
    let preview = store
        .inspect(&bundle, ImportContext::new("newswire-test"))
        .expect("inspect")
        .expect_preview();
    assert_eq!(
        preview.eligible_count().expect("eligible count"),
        signed.len()
    );
    preview.plan_all().expect("plan").commit().expect("commit")
}

fn exact_prefixes(fixture: &Fixture) -> [Path; 3] {
    [
        Path::from_slices(&[b"newswire", b"v1", b"descriptors"]).expect("descriptor prefix"),
        Path::from_slices(&[b"newswire", b"v1", &fixture.descriptor_id, b"posts"])
            .expect("post prefix"),
        Path::from_slices(&[b"newswire", b"v1", &fixture.descriptor_id, b"actions"])
            .expect("action prefix"),
    ]
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

fn signed_at_path(
    author: &EvidenceAuthor,
    path: Path,
    timestamp: u64,
    payload: Vec<u8>,
) -> SignedWillowEntry {
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

#[test]
fn newswire_prefix_reservation_requires_two_exact_raw_components() {
    assert!(is_newswire_prefix(
        &Path::from_slices(&[b"newswire", b"v1"]).expect("bare prefix")
    ));
    assert!(is_newswire_prefix(
        &Path::from_slices(&[b"newswire", b"v1", b"malformed"]).expect("reserved descendant")
    ));
    assert!(!is_newswire_prefix(
        &Path::from_slices(&[b"newswire"]).expect("one component")
    ));
    assert!(!is_newswire_prefix(
        &Path::from_slices(&[b"newswire", b"V1"]).expect("wrong raw version")
    ));
}

#[test]
fn production_newswire_bundle_imports_retains_loads_and_projects_idempotently() {
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
        CommitOutcome::Committed(ref receipt) if receipt.dispositions.len() == 4
    ));
    assert_eq!(
        store
            .live_entry_ids()
            .expect("live ids")
            .into_iter()
            .collect::<BTreeSet<_>>(),
        expected_ids
    );

    let prefixes = exact_prefixes(&fixture);
    for (prefix, expected_indexes) in [
        (&prefixes[0], vec![0usize]),
        (&prefixes[1], vec![1usize, 2]),
        (&prefixes[2], vec![3usize]),
    ] {
        let matches = store.entries_with_prefix(prefix).expect("prefix query");
        assert_eq!(matches.len(), expected_indexes.len());
        let retained = matches
            .into_iter()
            .map(|(_, _, payload)| payload.expect("Newswire payload retained"))
            .collect::<BTreeSet<_>>();
        let expected = expected_indexes
            .into_iter()
            .map(|index| fixture.signed[index].payload_bytes.clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(retained, expected);
    }

    assert_eq!(
        load_space_descriptor(&store, fixture.descriptor_id).expect("load descriptor"),
        fixture.descriptor
    );
    assert_eq!(
        load_space_records(&store, fixture.descriptor_id).expect("load records"),
        fixture.records
    );
    let clock = ProjectionClockV1::system().expect("projection clock");
    assert_eq!(
        project_space(&store, fixture.descriptor_id, clock).expect("project store"),
        project(&fixture.descriptor, &fixture.records, clock).expect("project pure records")
    );

    let duplicate = commit(&store, &fixture.signed);
    assert!(matches!(
        duplicate,
        CommitOutcome::NoChanges(ref result)
            if result.entry_ids.iter().copied().collect::<BTreeSet<_>>() == expected_ids
    ));
    assert_eq!(store.generation().expect("generation"), 1);
}

#[test]
fn import_and_projection_are_order_independent() {
    let fixture = fixture("Market");
    let mut reversed = fixture.signed.clone();
    reversed.reverse();
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    assert!(matches!(
        commit(&store, &reversed),
        CommitOutcome::Committed(_)
    ));
    assert_eq!(
        load_space_descriptor(&store, fixture.descriptor_id).expect("descriptor"),
        fixture.descriptor
    );
    assert_eq!(
        load_space_records(&store, fixture.descriptor_id).expect("records"),
        fixture.records
    );
    let clock = ProjectionClockV1::system().expect("clock");
    assert_eq!(
        project_space(&store, fixture.descriptor_id, clock).expect("stored projection"),
        project(&fixture.descriptor, &fixture.records, clock).expect("pure projection")
    );
}

#[test]
fn invalid_newswire_sibling_is_isolated_from_a_valid_descriptor() {
    let fixture = fixture("Sibling");
    let author = generate_communal_author_for_namespace(fixture.namespace_id).expect("author");
    let malformed_payload = b"not canonical Newswire CBOR".to_vec();
    let malformed_path = newswire_path(
        NewswirePathKind::Descriptor,
        7,
        &william3_digest(&malformed_payload),
    )
    .expect("structurally valid Newswire path");
    let invalid = signed_at_path(&author, malformed_path, 7, malformed_payload);
    let bytes = frame_raw(&[fixture.signed[0].clone(), invalid]);
    let BundleDecodeOutcome::Decoded(decoded) = decode_bundle(&bytes) else {
        panic!("an invalid item must not reject its valid sibling");
    };
    assert!(matches!(decoded.items[0].status, ItemStatus::Valid(_)));
    assert!(matches!(
        decoded.items[1].status,
        ItemStatus::Invalid(ref diagnostic) if diagnostic.code == DiagnosticCode::UnsupportedSchema
    ));

    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let preview = store
        .inspect(&bytes, ImportContext::new("mixed-newswire"))
        .expect("inspect")
        .expect_preview();
    assert_eq!(preview.eligible_count().expect("eligible"), 1);
    preview.plan_all().expect("plan").commit().expect("commit");
    assert_eq!(
        store.live_entry_ids().expect("live"),
        vec![fixture.descriptor_id]
    );
}

#[test]
fn malformed_newswire_prefix_never_falls_through_to_alert_admission() {
    let author = generate_communal_author().expect("author");
    let payload = encode_alert(&AlertPayload {
        object_id: [3; 16],
        revision_id: [4; 16],
        created_at: 1_000,
        valid_from: None,
        expires_at: 2_000,
        language: "en".into(),
        urgency: Urgency::Immediate,
        severity: Severity::Severe,
        certainty: Certainty::Observed,
        headline: "Valid alert bytes".into(),
        description: "These bytes must not rescue a reserved Newswire path.".into(),
        affected_area_claim: None,
        source_claims: vec!["fixture".into()],
        ai_assisted: false,
    })
    .expect("alert payload");
    let malformed_path =
        Path::from_slices(&[b"newswire", b"v1", b"not-a-family"]).expect("malformed reserved path");
    let signed = signed_at_path(&author, malformed_path, 100, payload);
    let bytes = frame_raw(&[signed]);
    let BundleDecodeOutcome::Decoded(decoded) = decode_bundle(&bytes) else {
        panic!("item diagnostic expected");
    };
    assert!(matches!(
        decoded.items[0].status,
        ItemStatus::Invalid(ref diagnostic) if diagnostic.code == DiagnosticCode::UnsupportedSchema
    ));

    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let preview = store
        .inspect(&bytes, ImportContext::new("malformed-newswire"))
        .expect("inspect")
        .expect_preview();
    assert_eq!(preview.eligible_count().expect("eligible"), 0);
}

#[test]
fn empty_store_has_a_typed_descriptor_not_found_error() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    assert_eq!(
        load_space_descriptor(&store, [0xA5; 32]),
        Err(NewswireStoreError::DescriptorNotFound)
    );
    store.close().expect("close store");
    assert_eq!(
        load_space_descriptor(&store, [0xA5; 32]),
        Err(NewswireStoreError::StoreQueryFailed)
    );
}

#[test]
fn sqlite_reopen_preserves_typed_namespace_scoped_scans() {
    let directory = TestDir::new();
    let database_path = directory.database();
    let first = fixture("North");
    let second = fixture("South");
    let mut signed = first.signed.clone();
    signed.extend(second.signed.clone());

    let database = RiotDatabase::open(&database_path, DatabaseConfig::default()).expect("database");
    let session = RiotSession::open_sqlite(database).expect("session");
    let store = session.create_store().expect("store");
    assert!(matches!(
        commit(&store, &signed),
        CommitOutcome::Committed(_)
    ));
    drop(store);
    drop(session);

    let database = RiotDatabase::open(&database_path, DatabaseConfig::default()).expect("reopen");
    let session = RiotSession::open_sqlite(database).expect("reopen session");
    let store = session.create_store().expect("reopen store");
    assert_eq!(
        load_space_descriptor(&store, first.descriptor_id).expect("first descriptor"),
        first.descriptor
    );
    assert_eq!(
        load_space_records(&store, first.descriptor_id).expect("first records"),
        first.records
    );
    assert_eq!(
        load_space_descriptor(&store, second.descriptor_id).expect("second descriptor"),
        second.descriptor
    );
    assert_eq!(
        load_space_records(&store, second.descriptor_id).expect("second records"),
        second.records
    );

    let first_posts = exact_prefixes(&first)[1].clone();
    assert!(store
        .entries_with_prefix_in_namespace(&second.namespace_id, &first_posts)
        .expect("wrong namespace query")
        .is_empty());
}
