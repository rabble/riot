use minicbor::Encoder;
use riot_core::import::{encode_bundle, BUNDLE_CODEC_ID, BUNDLE_MAGIC, MAX_BUNDLE_BYTES};
use riot_core::model::{AlertPayload, Certainty, Severity, Urgency};
use riot_core::sync::{ReconcileSession, SyncAction, SyncError, SyncFrame, MAX_SYNC_IDS};
use riot_core::willow::{
    authorise_entry, build_alert_entry, encode_capability, encode_entry, entry_id,
    generate_communal_author, generate_communal_author_for_namespace, EvidenceAuthor,
    SignedWillowEntry,
};

const NAMESPACE: [u8; 32] = [0x42; 32];

fn signed(author: &EvidenceAuthor, object: u8) -> SignedWillowEntry {
    let payload = riot_core::model::encode_alert(&AlertPayload {
        object_id: [object; 16],
        revision_id: [object; 16],
        created_at: 1_000,
        valid_from: None,
        expires_at: 2_000,
        language: "en".into(),
        urgency: Urgency::Immediate,
        severity: Severity::Severe,
        certainty: Certainty::Observed,
        headline: format!("sync state alert {object}"),
        description: "State transition fixture".into(),
        affected_area_claim: None,
        source_claims: vec!["fixture".into()],
        ai_assisted: false,
    })
    .unwrap();
    let entry = build_alert_entry(author, &[object; 16], &[object; 16], 1_000, &payload).unwrap();
    let authorised = authorise_entry(author, entry).unwrap();
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    SignedWillowEntry {
        entry_bytes: encode_entry(authorised.entry()),
        capability_bytes: encode_capability(token.capability()),
        signature: signature.to_bytes(),
        payload_bytes: payload,
    }
}

fn sent(action: SyncAction) -> SyncFrame {
    match action {
        SyncAction::Send(frame) => frame,
        other => panic!("expected outbound frame, got {other:?}"),
    }
}

fn frame_raw(item: &SignedWillowEntry, signature: &[u8]) -> Vec<u8> {
    let mut bytes = BUNDLE_MAGIC.to_vec();
    let mut encoder = Encoder::new(&mut bytes);
    encoder.map(2).unwrap();
    encoder.u8(0).unwrap().str(BUNDLE_CODEC_ID).unwrap();
    encoder.u8(1).unwrap().array(1).unwrap();
    encoder.map(4).unwrap();
    encoder.u8(0).unwrap().bytes(&item.entry_bytes).unwrap();
    encoder
        .u8(1)
        .unwrap()
        .bytes(&item.capability_bytes)
        .unwrap();
    encoder.u8(2).unwrap().bytes(signature).unwrap();
    encoder.u8(3).unwrap().bytes(&item.payload_bytes).unwrap();
    bytes
}

#[test]
fn construction_rejects_count_entry_namespace_duplicate_and_bundle_failures() {
    let empty = SignedWillowEntry {
        entry_bytes: vec![],
        capability_bytes: vec![],
        signature: [0; 64],
        payload_bytes: vec![],
    };
    assert!(matches!(
        ReconcileSession::new(NAMESPACE, vec![empty.clone(); MAX_SYNC_IDS + 1]),
        Err(SyncError::TooManyEntryIds)
    ));
    assert!(matches!(
        ReconcileSession::new(NAMESPACE, vec![empty]),
        Err(SyncError::InvalidBundle)
    ));

    let author = generate_communal_author().unwrap();
    let namespace_id = author.identity().namespace_id;
    let entry = signed(&author, 1);
    assert!(matches!(
        ReconcileSession::new([0x99; 32], vec![entry.clone()]),
        Err(SyncError::NamespaceMismatch)
    ));
    assert!(matches!(
        ReconcileSession::new(namespace_id, vec![entry.clone(), entry.clone()]),
        Err(SyncError::DuplicateEntryId)
    ));

    let mut oversized = entry;
    oversized.payload_bytes = vec![0; MAX_BUNDLE_BYTES];
    assert!(matches!(
        ReconcileSession::new(namespace_id, vec![oversized]),
        Err(SyncError::InvalidBundle)
    ));
}

