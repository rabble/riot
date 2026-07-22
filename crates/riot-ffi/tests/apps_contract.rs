//! FFI contract for the signed-JS-apps runtime surface: manifest install,
//! per-profile trust decisions, namespace-scoped app data put/get/list, and
//! the app-directory surface (listings, share, endorse) — end-to-end through
//! the UniFFI layer, in-process, same as `mobile_contract.rs`.

use riot_ffi::{
    open_local_profile, open_profile_from_sealed_identity, AlertCertainty, AlertDraftInput,
    AlertSeverity, AlertUrgency, AppRuntimeSession, MobileError, MobileProfile, MobileSyncSession,
    PublicSpace, SyncOutcomeKind,
};
use std::sync::Arc;

/// A minimal well-formed alert draft, used only to produce a real (non
/// app-data) signed bundle for the replay strictness test.
fn alert_input() -> AlertDraftInput {
    AlertDraftInput {
        valid_from: None,
        expires_at: u64::MAX - 1,
        language: "en".to_string(),
        urgency: AlertUrgency::Immediate,
        severity: AlertSeverity::Severe,
        certainty: AlertCertainty::Observed,
        headline: "Replay strictness fixture".to_string(),
        description: "A signed alert must never replay through the app-data path.".to_string(),
        affected_area_claim: None,
        source_claims: vec!["fixture".to_string()],
        ai_assisted: false,
    }
}

/// Hex string (as returned by `install_app`/`identity`) to raw bytes (as the
/// directory surface uses for 32-byte ids).
fn unhex(value: &str) -> Vec<u8> {
    (0..value.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16).expect("hex"))
        .collect()
}

fn manifest_and_bundle() -> (Vec<u8>, Vec<u8>) {
    manifest_and_bundle_with_resource(b"<html>checklist</html>".to_vec())
}

fn manifest_and_bundle_with_resource(resource_bytes: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
    // Manifest/bundle bytes are produced with riot-core's own codecs — the
    // same way the future `riot-app` packaging tool will produce them.
    use riot_core::apps::bundle::{encode_app_bundle, AppBundle, AppResource};
    use riot_core::apps::manifest::{encode_manifest, AppManifest};
    use riot_core::willow::generate_communal_author;

    let author = generate_communal_author().expect("author");
    let bundle = AppBundle {
        entry_point: "index.html".to_string(),
        resources: vec![AppResource {
            path: "index.html".to_string(),
            content_type: "text/html".to_string(),
            bytes: resource_bytes,
        }],
    };
    let manifest = AppManifest {
        name: "Checklist".to_string(),
        description: "Lets people add and check off shared to-dos.".to_string(),
        version: "1.0.0".to_string(),
        author: author.identity(),
        permissions: vec!["own-app-data".to_string()],
        entry_point: "index.html".to_string(),
    };
    (
        encode_manifest(&manifest).expect("manifest"),
        encode_app_bundle(&bundle).expect("bundle"),
    )
}

fn signed_bundle_at(
    author: &riot_core::willow::EvidenceAuthor,
    path: riot_core::willow::Path,
    payload: &[u8],
) -> Vec<u8> {
    let signed = signed_entry_at(author, path, payload);
    riot_core::import::encode_bundle(&[signed]).unwrap()
}

fn signed_entry_at(
    author: &riot_core::willow::EvidenceAuthor,
    path: riot_core::willow::Path,
    payload: &[u8],
) -> riot_core::willow::SignedWillowEntry {
    let entry = riot_core::willow::Entry::builder()
        .namespace_id(author.namespace_id().clone())
        .subspace_id(author.subspace_id())
        .path(path)
        .timestamp(1)
        .payload(payload)
        .build();
    let authorised = riot_core::willow::authorise_entry(author, entry).unwrap();
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    riot_core::willow::SignedWillowEntry {
        entry_bytes: riot_core::willow::encode_entry(authorised.entry()),
        capability_bytes: riot_core::willow::encode_capability(token.capability()),
        signature: signature.to_bytes(),
        payload_bytes: payload.to_vec(),
    }
}

fn frame_unchecked(signed: &riot_core::willow::SignedWillowEntry) -> Vec<u8> {
    use minicbor::Encoder;
    let mut bytes = riot_core::import::BUNDLE_MAGIC.to_vec();
    let mut encoder = Encoder::new(&mut bytes);
    encoder.map(2).unwrap();
    encoder
        .u8(0)
        .unwrap()
        .str(riot_core::import::BUNDLE_CODEC_ID)
        .unwrap();
    encoder.u8(1).unwrap().array(1).unwrap();
    encoder.map(4).unwrap();
    encoder.u8(0).unwrap().bytes(&signed.entry_bytes).unwrap();
    encoder
        .u8(1)
        .unwrap()
        .bytes(&signed.capability_bytes)
        .unwrap();
    encoder.u8(2).unwrap().bytes(&signed.signature).unwrap();
    encoder.u8(3).unwrap().bytes(&signed.payload_bytes).unwrap();
    bytes
}

fn take_frame(session: &MobileSyncSession) -> Vec<u8> {
    session
        .take_outbound_frame()
        .expect("take outbound frame")
        .expect("sync outcome queued a frame")
}

fn sync_to_review(
    receiver: &Arc<MobileProfile>,
    sender: &Arc<MobileProfile>,
) -> (
    Arc<MobileSyncSession>,
    Arc<MobileSyncSession>,
    riot_ffi::SyncOutcome,
) {
    let initiator = receiver.open_sync_session().expect("receiver sync");
    let responder = sender.open_sync_session().expect("sender sync");
    initiator.begin().expect("begin");
    responder
        .receive_frame(take_frame(&initiator))
        .expect("receive hello");
    initiator
        .receive_frame(take_frame(&responder))
        .expect("receive summary");
    responder
        .receive_frame(take_frame(&initiator))
        .expect("receive request");
    let review = initiator
        .receive_frame(take_frame(&responder))
        .expect("receive entries");
    (initiator, responder, review)
}

fn accept_and_finish(initiator: &MobileSyncSession, responder: &MobileSyncSession) {
    assert_eq!(
        initiator.accept_import().expect("accept import").kind,
        SyncOutcomeKind::FrameReady
    );
    let responder_outcome = responder
        .receive_frame(take_frame(initiator))
        .expect("receive post-import summary");
    if !responder_outcome.terminal {
        initiator
            .receive_frame(take_frame(responder))
            .expect("send reverse entries");
        let reverse_review = responder
            .receive_frame(take_frame(initiator))
            .expect("review reverse entries");
        assert_eq!(reverse_review.kind, SyncOutcomeKind::ReviewImport);
        assert_eq!(
            responder
                .accept_import()
                .expect("accept reverse import")
                .kind,
            SyncOutcomeKind::FrameReady
        );
    }
    assert_eq!(
        initiator
            .receive_frame(take_frame(responder))
            .expect("receive complete")
            .kind,
        SyncOutcomeKind::Complete
    );
}

#[test]
fn nearby_sync_carries_app_data_and_shared_app_without_fake_alert_rows() {
    let sender = open_local_profile().expect("sender");
    let space = sender
        .create_public_space("App sync".into())
        .expect("space");
    let sender_runtime = sender.app_runtime();
    let (manifest_bytes, bundle_bytes) = manifest_and_bundle();
    let app = sender_runtime
        .install_app(manifest_bytes, bundle_bytes)
        .expect("install");
    sender_runtime
        .app_data_put(
            app.app_id.clone(),
            "items/a".into(),
            b"from sender".to_vec(),
        )
        .expect("put app data");
    sender_runtime
        .share_app(app.app_id_bytes.clone(), space.clone())
        .expect("share app");

    let receiver = open_local_profile().expect("receiver");
    receiver.join_public_space(space, Vec::new()).expect("join");
    let (initiator, responder, review) = sync_to_review(&receiver, &sender);
    assert_eq!(review.kind, SyncOutcomeKind::ReviewImport);
    assert!(review.entries.is_empty(), "app entries are not fake alerts");
    assert!(review.import_bundle_bytes.is_some());
    accept_and_finish(&initiator, &responder);

    assert_eq!(
        receiver
            .app_runtime()
            .app_data_get(app.app_id.clone(), "items/a".into())
            .expect("read synced app data"),
        Some(b"from sender".to_vec())
    );
    let listing = receiver
        .app_runtime()
        .directory_listings()
        .expect("directory")
        .into_iter()
        .find(|listing| listing.app_id == app.app_id_bytes)
        .expect("synced shared app");
    assert!(listing.bundle_present);
    assert!(!listing.built_in);
}

#[test]
fn nearby_sync_mixes_alert_ui_with_app_entries_and_commits_both() {
    let sender = open_local_profile().expect("sender");
    let space = sender
        .create_public_space("Mixed sync".into())
        .expect("space");
    let signed_alert = sender
        .sign_draft(
            sender
                .create_draft_alert(alert_input())
                .expect("draft")
                .draft_id,
        )
        .expect("sign alert");
    let runtime = sender.app_runtime();
    let (manifest_bytes, bundle_bytes) = manifest_and_bundle();
    let app = runtime
        .install_app(manifest_bytes, bundle_bytes)
        .expect("install");
    runtime
        .app_data_put(app.app_id.clone(), "items/a".into(), b"mixed".to_vec())
        .expect("put");

    let receiver = open_local_profile().expect("receiver");
    receiver.join_public_space(space, Vec::new()).expect("join");
    let (initiator, responder, review) = sync_to_review(&receiver, &sender);
    assert_eq!(review.kind, SyncOutcomeKind::ReviewImport);
    assert_eq!(review.entries, vec![signed_alert.entry.clone()]);
    accept_and_finish(&initiator, &responder);
    assert_eq!(
        receiver.list_current_entries().unwrap(),
        vec![signed_alert.entry]
    );
    assert_eq!(
        receiver
            .app_runtime()
            .app_data_get(app.app_id, "items/a".into())
            .unwrap(),
        Some(b"mixed".to_vec())
    );
}

