use riot_core::apps::index::app_index_trust_path;
use riot_core::apps::trust::{
    decode_trust_marker, encode_trust_marker, trust_markers_for, write_trust_marker, TrustMarker,
    TrustMarkerKind,
};
use riot_core::apps::AppsError;
use riot_core::import::encode_bundle;
use riot_core::session::{CommitOutcome, EvidenceStore, ImportContext, RiotSession, SessionError};
use riot_core::willow::{
    authorise_entry, encode_capability, encode_entry, generate_communal_author, Entry,
    EvidenceAuthor, SignedWillowEntry,
};
use willow25::entry::EntrylikeExt;

fn payload_marker(app_id: [u8; 32], kind: TrustMarkerKind) -> TrustMarker {
    TrustMarker {
        app_id,
        author_subspace_id: [0u8; 32],
        kind,
        timestamp_micros: 0,
    }
}

fn signed_trust_marker(
    author: &EvidenceAuthor,
    app_id: [u8; 32],
    kind: TrustMarkerKind,
    timestamp: u64,
) -> (SignedWillowEntry, Entry) {
    let payload = encode_trust_marker(&payload_marker(app_id, kind)).expect("payload");
    let path = app_index_trust_path(&app_id, author.subspace_id().as_bytes()).expect("path");
    let entry = Entry::builder()
        .namespace_id(author.namespace_id().clone())
        .subspace_id(author.subspace_id())
        .path(path)
        .timestamp(timestamp)
        .payload(&payload)
        .build();
    let authorised = authorise_entry(author, entry.clone()).expect("authorise");
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    (
        SignedWillowEntry {
            entry_bytes: encode_entry(authorised.entry()),
            capability_bytes: encode_capability(token.capability()),
            signature: signature.to_bytes(),
            payload_bytes: payload,
        },
        entry,
    )
}

fn import_signed(store: &EvidenceStore, signed: &SignedWillowEntry) {
    let bundle = encode_bundle(std::slice::from_ref(signed)).expect("bundle");
    let preview = store
        .inspect(&bundle, ImportContext::new("equal-timestamp-test"))
        .expect("inspect")
        .expect_preview();
    let plan = preview.plan_all().expect("plan");
    match plan.commit().expect("commit") {
        CommitOutcome::Committed(_) | CommitOutcome::NoChanges(_) => {}
    }
}

fn native_winner(trust_entry: &Entry, revoke_entry: &Entry) -> (TrustMarkerKind, TrustMarkerKind) {
    if trust_entry.cmp_recency(revoke_entry).is_gt() {
        (TrustMarkerKind::Trust, TrustMarkerKind::Revoke)
    } else {
        (TrustMarkerKind::Revoke, TrustMarkerKind::Trust)
    }
}

#[test]
fn trust_marker_codec_round_trips_trust_and_revoke() {
    for kind in [TrustMarkerKind::Trust, TrustMarkerKind::Revoke] {
        let marker = payload_marker([7u8; 32], kind);
        let encoded = encode_trust_marker(&marker).expect("encode");
        let decoded = decode_trust_marker(&encoded).expect("decode");
        assert_eq!(decoded.app_id, marker.app_id);
        assert_eq!(decoded.kind, marker.kind);
    }
}

#[test]
fn trust_marker_codec_rejects_tampering_truncation_and_trailing_bytes() {
    let encoded =
        encode_trust_marker(&payload_marker([7u8; 32], TrustMarkerKind::Trust)).expect("encode");

    let mut tampered = encoded.clone();
    tampered[1] = 2; // canonical key 0 becomes an unknown key
    assert!(decode_trust_marker(&tampered).is_err());
    assert!(decode_trust_marker(&encoded[..encoded.len() - 1]).is_err());

    let mut trailing = encoded;
    trailing.push(0);
    assert!(decode_trust_marker(&trailing).is_err());
}

