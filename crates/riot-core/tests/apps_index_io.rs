use riot_core::apps::bundle::{encode_app_bundle, AppBundle, AppResource};
use riot_core::apps::directory::AppProvenance;
use riot_core::apps::endorse::{encode_endorsement, write_endorsement, EndorsementMarker};
use riot_core::apps::index::{
    app_bundle_digest, app_index_bundle_path, app_index_endorsement_path, app_index_manifest_path,
    publish_app_index, scan_app_index, MAX_SCANNED_ENDORSEMENTS_PER_APP,
};
use riot_core::apps::manifest::{app_id_for, encode_manifest, AppManifest};
use riot_core::apps::trust::{write_trust_marker, TrustMarkerKind};
use riot_core::import::encode_bundle;
use riot_core::session::{CommitOutcome, EvidenceStore, ImportContext, RiotSession};
use riot_core::willow::{
    authorise_entry, encode_capability, encode_entry, generate_communal_author, Entry,
    EvidenceAuthor, Path, SignedWillowEntry,
};

fn sample_pair(author: &EvidenceAuthor) -> (Vec<u8>, Vec<u8>, [u8; 32]) {
    let manifest = AppManifest {
        name: "Checklist".into(),
        description: "Shared tasks".into(),
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
            bytes: b"<html>checklist</html>".to_vec(),
        }],
    };
    let manifest_bytes = encode_manifest(&manifest).expect("manifest");
    let bundle_bytes = encode_app_bundle(&bundle).expect("bundle");
    let app_id = app_id_for(&manifest, &app_bundle_digest(&bundle_bytes)).expect("app id");
    (manifest_bytes, bundle_bytes, app_id)
}

fn signed_at(
    author: &EvidenceAuthor,
    path: Path,
    payload: &[u8],
    timestamp: u64,
) -> SignedWillowEntry {
    let entry = Entry::builder()
        .namespace_id(author.namespace_id().clone())
        .subspace_id(author.subspace_id())
        .path(path)
        .timestamp(timestamp)
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

fn import_signed(store: &EvidenceStore, entries: &[SignedWillowEntry]) {
    let bytes = encode_bundle(entries).expect("bundle");
    let preview = store
        .inspect(&bytes, ImportContext::new("app-index-io-test"))
        .expect("inspect")
        .expect_preview();
    match preview.plan_all().expect("plan").commit().expect("commit") {
        CommitOutcome::Committed(_) => {}
        CommitOutcome::NoChanges(_) => panic!("expected new entries"),
    }
}

#[test]
fn publish_then_scan_returns_content_id_bundle_and_carrier() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let carrier = generate_communal_author().expect("carrier");
    let developer = generate_communal_author().expect("developer");
    let (manifest, bundle, expected_id) = sample_pair(&developer);

    let app_id = publish_app_index(&store, &carrier, &manifest, &bundle, 100).expect("publish");
    assert_eq!(app_id, expected_id);
    let scanned = scan_app_index(&store).expect("scan");
    assert_eq!(scanned.apps.len(), 1);
    assert_eq!(scanned.apps[0].app_id, expected_id);
    assert!(scanned.apps[0].bundle_present);
    assert_eq!(scanned.apps[0].manifest_timestamp_micros, 100);
    assert_eq!(
        scanned.apps[0].provenance,
        AppProvenance::Carried {
            carrier_subspace_id: *carrier.subspace_id().as_bytes(),
        }
    );
}

#[test]
fn publish_rejects_entry_point_mismatch_without_mutation() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let carrier = generate_communal_author().expect("carrier");
    let developer = generate_communal_author().expect("developer");
    let (manifest, _, _) = sample_pair(&developer);
    let other = encode_app_bundle(&AppBundle {
        entry_point: "main.html".into(),
        resources: vec![AppResource {
            path: "main.html".into(),
            content_type: "text/html".into(),
            bytes: vec![],
        }],
    })
    .expect("bundle");
    let generation = store.generation().expect("generation");

    assert!(publish_app_index(&store, &carrier, &manifest, &other, 100).is_err());
    assert_eq!(store.generation().expect("generation"), generation);
    assert_eq!(store.live_count().expect("live count"), 0);
}

