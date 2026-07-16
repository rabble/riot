//! Fail-closed evidence for the session arbiter, the namespace-local join, and
//! communal/owned author generation. Every test provokes a *real* refusal or
//! terminal condition and asserts the exact error variant plus, where it
//! applies, that observable state is unchanged after the refusal.
//!
//! Requires the `conformance` feature: it uses the injectable entropy source
//! (`generate_communal_author_with`) to force a mid-generation entropy
//! failure, which lives behind that feature.

use riot_core::apps::entry::app_data_path;
use riot_core::import::encode_bundle;
use riot_core::model::{AlertPayload, Certainty, Severity, Urgency};
use riot_core::session::{
    CommitOutcome, EvidenceStore, ImportContext, InspectOutcome, RiotSession, SessionError,
};
use riot_core::willow::{
    authorise_entry, build_alert_entry, encode_capability, encode_entry, entry_id,
    generate_communal_author, generate_communal_author_for_namespace,
    generate_communal_author_with, EntropySource, Entry, EntryId, EvidenceAuthor,
    SignedWillowEntry, WillowError,
};
use willow25::entry::NamespaceSecret;

// ---------------------------------------------------------------------------
// Shared builders.
// ---------------------------------------------------------------------------

fn alert_payload(tag: u8) -> Vec<u8> {
    riot_core::model::encode_alert(&AlertPayload {
        object_id: [tag; 16],
        revision_id: [tag; 16],
        created_at: 1_000,
        valid_from: None,
        expires_at: 2_000,
        language: "en".into(),
        urgency: Urgency::Immediate,
        severity: Severity::Severe,
        certainty: Certainty::Observed,
        headline: format!("fail-closed alert {tag}"),
        description: "Fail-closed fixture.".into(),
        affected_area_claim: None,
        source_claims: vec!["fixture".into()],
        ai_assisted: false,
    })
    .expect("valid alert payload")
}

fn signed_alert(author: &EvidenceAuthor, tag: u8) -> SignedWillowEntry {
    let payload = alert_payload(tag);
    let entry = build_alert_entry(author, &[tag; 16], &[tag; 16], 100, &payload).expect("entry");
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

fn commit_alert(store: &EvidenceStore, author: &EvidenceAuthor, tag: u8) -> EntryId {
    let signed = signed_alert(author, tag);
    let id = entry_id(&signed.entry_bytes);
    let bytes = encode_bundle(std::slice::from_ref(&signed)).expect("bundle");
    let preview = store
        .inspect(&bytes, ImportContext::new("fail-closed"))
        .expect("inspect")
        .expect_preview();
    match preview.plan_all().expect("plan").commit().expect("commit") {
        CommitOutcome::Committed(_) => {}
        CommitOutcome::NoChanges(_) => panic!("expected a fresh alert to commit"),
    }
    id
}

fn signed_app_data(author: &EvidenceAuthor, key: &str, payload: &[u8]) -> SignedWillowEntry {
    let path = app_data_path(author_app_id(), key).expect("app-data path");
    let entry = Entry::builder()
        .namespace_id(author.namespace_id().clone())
        .subspace_id(author.subspace_id())
        .path(path)
        .timestamp(100)
        .payload(payload)
        .build();
    let authorised = authorise_entry(author, entry).expect("authorise");
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    SignedWillowEntry {
        entry_bytes: encode_entry(authorised.entry()),
        capability_bytes: encode_capability(token.capability()),
        signature: signature.to_bytes(),
        payload_bytes: payload.to_vec(),
    }
}

fn author_app_id() -> &'static [u8; 32] {
    &[3u8; 32]
}

