//! What the FFI boundary says NO to, and what it says while doing it.
//!
//! Every refusal here is provoked through the exported surface a native shell
//! actually calls, with the real cause (no space, an open sync session, bytes
//! that are not a bundle, a database path that cannot be opened) rather than a
//! stub. The error codes are asserted verbatim because iOS and Android switch on
//! them: a renamed code is a broken app, not a cosmetic change.

use riot_core::import::{decode_bundle, BundleDecodeOutcome};
use riot_core::model::{decode_alert, Certainty, Severity, Urgency};
use riot_core::sync::MAX_SYNC_FRAME_BYTES;
use riot_ffi::{
    open_local_profile, open_local_profile_with_database, open_profile_from_sealed_identity,
    open_profile_from_sealed_identity_with_database, AlertCertainty, AlertDraftInput,
    AlertSeverity, AlertUrgency, MobileError, MobileProfile, PublicSpace, SignedAlert,
};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

const TEST_WRAPPING_KEY: [u8; 32] = [0x42; 32];

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
        headline: "Marshalling point moved".into(),
        description: "The south gate is closed; use the north entrance.".into(),
        affected_area_claim: None,
        source_claims: vec!["Field observer".into()],
        ai_assisted: false,
    }
}

fn profile_with_space() -> Arc<MobileProfile> {
    let profile = open_local_profile().expect("profile");
    profile
        .create_public_space("Bounded incident".into())
        .expect("space");
    profile
}

/// A profile holding one signed alert, and that alert's bundle — the bytes a
/// peer would hand over.
fn profile_with_signed_alert() -> (Arc<MobileProfile>, SignedAlert) {
    let profile = profile_with_space();
    let record = profile.create_draft_alert(draft()).expect("draft");
    let signed = profile.sign_draft(record.draft_id).expect("sign");
    (profile, signed)
}

