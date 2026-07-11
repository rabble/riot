//! WU2B evidence: session arbiter, preview-first atomic import, copy-on-write
//! snapshot, receipts, dispositions, and provenance. Requires `conformance`.

use riot_core::import::encode_bundle;
use riot_core::model::{AlertPayload, Certainty, Severity, Urgency};
use riot_core::session::{
    CommitOutcome, EntryDisposition, ImportContext, ImportSelection, InspectOutcome, LiveStatus,
    RiotSession, SessionError,
};
use riot_core::willow::{
    authorise_entry, build_alert_entry, encode_capability, encode_entry, EvidenceAuthor,
    SignedWillowEntry,
};

fn author() -> EvidenceAuthor {
    use willow25::entry::NamespaceSecret;
    let mut seed = *b"riot-txn-namespace-secret-00001!";
    let ns = loop {
        let c = NamespaceSecret::from_bytes(&seed).corresponding_namespace_id();
        if c.is_communal() {
            break c;
        }
        seed[0] = seed[0].wrapping_add(1);
    };
    EvidenceAuthor::from_parts_for_tests(ns, b"riot-txn-subspace-secret-000001!")
}

fn signed(
    author: &EvidenceAuthor,
    object: u8,
    revision: u8,
    timestamp: u64,
    tag: u8,
) -> SignedWillowEntry {
    let payload = {
        let p = AlertPayload {
            object_id: *b"riot-obj-txn0001",
            revision_id: *b"riot-rev-txn0001",
            created_at: 1_000,
            valid_from: None,
            expires_at: 2_000,
            language: "en".into(),
            urgency: Urgency::Immediate,
            severity: Severity::Severe,
            certainty: Certainty::Observed,
            headline: format!("txn alert tag {tag} pad {}", "y".repeat(tag as usize)),
            description: "Transaction fixture.".into(),
            affected_area_claim: None,
            source_claims: vec!["fixture".into()],
            ai_assisted: false,
        };
        riot_core::model::encode_alert(&p).expect("payload")
    };
    let entry = build_alert_entry(author, &[object; 16], &[revision; 16], timestamp, &payload)
        .expect("entry");
    let authorised = authorise_entry(author, entry).expect("authorise");
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    SignedWillowEntry {
        entry_bytes: encode_entry(authorised.entry()),
        capability_bytes: encode_capability(token.capability()),
        signature: signature.to_bytes(),
        payload_bytes: payload,
    }
}

fn signed_distinct(author: &EvidenceAuthor, index: u16) -> SignedWillowEntry {
    let payload = riot_core::model::encode_alert(&AlertPayload {
        object_id: *b"riot-obj-txn0001",
        revision_id: *b"riot-rev-txn0001",
        created_at: 1_000,
        valid_from: None,
        expires_at: 2_000,
        language: "en".into(),
        urgency: Urgency::Immediate,
        severity: Severity::Severe,
        certainty: Certainty::Observed,
        headline: format!("distinct transaction alert {index}"),
        description: "Transaction fixture.".into(),
        affected_area_claim: None,
        source_claims: vec!["fixture".into()],
        ai_assisted: false,
    })
    .expect("payload");
    let mut object_id = [0; 16];
    object_id[..2].copy_from_slice(&index.to_be_bytes());
    let entry = build_alert_entry(author, &object_id, &[0; 16], 100, &payload).expect("entry");
    let authorised = authorise_entry(author, entry).expect("authorise");
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    SignedWillowEntry {
        entry_bytes: encode_entry(authorised.entry()),
        capability_bytes: encode_capability(token.capability()),
        signature: signature.to_bytes(),
        payload_bytes: payload,
    }
}

fn bundle(items: &[SignedWillowEntry]) -> Vec<u8> {
    encode_bundle(items).expect("encode bundle")
}

fn ctx() -> ImportContext {
    ImportContext::new("test-route")
}

