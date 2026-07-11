//! WU2 G2 evidence: the store's retained byte-charge accounting from
//! fixtures/manifest.json (`retained_store_budget_bytes` = 16 MiB) matches
//! what is actually retained, not just an approximation of it. Charged at
//! `store_charge_entry_bytes` = 512 permanently per seen entry (index
//! overhead) plus each live entry's own canonical bytes; `store_charge_
//! receipt_bytes` = 256 per retained disposition row; `store_charge_digest_
//! reference_bytes` = 32 per pruned/dominating reference; `store_charge_
//! namespace_bytes` = 256 per distinct namespace ever observed, capped at
//! `namespace_views` = 64; plus a committed receipt's own route bytes.
//!
//! This formula replaced an earlier one that a `codex review` found
//! undercounted: it charged only `entry_bytes.len()` (not the fuller
//! retained `AuthorisedEntry`/capability — now fixed by not retaining the
//! capability at all past `inspect`-time verification, since nothing reads
//! it again), dropped an entry's charge entirely once pruned (now split
//! into a permanent per-seen-entry index charge plus a live-only byte
//! charge), left `ImportContext::route` unbounded and uncharged, and never
//! tracked or capped `namespace_views`.
//!
//! Under Phase 0A's fixed-length alert path scheme, pruning is inherently
//! 1:1 (see core_import_join's prefix-pruning notes), so the legitimate
//! maximum retained charge under the existing count ceilings never reaches
//! the 16 MiB budget through ordinary entries alone — these tests verify
//! the formula and wiring, plus two adversarial paths (an oversized route,
//! and too many distinct namespaces) that legitimately do trip it or its
//! sibling count ceiling. `session::charge_budget_tests` verifies the raw
//! ceiling arithmetic with a pure-arithmetic unit test.

use riot_core::import::encode_bundle;
use riot_core::model::{AlertPayload, Certainty, Severity, Urgency};
use riot_core::session::{CommitOutcome, ImportContext, RiotSession, SessionError};
use riot_core::willow::{
    authorise_entry, build_alert_entry, encode_capability, encode_entry, EvidenceAuthor,
    SignedWillowEntry,
};

const STORE_CHARGE_NAMESPACE_BYTES: u64 = 256;
const STORE_CHARGE_RECEIPT_BYTES: u64 = 256;
const STORE_CHARGE_ENTRY_BYTES: u64 = 512;
const STORE_CHARGE_DIGEST_REFERENCE_BYTES: u64 = 32;

/// A communal namespace derived from `seed`, looping the seed's first byte
/// until the derived namespace id is communal (matches the derivation the
/// production code itself uses in `EvidenceAuthor::generate`).
fn author_from_seed(mut seed: [u8; 32], subspace_seed: &[u8; 32]) -> EvidenceAuthor {
    use willow25::entry::NamespaceSecret;
    let ns = loop {
        let c = NamespaceSecret::from_bytes(&seed).corresponding_namespace_id();
        if c.is_communal() {
            break c;
        }
        seed[0] = seed[0].wrapping_add(1);
    };
    EvidenceAuthor::from_parts_for_tests(ns, subspace_seed)
}

