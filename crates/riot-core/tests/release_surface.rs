//! Release-API containment: this test compiles WITHOUT the `conformance`
//! feature (no `required-features`), so it exercises exactly the surface a
//! release consumer like `riot-ffi` sees. It proves the production factories
//! exist and take no injectable sources. The *absence* of the injection
//! surface is proven by the compile-fail expectations documented below and
//! enforced structurally by `cargo xtask validate-contracts`
//! (`check_resolved_feature_graph` rejects `riot-core feature "conformance"`
//! in the riot-ffi closure).

use riot_core::willow::{create_signed_alert, generate_communal_author, system_snapshot};

#[test]
fn production_factories_take_no_injectable_sources() {
    // Production author generation: no entropy argument.
    let author = generate_communal_author().expect("os entropy");
    assert!(author.namespace_id().is_communal());

    // Production clock: no injectable source.
    let snapshot = system_snapshot().expect("system clock");
    assert!(snapshot.unix_seconds > 0);

    // Production signed-alert factory: (author, draft) only — no entropy or
    // clock parameters exist on this path.
    use riot_core::model::{Certainty, Severity, Urgency};
    use riot_core::willow::AlertDraft;
    let draft = AlertDraft {
        valid_from: None,
        expires_at: snapshot.unix_seconds + 3600,
        language: "en".into(),
        urgency: Urgency::Immediate,
        severity: Severity::Severe,
        certainty: Certainty::Observed,
        headline: "Release-surface alert".into(),
        description: "Created via the non-injectable production factory.".into(),
        affected_area_claim: None,
        source_claims: vec!["release-surface test".into()],
        ai_assisted: false,
    };
    let signed = create_signed_alert(&author, draft).expect("signs");
    assert_eq!(signed.signed.signature.len(), 64);
    assert_eq!(signed.payload.created_at, signed.snapshot.unix_seconds);
}

// The following must NOT compile in a release (no-conformance) build; each is
// gated behind `#[cfg(feature = "conformance")]`. Kept as documentation of the
// containment boundary (uncomment under `--features conformance` to confirm
// they resolve there):
//
//   riot_core::willow::EntropySource
//   riot_core::willow::ClockSource
//   riot_core::willow::OsEntropy
//   riot_core::willow::SystemClock
//   riot_core::willow::snapshot_from_unix_seconds(..)
//   riot_core::willow::generate_communal_author_with(..)
//   riot_core::willow::create_signed_alert_with(..)
//   riot_core::willow::EvidenceAuthor::from_parts_for_tests(..)
