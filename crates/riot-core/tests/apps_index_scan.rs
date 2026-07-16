//! Scan/read-path evidence for `apps::index`. These tests exercise the parts
//! of the app-index reader that are reachable through the ordinary admission
//! pipeline: the `app_pair_bytes` carrier filter skipping a non-manifest/bundle
//! slot, and the two multi-element sort comparators in `scan_app_index`
//! (pending manifests and per-space trust markers). Each provokes the real
//! store state and asserts the resulting order/content.
//!
//! NOTE (reported to the coverage caller, not a source change): several
//! item-local `continue`/fallthrough guards inside `scan_app_index`
//! (a bundle payload failing `decode_app_bundle`; an endorsement/trust entry
//! whose subspace or decoded `app_id` disagrees with its path) are
//! unreachable through this pipeline: `import/bundle.rs::verify_frame` and
//! `session::inspect_inner`'s slot-ownership binding already fail closed on
//! exactly those conditions before such an entry can become live, so no scan
//! input can carry them. They are defense-in-depth re-checks.

use riot_core::apps::bundle::{encode_app_bundle, AppBundle, AppResource};
use riot_core::apps::index::{
    app_bundle_digest, app_index_manifest_path, app_pair_bytes, publish_app_index, scan_app_index,
};
use riot_core::apps::manifest::{app_id_for, encode_manifest, AppManifest};
use riot_core::apps::trust::{write_trust_marker, TrustMarkerKind};
use riot_core::import::encode_bundle;
use riot_core::session::{CommitOutcome, EvidenceStore, ImportContext, RiotSession};
use riot_core::willow::{
    authorise_entry, encode_capability, encode_entry, generate_communal_author, Entry,
    EvidenceAuthor, Path, SignedWillowEntry,
};

fn pair(author: &EvidenceAuthor, name: &str) -> (Vec<u8>, Vec<u8>, [u8; 32]) {
    let manifest = AppManifest {
        name: name.into(),
        description: "scan fixture".into(),
        version: "1.0.0".into(),
        author: author.identity(),
        permissions: vec!["app-data".into()],
        entry_point: "index.html".into(),
    };
    let bundle = AppBundle {
        entry_point: "index.html".into(),
        resources: vec![AppResource {
            path: "index.html".into(),
            content_type: "text/html".into(),
            bytes: format!("<html>{name}</html>").into_bytes(),
        }],
    };
    let manifest_bytes = encode_manifest(&manifest).expect("manifest");
    let bundle_bytes = encode_app_bundle(&bundle).expect("bundle");
    let app_id = app_id_for(&manifest, &app_bundle_digest(&bundle_bytes)).expect("app id");
    (manifest_bytes, bundle_bytes, app_id)
}

fn signed_at(author: &EvidenceAuthor, path: Path, payload: &[u8]) -> SignedWillowEntry {
    let entry = Entry::builder()
        .namespace_id(author.namespace_id().clone())
        .subspace_id(author.subspace_id())
        .path(path)
        .timestamp(100)
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

fn import(store: &EvidenceStore, entries: &[SignedWillowEntry]) {
    let bytes = encode_bundle(entries).expect("bundle");
    let preview = store
        .inspect(&bytes, ImportContext::new("scan-test"))
        .expect("inspect")
        .expect_preview();
    match preview.plan_all().expect("plan").commit().expect("commit") {
        CommitOutcome::Committed(_) => {}
        CommitOutcome::NoChanges(_) => panic!("expected new entries"),
    }
}

// ---------------------------------------------------------------------------
// index.rs line 163: app_pair_bytes classifies every entry under the
// `app-index/<app_id>` prefix; a trust marker at that prefix is neither a
// Manifest nor a Bundle slot, so it falls to the `_ => {}` arm. The complete
// pair is still returned.
// ---------------------------------------------------------------------------

#[test]
fn app_pair_bytes_skips_non_manifest_bundle_slots_under_the_prefix() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let carrier = generate_communal_author().expect("carrier");
    let developer = generate_communal_author().expect("developer");
    let (manifest, bundle, app_id) = pair(&developer, "Checklist");

    publish_app_index(&store, &carrier, &manifest, &bundle, 100).expect("publish");
    // A trust marker lands at app-index/<app_id>/trust/<organizer> — under the
    // same prefix app_pair_bytes scans, but not a manifest/bundle slot.
    write_trust_marker(&store, &carrier, &app_id, TrustMarkerKind::Trust, 150).expect("trust");

    let recovered = app_pair_bytes(&store, &app_id)
        .expect("app_pair_bytes")
        .expect("complete pair present");
    assert_eq!(recovered.manifest_bytes, manifest);
    assert_eq!(recovered.bundle_bytes, bundle);
}

