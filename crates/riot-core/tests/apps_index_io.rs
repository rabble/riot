use riot_core::apps::bundle::{encode_app_bundle, AppBundle, AppResource};
use riot_core::apps::directory::AppProvenance;
use riot_core::apps::endorse::{encode_endorsement, write_endorsement, EndorsementMarker};
use riot_core::apps::index::{
    app_bundle_digest, app_index_bundle_path, app_index_endorsement_path, app_index_manifest_path,
    app_pair_bytes, publish_app_index, scan_app_index, verify_app_pair,
    MAX_SCANNED_ENDORSEMENTS_PER_APP,
};
use riot_core::apps::manifest::{app_id_for, encode_manifest, AppManifest};
use riot_core::apps::trust::{write_trust_marker, TrustMarkerKind};
use riot_core::import::encode_bundle;
use riot_core::session::{CommitOutcome, EvidenceStore, ImportContext, RiotSession};
use riot_core::willow::{
    authorise_entry, encode_capability, encode_entry, generate_communal_author, Entry,
    EvidenceAuthor, Path, SignedWillowEntry,
};
use std::collections::BTreeSet;

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
fn manifest_only_is_pending_but_bundle_only_is_absent() {
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
    assert!(scanned.apps.is_empty());
    assert_eq!(scanned.pending_manifests.len(), 1);
    assert_eq!(scanned.pending_manifests[0].claimed_app_id, app_id);
    assert_eq!(scanned.pending_manifests[0].manifest.name, "Checklist");
    assert_eq!(
        scanned.pending_manifests[0].carrier_subspace_id,
        *carrier.subspace_id().as_bytes()
    );
    assert_eq!(scanned.pending_manifests[0].manifest_timestamp_micros, 100);
}

#[test]
fn matching_bundle_promotes_pending_manifest_to_verified_app() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let carrier = generate_communal_author().expect("carrier");
    let developer = generate_communal_author().expect("developer");
    let (manifest, bundle, app_id) = sample_pair(&developer);
    import_signed(
        &store,
        &[signed_at(
            &carrier,
            app_index_manifest_path(&app_id).expect("path"),
            &manifest,
            100,
        )],
    );
    let pending = scan_app_index(&store).expect("pending scan");
    assert!(pending.apps.is_empty());
    assert_eq!(pending.pending_manifests.len(), 1);

    import_signed(
        &store,
        &[signed_at(
            &carrier,
            app_index_bundle_path(&app_id).expect("path"),
            &bundle,
            101,
        )],
    );
    let promoted = scan_app_index(&store).expect("promoted scan");
    assert_eq!(promoted.apps.len(), 1);
    assert_eq!(promoted.apps[0].app_id, app_id);
    assert!(promoted.apps[0].bundle_present);
    assert!(promoted.pending_manifests.is_empty());
}

#[test]
fn forged_manifest_claiming_legitimate_id_never_enters_verified_apps() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let carrier = generate_communal_author().expect("carrier");
    let legitimate_developer = generate_communal_author().expect("developer");
    let attacker = generate_communal_author().expect("attacker");
    let (_, _, legitimate_id) = sample_pair(&legitimate_developer);
    let (forged_manifest, _, forged_id) = sample_pair(&attacker);
    assert_ne!(legitimate_id, forged_id);
    import_signed(
        &store,
        &[signed_at(
            &carrier,
            app_index_manifest_path(&legitimate_id).expect("path"),
            &forged_manifest,
            100,
        )],
    );

    let scanned = scan_app_index(&store).expect("scan");
    assert!(scanned.apps.is_empty());
    assert_eq!(scanned.pending_manifests.len(), 1);
    assert_eq!(scanned.pending_manifests[0].claimed_app_id, legitimate_id);
    assert_eq!(
        scanned.pending_manifests[0].manifest.author,
        attacker.identity()
    );
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
    let scanned = scan_app_index(&store).expect("scan");
    assert!(scanned.apps.is_empty());
    assert!(scanned.pending_manifests.is_empty());
}