#[test]
fn empty_peers_cover_initial_summary_complete_and_terminal_transitions() {
    let mut initiator = ReconcileSession::new(NAMESPACE, vec![]).unwrap();
    let mut responder = ReconcileSession::new(NAMESPACE, vec![]).unwrap();

    let hello = sent(initiator.begin().unwrap());
    assert_eq!(initiator.begin(), Err(SyncError::UnexpectedFrame));
    let responder_summary = sent(responder.receive(hello).unwrap());
    let initiator_summary = sent(initiator.receive(responder_summary).unwrap());
    let complete = sent(responder.receive(initiator_summary).unwrap());
    assert_eq!(initiator.receive(complete), Ok(SyncAction::Complete));
    assert_eq!(
        initiator.receive(SyncFrame::Hello {
            namespace_id: NAMESPACE
        }),
        Err(SyncError::UnexpectedFrame)
    );
    assert_eq!(initiator.import_accepted(), Err(SyncError::UnexpectedFrame));
    assert_eq!(
        initiator.import_rejected(1),
        Err(SyncError::UnexpectedFrame)
    );
}

#[test]
fn reject_is_terminal_from_an_active_exchange() {
    let mut peer = ReconcileSession::new(NAMESPACE, vec![]).unwrap();
    assert_eq!(
        peer.receive(SyncFrame::Reject {
            namespace_id: NAMESPACE,
            code: 9,
        }),
        Ok(SyncAction::Rejected(9))
    );
    assert_eq!(peer.begin(), Err(SyncError::UnexpectedFrame));
}

#[test]
fn frames_from_another_namespace_and_out_of_order_frames_do_not_advance_state() {
    let mut peer = ReconcileSession::new(NAMESPACE, vec![]).unwrap();
    peer.begin().unwrap();
    assert_eq!(
        peer.receive(SyncFrame::Summary {
            namespace_id: [0x99; 32],
            entry_ids: vec![],
        }),
        Err(SyncError::NamespaceMismatch)
    );
    assert_eq!(
        peer.receive(SyncFrame::Complete {
            namespace_id: NAMESPACE,
        }),
        Err(SyncError::UnexpectedFrame)
    );
    assert!(matches!(
        peer.receive(SyncFrame::Summary {
            namespace_id: NAMESPACE,
            entry_ids: vec![],
        }),
        Ok(SyncAction::Send(SyncFrame::Summary { .. }))
    ));
}

#[test]
fn requests_reject_duplicate_out_of_order_and_unknown_entry_ids() {
    let author = generate_communal_author().unwrap();
    let namespace_id = author.identity().namespace_id;
    let known = signed(&author, 1);
    let known_id = entry_id(&known.entry_bytes);

    for requested in [
        vec![known_id, known_id],
        vec![[0xff; 32], known_id],
        vec![[0x55; 32]],
    ] {
        let mut responder = ReconcileSession::new(namespace_id, vec![known.clone()]).unwrap();
        responder
            .receive(SyncFrame::Hello { namespace_id })
            .unwrap();
        let expected = if requested[0] == [0x55; 32] {
            SyncError::UnknownEntryId
        } else if requested[0] == requested[1] {
            SyncError::DuplicateEntryId
        } else {
            SyncError::EntryIdsNotSorted
        };
        assert_eq!(
            responder.receive(SyncFrame::Request {
                namespace_id,
                entry_ids: requested,
            }),
            Err(expected)
        );
    }
}

#[test]
fn inventory_plus_remote_missing_ids_respects_the_global_limit() {
    let author = generate_communal_author().unwrap();
    let namespace_id = author.identity().namespace_id;
    let entries: Vec<_> = (0..MAX_SYNC_IDS)
        .map(|object| signed(&author, object as u8))
        .collect();
    let mut peer = ReconcileSession::new(namespace_id, entries).unwrap();
    peer.begin().unwrap();
    assert_eq!(
        peer.receive(SyncFrame::Summary {
            namespace_id,
            entry_ids: vec![[0xff; 32]],
        }),
        Err(SyncError::TooManyEntryIds)
    );
}

#[test]
fn invalid_summaries_fail_in_both_summary_processing_phases() {
    let mut initiator = ReconcileSession::new(NAMESPACE, vec![]).unwrap();
    initiator.begin().unwrap();
    assert_eq!(
        initiator.receive(SyncFrame::Summary {
            namespace_id: NAMESPACE,
            entry_ids: vec![[1; 32], [1; 32]],
        }),
        Err(SyncError::DuplicateEntryId)
    );

    let mut responder = ReconcileSession::new(NAMESPACE, vec![]).unwrap();
    responder
        .receive(SyncFrame::Hello {
            namespace_id: NAMESPACE,
        })
        .unwrap();
    assert_eq!(
        responder.receive(SyncFrame::Summary {
            namespace_id: NAMESPACE,
            entry_ids: vec![[2; 32], [1; 32]],
        }),
        Err(SyncError::EntryIdsNotSorted)
    );
}