// ---------------------------------------------------------------------------
// index.rs lines 383-386: pending_manifests.sort_by_key. Two live manifests
// with no live bundle (from two carriers, different app_ids) make a
// multi-element pending list, so the comparator actually runs. The result is
// sorted by (app_id, carrier namespace, carrier subspace, timestamp).
// ---------------------------------------------------------------------------

#[test]
fn pending_manifests_are_sorted_by_claimed_app_id_and_carrier() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let carrier = generate_communal_author().expect("carrier");
    let developer = generate_communal_author().expect("developer");

    // Two distinct apps, manifests only (no bundles) → both pending.
    let (manifest_a, _bundle_a, app_a) = pair(&developer, "Alpha");
    let (manifest_b, _bundle_b, app_b) = pair(&developer, "Bravo");

    import(
        &store,
        &[
            signed_at(
                &carrier,
                app_index_manifest_path(&app_a).expect("path"),
                &manifest_a,
            ),
            signed_at(
                &carrier,
                app_index_manifest_path(&app_b).expect("path"),
                &manifest_b,
            ),
        ],
    );

    let scanned = scan_app_index(&store).expect("scan");
    assert!(scanned.apps.is_empty(), "no complete pair is present");
    assert_eq!(scanned.pending_manifests.len(), 2);

    let mut ids: Vec<[u8; 32]> = scanned
        .pending_manifests
        .iter()
        .map(|p| p.claimed_app_id)
        .collect();
    let mut expected = ids.clone();
    expected.sort();
    assert_eq!(ids, expected, "pending manifests come out app_id-sorted");
    // Sanity: the two apps really are the two we published.
    ids.sort();
    let mut want = [app_a, app_b];
    want.sort();
    assert_eq!(ids, want.to_vec());
}

// ---------------------------------------------------------------------------
// index.rs lines 397-399: the per-space `markers.sort_by_key` closure. One
// organizer writing two trust markers (different app_ids) into one namespace
// gives that namespace a multi-marker vector, so the comparator runs. Markers
// come out sorted by (app_id, author subspace, timestamp).
// ---------------------------------------------------------------------------

#[test]
fn trust_markers_within_a_space_are_sorted_by_app_id() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let organizer = generate_communal_author().expect("organizer");

    let app_high = [0xF0u8; 32];
    let app_low = [0x01u8; 32];
    // Write the high app_id first so store order is not already sorted.
    write_trust_marker(&store, &organizer, &app_high, TrustMarkerKind::Trust, 200).expect("high");
    write_trust_marker(&store, &organizer, &app_low, TrustMarkerKind::Revoke, 100).expect("low");

    let scanned = scan_app_index(&store).expect("scan");
    assert_eq!(scanned.spaces.len(), 1, "one organizer namespace");
    let markers = &scanned.spaces[0].markers;
    assert_eq!(markers.len(), 2);
    // sort_by_key on (app_id, author_subspace, timestamp): app_low precedes.
    assert_eq!(markers[0].app_id, app_low);
    assert_eq!(markers[1].app_id, app_high);
}
