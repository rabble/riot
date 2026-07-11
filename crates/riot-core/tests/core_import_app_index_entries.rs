//! Admission-boundary evidence for app-index entries
//! (`app-index/<32-byte app_id>/{manifest|bundle|endorsements/<subspace>}`).
//! Unlike app-data (opaque payloads, `core_import_app_entries.rs`), every
//! app-index slot carries a *decodable* payload, so the two import gates
//! check more: `verify_frame` requires the slot's canonical codec to accept
//! the payload (and, for endorsements, that the payload's `app_id` matches
//! the path's), and `inspect` requires an endorsement entry's signing
//! subspace to equal the endorser component of its path — nobody can write
//! into someone else's endorsement slot. Anything the classifier does not
//! recognize stays `UnsupportedSchema`, on the same `verify_frame` surface a
//! hostile peer's bundle would hit.

use riot_core::apps::bundle::{encode_app_bundle, AppBundle, AppResource};
use riot_core::apps::endorse::{encode_endorsement, EndorsementMarker};
use riot_core::apps::index::{
    app_index_bundle_path, app_index_endorsement_path, app_index_manifest_path,
    classify_app_index_path, AppIndexSlot, APP_INDEX_COMPONENT,
};
use riot_core::apps::manifest::{encode_manifest, AppManifest};
use riot_core::import::{encode_bundle, BundleEncodeError, DiagnosticCode, ItemComponent};
use riot_core::model::{encode_alert, AlertPayload, Certainty, Severity, Urgency};
use riot_core::session::{
    CommitOutcome, EvidenceStore, ImportContext, InspectOutcome, RiotSession,
};
use riot_core::willow::{
    authorise_entry, encode_capability, encode_entry, generate_communal_author, Entry,
    EvidenceAuthor, Path, SignedWillowEntry,
};

fn signed_at_path(author: &EvidenceAuthor, path: Path, payload: &[u8]) -> SignedWillowEntry {
    let entry = Entry::builder()
        .namespace_id(author.namespace_id().clone())
        .subspace_id(author.subspace_id())
        .path(path)
        .timestamp(100u64)
        .payload(payload)
        .build();
    let authorised = authorise_entry(author, entry).expect("authorise");
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    SignedWillowEntry {
        entry_bytes: encode_entry(authorised.entry()),
        capability_bytes: encode_capability(token.capability()),
        signature: signature.to_bytes(),
        payload_bytes: payload.to_vec(),
    }
}

fn expect_unsupported_schema(entry: SignedWillowEntry) {
    match encode_bundle(std::slice::from_ref(&entry)) {
        Err(BundleEncodeError::InvalidItem(diagnostic)) => {
            assert_eq!(diagnostic.component, ItemComponent::Schema);
            assert_eq!(diagnostic.code, DiagnosticCode::UnsupportedSchema);
        }
        other => panic!("expected InvalidItem(UnsupportedSchema), got {other:?}"),
    }
}

/// Inspect + plan + commit one signed entry, asserting it survives every
/// stage — the full pipeline a synced app-index entry must pass.
fn commit_entry(store: &EvidenceStore, signed: &SignedWillowEntry) {
    let bundle_bytes = encode_bundle(std::slice::from_ref(signed)).expect("encode bundle");
    let preview = match store
        .inspect(&bundle_bytes, ImportContext::new("test-route"))
        .expect("inspect")
    {
        InspectOutcome::Preview(p) => p,
        InspectOutcome::Rejected(r) => panic!("rejected: {r:?}"),
    };
    let plan = preview.plan_all().expect("plan_all");
    match plan.commit().expect("commit") {
        CommitOutcome::Committed(_) => {}
        CommitOutcome::NoChanges(_) => panic!("entry was silently dropped, not committed"),
    }
}

fn sample_manifest(author: &EvidenceAuthor) -> AppManifest {
    AppManifest {
        name: "Checklist".to_string(),
        description: "Lets people add and check off shared to-dos.".to_string(),
        version: "1.0.0".to_string(),
        author: author.identity(),
        permissions: vec!["own-app-data".to_string()],
        entry_point: "index.html".to_string(),
    }
}

