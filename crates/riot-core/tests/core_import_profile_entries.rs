//! Admission-boundary evidence for profile entries
//! (`profile/<32-byte subspace_id>/card`). The import pipeline rejects every
//! path family as `UnsupportedSchema` until it is explicitly admitted at two
//! independent gates, and the `profile/` family is admitted here:
//!
//! - `verify_frame` (schema): the payload must decode as a canonical
//!   `ProfileCard`, and the whole `profile/` prefix is RESERVED — a malformed
//!   profile path is rejected outright and can never fall through to the alert
//!   schema check, not even carrying a perfectly valid alert payload.
//! - `inspect` (binding): the entry's signing subspace must EQUAL the subspace
//!   component of its path — this is what stops one person writing a display
//!   name into someone else's slot.
//!
//! Deliberately policy-free: nothing here checks whether a name is allowed,
//! unique, or inoffensive. Admission checks shape, schema, and slot ownership
//! only; name collisions are a render-time concern (the key-suffix rule).

use riot_core::apps::entry::app_data_path;
use riot_core::import::{encode_bundle, BundleEncodeError, DiagnosticCode, ItemComponent};
use riot_core::model::{encode_alert, AlertPayload, Certainty, Severity, Urgency};
use riot_core::profile::card::{decode_profile_card, encode_profile_card, ProfileCard};
use riot_core::profile::path::{profile_card_path, profile_prefix, PROFILE_COMPONENT};
use riot_core::session::{
    CommitOutcome, EvidenceStore, ImportContext, InspectOutcome, RiotSession,
};
use riot_core::willow::{
    authorise_entry, build_alert_entry, encode_capability, encode_entry, generate_communal_author,
    Entry, EvidenceAuthor, Path, SignedWillowEntry,
};

fn signed_at_path(author: &EvidenceAuthor, path: Path, payload: &[u8]) -> SignedWillowEntry {
    let entry = Entry::builder()
        .namespace_id(author.namespace_id().clone())
        .subspace_id(author.subspace_id())
        .path(path)
        .timestamp(100u64)
        .payload(payload)
        .build();
    sign_entry(author, entry, payload)
}

/// Signs an already-built `Entry` — used where the entry's path must come from
/// a real builder (`build_alert_entry`) rather than a literal path.
fn sign_entry(author: &EvidenceAuthor, entry: Entry, payload: &[u8]) -> SignedWillowEntry {
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

fn expect_unsupported_schema(entry: SignedWillowEntry) {
    match encode_bundle(std::slice::from_ref(&entry)) {
        Err(BundleEncodeError::InvalidItem(diagnostic)) => {
            assert_eq!(diagnostic.component, ItemComponent::Schema);
            assert_eq!(diagnostic.code, DiagnosticCode::UnsupportedSchema);
        }
        other => panic!("expected InvalidItem(UnsupportedSchema), got {other:?}"),
    }
}

/// Inspect + plan + commit one signed entry, asserting it survives every
/// stage — the full pipeline a synced profile entry must pass.
fn commit_entry(store: &EvidenceStore, signed: &SignedWillowEntry) {
    let bundle_bytes = encode_bundle(std::slice::from_ref(signed)).expect("encode bundle");
    let preview = match store
        .inspect(&bundle_bytes, ImportContext::new("test-route"))
        .expect("inspect")
    {
        InspectOutcome::Preview(p) => p,
        InspectOutcome::Rejected(r) => panic!("rejected: {r:?}"),
    };
    let plan = preview.plan_all().expect("plan_all");
    match plan.commit().expect("commit") {
        CommitOutcome::Committed(_) => {}
        CommitOutcome::NoChanges(_) => panic!("entry was silently dropped, not committed"),
    }
}

fn sample_card(display_name: &str) -> ProfileCard {
    ProfileCard {
        display_name: display_name.to_string(),
    }
}

fn sample_alert_payload(object_id: &[u8; 16], revision_id: &[u8; 16], headline: &str) -> Vec<u8> {
    encode_alert(&AlertPayload {
        object_id: *object_id,
        revision_id: *revision_id,
        created_at: 100,
        valid_from: None,
        expires_at: 200,
        language: "en".to_string(),
        urgency: Urgency::Immediate,
        severity: Severity::Severe,
        certainty: Certainty::Observed,
        headline: headline.to_string(),
        description: "Alert fixture.".to_string(),
        affected_area_claim: None,
        source_claims: vec!["test fixture".to_string()],
        ai_assisted: false,
    })
    .expect("encode alert")
}

#[test]
fn valid_profile_card_at_own_slot_commits_and_retains_its_payload() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let author = generate_communal_author().expect("author");

    let card = sample_card("Ana");
    let payload = encode_profile_card(&card).expect("encode card");
    let path = profile_card_path(author.subspace_id().as_bytes()).expect("path");
    commit_entry(&store, &signed_at_path(&author, path, &payload));

    assert_eq!(store.live_count().expect("live_count"), 1);

    // The payload must be retained and readable back from the live view: the
    // resolver reads display names out of `profile/` entries.
    let prefix = profile_prefix().expect("prefix");
    let matches = store.entries_with_prefix(&prefix).expect("query");
    assert_eq!(matches.len(), 1);
    let stored_payload = matches[0]
        .2
        .as_deref()
        .expect("profile payload must be retained with the live entry");
    assert_eq!(stored_payload, payload.as_slice());
    assert_eq!(
        decode_profile_card(stored_payload).expect("decode card"),
        card
    );
}