#[test]
fn same_coordinate_entry_point_mismatch_is_skipped_not_pending() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let carrier = generate_communal_author().expect("carrier");
    let developer = generate_communal_author().expect("developer");
    let (manifest, _, app_id) = sample_pair(&developer);
    let mismatched_bundle = encode_app_bundle(&AppBundle {
        entry_point: "main.html".into(),
        resources: vec![AppResource {
            path: "main.html".into(),
            content_type: "text/html".into(),
            bytes: b"<html>other</html>".to_vec(),
        }],
    })
    .expect("bundle");
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
                app_index_bundle_path(&app_id).expect("path"),
                &mismatched_bundle,
                100,
            ),
        ],
    );

    let scanned = scan_app_index(&store).expect("scan");
    assert!(scanned.apps.is_empty());
    assert!(scanned.pending_manifests.is_empty());
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
    let mut endorser_ids = Vec::new();
    for i in 0..=MAX_SCANNED_ENDORSEMENTS_PER_APP {
        let mut secret = [0u8; 32];
        secret[..8].copy_from_slice(&(i as u64 + 1).to_be_bytes());
        let endorser =
            EvidenceAuthor::from_parts_for_tests(namespace.namespace_id().clone(), &secret);
        endorser_ids.push(*endorser.subspace_id().as_bytes());
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
    let exact: Vec<_> = scan_app_index(&store)
        .expect("scan exact")
        .endorsements
        .into_iter()
        .map(|record| record.endorser_subspace_id)
        .collect();
    let mut expected_exact = endorser_ids[..MAX_SCANNED_ENDORSEMENTS_PER_APP].to_vec();
    expected_exact.sort_unstable();
    assert_eq!(exact, expected_exact);
    import_signed(&store, &entries[MAX_SCANNED_ENDORSEMENTS_PER_APP..]);
    let over: Vec<_> = scan_app_index(&store)
        .expect("scan over")
        .endorsements
        .into_iter()
        .map(|record| record.endorser_subspace_id)
        .collect();
    endorser_ids.sort_unstable();
    assert_eq!(over, endorser_ids[..MAX_SCANNED_ENDORSEMENTS_PER_APP]);
}

#[cfg(feature = "conformance")]
#[test]
fn endorsement_namespace_copies_dedup_by_subspace_and_newest_wins() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let first_namespace = generate_communal_author().expect("namespace");
    let second_namespace = generate_communal_author().expect("namespace");
    let secret = [0x41; 32];
    let first =
        EvidenceAuthor::from_parts_for_tests(first_namespace.namespace_id().clone(), &secret);
    let second =
        EvidenceAuthor::from_parts_for_tests(second_namespace.namespace_id().clone(), &secret);
    let app_id = [8u8; 32];
    let active = encode_endorsement(&EndorsementMarker {
        app_id,
        note: String::new(),
        retracted: false,
    })
    .expect("active");
    let retracted = encode_endorsement(&EndorsementMarker {
        app_id,
        note: String::new(),
        retracted: true,
    })
    .expect("retracted");
    import_signed(
        &store,
        &[
            signed_at(
                &first,
                app_index_endorsement_path(&app_id, first.subspace_id().as_bytes()).expect("path"),
                &active,
                200,
            ),
            signed_at(
                &second,
                app_index_endorsement_path(&app_id, second.subspace_id().as_bytes()).expect("path"),
                &retracted,
                100,
            ),
        ],
    );

    let scanned = scan_app_index(&store).expect("scan");
    assert_eq!(scanned.endorsements.len(), 1);
    assert_eq!(
        scanned.endorsements[0].endorser_subspace_id,
        *first.subspace_id().as_bytes()
    );
    assert!(
        !scanned.endorsements[0].retracted,
        "newer namespace copy wins"
    );
}

