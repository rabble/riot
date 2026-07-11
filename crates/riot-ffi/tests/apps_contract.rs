//! FFI contract for the signed-JS-apps runtime surface: manifest install,
//! per-profile trust decisions, and namespace-scoped app data put/get/list —
//! end-to-end through the UniFFI layer, in-process, same as
//! `mobile_contract.rs`.

use riot_ffi::{open_local_profile, MobileError};

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