#[test]
fn portable_app_only_review_can_plan_hidden_entries_without_fake_rows() {
    let sender = open_local_profile().unwrap();
    let space = sender.create_public_space("Portable apps".into()).unwrap();
    let runtime = sender.app_runtime();
    let (manifest_bytes, bundle_bytes) = manifest_and_bundle();
    let app = runtime.install_app(manifest_bytes, bundle_bytes).unwrap();
    let receipt = runtime
        .app_data_put_with_receipt(app.app_id.clone(), "items/a".into(), b"portable".to_vec())
        .unwrap();

    let receiver = open_local_profile().unwrap();
    receiver.join_public_space(space, Vec::new()).unwrap();
    let preview = receiver.inspect_bytes(receipt, "portable".into()).unwrap();
    assert!(preview.eligible_entries().unwrap().is_empty());
    let accepted = preview.create_plan(Vec::new()).unwrap().accept().unwrap();
    assert_eq!(accepted.accepted_entry_ids.len(), 1);
    assert_eq!(
        receiver
            .app_runtime()
            .app_data_get(app.app_id, "items/a".into())
            .unwrap(),
        Some(b"portable".to_vec())
    );
}

#[test]
fn nearby_sync_reconciles_trust_and_revoke_for_the_same_organizer_identity() {
    let sender = open_local_profile().expect("sender");
    let space = sender
        .create_public_space("Trust sync".into())
        .expect("space");
    let wrapping_key = vec![0x5a; 32];
    let sealed = sender
        .seal_identity(wrapping_key.clone())
        .expect("seal identity");
    let receiver = open_profile_from_sealed_identity(wrapping_key, sealed).expect("restore");
    receiver.join_public_space(space, Vec::new()).expect("join");

    let starter =
        riot_core::apps::starter::verify_starter_catalog(riot_core::apps::starter::STARTER_CATALOG)
            .pop()
            .expect("starter");
    let app_id: String = starter
        .app_id
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    sender
        .app_runtime()
        .trust_app(app_id.clone())
        .expect("trust");
    let (initiator, responder, review) = sync_to_review(&receiver, &sender);
    assert!(review.entries.is_empty());
    accept_and_finish(&initiator, &responder);
    assert!(receiver
        .app_runtime()
        .is_app_trusted(app_id.clone())
        .unwrap());

    sender
        .app_runtime()
        .untrust_app(app_id.clone())
        .expect("revoke");
    let (initiator, responder, _) = sync_to_review(&receiver, &sender);
    accept_and_finish(&initiator, &responder);
    assert!(!receiver.app_runtime().is_app_trusted(app_id).unwrap());
}

#[test]
fn rejected_app_only_sync_never_commits_app_data() {
    let sender = open_local_profile().expect("sender");
    let space = sender
        .create_public_space("Reject apps".into())
        .expect("space");
    let runtime = sender.app_runtime();
    let (manifest_bytes, bundle_bytes) = manifest_and_bundle();
    let app = runtime.install_app(manifest_bytes, bundle_bytes).unwrap();
    runtime
        .app_data_put(app.app_id.clone(), "items/a".into(), b"no".to_vec())
        .unwrap();
    let cancelled_receiver = open_local_profile().unwrap();
    cancelled_receiver
        .join_public_space(space.clone(), Vec::new())
        .unwrap();
    let (cancelled, cancelled_responder, review) = sync_to_review(&cancelled_receiver, &sender);
    assert!(review.entries.is_empty());
    cancelled.cancel().unwrap();
    cancelled_responder.cancel().unwrap();
    assert_eq!(
        cancelled_receiver
            .app_runtime()
            .app_data_get(app.app_id.clone(), "items/a".into())
            .unwrap(),
        None
    );

    let receiver = open_local_profile().unwrap();
    receiver.join_public_space(space, Vec::new()).unwrap();
    let (initiator, responder, review) = sync_to_review(&receiver, &sender);
    assert!(review.entries.is_empty());
    assert!(initiator.reject_import(9).unwrap().terminal);
    let rejected = responder.receive_frame(take_frame(&initiator)).unwrap();
    assert_eq!(rejected.kind, SyncOutcomeKind::Rejected);
    assert_eq!(
        receiver
            .app_runtime()
            .app_data_get(app.app_id, "items/a".into())
            .unwrap(),
        None
    );
}

#[test]
fn app_overwrite_prunes_old_proof_and_exact_sync_id_limit_is_enforced() {
    let sender = open_local_profile().unwrap();
    let space = sender
        .create_public_space("Inventory bounds".into())
        .unwrap();
    let runtime = sender.app_runtime();
    let (manifest_bytes, bundle_bytes) = manifest_and_bundle();
    let app = runtime.install_app(manifest_bytes, bundle_bytes).unwrap();
    runtime
        .app_data_put(
            app.app_id.clone(),
            "items/overwrite".into(),
            b"old".to_vec(),
        )
        .unwrap();
    runtime
        .app_data_put(
            app.app_id.clone(),
            "items/overwrite".into(),
            b"new".to_vec(),
        )
        .unwrap();
    for index in 1..riot_core::sync::MAX_SYNC_IDS {
        runtime
            .app_data_put(
                app.app_id.clone(),
                format!("items/item-{index}"),
                vec![index as u8],
            )
            .expect("exact cap remains admissible");
    }
    sender
        .open_sync_session()
        .expect("64 live entries sync")
        .cancel()
        .unwrap();
    assert!(matches!(
        runtime.app_data_put(app.app_id.clone(), "items/overflow".into(), b"x".to_vec()),
        Err(MobileError::SessionLimit)
    ));

    let receiver = open_local_profile().unwrap();
    receiver.join_public_space(space, Vec::new()).unwrap();
    let (initiator, responder, _) = sync_to_review(&receiver, &sender);
    accept_and_finish(&initiator, &responder);
    assert_eq!(
        receiver
            .app_runtime()
            .app_data_get(app.app_id, "items/overwrite".into())
            .unwrap(),
        Some(b"new".to_vec())
    );
}

#[test]
fn app_prefix_write_prunes_descendant_proofs_before_sync() {
    let sender = open_local_profile().unwrap();
    let space = sender.create_public_space("Prefix pruning".into()).unwrap();
    let runtime = sender.app_runtime();
    let (manifest_bytes, bundle_bytes) = manifest_and_bundle();
    let app = runtime.install_app(manifest_bytes, bundle_bytes).unwrap();
    runtime
        .app_data_put(app.app_id.clone(), "items/a".into(), b"child".to_vec())
        .unwrap();
    runtime
        .app_data_put(app.app_id.clone(), "items".into(), b"parent".to_vec())
        .unwrap();
    sender.open_sync_session().unwrap().cancel().unwrap();

    let receiver = open_local_profile().unwrap();
    receiver.join_public_space(space, Vec::new()).unwrap();
    let (initiator, responder, _) = sync_to_review(&receiver, &sender);
    accept_and_finish(&initiator, &responder);
    assert_eq!(
        receiver
            .app_runtime()
            .app_data_get(app.app_id.clone(), "items/a".into())
            .unwrap(),
        None
    );
    assert_eq!(
        receiver
            .app_runtime()
            .app_data_get(app.app_id, "items".into())
            .unwrap(),
        Some(b"parent".to_vec())
    );
}

#[test]
fn share_pair_is_atomic_when_only_one_sync_id_slot_remains() {
    let profile = open_local_profile().unwrap();
    let space = profile
        .create_public_space("Atomic share IDs".into())
        .unwrap();
    let runtime = profile.app_runtime();
    let (manifest_bytes, bundle_bytes) = manifest_and_bundle();
    let app = runtime.install_app(manifest_bytes, bundle_bytes).unwrap();
    for index in 0..(riot_core::sync::MAX_SYNC_IDS - 1) {
        runtime
            .app_data_put(
                app.app_id.clone(),
                format!("items/value-{index}"),
                vec![index as u8],
            )
            .unwrap();
    }

    assert!(matches!(
        runtime.share_app(app.app_id_bytes.clone(), space),
        Err(MobileError::SessionLimit)
    ));
    assert!(runtime
        .directory_listings()
        .unwrap()
        .iter()
        .all(|listing| listing.app_id != app.app_id_bytes));

    // An unchanged 63-entry inventory remains complete and still has one
    // free slot for an ordinary single-entry write.
    profile.open_sync_session().unwrap().cancel().unwrap();
    runtime
        .app_data_put(app.app_id, "items/after-failure".into(), b"ok".to_vec())
        .expect("failed pair retained the final slot");
}

#[test]
fn share_pair_is_atomic_when_manifest_fits_but_pair_exceeds_inventory_bytes() {
    let profile = open_local_profile().unwrap();
    let space = profile
        .create_public_space("Atomic share bytes".into())
        .unwrap();
    let runtime = profile.app_runtime();
    let (small_manifest, small_bundle) = manifest_and_bundle();
    let data_app = runtime.install_app(small_manifest, small_bundle).unwrap();
    for index in 0..7 {
        runtime
            .app_data_put(
                data_app.app_id.clone(),
                format!("items/large-{index}"),
                vec![index as u8; riot_core::import::MAX_ITEM_PAYLOAD_BYTES],
            )
            .unwrap();
    }
    let (large_manifest, large_bundle) = manifest_and_bundle_with_resource(vec![0x5a; 1_048_000]);
    let large_app = runtime.install_app(large_manifest, large_bundle).unwrap();

    assert!(matches!(
        runtime.share_app(large_app.app_id_bytes.clone(), space),
        Err(MobileError::SessionLimit)
    ));
    assert!(runtime
        .directory_listings()
        .unwrap()
        .iter()
        .all(|listing| listing.app_id != large_app.app_id_bytes));
    profile
        .open_sync_session()
        .expect("failed pair retained a bounded complete inventory")
        .cancel()
        .unwrap();
}