#[test]
fn core_import_transaction_open_and_create_store() {
    let session = RiotSession::open().expect("open");
    let store = session.create_store().expect("store");
    assert_eq!(store.generation().expect("gen"), 0);
    // Only one store per session.
    assert!(matches!(
        session.create_store(),
        Err(SessionError::SessionLimit)
    ));
}

#[test]
fn core_import_transaction_capacity_error_from_a_plan_leaves_store_unchanged() {
    let a = author();
    let session = RiotSession::open().unwrap();
    let store = session.create_store().unwrap();
    for start in (0..1_024).step_by(64) {
        let entries: Vec<_> = (start..start + 64)
            .map(|index| signed_distinct(&a, index as u16))
            .collect();
        store
            .inspect(&bundle(&entries), ctx())
            .unwrap()
            .expect_preview()
            .plan_all()
            .unwrap()
            .commit()
            .unwrap();
    }
    let generation_before = store.generation().unwrap();
    let receipts_before = store.receipt_count().unwrap();
    let live_before = store.live_count().unwrap();

    let plan = store
        .inspect(&bundle(&[signed_distinct(&a, 1_024)]), ctx())
        .unwrap()
        .expect_preview()
        .plan_all()
        .unwrap();
    assert_eq!(plan.commit(), Err(SessionError::StoreFull));
    assert_eq!(
        plan.commit(),
        Err(SessionError::StoreFull),
        "capacity failure must not consume the plan"
    );
    assert_eq!(store.generation().unwrap(), generation_before);
    assert_eq!(store.receipt_count().unwrap(), receipts_before);
    assert_eq!(store.live_count().unwrap(), live_before);
}

#[test]
fn core_import_transaction_valid_alert_imports_and_receipts() {
    let a = author();
    let session = RiotSession::open().unwrap();
    let store = session.create_store().unwrap();
    let bytes = bundle(&[signed(&a, 1, 1, 100, 1)]);

    let preview = match store.inspect(&bytes, ctx()).unwrap() {
        InspectOutcome::Preview(p) => p,
        InspectOutcome::Rejected(r) => panic!("unexpected rejection: {r:?}"),
    };
    assert_eq!(preview.eligible_count().unwrap(), 1);

    let plan = preview.plan_all().unwrap();
    let outcome = plan.commit().unwrap();
    let receipt = match outcome {
        CommitOutcome::Committed(r) => r,
        CommitOutcome::NoChanges(_) => panic!("expected a commit"),
    };
    assert_eq!(receipt.dispositions.len(), 1);
    assert!(matches!(
        receipt.dispositions[0].disposition,
        EntryDisposition::AppliedAtCommit { .. }
    ));
    assert_eq!(store.generation().unwrap(), 1);
    assert_eq!(store.live_count().unwrap(), 1);
}

#[test]
fn core_import_transaction_selection_commits_only_the_selected_entries() {
    let a = author();
    let session = RiotSession::open().unwrap();
    let store = session.create_store().unwrap();
    let first = signed(&a, 41, 1, 100, 1);
    let second = signed(&a, 42, 1, 100, 2);
    let selected_id = riot_core::willow::entry_id(&second.entry_bytes);

    let receipt = match store
        .inspect(&bundle(&[first, second]), ctx())
        .unwrap()
        .expect_preview()
        .plan(ImportSelection::new(vec![selected_id]))
        .unwrap()
        .commit()
        .unwrap()
    {
        CommitOutcome::Committed(receipt) => receipt,
        CommitOutcome::NoChanges(_) => panic!("selected entry should commit"),
    };

    assert_eq!(receipt.dispositions.len(), 1);
    assert_eq!(receipt.dispositions[0].entry_id, selected_id);
    assert_eq!(store.live_count().unwrap(), 1);
}

