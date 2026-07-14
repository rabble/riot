//! The rules that keep a review honest once the store has moved underneath it.
//!
//! A preview is a promise about a *specific* store generation: "these are the
//! entries that would be admitted, given what the store holds right now". The
//! moment the store changes, that promise is void — and the only safe thing to
//! do is refuse, because a plan built against a stale view could admit an entry
//! whose supersession relationships have since changed.
//!
//! `forget_entry` is what moves the store without going through a review, so it
//! is the lever these tests pull. Every other mutation runs through the review
//! itself and consumes it.

use riot_core::import::encode_bundle;
use riot_core::model::{encode_alert, AlertPayload, Certainty, Severity, Urgency};
use riot_core::session::{
    EvidenceStore, ImportContext, ImportSelection, InspectOutcome, RiotSession, SessionError,
};
use riot_core::willow::{
    authorise_entry, build_alert_entry, encode_capability, encode_entry, entry_id,
    generate_communal_author, EvidenceAuthor, SignedWillowEntry,
};

fn alert_bytes(object: u8, headline: &str) -> Vec<u8> {
    encode_alert(&AlertPayload {
        // The payload's object id is bound to the path's, so it has to vary
        // with it — an alert whose payload names a different object than its
        // own path is refused at admission.
        object_id: [object; 16],
        revision_id: [2; 16],
        created_at: 1_800_000_000,
        valid_from: None,
        expires_at: 1_900_000_000,
        language: "en".into(),
        urgency: Urgency::Immediate,
        severity: Severity::Severe,
        certainty: Certainty::Observed,
        headline: headline.into(),
        description: "A description long enough to be a real alert.".into(),
        affected_area_claim: None,
        source_claims: vec!["field observer".into()],
        ai_assisted: false,
    })
    .expect("encode alert")
}

