//! What happens when a peer offers us entries we cannot admit.
//!
//! The wire layer and the admission layer check different things, and the gap
//! between them is the subject of this file. `ReconcileSession` will hand up an
//! `ImportBundle` for any bundle whose items are cryptographically valid, sit in
//! our namespace, and are exactly the ids we asked for. It does **not** check
//! that an alert's signed path matches its payload, and it does not know our
//! inventory ceiling. Both of those are enforced afterwards, in
//! `prepare_sync_import`.
//!
//! So a peer really can get a bundle all the way to admission and have it
//! refused there. When that happens the session must not simply fall over: it
//! sends the peer a `Reject` frame carrying a code that says *why* (1 = we would
//! not admit these bytes, 2 = we do not have room), ends the exchange, and
//! leaves our store exactly as it was. A half-applied import, or a silent
//! truncation of what the peer offered, would both be worse than refusing.
//!
//! The peer here is crafted at the wire level rather than being a second
//! `MobileProfile`, because an honest profile cannot construct these offers —
//! which is the point: this is what a buggy or hostile peer sends.

use riot_core::import::encode_bundle;
use riot_core::model::{encode_alert, AlertPayload, Certainty, Severity, Urgency};
use riot_core::sync::{decode_frame, encode_frame, SyncFrame};
use riot_core::willow::{
    authorise_entry, build_alert_entry, encode_capability, encode_entry, entry_id,
    generate_communal_author_for_namespace, EvidenceAuthor, SignedWillowEntry,
};
use riot_ffi::{
    open_local_profile, AlertCertainty, AlertDraftInput, AlertSeverity, AlertUrgency,
    MobileProfile, MobileSyncSession, SyncOutcomeKind,
};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

fn expires_later() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after unix epoch")
        .as_secs()
        + 3_600
}

fn draft() -> AlertDraftInput {
    AlertDraftInput {
        valid_from: None,
        expires_at: expires_later(),
        language: "en".into(),
        urgency: AlertUrgency::Immediate,
        severity: AlertSeverity::Severe,
        certainty: AlertCertainty::Observed,
        headline: "Our own alert".into(),
        description: "The one entry this profile already holds.".into(),
        affected_area_claim: None,
        source_claims: vec!["Field observer".into()],
        ai_assisted: false,
    }
}

fn namespace_bytes(hex: &str) -> [u8; 32] {
    let mut out = [0_u8; 32];
    for (index, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).expect("hex namespace");
    }
    out
}

/// A receiver holding exactly one live entry of its own, and the namespace of
/// the space it is in.
fn receiver() -> (Arc<MobileProfile>, [u8; 32]) {
    let profile = open_local_profile().expect("profile");
    let space = profile
        .create_public_space("Bounded incident".into())
        .expect("space");
    let record = profile.create_draft_alert(draft()).expect("draft");
    profile.sign_draft(record.draft_id).expect("sign");
    let namespace_id = namespace_bytes(&space.namespace_id);
    (profile, namespace_id)
}

/// An alert payload naming `object` as the object it is about.
fn alert_payload(object: u8, headline: &str) -> Vec<u8> {
    encode_alert(&AlertPayload {
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
        source_claims: vec!["a peer".into()],
        ai_assisted: false,
    })
    .expect("encode alert")
}