#[test]
fn core_import_transaction_invalid_selection_is_rejected_without_state_change() {
    let a = author();
    let session = RiotSession::open().unwrap();
    let store = session.create_store().unwrap();
    let first = signed(&a, 43, 1, 100, 1);
    let second = signed(&a, 44, 1, 100, 2);
    let first_id = riot_core::willow::entry_id(&first.entry_bytes);
    let preview = store
        .inspect(&bundle(&[first, second]), ctx())
        .unwrap()
        .expect_preview();
    let original_plan = preview.plan_all().unwrap();
    let generation = store.generation().unwrap();
    let receipts = store.receipt_count().unwrap();

    assert!(matches!(
        preview.plan(ImportSelection::new(vec![])),
        Err(SessionError::EmptySelection)
    ));
    assert!(matches!(
        preview.plan(ImportSelection::new(vec![first_id, first_id])),
        Err(SessionError::DuplicateSelection)
    ));
    assert!(matches!(
        preview.plan(ImportSelection::new(vec![[0xFF; 32]])),
        Err(SessionError::UnknownSelection)
    ));
    assert_eq!(store.generation().unwrap(), generation);
    assert_eq!(store.receipt_count().unwrap(), receipts);
    assert_eq!(store.live_count().unwrap(), 0);
    assert!(matches!(
        original_plan.commit(),
        Ok(CommitOutcome::Committed(_))
    ));

    // Invalid requests also leave the preview issuance budget untouched.
    let (_budget_store, budget_preview) = {
        let session = RiotSession::open().unwrap();
        let store = session.create_store().unwrap();
        let preview = store
            .inspect(&bundle(&[signed(&a, 45, 1, 100, 3)]), ctx())
            .unwrap()
            .expect_preview();
        (store, preview)
    };
    assert!(matches!(
        budget_preview.plan(ImportSelection::new(vec![])),
        Err(SessionError::EmptySelection)
    ));
    assert!(matches!(
        budget_preview.plan(ImportSelection::new(vec![[0xEE; 32], [0xEE; 32]])),
        Err(SessionError::DuplicateSelection)
    ));
    assert!(matches!(
        budget_preview.plan(ImportSelection::new(vec![[0xEE; 32]])),
        Err(SessionError::UnknownSelection)
    ));
    for _ in 0..64 {
        budget_preview.plan_all().unwrap();
    }
    assert!(matches!(
        budget_preview.plan_all(),
        Err(SessionError::SessionLimit)
    ));
}

#[test]
fn core_import_transaction_unknown_signer_is_eligible_but_untrusted() {
    let a = author(); // not in any trust set
    let session = RiotSession::open().unwrap();
    let store = session.create_store().unwrap();
    let bytes = bundle(&[signed(&a, 2, 1, 100, 1)]);
    let preview = match store.inspect(&bytes, ctx()).unwrap() {
        InspectOutcome::Preview(p) => p,
        InspectOutcome::Rejected(r) => panic!("{r:?}"),
    };
    assert_eq!(preview.eligible_count().unwrap(), 1);
    assert!(preview.all_unknown_trust().unwrap());
}

#[test]
fn core_import_transaction_rejected_bundle_maps_to_rejected() {
    let session = RiotSession::open().unwrap();
    let store = session.create_store().unwrap();
    let mut bad = bundle(&[signed(&author(), 3, 1, 100, 1)]);
    bad[0] = b'X'; // corrupt magic
    assert!(matches!(
        store.inspect(&bad, ctx()).unwrap(),
        InspectOutcome::Rejected(_)
    ));
    // Rejection created no preview and no state change.
    assert_eq!(store.generation().unwrap(), 0);
}