fn sample_bundle() -> AppBundle {
    AppBundle {
        entry_point: "index.html".to_string(),
        resources: vec![AppResource {
            path: "index.html".to_string(),
            content_type: "text/html".to_string(),
            bytes: b"<html></html>".to_vec(),
        }],
    }
}

#[test]
fn valid_manifest_entry_at_manifest_slot_commits() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let author = generate_communal_author().expect("author");

    let app_id = [7u8; 32];
    let payload = encode_manifest(&sample_manifest(&author)).expect("encode manifest");
    let path = app_index_manifest_path(&app_id).expect("path");
    commit_entry(&store, &signed_at_path(&author, path.clone(), &payload));

    assert_eq!(store.live_count().expect("live_count"), 1);
    // The payload must be retained and readable back — the directory scan
    // (Task 4) reads manifests from the live view, same as app-data.
    let matches = store.entries_with_prefix(&path).expect("query");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].2.as_deref(), Some(payload.as_slice()));
}

#[test]
fn valid_bundle_entry_at_bundle_slot_commits() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let author = generate_communal_author().expect("author");

    let app_id = [8u8; 32];
    let payload = encode_app_bundle(&sample_bundle()).expect("encode bundle payload");
    let path = app_index_bundle_path(&app_id).expect("path");
    commit_entry(&store, &signed_at_path(&author, path.clone(), &payload));

    assert_eq!(store.live_count().expect("live_count"), 1);
    let matches = store.entries_with_prefix(&path).expect("query");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].2.as_deref(), Some(payload.as_slice()));
}

#[test]
fn garbage_payload_at_manifest_slot_is_rejected_as_unsupported_schema() {
    let author = generate_communal_author().expect("author");
    let path = app_index_manifest_path(&[7u8; 32]).expect("path");
    expect_unsupported_schema(signed_at_path(&author, path, b"not-cbor"));
}

#[test]
fn endorsement_with_mismatched_payload_app_id_is_rejected() {
    let author = generate_communal_author().expect("author");
    let payload = encode_endorsement(&EndorsementMarker {
        app_id: [1u8; 32],
        note: "we ran jail support with this".to_string(),
        retracted: false,
    })
    .expect("encode endorsement");
    // Path names a different app than the signed marker claims to endorse.
    let path =
        app_index_endorsement_path(&[2u8; 32], author.subspace_id().as_bytes()).expect("path");
    expect_unsupported_schema(signed_at_path(&author, path, &payload));
}

#[test]
fn endorsement_written_into_someone_elses_slot_is_rejected_at_inspect() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let author = generate_communal_author().expect("author");

    let app_id = [3u8; 32];
    let payload = encode_endorsement(&EndorsementMarker {
        app_id,
        note: String::new(),
        retracted: false,
    })
    .expect("encode endorsement");

    // Writing into the signer's OWN endorsement slot passes both gates.
    let own_slot =
        app_index_endorsement_path(&app_id, author.subspace_id().as_bytes()).expect("path");
    commit_entry(&store, &signed_at_path(&author, own_slot, &payload));
    assert_eq!(store.live_count().expect("live_count"), 1);

    // The same signed marker aimed at ANOTHER subspace's slot passes the
    // schema gate (payload decodes, app_id matches the path) but must be
    // rejected by inspect's slot binding — same rejection surface as the
    // alert path-binding tests: the entry is simply not eligible.
    let someone_else = [9u8; 32];
    assert_ne!(&someone_else, author.subspace_id().as_bytes());
    let spoofed_slot = app_index_endorsement_path(&app_id, &someone_else).expect("path");
    let spoofed = signed_at_path(&author, spoofed_slot, &payload);
    let bundle_bytes = encode_bundle(std::slice::from_ref(&spoofed)).expect("encode bundle");
    let preview = store
        .inspect(&bundle_bytes, ImportContext::new("test-route"))
        .expect("inspect")
        .expect_preview();
    assert_eq!(
        preview.eligible_count().expect("eligible_count"),
        0,
        "an endorsement signed by one subspace but addressed to another's slot must not be eligible"
    );
    assert_eq!(store.live_count().expect("live_count"), 1);
}