/// One signed alert, distinguished by its object id so each is a separate
/// Willow coordinate rather than a supersession of the last.
fn signed_alert(author: &EvidenceAuthor, object: u8, headline: &str) -> SignedWillowEntry {
    let payload = alert_bytes(object, headline);
    let entry = build_alert_entry(author, &[object; 16], &[2; 16], 100, &payload).expect("entry");
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

fn store() -> EvidenceStore {
    let session = RiotSession::open().expect("session");
    session.create_store().expect("store")
}

/// Commits `signed` through a full review and returns its entry id.
fn admit(store: &EvidenceStore, signed: &SignedWillowEntry) -> [u8; 32] {
    let bytes = encode_bundle(std::slice::from_ref(signed)).expect("bundle");
    let preview = store
        .inspect(&bytes, ImportContext::new("peer"))
        .expect("inspect")
        .expect_preview();
    preview.plan_all().expect("plan").commit().expect("commit");
    entry_id(&signed.entry_bytes)
}

/// A preview built before the store moved cannot be planned against it. The
/// store changed underneath the review, so the entries the caller was shown are
/// no longer the entries that would be admitted.
#[test]
fn a_preview_is_stale_once_the_store_moves_under_it() {
    let author = generate_communal_author().expect("author");
    let store = store();

    // Something to forget, so the store can move without going through a review.
    let resident = signed_alert(&author, 1, "Already here");
    let resident_id = admit(&store, &resident);

    // A peer's bundle, previewed against the store as it stands.
    let incoming = signed_alert(&author, 2, "Just arrived");
    let bytes = encode_bundle(std::slice::from_ref(&incoming)).expect("bundle");
    let preview = store
        .inspect(&bytes, ImportContext::new("peer"))
        .expect("inspect")
        .expect_preview();
    assert_eq!(preview.eligible_count().expect("eligible"), 1);

    // The store moves — not through this review, so the preview is left
    // installed but is now describing a store that no longer exists.
    store.forget_entry(&resident_id).expect("forget");

    assert!(
        matches!(
            preview.plan(ImportSelection::all()),
            Err(SessionError::StalePreview)
        ),
        "a plan built on a stale view could admit an entry against the wrong supersession set"
    );
    assert!(
        matches!(preview.plan_all(), Err(SessionError::StalePreview)),
        "plan_all is the same admission, and must refuse identically"
    );

    // The store is unharmed and still usable: a fresh review of the same bytes
    // works, which is the caller's actual remedy.
    let preview = store
        .inspect(&bytes, ImportContext::new("peer-retry"))
        .expect("inspect")
        .expect_preview();
    preview.plan_all().expect("plan").commit().expect("commit");
}

/// The same rule one step later: a *plan* is also bound to the generation it was
/// built against, and refuses to commit into a store that has moved since.
#[test]
fn a_plan_is_stale_once_the_store_moves_under_it() {
    let author = generate_communal_author().expect("author");
    let store = store();

    let resident = signed_alert(&author, 1, "Already here");
    let resident_id = admit(&store, &resident);

    let incoming = signed_alert(&author, 2, "Just arrived");
    let bytes = encode_bundle(std::slice::from_ref(&incoming)).expect("bundle");
    let preview = store
        .inspect(&bytes, ImportContext::new("peer"))
        .expect("inspect")
        .expect_preview();
    let plan = preview.plan_all().expect("plan");

    // The plan was built here; the store moves after it.
    let live_before = store.live_count().expect("live count");
    store.forget_entry(&resident_id).expect("forget");

    assert!(
        matches!(plan.commit(), Err(SessionError::StalePreview)),
        "a plan must not commit into a store it was not built against"
    );

    // Nothing from the refused plan reached the store: the only change is the
    // forget that made it stale.
    assert_eq!(store.live_count().expect("live count"), live_before - 1);
}

/// A superseded plan is closed, and says so. Two plans can be issued from one
/// preview — the second replaces the first — and the first is then terminal:
/// closing it reports *why* it is dead rather than silently succeeding.
#[test]
fn a_superseded_plan_reports_why_it_is_dead() {
    let author = generate_communal_author().expect("author");
    let store = store();

    let first = signed_alert(&author, 1, "First");
    let second = signed_alert(&author, 2, "Second");
    let bytes = encode_bundle(&[first.clone(), second.clone()]).expect("bundle");
    let preview = store
        .inspect(&bytes, ImportContext::new("peer"))
        .expect("inspect")
        .expect_preview();

    // Two plans from one preview: the caller changed their selection.
    let discarded = preview
        .plan(ImportSelection::new(vec![entry_id(&first.entry_bytes)]))
        .expect("first plan");
    let live = preview
        .plan(ImportSelection::new(vec![entry_id(&second.entry_bytes)]))
        .expect("second plan supersedes the first");

    assert_eq!(
        discarded.close(),
        Err(SessionError::PlanSuperseded),
        "closing a superseded plan must name what happened to it"
    );
    assert!(
        matches!(discarded.commit(), Err(SessionError::PlanSuperseded)),
        "and committing it must refuse for the same, stated reason"
    );

    // The live plan is unaffected and commits exactly its own selection.
    live.commit().expect("the surviving plan commits");
    let live_ids = store.live_entry_ids().expect("live ids");
    assert_eq!(live_ids, vec![entry_id(&second.entry_bytes)]);
}

/// Forgetting an entry the store never accepted is a caller error, not a
/// silent no-op — and it leaves the store untouched.
#[test]
fn forgetting_an_unknown_entry_is_refused_without_moving_the_store() {
    let author = generate_communal_author().expect("author");
    let store = store();
    let resident = signed_alert(&author, 1, "Already here");
    let resident_id = admit(&store, &resident);

    let generation = store.generation().expect("generation");
    assert_eq!(
        store.forget_entry(&[0xab; 32]),
        Err(SessionError::Internal),
        "an entry that was never accepted cannot be forgotten"
    );
    assert_eq!(
        store.generation().expect("generation"),
        generation,
        "a refused forget must not consume a generation"
    );
    assert_eq!(store.live_entry_ids().expect("live"), vec![resident_id]);

    // Forgetting it twice is likewise refused the second time: after the first,
    // it is no longer live.
    store.forget_entry(&resident_id).expect("forget");
    assert_eq!(
        store.forget_entry(&resident_id),
        Err(SessionError::Internal)
    );
    assert!(store.live_entry_ids().expect("live").is_empty());
}

/// `expect_preview` is a test convenience, and it is honest about it: handed a
/// rejection, it panics naming the rejection rather than unwrapping into
/// something misleading.
#[test]
#[should_panic(expected = "expected preview, got rejection")]
fn expect_preview_panics_on_a_rejection_rather_than_hiding_it() {
    let store = store();
    let outcome = store
        .inspect(&[0xff; 32], ImportContext::new("garbage"))
        .expect("inspect returns a rejection, not an error");
    assert!(matches!(outcome, InspectOutcome::Rejected(_)));
    let _ = outcome.expect_preview();
}
