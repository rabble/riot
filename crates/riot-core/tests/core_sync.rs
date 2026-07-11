//! Conference sync contract: canonical, bounded frames request only facts the
//! receiver does not already retain. Transport and import admission stay out
//! of this codec layer.

use riot_core::import::encode_bundle;
use riot_core::model::{AlertPayload, Certainty, Severity, Urgency};
use riot_core::session::{ImportContext, RiotSession};
use riot_core::sync::{
    decode_frame, encode_frame, missing_entry_ids, ReconcileSession, SyncAction, SyncError,
    SyncFrame, MAX_SYNC_FRAME_BYTES, MAX_SYNC_IDS,
};
use riot_core::willow::{
    authorise_entry, build_alert_entry, encode_capability, encode_entry, generate_communal_author,
    generate_communal_author_for_namespace, EvidenceAuthor, SignedWillowEntry,
};

const NAMESPACE: [u8; 32] = [0x42; 32];

fn id(value: u8) -> [u8; 32] {
    [value; 32]
}

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
        headline: format!("conference sync alert {object}"),
        description: "Bounded incremental sync fixture.".into(),
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

#[test]
fn summary_round_trips_canonically_and_requests_only_missing_ids() {
    let summary = SyncFrame::Summary {
        namespace_id: NAMESPACE,
        entry_ids: vec![id(1), id(2), id(3)],
    };

    let first = encode_frame(&summary).expect("bounded summary encodes");
    let decoded = decode_frame(&first).expect("canonical summary decodes");
    let second = encode_frame(&decoded).expect("decoded summary re-encodes");

    assert_eq!(decoded, summary);
    assert_eq!(second, first, "wire encoding must be deterministic");
    assert_eq!(
        missing_entry_ids(&[id(1), id(3)], &[id(1), id(2), id(3)]).unwrap(),
        vec![id(2)],
        "incremental reconciliation must not request overlapping facts"
    );
    assert!(
        missing_entry_ids(&[id(1), id(2), id(3)], &[id(1), id(2), id(3)])
            .unwrap()
            .is_empty()
    );
}

#[test]
fn duplicate_or_over_cap_summaries_are_rejected_without_allocating_a_request() {
    assert_eq!(
        missing_entry_ids(&[], &[id(1), id(1)]),
        Err(SyncError::DuplicateEntryId)
    );

    let too_many = vec![id(7); MAX_SYNC_IDS + 1];
    assert_eq!(
        missing_entry_ids(&[], &too_many),
        Err(SyncError::TooManyEntryIds)
    );
    assert_eq!(
        encode_frame(&SyncFrame::Summary {
            namespace_id: NAMESPACE,
            entry_ids: too_many,
        }),
        Err(SyncError::TooManyEntryIds)
    );
}

#[test]
fn malformed_noncanonical_and_trailing_frames_are_rejected() {
    assert_eq!(decode_frame(&[]), Err(SyncError::MalformedFrame));

    let canonical = encode_frame(&SyncFrame::Complete {
        namespace_id: NAMESPACE,
    })
    .unwrap();
    let mut trailing = canonical;
    trailing.push(0);
    assert_eq!(decode_frame(&trailing), Err(SyncError::NonCanonicalFrame));
}

#[test]
fn every_frame_kind_round_trips_and_wire_ceilings_apply_to_requests_and_bytes() {
    let frames = [
        SyncFrame::Hello {
            namespace_id: NAMESPACE,
        },
        SyncFrame::Summary {
            namespace_id: NAMESPACE,
            entry_ids: vec![id(1)],
        },
        SyncFrame::Request {
            namespace_id: NAMESPACE,
            entry_ids: vec![id(2)],
        },
        SyncFrame::Entries {
            namespace_id: NAMESPACE,
            bundle_bytes: vec![1, 2, 3],
        },
        SyncFrame::Complete {
            namespace_id: NAMESPACE,
        },
        SyncFrame::Reject {
            namespace_id: NAMESPACE,
            code: 7,
        },
    ];
    for frame in frames {
        let bytes = encode_frame(&frame).unwrap();
        assert_eq!(decode_frame(&bytes), Ok(frame));
    }

    let ids: Vec<_> = (0..=MAX_SYNC_IDS as u8).map(id).collect();
    assert_eq!(
        encode_frame(&SyncFrame::Request {
            namespace_id: NAMESPACE,
            entry_ids: ids,
        }),
        Err(SyncError::TooManyEntryIds)
    );
    assert_eq!(
        decode_frame(&vec![0; MAX_SYNC_FRAME_BYTES + 1]),
        Err(SyncError::FrameTooLarge)
    );
}

