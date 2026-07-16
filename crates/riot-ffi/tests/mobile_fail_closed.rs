//! Fail-closed contract for the mobile FFI surface.
//!
//! Every test here provokes a REAL refusal condition on the public UniFFI
//! surface — a sync session left open while a mutating call runs, a bundle
//! that does not decode, a namespace that is not communal, a cap exceeded — and
//! asserts the exact `MobileError` variant returned, and (where cheap) that the
//! refusal left profile state unchanged. These are the deny-closed branches of
//! `mobile_state.rs`, plus the stable error-code contract the native apps read
//! off `MobileError`'s `Display`.
//!
//! Companion to `mobile_contract.rs` (the happy-path contract); helpers mirror
//! that file's shape without editing it.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use riot_ffi::{
    open_local_profile, open_local_profile_with_database, AlertCertainty, AlertDraftInput,
    AlertSeverity, AlertUrgency, MobileError, MobileProfile, PublicSpace, SignedAlert,
};

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
        headline: "Ferry terminal access restricted".into(),
        description: "Use the north entrance; the south pier is closed.".into(),
        affected_area_claim: None,
        source_claims: vec!["Two field observers".into()],
        ai_assisted: false,
    }
}

fn profile_with_space() -> Arc<MobileProfile> {
    let profile = open_local_profile().expect("profile");
    profile
        .create_public_space("Fail-closed space".into())
        .unwrap();
    profile
}

/// A sender profile with one signed alert, and a receiver joined to the sender's
/// space holding a live import PREVIEW over that alert (so `profile.preview` is
/// `Some`). Returns `(receiver, signed)` for the branch tests that need a
/// preview parked on the profile.
fn receiver_holding_a_preview() -> (Arc<MobileProfile>, SignedAlert) {
    let sender = open_local_profile().unwrap();
    let space = sender.create_public_space("Preview source".into()).unwrap();
    let record = sender.create_draft_alert(draft()).unwrap();
    let signed = sender.sign_draft(record.draft_id).unwrap();

    let receiver = open_local_profile().unwrap();
    receiver.join_public_space(space).unwrap();
    receiver
        .inspect_bytes(signed.bundle_bytes.clone(), "nearby-device".into())
        .expect("preview parked on the receiver");
    (receiver, signed)
}

// ---------------------------------------------------------------------------
// MobileError Display: the stable code contract the native apps read.
// ---------------------------------------------------------------------------

#[test]
fn mobile_error_renders_its_documented_stable_code_for_every_variant() {
    // These strings are a contract: iOS/Android read them off the error. A
    // rename here is a breaking change, so every variant is pinned exactly.
    let cases: &[(MobileError, &str)] = &[
        (MobileError::Internal, "INTERNAL_ERROR"),
        (MobileError::SessionFailed, "SESSION_FAILED"),
        (MobileError::InvalidInput, "INVALID_INPUT"),
        (MobileError::DraftNotFound, "DRAFT_NOT_FOUND"),
        (MobileError::ImportRejected, "IMPORT_REJECTED"),
        (MobileError::StoreFull, "STORE_FULL"),
        (MobileError::SessionLimit, "SESSION_LIMIT"),
        (MobileError::ObjectClosed, "OBJECT_CLOSED"),
        (MobileError::PreviewConsumed, "PREVIEW_CONSUMED"),
        (MobileError::PlanConsumed, "PLAN_CONSUMED"),
        (MobileError::StalePreview, "STALE_PREVIEW"),
        (MobileError::EntropyUnavailable, "ENTROPY_UNAVAILABLE"),
        (MobileError::ClockUnavailable, "CLOCK_UNAVAILABLE"),
        (MobileError::AppRejected, "APP_REJECTED"),
        (MobileError::NotSpaceOrganizer, "NOT_SPACE_ORGANIZER"),
        (
            MobileError::LegacyProfileCannotOrganize,
            "LEGACY_PROFILE_CANNOT_ORGANIZE",
        ),
        (MobileError::Database, "DATABASE_ERROR"),
    ];
    for (variant, code) in cases {
        assert_eq!(&variant.to_string(), code, "stable code for {variant:?}");
    }
}