#[test]
fn aggregate_sync_inventory_is_bounded_to_one_bundle_before_session_clone() {
    let profile = open_local_profile().unwrap();
    profile.create_public_space("Byte bound".into()).unwrap();
    let runtime = profile.app_runtime();
    let (manifest_bytes, bundle_bytes) = manifest_and_bundle();
    let app = runtime.install_app(manifest_bytes, bundle_bytes).unwrap();
    for index in 0..7 {
        runtime
            .app_data_put(
                app.app_id.clone(),
                format!("items/large-{index}"),
                vec![index as u8; riot_core::import::MAX_ITEM_PAYLOAD_BYTES],
            )
            .expect("inventory below aggregate byte cap");
    }
    assert!(matches!(
        runtime.app_data_put(
            app.app_id,
            "items/large-7".into(),
            vec![7; riot_core::import::MAX_ITEM_PAYLOAD_BYTES],
        ),
        Err(MobileError::SessionLimit)
    ));
    profile
        .open_sync_session()
        .expect("bounded inventory opens")
        .cancel()
        .unwrap();
}

#[test]
fn app_import_rejects_foreign_namespace_and_spoofed_index_slots() {
    use riot_core::apps::endorse::{encode_endorsement, EndorsementMarker};
    use riot_core::apps::entry::app_data_path;
    use riot_core::apps::index::app_index_endorsement_path;
    use riot_core::willow::generate_communal_author;

    let receiver = open_local_profile().unwrap();
    receiver.create_public_space("Admission".into()).unwrap();
    let foreign = generate_communal_author().unwrap();
    let app_id = [0x33; 32];
    let foreign_app = signed_bundle_at(
        &foreign,
        app_data_path(&app_id, "items/a").unwrap(),
        b"foreign",
    );
    assert!(matches!(
        receiver.inspect_bytes(foreign_app, "nearby".into()),
        Err(MobileError::ImportRejected)
    ));

    let author_identity = receiver.identity().unwrap();
    let author = riot_core::willow::generate_communal_author_for_namespace(
        unhex(&author_identity.namespace_id).try_into().unwrap(),
    )
    .unwrap();
    let marker = EndorsementMarker {
        app_id,
        note: "spoof".into(),
        retracted: false,
    };
    let spoofed_slot = app_index_endorsement_path(&app_id, &[0x77; 32]).unwrap();
    let spoofed = frame_unchecked(&signed_entry_at(
        &author,
        spoofed_slot,
        &encode_endorsement(&marker).unwrap(),
    ));
    assert!(matches!(
        receiver.inspect_bytes(spoofed, "nearby".into()),
        Err(MobileError::ImportRejected)
    ));

    let wrong_payload = EndorsementMarker {
        app_id: [0x44; 32],
        note: "wrong app".into(),
        retracted: false,
    };
    let mismatched = frame_unchecked(&signed_entry_at(
        &author,
        app_index_endorsement_path(&app_id, author.subspace_id().as_bytes()).unwrap(),
        &encode_endorsement(&wrong_payload).unwrap(),
    ));
    assert!(matches!(
        receiver.inspect_bytes(mismatched, "nearby".into()),
        Err(MobileError::ImportRejected)
    ));
}

#[test]
fn install_returns_a_deterministic_app_id_and_rejects_garbage() {
    let profile = open_local_profile().expect("profile");
    let runtime = profile.app_runtime();
    let (manifest_bytes, bundle_bytes) = manifest_and_bundle();

    let first = runtime
        .install_app(manifest_bytes.clone(), bundle_bytes.clone())
        .expect("install");
    assert_eq!(first.name, "Checklist");
    assert_eq!(first.entry_point, "index.html");
    assert_eq!(first.app_id.len(), 64); // 32 bytes, hex

    // Reinstalling the same pair is idempotent and yields the same id.
    let second = runtime
        .install_app(manifest_bytes, bundle_bytes)
        .expect("reinstall");
    assert_eq!(first.app_id, second.app_id);

    assert!(matches!(
        runtime.install_app(vec![0xff; 8], vec![0xff; 8]),
        Err(MobileError::AppRejected)
    ));
}

#[test]
fn install_refuses_a_bundle_that_references_webrtc() {
    // Risk 9: WebRTC egress bypasses the runtime URL-loader backstop, so a
    // bundle that references it is refused at import — before it can ever be
    // hosted or served. The refusal is by CONTENT: the script is hidden in a
    // resource that names itself a stylesheet, and it is still refused.
    let profile = open_local_profile().expect("profile");
    let runtime = profile.app_runtime();

    let (clean_manifest, clean_bundle) = manifest_and_bundle();
    runtime
        .install_app(clean_manifest, clean_bundle)
        .expect("a clean bundle installs");

    let (manifest_bytes, hostile_bundle) = manifest_and_bundle_with_resource(
        b"body{color:red} /* new RTCPeerConnection({iceServers:[]}) */".to_vec(),
    );
    assert!(
        matches!(
            runtime.install_app(manifest_bytes, hostile_bundle),
            Err(MobileError::AppRejected)
        ),
        "a WebRTC-referencing bundle must not install, regardless of resource labeling",
    );
}

#[test]
fn only_the_recognized_organizer_grants_app_authority() {
    // The demo (Unit 0B) makes tools openable by shipping the ORGANIZER's
    // signed Trust markers in the space, not by a bypass. This pins the other
    // half of that guarantee: a Trust marker signed by anyone who is NOT the
    // recognized organizer is admitted as a well-formed entry but grants no
    // authority. Only the organizer coordinate (subspace == namespace) counts.
    use riot_core::apps::index::app_index_trust_path;
    use riot_core::apps::trust::{encode_trust_marker, TrustMarker, TrustMarkerKind};
    use riot_core::willow::generate_communal_author_for_namespace;

    // A locally opened profile is organizer-shaped, so it is the recognized
    // organizer of its own space.
    let profile = open_local_profile().expect("profile");
    profile
        .create_public_space("Authority".into())
        .expect("space");
    let runtime = profile.app_runtime();

    let starter =
        riot_core::apps::starter::verify_starter_catalog(riot_core::apps::starter::STARTER_CATALOG)
            .pop()
            .expect("starter");
    let app_id_hex: String = starter.app_id.iter().map(|b| format!("{b:02x}")).collect();
    assert!(!runtime
        .is_app_trusted(app_id_hex.clone())
        .expect("starts untrusted"));

    // A member of the same space — a fresh subspace, which is NOT the namespace
    // coordinate — signs a Trust marker for the app at its own trust slot. The
    // slot is well formed and ownership binds (signer == the slot's subspace),
    // so import admits it. It must still grant nothing.
    let namespace_id: [u8; 32] = unhex(&profile.identity().expect("identity").namespace_id)
        .try_into()
        .expect("namespace id");
    let member = generate_communal_author_for_namespace(namespace_id).expect("member");
    assert_ne!(
        *member.subspace_id().as_bytes(),
        namespace_id,
        "the member is deliberately not the organizer coordinate"
    );
    let marker = TrustMarker {
        app_id: starter.app_id,
        author_subspace_id: *member.subspace_id().as_bytes(),
        kind: TrustMarkerKind::Trust,
        timestamp_micros: 0,
    };
    let payload = encode_trust_marker(&marker).expect("encode marker");
    let path =
        app_index_trust_path(&starter.app_id, member.subspace_id().as_bytes()).expect("trust path");
    let bundle = signed_bundle_at(&member, path, &payload);
    let preview = profile
        .inspect_bytes(bundle, "nearby".into())
        .expect("inspect");
    preview
        .create_plan(Vec::new())
        .expect("plan")
        .accept()
        .expect("accept");

    // The member's marker is now live in the store, yet the app is still
    // untrusted: only the recognized organizer's decision carries authority.
    assert!(
        !runtime
            .is_app_trusted(app_id_hex.clone())
            .expect("member marker ignored"),
        "a non-organizer's Trust marker grants no authority — no bypass"
    );

    // Airtight no-bypass: a marker forged AT the organizer's own coordinate but
    // signed by someone else is REFUSED at import. Authority follows the
    // organizer's verified signature, never mere presence at the coordinate —
    // this is exactly what separates "deterministic admission" from a bypass.
    let spoof_path =
        app_index_trust_path(&starter.app_id, &namespace_id).expect("organizer coordinate path");
    let spoof = signed_bundle_at(&member, spoof_path, &payload);
    assert!(
        matches!(
            profile.inspect_bytes(spoof, "nearby".into()),
            Err(MobileError::ImportRejected)
        ),
        "a Trust marker at the organizer coordinate signed by a non-organizer is refused at import"
    );
    assert!(
        !runtime
            .is_app_trusted(app_id_hex.clone())
            .expect("spoof granted nothing"),
        "a forged organizer-coordinate marker never reaches the store, so it grants nothing"
    );

    // The organizer's own decision does grant it — real, verified authority.
    runtime
        .trust_app(app_id_hex.clone())
        .expect("organizer trusts");
    assert!(runtime
        .is_app_trusted(app_id_hex)
        .expect("organizer marker honored"));
}

#[test]
fn trust_lifecycle_is_lww_per_app() {
    let profile = open_local_profile().expect("profile");
    let runtime = profile.app_runtime();
    let (manifest_bytes, bundle_bytes) = manifest_and_bundle();
    let app = runtime
        .install_app(manifest_bytes, bundle_bytes)
        .expect("install");

    assert!(!runtime.is_app_trusted(app.app_id.clone()).expect("check"));
    runtime.trust_app(app.app_id.clone()).expect("trust");
    assert!(runtime.is_app_trusted(app.app_id.clone()).expect("check"));
    runtime.untrust_app(app.app_id.clone()).expect("untrust");
    assert!(!runtime.is_app_trusted(app.app_id.clone()).expect("check"));
    runtime.trust_app(app.app_id.clone()).expect("re-trust");
    assert!(runtime.is_app_trusted(app.app_id.clone()).expect("check"));
}

