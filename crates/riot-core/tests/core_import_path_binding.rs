//! Debt-audit evidence: `EvidenceStore::inspect()` must reject a validly
//! signed entry whose Willow path does not bind to its own payload's
//! `object_id`/`revision_id`. `alert_entry_path_matches_payload` existed
//! (added in `58b50cd`) but was only wired into the FFI's read-only
//! `inspectable_alert_entries` listing helper, never into the actual
//! commit-capable import path -- so a mismatched entry was importable and
//! committable through the core session API.

use riot_core::import::encode_bundle;
use riot_core::model::{AlertPayload, Certainty, Severity, Urgency};
use riot_core::session::{ImportContext, RiotSession};
use riot_core::willow::{
    authorise_entry, build_alert_entry, encode_capability, encode_entry, EvidenceAuthor,
    SignedWillowEntry,
};

fn author() -> EvidenceAuthor {
    use willow25::entry::NamespaceSecret;
    let mut seed = *b"riot-pth-namespace-secret-00001!";
    let ns = loop {
        let c = NamespaceSecret::from_bytes(&seed).corresponding_namespace_id();
        if c.is_communal() {
            break c;
        }
        seed[0] = seed[0].wrapping_add(1);
    };
    EvidenceAuthor::from_parts_for_tests(ns, b"riot-pth-subspace-secret-000001!")
}

/// Signs an entry whose Willow path is bound to `path_object`/`path_revision`
/// but whose payload claims `payload_object`/`payload_revision` -- the exact
/// shape a hostile or buggy peer could sign to desynchronize an entry's
/// on-the-wire identity from the content a renderer/importer trusts it to
/// describe.
fn signed_with_mismatch(
    author: &EvidenceAuthor,
    path_object: &[u8; 16],
    path_revision: &[u8; 16],
    payload_object: &[u8; 16],
    payload_revision: &[u8; 16],
) -> SignedWillowEntry {
    let payload = riot_core::model::encode_alert(&AlertPayload {
        object_id: *payload_object,
        revision_id: *payload_revision,
        created_at: 1_000,
        valid_from: None,
        expires_at: 2_000,
        language: "en".into(),
        urgency: Urgency::Immediate,
        severity: Severity::Severe,
        certainty: Certainty::Observed,
        headline: "path binding fixture".into(),
        description: "Path binding fixture.".into(),
        affected_area_claim: None,
        source_claims: vec!["fixture".into()],
        ai_assisted: false,
    })
    .expect("payload");
    let entry =
        build_alert_entry(author, path_object, path_revision, 100, &payload).expect("entry");
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
    ImportContext::new("path-binding-route")
}

#[test]
fn core_import_path_binding_rejects_entry_whose_path_does_not_match_its_payload() {
    let a = author();
    let session = RiotSession::open().unwrap();
    let store = session.create_store().unwrap();
    let mismatched = signed_with_mismatch(&a, &[1; 16], &[1; 16], &[2; 16], &[2; 16]);

    let preview = store
        .inspect(&bundle(&[mismatched]), ctx())
        .unwrap()
        .expect_preview();

    assert_eq!(
        preview.eligible_count().unwrap(),
        0,
        "an entry whose path doesn't bind to its payload must not be eligible to import"
    );
}

#[test]
fn core_import_path_binding_matched_entry_alongside_a_mismatched_one_is_still_eligible() {
    let a = author();
    let session = RiotSession::open().unwrap();
    let store = session.create_store().unwrap();
    let matched = signed_with_mismatch(&a, &[3; 16], &[3; 16], &[3; 16], &[3; 16]);
    let mismatched = signed_with_mismatch(&a, &[4; 16], &[4; 16], &[5; 16], &[5; 16]);

    let preview = store
        .inspect(&bundle(&[matched, mismatched]), ctx())
        .unwrap()
        .expect_preview();

    assert_eq!(
        preview.eligible_count().unwrap(),
        1,
        "a per-item failure must not hide a valid sibling in the same bundle"
    );
}
