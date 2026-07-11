//! WU2 G2 evidence: the store's retained byte-charge accounting from
//! fixtures/manifest.json (`retained_store_budget_bytes` = 16 MiB, charged
//! at `store_charge_entry_bytes` = 512/entry, `store_charge_receipt_bytes`
//! = 256/receipt, `store_charge_digest_reference_bytes` = 32/reference,
//! `store_charge_namespace_bytes` = 256 once) is wired into the arbiter and
//! matches the exact formula, not just present as unused ceilings.
//!
//! Under Phase 0A's fixed-length alert path scheme, pruning is inherently
//! 1:1 (see core_import_join's prefix-pruning notes), so the legitimate
//! maximum retained charge under the existing count ceilings (MAX_STORE_
//! ENTRIES * (512 + MAX_ENTRY_BYTES) + MAX_RECEIPTS * (256 + MAX_BUNDLE_
//! ENTRIES * 32) ≈ 5.06 MiB) never reaches the 16 MiB budget — these tests
//! verify the formula and wiring; `session::charge_budget_tests` verifies
//! the ceiling boundary itself with a pure-arithmetic unit test.

use riot_core::import::encode_bundle;
use riot_core::model::{AlertPayload, Certainty, Severity, Urgency};
use riot_core::session::{CommitOutcome, ImportContext, RiotSession};
use riot_core::willow::{
    authorise_entry, build_alert_entry, encode_capability, encode_entry, EvidenceAuthor,
    SignedWillowEntry,
};

const STORE_CHARGE_NAMESPACE_BYTES: u64 = 256;
const STORE_CHARGE_RECEIPT_BYTES: u64 = 256;
const STORE_CHARGE_ENTRY_BYTES: u64 = 512;
const STORE_CHARGE_DIGEST_REFERENCE_BYTES: u64 = 32;

fn author() -> EvidenceAuthor {
    use willow25::entry::NamespaceSecret;
    let mut seed = *b"riot-chg-namespace-secret-00001!";
    let ns = loop {
        let c = NamespaceSecret::from_bytes(&seed).corresponding_namespace_id();
        if c.is_communal() {
            break c;
        }
        seed[0] = seed[0].wrapping_add(1);
    };
    EvidenceAuthor::from_parts_for_tests(ns, b"riot-chg-subspace-secret-000001!")
}

fn signed(author: &EvidenceAuthor, object: u8, revision: u8, timestamp: u64) -> SignedWillowEntry {
    let payload = riot_core::model::encode_alert(&AlertPayload {
        object_id: [object; 16],
        revision_id: [revision; 16],
        created_at: 1_000,
        valid_from: None,
        expires_at: 2_000,
        language: "en".into(),
        urgency: Urgency::Immediate,
        severity: Severity::Severe,
        certainty: Certainty::Observed,
        headline: "charge budget alert".into(),
        description: "Charge budget fixture.".into(),
        affected_area_claim: None,
        source_claims: vec!["fixture".into()],
        ai_assisted: false,
    })
    .expect("payload");
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

fn bundle(items: &[SignedWillowEntry]) -> Vec<u8> {
    encode_bundle(items).expect("encode bundle")
}

fn ctx() -> ImportContext {
    ImportContext::new("charge-route")
}

#[test]
fn core_import_charge_budget_new_store_starts_at_the_namespace_charge() {
    let session = RiotSession::open().unwrap();
    let store = session.create_store().unwrap();
    assert_eq!(
        store
            .retained_store_charge_bytes_for_conformance()
            .unwrap(),
        STORE_CHARGE_NAMESPACE_BYTES
    );
}

#[test]
fn core_import_charge_budget_single_commit_charges_exactly_the_formula() {
    let a = author();
    let session = RiotSession::open().unwrap();
    let store = session.create_store().unwrap();
    let entry = signed(&a, 1, 1, 100);
    let entry_bytes_len = entry.entry_bytes.len() as u64;

    store
        .inspect(&bundle(&[entry]), ctx())
        .unwrap()
        .expect_preview()
        .plan_all()
        .unwrap()
        .commit()
        .unwrap();

    let expected = STORE_CHARGE_NAMESPACE_BYTES
        + STORE_CHARGE_RECEIPT_BYTES // one receipt, no references (a fresh winner)
        + STORE_CHARGE_ENTRY_BYTES
        + entry_bytes_len;
    assert_eq!(
        store
            .retained_store_charge_bytes_for_conformance()
            .unwrap(),
        expected
    );
}

#[test]
fn core_import_charge_budget_pruning_drops_entry_charge_but_keeps_receipt_history_charge() {
    let a = author();
    let session = RiotSession::open().unwrap();
    let store = session.create_store().unwrap();

    let older = signed(&a, 2, 2, 100);
    let older_bytes_len = older.entry_bytes.len() as u64;
    store
        .inspect(&bundle(&[older]), ctx())
        .unwrap()
        .expect_preview()
        .plan_all()
        .unwrap()
        .commit()
        .unwrap();
    let charge_after_first = store
        .retained_store_charge_bytes_for_conformance()
        .unwrap();
    assert_eq!(
        charge_after_first,
        STORE_CHARGE_NAMESPACE_BYTES + STORE_CHARGE_RECEIPT_BYTES + STORE_CHARGE_ENTRY_BYTES + older_bytes_len
    );

    // A newer entry at the same coordinate prunes the older one: exactly
    // one digest reference is charged, the older entry's own bytes stop
    // being charged as a live entry, and the newer entry's bytes start.
    let newer = signed(&a, 2, 2, 200);
    let newer_bytes_len = newer.entry_bytes.len() as u64;
    let outcome = store
        .inspect(&bundle(&[newer]), ctx())
        .unwrap()
        .expect_preview()
        .plan_all()
        .unwrap()
        .commit()
        .unwrap();
    assert!(matches!(outcome, CommitOutcome::Committed(_)));

    let charge_after_second = store
        .retained_store_charge_bytes_for_conformance()
        .unwrap();
    let expected = STORE_CHARGE_NAMESPACE_BYTES
        + 2 * STORE_CHARGE_RECEIPT_BYTES
        + STORE_CHARGE_DIGEST_REFERENCE_BYTES // one pruned reference in receipt 2
        + STORE_CHARGE_ENTRY_BYTES
        + newer_bytes_len; // only the newer entry is live now
    assert_eq!(charge_after_second, expected);
    assert!(
        charge_after_second < charge_after_first + STORE_CHARGE_RECEIPT_BYTES + STORE_CHARGE_DIGEST_REFERENCE_BYTES + newer_bytes_len,
        "the older entry's own bytes must stop being charged once pruned"
    );
}