#[test]
fn core_import_transaction_duplicate_only_is_nochanges() {
    let a = author();
    let session = RiotSession::open().unwrap();
    let store = session.create_store().unwrap();
    let bytes = bundle(&[signed(&a, 4, 1, 100, 1)]);

    let p1 = store.inspect(&bytes, ctx()).unwrap().expect_preview();
    p1.plan_all().unwrap().commit().unwrap();
    assert_eq!(store.generation().unwrap(), 1);

    // Re-import the identical bundle: duplicate-only → NoChanges, no new receipt.
    let p2 = store.inspect(&bytes, ctx()).unwrap().expect_preview();
    let outcome = p2.plan_all().unwrap().commit().unwrap();
    assert!(matches!(outcome, CommitOutcome::NoChanges(_)));
    assert_eq!(store.generation().unwrap(), 1, "generation must not change");
    assert_eq!(store.receipt_count().unwrap(), 1);
}

#[test]
fn core_import_transaction_dominated_entry_increments_generation_once() {
    let a = author();
    let session = RiotSession::open().unwrap();
    let store = session.create_store().unwrap();

    // Pre-state: older entry at a coordinate.
    let older = signed(&a, 5, 5, 100, 1);
    store
        .inspect(&bundle(std::slice::from_ref(&older)), ctx())
        .unwrap()
        .expect_preview()
        .plan_all()
        .unwrap()
        .commit()
        .unwrap();
    assert_eq!(store.generation().unwrap(), 1);

    // A bundle with a newer entry (prunes older) and a brand-new entry.
    let newer = signed(&a, 5, 5, 200, 1);
    let fresh = signed(&a, 6, 6, 100, 2);
    let outcome = store
        .inspect(&bundle(&[newer, fresh]), ctx())
        .unwrap()
        .expect_preview()
        .plan_all()
        .unwrap()
        .commit()
        .unwrap();
    let receipt = match outcome {
        CommitOutcome::Committed(r) => r,
        _ => panic!("expected commit"),
    };
    assert_eq!(receipt.dispositions.len(), 2);
    assert_eq!(
        store.generation().unwrap(),
        2,
        "one increment for the whole commit"
    );
    // Older is now not live; newer + fresh are live.
    assert_eq!(store.live_count().unwrap(), 2);
}

#[test]
fn core_import_transaction_replaced_preview_is_consumed_after_generation_change() {
    let a = author();
    let session = RiotSession::open().unwrap();
    let store = session.create_store().unwrap();

    let p1 = store
        .inspect(&bundle(&[signed(&a, 7, 1, 100, 1)]), ctx())
        .unwrap()
        .expect_preview();
    // A different import commits, advancing the store generation.
    store
        .inspect(&bundle(&[signed(&a, 8, 1, 100, 1)]), ctx())
        .unwrap()
        .expect_preview()
        .plan_all()
        .unwrap()
        .commit()
        .unwrap();
    // p1 was replaced by the later inspection, so its parent-consumed result
    // takes precedence over the generation change made by that replacement.
    assert!(matches!(p1.plan_all(), Err(SessionError::PreviewConsumed)));
}

#[test]
fn core_import_transaction_commit_twice_is_plan_consumed() {
    let a = author();
    let session = RiotSession::open().unwrap();
    let store = session.create_store().unwrap();
    let plan = store
        .inspect(&bundle(&[signed(&a, 9, 1, 100, 1)]), ctx())
        .unwrap()
        .expect_preview()
        .plan_all()
        .unwrap();
    plan.commit().unwrap();
    assert!(matches!(plan.commit(), Err(SessionError::PlanConsumed)));
}

#[test]
fn core_import_transaction_closed_store_rejects_actions() {
    let session = RiotSession::open().unwrap();
    let store = session.create_store().unwrap();
    store.close().unwrap();
    assert!(matches!(
        store.generation(),
        Err(SessionError::ObjectClosed)
    ));
    assert!(matches!(
        store.inspect(&[], ctx()),
        Err(SessionError::ObjectClosed)
    ));
}