#[test]
fn garbage_payload_at_profile_slot_is_rejected_as_unsupported_schema() {
    let author = generate_communal_author().expect("author");
    let path = profile_card_path(author.subspace_id().as_bytes()).expect("path");
    expect_unsupported_schema(signed_at_path(&author, path, b"not-cbor"));
}

#[test]
fn profile_written_into_someone_elses_slot_is_rejected_at_inspect() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let attacker = generate_communal_author().expect("author");

    let card = sample_card("Ana");
    let payload = encode_profile_card(&card).expect("encode card");

    // First prove the author CAN write their OWN slot — otherwise the spoof
    // assertion below could pass vacuously (e.g. if profile entries were
    // simply never admitted at all).
    let own_slot = profile_card_path(attacker.subspace_id().as_bytes()).expect("path");
    commit_entry(&store, &signed_at_path(&attacker, own_slot, &payload));
    assert_eq!(store.live_count().expect("live_count"), 1);

    // The same valid card aimed at ANOTHER subspace's card slot passes the
    // schema gate (the payload is a canonical card) but must be rejected by
    // inspect's slot binding — the same rejection surface the alert
    // path-binding tests assert: the entry is simply not eligible.
    let victim_subspace = [9u8; 32];
    assert_ne!(&victim_subspace, attacker.subspace_id().as_bytes());
    let victim_slot = profile_card_path(&victim_subspace).expect("path");
    let spoofed = signed_at_path(&attacker, victim_slot, &payload);

    // It must ENCODE fine: that is what proves verify_frame passed it and the
    // rejection below really comes from inspect's binding gate.
    let bundle_bytes = encode_bundle(std::slice::from_ref(&spoofed)).expect("encode bundle");
    let preview = match store
        .inspect(&bundle_bytes, ImportContext::new("test-route"))
        .expect("inspect")
    {
        InspectOutcome::Preview(p) => p,
        InspectOutcome::Rejected(r) => panic!("expected a preview, got rejection: {r:?}"),
    };
    assert_eq!(
        preview.eligible_count().expect("eligible_count"),
        0,
        "a card signed by one subspace but addressed to another's slot must not be eligible"
    );
    assert_eq!(store.live_count().expect("live_count"), 1);
}

#[test]
fn malformed_profile_path_does_not_fall_through_to_alert_schema() {
    let author = generate_communal_author().expect("author");
    // `profile/<32B>/avatar`: the classifier returns None, but the path IS
    // under the reserved `profile/` prefix. Even a perfectly valid canonical
    // alert payload must not rescue it — the strongest witness that the
    // reserved prefix cannot fall through to the alert schema check.
    let payload = sample_alert_payload(
        &[1u8; 16],
        &[2u8; 16],
        "Valid alert bytes must not rescue a profile path.",
    );
    let path = Path::from_slices(&[
        PROFILE_COMPONENT,
        author.subspace_id().as_bytes().as_slice(),
        b"avatar".as_slice(),
    ])
    .expect("path");
    expect_unsupported_schema(signed_at_path(&author, path, &payload));
}

#[test]
fn profile_path_with_extra_components_is_rejected() {
    let author = generate_communal_author().expect("author");
    // Even a valid card payload is rejected when the path carries trailing
    // components the classifier does not recognize.
    let payload = encode_profile_card(&sample_card("Ana")).expect("encode card");
    let path = Path::from_slices(&[
        PROFILE_COMPONENT,
        author.subspace_id().as_bytes().as_slice(),
        b"card".as_slice(),
        b"extra".as_slice(),
    ])
    .expect("path");
    expect_unsupported_schema(signed_at_path(&author, path, &payload));
}

#[test]
fn alerts_and_app_entries_are_unaffected() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let author = generate_communal_author().expect("author");

    // A normal alert still commits: its path binds to its payload, and the
    // profile arm must not have shadowed the alert schema/binding checks.
    let object_id = [3u8; 16];
    let revision_id = [4u8; 16];
    // The payload must claim the same ids the path binds, or inspect's alert
    // path-binding gate (correctly) rejects it.
    let alert_payload =
        sample_alert_payload(&object_id, &revision_id, "Ordinary alert still commits.");
    let alert_entry =
        build_alert_entry(&author, &object_id, &revision_id, 100, &alert_payload).expect("entry");
    commit_entry(&store, &sign_entry(&author, alert_entry, &alert_payload));
    assert_eq!(store.live_count().expect("live_count"), 1);

    // A normal app-data entry (opaque payload) still commits too.
    let app_path = app_data_path(&[7u8; 32], "items/a").expect("app data path");
    let app_payload = b"{\"done\":false}";
    commit_entry(&store, &signed_at_path(&author, app_path, app_payload));
    assert_eq!(store.live_count().expect("live_count"), 2);
}
