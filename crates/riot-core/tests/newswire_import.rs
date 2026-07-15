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
    build_share_reference, create_signed_editorial_action, create_signed_news_post,
    create_signed_space_descriptor, decode_share_reference, encode_share_reference,
    encode_space_descriptor, inspect_news_record, is_newswire_prefix, load_space_descriptor,
    load_space_records, newswire_path, project, project_space, verify_descriptor_matches,
    EditorialActionKind, EditorialActionV1, NewsPostV1, NewswirePathKind, NewswirePayload,
    NewswireShareReferenceV1, NewswireStoreError, ProjectionClockV1, ShareReferenceError,
    SpaceDescriptorV1, VerifiedNewswireRecord,
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

/// The same signed record, framed as a file import and then re-offered as a
/// nearby-sync bundle, must merge to a single entry — the import boundary is
/// content-addressed by entry id, so the transport it arrived on is irrelevant.
#[test]
fn one_record_from_file_then_nearby_merges_to_a_single_entry() {
    let fixture = fixture("Merge");
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");

    // "File" import: the descriptor + its first post as a saved bundle blob.
    let from_file = vec![fixture.signed[0].clone(), fixture.signed[1].clone()];
    assert!(matches!(
        commit(&store, &from_file),
        CommitOutcome::Committed(ref receipt) if receipt.dispositions.len() == 2
    ));

    // "Nearby" re-offer of the identical record set is a no-op: no second copy.
    let from_nearby = frame_raw(&from_file);
    let preview = store
        .inspect(&from_nearby, ImportContext::new("nearby-newswire"))
        .expect("inspect")
        .expect_preview();
    assert_eq!(preview.eligible_count().expect("eligible"), 2);
    assert!(matches!(
        preview.plan_all().expect("plan").commit().expect("commit"),
        CommitOutcome::NoChanges(_)
    ));
    assert_eq!(store.generation().expect("generation"), 1);
    let live = store.live_entry_ids().expect("live");
    assert_eq!(live.len(), 2);
}

/// The share/join reference binds the descriptor's WILLIAM3 content digest, its
/// entry id, and its namespace — so a peer can prove a received descriptor is
/// the one the reference named, not a substitute.
#[test]
fn share_reference_binds_the_descriptor_content_digest() {
    let fixture = fixture("Bind");
    let reference = build_share_reference(&fixture.descriptor).expect("build reference");
    assert_eq!(reference.namespace_id, fixture.descriptor.namespace_id());
    assert_eq!(reference.descriptor_entry_id, fixture.descriptor.entry_id());
    let NewswirePayload::SpaceDescriptor(descriptor) = fixture.descriptor.payload() else {
        panic!("descriptor payload");
    };
    let expected =
        william3_digest(&encode_space_descriptor(descriptor).expect("encode descriptor"));
    assert_eq!(reference.content_digest, expected);
    assert!(verify_descriptor_matches(&reference, &fixture.descriptor));
}

/// Anti-substitution: a descriptor with different content (name/roster) does not
/// match a reference minted for the genuine one, and the digest field is
/// load-bearing — corrupting it alone flips verification to false.
#[test]
fn a_substituted_descriptor_fails_share_reference_verification() {
    let genuine = fixture("Genuine");
    let reference = build_share_reference(&genuine.descriptor).expect("reference");
    assert!(verify_descriptor_matches(&reference, &genuine.descriptor));

    let substitute = fixture("Substitute");
    assert!(!verify_descriptor_matches(
        &reference,
        &substitute.descriptor
    ));

    let tampered = NewswireShareReferenceV1 {
        content_digest: [0xEE; 32],
        ..reference.clone()
    };
    assert!(!verify_descriptor_matches(&tampered, &genuine.descriptor));
}

/// A non-descriptor record cannot mint a community share reference.
#[test]
fn share_reference_requires_a_descriptor_payload() {
    let fixture = fixture("Post");
    let post = fixture
        .records
        .iter()
        .find(|record| matches!(record.payload(), NewswirePayload::NewsPost(_)))
        .expect("a news post record");
    assert_eq!(
        build_share_reference(post),
        Err(ShareReferenceError::NotADescriptor)
    );
}

/// The encoded reference round-trips exactly, and a malformed string is rejected
/// rather than silently decoded into partial coordinates.
#[test]
fn share_reference_encodes_and_decodes_round_trip() {
    let fixture = fixture("Round");
    let reference = build_share_reference(&fixture.descriptor).expect("reference");
    let encoded = encode_share_reference(&reference);
    assert!(encoded.starts_with("riot://newswire/join/v1/"));
    assert_eq!(decode_share_reference(&encoded).expect("decode"), reference);
    assert_eq!(
        decode_share_reference("riot://newswire/join/v1/abc"),
        Err(ShareReferenceError::Malformed)
    );
    assert_eq!(
        decode_share_reference("https://example.com/not-a-reference"),
        Err(ShareReferenceError::Malformed)
    );
}

// ---------------------------------------------------------------------------
// Cross-platform golden vector. This committed fixture is the byte-identity
// anchor for the Rust, iOS, and Android encoders: every platform reconstructs
// the same SpaceDescriptorV1 from the declared fields and must reproduce the
// canonical CBOR, its WILLIAM3 digest, and the share-reference string. The
// entry id / namespace are fixed test coordinates, not a signed entry — the
// harness proves encoding identity, not signing.
// ---------------------------------------------------------------------------

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hex32(value: &serde_json::Value) -> [u8; 32] {
    let text = value.as_str().expect("hex string");
    let bytes = (0..text.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&text[index..index + 2], 16).expect("hex byte"))
        .collect::<Vec<_>>();
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    out
}

