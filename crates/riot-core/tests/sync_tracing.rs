//! Sync tracing contract: when the reconciliation state machine rejects a
//! frame or a bundle, the precise `SyncError` variant reaches the tracing
//! subscriber — not just the coarse code that later collapses at the FFI
//! boundary. This file pins the "logs get detail" half of the diagnostics
//! design before the instrumentation is relied upon.
//!
//! These tests do NOT install the os_log subscriber; `#[traced_test]` installs
//! its own in-memory subscriber that captures every span and event, so the
//! assertions see exactly what an os_log / `log stream` consumer would see.

use riot_core::import::encode_bundle;
use riot_core::model::{AlertPayload, Certainty, Severity, Urgency};
use riot_core::sync::{ReconcileSession, SyncAction, SyncError, SyncFrame};
use riot_core::willow::{
    authorise_entry, build_alert_entry, encode_capability, encode_entry, generate_communal_author,
    generate_communal_author_for_namespace, EvidenceAuthor, SignedWillowEntry,
};
use tracing_test::traced_test;

/// Builds a signed alert entry at a fixed object id, reusing the exact fixture
/// shape from `core_sync.rs` so the reconciliation codec treats it as canonical.
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
        headline: format!("tracing fixture alert {object}"),
        description: "Sync tracing contract fixture.".into(),
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

/// Drives the named helper that extracts an outbound `SyncFrame` from a
/// `SyncAction::Send`, panicking on any other action.
fn sent(action: SyncAction) -> SyncFrame {
    match action {
        SyncAction::Send(frame) => frame,
        other => panic!("expected outbound frame, got {other:?}"),
    }
}

#[traced_test]
#[test]
fn a_namespace_mismatch_is_logged_with_its_precise_variant() {
    let alice = generate_communal_author().unwrap();
    let namespace_id = alice.identity().namespace_id;
    let alice_entry = signed(&alice, 11);

    let mut initiator = ReconcileSession::new(namespace_id, vec![alice_entry.clone()]).unwrap();
    let _ = initiator.begin().unwrap();

    // A frame carrying a foreign namespace must be refused. The refusal is the
    // only path that can reach `os_log` for a namespace mismatch, so its exact
    // variant must appear in the captured logs.
    let foreign = SyncFrame::Entries {
        namespace_id: [0xAA; 32],
        bundle_bytes: encode_bundle(std::slice::from_ref(&alice_entry)).unwrap(),
    };
    assert_eq!(
        initiator.receive(foreign),
        Err(SyncError::NamespaceMismatch),
        "a foreign-namespace frame must be refused"
    );

    assert!(
        logs_contain("NamespaceMismatch"),
        "the precise SyncError variant must reach the tracing subscriber"
    );
    assert!(
        logs_contain("sync.receive rejected frame from foreign namespace"),
        "the human-readable context must accompany the variant"
    );
}

#[traced_test]
#[test]
fn an_invalid_bundle_is_logged_with_its_precise_variant() {
    let alice = generate_communal_author().unwrap();
    let namespace_id = alice.identity().namespace_id;
    let alice_entry = signed(&alice, 11);
    let bob = generate_communal_author_for_namespace(namespace_id).unwrap();
    let bob_entry = signed(&bob, 22);

    let mut initiator = ReconcileSession::new(namespace_id, vec![alice_entry.clone()]).unwrap();
    let mut responder = ReconcileSession::new(namespace_id, vec![bob_entry]).unwrap();

    // Drive the exchange far enough that the initiator is awaiting the bob entry.
    let hello = sent(initiator.begin().unwrap());
    let responder_summary = sent(responder.receive(hello).unwrap());
    let _request = sent(initiator.receive(responder_summary).unwrap());

    // Offer the WRONG entry (alice's own, not the requested bob entry). This is
    // the `verify_received_bundle` received != expected branch, which must log
    // InvalidBundle before the error is returned.
    let mismatched_entries = SyncFrame::Entries {
        namespace_id,
        bundle_bytes: encode_bundle(&[alice_entry]).unwrap(),
    };
    assert_eq!(
        initiator.receive(mismatched_entries),
        Err(SyncError::InvalidBundle),
        "an unexpected entry set must not satisfy the pending request"
    );

    assert!(
        logs_contain("InvalidBundle"),
        "the InvalidBundle variant must be captured for diagnostics"
    );
}

#[traced_test]
#[test]
fn an_unexpected_frame_is_logged_with_its_precise_variant() {
    let alice = generate_communal_author().unwrap();
    let namespace_id = alice.identity().namespace_id;
    let entry = signed(&alice, 5);

    let mut receiver = ReconcileSession::new(namespace_id, vec![]).unwrap();
    let mut sender = ReconcileSession::new(namespace_id, vec![entry]).unwrap();

    // Complete a full one-sided exchange so the receiver has accepted an import
    // and the exchange is done.
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

    // After rejection the receiver is Complete; any further frame is unexpected.
    // Calling import_accepted on a Complete session also exercises the
    // UnexpectedFrame branch in a different method.
    assert_eq!(
        receiver.import_accepted(),
        Err(SyncError::UnexpectedFrame),
        "import_accepted after completion must refuse"
    );

    assert!(
        logs_contain("UnexpectedFrame"),
        "the UnexpectedFrame variant must be captured"
    );
}

#[traced_test]
#[test]
fn a_successful_exchange_emits_phase_transitions_at_debug_level() {
    let alice = generate_communal_author().unwrap();
    let namespace_id = alice.identity().namespace_id;
    let entry = signed(&alice, 7);

    let mut receiver = ReconcileSession::new(namespace_id, vec![]).unwrap();
    let mut sender = ReconcileSession::new(namespace_id, vec![entry]).unwrap();

    let hello = sent(receiver.begin().unwrap());
    let summary = sent(sender.receive(hello).unwrap());
    let _request = sent(receiver.receive(summary).unwrap());

    // The healthy exchange must leave a breadcrumb trail of phase transitions
    // so a successful sync is also legible in the log, not only the failures.
    assert!(
        logs_contain("AwaitingInitialRequestOrPeerSummary"),
        "the healthy Hello→summary transition must be logged"
    );
}