#[test]
fn endorsement_overwrite_returns_one_retracted_marker() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let carrier = generate_communal_author().expect("carrier");
    let developer = generate_communal_author().expect("developer");
    let endorser = generate_communal_author().expect("endorser");
    let (manifest, bundle, _) = sample_pair(&developer);
    let app_id = publish_app_index(&store, &carrier, &manifest, &bundle, 100).expect("publish");
    write_endorsement(
        &store,
        &endorser,
        &EndorsementMarker {
            app_id,
            note: "used".into(),
            retracted: false,
        },
        200,
    )
    .expect("endorse");
    write_endorsement(
        &store,
        &endorser,
        &EndorsementMarker {
            app_id,
            note: String::new(),
            retracted: true,
        },
        300,
    )
    .expect("retract");

    let scanned = scan_app_index(&store).expect("scan");
    assert_eq!(scanned.endorsements.len(), 1);
    assert_eq!(scanned.endorsements[0].app_id, app_id);
    assert_eq!(
        scanned.endorsements[0].endorser_subspace_id,
        *endorser.subspace_id().as_bytes()
    );
    assert!(scanned.endorsements[0].retracted);
}

#[cfg(feature = "conformance")]
#[test]
fn trust_scan_keeps_namespace_author_and_timestamp() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let first_namespace = generate_communal_author().expect("namespace");
    let second_namespace = generate_communal_author().expect("namespace");
    let secret = [42u8; 32];
    let first =
        EvidenceAuthor::from_parts_for_tests(first_namespace.namespace_id().clone(), &secret);
    let second =
        EvidenceAuthor::from_parts_for_tests(second_namespace.namespace_id().clone(), &secret);
    let app_id = [9u8; 32];
    write_trust_marker(&store, &first, &app_id, TrustMarkerKind::Trust, 100).expect("trust");
    write_trust_marker(&store, &second, &app_id, TrustMarkerKind::Revoke, 200).expect("revoke");

    let scanned = scan_app_index(&store).expect("scan");
    assert_eq!(scanned.spaces.len(), 2);
    assert_eq!(scanned.spaces[0].markers.len(), 1);
    assert_eq!(scanned.spaces[1].markers.len(), 1);
    assert_ne!(
        scanned.spaces[0].space_namespace_id,
        scanned.spaces[1].space_namespace_id
    );
    let mut timestamps: Vec<_> = scanned
        .spaces
        .iter()
        .map(|space| space.markers[0].timestamp_micros)
        .collect();
    timestamps.sort_unstable();
    assert_eq!(timestamps, vec![100, 200]);
    assert!(scanned.spaces.iter().all(|space| {
        space.markers[0].author_subspace_id == *first.subspace_id().as_bytes()
            && space.organizer_subspace_ids.is_empty()
    }));
}

#[test]
fn manifest_only_is_listed_but_bundle_only_is_not() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let carrier = generate_communal_author().expect("carrier");
    let developer = generate_communal_author().expect("developer");
    let (manifest, bundle, app_id) = sample_pair(&developer);
    let bundle_only_id = [0xabu8; 32];
    import_signed(
        &store,
        &[
            signed_at(
                &carrier,
                app_index_manifest_path(&app_id).expect("path"),
                &manifest,
                100,
            ),
            signed_at(
                &carrier,
                app_index_bundle_path(&bundle_only_id).expect("path"),
                &bundle,
                100,
            ),
        ],
    );

    let scanned = scan_app_index(&store).expect("scan");
    assert_eq!(scanned.apps.len(), 1);
    assert_eq!(scanned.apps[0].app_id, app_id);
    assert!(!scanned.apps[0].bundle_present);
}