/// A signed alert whose *path* names `path_object` while its *payload* names
/// `payload_object`. Passing the same value for both produces an honest entry;
/// passing different values produces one whose signed path does not match what
/// it says it is about — valid crypto, dishonest binding.
fn signed_alert(
    author: &EvidenceAuthor,
    path_object: u8,
    payload_object: u8,
    headline: &str,
) -> SignedWillowEntry {
    let payload = alert_payload(payload_object, headline);
    let entry = build_alert_entry(author, &[path_object; 16], &[2; 16], 100, &payload)
        .expect("build alert entry");
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

/// A signed app-data entry. App data carries an opaque payload rather than an
/// alert, so it is the only way a peer can legally offer us a megabyte at a time.
fn signed_app_data(
    author: &EvidenceAuthor,
    app_id: &[u8; 32],
    key: &str,
    value: &[u8],
    timestamp: u64,
) -> SignedWillowEntry {
    let entry = riot_core::willow::Entry::builder()
        .namespace_id(author.namespace_id().clone())
        .subspace_id(author.subspace_id())
        .path(riot_core::apps::entry::app_data_path(app_id, key).expect("app data path"))
        .timestamp(timestamp)
        .payload(value)
        .build();
    let authorised = authorise_entry(author, entry).expect("authorise");
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    SignedWillowEntry {
        entry_bytes: encode_entry(authorised.entry()),
        capability_bytes: encode_capability(token.capability()),
        signature: signature.to_bytes(),
        payload_bytes: value.to_vec(),
    }
}

/// Drives the receiver's session, playing the peer at the wire level, until the
/// receiver has been handed `offered` as an import bundle. Returns the outcome
/// of that final `receive_frame`.
///
/// This is the honest protocol dance — Hello, Summary, Request, Entries — so the
/// bundle arrives through exactly the door a real peer's bundle arrives through.
fn offer_entries(
    session: &MobileSyncSession,
    namespace_id: [u8; 32],
    offered: &[SignedWillowEntry],
) -> riot_ffi::SyncOutcome {
    // The receiver opens with Hello.
    let outcome = session.begin().expect("begin");
    assert_eq!(outcome.kind, SyncOutcomeKind::FrameReady);
    let hello = session
        .take_outbound_frame()
        .expect("take hello")
        .expect("begin queues a frame");
    assert!(matches!(
        decode_frame(&hello).expect("decode hello"),
        SyncFrame::Hello { .. }
    ));

    // The peer summarises what it holds. Ids must be canonically ordered.
    let mut ids: Vec<[u8; 32]> = offered
        .iter()
        .map(|signed| entry_id(&signed.entry_bytes))
        .collect();
    ids.sort_unstable();
    let summary = encode_frame(&SyncFrame::Summary {
        namespace_id,
        entry_ids: ids.clone(),
    })
    .expect("encode summary");

    // The receiver asks for everything it lacks — which is all of it.
    let outcome = session.receive_frame(summary).expect("receive summary");
    assert_eq!(outcome.kind, SyncOutcomeKind::FrameReady);
    let request = session
        .take_outbound_frame()
        .expect("take request")
        .expect("a request is queued");
    match decode_frame(&request).expect("decode request") {
        SyncFrame::Request { entry_ids, .. } => assert_eq!(
            entry_ids, ids,
            "the receiver must ask for exactly what it lacks"
        ),
        other => panic!("expected a request, got {other:?}"),
    }

    // The peer sends them. The bundle's items must be in requested-id order.
    let mut sorted = offered.to_vec();
    sorted.sort_by_key(|signed| entry_id(&signed.entry_bytes));
    let bundle_bytes = encode_bundle(&sorted).expect("encode bundle");
    let entries = encode_frame(&SyncFrame::Entries {
        namespace_id,
        bundle_bytes,
    })
    .expect("encode entries frame");

    session.receive_frame(entries).expect("receive entries")
}

/// Asserts the session ended by telling the peer `code` and nothing else moved.
fn assert_rejected_with(
    profile: &MobileProfile,
    session: &MobileSyncSession,
    outcome: &riot_ffi::SyncOutcome,
    code: u8,
) {
    // The refusal is a frame *for the peer*, so the local outcome is "there is a
    // frame to send", and the exchange is over.
    assert_eq!(outcome.kind, SyncOutcomeKind::FrameReady);
    assert!(outcome.terminal, "a refused import ends the exchange");
    assert!(
        outcome.import_bundle_bytes.is_none(),
        "a refused import must not hand its bytes to the caller to persist"
    );

    let frame = session
        .take_outbound_frame()
        .expect("take reject")
        .expect("a reject frame is queued for the peer");
    match decode_frame(&frame).expect("decode reject") {
        SyncFrame::Reject {
            code: sent_code, ..
        } => assert_eq!(
            sent_code, code,
            "the peer must be told which kind of refusal this was"
        ),
        other => panic!("expected a reject frame, got {other:?}"),
    }

    // Nothing the peer offered was admitted: the profile still holds only the
    // one entry it signed itself.
    let entries = profile.list_current_entries().expect("listing");
    assert_eq!(
        entries.len(),
        1,
        "a refused import must leave the store exactly as it was"
    );
    assert_eq!(entries[0].headline, "Our own alert");
}

/// A peer offers an alert whose signed path says it is about one object while
/// its payload says another. The crypto is perfect and the wire layer passes it
/// straight through — it is admission that catches the lie.
///
/// Code 1: we will not admit these bytes. Not code 2 — this is not about room.
#[test]
fn a_peer_offering_an_alert_whose_path_belies_its_payload_is_refused_with_code_one() {
    let (profile, namespace_id) = receiver();
    // The peer writes from inside our own space — it is a member, not a stranger.
    let peer = generate_communal_author_for_namespace(namespace_id).expect("peer author");

    // Path says object 9; the payload inside says object 1.
    let dishonest = signed_alert(&peer, 9, 1, "A path that does not match its payload");

    let session = profile.open_sync_session().expect("sync session");
    let outcome = offer_entries(&session, namespace_id, std::slice::from_ref(&dishonest));

    assert_rejected_with(&profile, &session, &outcome, 1);
}

/// A peer offers entries that are individually fine and few in number, but whose
/// bytes would push our retained proof set past the inventory ceiling. That is a
/// different failure from a dishonest offer and gets a different code — the peer
/// can tell "I will not admit your bytes" from "I have no room", and only the
/// second is worth retrying against later.
///
/// This ceiling bounds the memory a peer can make us hold, so it must refuse the
/// offer whole rather than admit a truncated prefix of it.
///
/// Note the *count* ceiling cannot get this far: `ReconcileSession` rejects an
/// offer that would take the inventory over `MAX_SYNC_IDS` back at the summary
/// step (`sync/state.rs:179`), long before admission. The byte ceiling is not
/// checked there, which is why it is the one that reaches this code path.
#[test]
fn a_peer_whose_bytes_overflow_the_inventory_ceiling_is_refused_with_code_two() {
    // A profile that already holds several megabytes of its own app data, still
    // comfortably inside the ceiling.
    let profile = open_local_profile().expect("profile");
    let space = profile
        .create_public_space("Bounded incident".into())
        .expect("space");
    let namespace_id = namespace_bytes(&space.namespace_id);
    let record = profile.create_draft_alert(draft()).expect("draft");
    profile.sign_draft(record.draft_id).expect("sign");

    let app_id = "11".repeat(32);
    let runtime = profile.app_runtime();
    for index in 0..6 {
        runtime
            .app_data_put(
                app_id.clone(),
                format!("blob-{index}"),
                vec![b'x'; 1_000_000],
            )
            .unwrap_or_else(|error| panic!("our own app data must fit: {error:?}"));
    }

    // The peer's offer is small in number and each entry is legal, but together
    // with what we already hold it does not fit.
    let peer = generate_communal_author_for_namespace(namespace_id).expect("peer author");
    let app_id_bytes = [0x11_u8; 32];
    let offered: Vec<SignedWillowEntry> = (0..3)
        .map(|index| {
            signed_app_data(
                &peer,
                &app_id_bytes,
                &format!("peer-blob-{index}"),
                &[b'y'; 1_000_000],
                200 + index,
            )
        })
        .collect();

    let session = profile.open_sync_session().expect("sync session");
    let outcome = offer_entries(&session, namespace_id, &offered);

    // The refusal is the resource code, and our own app data is untouched.
    assert_eq!(outcome.kind, SyncOutcomeKind::FrameReady);
    assert!(outcome.terminal);
    assert!(outcome.import_bundle_bytes.is_none());
    let frame = session
        .take_outbound_frame()
        .expect("take reject")
        .expect("a reject frame is queued for the peer");
    match decode_frame(&frame).expect("decode reject") {
        SyncFrame::Reject {
            code: sent_code, ..
        } => assert_eq!(
            sent_code, 2,
            "no-room must be told apart from will-not-admit"
        ),
        other => panic!("expected a reject frame, got {other:?}"),
    }
    assert_eq!(
        runtime
            .app_data_get(app_id, "blob-0".into())
            .expect("read back"),
        Some(vec![b'x'; 1_000_000]),
        "a refused import must leave our own data exactly as it was"
    );
}

/// The control: the very same protocol dance with an honest, in-bounds offer is
/// *not* refused — it reaches the human review step. Without this, the two tests
/// above would pass just as well against a session that refused everything.
#[test]
fn an_honest_offer_reaches_review_rather_than_being_refused() {
    let (profile, namespace_id) = receiver();
    let peer = generate_communal_author_for_namespace(namespace_id).expect("peer author");
    let honest = signed_alert(&peer, 3, 3, "An honest alert from a peer");

    let session = profile.open_sync_session().expect("sync session");
    let outcome = offer_entries(&session, namespace_id, std::slice::from_ref(&honest));

    assert_eq!(
        outcome.kind,
        SyncOutcomeKind::ReviewImport,
        "an admissible offer must be shown to the person, not auto-refused"
    );
    assert!(!outcome.terminal);
    assert_eq!(outcome.entries.len(), 1);
    assert_eq!(outcome.entries[0].headline, "An honest alert from a peer");
    assert!(
        outcome.import_bundle_bytes.is_some(),
        "the reviewed bytes must be handed back for the host to persist"
    );

    // And accepting it actually admits the entry.
    session.accept_import().expect("accept");
    let entries = profile.list_current_entries().expect("listing");
    assert_eq!(entries.len(), 2, "the reviewed entry is now held");
    assert!(entries
        .iter()
        .any(|entry| entry.headline == "An honest alert from a peer"));
}