// ---------------------------------------------------------------------------
// session.rs: InspectOutcome::expect_preview panics on a rejected bundle.
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "expected preview, got rejection")]
fn expect_preview_panics_when_the_bundle_is_rejected() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    // Bytes with no valid bundle framing decode to a structural rejection,
    // never a preview.
    let outcome = store
        .inspect(b"definitely not a riot bundle", ImportContext::new("junk"))
        .expect("inspect returns a decode outcome");
    assert!(matches!(outcome, InspectOutcome::Rejected(_)));
    // Consumes the rejection through the test-convenience unwrap, which panics.
    let _ = outcome.expect_preview();
}

// ---------------------------------------------------------------------------
// Forgetting an id that is not live is a refusal: session.rs forget_entry
// surfaces Internal because join.rs JoinState::forget_entry returns false, and
// the generation and live view are left untouched.
// ---------------------------------------------------------------------------

#[test]
fn forgetting_a_non_live_entry_is_internal_and_changes_nothing() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let author = generate_communal_author().expect("author");
    let live = commit_alert(&store, &author, 1);

    let generation = store.generation().expect("generation");
    let live_before = store.live_entry_ids().expect("live ids");
    assert!(live_before.contains(&live));

    // An id that was never accepted is not in the live set: forget_entry's
    // inner `JoinState::forget_entry` returns false, surfaced as Internal.
    let never_seen = [0xABu8; 32];
    assert_eq!(store.forget_entry(&never_seen), Err(SessionError::Internal));

    // The refusal is a no-op: same generation, same live view.
    assert_eq!(store.generation().expect("generation"), generation);
    assert_eq!(store.live_entry_ids().expect("live ids"), live_before);
}

// ---------------------------------------------------------------------------
// session.rs StalePreview at plan(): a commit-free generation bump
// via forget_entry makes a still-current preview stale before it can plan.
// ---------------------------------------------------------------------------

#[test]
fn planning_a_preview_after_a_generation_bump_is_stale() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let author = generate_communal_author().expect("author");
    let live = commit_alert(&store, &author, 1);
    let base = store.generation().expect("generation");

    // A live preview is pinned to `base`.
    let bytes = encode_bundle(std::slice::from_ref(&signed_alert(&author, 2))).expect("bundle");
    let preview = store
        .inspect(&bytes, ImportContext::new("stale-plan"))
        .expect("inspect")
        .expect_preview();

    // forget_entry advances the store generation without touching the preview.
    store.forget_entry(&live).expect("forget");
    assert_eq!(store.generation().expect("generation"), base + 1);

    // Planning now sees a generation different from the preview's base.
    assert!(matches!(
        preview.plan_all(),
        Err(SessionError::StalePreview)
    ));
}

// ---------------------------------------------------------------------------
// session.rs StalePreview at commit(): the plan was created at the
// current generation, then forget_entry bumped it before commit.
// ---------------------------------------------------------------------------

#[test]
fn committing_a_plan_after_a_generation_bump_is_stale() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let author = generate_communal_author().expect("author");
    let live = commit_alert(&store, &author, 1);
    let base = store.generation().expect("generation");

    let bytes = encode_bundle(std::slice::from_ref(&signed_alert(&author, 2))).expect("bundle");
    let preview = store
        .inspect(&bytes, ImportContext::new("stale-commit"))
        .expect("inspect")
        .expect_preview();
    // The plan captures `base` as its base generation.
    let plan = preview.plan_all().expect("plan");

    // Bump the generation out from under the still-current plan.
    store.forget_entry(&live).expect("forget");
    assert_eq!(store.generation().expect("generation"), base + 1);

    assert_eq!(plan.commit(), Err(SessionError::StalePreview));
    // The refusal did not advance the generation further.
    assert_eq!(store.generation().expect("generation"), base + 1);
}

// ---------------------------------------------------------------------------
// session.rs StoreFull from store_charge_exceeds_budget at commit: enough live app-data payload bytes to cross the 16 MiB retained
// store budget. Each ~1 MiB app-data entry stays live (distinct path, no
// pruning), so the retained-live-byte charge accumulates until one further
// commit would exceed the budget.
// ---------------------------------------------------------------------------