#[test]
fn wrong_app_id_pair_is_skipped() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let carrier = generate_communal_author().expect("carrier");
    let developer = generate_communal_author().expect("developer");
    let (manifest, bundle, _) = sample_pair(&developer);
    let wrong_id = [0xee; 32];
    import_signed(
        &store,
        &[
            signed_at(
                &carrier,
                app_index_manifest_path(&wrong_id).expect("path"),
                &manifest,
                100,
            ),
            signed_at(
                &carrier,
                app_index_bundle_path(&wrong_id).expect("path"),
                &bundle,
                100,
            ),
        ],
    );
    assert!(scan_app_index(&store).expect("scan").apps.is_empty());
}

#[test]
fn mismatched_endorsement_is_rejected_and_never_scanned() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let endorser = generate_communal_author().expect("endorser");
    let path_app = [1u8; 32];
    let payload = encode_endorsement(&EndorsementMarker {
        app_id: [2u8; 32],
        note: String::new(),
        retracted: false,
    })
    .expect("encode");
    let hostile = signed_at(
        &endorser,
        app_index_endorsement_path(&path_app, endorser.subspace_id().as_bytes()).expect("path"),
        &payload,
        100,
    );
    assert!(encode_bundle(&[hostile]).is_err());
    assert!(scan_app_index(&store)
        .expect("scan")
        .endorsements
        .is_empty());
}

#[cfg(feature = "conformance")]
#[test]
fn multiple_carriers_choose_complete_then_lowest_willow_coordinate() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let namespace = generate_communal_author().expect("namespace");
    let carrier_a =
        EvidenceAuthor::from_parts_for_tests(namespace.namespace_id().clone(), &[1; 32]);
    let carrier_b =
        EvidenceAuthor::from_parts_for_tests(namespace.namespace_id().clone(), &[2; 32]);
    let developer = generate_communal_author().expect("developer");
    let (manifest, bundle, app_id) = sample_pair(&developer);
    // Publish in reverse of the deterministic carrier order.
    publish_app_index(&store, &carrier_b, &manifest, &bundle, 100).expect("publish b");
    publish_app_index(&store, &carrier_a, &manifest, &bundle, 100).expect("publish a");
    let expected = [carrier_a.subspace_id(), carrier_b.subspace_id()]
        .into_iter()
        .map(|id| *id.as_bytes())
        .min()
        .expect("carrier");

    let scanned = scan_app_index(&store).expect("scan");
    assert_eq!(scanned.apps.len(), 1);
    assert_eq!(scanned.apps[0].app_id, app_id);
    assert_eq!(
        scanned.apps[0].provenance,
        AppProvenance::Carried {
            carrier_subspace_id: expected
        }
    );
}

#[cfg(feature = "conformance")]
#[test]
fn endorsement_scan_is_capped_at_exactly_256() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let namespace = generate_communal_author().expect("namespace");
    let app_id = [7u8; 32];
    let payload = encode_endorsement(&EndorsementMarker {
        app_id,
        note: String::new(),
        retracted: false,
    })
    .expect("encode");
    let mut entries = Vec::new();
    for i in 0..=MAX_SCANNED_ENDORSEMENTS_PER_APP {
        let mut secret = [0u8; 32];
        secret[..8].copy_from_slice(&(i as u64 + 1).to_be_bytes());
        let endorser =
            EvidenceAuthor::from_parts_for_tests(namespace.namespace_id().clone(), &secret);
        entries.push(signed_at(
            &endorser,
            app_index_endorsement_path(&app_id, endorser.subspace_id().as_bytes()).expect("path"),
            &payload,
            100,
        ));
    }
    for chunk in entries[..MAX_SCANNED_ENDORSEMENTS_PER_APP].chunks(64) {
        import_signed(&store, chunk);
    }
    assert_eq!(
        scan_app_index(&store)
            .expect("scan exact")
            .endorsements
            .len(),
        MAX_SCANNED_ENDORSEMENTS_PER_APP
    );
    import_signed(&store, &entries[MAX_SCANNED_ENDORSEMENTS_PER_APP..]);
    assert_eq!(
        scan_app_index(&store)
            .expect("scan over")
            .endorsements
            .len(),
        MAX_SCANNED_ENDORSEMENTS_PER_APP
    );
}