// ---------------------------------------------------------------------------
// Database open failure -> MobileError::Database (map_database_error).
// ---------------------------------------------------------------------------

#[test]
fn opening_a_database_in_a_nonexistent_directory_is_a_typed_database_error() {
    // A path whose parent directory does not exist cannot be opened; the
    // DatabaseError is mapped to the typed `Database` code, not a generic
    // `Internal`.
    let missing = "/riot-nonexistent-dir-fail-closed/does/not/exist/riot.db";
    assert!(matches!(
        open_local_profile_with_database(missing.into()),
        Err(MobileError::Database)
    ));
}

// ---------------------------------------------------------------------------
// Space lifecycle refusals.
// ---------------------------------------------------------------------------

#[test]
fn create_public_space_rejects_empty_and_oversized_titles_without_listing_one() {
    let profile = open_local_profile().unwrap();
    for bad in ["", "   ", &"t".repeat(513)] {
        assert!(matches!(
            profile.create_public_space(bad.into()),
            Err(MobileError::InvalidInput)
        ));
    }
    // Nothing was listed, so the board cannot yet be read.
    assert!(matches!(
        profile.list_current_entries(),
        Err(MobileError::InvalidInput)
    ));
}

#[test]
fn create_public_space_is_refused_while_a_sync_session_is_open() {
    let profile = profile_with_space();
    let _sync = profile.open_sync_session().unwrap();
    assert!(matches!(
        profile.create_public_space("Second space".into()),
        Err(MobileError::InvalidInput)
    ));
}

#[test]
fn join_public_space_is_refused_while_a_sync_session_is_open() {
    let host = open_local_profile().unwrap();
    let space = host.create_public_space("Joinable".into()).unwrap();

    let joiner = profile_with_space();
    let _sync = joiner.open_sync_session().unwrap();
    assert!(matches!(
        joiner.join_public_space(space),
        Err(MobileError::InvalidInput)
    ));
}

#[test]
fn join_public_space_refuses_a_non_public_or_malformed_space() {
    let profile = open_local_profile().unwrap();
    let not_public = PublicSpace {
        namespace_id: "ab".repeat(32),
        title: "Marked private".into(),
        is_public: false,
    };
    assert!(matches!(
        profile.join_public_space(not_public),
        Err(MobileError::InvalidInput)
    ));
    // The refusal listed nothing, so the board still reads empty.
    assert!(matches!(
        profile.list_current_entries(),
        Err(MobileError::InvalidInput)
    ));
}

#[test]
fn join_public_space_refuses_a_non_communal_namespace() {
    // A namespace whose public key is non-communal (odd last byte) has no
    // communal-author derivation, so joining it must fail closed. This drives
    // `generate_communal_author_for_namespace` to `NamespaceNotCommunal`, which
    // maps to `InvalidInput`.
    let non_communal = format!("{}01", "00".repeat(31));
    assert_eq!(non_communal.len(), 64);
    let space = PublicSpace {
        namespace_id: non_communal,
        title: "Not communal".into(),
        is_public: true,
    };
    let profile = open_local_profile().unwrap();
    assert!(matches!(
        profile.join_public_space(space),
        Err(MobileError::InvalidInput)
    ));
}

// ---------------------------------------------------------------------------
// Draft creation refusals + the FFI->core enum conversions.
// ---------------------------------------------------------------------------

#[test]
fn create_draft_alert_is_refused_before_any_space_is_listed() {
    let profile = open_local_profile().unwrap();
    assert!(matches!(
        profile.create_draft_alert(draft()),
        Err(MobileError::InvalidInput)
    ));
}