#[test]
fn trust_marker_codec_rejects_unknown_kind() {
    let mut encoded =
        encode_trust_marker(&payload_marker([7u8; 32], TrustMarkerKind::Trust)).expect("encode");
    *encoded.last_mut().expect("kind byte") = 2;
    assert!(decode_trust_marker(&encoded).is_err());
}

#[test]
fn write_then_scan_derives_author_and_timestamp_from_the_entry() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let organizer = generate_communal_author().expect("organizer");
    let app_id = [7u8; 32];

    write_trust_marker(&store, &organizer, &app_id, TrustMarkerKind::Trust, 123)
        .expect("write trust");

    assert_eq!(
        trust_markers_for(&store, organizer.namespace_id().as_bytes(), &app_id).expect("scan"),
        vec![TrustMarker {
            app_id,
            author_subspace_id: *organizer.subspace_id().as_bytes(),
            kind: TrustMarkerKind::Trust,
            timestamp_micros: 123,
        }]
    );
}

#[test]
fn later_revoke_replaces_trust_in_the_organizers_lww_slot() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let organizer = generate_communal_author().expect("organizer");
    let app_id = [7u8; 32];

    write_trust_marker(&store, &organizer, &app_id, TrustMarkerKind::Trust, 123)
        .expect("write trust");
    write_trust_marker(&store, &organizer, &app_id, TrustMarkerKind::Revoke, 124)
        .expect("write revoke");

    assert_eq!(
        trust_markers_for(&store, organizer.namespace_id().as_bytes(), &app_id).expect("scan"),
        vec![TrustMarker {
            app_id,
            author_subspace_id: *organizer.subspace_id().as_bytes(),
            kind: TrustMarkerKind::Revoke,
            timestamp_micros: 124,
        }]
    );
}

#[test]
fn scan_isolates_the_same_app_and_subspace_across_namespaces() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let first_namespace = generate_communal_author().expect("first namespace");
    let second_namespace = generate_communal_author().expect("second namespace");
    let subspace_secret = [42u8; 32];
    let first = EvidenceAuthor::from_parts_for_tests(
        first_namespace.namespace_id().clone(),
        &subspace_secret,
    );
    let second = EvidenceAuthor::from_parts_for_tests(
        second_namespace.namespace_id().clone(),
        &subspace_secret,
    );
    assert_eq!(first.subspace_id(), second.subspace_id());
    assert_ne!(first.namespace_id(), second.namespace_id());
    let app_id = [8u8; 32];

    write_trust_marker(&store, &first, &app_id, TrustMarkerKind::Trust, 100).expect("first write");
    write_trust_marker(&store, &second, &app_id, TrustMarkerKind::Revoke, 200)
        .expect("second write");

    let first_markers =
        trust_markers_for(&store, first.namespace_id().as_bytes(), &app_id).expect("first scan");
    let second_markers =
        trust_markers_for(&store, second.namespace_id().as_bytes(), &app_id).expect("second scan");
    assert_eq!(first_markers.len(), 1);
    assert_eq!(first_markers[0].kind, TrustMarkerKind::Trust);
    assert_eq!(second_markers.len(), 1);
    assert_eq!(second_markers[0].kind, TrustMarkerKind::Revoke);
}

#[test]
fn lower_timestamp_revoke_errors_and_leaves_newer_trust_live() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let organizer = generate_communal_author().expect("organizer");
    let app_id = [9u8; 32];

    write_trust_marker(&store, &organizer, &app_id, TrustMarkerKind::Trust, 200)
        .expect("newer trust");
    assert_eq!(
        write_trust_marker(&store, &organizer, &app_id, TrustMarkerKind::Revoke, 100),
        Err(AppsError::StaleWrite)
    );
    let markers =
        trust_markers_for(&store, organizer.namespace_id().as_bytes(), &app_id).expect("scan");
    assert_eq!(markers.len(), 1);
    assert_eq!(markers[0].kind, TrustMarkerKind::Trust);
    assert_eq!(markers[0].timestamp_micros, 200);
}