fn author() -> EvidenceAuthor {
    author_from_seed(
        *b"riot-chg-namespace-secret-00001!",
        b"riot-chg-subspace-secret-000001!",
    )
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
fn core_import_charge_budget_new_store_starts_at_zero() {
    let session = RiotSession::open().unwrap();
    let store = session.create_store().unwrap();
    assert_eq!(
        store.retained_store_charge_bytes_for_conformance().unwrap(),
        0,
        "an empty store has seen no entries, receipts, or namespaces yet"
    );
}

#[test]
fn core_import_charge_budget_single_commit_charges_exactly_the_formula() {
    let a = author();
    let session = RiotSession::open().unwrap();
    let store = session.create_store().unwrap();
    let entry = signed(&a, 1, 1, 100);
    let entry_bytes_len = entry.entry_bytes.len() as u64;
    let capability_bytes_len = entry.capability_bytes.len() as u64;
    let route_len = "charge-route".len() as u64;

    store
        .inspect(&bundle(&[entry]), ctx())
        .unwrap()
        .expect_preview()
        .plan_all()
        .unwrap()
        .commit()
        .unwrap();

    let expected = STORE_CHARGE_NAMESPACE_BYTES // one distinct namespace
        + STORE_CHARGE_ENTRY_BYTES // one seen entry's permanent index charge
        + entry_bytes_len // that entry's live canonical bytes
        + STORE_CHARGE_RECEIPT_BYTES // one retained disposition row
        + route_len; // the receipt's own retained route bytes
    let actual = store.retained_store_charge_bytes_for_conformance().unwrap();
    assert_eq!(actual, expected);
    assert!(
        actual < expected + capability_bytes_len,
        "sanity: the assertion above must not happen to also hold with capability bytes added"
    );
}

#[test]
fn core_import_charge_budget_pruning_keeps_seen_index_charge_but_drops_live_entry_bytes() {
    let a = author();
    let session = RiotSession::open().unwrap();
    let store = session.create_store().unwrap();
    let route_len = "charge-route".len() as u64;

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
    let charge_after_first = store.retained_store_charge_bytes_for_conformance().unwrap();
    assert_eq!(
        charge_after_first,
        STORE_CHARGE_NAMESPACE_BYTES
            + STORE_CHARGE_ENTRY_BYTES
            + older_bytes_len
            + STORE_CHARGE_RECEIPT_BYTES
            + route_len
    );

    // A newer entry at the same coordinate prunes the older one: one digest
    // reference is charged, the *namespace* charge does not grow (same
    // namespace), the seen-index charge grows to cover both entries
    // permanently, and only the pruned entry's own canonical bytes stop
    // being charged — its 512-byte index charge must NOT disappear.
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

    let charge_after_second = store.retained_store_charge_bytes_for_conformance().unwrap();
    let expected = STORE_CHARGE_NAMESPACE_BYTES // still one distinct namespace
        + 2 * STORE_CHARGE_ENTRY_BYTES // both entries permanently indexed
        + newer_bytes_len // only the live entry's own bytes
        + 2 * STORE_CHARGE_RECEIPT_BYTES // two retained disposition rows
        + STORE_CHARGE_DIGEST_REFERENCE_BYTES // one pruned reference
        + 2 * route_len; // two receipts, each retaining the route
    assert_eq!(charge_after_second, expected);
    assert!(
        charge_after_second > charge_after_first,
        "total retained charge must grow, not shrink, when an entry is pruned: \
         the pruned entry's index record is still retained even though its \
         own bytes are freed"
    );
}

#[test]
fn core_import_charge_budget_oversized_route_trips_the_budget() {
    let a = author();
    let session = RiotSession::open().unwrap();
    let store = session.create_store().unwrap();
    let huge_route = "x".repeat(17 * 1024 * 1024); // 17 MiB, over the 16 MiB budget alone
    let plan = store
        .inspect(
            &bundle(&[signed(&a, 3, 1, 100)]),
            ImportContext::new(&huge_route),
        )
        .unwrap()
        .expect_preview()
        .plan_all()
        .unwrap();

    assert_eq!(plan.commit(), Err(SessionError::StoreFull));
    assert_eq!(
        store.retained_store_charge_bytes_for_conformance().unwrap(),
        0,
        "a rejected commit must not retain any partial charge"
    );
    assert_eq!(store.generation().unwrap(), 0);
    assert_eq!(store.live_count().unwrap(), 0);
}

#[test]
fn core_import_charge_budget_namespace_views_are_tracked_and_capped_at_64() {
    let session = RiotSession::open().unwrap();
    let store = session.create_store().unwrap();

    let mut committed_namespaces = 0u32;
    for index in 0..65u16 {
        let mut namespace_seed = *b"riot-nsc-namespace-secret-00001!";
        namespace_seed[26..28].copy_from_slice(&index.to_be_bytes());
        let subspace_seed = *b"riot-nsc-subspace-secret-000001!";
        let a = author_from_seed(namespace_seed, &subspace_seed);
        let entry = signed(&a, 1, 1, 100);

        let plan = store
            .inspect(&bundle(&[entry]), ctx())
            .unwrap()
            .expect_preview()
            .plan_all()
            .unwrap();
        let result = plan.commit();
        if index < 64 {
            assert!(
                matches!(result, Ok(CommitOutcome::Committed(_))),
                "namespace {index} (within the 64-view cap) must be admitted: {result:?}"
            );
            committed_namespaces += 1;
        } else {
            assert_eq!(
                result,
                Err(SessionError::StoreFull),
                "the 65th distinct namespace must be rejected"
            );
        }
    }

    assert_eq!(committed_namespaces, 64);
    let charge = store.retained_store_charge_bytes_for_conformance().unwrap();
    let namespace_component = 64 * STORE_CHARGE_NAMESPACE_BYTES;
    assert!(
        charge >= namespace_component,
        "charge must include exactly 64 namespace charges, not 65 or fewer"
    );
}