#[test]
fn every_alert_enum_variant_converts_and_produces_a_valid_draft() {
    // Exercises every arm of urgency_from_ffi / severity_from_ffi /
    // certainty_from_ffi. The conversion runs while the draft record is built,
    // and each variant yields a draft the core codec accepts.
    let profile = profile_with_space();
    let urgencies = [
        AlertUrgency::Immediate,
        AlertUrgency::Expected,
        AlertUrgency::Future,
        AlertUrgency::Past,
        AlertUrgency::Unknown,
    ];
    let severities = [
        AlertSeverity::Extreme,
        AlertSeverity::Severe,
        AlertSeverity::Moderate,
        AlertSeverity::Minor,
        AlertSeverity::Unknown,
    ];
    let certainties = [
        AlertCertainty::Observed,
        AlertCertainty::Likely,
        AlertCertainty::Possible,
        AlertCertainty::Unlikely,
        AlertCertainty::Unknown,
    ];
    for urgency in urgencies {
        let mut input = draft();
        input.urgency = urgency;
        assert!(profile.create_draft_alert(input).is_ok());
    }
    for severity in severities {
        let mut input = draft();
        input.severity = severity;
        assert!(profile.create_draft_alert(input).is_ok());
    }
    for certainty in certainties {
        let mut input = draft();
        input.certainty = certainty;
        assert!(profile.create_draft_alert(input).is_ok());
    }
}

// ---------------------------------------------------------------------------
// inspect_bytes refusals.
// ---------------------------------------------------------------------------

#[test]
fn inspect_bytes_is_refused_while_a_sync_session_is_open() {
    let profile = profile_with_space();
    let _sync = profile.open_sync_session().unwrap();
    assert!(matches!(
        profile.inspect_bytes(vec![0x01, 0x02], "nearby".into()),
        Err(MobileError::InvalidInput)
    ));
}

#[test]
fn inspect_bytes_rejects_an_empty_or_oversized_route() {
    let profile = profile_with_space();
    for bad_route in ["", "   ", &"r".repeat(257)] {
        assert!(matches!(
            profile.inspect_bytes(vec![0x01], bad_route.into()),
            Err(MobileError::InvalidInput)
        ));
    }
}