fn str_vec(value: &serde_json::Value) -> Vec<String> {
    value
        .as_array()
        .expect("array")
        .iter()
        .map(|item| item.as_str().expect("string").to_string())
        .collect()
}

fn golden_fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/newswire/newswire-golden-1.json")
}

fn golden_descriptor(doc: &serde_json::Value) -> SpaceDescriptorV1 {
    let d = &doc["descriptor"];
    SpaceDescriptorV1 {
        namespace_id: hex32(&d["namespace_id_hex"]),
        name: d["name"].as_str().expect("name").to_string(),
        summary: d["summary"].as_str().expect("summary").to_string(),
        languages: str_vec(&d["languages"]),
        geographic_tags: str_vec(&d["geographic_tags"]),
        topic_tags: str_vec(&d["topic_tags"]),
        editorial_roster: d["editorial_roster_hex"]
            .as_array()
            .expect("roster")
            .iter()
            .map(hex32)
            .collect(),
        predecessor: None,
        successor: None,
    }
}

/// Regenerates the committed golden fixture from the canonical field definition
/// below. Guarded by an env flag so it never runs in the normal suite; the
/// assertion test that follows is the one that runs everywhere. Regenerate with:
///   REGEN_NEWSWIRE_GOLDEN=1 cargo test -p riot-core --test newswire_import \
///     regenerate_newswire_golden_fixture -- --ignored
#[test]
#[ignore = "regenerator; run explicitly with REGEN_NEWSWIRE_GOLDEN=1"]
fn regenerate_newswire_golden_fixture() {
    if std::env::var("REGEN_NEWSWIRE_GOLDEN").is_err() {
        return;
    }
    let namespace_id = [0x11u8; 32];
    let descriptor_entry_id = [0x44u8; 32];
    let editors = [[0x22u8; 32], [0x33u8; 32]];
    let descriptor = SpaceDescriptorV1 {
        namespace_id,
        name: "Harbor Commons Newswire".into(),
        summary: "Human-published neighborhood reporting for the Harbor Commons.".into(),
        languages: vec!["en".into()],
        geographic_tags: vec!["harbor-commons".into()],
        topic_tags: vec!["local".into(), "mutual-aid".into()],
        editorial_roster: editors.to_vec(),
        predecessor: None,
        successor: None,
    };
    let cbor = encode_space_descriptor(&descriptor).expect("encode descriptor");
    let content_digest = william3_digest(&cbor);
    let reference = NewswireShareReferenceV1 {
        namespace_id,
        descriptor_entry_id,
        content_digest,
    };
    let doc = serde_json::json!({
        "contract": "riot-newswire-golden/1",
        "provenance_note": "Deterministic canonical encodings produced by riot-core's \
            encode_space_descriptor + WILLIAM3 + share-reference encoder. namespace_id and \
            descriptor_entry_id are fixed test coordinates, not a signed entry: this vector \
            proves cross-platform ENCODING identity (Rust/iOS/Android), not signing. \
            Regenerate with REGEN_NEWSWIRE_GOLDEN=1.",
        "descriptor": {
            "namespace_id_hex": hex_encode(&namespace_id),
            "name": descriptor.name,
            "summary": descriptor.summary,
            "languages": descriptor.languages,
            "geographic_tags": descriptor.geographic_tags,
            "topic_tags": descriptor.topic_tags,
            "editorial_roster_hex": editors.iter().map(|e| hex_encode(e)).collect::<Vec<_>>(),
            "canonical_cbor_hex": hex_encode(&cbor),
            "content_digest_hex": hex_encode(&content_digest),
        },
        "share_reference": {
            "namespace_id_hex": hex_encode(&namespace_id),
            "descriptor_entry_id_hex": hex_encode(&descriptor_entry_id),
            "content_digest_hex": hex_encode(&content_digest),
            "encoded": encode_share_reference(&reference),
        }
    });
    let path = golden_fixture_path();
    fs::create_dir_all(path.parent().expect("fixture dir")).expect("create fixture dir");
    fs::write(
        &path,
        format!("{}\n", serde_json::to_string_pretty(&doc).expect("json")),
    )
    .expect("write golden fixture");
}

#[test]
fn newswire_golden_fixture_reproduces_canonical_encoding() {
    let raw = fs::read_to_string(golden_fixture_path()).expect("read golden fixture");
    let doc: serde_json::Value = serde_json::from_str(&raw).expect("valid golden JSON");
    assert_eq!(doc["contract"], "riot-newswire-golden/1");

    // The descriptor body encodes to exactly the committed canonical CBOR, and
    // its WILLIAM3 digest is exactly the committed content digest.
    let descriptor = golden_descriptor(&doc);
    let cbor = encode_space_descriptor(&descriptor).expect("encode descriptor");
    assert_eq!(
        hex_encode(&cbor),
        doc["descriptor"]["canonical_cbor_hex"]
            .as_str()
            .expect("cbor hex")
    );
    let digest = william3_digest(&cbor);
    assert_eq!(
        hex_encode(&digest),
        doc["descriptor"]["content_digest_hex"]
            .as_str()
            .expect("digest hex")
    );

    // The share reference encodes to exactly the committed string and decodes
    // back to the committed coordinates.
    let share = &doc["share_reference"];
    let reference = NewswireShareReferenceV1 {
        namespace_id: hex32(&share["namespace_id_hex"]),
        descriptor_entry_id: hex32(&share["descriptor_entry_id_hex"]),
        content_digest: hex32(&share["content_digest_hex"]),
    };
    let encoded = share["encoded"].as_str().expect("encoded");
    assert_eq!(encode_share_reference(&reference), encoded);
    assert_eq!(decode_share_reference(encoded).expect("decode"), reference);
    // The reference's content digest is the descriptor's own digest — the
    // binding the whole harness exists to prove.
    assert_eq!(reference.content_digest, digest);
}