#[test]
fn accumulated_live_payload_bytes_trip_the_retained_store_budget() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let author = generate_communal_author().expect("author");

    // Just under the 1 MiB per-item payload ceiling; comfortably under the
    // 2 MiB preview-output budget for a single entry.
    let payload = vec![0x5Au8; 1_040_000];

    let mut committed = 0usize;
    let mut hit_store_full = false;
    for i in 0..40u32 {
        let key = format!("items/e{i:06}");
        let signed = signed_app_data(&author, &key, &payload);
        let bytes = encode_bundle(std::slice::from_ref(&signed)).expect("bundle");
        let preview = store
            .inspect(&bytes, ImportContext::new("budget"))
            .expect("inspect")
            .expect_preview();
        let plan = preview.plan_all().expect("plan");
        match plan.commit() {
            Ok(CommitOutcome::Committed(_)) => committed += 1,
            Ok(CommitOutcome::NoChanges(_)) => panic!("distinct app-data must commit"),
            Err(SessionError::StoreFull) => {
                hit_store_full = true;
                break;
            }
            Err(other) => panic!("unexpected error before budget: {other:?}"),
        }
    }

    assert!(
        hit_store_full,
        "expected the retained-store budget to reject a commit"
    );
    // The rejected commit left the previously committed entries intact.
    assert_eq!(store.live_count().expect("live"), committed);
    assert_eq!(store.generation().expect("generation"), committed as u64);
}

// ---------------------------------------------------------------------------
// willow/identity.rs NamespaceNotCommunal: a non-communal namespace
// id is refused before any signing material is drawn.
// ---------------------------------------------------------------------------

#[test]
fn communal_author_for_a_non_communal_namespace_is_refused() {
    // A namespace id whose public key is not communal (odd final byte).
    let mut non_communal = [0u8; 32];
    non_communal[31] = 1;
    assert!(
        matches!(
            generate_communal_author_for_namespace(non_communal),
            Err(WillowError::NamespaceNotCommunal)
        ),
        "a non-communal namespace id must be rejected"
    );
}

// ---------------------------------------------------------------------------
// willow/identity.rs subspace-secret error propagation in `generate`: entropy that succeeds long enough to fix a communal namespace, then
// fails on the subspace draw, propagates EntropyUnavailable.
// ---------------------------------------------------------------------------

struct EntropyThatFailsAfterFirstDraw {
    calls: usize,
    communal_secret: [u8; 32],
}

impl EntropySource for EntropyThatFailsAfterFirstDraw {
    fn fill(&mut self, buf: &mut [u8]) -> Result<(), WillowError> {
        self.calls += 1;
        if self.calls == 1 {
            // First draw fixes a communal namespace in one iteration.
            buf.copy_from_slice(&self.communal_secret);
            Ok(())
        } else {
            // The subsequent subspace draw fails.
            Err(WillowError::EntropyUnavailable)
        }
    }
}

#[test]
fn generation_propagates_entropy_failure_on_the_subspace_draw() {
    // Pick a secret whose namespace public key is communal so the namespace
    // loop completes on the first draw and the *second* draw is the subspace.
    let mut seed = [0u8; 32];
    let communal_secret = loop {
        if NamespaceSecret::from_bytes(&seed)
            .corresponding_namespace_id()
            .is_communal()
        {
            break seed;
        }
        seed[0] = seed[0].wrapping_add(1);
    };

    let mut entropy = EntropyThatFailsAfterFirstDraw {
        calls: 0,
        communal_secret,
    };
    assert!(
        matches!(
            generate_communal_author_with(&mut entropy),
            Err(WillowError::EntropyUnavailable)
        ),
        "a subspace-draw failure must abort generation"
    );
    // The namespace draw succeeded, the subspace draw failed: exactly two draws.
    assert_eq!(entropy.calls, 2);
}