#[test]
fn inspect_bytes_rejects_bytes_that_do_not_decode_as_a_bundle() {
    let profile = profile_with_space();
    assert!(matches!(
        profile.inspect_bytes(vec![0xff, 0x00, 0xff], "nearby".into()),
        Err(MobileError::ImportRejected)
    ));
    // Rejecting undecodable bytes parked no preview and listed nothing.
    assert!(profile.list_current_entries().unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// open_sync_session / load_demo_space / create_plan refusals over a parked
// preview.
// ---------------------------------------------------------------------------

#[test]
fn open_sync_session_is_refused_while_an_import_preview_is_parked() {
    let (receiver, _signed) = receiver_holding_a_preview();
    assert!(matches!(
        receiver.open_sync_session(),
        Err(MobileError::InvalidInput)
    ));
}

#[test]
fn create_plan_with_empty_selection_over_a_parked_preview_is_invalid_input() {
    let (receiver, signed) = receiver_holding_a_preview();
    let preview = receiver
        .inspect_bytes(signed.bundle_bytes, "nearby".into())
        .expect("re-park the preview and take its handle");
    assert!(matches!(
        preview.create_plan(Vec::new()),
        Err(MobileError::InvalidInput)
    ));
    // The empty-selection refusal leaves the preview usable.
    assert_eq!(preview.eligible_entries().unwrap().len(), 1);
}

#[test]
fn load_demo_space_is_refused_while_an_import_preview_is_parked() {
    let (receiver, _signed) = receiver_holding_a_preview();
    assert!(matches!(
        receiver.load_demo_space(vec![0x01, 0x02]),
        Err(MobileError::InvalidInput)
    ));
}

#[test]
fn load_demo_space_rejects_bytes_that_do_not_decode_as_a_bundle() {
    let profile = open_local_profile().unwrap();
    assert!(matches!(
        profile.load_demo_space(vec![0xff, 0x00, 0xff]),
        Err(MobileError::ImportRejected)
    ));
}

// ---------------------------------------------------------------------------
// Sync-session refusals.
// ---------------------------------------------------------------------------

#[test]
fn reject_import_with_no_pending_review_is_invalid_input() {
    let profile = profile_with_space();
    let sync = profile.open_sync_session().unwrap();
    // No import bundle has been received, so there is nothing to reject.
    assert!(matches!(
        sync.reject_import(7),
        Err(MobileError::InvalidInput)
    ));
}

#[test]
fn cancelling_a_superseded_sync_handle_reports_object_closed() {
    let profile = profile_with_space();
    let first = profile.open_sync_session().unwrap();
    // Cancel releases the single sync slot, then a second session takes it.
    first.cancel().unwrap();
    let _second = profile.open_sync_session().unwrap();
    // The first handle now names a session that is no longer the active one.
    assert!(matches!(first.cancel(), Err(MobileError::ObjectClosed)));
}

#[test]
fn receiving_a_frame_larger_than_the_protocol_cap_is_a_session_limit() {
    let profile = profile_with_space();
    let sync = profile.open_sync_session().unwrap();
    let oversized = vec![0u8; riot_core::sync::MAX_SYNC_FRAME_BYTES + 1];
    assert!(matches!(
        sync.receive_frame(oversized),
        Err(MobileError::SessionLimit)
    ));
}

#[test]
fn replay_app_data_bundle_is_refused_while_a_sync_session_is_open() {
    let profile = profile_with_space();
    let sync = profile.open_sync_session().unwrap();
    assert!(matches!(
        profile.app_runtime().replay_app_data_bundle(vec![0x01]),
        Err(MobileError::InvalidInput)
    ));
    drop(sync);
}

// ---------------------------------------------------------------------------
// Installed-app cap.
// ---------------------------------------------------------------------------

/// A distinct manifest/bundle pair each call (fresh author -> distinct app id),
/// produced with riot-core's own app codecs exactly like `apps_contract.rs`.
fn distinct_manifest_and_bundle() -> (Vec<u8>, Vec<u8>) {
    use riot_core::apps::bundle::{encode_app_bundle, AppBundle, AppResource};
    use riot_core::apps::manifest::{encode_manifest, AppManifest};
    use riot_core::willow::generate_communal_author;

    let author = generate_communal_author().expect("author");
    let bundle = AppBundle {
        entry_point: "index.html".into(),
        resources: vec![AppResource {
            path: "index.html".into(),
            content_type: "text/html".into(),
            bytes: b"<html>app</html>".to_vec(),
        }],
    };
    let manifest = AppManifest {
        name: "Capped".into(),
        description: "Distinct per install.".into(),
        version: "1.0.0".into(),
        author: author.identity(),
        permissions: vec!["own-app-data".into()],
        entry_point: "index.html".into(),
    };
    (
        encode_manifest(&manifest).expect("manifest"),
        encode_app_bundle(&bundle).expect("bundle"),
    )
}

#[test]
fn installing_more_than_the_app_cap_is_refused_with_session_limit() {
    let profile = open_local_profile().unwrap();
    let runtime = profile.app_runtime();
    // MAX_INSTALLED_APPS == 16 distinct apps install cleanly.
    for _ in 0..16 {
        let (manifest, bundle) = distinct_manifest_and_bundle();
        runtime
            .install_app(manifest, bundle)
            .expect("within the app cap");
    }
    let (manifest, bundle) = distinct_manifest_and_bundle();
    assert!(matches!(
        runtime.install_app(manifest, bundle),
        Err(MobileError::SessionLimit)
    ));
}
