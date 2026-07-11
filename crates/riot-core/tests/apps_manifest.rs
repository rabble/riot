use riot_core::apps::manifest::{
    app_id_for, decode_manifest, encode_manifest, AppManifest, MAX_APP_DESCRIPTION_BYTES,
};
use riot_core::apps::AppsError;
use riot_core::willow::generate_communal_author;

fn sample_manifest(author_identity: riot_core::willow::AuthorIdentity) -> AppManifest {
    AppManifest {
        name: "Checklist".to_string(),
        description: "Lets people add and check off shared to-dos.".to_string(),
        version: "1.0.0".to_string(),
        author: author_identity,
        permissions: vec!["own-app-data".to_string()],
        entry_point: "index.html".to_string(),
    }
}

#[test]
fn manifest_round_trips_through_encode_decode() {
    let author = generate_communal_author().expect("author");
    let manifest = sample_manifest(author.identity());
    let bytes = encode_manifest(&manifest).expect("encode");
    let decoded = decode_manifest(&bytes).expect("decode");
    assert_eq!(decoded, manifest);
}

#[test]
fn oversized_description_is_rejected() {
    let author = generate_communal_author().expect("author");
    let mut manifest = sample_manifest(author.identity());
    manifest.description = "x".repeat(MAX_APP_DESCRIPTION_BYTES + 1);
    assert_eq!(
        encode_manifest(&manifest),
        Err(AppsError::ManifestFieldInvalid)
    );
}

#[test]
fn app_id_is_deterministic_and_bundle_sensitive() {
    let author = generate_communal_author().expect("author");
    let manifest = sample_manifest(author.identity());
    let bundle_digest_a = [1u8; 32];
    let bundle_digest_b = [2u8; 32];
    let id_a1 = app_id_for(&manifest, &bundle_digest_a).expect("id");
    let id_a2 = app_id_for(&manifest, &bundle_digest_a).expect("id");
    let id_b = app_id_for(&manifest, &bundle_digest_b).expect("id");
    assert_eq!(id_a1, id_a2);
    assert_ne!(id_a1, id_b);
}