#[cfg(feature = "conformance")]
#[test]
fn equal_timestamp_retraction_wins_in_both_import_orders() {
    let first_namespace = generate_communal_author().expect("namespace");
    let second_namespace = generate_communal_author().expect("namespace");
    let secret = [0x42; 32];
    let first =
        EvidenceAuthor::from_parts_for_tests(first_namespace.namespace_id().clone(), &secret);
    let second =
        EvidenceAuthor::from_parts_for_tests(second_namespace.namespace_id().clone(), &secret);
    let app_id = [9u8; 32];
    let active = encode_endorsement(&EndorsementMarker {
        app_id,
        note: String::new(),
        retracted: false,
    })
    .expect("active");
    let retracted = encode_endorsement(&EndorsementMarker {
        app_id,
        note: String::new(),
        retracted: true,
    })
    .expect("retracted");
    let active_entry = signed_at(
        &first,
        app_index_endorsement_path(&app_id, first.subspace_id().as_bytes()).expect("path"),
        &active,
        100,
    );
    let retracted_entry = signed_at(
        &second,
        app_index_endorsement_path(&app_id, second.subspace_id().as_bytes()).expect("path"),
        &retracted,
        100,
    );

    for entries in [
        [&active_entry, &retracted_entry],
        [&retracted_entry, &active_entry],
    ] {
        let session = RiotSession::open().expect("session");
        let store = session.create_store().expect("store");
        import_signed(&store, std::slice::from_ref(entries[0]));
        import_signed(&store, std::slice::from_ref(entries[1]));
        let scanned = scan_app_index(&store).expect("scan");
        assert_eq!(scanned.endorsements.len(), 1);
        assert!(scanned.endorsements[0].retracted);
    }
}

#[test]
fn app_pair_bytes_round_trips_a_published_pair() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let carrier = generate_communal_author().expect("carrier");
    let developer = generate_communal_author().expect("developer");
    let (manifest, bundle, app_id) = sample_pair(&developer);
    publish_app_index(&store, &carrier, &manifest, &bundle, 100).expect("publish");

    let pair = app_pair_bytes(&store, &app_id)
        .expect("read")
        .expect("a published app is installable");
    assert_eq!(pair.manifest_bytes, manifest);
    assert_eq!(pair.bundle_bytes, bundle);
    // The bytes handed back are exactly the bytes the install path re-derives
    // the content identity from.
    assert_eq!(
        verify_app_pair(&pair.manifest_bytes, &pair.bundle_bytes),
        Ok(app_id)
    );
}

#[test]
fn app_pair_bytes_is_none_for_an_unknown_app() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    assert_eq!(app_pair_bytes(&store, &[0xcd; 32]).expect("read"), None);
}

#[test]
fn app_pair_bytes_is_none_while_the_bundle_is_still_arriving() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let carrier = generate_communal_author().expect("carrier");
    let developer = generate_communal_author().expect("developer");
    let (manifest, _, app_id) = sample_pair(&developer);
    import_signed(
        &store,
        &[signed_at(
            &carrier,
            app_index_manifest_path(&app_id).expect("path"),
            &manifest,
            100,
        )],
    );

    // A partial arrival has no installable bytes, and the directory agrees:
    // it is pending, not listed as present.
    assert_eq!(app_pair_bytes(&store, &app_id).expect("read"), None);
    let scanned = scan_app_index(&store).expect("scan");
    assert!(scanned.apps.is_empty());
    assert_eq!(scanned.pending_manifests.len(), 1);
}

#[test]
fn app_pair_bytes_refuses_a_pair_that_does_not_re_derive_the_requested_id() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let carrier = generate_communal_author().expect("carrier");
    let developer = generate_communal_author().expect("developer");
    let (manifest, _, app_id) = sample_pair(&developer);
    // Same entry point as the manifest, so only the content identity check can
    // catch it: these bytes are not the bundle `app_id` was derived from.
    let tampered = encode_app_bundle(&AppBundle {
        entry_point: "index.html".into(),
        resources: vec![AppResource {
            path: "index.html".into(),
            content_type: "text/html".into(),
            bytes: b"<html>malware</html>".to_vec(),
        }],
    })
    .expect("bundle");
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
                app_index_bundle_path(&app_id).expect("path"),
                &tampered,
                100,
            ),
        ],
    );

    assert_eq!(app_pair_bytes(&store, &app_id).expect("read"), None);
    assert!(scan_app_index(&store).expect("scan").apps.is_empty());
}