// --- WU-002a: two-phase trust prepare/persist/finalize ---

fn organizer_with_installed_untrusted_app() -> (Arc<MobileProfile>, Arc<AppRuntimeSession>, String)
{
    let profile = open_local_profile().expect("profile");
    let runtime = profile.app_runtime();
    let (manifest_bytes, bundle_bytes) = manifest_and_bundle();
    let app = runtime
        .install_app(manifest_bytes, bundle_bytes)
        .expect("install");
    assert!(!runtime
        .is_app_trusted(app.app_id.clone())
        .expect("untrusted"));
    (profile, runtime, app.app_id)
}

#[test]
fn prepare_trust_does_not_mutate_and_finalize_commits() {
    let (_profile, runtime, app_id) = organizer_with_installed_untrusted_app();
    let prepared = runtime
        .prepare_app_trust(app_id.clone(), true)
        .expect("prepare");
    assert!(prepared.trusted);
    assert!(!prepared.app_id.is_empty());
    // prepare must NOT touch the live store.
    assert!(
        !runtime.is_app_trusted(app_id.clone()).unwrap(),
        "prepare must not grant trust"
    );
    runtime.finalize_app_trust().expect("finalize");
    assert!(
        runtime.is_app_trusted(app_id).unwrap(),
        "finalize commits the grant"
    );
}

#[test]
fn prepare_without_finalize_leaves_trust_untouched() {
    // Simulates a crash between the durable persist and finalize: the live store
    // was never mutated by prepare, so trust stays off.
    let (_profile, runtime, app_id) = organizer_with_installed_untrusted_app();
    runtime.prepare_app_trust(app_id.clone(), true).unwrap();
    assert!(!runtime.is_app_trusted(app_id.clone()).unwrap());
    // A superseding prepare replaces the abandoned one, then finalize grants.
    runtime.prepare_app_trust(app_id.clone(), true).unwrap();
    runtime.finalize_app_trust().unwrap();
    assert!(runtime.is_app_trusted(app_id).unwrap());
}

#[test]
fn discard_clears_a_prepared_grant() {
    let (_profile, runtime, app_id) = organizer_with_installed_untrusted_app();
    runtime.prepare_app_trust(app_id.clone(), true).unwrap();
    runtime.discard_prepared_trust().unwrap();
    assert!(
        runtime.finalize_app_trust().is_err(),
        "nothing to finalize after discard"
    );
    assert!(!runtime.is_app_trusted(app_id).unwrap());
}

#[test]
fn re_issuing_trust_for_an_already_trusted_app_is_idempotent() {
    // Trust restart re-issues per persisted id (not a byte replay). Re-issue must
    // stay trusted with no marker-cap growth (LWW at the same coordinate).
    let (_profile, runtime, app_id) = organizer_with_installed_untrusted_app();
    runtime.prepare_app_trust(app_id.clone(), true).unwrap();
    runtime.finalize_app_trust().unwrap();
    assert!(runtime.is_app_trusted(app_id.clone()).unwrap());
    runtime.prepare_app_trust(app_id.clone(), true).unwrap();
    runtime.finalize_app_trust().unwrap();
    assert!(
        runtime.is_app_trusted(app_id).unwrap(),
        "still trusted, exactly once"
    );
}

#[test]
fn finalize_with_nothing_prepared_errors() {
    let (_profile, runtime, _app_id) = organizer_with_installed_untrusted_app();
    assert!(runtime.finalize_app_trust().is_err());
}

#[test]
fn app_data_round_trips_through_the_ffi_layer() {
    let profile = open_local_profile().expect("profile");
    let runtime = profile.app_runtime();
    let (manifest_bytes, bundle_bytes) = manifest_and_bundle();
    let app = runtime
        .install_app(manifest_bytes, bundle_bytes)
        .expect("install");
    runtime.trust_app(app.app_id.clone()).expect("trust");

    runtime
        .app_data_put(
            app.app_id.clone(),
            "items/a".to_string(),
            b"{\"done\":false}".to_vec(),
        )
        .expect("put");

    let value = runtime
        .app_data_get(app.app_id.clone(), "items/a".to_string())
        .expect("get");
    assert_eq!(value, Some(b"{\"done\":false}".to_vec()));

    let missing = runtime
        .app_data_get(app.app_id.clone(), "items/missing".to_string())
        .expect("get missing");
    assert_eq!(missing, None);

    let listed = runtime
        .app_data_list(app.app_id.clone(), "items".to_string())
        .expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].key, "items/a");
    assert_eq!(listed[0].value, b"{\"done\":false}".to_vec());
}

#[test]
fn hostile_inputs_are_rejected_without_state_damage() {
    let profile = open_local_profile().expect("profile");
    let runtime = profile.app_runtime();
    let (manifest_bytes, bundle_bytes) = manifest_and_bundle();
    let app = runtime
        .install_app(manifest_bytes, bundle_bytes)
        .expect("install");

    // Traversal-shaped key.
    assert!(matches!(
        runtime.app_data_put(app.app_id.clone(), "../escape".to_string(), b"x".to_vec()),
        Err(MobileError::AppRejected)
    ));
    // Malformed app ids (non-hex, wrong length).
    assert!(runtime.is_app_trusted("zz".repeat(32)).is_err());
    assert!(runtime
        .app_data_get("abcd".to_string(), "items/a".to_string())
        .is_err());

    // The profile still works afterwards.
    let listed = runtime
        .app_data_list(app.app_id, "items".to_string())
        .expect("list");
    assert!(listed.is_empty());
}

#[test]
fn app_data_put_does_not_break_sync_sessions() {
    // Regression (review C1): a put must neither brick a later
    // open_sync_session (sync-inventory completeness is alert-only) nor be
    // allowed while a sync session is active (store.inspect would clobber
    // the in-flight sync review).
    let profile = open_local_profile().expect("profile");
    profile
        .create_public_space("Sync fixture".into())
        .expect("space");
    let runtime = profile.app_runtime();
    let (manifest_bytes, bundle_bytes) = manifest_and_bundle();
    let app = runtime
        .install_app(manifest_bytes, bundle_bytes)
        .expect("install");
    runtime.trust_app(app.app_id.clone()).expect("trust");

    runtime
        .app_data_put(app.app_id.clone(), "items/a".to_string(), b"x".to_vec())
        .expect("put");

    let sync = profile.open_sync_session().expect("sync opens after a put");
    assert!(matches!(
        runtime.app_data_put(app.app_id.clone(), "items/b".to_string(), b"y".to_vec()),
        Err(MobileError::InvalidInput)
    ));
    sync.cancel().expect("cancel");

    runtime
        .app_data_put(app.app_id, "items/b".to_string(), b"y".to_vec())
        .expect("put works again after cancel");
}

#[test]
fn app_write_never_replaces_an_active_portable_review() {
    let sender = open_local_profile().unwrap();
    let space = sender.create_public_space("Review guard".into()).unwrap();
    let signed = sender
        .sign_draft(sender.create_draft_alert(alert_input()).unwrap().draft_id)
        .unwrap();
    let receiver = open_local_profile().unwrap();
    receiver
        .join_public_space(space.clone(), Vec::new())
        .unwrap();
    let preview = receiver
        .inspect_bytes(signed.bundle_bytes, "portable".into())
        .unwrap();
    let (manifest_bytes, bundle_bytes) = manifest_and_bundle();
    let app = receiver
        .app_runtime()
        .install_app(manifest_bytes, bundle_bytes)
        .unwrap();
    assert!(matches!(
        receiver
            .app_runtime()
            .share_app(app.app_id_bytes.clone(), space),
        Err(MobileError::InvalidInput)
    ));
    assert!(matches!(
        receiver.app_runtime().app_data_put(
            app.app_id.clone(),
            "items/a".into(),
            b"blocked".to_vec()
        ),
        Err(MobileError::InvalidInput)
    ));
    assert!(receiver
        .app_runtime()
        .directory_listings()
        .unwrap()
        .iter()
        .all(|listing| listing.app_id != app.app_id_bytes));
    assert_eq!(preview.eligible_entries().unwrap(), vec![signed.entry]);
}

#[test]
fn shared_app_appears_in_directory_with_carrier_provenance() {
    let profile = open_local_profile().expect("profile");
    let space = profile
        .create_public_space("Directory fixture".into())
        .expect("space");
    let runtime = profile.app_runtime();
    let (manifest_bytes, bundle_bytes) = manifest_and_bundle();
    let app = runtime
        .install_app(manifest_bytes, bundle_bytes)
        .expect("install");
    // The raw-bytes id crosses straight into the directory surface — no
    // hex bridging on the native side.
    let app_id = app.app_id_bytes.clone();
    assert_eq!(app_id, unhex(&app.app_id));

    // Not listed before sharing: install alone publishes nothing.
    let before = runtime.directory_listings().expect("listings");
    assert!(before.iter().all(|listing| listing.app_id != app_id));

    runtime
        .share_app(app_id.clone(), space.clone())
        .expect("share");

    let listings = runtime.directory_listings().expect("listings");
    let listing = listings
        .iter()
        .find(|listing| listing.app_id == app_id)
        .expect("shared app listed");
    assert_eq!(listing.name, "Checklist");
    assert_eq!(listing.version, "1.0.0");
    assert!(listing.bundle_present);
    assert!(!listing.built_in);
    assert!(listing.installed);
    assert!(listing.carrier_subspace_id.is_some());
    assert_eq!(listing.superseded_by, None);
    assert!(listing.trusted_in_spaces.is_empty()); // sharing never auto-trusts

    // A space the profile has not joined is not a valid share target.
    let foreign_space = PublicSpace {
        namespace_id: "ab".repeat(32),
        title: "Elsewhere".into(),
        is_public: true,
    };
    assert!(matches!(
        runtime.share_app(app_id, foreign_space),
        Err(MobileError::InvalidInput)
    ));

    // Sharing an app id nothing local can resolve has nothing to publish.
    assert!(matches!(
        runtime.share_app(vec![0x5a; 32], space),
        Err(MobileError::AppRejected)
    ));
}

