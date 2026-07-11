//! FFI contract for the signed-JS-apps runtime surface: manifest install,
//! per-profile trust decisions, namespace-scoped app data put/get/list, and
//! the app-directory surface (listings, share, endorse) — end-to-end through
//! the UniFFI layer, in-process, same as `mobile_contract.rs`.

use riot_ffi::{open_local_profile, MobileError, PublicSpace};

/// Hex string (as returned by `install_app`/`identity`) to raw bytes (as the
/// directory surface uses for 32-byte ids).
fn unhex(value: &str) -> Vec<u8> {
    (0..value.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16).expect("hex"))
        .collect()
}

fn manifest_and_bundle() -> (Vec<u8>, Vec<u8>) {
    // Manifest/bundle bytes are produced with riot-core's own codecs — the
    // same way the future `riot-app` packaging tool will produce them.
    use riot_core::apps::bundle::{encode_app_bundle, AppBundle, AppResource};
    use riot_core::apps::manifest::{encode_manifest, AppManifest};
    use riot_core::willow::generate_communal_author;

    let author = generate_communal_author().expect("author");
    let bundle = AppBundle {
        entry_point: "index.html".to_string(),
        resources: vec![AppResource {
            path: "index.html".to_string(),
            content_type: "text/html".to_string(),
            bytes: b"<html>checklist</html>".to_vec(),
        }],
    };
    let manifest = AppManifest {
        name: "Checklist".to_string(),
        description: "Lets people add and check off shared to-dos.".to_string(),
        version: "1.0.0".to_string(),
        author: author.identity(),
        permissions: vec!["own-app-data".to_string()],
        entry_point: "index.html".to_string(),
    };
    (
        encode_manifest(&manifest).expect("manifest"),
        encode_app_bundle(&bundle).expect("bundle"),
    )
}

#[test]
fn install_returns_a_deterministic_app_id_and_rejects_garbage() {
    let profile = open_local_profile().expect("profile");
    let runtime = profile.app_runtime();
    let (manifest_bytes, bundle_bytes) = manifest_and_bundle();

    let first = runtime
        .install_app(manifest_bytes.clone(), bundle_bytes.clone())
        .expect("install");
    assert_eq!(first.name, "Checklist");
    assert_eq!(first.entry_point, "index.html");
    assert_eq!(first.app_id.len(), 64); // 32 bytes, hex

    // Reinstalling the same pair is idempotent and yields the same id.
    let second = runtime
        .install_app(manifest_bytes, bundle_bytes)
        .expect("reinstall");
    assert_eq!(first.app_id, second.app_id);

    assert!(matches!(
        runtime.install_app(vec![0xff; 8], vec![0xff; 8]),
        Err(MobileError::AppRejected)
    ));
}

#[test]
fn trust_lifecycle_is_lww_per_app() {
    let profile = open_local_profile().expect("profile");
    let runtime = profile.app_runtime();
    let (manifest_bytes, bundle_bytes) = manifest_and_bundle();
    let app = runtime
        .install_app(manifest_bytes, bundle_bytes)
        .expect("install");

    assert!(!runtime.is_app_trusted(app.app_id.clone()).expect("check"));
    runtime.trust_app(app.app_id.clone()).expect("trust");
    assert!(runtime.is_app_trusted(app.app_id.clone()).expect("check"));
    runtime.untrust_app(app.app_id.clone()).expect("untrust");
    assert!(!runtime.is_app_trusted(app.app_id.clone()).expect("check"));
    runtime.trust_app(app.app_id.clone()).expect("re-trust");
    assert!(runtime.is_app_trusted(app.app_id.clone()).expect("check"));
}

#[test]
fn app_data_round_trips_through_the_ffi_layer() {
    let profile = open_local_profile().expect("profile");
    let runtime = profile.app_runtime();
    let (manifest_bytes, bundle_bytes) = manifest_and_bundle();
    let app = runtime
        .install_app(manifest_bytes, bundle_bytes)
        .expect("install");
    runtime.trust_app(app.app_id.clone()).expect("trust");

    runtime
        .app_data_put(
            app.app_id.clone(),
            "items/a".to_string(),
            b"{\"done\":false}".to_vec(),
        )
        .expect("put");

    let value = runtime
        .app_data_get(app.app_id.clone(), "items/a".to_string())
        .expect("get");
    assert_eq!(value, Some(b"{\"done\":false}".to_vec()));

    let missing = runtime
        .app_data_get(app.app_id.clone(), "items/missing".to_string())
        .expect("get missing");
    assert_eq!(missing, None);

    let listed = runtime
        .app_data_list(app.app_id.clone(), "items".to_string())
        .expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].key, "items/a");
    assert_eq!(listed[0].value, b"{\"done\":false}".to_vec());
}

