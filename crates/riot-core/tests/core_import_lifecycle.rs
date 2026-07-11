//! Lifecycle evidence for explicit selection and durable plan terminals.

use riot_core::import::encode_bundle;
use riot_core::model::{AlertPayload, Certainty, Severity, Urgency};
use riot_core::session::{ImportContext, InspectOutcome, RiotSession, SessionError};
use riot_core::willow::{
    authorise_entry, build_alert_entry, encode_capability, encode_entry, EvidenceAuthor,
    SignedWillowEntry,
};

fn author() -> EvidenceAuthor {
    use willow25::entry::NamespaceSecret;
    let mut seed = *b"riot-life-namespace-secret-0000!";
    let ns = loop {
        let candidate = NamespaceSecret::from_bytes(&seed).corresponding_namespace_id();
        if candidate.is_communal() {
            break candidate;
        }
        seed[0] = seed[0].wrapping_add(1);
    };
    EvidenceAuthor::from_parts_for_tests(ns, b"riot-life-subspace-secret-00000!")
}

fn signed(author: &EvidenceAuthor, tag: u8) -> SignedWillowEntry {
    let payload = riot_core::model::encode_alert(&AlertPayload {
        object_id: *b"riot-obj-life001",
        revision_id: *b"riot-rev-life001",
        created_at: 1_000,
        valid_from: None,
        expires_at: 2_000,
        language: "en".into(),
        urgency: Urgency::Immediate,
        severity: Severity::Severe,
        certainty: Certainty::Observed,
        headline: format!("lifecycle alert {tag}"),
        description: "Lifecycle fixture.".into(),
        affected_area_claim: None,
        source_claims: vec!["fixture".into()],
        ai_assisted: false,
    })
    .unwrap();
    let entry = build_alert_entry(author, &[tag; 16], &[tag; 16], 100, &payload).unwrap();
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

fn ctx() -> ImportContext {
    ImportContext::new("lifecycle-route")
}

fn preview() -> (
    riot_core::session::EvidenceStore,
    riot_core::session::ImportPreview,
) {
    let session = RiotSession::open().unwrap();
    let store = session.create_store().unwrap();
    let bytes = encode_bundle(&[signed(&author(), 1)]).unwrap();
    let preview = match store.inspect(&bytes, ctx()).unwrap() {
        InspectOutcome::Preview(preview) => preview,
        InspectOutcome::Rejected(rejection) => panic!("unexpected rejection: {rejection:?}"),
    };
    (store, preview)
}

#[test]
fn core_import_lifecycle_preserves_plan_terminal_reasons() {
    let (store, preview) = preview();
    let superseded = preview.plan_all().unwrap();
    let current = preview.plan_all().unwrap();
    assert_eq!(store.generation().unwrap(), 0);
    assert_eq!(store.receipt_count().unwrap(), 0);
    assert_eq!(store.live_count().unwrap(), 0);
    assert_eq!(superseded.commit(), Err(SessionError::PlanSuperseded));

    current.close().unwrap();
    assert_eq!(current.commit(), Err(SessionError::PlanClosed));

    let committed = preview.plan_all().unwrap();
    committed.commit().unwrap();
    assert_eq!(committed.commit(), Err(SessionError::PlanConsumed));
}

#[test]
fn core_import_lifecycle_allows_64_session_wide_plan_issues_but_rejects_the_65th() {
    let (_store, preview) = preview();
    for _ in 0..64 {
        preview.plan_all().unwrap();
    }
    assert!(matches!(
        preview.plan_all(),
        Err(SessionError::SessionLimit)
    ));
}

#[test]
fn core_import_lifecycle_close_allows_replacement_without_store_mutation() {
    let (store, preview) = preview();
    let plan = preview.plan_all().unwrap();
    plan.close().unwrap();
    assert_eq!(store.generation().unwrap(), 0);
    assert_eq!(store.receipt_count().unwrap(), 0);
    assert_eq!(store.live_count().unwrap(), 0);

    let replacement = preview.plan_all().unwrap();
    assert_eq!(store.generation().unwrap(), 0);
    assert_eq!(store.receipt_count().unwrap(), 0);
    assert_eq!(store.live_count().unwrap(), 0);
    replacement.commit().unwrap();
    assert_eq!(store.generation().unwrap(), 1);
}

#[test]
fn core_import_lifecycle_plan_budget_is_session_wide_and_preserves_terminal_reasons() {
    let (store, preview) = preview();
    let closed = preview.plan_all().unwrap();
    closed.close().unwrap();
    let superseded = preview.plan_all().unwrap();
    let replacement = preview.plan_all().unwrap();
    assert_eq!(superseded.commit(), Err(SessionError::PlanSuperseded));
    replacement.close().unwrap();

    // Complete the session-wide issuance budget with terminal plans.
    for _ in 0..60 {
        preview.plan_all().unwrap();
    }
    let final_plan = preview.plan_all().unwrap();
    final_plan.close().unwrap();

    // A later preview may still be inspected, but it cannot issue a 65th
    // plan. The original handles keep their durable, distinct terminal
    // reason after all 64 plan tombstones have been retained.
    let bytes = encode_bundle(&[signed(&author(), 2)]).unwrap();
    let later_preview = match store.inspect(&bytes, ctx()).unwrap() {
        InspectOutcome::Preview(preview) => preview,
        InspectOutcome::Rejected(rejection) => panic!("unexpected rejection: {rejection:?}"),
    };
    assert!(matches!(
        later_preview.plan_all(),
        Err(SessionError::SessionLimit)
    ));

    assert_eq!(closed.commit(), Err(SessionError::PlanClosed));
    assert_eq!(superseded.commit(), Err(SessionError::PlanSuperseded));
}