#[test]
fn starter_checklist_is_listed_built_in_with_canonical_id() {
    let profile = open_local_profile().expect("profile");
    let runtime = profile.app_runtime();

    let expected =
        riot_core::apps::starter::verify_starter_catalog(riot_core::apps::starter::STARTER_CATALOG);
    assert!(
        !expected.is_empty(),
        "embedded starter catalog must verify against its own codecs"
    );

    let listings = runtime.directory_listings().expect("listings");
    for starter in &expected {
        let listing = listings
            .iter()
            .find(|listing| listing.app_id == starter.app_id.to_vec())
            .expect("starter app listed under its canonical id");
        assert!(listing.built_in);
        assert!(listing.bundle_present);
        assert!(!listing.installed); // built-ins start uninstalled
        assert!(listing.carrier_subspace_id.is_none());
        assert_eq!(listing.name, starter.manifest.name);
    }
}

#[test]
fn installing_a_starter_pair_flips_the_listing_installed_flag() {
    let profile = open_local_profile().expect("profile");
    let runtime = profile.app_runtime();
    let (manifest_bytes, bundle_bytes) = riot_core::apps::starter::STARTER_CATALOG[0];

    let record = runtime
        .install_app(manifest_bytes.to_vec(), bundle_bytes.to_vec())
        .expect("install starter pair");
    // The raw-bytes id matches the canonical starter id and the listing id
    // directly — no hex bridging.
    let starter =
        riot_core::apps::starter::verify_starter_catalog(riot_core::apps::starter::STARTER_CATALOG)
            .remove(0);
    assert_eq!(record.app_id_bytes, starter.app_id.to_vec());

    let listings = runtime.directory_listings().expect("listings");
    let listing = listings
        .iter()
        .find(|listing| listing.app_id == record.app_id_bytes)
        .expect("starter listed");
    assert!(listing.built_in);
    assert!(listing.installed);
}

#[test]
fn trusting_an_app_marks_the_space_in_listings() {
    let profile = open_local_profile().expect("profile");
    let runtime = profile.app_runtime();
    let starter =
        riot_core::apps::starter::verify_starter_catalog(riot_core::apps::starter::STARTER_CATALOG)
            .pop()
            .expect("starter app");
    let app_id_hex: String = starter.app_id.iter().map(|b| format!("{b:02x}")).collect();
    let own_namespace = unhex(&profile.identity().expect("identity").namespace_id);

    runtime.trust_app(app_id_hex.clone()).expect("trust");
    let listings = runtime.directory_listings().expect("listings");
    let listing = listings
        .iter()
        .find(|listing| listing.app_id == starter.app_id.to_vec())
        .expect("starter listed");
    assert_eq!(listing.trusted_in_spaces, vec![own_namespace]);

    runtime.untrust_app(app_id_hex).expect("untrust");
    let listings = runtime.directory_listings().expect("listings");
    let listing = listings
        .iter()
        .find(|listing| listing.app_id == starter.app_id.to_vec())
        .expect("starter listed");
    assert!(listing.trusted_in_spaces.is_empty());
}

#[test]
fn endorsement_bumps_counts_and_retraction_clears_them() {
    let profile = open_local_profile().expect("profile");
    let runtime = profile.app_runtime();
    let starter =
        riot_core::apps::starter::verify_starter_catalog(riot_core::apps::starter::STARTER_CATALOG)
            .pop()
            .expect("starter app");
    let app_id = starter.app_id.to_vec();

    runtime
        .endorse_app(app_id.clone(), "we ran the drill with this".into(), false)
        .expect("endorse");
    let listings = runtime.directory_listings().expect("listings");
    let listing = listings
        .iter()
        .find(|listing| listing.app_id == app_id)
        .expect("starter listed");
    // The endorsement entry itself is live in the local store, so this
    // profile's own subspace counts as met.
    assert_eq!(listing.endorsing_met_subspaces.len(), 1);
    assert_eq!(listing.endorsing_unmet_count, 0);

    // The named method is the same operation as endorse_app(.., "", true).
    runtime
        .retract_endorsement(app_id.clone())
        .expect("retract");
    let listings = runtime.directory_listings().expect("listings");
    let listing = listings
        .iter()
        .find(|listing| listing.app_id == app_id)
        .expect("starter listed");
    assert!(listing.endorsing_met_subspaces.is_empty());
    assert_eq!(listing.endorsing_unmet_count, 0);

    // Note length is enforced by the core codec, surfaced as AppRejected.
    assert!(matches!(
        runtime.endorse_app(app_id, "x".repeat(201), false),
        Err(MobileError::AppRejected)
    ));
    // Malformed app id.
    assert!(matches!(
        runtime.endorse_app(vec![1; 8], String::new(), false),
        Err(MobileError::InvalidInput)
    ));
}

#[test]
fn share_and_endorse_respect_active_sync_and_never_brick_it() {
    // Same discipline as app_data_put: app-index writes are refused while a
    // sync session is active, and entries they add must not violate the
    // alert-only sync-inventory completeness invariant afterwards.
    let profile = open_local_profile().expect("profile");
    let space = profile
        .create_public_space("Sync guard fixture".into())
        .expect("space");
    let runtime = profile.app_runtime();
    let (manifest_bytes, bundle_bytes) = manifest_and_bundle();
    let app = runtime
        .install_app(manifest_bytes, bundle_bytes)
        .expect("install");
    let app_id = unhex(&app.app_id);

    runtime
        .share_app(app_id.clone(), space.clone())
        .expect("share");
    runtime
        .endorse_app(app_id.clone(), "endorsed".into(), false)
        .expect("endorse");

    let sync = profile
        .open_sync_session()
        .expect("sync opens after app-index writes");
    assert!(matches!(
        runtime.share_app(app_id.clone(), space.clone()),
        Err(MobileError::InvalidInput)
    ));
    assert!(matches!(
        runtime.endorse_app(app_id.clone(), String::new(), true),
        Err(MobileError::InvalidInput)
    ));
    sync.cancel().expect("cancel");

    runtime
        .share_app(app_id.clone(), space)
        .expect("re-share works after cancel");
    runtime
        .endorse_app(app_id, String::new(), true)
        .expect("retract works after cancel");
}

#[test]
fn trust_toggles_never_exhaust_the_marker_cap() {
    // Regression (review M2): well below the store's separate 256-receipt
    // lifetime bound, markers compact to latest-per-app, so the marker cap
    // bounds distinct apps rather than ordinary toggle count. Automatic
    // receipt compaction is a separate follow-up (iOS plan d370ac0).
    let profile = open_local_profile().expect("profile");
    let runtime = profile.app_runtime();
    let (manifest_bytes, bundle_bytes) = manifest_and_bundle();
    let app = runtime
        .install_app(manifest_bytes, bundle_bytes)
        .expect("install");

    for _ in 0..100 {
        runtime.trust_app(app.app_id.clone()).expect("trust");
        runtime.untrust_app(app.app_id.clone()).expect("untrust");
    }
    runtime.trust_app(app.app_id.clone()).expect("final trust");
    assert!(runtime.is_app_trusted(app.app_id).expect("check"));
}

#[test]
fn trust_store_full_leaves_cache_and_sync_inventory_on_the_last_committed_marker() {
    let profile = open_local_profile().expect("profile");
    profile.create_public_space("Receipt bound".into()).unwrap();
    let runtime = profile.app_runtime();
    let starter =
        riot_core::apps::starter::verify_starter_catalog(riot_core::apps::starter::STARTER_CATALOG)
            .pop()
            .unwrap();
    let app_id: String = starter
        .app_id
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();

    for index in 0..256 {
        if index % 2 == 0 {
            runtime
                .trust_app(app_id.clone())
                .expect("within receipt bound");
        } else {
            runtime
                .untrust_app(app_id.clone())
                .expect("within receipt bound");
        }
    }
    assert!(!runtime.is_app_trusted(app_id.clone()).unwrap());
    assert!(matches!(
        runtime.trust_app(app_id.clone()),
        Err(MobileError::StoreFull)
    ));
    assert!(
        !runtime.is_app_trusted(app_id).unwrap(),
        "cache changes only after a successful commit"
    );
    profile
        .open_sync_session()
        .expect("failed write did not break complete inventory")
        .cancel()
        .unwrap();
}

#[test]
fn app_data_persists_across_a_fresh_profile_via_replay() {
    // The relaunch-persistence contract end to end: a put hands back the
    // committed bundle bytes, and a FRESH profile that joins the same space
    // and replays those bytes reads the value back — exactly how the native
    // host survives a process restart of the in-memory Rust store.
    let author = open_local_profile().expect("profile");
    let space = author
        .create_public_space("Persist fixture".into())
        .expect("space");
    let runtime = author.app_runtime();
    let (manifest_bytes, bundle_bytes) = manifest_and_bundle();
    let app = runtime
        .install_app(manifest_bytes, bundle_bytes)
        .expect("install");

    let receipt = runtime
        .app_data_put_with_receipt(
            app.app_id.clone(),
            "items/a".to_string(),
            b"{\"done\":true}".to_vec(),
        )
        .expect("put with receipt");
    assert!(!receipt.is_empty(), "receipt carries the committed bundle");

    let reopened = open_local_profile().expect("fresh profile");
    reopened
        .join_public_space(space, Vec::new())
        .expect("join same space");
    let reopened_runtime = reopened.app_runtime();
    reopened_runtime
        .replay_app_data_bundle(receipt)
        .expect("replay committed bundle");

    let value = reopened_runtime
        .app_data_get(app.app_id, "items/a".to_string())
        .expect("get after replay");
    assert_eq!(value, Some(b"{\"done\":true}".to_vec()));
}