#[test]
fn two_peers_transfer_only_the_missing_entry_through_preview_first_import() {
    let alice = generate_communal_author().unwrap();
    let namespace_id = alice.identity().namespace_id;
    let bob = generate_communal_author_for_namespace(namespace_id).unwrap();
    let first = signed(&alice, 1);
    let second = signed(&bob, 2);
    let second_id = riot_core::willow::entry_id(&second.entry_bytes);

    let mut receiver = ReconcileSession::new(namespace_id, vec![first.clone()]).unwrap();
    let mut sender =
        ReconcileSession::new(namespace_id, vec![first.clone(), second.clone()]).unwrap();

    let hello = sent(receiver.begin().unwrap());
    let summary = sent(sender.receive(hello).unwrap());
    let request = sent(receiver.receive(summary).unwrap());
    assert_eq!(
        request,
        SyncFrame::Request {
            namespace_id,
            entry_ids: vec![second_id]
        }
    );
    let entries = sent(sender.receive(request).unwrap());
    let bundle_bytes = match receiver.receive(entries).unwrap() {
        SyncAction::ImportBundle(bytes) => bytes,
        other => panic!("expected import handoff, got {other:?}"),
    };

    let session = RiotSession::open().unwrap();
    let store = session.create_store().unwrap();
    store
        .inspect(
            &encode_bundle(&[first]).unwrap(),
            ImportContext::new("local"),
        )
        .unwrap()
        .expect_preview()
        .plan_all()
        .unwrap()
        .commit()
        .unwrap();
    store
        .inspect(&bundle_bytes, ImportContext::new("conference-sync"))
        .unwrap()
        .expect_preview()
        .plan_all()
        .unwrap()
        .commit()
        .unwrap();
    assert_eq!(store.live_count().unwrap(), 2);

    let receiver_summary = sent(receiver.import_accepted().unwrap());
    let complete = sent(sender.receive(receiver_summary).unwrap());
    assert_eq!(receiver.receive(complete), Ok(SyncAction::Complete));
}

#[test]
fn out_of_sequence_or_foreign_namespace_frames_do_not_advance_the_exchange() {
    let alice = generate_communal_author().unwrap();
    let namespace_id = alice.identity().namespace_id;
    let entry = signed(&alice, 1);
    let mut peer = ReconcileSession::new(namespace_id, vec![entry]).unwrap();
    peer.begin().unwrap();

    assert_eq!(
        peer.receive(SyncFrame::Complete { namespace_id }),
        Err(SyncError::UnexpectedFrame)
    );
    assert_eq!(
        peer.receive(SyncFrame::Summary {
            namespace_id: [0x99; 32],
            entry_ids: vec![]
        }),
        Err(SyncError::NamespaceMismatch)
    );

    // Both failures leave the peer waiting for the legitimate summary.
    let peer_summary = peer
        .receive(SyncFrame::Summary {
            namespace_id,
            entry_ids: vec![],
        })
        .unwrap();
    assert!(matches!(
        peer_summary,
        SyncAction::Send(SyncFrame::Summary { .. })
    ));
}

