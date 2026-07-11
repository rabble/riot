//! FFI contract for the signed-JS-apps runtime surface: manifest install,
//! per-profile trust decisions, namespace-scoped app data put/get/list, and
//! the app-directory surface (listings, share, endorse) — end-to-end through
//! the UniFFI layer, in-process, same as `mobile_contract.rs`.

use riot_ffi::{
    open_local_profile, open_profile_from_sealed_identity, AlertCertainty, AlertDraftInput,
    AlertSeverity, AlertUrgency, MobileError, MobileProfile, MobileSyncSession, PublicSpace,
    SyncOutcomeKind,
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
            bytes: b"<html>checklist</html>".to_vec(),
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
    receiver.join_public_space(space).expect("join");
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
    receiver.join_public_space(space).expect("join");
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
    receiver.join_public_space(space).unwrap();
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
    receiver.join_public_space(space).expect("join");

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
    cancelled_receiver.join_public_space(space.clone()).unwrap();
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
    receiver.join_public_space(space).unwrap();
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
    receiver.join_public_space(space).unwrap();
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
    receiver.join_public_space(space).unwrap();
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
    receiver.join_public_space(space).unwrap();
    let preview = receiver
        .inspect_bytes(signed.bundle_bytes, "portable".into())
        .unwrap();
    let app_id = "11".repeat(32);
    assert!(matches!(
        receiver
            .app_runtime()
            .app_data_put(app_id, "items/a".into(), b"blocked".to_vec()),
        Err(MobileError::InvalidInput)
    ));
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
    reopened.join_public_space(space).expect("join same space");
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
fn app_display_name_is_short_stable_and_non_identifying() {
    let profile = open_local_profile().expect("profile");
    let runtime = profile.app_runtime();

    let name = runtime.app_display_name().expect("display name");
    assert!(name.starts_with("member-"));
    assert!(!name.is_empty());
    assert!(name.len() < 24);
    assert_ne!(name.len(), 64, "must not be full 64-hex key material");

    let suffix = name.strip_prefix("member-").expect("member- prefix");
    assert_eq!(suffix.len(), 8, "8 hex chars = first 4 subspace bytes");
    assert!(suffix
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));

    // Stable across calls within a profile.
    assert_eq!(runtime.app_display_name().expect("name again"), name);
}