#[test]
fn replay_rejects_a_non_app_data_bundle() {
    // Strictness: replay is app-data-only. A real signed alert bundle (a
    // non-app-data path) must be refused, so replay can never smuggle alert
    // entries past the alert review surface.
    let profile = open_local_profile().expect("profile");
    profile
        .create_public_space("Alert fixture".into())
        .expect("space");
    let draft = profile
        .create_draft_alert(alert_input())
        .expect("draft alert");
    let signed = profile.sign_draft(draft.draft_id).expect("sign draft");

    let runtime = profile.app_runtime();
    assert!(matches!(
        runtime.replay_app_data_bundle(signed.bundle_bytes),
        Err(MobileError::ImportRejected)
    ));
    // Garbage bytes are refused the same way.
    assert!(matches!(
        runtime.replay_app_data_bundle(vec![0xff; 16]),
        Err(MobileError::ImportRejected)
    ));
}

#[test]
fn app_display_name_is_the_rendered_name_and_never_full_key_material() {
    let profile = open_local_profile().expect("profile");
    let runtime = profile.app_runtime();

    // Before anyone claims a name: the fallback, in the SAME `<name> · <tag>`
    // shape a claimed name takes. This used to be a bare `member-<hex>` label
    // with nowhere for a real name to go.
    let name = runtime.app_display_name().expect("display name");
    let tag = name.strip_prefix("member · ").expect("member · <tag>");
    assert_eq!(tag.len(), 8, "8 hex chars = first 4 subspace bytes");
    assert!(tag
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    assert_ne!(name.len(), 64, "must not be full 64-hex key material");

    // Stable across calls within a profile.
    assert_eq!(runtime.app_display_name().expect("name again"), name);

    // Once a name is claimed, this is what `riot.whoami()` shows — the claimed
    // name, still carrying the key tag that makes it comparable.
    profile
        .profile()
        .set_display_name("Ana".into())
        .expect("set name");
    let named = runtime.app_display_name().expect("named");
    assert_eq!(named, format!("Ana · {tag}"));
    assert_ne!(named.len(), 64, "must not be full 64-hex key material");
}

// ---------------------------------------------------------------------------
// Opening a carried app: the last hop of community discovery. An app that
// arrived over nearby sync could be listed but never run — install_app was the
// only way into the runtime, and it needs bytes the receiver never had. These
// pin that a carried app installs from the store's own bytes, on exactly the
// same terms as a direct install.
// ---------------------------------------------------------------------------

#[test]
fn a_carried_app_installs_from_the_store_exactly_as_a_direct_install_would() {
    let sender = open_local_profile().expect("sender");
    let space = sender
        .create_public_space("Carried apps".into())
        .expect("space");
    let sender_runtime = sender.app_runtime();
    let (manifest_bytes, bundle_bytes) = manifest_and_bundle();
    let direct = sender_runtime
        .install_app(manifest_bytes, bundle_bytes)
        .expect("direct install");
    sender_runtime
        .share_app(direct.app_id_bytes.clone(), space.clone())
        .expect("share app");

    // The app arrives at a neighbour who has never held its manifest or bundle.
    let receiver = open_local_profile().expect("receiver");
    receiver.join_public_space(space, Vec::new()).expect("join");
    let (initiator, responder, review) = sync_to_review(&receiver, &sender);
    assert_eq!(review.kind, SyncOutcomeKind::ReviewImport);
    accept_and_finish(&initiator, &responder);

    let receiver_runtime = receiver.app_runtime();
    let listing = receiver_runtime
        .directory_listings()
        .expect("directory")
        .into_iter()
        .find(|listing| listing.app_id == direct.app_id_bytes)
        .expect("the carried app is listed");
    assert!(listing.bundle_present);
    assert!(!listing.installed, "seen, but not yet opened");

    // Opening it passes no bytes at all — they come from the store — and lands
    // the identical record a direct install of the same pair produces.
    let carried = receiver_runtime
        .install_from_directory(direct.app_id_bytes.clone())
        .expect("install from directory");
    assert_eq!(carried, direct);

    assert!(
        receiver_runtime
            .directory_listings()
            .expect("directory")
            .into_iter()
            .find(|listing| listing.app_id == direct.app_id_bytes)
            .expect("still listed")
            .installed,
        "the carried app is now installed on this profile"
    );

    // Admitting the app is not enough: the host still has to serve its pages,
    // and for a carried app the store holds the only copy of them.
    let pair = receiver_runtime
        .app_pair_bytes(direct.app_id_bytes.clone())
        .expect("pair bytes for serving and persisting");
    let served = riot_core::apps::bundle::decode_app_bundle(&pair.bundle_bytes).expect("decodes");
    // The manifest half is what a host re-admits the app with after a relaunch;
    // from one verified read the two halves cannot disagree about the app.
    assert_eq!(
        riot_core::apps::index::verify_app_pair(&pair.manifest_bytes, &pair.bundle_bytes)
            .expect("the pair re-verifies")
            .to_vec(),
        direct.app_id_bytes,
    );
    assert_eq!(served.entry_point, carried.entry_point);
    assert!(
        served
            .resources
            .iter()
            .any(|resource| resource.path == served.entry_point),
        "the entry point is actually servable"
    );
}

#[test]
fn app_pair_bytes_refuses_an_app_this_profile_has_never_seen() {
    let profile = open_local_profile().expect("profile");
    assert!(matches!(
        profile.app_runtime().app_pair_bytes(vec![0x9a; 32]),
        Err(MobileError::AppRejected)
    ));
}

#[test]
fn install_from_directory_refuses_an_app_this_profile_has_never_seen() {
    let profile = open_local_profile().expect("profile");
    assert!(matches!(
        profile.app_runtime().install_from_directory(vec![0x9a; 32]),
        Err(MobileError::AppRejected)
    ));
}

#[test]
fn install_from_directory_refuses_an_app_id_that_is_not_32_bytes() {
    let profile = open_local_profile().expect("profile");
    assert!(matches!(
        profile.app_runtime().install_from_directory(vec![0x01; 31]),
        Err(MobileError::InvalidInput)
    ));
}

#[test]
fn install_from_directory_refuses_a_bundle_that_is_still_arriving() {
    use riot_core::apps::index::{app_index_manifest_path, verify_app_pair};

    let receiver = open_local_profile().expect("receiver");
    receiver
        .create_public_space("Partial arrival".into())
        .unwrap();
    let (manifest_bytes, bundle_bytes) = manifest_and_bundle();
    let app_id = verify_app_pair(&manifest_bytes, &bundle_bytes).expect("pair");

    // Only the manifest lands; the bundle is still in flight.
    let identity = receiver.identity().expect("identity");
    let author = riot_core::willow::generate_communal_author_for_namespace(
        unhex(&identity.namespace_id).try_into().unwrap(),
    )
    .expect("author");
    let manifest_only = signed_bundle_at(
        &author,
        app_index_manifest_path(&app_id).expect("path"),
        &manifest_bytes,
    );
    let preview = receiver
        .inspect_bytes(manifest_only, "nearby".into())
        .expect("inspect");
    preview
        .create_plan(Vec::new())
        .expect("plan")
        .accept()
        .expect("accept");

    // A half-arrived app is not installable, and the directory does not claim
    // otherwise.
    let runtime = receiver.app_runtime();
    assert!(matches!(
        runtime.install_from_directory(app_id.to_vec()),
        Err(MobileError::AppRejected)
    ));
    assert!(
        !runtime
            .directory_listings()
            .expect("directory")
            .iter()
            .any(|listing| listing.app_id == app_id.to_vec() && listing.bundle_present),
        "a manifest without its bundle is never listed as present"
    );
}

// ---------------------------------------------------------------------------
// Opening a BUILT-IN app. Built-ins ship as bytes inside the binary: they are
// never written to the store and never arrive over sync. The directory listed
// them anyway (it merges the starter catalog into its scan of the store), but
// install and page-serving resolved bytes from the store alone — so every
// built-in was permanently un-openable, and the UI told the user to go find a
// peer carrying an app that was already compiled into their binary. These pin
// that a profile which has never met anyone can open what it already has.
// ---------------------------------------------------------------------------

/// Every built-in, derived from the committed catalog — never a hard-coded id,
/// which goes stale the moment anyone repacks an app.
fn built_in_apps() -> Vec<riot_core::apps::directory::IndexedApp> {
    let built_ins =
        riot_core::apps::starter::verify_starter_catalog(riot_core::apps::starter::STARTER_CATALOG);
    assert!(
        !built_ins.is_empty(),
        "the starter catalog must ship at least one verifiable built-in"
    );
    built_ins
}

fn built_in_checklist() -> riot_core::apps::directory::IndexedApp {
    built_in_apps()
        .into_iter()
        .find(|app| app.manifest.name == "Checklist")
        .expect("the starter catalog ships the Checklist app")
}

#[test]
fn a_built_in_app_installs_on_a_profile_that_has_never_synced_with_anyone() {
    // No space, no peer, no sync: nothing has ever entered this profile's
    // store. The checklist's bytes are in the binary, and that must be enough.
    let profile = open_local_profile().expect("profile");
    let runtime = profile.app_runtime();
    let checklist = built_in_checklist();
    let app_id = checklist.app_id.to_vec();

    let listing = runtime
        .directory_listings()
        .expect("directory")
        .into_iter()
        .find(|listing| listing.app_id == app_id)
        .expect("the built-in checklist is listed");
    assert!(listing.built_in);
    assert!(listing.bundle_present);
    assert!(!listing.installed, "listed, but not yet opened");
    assert!(
        listing.carrier_subspace_id.is_none(),
        "no carrier holds it — its bytes are compiled in"
    );

    // The regression: this returned AppRejected, which the UI renders as
    // "Checklist isn't all here yet. Sync with the group carrying it."
    let installed = runtime
        .install_from_directory(app_id.clone())
        .expect("a built-in installs with no peer in sight");
    assert_eq!(installed.app_id_bytes, app_id);
    assert_eq!(installed.name, checklist.manifest.name);
    assert_eq!(installed.entry_point, checklist.manifest.entry_point);

    assert!(
        runtime
            .directory_listings()
            .expect("directory")
            .into_iter()
            .find(|listing| listing.app_id == app_id)
            .expect("still listed")
            .installed,
        "the built-in is now installed on this profile"
    );
}

#[test]
fn every_built_in_in_the_catalog_installs_and_serves_its_pages_unsynced() {
    let profile = open_local_profile().expect("profile");
    let runtime = profile.app_runtime();

    for built_in in built_in_apps() {
        let app_id = built_in.app_id.to_vec();

        // Page serving is resolved before install, exactly as a host that has
        // to render the app's entry point does.
        let pair = runtime
            .app_pair_bytes(app_id.clone())
            .unwrap_or_else(|error| {
                panic!(
                    "built-in {} must serve its pages unsynced, got {error:?}",
                    built_in.manifest.name
                )
            });
        assert_eq!(
            riot_core::apps::index::verify_app_pair(&pair.manifest_bytes, &pair.bundle_bytes)
                .expect("the built-in pair re-verifies")
                .to_vec(),
            app_id,
            "the served bytes are the ones the id was derived from"
        );
        let served =
            riot_core::apps::bundle::decode_app_bundle(&pair.bundle_bytes).expect("bundle");
        assert!(
            served
                .resources
                .iter()
                .any(|resource| resource.path == served.entry_point),
            "the entry point is actually servable"
        );

        let installed = runtime
            .install_from_directory(app_id.clone())
            .unwrap_or_else(|error| {
                panic!(
                    "built-in {} must install unsynced, got {error:?}",
                    built_in.manifest.name
                )
            });
        assert_eq!(installed.app_id_bytes, app_id);

        // And once installed, serving still resolves to the same bytes.
        let after = runtime
            .app_pair_bytes(app_id.clone())
            .expect("still serves");
        assert_eq!(after.manifest_bytes, pair.manifest_bytes);
        assert_eq!(after.bundle_bytes, pair.bundle_bytes);
    }
}

#[test]
fn an_app_that_is_neither_built_in_nor_carried_is_still_refused() {
    // The "isn't all here yet" copy stays honest for an app this profile
    // genuinely cannot resolve: not in the binary, not in the store.
    let profile = open_local_profile().expect("profile");
    let runtime = profile.app_runtime();
    let (manifest_bytes, bundle_bytes) = manifest_and_bundle();
    let stranger = riot_core::apps::index::verify_app_pair(&manifest_bytes, &bundle_bytes)
        .expect("pair")
        .to_vec();
    assert!(
        !built_in_apps()
            .iter()
            .any(|built_in| built_in.app_id.to_vec() == stranger),
        "fixture app must not collide with a built-in"
    );

    assert!(matches!(
        runtime.install_from_directory(stranger.clone()),
        Err(MobileError::AppRejected)
    ));
    assert!(matches!(
        runtime.app_pair_bytes(stranger),
        Err(MobileError::AppRejected)
    ));
}

// ===========================================================================
// Unit 0C — Runtime containment & invalidation (SECURITY-CRITICAL)
//
// The signed-JS-apps runtime hands a WebView a data bridge. Before Unit 0C the
// bridge called `app_data_put/get/list` directly on the profile, and those
// calls trust-gated NOTHING: an app whose approval was revoked, whose namespace
// was swapped out from under it, or that had been explicitly torn down could
// still read and write. Containment lived entirely in the native host "not
// calling" — a policy, not a mechanism.
//
// `AppExecutionSession` is the mechanism. A session is opened for exactly one
// app id, captures the approval GENERATION and the NAMESPACE at open, and
// revalidates BOTH — plus a live trust check and a destruction flag — on every
// single read and commit. Four independent ways an app can be invalidated must
// each make the very next op fail *before* it touches data:
//
//   1. revoke                 — trust withdrawn
//   2. namespace replacement  — the profile's namespace is swapped
//   3. explicit destruction   — the host tears the session down
//   4. stale approval-generation — the app is re-approved (generation bumped),
//                                   so an op carrying the OLD generation fails
//                                   even though trust is TRUE and namespace
//                                   matches. This is the airtight no-bypass
//                                   proof: a check that only asked "is it
//                                   trusted now?" would wrongly succeed here.
// ===========================================================================

/// A fresh organizer-shaped profile with one installed, trusted app. Mirrors
/// `trust_lifecycle_is_lww_per_app`: a locally opened profile is the recognized
/// organizer of its own space, so `trust_app` grants real authority.
fn organizer_with_trusted_app() -> (Arc<MobileProfile>, Arc<riot_ffi::AppRuntimeSession>, String) {
    let profile = open_local_profile().expect("profile");
    let runtime = profile.app_runtime();
    let (manifest_bytes, bundle_bytes) = manifest_and_bundle();
    let app = runtime
        .install_app(manifest_bytes, bundle_bytes)
        .expect("install");
    runtime.trust_app(app.app_id.clone()).expect("trust");
    (profile, runtime, app.app_id)
}

#[test]
fn app_execution_reads_and_commits_only_while_valid() {
    // Positive control: the guard is not a blanket refusal. A live session with
    // current trust, matching namespace, current generation, not destroyed
    // reads, writes, and lists normally.
    let (profile, _runtime, app_id) = organizer_with_trusted_app();
    let session = profile
        .open_app_execution(app_id)
        .expect("open a live execution session for a trusted app");

    session
        .app_data_put("items/a".to_string(), b"{\"done\":false}".to_vec())
        .expect("commit while valid");
    assert_eq!(
        session.app_data_get("items/a".to_string()).expect("read"),
        Some(b"{\"done\":false}".to_vec())
    );
    let listed = session.app_data_list("items".to_string()).expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].key, "items/a");
}

