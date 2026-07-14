//! Cross-family admission evidence. Three path families now widen the import
//! boundary — `apps/` (opaque app data), `app-index/` (manifest/bundle/
//! endorsement/trust slots), `profile/` (cards) — each landed by a different
//! session, each with its own test file, none testing the others. This file
//! tests them AGAINST each other: the classifiers must be mutually exclusive,
//! and — the property the gate's own comments claim — a valid alert payload
//! must never rescue a malformed path under a reserved prefix.
//!
//! Requires `conformance`.

use riot_core::apps::entry::is_app_data_path;
use riot_core::apps::index::classify_app_index_path;
use riot_core::import::{encode_bundle, BundleEncodeError, DiagnosticCode, ItemComponent};
use riot_core::model::{AlertPayload, Certainty, Severity, Urgency};
use riot_core::profile::path::classify_profile_path;
use riot_core::willow::{
    authorise_entry, encode_capability, encode_entry, generate_communal_author, Entry,
    EvidenceAuthor, Path, SignedWillowEntry,
};

fn alert_payload() -> Vec<u8> {
    riot_core::model::encode_alert(&AlertPayload {
        object_id: [4; 16],
        revision_id: [4; 16],
        created_at: 1_000,
        valid_from: None,
        expires_at: 2_000,
        language: "en".into(),
        urgency: Urgency::Immediate,
        severity: Severity::Severe,
        certainty: Certainty::Observed,
        headline: "cross-family fixture".into(),
        description: "A perfectly valid alert payload.".into(),
        affected_area_claim: None,
        source_claims: vec!["fixture".into()],
        ai_assisted: false,
    })
    .expect("payload")
}

fn signed_at_path(author: &EvidenceAuthor, path: Path, payload: &[u8]) -> SignedWillowEntry {
    let entry = Entry::builder()
        .namespace_id(author.namespace_id().clone())
        .subspace_id(author.subspace_id())
        .path(path)
        .timestamp(100u64)
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

fn expect_unsupported_schema(entry: SignedWillowEntry, what: &str) {
    match encode_bundle(std::slice::from_ref(&entry)) {
        Err(BundleEncodeError::InvalidItem(diagnostic)) => {
            assert_eq!(diagnostic.component, ItemComponent::Schema, "{what}");
            assert_eq!(diagnostic.code, DiagnosticCode::UnsupportedSchema, "{what}");
        }
        other => panic!("{what}: expected InvalidItem(UnsupportedSchema), got {other:?}"),
    }
}

/// The three classifiers must never both claim the same path: a family's
/// admission rules are only sound if no other family can also be applied.
#[test]
fn the_three_path_families_never_claim_the_same_path() {
    let app_id = [7u8; 32];
    let subspace = [8u8; 32];

    let app_data = riot_core::apps::entry::app_data_path(&app_id, "items/a").expect("app data");
    let app_index = riot_core::apps::index::app_index_manifest_path(&app_id).expect("manifest");
    let profile = riot_core::profile::path::profile_card_path(&subspace).expect("card");

    assert!(is_app_data_path(&app_data));
    assert!(classify_app_index_path(&app_data).is_none());
    assert!(classify_profile_path(&app_data).is_none());

    assert!(!is_app_data_path(&app_index));
    assert!(classify_app_index_path(&app_index).is_some());
    assert!(classify_profile_path(&app_index).is_none());

    assert!(!is_app_data_path(&profile));
    assert!(classify_app_index_path(&profile).is_none());
    assert!(classify_profile_path(&profile).is_some());
}

/// The property `verify_frame`'s own comments claim for `app-index/` and
/// `profile/`: a malformed path under a RESERVED prefix must be refused
/// outright, so a valid alert payload can never rescue it into the store.
/// `apps/` is a reserved prefix on exactly the same footing and must behave
/// the same way.
#[test]
fn a_valid_alert_payload_cannot_rescue_a_malformed_reserved_path() {
    let author = generate_communal_author().expect("author");
    let payload = alert_payload();
    let short_id = [7u8; 16]; // not a 32-byte app/subspace id: malformed everywhere

    // `app-index/` — the gate already refuses this.
    let malformed_index =
        Path::from_slices(&[b"app-index".as_slice(), &short_id, b"manifest".as_slice()])
            .expect("path");
    expect_unsupported_schema(
        signed_at_path(&author, malformed_index, &payload),
        "malformed app-index path with a valid alert payload",
    );

    // `profile/` — the gate already refuses this.
    let malformed_profile =
        Path::from_slices(&[b"profile".as_slice(), &short_id, b"card".as_slice()]).expect("path");
    expect_unsupported_schema(
        signed_at_path(&author, malformed_profile, &payload),
        "malformed profile path with a valid alert payload",
    );

    // `apps/` — same reserved prefix, same rule. A path that is not a valid
    // app-data path must not be admitted just because the bytes underneath it
    // happen to decode as an alert: it would land an "alert" at a path no
    // alert can own, inside the app-data subtree.
    let malformed_app_data =
        Path::from_slices(&[b"apps".as_slice(), &short_id, b"items".as_slice()]).expect("path");
    expect_unsupported_schema(
        signed_at_path(&author, malformed_app_data, &payload),
        "malformed apps path with a valid alert payload",
    );
}

/// The reserved prefixes are reserved even when nothing follows them.
#[test]
fn a_bare_reserved_prefix_is_never_admissible() {
    let author = generate_communal_author().expect("author");
    let payload = alert_payload();

    for prefix in [
        b"apps".as_slice(),
        b"app-index".as_slice(),
        b"profile".as_slice(),
    ] {
        let path = Path::from_slices(&[prefix]).expect("path");
        expect_unsupported_schema(
            signed_at_path(&author, path, &payload),
            "a bare reserved prefix carrying a valid alert payload",
        );
    }
}

/// An ordinary path outside every reserved prefix still takes the alert rules
/// — the isolation above must not have broken the common case.
#[test]
fn an_alert_outside_the_reserved_prefixes_is_still_admitted_on_its_own_rules() {
    let author = generate_communal_author().expect("author");
    let payload = alert_payload();

    // The real alert path binds to the payload's ids; that pair is admissible.
    let entry = riot_core::willow::build_alert_entry(&author, &[4; 16], &[4; 16], 100, &payload)
        .expect("entry");
    let authorised = authorise_entry(&author, entry).expect("authorise");
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    let signed = SignedWillowEntry {
        entry_bytes: encode_entry(authorised.entry()),
        capability_bytes: encode_capability(token.capability()),
        signature: signature.to_bytes(),
        payload_bytes: payload.clone(),
    };
    assert!(encode_bundle(std::slice::from_ref(&signed)).is_ok());

    // And a non-reserved path with a non-alert payload is still refused.
    let notes = Path::from_slices(&[b"notes".as_slice(), b"x".as_slice()]).expect("path");
    expect_unsupported_schema(
        signed_at_path(&author, notes, b"{}"),
        "an opaque payload outside every reserved prefix",
    );
}
