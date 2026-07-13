use riot_core::model::{AlertPayload, Certainty, Severity, Urgency};
use riot_core::sync::{ByteSyncOutcome, ByteSyncSession, SyncError};
use riot_core::willow::{
    authorise_entry, build_alert_entry, encode_capability, encode_entry, generate_communal_author,
    generate_communal_author_for_namespace, EvidenceAuthor, SignedWillowEntry,
};

const EMPTY_NAMESPACE: [u8; 32] = [0x42; 32];

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
        headline: format!("byte sync alert {object}"),
        description: "FFI bridge fixture".into(),
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

fn take(session: &mut ByteSyncSession) -> Vec<u8> {
    session
        .take_outbound_frame()
        .expect("FrameReady always retains one outbound frame")
}

#[test]
fn empty_sessions_exchange_bytes_and_reach_both_complete_outcomes() {
    let mut initiator = ByteSyncSession::new(EMPTY_NAMESPACE, vec![]).unwrap();
    let mut responder = ByteSyncSession::new(EMPTY_NAMESPACE, vec![]).unwrap();
    assert!(!initiator.is_terminal());
    assert_eq!(initiator.take_outbound_frame(), None);

    assert_eq!(initiator.begin(), Ok(ByteSyncOutcome::FrameReady));
    assert_eq!(initiator.begin(), Err(SyncError::UnexpectedFrame));
    assert_eq!(
        initiator.receive_bytes(&[]),
        Err(SyncError::UnexpectedFrame),
        "a retained outbound frame must be consumed before any next action"
    );
    assert_eq!(initiator.import_accepted(), Err(SyncError::UnexpectedFrame));
    assert_eq!(
        initiator.import_rejected(1),
        Err(SyncError::UnexpectedFrame)
    );

    let hello = take(&mut initiator);
    assert_eq!(
        responder.receive_bytes(&hello),
        Ok(ByteSyncOutcome::FrameReady)
    );
    let responder_summary = take(&mut responder);
    assert_eq!(
        initiator.receive_bytes(&responder_summary),
        Ok(ByteSyncOutcome::FrameReady)
    );
    let initiator_summary = take(&mut initiator);
    assert_eq!(
        responder.receive_bytes(&initiator_summary),
        Ok(ByteSyncOutcome::FrameReady)
    );
    assert!(responder.is_terminal(), "outbound Complete is terminal");
    let complete = take(&mut responder);
    assert_eq!(
        initiator.receive_bytes(&complete),
        Ok(ByteSyncOutcome::Complete)
    );
    assert!(initiator.is_terminal());
    assert_eq!(initiator.begin(), Err(SyncError::UnexpectedFrame));
    assert_eq!(
        initiator.receive_bytes(&hello),
        Err(SyncError::UnexpectedFrame)
    );
}

#[test]
fn divergent_sessions_handoff_import_bytes_then_accept_both_halves() {
    let alice = generate_communal_author().unwrap();
    let namespace_id = alice.identity().namespace_id;
    let bob = generate_communal_author_for_namespace(namespace_id).unwrap();
    let mut initiator = ByteSyncSession::new(namespace_id, vec![signed(&alice, 1)]).unwrap();
    let mut responder = ByteSyncSession::new(namespace_id, vec![signed(&bob, 2)]).unwrap();

    assert_eq!(initiator.begin(), Ok(ByteSyncOutcome::FrameReady));
    assert_eq!(
        responder.receive_bytes(&take(&mut initiator)),
        Ok(ByteSyncOutcome::FrameReady)
    );
    assert_eq!(
        initiator.receive_bytes(&take(&mut responder)),
        Ok(ByteSyncOutcome::FrameReady)
    );
    assert_eq!(
        responder.receive_bytes(&take(&mut initiator)),
        Ok(ByteSyncOutcome::FrameReady)
    );
    let bob_bundle = take(&mut responder);
    assert!(matches!(
        initiator.receive_bytes(&bob_bundle),
        Ok(ByteSyncOutcome::ImportBundle(bytes)) if !bytes.is_empty()
    ));

    assert_eq!(initiator.import_accepted(), Ok(ByteSyncOutcome::FrameReady));
    assert_eq!(
        responder.receive_bytes(&take(&mut initiator)),
        Ok(ByteSyncOutcome::FrameReady)
    );
    assert_eq!(
        initiator.receive_bytes(&take(&mut responder)),
        Ok(ByteSyncOutcome::FrameReady)
    );
    let alice_bundle = take(&mut initiator);
    assert!(matches!(
        responder.receive_bytes(&alice_bundle),
        Ok(ByteSyncOutcome::ImportBundle(bytes)) if !bytes.is_empty()
    ));
    assert_eq!(responder.import_accepted(), Ok(ByteSyncOutcome::FrameReady));
    assert!(responder.is_terminal());
}

#[test]
fn rejected_import_emits_terminal_reject_and_peer_observes_rejection() {
    let author = generate_communal_author().unwrap();
    let namespace_id = author.identity().namespace_id;
    let mut receiver = ByteSyncSession::new(namespace_id, vec![]).unwrap();
    let mut sender = ByteSyncSession::new(namespace_id, vec![signed(&author, 3)]).unwrap();

    assert_eq!(receiver.begin(), Ok(ByteSyncOutcome::FrameReady));
    assert_eq!(
        sender.receive_bytes(&take(&mut receiver)),
        Ok(ByteSyncOutcome::FrameReady)
    );
    assert_eq!(
        receiver.receive_bytes(&take(&mut sender)),
        Ok(ByteSyncOutcome::FrameReady)
    );
    assert_eq!(
        sender.receive_bytes(&take(&mut receiver)),
        Ok(ByteSyncOutcome::FrameReady)
    );
    assert!(matches!(
        receiver.receive_bytes(&take(&mut sender)),
        Ok(ByteSyncOutcome::ImportBundle(_))
    ));

    assert_eq!(
        receiver.import_rejected(23),
        Ok(ByteSyncOutcome::FrameReady)
    );
    assert!(receiver.is_terminal());
    let reject = take(&mut receiver);
    assert_eq!(
        sender.receive_bytes(&reject),
        Ok(ByteSyncOutcome::Rejected(23))
    );
    assert!(sender.is_terminal());
}

#[test]
fn construction_and_input_errors_are_forwarded_without_outbound_bytes() {
    let invalid = SignedWillowEntry {
        entry_bytes: vec![],
        capability_bytes: vec![],
        signature: [0; 64],
        payload_bytes: vec![],
    };
    assert!(matches!(
        ByteSyncSession::new(EMPTY_NAMESPACE, vec![invalid]),
        Err(SyncError::InvalidBundle)
    ));

    let mut session = ByteSyncSession::new(EMPTY_NAMESPACE, vec![]).unwrap();
    assert_eq!(session.receive_bytes(&[]), Err(SyncError::MalformedFrame));
    assert_eq!(session.take_outbound_frame(), None);
    assert_eq!(session.import_accepted(), Err(SyncError::UnexpectedFrame));
    assert_eq!(session.import_rejected(9), Err(SyncError::UnexpectedFrame));
}