#[cfg(feature = "conformance")]
#[test]
fn a_hostile_carrier_cannot_block_installing_an_app_a_good_carrier_holds() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let namespace = generate_communal_author().expect("namespace");
    let first = EvidenceAuthor::from_parts_for_tests(namespace.namespace_id().clone(), &[1; 32]);
    let second = EvidenceAuthor::from_parts_for_tests(namespace.namespace_id().clone(), &[2; 32]);
    // The attacker takes the carrier coordinate that is considered first, so
    // the honest pair can only be found if an unverifiable carrier is skipped
    // rather than fatal.
    let (attacker, honest) = if first.subspace_id().as_bytes() < second.subspace_id().as_bytes() {
        (first, second)
    } else {
        (second, first)
    };
    let developer = generate_communal_author().expect("developer");
    let (manifest, bundle, app_id) = sample_pair(&developer);
    let tampered = encode_app_bundle(&AppBundle {
        entry_point: "index.html".into(),
        resources: vec![AppResource {
            path: "index.html".into(),
            content_type: "text/html".into(),
            bytes: b"<html>malware</html>".to_vec(),
        }],
    })
    .expect("bundle");
    // The honest app is published locally first; the hostile copy then arrives
    // over the wire, the way a synced app-index entry from a peer would.
    // (A local publish after an imported review would be refused as StoreBusy —
    // `commit_at` will not run while a review preview is installed.)
    publish_app_index(&store, &honest, &manifest, &bundle, 100).expect("publish");
    import_signed(
        &store,
        &[
            signed_at(
                &attacker,
                app_index_manifest_path(&app_id).expect("path"),
                &manifest,
                100,
            ),
            signed_at(
                &attacker,
                app_index_bundle_path(&app_id).expect("path"),
                &tampered,
                100,
            ),
        ],
    );

    // Both bundle payloads really are live at the same app-index slot, so the
    // choice below is a real one — admission does not filter the hostile copy.
    let live_bundles = store
        .entries_with_prefix(&app_index_bundle_path(&app_id).expect("path"))
        .expect("live bundles");
    assert_eq!(live_bundles.len(), 2);

    let pair = app_pair_bytes(&store, &app_id)
        .expect("read")
        .expect("the honest carrier's pair is still installable");
    assert_eq!(pair.bundle_bytes, bundle);
    assert_ne!(pair.bundle_bytes, tampered);
    assert_eq!(
        verify_app_pair(&pair.manifest_bytes, &pair.bundle_bytes),
        Ok(app_id)
    );
}

#[cfg(feature = "conformance")]
#[test]
fn namespace_duplicates_do_not_crowd_unique_endorsers_before_cap() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let app_id = [10u8; 32];
    let payload = encode_endorsement(&EndorsementMarker {
        app_id,
        note: String::new(),
        retracted: false,
    })
    .expect("marker");
    let duplicate_secret = [0x51; 32];
    let mut entries = Vec::new();
    // The real store admits at most 64 namespaces. Exercise 63 duplicate
    // namespace copies here; index.rs's private reconciliation unit test
    // covers the requested exact 256-copy hostile input.
    for _ in 0..63 {
        let namespace = generate_communal_author().expect("namespace");
        let endorser = EvidenceAuthor::from_parts_for_tests(
            namespace.namespace_id().clone(),
            &duplicate_secret,
        );
        entries.push(signed_at(
            &endorser,
            app_index_endorsement_path(&app_id, endorser.subspace_id().as_bytes()).expect("path"),
            &payload,
            100,
        ));
    }
    let unique_namespace = generate_communal_author().expect("namespace");
    let mut expected = BTreeSet::new();
    expected.insert(
        *EvidenceAuthor::from_parts_for_tests(
            unique_namespace.namespace_id().clone(),
            &duplicate_secret,
        )
        .subspace_id()
        .as_bytes(),
    );
    for i in 0..(MAX_SCANNED_ENDORSEMENTS_PER_APP - 1) {
        let mut secret = [0u8; 32];
        secret[..8].copy_from_slice(&(i as u64 + 1).to_be_bytes());
        let endorser =
            EvidenceAuthor::from_parts_for_tests(unique_namespace.namespace_id().clone(), &secret);
        expected.insert(*endorser.subspace_id().as_bytes());
        entries.push(signed_at(
            &endorser,
            app_index_endorsement_path(&app_id, endorser.subspace_id().as_bytes()).expect("path"),
            &payload,
            100,
        ));
    }
    for chunk in entries.chunks(64) {
        import_signed(&store, chunk);
    }

    let actual: BTreeSet<_> = scan_app_index(&store)
        .expect("scan")
        .endorsements
        .into_iter()
        .map(|record| record.endorser_subspace_id)
        .collect();
    assert_eq!(actual, expected);
    assert_eq!(actual.len(), MAX_SCANNED_ENDORSEMENTS_PER_APP);
}