#[test]
fn hostile_inputs_are_rejected_without_state_damage() {
    let profile = open_local_profile().expect("profile");
    let runtime = profile.app_runtime();
    let (manifest_bytes, bundle_bytes) = manifest_and_bundle();
    let app = runtime
        .install_app(manifest_bytes, bundle_bytes)
        .expect("install");

    // Traversal-shaped key.
    assert!(matches!(
        runtime.app_data_put(app.app_id.clone(), "../escape".to_string(), b"x".to_vec()),
        Err(MobileError::AppRejected)
    ));
    // Malformed app ids (non-hex, wrong length).
    assert!(runtime.is_app_trusted("zz".repeat(32)).is_err());
    assert!(runtime
        .app_data_get("abcd".to_string(), "items/a".to_string())
        .is_err());

    // The profile still works afterwards.
    let listed = runtime
        .app_data_list(app.app_id, "items".to_string())
        .expect("list");
    assert!(listed.is_empty());
}

#[test]
fn app_data_put_does_not_break_sync_sessions() {
    // Regression (review C1): a put must neither brick a later
    // open_sync_session (sync-inventory completeness is alert-only) nor be
    // allowed while a sync session is active (store.inspect would clobber
    // the in-flight sync review).
    let profile = open_local_profile().expect("profile");
    profile
        .create_public_space("Sync fixture".into())
        .expect("space");
    let runtime = profile.app_runtime();
    let (manifest_bytes, bundle_bytes) = manifest_and_bundle();
    let app = runtime
        .install_app(manifest_bytes, bundle_bytes)
        .expect("install");
    runtime.trust_app(app.app_id.clone()).expect("trust");

    runtime
        .app_data_put(app.app_id.clone(), "items/a".to_string(), b"x".to_vec())
        .expect("put");

    let sync = profile.open_sync_session().expect("sync opens after a put");
    assert!(matches!(
        runtime.app_data_put(app.app_id.clone(), "items/b".to_string(), b"y".to_vec()),
        Err(MobileError::InvalidInput)
    ));
    sync.cancel().expect("cancel");

    runtime
        .app_data_put(app.app_id, "items/b".to_string(), b"y".to_vec())
        .expect("put works again after cancel");
}

#[test]
fn shared_app_appears_in_directory_with_carrier_provenance() {
    let profile = open_local_profile().expect("profile");
    let space = profile
        .create_public_space("Directory fixture".into())
        .expect("space");
    let runtime = profile.app_runtime();
    let (manifest_bytes, bundle_bytes) = manifest_and_bundle();
    let app = runtime
        .install_app(manifest_bytes, bundle_bytes)
        .expect("install");
    let app_id = unhex(&app.app_id);

    // Not listed before sharing: install alone publishes nothing.
    let before = runtime.directory_listings().expect("listings");
    assert!(before.iter().all(|listing| listing.app_id != app_id));

    runtime
        .share_app(app_id.clone(), space.clone())
        .expect("share");

    let listings = runtime.directory_listings().expect("listings");
    let listing = listings
        .iter()
        .find(|listing| listing.app_id == app_id)
        .expect("shared app listed");
    assert_eq!(listing.name, "Checklist");
    assert_eq!(listing.version, "1.0.0");
    assert!(listing.bundle_present);
    assert!(!listing.built_in);
    assert!(listing.carrier_subspace_id.is_some());
    assert_eq!(listing.superseded_by, None);
    assert!(listing.trusted_in_spaces.is_empty()); // sharing never auto-trusts

    // A space the profile has not joined is not a valid share target.
    let foreign_space = PublicSpace {
        namespace_id: "ab".repeat(32),
        title: "Elsewhere".into(),
        is_public: true,
    };
    assert!(matches!(
        runtime.share_app(app_id, foreign_space),
        Err(MobileError::InvalidInput)
    ));

    // Sharing an app id nothing local can resolve has nothing to publish.
    assert!(matches!(
        runtime.share_app(vec![0x5a; 32], space),
        Err(MobileError::AppRejected)
    ));
}

#[test]
fn starter_checklist_is_listed_built_in_with_canonical_id() {
    let profile = open_local_profile().expect("profile");
    let runtime = profile.app_runtime();

    let expected =
        riot_core::apps::starter::verify_starter_catalog(riot_core::apps::starter::STARTER_CATALOG);
    assert!(
        !expected.is_empty(),
        "embedded starter catalog must verify against its own codecs"
    );

    let listings = runtime.directory_listings().expect("listings");
    for starter in &expected {
        let listing = listings
            .iter()
            .find(|listing| listing.app_id == starter.app_id.to_vec())
            .expect("starter app listed under its canonical id");
        assert!(listing.built_in);
        assert!(listing.bundle_present);
        assert!(listing.carrier_subspace_id.is_none());
        assert_eq!(listing.name, starter.manifest.name);
    }
}