#[test]
fn open_app_execution_refuses_an_untrusted_app_at_the_gate() {
    // The launch gate is now in Rust, not a native-host policy: a session cannot
    // even be opened for an app that is not currently trusted.
    let profile = open_local_profile().expect("profile");
    let runtime = profile.app_runtime();
    let (manifest_bytes, bundle_bytes) = manifest_and_bundle();
    let app = runtime
        .install_app(manifest_bytes, bundle_bytes)
        .expect("install");
    assert!(
        matches!(
            profile.open_app_execution(app.app_id),
            Err(MobileError::AppRejected)
        ),
        "opening an execution session for an untrusted app is refused before any data access"
    );
}

#[test]
fn app_execution_is_valid_distinguishes_invalidation_from_a_per_op_rejection() {
    // §4.7 disambiguator. A malformed key and a revoked session both surface as
    // `AppRejected` from a data call. `is_valid()` is how the host tells them
    // apart: a per-op rejection leaves the session valid (stay open, inline
    // error); an invalidation makes it invalid (close to "Return to Tools").
    let (profile, runtime, app_id) = organizer_with_trusted_app();
    let session = profile.open_app_execution(app_id.clone()).expect("open");

    // A traversal-shaped key is rejected, but the SESSION is still valid.
    assert!(matches!(
        session.app_data_put("../escape".to_string(), b"x".to_vec()),
        Err(MobileError::AppRejected)
    ));
    assert!(
        session.is_valid(),
        "a malformed-key rejection must leave the session valid — it is not an invalidation"
    );

    // Revoke: now the SAME AppRejected means the session is gone.
    runtime.untrust_app(app_id).expect("revoke");
    assert!(matches!(
        session.app_data_get("k".to_string()),
        Err(MobileError::AppRejected)
    ));
    assert!(
        !session.is_valid(),
        "a revoked session must report invalid so the host closes to Return to Tools"
    );

    // Destruction also reads invalid.
    let (profile2, _r2, app2) = organizer_with_trusted_app();
    let s2 = profile2.open_app_execution(app2).expect("open");
    assert!(s2.is_valid());
    s2.invalidate();
    assert!(!s2.is_valid(), "a destroyed session reports invalid");
}

#[test]
fn revoke_fails_the_next_app_execution_read_and_commit() {
    let (profile, runtime, app_id) = organizer_with_trusted_app();
    let session = profile.open_app_execution(app_id.clone()).expect("open");
    session
        .app_data_put("k1".to_string(), b"v1".to_vec())
        .expect("commit while trusted");

    // Trust is revoked out from under the running app.
    runtime.untrust_app(app_id.clone()).expect("revoke");

    // The SAME session must now fail both read and commit — before touching data.
    assert!(
        matches!(
            session.app_data_get("k1".to_string()),
            Err(MobileError::AppRejected)
        ),
        "a revoked app cannot read even a key it wrote while trusted"
    );
    assert!(
        matches!(
            session.app_data_put("k2".to_string(), b"v2".to_vec()),
            Err(MobileError::AppRejected)
        ),
        "a revoked app cannot commit"
    );

    // Prove the blocked commit never touched data: re-approve, open a fresh
    // session, and confirm k2 was never written.
    runtime.trust_app(app_id.clone()).expect("re-approve");
    let fresh = profile.open_app_execution(app_id).expect("reopen");
    assert_eq!(
        fresh.app_data_get("k2".to_string()).expect("read"),
        None,
        "the commit blocked by revocation must not have reached the store"
    );
}

#[test]
fn namespace_replacement_fails_stale_app_execution_access() {
    let (profile, _runtime, app_id) = organizer_with_trusted_app();
    let session = profile.open_app_execution(app_id).expect("open");
    session
        .app_data_put("k1".to_string(), b"v1".to_vec())
        .expect("commit in the original namespace");

    // Another profile's space; joining it regenerates our author into a
    // different namespace (`join_public_space`), invalidating the session that
    // was bound to the original namespace.
    let other = open_local_profile().expect("other profile");
    let other_space = other
        .create_public_space("Elsewhere".into())
        .expect("space");
    profile
        .join_public_space(other_space, Vec::new())
        .expect("join");

    assert!(
        matches!(
            session.app_data_get("k1".to_string()),
            Err(MobileError::AppRejected)
        ),
        "a session bound to the replaced namespace cannot read"
    );
    assert!(
        matches!(
            session.app_data_put("k2".to_string(), b"v2".to_vec()),
            Err(MobileError::AppRejected)
        ),
        "a session bound to the replaced namespace cannot commit"
    );
}

