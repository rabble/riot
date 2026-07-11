//! Debt-audit evidence: `fixtures/manifest.json` defines `path_components`
//! (64), `path_component_bytes` (256), and `path_total_bytes` (2048), but
//! nothing enforced them at runtime -- `willow25`'s own `Entry`/`Path`
//! bounds (MCL=MCC=MPL=4096, hardcoded in the crate) are far looser, so a
//! validly signed entry could carry an oversized/malformed path with
//! nothing rejecting it. Each test below constructs a path that violates
//! exactly one riot-core ceiling while staying comfortably under willow25's
//! own looser bounds, so `Path::from_slices` itself does not refuse
//! construction -- isolating what riot-core's own decoder must catch.

use riot_core::import::{
    decode_bundle, BundleDecodeOutcome, DiagnosticCode, ItemComponent, ItemStatus,
};
use riot_core::model::{AlertPayload, Certainty, Severity, Urgency};
use riot_core::willow::{
    authorise_entry, encode_capability, encode_entry, Entry, EvidenceAuthor, Path,
    SignedWillowEntry,
};

fn author() -> EvidenceAuthor {
    use willow25::entry::NamespaceSecret;
    let mut seed = *b"riot-pth-namespace-secret-00002!";
    let ns = loop {
        let c = NamespaceSecret::from_bytes(&seed).corresponding_namespace_id();
        if c.is_communal() {
            break c;
        }
        seed[0] = seed[0].wrapping_add(1);
    };
    EvidenceAuthor::from_parts_for_tests(ns, b"riot-pth-subspace-secret-000002!")
}

/// Signs an entry at an arbitrary `path`, bypassing the fixed alert path
/// shape -- the shape a hostile or buggy peer's signer could produce.
fn signed_at_path(author: &EvidenceAuthor, path: Path) -> SignedWillowEntry {
    let payload = riot_core::model::encode_alert(&AlertPayload {
        object_id: [9; 16],
        revision_id: [9; 16],
        created_at: 1_000,
        valid_from: None,
        expires_at: 2_000,
        language: "en".into(),
        urgency: Urgency::Immediate,
        severity: Severity::Severe,
        certainty: Certainty::Observed,
        headline: "path size fixture".into(),
        description: "Path size fixture.".into(),
        affected_area_claim: None,
        source_claims: vec!["fixture".into()],
        ai_assisted: false,
    })
    .expect("payload");
    let entry = Entry::builder()
        .namespace_id(author.namespace_id().clone())
        .subspace_id(author.subspace_id())
        .path(path)
        .timestamp(100u64)
        .payload(&payload)
        .build();
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

/// `encode_bundle` re-verifies every item and refuses to export bytes Riot
/// would itself reject (`BundleEncodeError::InvalidItem`), so a hostile
/// path shape is caught right there -- the same `verify_frame` check a
/// hostile bundle from an untrusted peer would hit at `decode_bundle` time.
fn expect_path_bounds_rejected(entry: SignedWillowEntry) {
    let result = riot_core::import::encode_bundle(&[entry]);
    match result {
        Err(riot_core::import::BundleEncodeError::InvalidItem(diagnostic)) => {
            assert_eq!(diagnostic.component, ItemComponent::Entry);
            assert_eq!(diagnostic.code, DiagnosticCode::PathBoundsExceeded);
        }
        other => panic!("expected InvalidItem(PathBoundsExceeded), got {other:?}"),
    }
}

#[test]
fn core_import_path_size_rejects_over_64_components() {
    // 65 one-byte components: 65 bytes total, well under willow25's own
    // 4096-byte/4096-count bounds, but one over riot-core's 64-component cap.
    let components: Vec<Vec<u8>> = (0u8..65).map(|i| vec![i]).collect();
    let slices: Vec<&[u8]> = components.iter().map(|c| c.as_slice()).collect();
    let path = Path::from_slices(&slices)
        .expect("65 tiny components must build under willow25's own bounds");

    expect_path_bounds_rejected(signed_at_path(&author(), path));
}

#[test]
fn core_import_path_size_rejects_component_over_256_bytes() {
    // One 300-byte component: under willow25's own 4096-byte component cap,
    // but over riot-core's 256-byte-per-component cap.
    let big_component = vec![7u8; 300];
    let path = Path::from_slices(&[&big_component])
        .expect("a 300-byte component must build under willow25's own bounds");

    expect_path_bounds_rejected(signed_at_path(&author(), path));
}

#[test]
fn core_import_path_size_rejects_total_over_2048_bytes() {
    // Nine 250-byte components: 2250 bytes total, over riot-core's 2048-byte
    // total cap, while each individual component (250) stays under the
    // 256-byte per-component cap and the count (9) stays under 64 -- this
    // isolates the total-bytes ceiling from the other two.
    let components: Vec<Vec<u8>> = (0..9u8).map(|i| vec![i; 250]).collect();
    let slices: Vec<&[u8]> = components.iter().map(|c| c.as_slice()).collect();
    let path = Path::from_slices(&slices)
        .expect("nine 250-byte components must build under willow25's own bounds");

    expect_path_bounds_rejected(signed_at_path(&author(), path));
}

#[test]
fn core_import_path_size_accepts_a_path_within_all_three_ceilings() {
    // Sanity check: the ordinary 4-component alert path (well within all
    // three ceilings) must still be accepted -- this fix must not reject
    // legitimate entries.
    let path = riot_core::willow::alert_path(&[1; 16], &[1; 16]).expect("alert path");
    let entry = signed_at_path(&author(), path);
    let bytes = riot_core::import::encode_bundle(&[entry]).expect("encode bundle");
    let decoded = match decode_bundle(&bytes) {
        BundleDecodeOutcome::Decoded(decoded) => decoded,
        BundleDecodeOutcome::Rejected(rejection) => panic!("unexpected rejection: {rejection:?}"),
    };
    assert!(
        matches!(decoded.items[0].status, ItemStatus::Valid(_)),
        "an ordinary in-bounds alert path must not be flagged: {:?}",
        decoded.items[0].status
    );
}