#[test]
fn divergent_peers_reconcile_both_directions_before_completing() {
    let alice = generate_communal_author().unwrap();
    let namespace_id = alice.identity().namespace_id;
    let bob = generate_communal_author_for_namespace(namespace_id).unwrap();
    let alice_entry = signed(&alice, 11);
    let bob_entry = signed(&bob, 22);
    let alice_id = riot_core::willow::entry_id(&alice_entry.entry_bytes);
    let bob_id = riot_core::willow::entry_id(&bob_entry.entry_bytes);

    let mut initiator = ReconcileSession::new(namespace_id, vec![alice_entry.clone()]).unwrap();
    let mut responder = ReconcileSession::new(namespace_id, vec![bob_entry]).unwrap();

    let hello = sent(initiator.begin().unwrap());
    let responder_summary = sent(responder.receive(hello).unwrap());
    let replayed_summary = responder_summary.clone();
    let request_bob = sent(initiator.receive(responder_summary).unwrap());
    assert!(matches!(
        &request_bob,
        SyncFrame::Request { entry_ids, .. } if entry_ids == &vec![bob_id]
    ));
    assert_eq!(
        initiator.receive(replayed_summary),
        Err(SyncError::UnexpectedFrame),
        "a replay must not advance the pending request"
    );
    assert_eq!(
        initiator.receive(SyncFrame::Entries {
            namespace_id: [0xAA; 32],
            bundle_bytes: encode_bundle(std::slice::from_ref(&alice_entry)).unwrap(),
        }),
        Err(SyncError::NamespaceMismatch)
    );
    assert_eq!(
        initiator.receive(SyncFrame::Entries {
            namespace_id,
            bundle_bytes: encode_bundle(&[alice_entry]).unwrap(),
        }),
        Err(SyncError::InvalidBundle),
        "an unexpected fact must not satisfy the pending request"
    );
    let bob_entries = sent(responder.receive(request_bob).unwrap());
    assert!(matches!(
        initiator.receive(bob_entries),
        Ok(SyncAction::ImportBundle(_))
    ));

    let initiator_summary = sent(initiator.import_accepted().unwrap());
    let request_alice = sent(responder.receive(initiator_summary).unwrap());
    assert!(matches!(
        &request_alice,
        SyncFrame::Request { entry_ids, .. } if entry_ids == &vec![alice_id]
    ));
    let alice_entries = sent(initiator.receive(request_alice).unwrap());
    assert!(matches!(
        responder.receive(alice_entries),
        Ok(SyncAction::ImportBundle(_))
    ));
    let complete = sent(responder.import_accepted().unwrap());
    assert_eq!(initiator.receive(complete), Ok(SyncAction::Complete));
}

#[test]
fn rejected_import_sends_reject_and_never_retains_pending_entries() {
    let sender_author = generate_communal_author().unwrap();
    let namespace_id = sender_author.identity().namespace_id;
    let entry = signed(&sender_author, 31);
    let mut receiver = ReconcileSession::new(namespace_id, vec![]).unwrap();
    let mut sender = ReconcileSession::new(namespace_id, vec![entry]).unwrap();

    let hello = sent(receiver.begin().unwrap());
    let summary = sent(sender.receive(hello).unwrap());
    let request = sent(receiver.receive(summary).unwrap());
    let entries = sent(sender.receive(request).unwrap());
    assert!(matches!(
        receiver.receive(entries),
        Ok(SyncAction::ImportBundle(_))
    ));

    let reject = sent(receiver.import_rejected(9).unwrap());
    assert_eq!(sender.receive(reject), Ok(SyncAction::Rejected(9)));
    assert_eq!(receiver.import_accepted(), Err(SyncError::UnexpectedFrame));
}

#[test]
fn identical_summaries_complete_without_an_entries_transfer() {
    let mut initiator = ReconcileSession::new(NAMESPACE, vec![]).unwrap();
    let mut responder = ReconcileSession::new(NAMESPACE, vec![]).unwrap();

    let hello = sent(initiator.begin().unwrap());
    let summary = sent(responder.receive(hello).unwrap());
    let initiator_summary = sent(initiator.receive(summary).unwrap());
    let complete = sent(responder.receive(initiator_summary).unwrap());
    assert!(matches!(complete, SyncFrame::Complete { .. }));
    assert_eq!(initiator.receive(complete), Ok(SyncAction::Complete));
}