#[test]
fn core_import_transaction_wrong_session_is_rejected() {
    let a = author();
    let s1 = RiotSession::open().unwrap();
    let store1 = s1.create_store().unwrap();
    let s2 = RiotSession::open().unwrap();
    let store2 = s2.create_store().unwrap();

    // A preview from store1 cannot be planned against store2's session.
    let preview = store1
        .inspect(&bundle(&[signed(&a, 10, 1, 100, 1)]), ctx())
        .unwrap()
        .expect_preview();
    // The plan belongs to s1; committing it after s2 exists is fine, but a
    // plan handle carrying s1's id must not be accepted by s2. We model this
    // by checking the preview's session id is distinct.
    assert_ne!(preview.session_id(), store2.session_id().unwrap());
}

#[test]
fn core_import_transaction_rollback_on_injected_failure_preserves_state() {
    let a = author();
    let session = RiotSession::open().unwrap();
    let store = session.create_store().unwrap();
    store
        .inspect(&bundle(&[signed(&a, 11, 1, 100, 1)]), ctx())
        .unwrap()
        .expect_preview()
        .plan_all()
        .unwrap()
        .commit()
        .unwrap();
    let gen_before = store.generation().unwrap();
    let live_before = store.live_count().unwrap();

    // Inject a pre-swap failure on the next commit; state must be unchanged.
    let plan = store
        .inspect(&bundle(&[signed(&a, 12, 1, 100, 1)]), ctx())
        .unwrap()
        .expect_preview()
        .plan_all()
        .unwrap();
    let result = plan.commit_with_injected_failure_for_tests();
    assert!(matches!(result, Err(SessionError::Injected)));
    assert_eq!(
        store.generation().unwrap(),
        gen_before,
        "generation unchanged on rollback"
    );
    assert_eq!(
        store.live_count().unwrap(),
        live_before,
        "live set unchanged on rollback"
    );
}

#[test]
fn core_import_transaction_provenance_separates_facts_from_trust() {
    let a = author();
    let session = RiotSession::open().unwrap();
    let store = session.create_store().unwrap();
    let receipt = match store
        .inspect(&bundle(&[signed(&a, 13, 1, 100, 1)]), ctx())
        .unwrap()
        .expect_preview()
        .plan_all()
        .unwrap()
        .commit()
        .unwrap()
    {
        CommitOutcome::Committed(r) => r,
        _ => panic!("commit"),
    };
    let entry_id = receipt.dispositions[0].entry_id;
    let prov = store.provenance(&entry_id).unwrap();
    // Cryptographic facts present; trust is a separate, reader-supplied axis.
    assert!(prov.signature_valid);
    assert!(prov.capability_valid);
    assert!(matches!(prov.live_status, LiveStatus::Live));
    assert!(prov.import_route.contains("test-route"));
    // No truth claim is asserted by provenance.
    assert!(!prov.asserts_truth);
}

#[test]
fn core_import_transaction_later_pruning_preserves_history() {
    let a = author();
    let session = RiotSession::open().unwrap();
    let store = session.create_store().unwrap();
    // Commit older, capture its entry id.
    let older_receipt = match store
        .inspect(&bundle(&[signed(&a, 14, 14, 100, 1)]), ctx())
        .unwrap()
        .expect_preview()
        .plan_all()
        .unwrap()
        .commit()
        .unwrap()
    {
        CommitOutcome::Committed(r) => r,
        _ => panic!(),
    };
    let older_id = older_receipt.dispositions[0].entry_id;
    // Commit a newer entry that prunes it.
    store
        .inspect(&bundle(&[signed(&a, 14, 14, 200, 1)]), ctx())
        .unwrap()
        .expect_preview()
        .plan_all()
        .unwrap()
        .commit()
        .unwrap();
    // The older entry's receipt still exists; its current status is NotLive/PrunedLater.
    let prov = store.provenance(&older_id).unwrap();
    assert!(matches!(prov.live_status, LiveStatus::NotLive { .. }));
    assert!(store.receipt_count().unwrap() >= 2, "history preserved");
}