fn receiver_waiting_for_entries(
    sender_entry: &SignedWillowEntry,
    namespace_id: [u8; 32],
) -> (ReconcileSession, SyncFrame) {
    let mut receiver = ReconcileSession::new(namespace_id, vec![]).unwrap();
    let mut sender = ReconcileSession::new(namespace_id, vec![sender_entry.clone()]).unwrap();
    let hello = sent(receiver.begin().unwrap());
    let summary = sent(sender.receive(hello).unwrap());
    let request = sent(receiver.receive(summary).unwrap());
    (receiver, request)
}

#[test]
fn received_entries_reject_malformed_invalid_foreign_and_unexpected_bundles() {
    let author = generate_communal_author().unwrap();
    let namespace_id = author.identity().namespace_id;
    let entry = signed(&author, 1);

    let (mut malformed_receiver, _) = receiver_waiting_for_entries(&entry, namespace_id);
    assert_eq!(
        malformed_receiver.receive(SyncFrame::Entries {
            namespace_id,
            bundle_bytes: vec![],
        }),
        Err(SyncError::InvalidBundle)
    );

    let mut bad_signature = entry.signature;
    bad_signature[0] ^= 1;
    let (mut invalid_receiver, _) = receiver_waiting_for_entries(&entry, namespace_id);
    assert_eq!(
        invalid_receiver.receive(SyncFrame::Entries {
            namespace_id,
            bundle_bytes: frame_raw(&entry, &bad_signature),
        }),
        Err(SyncError::InvalidBundle)
    );

    let foreign_author = generate_communal_author().unwrap();
    let foreign = signed(&foreign_author, 2);
    let foreign_id = entry_id(&foreign.entry_bytes);
    let mut foreign_receiver = ReconcileSession::new(namespace_id, vec![]).unwrap();
    foreign_receiver.begin().unwrap();
    assert!(matches!(
        foreign_receiver.receive(SyncFrame::Summary {
            namespace_id,
            entry_ids: vec![foreign_id],
        }),
        Ok(SyncAction::Send(SyncFrame::Request { .. }))
    ));
    assert_eq!(
        foreign_receiver.receive(SyncFrame::Entries {
            namespace_id,
            bundle_bytes: encode_bundle(&[foreign]).unwrap(),
        }),
        Err(SyncError::NamespaceMismatch)
    );

    let other = signed(&author, 3);
    let (mut unexpected_receiver, _) = receiver_waiting_for_entries(&entry, namespace_id);
    assert_eq!(
        unexpected_receiver.receive(SyncFrame::Entries {
            namespace_id,
            bundle_bytes: encode_bundle(&[other]).unwrap(),
        }),
        Err(SyncError::InvalidBundle)
    );
}

#[test]
fn accepted_and_rejected_imports_cover_both_reconciliation_halves() {
    let alice = generate_communal_author().unwrap();
    let namespace_id = alice.identity().namespace_id;
    let bob = generate_communal_author_for_namespace(namespace_id).unwrap();
    let alice_entry = signed(&alice, 11);
    let bob_entry = signed(&bob, 22);

    let mut initiator = ReconcileSession::new(namespace_id, vec![alice_entry]).unwrap();
    let mut responder = ReconcileSession::new(namespace_id, vec![bob_entry]).unwrap();
    let hello = sent(initiator.begin().unwrap());
    let responder_summary = sent(responder.receive(hello).unwrap());
    let request_bob = sent(initiator.receive(responder_summary).unwrap());
    let bob_entries = sent(responder.receive(request_bob).unwrap());
    assert!(matches!(
        initiator.receive(bob_entries),
        Ok(SyncAction::ImportBundle(_))
    ));
    let initiator_summary = sent(initiator.import_accepted().unwrap());
    let request_alice = sent(responder.receive(initiator_summary).unwrap());
    let alice_entries = sent(initiator.receive(request_alice).unwrap());
    assert!(matches!(
        responder.receive(alice_entries),
        Ok(SyncAction::ImportBundle(_))
    ));
    assert!(matches!(
        responder.import_accepted(),
        Ok(SyncAction::Send(SyncFrame::Complete { .. }))
    ));

    let entry = signed(&alice, 33);
    let (mut rejecting, request) = receiver_waiting_for_entries(&entry, namespace_id);
    let mut sender = ReconcileSession::new(namespace_id, vec![entry]).unwrap();
    sender.receive(SyncFrame::Hello { namespace_id }).unwrap();
    let entries = sent(sender.receive(request).unwrap());
    assert!(matches!(
        rejecting.receive(entries),
        Ok(SyncAction::ImportBundle(_))
    ));
    assert_eq!(
        rejecting.import_rejected(u8::MAX),
        Ok(SyncAction::Send(SyncFrame::Reject {
            namespace_id,
            code: u8::MAX,
        }))
    );
}