#[test]
fn explicit_destruction_fails_subsequent_app_execution_ops() {
    let (profile, _runtime, app_id) = organizer_with_trusted_app();
    let session = profile.open_app_execution(app_id).expect("open");
    session
        .app_data_put("k1".to_string(), b"v1".to_vec())
        .expect("commit while live");

    session.invalidate();

    assert!(
        matches!(
            session.app_data_get("k1".to_string()),
            Err(MobileError::AppRejected)
        ),
        "a destroyed session cannot read"
    );
    assert!(
        matches!(
            session.app_data_put("k2".to_string(), b"v2".to_vec()),
            Err(MobileError::AppRejected)
        ),
        "a destroyed session cannot commit"
    );
}

#[test]
fn stale_approval_generation_fails_an_op_carrying_the_old_generation() {
    // The airtight no-bypass proof. After re-approval the app is trusted again
    // and the namespace is unchanged and the session is not destroyed — the ONLY
    // thing wrong is the stale generation. A guard that merely re-asked "trusted
    // now?" would wrongly let this through. The generation check must fail it.
    let (profile, runtime, app_id) = organizer_with_trusted_app();
    let session = profile
        .open_app_execution(app_id.clone())
        .expect("open at gen N");
    session
        .app_data_put("k1".to_string(), b"v1".to_vec())
        .expect("commit at gen N");

    // Re-approval: withdraw then grant again. Trust ends TRUE, but the approval
    // generation has advanced past what the session captured.
    runtime.untrust_app(app_id.clone()).expect("revoke");
    runtime.trust_app(app_id.clone()).expect("re-approve");

    assert!(
        matches!(
            session.app_data_get("k1".to_string()),
            Err(MobileError::AppRejected)
        ),
        "an op carrying a stale approval generation must fail even though trust is TRUE"
    );
    assert!(
        matches!(
            session.app_data_put("k2".to_string(), b"v2".to_vec()),
            Err(MobileError::AppRejected)
        ),
        "a stale-generation commit must fail even though trust is TRUE"
    );

    // A session opened at the NEW generation works and still sees the earlier
    // write — proving the block is generation-specific, not a blanket refusal.
    let fresh = profile.open_app_execution(app_id).expect("open at gen N+1");
    assert_eq!(
        fresh.app_data_get("k1".to_string()).expect("read"),
        Some(b"v1".to_vec()),
        "a current-generation session reads normally"
    );
    fresh
        .app_data_put("k2".to_string(), b"v2".to_vec())
        .expect("a current-generation session commits normally");
}

// --- WU-001: 32-app count cap + 3 MiB aggregate byte quota ---

/// A distinct, tiny, valid manifest+bundle pair. A unique name + unique resource
/// bytes give a unique app id. Each pair is a few hundred bytes — far under the
/// 3 MiB / 32 ≈ 96 KiB per-pair budget, so 32 of them never trip the byte quota
/// before the count cap.
fn distinct_small_pair(index: usize) -> (Vec<u8>, Vec<u8>) {
    use riot_core::apps::bundle::{encode_app_bundle, AppBundle, AppResource};
    use riot_core::apps::manifest::{encode_manifest, AppManifest};
    use riot_core::willow::generate_communal_author;

    let author = generate_communal_author().expect("author");
    let bundle = AppBundle {
        entry_point: "index.html".to_string(),
        resources: vec![AppResource {
            path: "index.html".to_string(),
            content_type: "text/html".to_string(),
            bytes: format!("<html>app {index}</html>").into_bytes(),
        }],
    };
    let manifest = AppManifest {
        name: format!("App {index}"),
        description: "distinct".to_string(),
        version: "1.0.0".to_string(),
        author: author.identity(),
        permissions: vec!["own-app-data".to_string()],
        entry_point: "index.html".to_string(),
    };
    (
        encode_manifest(&manifest).expect("manifest"),
        encode_app_bundle(&bundle).expect("bundle"),
    )
}

/// A distinct, valid pair whose bundle is ~900 KiB (well under the 1 MiB
/// per-bundle `MAX_BUNDLE_TOTAL_BYTES`). Four of these sum past the 3 MiB
/// aggregate quota while the count stays far below 32, so the refusal is on
/// bytes, not count. (Three ~1 MiB bundles cannot reliably exceed 3 MiB because
/// each single bundle is capped at 1 MiB, hence four smaller ones.)
fn big_pair(index: usize) -> (Vec<u8>, Vec<u8>) {
    use riot_core::apps::bundle::{encode_app_bundle, AppBundle, AppResource};
    use riot_core::apps::manifest::{encode_manifest, AppManifest};
    use riot_core::willow::generate_communal_author;

    let author = generate_communal_author().expect("author");
    let mut bytes = vec![b'a'; 900_000];
    // Keep each bundle distinct so it gets a distinct app id.
    bytes.extend_from_slice(format!("<!-- {index} -->").as_bytes());
    let bundle = AppBundle {
        entry_point: "index.html".to_string(),
        resources: vec![AppResource {
            path: "index.html".to_string(),
            content_type: "text/html".to_string(),
            bytes,
        }],
    };
    let manifest = AppManifest {
        name: format!("Big {index}"),
        description: "large".to_string(),
        version: "1.0.0".to_string(),
        author: author.identity(),
        permissions: vec!["own-app-data".to_string()],
        entry_point: "index.html".to_string(),
    };
    (
        encode_manifest(&manifest).expect("manifest"),
        encode_app_bundle(&bundle).expect("bundle"),
    )
}

#[test]
fn install_count_cap_is_thirty_two_not_sixteen() {
    // Install 32 distinct valid pairs, then assert the 33rd is refused with the
    // count-specific error (SessionLimit), not the byte error.
    let profile = open_local_profile().expect("profile");
    let runtime = profile.app_runtime();
    for index in 0..32 {
        let (manifest, bundle) = distinct_small_pair(index);
        runtime
            .install_app(manifest, bundle)
            .unwrap_or_else(|error| panic!("app {index} refused within cap: {error:?}"));
    }
    let (manifest, bundle) = distinct_small_pair(32);
    let err = runtime
        .install_app(manifest, bundle)
        .expect_err("33rd refused");
    assert!(matches!(err, MobileError::SessionLimit));
}

#[test]
fn install_refuses_when_aggregate_pair_bytes_exceed_three_mib() {
    // Install large-but-valid pairs whose running total crosses 3 MiB before the
    // count cap; assert StoreFull (byte-specific), distinct from SessionLimit.
    let profile = open_local_profile().expect("profile");
    let runtime = profile.app_runtime();
    // Three ~900 KiB pairs (~2.7 MiB aggregate) still fit.
    for index in 0..3 {
        let (manifest, bundle) = big_pair(index);
        runtime
            .install_app(manifest, bundle)
            .unwrap_or_else(|error| panic!("big pair {index} refused early: {error:?}"));
    }
    // The fourth crosses 3 MiB aggregate (~3.6 MiB) with count still at 3 << 32.
    let (manifest, bundle) = big_pair(3);
    let err = runtime
        .install_app(manifest, bundle)
        .expect_err("aggregate over 3 MiB refused");
    assert!(matches!(err, MobileError::StoreFull));
}

#[test]
fn reinstalling_a_held_pair_is_idempotent_at_the_cap() {
    // Fill to 32, then reinstall a held ID: succeeds, count unchanged (a new ID
    // still refuses on count afterwards).
    let profile = open_local_profile().expect("profile");
    let runtime = profile.app_runtime();
    let first_pair = distinct_small_pair(0);
    runtime
        .install_app(first_pair.0.clone(), first_pair.1.clone())
        .expect("first install");
    for index in 1..32 {
        let (manifest, bundle) = distinct_small_pair(index);
        runtime
            .install_app(manifest, bundle)
            .unwrap_or_else(|error| panic!("app {index} refused within cap: {error:?}"));
    }
    // Reinstalling the already-held pair 0 is idempotent, not a limit failure.
    runtime
        .install_app(first_pair.0, first_pair.1)
        .expect("reinstalling a held pair at the cap");
    // A genuinely new 33rd distinct app still refuses on count.
    let (manifest, bundle) = distinct_small_pair(32);
    assert!(matches!(
        runtime.install_app(manifest, bundle),
        Err(MobileError::SessionLimit)
    ));
}

#[test]
fn a_restored_generation_one_profile_still_serves_a_held_built_in() {
    // open_profile_from_sealed_identity takes the restore path, which sets
    // generation = None (gen-1). It must still resolve/serve a held built-in
    // pair by exact ID via the dual-catalog resolver — a regression guard on the
    // restore path + generation field (uses only pub FFI, no private access).
    let original = open_local_profile().expect("profile");
    let wrapping_key = vec![0x5a; 32];
    let sealed = original
        .seal_identity(wrapping_key.clone())
        .expect("seal identity");
    let restored = open_profile_from_sealed_identity(wrapping_key, sealed).expect("restore");

    let built_in = built_in_apps().into_iter().next().expect("a built-in");
    let app_id = built_in.app_id.to_vec();

    let pair = restored
        .app_runtime()
        .app_pair_bytes(app_id.clone())
        .expect("restored gen-1 profile resolves a built-in by exact id");
    assert_eq!(
        riot_core::apps::index::verify_app_pair(&pair.manifest_bytes, &pair.bundle_bytes)
            .expect("the built-in pair re-verifies")
            .to_vec(),
        app_id,
        "the served bytes are the ones the id was derived from",
    );
}