#[test]
fn equal_timestamp_signed_entries_converge_by_willow_recency_in_both_import_orders() {
    let organizer = generate_communal_author().expect("organizer");
    let app_id = [10u8; 32];
    let (trust_signed, trust_entry) =
        signed_trust_marker(&organizer, app_id, TrustMarkerKind::Trust, 300);
    let (revoke_signed, revoke_entry) =
        signed_trust_marker(&organizer, app_id, TrustMarkerKind::Revoke, 300);
    let (winner, _) = native_winner(&trust_entry, &revoke_entry);

    for order in [
        [&trust_signed, &revoke_signed],
        [&revoke_signed, &trust_signed],
    ] {
        let session = RiotSession::open().expect("session");
        let store = session.create_store().expect("store");
        import_signed(&store, order[0]);
        import_signed(&store, order[1]);

        assert_eq!(store.live_count().expect("live count"), 1);
        let markers =
            trust_markers_for(&store, organizer.namespace_id().as_bytes(), &app_id).expect("scan");
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].kind, winner);
        assert_eq!(markers[0].timestamp_micros, 300);
    }
}

#[test]
fn equal_timestamp_local_writes_report_the_native_winner_and_loser() {
    let organizer = generate_communal_author().expect("organizer");
    let app_id = [14u8; 32];
    let (_, trust_entry) = signed_trust_marker(&organizer, app_id, TrustMarkerKind::Trust, 300);
    let (_, revoke_entry) = signed_trust_marker(&organizer, app_id, TrustMarkerKind::Revoke, 300);
    let (winner, loser) = native_winner(&trust_entry, &revoke_entry);

    let loser_first_session = RiotSession::open().expect("session");
    let loser_first_store = loser_first_session.create_store().expect("store");
    write_trust_marker(&loser_first_store, &organizer, &app_id, loser, 300)
        .expect("loser is initially live");
    write_trust_marker(&loser_first_store, &organizer, &app_id, winner, 300)
        .expect("native winner replaces loser");

    let winner_first_session = RiotSession::open().expect("session");
    let winner_first_store = winner_first_session.create_store().expect("store");
    write_trust_marker(&winner_first_store, &organizer, &app_id, winner, 300)
        .expect("winner is live");
    assert_eq!(
        write_trust_marker(&winner_first_store, &organizer, &app_id, loser, 300),
        Err(AppsError::StaleWrite)
    );

    for store in [&loser_first_store, &winner_first_store] {
        assert_eq!(store.live_count().expect("live count"), 1);
        let markers =
            trust_markers_for(store, organizer.namespace_id().as_bytes(), &app_id).expect("scan");
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].kind, winner);
    }
}

#[test]
fn exact_same_timestamp_and_kind_rewrite_is_idempotent() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let organizer = generate_communal_author().expect("organizer");
    let app_id = [12u8; 32];

    write_trust_marker(&store, &organizer, &app_id, TrustMarkerKind::Trust, 400)
        .expect("first write");
    write_trust_marker(&store, &organizer, &app_id, TrustMarkerKind::Trust, 400)
        .expect("exact duplicate");

    let markers =
        trust_markers_for(&store, organizer.namespace_id().as_bytes(), &app_id).expect("scan");
    assert_eq!(markers.len(), 1);
    assert_eq!(markers[0].kind, TrustMarkerKind::Trust);
    assert_eq!(markers[0].timestamp_micros, 400);
}

#[test]
fn local_trust_write_does_not_replace_an_active_import_preview() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let organizer = generate_communal_author().expect("organizer");
    let empty_bundle = encode_bundle(&[]).expect("empty bundle");
    let preview = store
        .inspect(&empty_bundle, ImportContext::new("active-review"))
        .expect("inspect")
        .expect_preview();

    assert_eq!(
        write_trust_marker(&store, &organizer, &[11u8; 32], TrustMarkerKind::Trust, 1,),
        Err(AppsError::StoreBusy)
    );
    assert!(matches!(
        preview.plan_all(),
        Err(SessionError::NoEligibleEntries)
    ));
}