#[test]
fn trusting_an_app_marks_the_space_in_listings() {
    let profile = open_local_profile().expect("profile");
    let runtime = profile.app_runtime();
    let starter =
        riot_core::apps::starter::verify_starter_catalog(riot_core::apps::starter::STARTER_CATALOG)
            .pop()
            .expect("starter app");
    let app_id_hex: String = starter.app_id.iter().map(|b| format!("{b:02x}")).collect();
    let own_namespace = unhex(&profile.identity().expect("identity").namespace_id);

    runtime.trust_app(app_id_hex.clone()).expect("trust");
    let listings = runtime.directory_listings().expect("listings");
    let listing = listings
        .iter()
        .find(|listing| listing.app_id == starter.app_id.to_vec())
        .expect("starter listed");
    assert_eq!(listing.trusted_in_spaces, vec![own_namespace]);

    runtime.untrust_app(app_id_hex).expect("untrust");
    let listings = runtime.directory_listings().expect("listings");
    let listing = listings
        .iter()
        .find(|listing| listing.app_id == starter.app_id.to_vec())
        .expect("starter listed");
    assert!(listing.trusted_in_spaces.is_empty());
}

#[test]
fn endorsement_bumps_counts_and_retraction_clears_them() {
    let profile = open_local_profile().expect("profile");
    let runtime = profile.app_runtime();
    let starter =
        riot_core::apps::starter::verify_starter_catalog(riot_core::apps::starter::STARTER_CATALOG)
            .pop()
            .expect("starter app");
    let app_id = starter.app_id.to_vec();

    runtime
        .endorse_app(app_id.clone(), "we ran the drill with this".into(), false)
        .expect("endorse");
    let listings = runtime.directory_listings().expect("listings");
    let listing = listings
        .iter()
        .find(|listing| listing.app_id == app_id)
        .expect("starter listed");
    // The endorsement entry itself is live in the local store, so this
    // profile's own subspace counts as met.
    assert_eq!(listing.endorsing_met_subspaces.len(), 1);
    assert_eq!(listing.endorsing_unmet_count, 0);

    runtime
        .endorse_app(app_id.clone(), String::new(), true)
        .expect("retract");
    let listings = runtime.directory_listings().expect("listings");
    let listing = listings
        .iter()
        .find(|listing| listing.app_id == app_id)
        .expect("starter listed");
    assert!(listing.endorsing_met_subspaces.is_empty());
    assert_eq!(listing.endorsing_unmet_count, 0);

    // Note length is enforced by the core codec, surfaced as AppRejected.
    assert!(matches!(
        runtime.endorse_app(app_id, "x".repeat(201), false),
        Err(MobileError::AppRejected)
    ));
    // Malformed app id.
    assert!(matches!(
        runtime.endorse_app(vec![1; 8], String::new(), false),
        Err(MobileError::InvalidInput)
    ));
}

#[test]
fn share_and_endorse_respect_active_sync_and_never_brick_it() {
    // Same discipline as app_data_put: app-index writes are refused while a
    // sync session is active, and entries they add must not violate the
    // alert-only sync-inventory completeness invariant afterwards.
    let profile = open_local_profile().expect("profile");
    let space = profile
        .create_public_space("Sync guard fixture".into())
        .expect("space");
    let runtime = profile.app_runtime();
    let (manifest_bytes, bundle_bytes) = manifest_and_bundle();
    let app = runtime
        .install_app(manifest_bytes, bundle_bytes)
        .expect("install");
    let app_id = unhex(&app.app_id);

    runtime
        .share_app(app_id.clone(), space.clone())
        .expect("share");
    runtime
        .endorse_app(app_id.clone(), "endorsed".into(), false)
        .expect("endorse");

    let sync = profile
        .open_sync_session()
        .expect("sync opens after app-index writes");
    assert!(matches!(
        runtime.share_app(app_id.clone(), space.clone()),
        Err(MobileError::InvalidInput)
    ));
    assert!(matches!(
        runtime.endorse_app(app_id.clone(), String::new(), true),
        Err(MobileError::InvalidInput)
    ));
    sync.cancel().expect("cancel");

    runtime
        .share_app(app_id.clone(), space)
        .expect("re-share works after cancel");
    runtime
        .endorse_app(app_id, String::new(), true)
        .expect("retract works after cancel");
}

#[test]
fn trust_toggles_never_exhaust_the_marker_cap() {
    // Regression (review M2): markers compact to latest-per-app, so the cap
    // bounds distinct apps, not lifetime toggles.
    let profile = open_local_profile().expect("profile");
    let runtime = profile.app_runtime();
    let (manifest_bytes, bundle_bytes) = manifest_and_bundle();
    let app = runtime
        .install_app(manifest_bytes, bundle_bytes)
        .expect("install");

    for _ in 0..300 {
        runtime.trust_app(app.app_id.clone()).expect("trust");
        runtime.untrust_app(app.app_id.clone()).expect("untrust");
    }
    runtime.trust_app(app.app_id.clone()).expect("final trust");
    assert!(runtime.is_app_trusted(app.app_id).expect("check"));
}
