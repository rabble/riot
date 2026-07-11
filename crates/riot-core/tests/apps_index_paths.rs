use riot_core::apps::index::{
    app_bundle_digest, app_index_bundle_path, app_index_endorsement_path, app_index_manifest_path,
    app_index_prefix_for, APP_INDEX_COMPONENT,
};
use riot_core::willow::Path;

#[test]
fn manifest_and_bundle_paths_have_expected_shape() {
    let app_id = [7u8; 32];
    let manifest = app_index_manifest_path(&app_id).expect("path");
    let bundle = app_index_bundle_path(&app_id).expect("path");
    assert_eq!(
        manifest,
        Path::from_slices(&[APP_INDEX_COMPONENT, &app_id, b"manifest"]).expect("path")
    );
    assert_eq!(
        bundle,
        Path::from_slices(&[APP_INDEX_COMPONENT, &app_id, b"bundle"]).expect("path")
    );
}

#[test]
fn endorsement_path_embeds_endorser_subspace() {
    let app_id = [7u8; 32];
    let endorser = [9u8; 32];
    let path = app_index_endorsement_path(&app_id, &endorser).expect("path");
    assert_eq!(
        path,
        Path::from_slices(&[APP_INDEX_COMPONENT, &app_id, b"endorsements", &endorser])
            .expect("path")
    );
}

#[test]
fn per_app_prefix_is_a_prefix_of_all_three() {
    let app_id = [7u8; 32];
    let prefix = app_index_prefix_for(&app_id).expect("prefix");
    assert!(prefix.is_prefix_of(&app_index_manifest_path(&app_id).expect("p")));
    assert!(prefix.is_prefix_of(&app_index_bundle_path(&app_id).expect("p")));
    assert!(prefix.is_prefix_of(&app_index_endorsement_path(&app_id, &[9u8; 32]).expect("p")));
}

#[test]
fn app_bundle_digest_is_deterministic_and_length_bound() {
    let a = app_bundle_digest(b"bytes");
    let b = app_bundle_digest(b"bytes");
    let c = app_bundle_digest(b"other");
    assert_eq!(a, b);
    assert_ne!(a, c);
    // Domain separation: not a bare SHA-256 of the input.
    use sha2::{Digest, Sha256};
    let bare: [u8; 32] = Sha256::digest(b"bytes").into();
    assert_ne!(a, bare);
}