/// Every error code the boundary can emit, rendered. Native shells switch on
/// these exact strings, so each one is pinned and all of them must be distinct.
#[test]
fn every_mobile_error_renders_its_stable_code() {
    let cases = [
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
    for (error, expected) in &cases {
        assert_eq!(error.to_string(), *expected, "{error:?} rendered wrongly");
    }
    let mut codes: Vec<_> = cases.iter().map(|(error, _)| error.to_string()).collect();
    codes.sort();
    codes.dedup();
    assert_eq!(
        codes.len(),
        cases.len(),
        "two error variants share one code, so a caller cannot tell them apart"
    );
}

/// A database path the host cannot open is a `Database` error — a distinct,
/// actionable code — and never a generic `Internal`. Both database-backed
/// constructors report it.
#[test]
fn an_unopenable_database_path_is_reported_as_a_database_error() {
    let unusable = "/riot-no-such-directory-exists/profile.sqlite".to_string();

    assert!(
        matches!(
            open_local_profile_with_database(unusable.clone()),
            Err(MobileError::Database)
        ),
        "an unopenable path must name the database as the cause"
    );

    // The sealed-identity constructor opens the database first, so a bad path
    // is reported before the identity is even considered.
    let sealed = {
        let profile = open_local_profile().expect("profile");
        profile
            .seal_identity(TEST_WRAPPING_KEY.to_vec())
            .expect("seal")
    };
    assert!(matches!(
        open_profile_from_sealed_identity_with_database(
            unusable,
            TEST_WRAPPING_KEY.to_vec(),
            sealed
        ),
        Err(MobileError::Database)
    ));
}

/// Every alert enum survives the crossing into the core's own vocabulary. These
/// are three parallel mappings that a reordered enum would silently scramble —
/// an `Immediate` alert arriving as `Past` is a safety bug, not a typo.
#[test]
fn every_alert_enum_crosses_the_boundary_intact() {
    let urgencies = [
        (AlertUrgency::Immediate, Urgency::Immediate),
        (AlertUrgency::Expected, Urgency::Expected),
        (AlertUrgency::Future, Urgency::Future),
        (AlertUrgency::Past, Urgency::Past),
        (AlertUrgency::Unknown, Urgency::Unknown),
    ];
    let severities = [
        (AlertSeverity::Extreme, Severity::Extreme),
        (AlertSeverity::Severe, Severity::Severe),
        (AlertSeverity::Moderate, Severity::Moderate),
        (AlertSeverity::Minor, Severity::Minor),
        (AlertSeverity::Unknown, Severity::Unknown),
    ];
    let certainties = [
        (AlertCertainty::Observed, Certainty::Observed),
        (AlertCertainty::Likely, Certainty::Likely),
        (AlertCertainty::Possible, Certainty::Possible),
        (AlertCertainty::Unlikely, Certainty::Unlikely),
        (AlertCertainty::Unknown, Certainty::Unknown),
    ];

    // Five signings, each carrying one urgency, one severity and one certainty,
    // so all fifteen mappings are read back off real signed bytes.
    for index in 0..5 {
        let (ffi_urgency, core_urgency) = urgencies[index];
        let (ffi_severity, core_severity) = severities[index];
        let (ffi_certainty, core_certainty) = certainties[index];

        let profile = profile_with_space();
        let record = profile
            .create_draft_alert(AlertDraftInput {
                urgency: ffi_urgency,
                severity: ffi_severity,
                certainty: ffi_certainty,
                ..draft()
            })
            .expect("draft");
        let signed = profile.sign_draft(record.draft_id).expect("sign");

        let BundleDecodeOutcome::Decoded(decoded) = decode_bundle(&signed.bundle_bytes) else {
            panic!("a freshly signed alert must decode");
        };
        let payload = decode_alert(decoded.items[0].frame.payload_bytes()).expect("decode alert");
        assert_eq!(payload.urgency, core_urgency, "urgency was scrambled");
        assert_eq!(payload.severity, core_severity, "severity was scrambled");
        assert_eq!(payload.certainty, core_certainty, "certainty was scrambled");
    }
}

/// A space needs a real title, and a profile may hold only one. Both refusals
/// are `InvalidInput`, and neither leaves a half-listed space behind.
#[test]
fn a_space_needs_a_title_and_a_profile_holds_only_one() {
    let profile = open_local_profile().expect("profile");
    for title in ["", "   ", &"t".repeat(513)] {
        assert!(
            matches!(
                profile.create_public_space(title.into()),
                Err(MobileError::InvalidInput)
            ),
            "a {}-byte title was accepted",
            title.len()
        );
    }
    // Nothing was listed, so drafting is still refused.
    assert!(matches!(
        profile.create_draft_alert(draft()),
        Err(MobileError::InvalidInput)
    ));

    let space = profile
        .create_public_space("Real space".into())
        .expect("space");

    // Joining is refused once a space is listed, and refused outright for a
    // space that is not public or has no title.
    assert!(matches!(
        profile.join_public_space(space.clone()),
        Err(MobileError::InvalidInput)
    ));

    let fresh = open_local_profile().expect("profile");
    for candidate in [
        PublicSpace {
            is_public: false,
            ..space.clone()
        },
        PublicSpace {
            title: "  ".into(),
            ..space.clone()
        },
        PublicSpace {
            namespace_id: "not-hex".into(),
            ..space.clone()
        },
        PublicSpace {
            // Well-formed hex, but an *owned* namespace (odd final byte). No
            // communal author can be minted inside it, so it is not a space
            // anyone can join — and saying so is the point: this is refused for
            // a different reason than the malformed id above, and the boundary
            // must not confuse the two into a crash.
            namespace_id: format!("{}01", "00".repeat(31)),
            ..space.clone()
        },
    ] {
        assert!(
            matches!(
                fresh.join_public_space(candidate),
                Err(MobileError::InvalidInput)
            ),
            "an unusable space was joined"
        );
    }
    // Still no space: the refusals did not half-join anything.
    assert!(matches!(
        fresh.create_draft_alert(draft()),
        Err(MobileError::InvalidInput)
    ));
}

/// An open sync session freezes every door that would clobber the preview slot
/// the review is holding. Each refusal is `InvalidInput`, and closing the
/// session reopens the door.
#[test]
fn an_open_sync_session_freezes_the_writing_doors() {
    let (profile, signed) = profile_with_signed_alert();
    let space = PublicSpace {
        namespace_id: signed.entry.namespace_id.clone(),
        title: "Bounded incident".into(),
        is_public: true,
    };
    let session = profile.open_sync_session().expect("sync session");

    assert!(matches!(
        profile.create_public_space("Another".into()),
        Err(MobileError::InvalidInput)
    ));
    assert!(matches!(
        profile.join_public_space(space),
        Err(MobileError::InvalidInput)
    ));
    assert!(matches!(
        profile.inspect_bytes(signed.bundle_bytes.clone(), "nearby".into()),
        Err(MobileError::InvalidInput)
    ));
    assert!(matches!(
        profile.load_demo_space(signed.bundle_bytes.clone()),
        Err(MobileError::InvalidInput)
    ));
    assert!(matches!(
        profile
            .app_runtime()
            .replay_app_data_bundle(signed.bundle_bytes.clone()),
        Err(MobileError::InvalidInput)
    ));
    let record = profile.create_draft_alert(draft()).expect("draft");
    assert!(matches!(
        profile.sign_draft(record.draft_id),
        Err(MobileError::InvalidInput)
    ));

    // Closing the session reopens the doors it was holding shut.
    session.cancel().expect("cancel");
    profile
        .sign_draft(record.draft_id)
        .expect("sign after cancel");
}

/// Bytes that are not an evidence bundle are refused at every door that takes
/// bytes, and always as `IMPORT_REJECTED` — never as a panic and never
/// half-imported.
#[test]
fn bytes_that_are_not_a_bundle_are_refused_at_every_door() {
    let profile = profile_with_space();
    let garbage = vec![0xff_u8; 64];

    assert!(matches!(
        profile.inspect_bytes(garbage.clone(), "nearby-device".into()),
        Err(MobileError::ImportRejected)
    ));
    assert!(matches!(
        profile
            .app_runtime()
            .replay_app_data_bundle(garbage.clone()),
        Err(MobileError::ImportRejected)
    ));

    // The demo door decodes the bundle whole before it will list anything.
    let fresh = open_local_profile().expect("profile");
    assert!(matches!(
        fresh.load_demo_space(garbage),
        Err(MobileError::ImportRejected)
    ));

    // Nothing was admitted anywhere.
    assert!(profile.list_current_entries().expect("listing").is_empty());
}

/// The import route is a label, and it is bounded: an empty one names nothing
/// and an unbounded one is a memory hole. Both are refused before the bytes are
/// even decoded.
#[test]
fn the_import_route_is_bounded() {
    let (_signer, signed) = profile_with_signed_alert();
    let peer = profile_with_space();

    for route in ["", "   ", &"r".repeat(257)] {
        assert!(
            matches!(
                peer.inspect_bytes(signed.bundle_bytes.clone(), route.into()),
                Err(MobileError::InvalidInput)
            ),
            "a {}-byte route was accepted",
            route.len()
        );
    }
}

/// Sync handles refuse calls made outside the state they belong to: reviewing
/// an import that was never offered, and using a handle whose session has been
/// replaced.
#[test]
fn a_sync_handle_refuses_calls_outside_its_state() {
    let (profile, signed) = profile_with_signed_alert();

    // A preview in flight blocks a sync session from opening at all: the sync
    // review would need the very preview slot the pending import is holding.
    let joiner = open_local_profile().expect("profile");
    joiner
        .join_public_space(PublicSpace {
            namespace_id: signed.entry.namespace_id.clone(),
            title: "Bounded incident".into(),
            is_public: true,
        })
        .expect("join");
    let _preview = joiner
        .inspect_bytes(signed.bundle_bytes.clone(), "nearby-device".into())
        .expect("preview");
    assert!(matches!(
        joiner.open_sync_session(),
        Err(MobileError::InvalidInput)
    ));

    let session = profile.open_sync_session().expect("sync session");

    // Nothing is pending review, so accepting or rejecting one is invalid.
    assert!(matches!(
        session.reject_import(3),
        Err(MobileError::InvalidInput)
    ));
    assert!(matches!(
        session.accept_import(),
        Err(MobileError::InvalidInput)
    ));

    // A handle whose session has been replaced is closed, not silently aimed at
    // the new session.
    session.cancel().expect("cancel");
    let replacement = profile.open_sync_session().expect("second sync session");
    assert!(matches!(session.cancel(), Err(MobileError::ObjectClosed)));
    assert!(matches!(session.begin(), Err(MobileError::ObjectClosed)));
    // The replacement is unharmed.
    replacement.begin().expect("the live session still works");
}

/// A frame past the wire ceiling is a `SESSION_LIMIT`, refused before it is
/// parsed — the ceiling is what stops a peer from making us allocate without
/// bound.
#[test]
fn an_oversized_sync_frame_is_a_session_limit() {
    let (profile, _signed) = profile_with_signed_alert();
    let session = profile.open_sync_session().expect("sync session");
    session.begin().expect("begin");
    // The opening frame has to leave before the session will read one in.
    session
        .take_outbound_frame()
        .expect("take outbound")
        .expect("begin queued a frame");

    let oversized = vec![0_u8; MAX_SYNC_FRAME_BYTES + 1];
    assert!(matches!(
        session.receive_frame(oversized),
        Err(MobileError::SessionLimit)
    ));
}

/// The demo space is additive: it will not displace a space that is already
/// listed, and it will not run while an import is in flight.
#[test]
fn the_demo_space_refuses_to_displace_a_real_space() {
    // A bundle from someone else's namespace — what a demo bundle looks like to
    // a profile that already has its own space.
    let (_stranger, signed) = profile_with_signed_alert();

    let profile = profile_with_space();
    assert_ne!(
        signed.entry.namespace_id,
        profile.identity().expect("identity").namespace_id,
        "the fixture must come from a different namespace to be a real test"
    );

    assert!(
        matches!(
            profile.load_demo_space(signed.bundle_bytes.clone()),
            Err(MobileError::ImportRejected)
        ),
        "the demo displaced a listed space"
    );

    // An import in flight also blocks it: the demo load would clobber the very
    // preview slot the pending import is holding.
    let (peer, peer_signed) = profile_with_signed_alert();
    let _preview = peer
        .inspect_bytes(peer_signed.bundle_bytes.clone(), "nearby-device".into())
        .expect("preview");
    assert!(matches!(
        peer.load_demo_space(signed.bundle_bytes),
        Err(MobileError::InvalidInput)
    ));
}

/// A replayed app-data bundle whose entries do not verify carries nothing to
/// admit, and is refused rather than admitting the zero entries that survived.
#[test]
fn a_replay_bundle_with_no_valid_entries_is_refused() {
    use minicbor::Encoder;
    use riot_core::apps::entry::app_data_path;
    use riot_core::import::{BUNDLE_CODEC_ID, BUNDLE_MAGIC};
    use riot_core::willow::{
        authorise_entry, encode_capability, encode_entry, generate_communal_author, Entry,
    };

    let author = generate_communal_author().expect("author");
    let app_id = [7_u8; 32];
    let payload = b"value".to_vec();
    let entry = Entry::builder()
        .namespace_id(author.namespace_id().clone())
        .subspace_id(author.subspace_id())
        .path(app_data_path(&app_id, "key").expect("app data path"))
        .timestamp(1)
        .payload(&payload)
        .build();
    let authorised = authorise_entry(&author, entry).expect("authorise");
    let token = authorised.authorisation_token();

    // A structurally perfect app-data entry carrying a signature that is simply
    // not the signature over it. `encode_bundle` would refuse to build this —
    // it verifies what it encodes — so the bundle is assembled directly, which
    // is exactly what a hostile peer would send. The bundle decodes; the item
    // inside it does not verify.
    let mut bundle = BUNDLE_MAGIC.to_vec();
    let mut encoder = Encoder::new(&mut bundle);
    encoder.map(2).expect("map");
    encoder
        .u8(0)
        .expect("key")
        .str(BUNDLE_CODEC_ID)
        .expect("codec");
    encoder.u8(1).expect("key").array(1).expect("items");
    encoder.map(4).expect("item");
    encoder
        .u8(0)
        .expect("key")
        .bytes(&encode_entry(authorised.entry()))
        .expect("entry");
    encoder
        .u8(1)
        .expect("key")
        .bytes(&encode_capability(token.capability()))
        .expect("capability");
    encoder
        .u8(2)
        .expect("key")
        .bytes(&[0_u8; 64])
        .expect("a signature that is not the signature");
    encoder
        .u8(3)
        .expect("key")
        .bytes(&payload)
        .expect("payload");

    assert!(
        matches!(decode_bundle(&bundle), BundleDecodeOutcome::Decoded(_)),
        "the bundle itself must decode, or this tests the wrong refusal"
    );

    let profile = profile_with_space();
    assert!(matches!(
        profile.app_runtime().replay_app_data_bundle(bundle),
        Err(MobileError::ImportRejected)
    ));
}

/// The installed-app cap is real: the seventeenth distinct app is refused with
/// `SESSION_LIMIT`, and the sixteen already installed are untouched.
#[test]
fn installing_past_the_cap_is_a_session_limit() {
    use riot_core::apps::bundle::{encode_app_bundle, AppBundle, AppResource};
    use riot_core::apps::manifest::{encode_manifest, AppManifest};
    use riot_core::willow::generate_communal_author;

    let author = generate_communal_author().expect("author");
    let pair = |index: usize| {
        let bundle = AppBundle {
            entry_point: "index.html".to_string(),
            resources: vec![AppResource {
                path: "index.html".to_string(),
                content_type: "text/html".to_string(),
                // Distinct bytes give a distinct digest, hence a distinct app id.
                bytes: format!("<html>app {index}</html>").into_bytes(),
            }],
        };
        let manifest = AppManifest {
            name: format!("App {index}"),
            description: "One of many".to_string(),
            version: "1.0.0".to_string(),
            author: author.identity(),
            permissions: vec!["own-app-data".to_string()],
            entry_point: "index.html".to_string(),
        };
        (
            encode_manifest(&manifest).expect("manifest"),
            encode_app_bundle(&bundle).expect("bundle"),
        )
    };

    let profile = profile_with_space();
    let runtime = profile.app_runtime();

    // MAX_INSTALLED_APPS is 16.
    let mut installed = Vec::new();
    for index in 0..16 {
        let (manifest, bundle) = pair(index);
        let record = runtime
            .install_app(manifest, bundle)
            .unwrap_or_else(|error| panic!("app {index} was refused: {error:?}"));
        installed.push(record.app_id);
    }
    installed.sort();
    installed.dedup();
    assert_eq!(
        installed.len(),
        16,
        "the apps must be distinct to fill the cap"
    );

    let (manifest, bundle) = pair(16);
    assert!(
        matches!(
            runtime.install_app(manifest, bundle),
            Err(MobileError::SessionLimit)
        ),
        "the cap did not hold"
    );

    // Re-installing one already held is idempotent, not a limit failure: the cap
    // counts distinct apps, not calls.
    let (manifest, bundle) = pair(0);
    runtime
        .install_app(manifest, bundle)
        .expect("re-installing a held app is not a new app");
}

/// A sealed identity that is not ours is refused as `InvalidInput`, and a
/// wrapping key of the wrong length never reaches the crypto at all.
#[test]
fn a_sealed_identity_that_does_not_open_is_invalid_input() {
    let profile = open_local_profile().expect("profile");
    let sealed = profile
        .seal_identity(TEST_WRAPPING_KEY.to_vec())
        .expect("seal");

    // Wrong key.
    assert!(matches!(
        open_profile_from_sealed_identity(vec![0x01; 32], sealed.clone()),
        Err(MobileError::InvalidInput)
    ));
    // Garbage ciphertext.
    assert!(matches!(
        open_profile_from_sealed_identity(TEST_WRAPPING_KEY.to_vec(), vec![0xff; 64]),
        Err(MobileError::InvalidInput)
    ));
    // A key that is not 32 bytes is refused before it is used.
    assert!(matches!(
        open_profile_from_sealed_identity(vec![0x42; 16], sealed),
        Err(MobileError::InvalidInput)
    ));
    assert!(matches!(
        profile.seal_identity(vec![0x42; 31]),
        Err(MobileError::InvalidInput)
    ));
}