#[test]
fn app_index_path_with_extra_components_is_rejected() {
    let author = generate_communal_author().expect("author");
    // Even a perfectly valid manifest payload is rejected when the path has
    // trailing components the classifier does not recognize.
    let payload = encode_manifest(&sample_manifest(&author)).expect("encode manifest");
    let app_id = [7u8; 32];
    let path = Path::from_slices(&[
        APP_INDEX_COMPONENT,
        &app_id,
        b"manifest".as_slice(),
        b"extra".as_slice(),
    ])
    .expect("path");
    expect_unsupported_schema(signed_at_path(&author, path, &payload));
}

#[test]
fn malformed_app_index_path_does_not_fall_through_to_alert_schema() {
    let author = generate_communal_author().expect("author");
    let payload = encode_alert(&AlertPayload {
        object_id: [1u8; 16],
        revision_id: [2u8; 16],
        created_at: 100,
        valid_from: None,
        expires_at: 200,
        language: "en".to_string(),
        urgency: Urgency::Immediate,
        severity: Severity::Severe,
        certainty: Certainty::Observed,
        headline: "Test alert".to_string(),
        description: "Valid alert bytes must not rescue an invalid app-index path.".to_string(),
        affected_area_claim: None,
        source_claims: vec!["test fixture".to_string()],
        ai_assisted: false,
    })
    .expect("encode alert");
    let app_id = [7u8; 32];
    let path = Path::from_slices(&[
        APP_INDEX_COMPONENT,
        &app_id,
        b"manifest".as_slice(),
        b"extra".as_slice(),
    ])
    .expect("path");

    expect_unsupported_schema(signed_at_path(&author, path, &payload));
}

#[test]
fn app_index_classifier_recognizes_only_exact_slot_shapes() {
    let app_id = [7u8; 32];
    let endorser = [8u8; 32];
    assert_eq!(
        classify_app_index_path(&app_index_manifest_path(&app_id).expect("manifest path")),
        Some(AppIndexSlot::Manifest { app_id })
    );
    assert_eq!(
        classify_app_index_path(&app_index_bundle_path(&app_id).expect("bundle path")),
        Some(AppIndexSlot::Bundle { app_id })
    );
    assert_eq!(
        classify_app_index_path(
            &app_index_endorsement_path(&app_id, &endorser).expect("endorsement path")
        ),
        Some(AppIndexSlot::Endorsement {
            app_id,
            endorser_subspace_id: endorser,
        })
    );

    let invalid_paths: Vec<Path> = vec![
        Path::from_slices(&[b"other", &app_id, b"manifest"]).expect("wrong prefix"),
        Path::from_slices(&[APP_INDEX_COMPONENT]).expect("missing app id"),
        Path::from_slices(&[APP_INDEX_COMPONENT, &[7u8; 31], b"manifest"]).expect("short app id"),
        Path::from_slices(&[APP_INDEX_COMPONENT, &app_id]).expect("missing slot"),
        Path::from_slices(&[APP_INDEX_COMPONENT, &app_id, b"unknown"]).expect("unknown slot"),
        Path::from_slices(&[APP_INDEX_COMPONENT, &app_id, b"endorsements"])
            .expect("missing endorser"),
        Path::from_slices(&[APP_INDEX_COMPONENT, &app_id, b"endorsements", &[8u8; 31]])
            .expect("short endorser"),
        Path::from_slices(&[APP_INDEX_COMPONENT, &app_id, b"bundle", b"extra"])
            .expect("extra component"),
        Path::from_slices(&[
            APP_INDEX_COMPONENT,
            &app_id,
            b"endorsements",
            &endorser,
            b"extra",
        ])
        .expect("extra endorsement component"),
    ];
    for path in invalid_paths {
        assert_eq!(classify_app_index_path(&path), None, "path: {path:?}");
    }
}
