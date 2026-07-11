//! Admission-boundary evidence for app-data entries. The import pipeline
//! historically accepted only canonical Riot alerts (schema check in
//! `verify_frame`, alert path/payload binding in `inspect`). App-data
//! entries (`apps/<32-byte app_id>/<key segments>`) carry opaque payloads
//! with no embedded identity to bind, so both gates special-case exactly
//! that path shape — and nothing looser. Each negative test below violates
//! one clause of the shape and must still be rejected as `UnsupportedSchema`
//! by the same `verify_frame` a hostile peer's bundle would hit.

use riot_core::apps::entry::{app_data_path, build_app_data_entry};
use riot_core::import::{encode_bundle, BundleEncodeError, DiagnosticCode, ItemComponent};
use riot_core::session::{CommitOutcome, ImportContext, InspectOutcome, RiotSession};
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

#[test]
fn app_data_entry_is_admitted_committed_and_live() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let author = generate_communal_author().expect("author");

    let entry = build_app_data_entry(&author, &[7u8; 32], "items/a", 1, b"{\"done\":false}")
        .expect("entry");
    let authorised = authorise_entry(&author, entry).expect("authorise");
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    let signed = SignedWillowEntry {
        entry_bytes: encode_entry(authorised.entry()),
        capability_bytes: encode_capability(token.capability()),
        signature: signature.to_bytes(),
        payload_bytes: b"{\"done\":false}".to_vec(),
    };
    let bundle_bytes = encode_bundle(std::slice::from_ref(&signed)).expect("encode bundle");

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
        CommitOutcome::NoChanges(_) => panic!("app entry was silently dropped, not committed"),
    }
    assert_eq!(store.live_count().expect("live_count"), 1);
}

#[test]
fn opaque_payload_outside_apps_prefix_is_still_rejected() {
    let author = generate_communal_author().expect("author");
    let path = Path::from_slices(&[b"notes".as_slice(), b"x".as_slice()]).expect("path");
    expect_unsupported_schema(signed_at_path(&author, path, b"{}"));
}

#[test]
fn apps_path_with_wrong_app_id_length_is_rejected() {
    let author = generate_communal_author().expect("author");
    let short_id = [7u8; 16];
    let path =
        Path::from_slices(&[b"apps".as_slice(), &short_id, b"items".as_slice()]).expect("path");
    expect_unsupported_schema(signed_at_path(&author, path, b"{}"));
}

#[test]
fn apps_path_without_key_segments_is_rejected() {
    let author = generate_communal_author().expect("author");
    let app_id = [7u8; 32];
    let path = Path::from_slices(&[b"apps".as_slice(), &app_id]).expect("path");
    expect_unsupported_schema(signed_at_path(&author, path, b"{}"));
}

#[test]
fn apps_path_with_invalid_key_segment_is_rejected() {
    let author = generate_communal_author().expect("author");
    let app_id = [7u8; 32];
    let path =
        Path::from_slices(&[b"apps".as_slice(), &app_id, b"Items".as_slice()]).expect("path");
    expect_unsupported_schema(signed_at_path(&author, path, b"{}"));
}

#[test]
fn app_data_path_and_admission_shape_check_agree() {
    // The local constructor and the admission check must never drift: any
    // path `app_data_path` can build is admissible.
    let path = app_data_path(&[3u8; 32], "items/abc-123").expect("path");
    assert!(riot_core::apps::entry::is_app_data_path(&path));
}
