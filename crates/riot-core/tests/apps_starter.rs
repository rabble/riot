use riot_core::apps::bundle::{encode_app_bundle, AppBundle, AppResource};
use riot_core::apps::directory::AppProvenance;
use riot_core::apps::manifest::{encode_manifest, AppManifest};
use riot_core::apps::starter::{verify_starter_catalog, STARTER_CATALOG};
use riot_core::willow::generate_communal_author;

fn pair(name: &str) -> (Vec<u8>, Vec<u8>) {
    let author = generate_communal_author().expect("author");
    let bundle_bytes = encode_app_bundle(&AppBundle {
        entry_point: "index.html".to_string(),
        resources: vec![AppResource {
            path: "index.html".to_string(),
            content_type: "text/html".to_string(),
            bytes: b"<html></html>".to_vec(),
        }],
    })
    .expect("bundle");
    let manifest_bytes = encode_manifest(&AppManifest {
        name: name.to_string(),
        description: "Built-in tool.".to_string(),
        version: "1.0.0".to_string(),
        author: author.identity(),
        permissions: vec!["own-app-data".to_string()],
        entry_point: "index.html".to_string(),
    })
    .expect("manifest");
    (manifest_bytes, bundle_bytes)
}

#[test]
fn valid_pairs_verify_with_built_in_provenance_and_zero_timestamp() {
    let (m, b) = pair("Checklist");
    let apps = verify_starter_catalog(&[(&m, &b)]);
    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].provenance, AppProvenance::BuiltIn);
    assert_eq!(apps[0].manifest_timestamp_micros, 0);
    assert!(apps[0].bundle_present);
}

#[test]
fn corrupted_built_in_is_silently_excluded() {
    let (m, b) = pair("Checklist");
    let mut corrupt = b.clone();
    // Flip the leading CBOR map-header byte, not the trailing byte: per
    // `apps_codec_hostile.rs`'s documented codec contract, a flip that
    // lands inside a resource's raw byte content ("rejected or stays
    // canonical") can decode as a *different, still-canonical* document —
    // that's the codec's tested, intended behavior, not something
    // `verify_starter_catalog` should be asked to catch. A flip of the
    // top-level framing byte, by contrast, is guaranteed to break
    // decoding regardless of content, which is what "corrupted" must mean
    // for this test to be meaningful.
    corrupt[0] ^= 0xFF;
    let apps = verify_starter_catalog(&[(&m, &corrupt)]);
    assert!(apps.is_empty(), "corrupt bundle must be excluded, not an error");
}

#[test]
fn the_shipped_catalog_verifies_completely() {
    // Guards the embedded catalog forever: every shipped pair must verify.
    let apps = verify_starter_catalog(STARTER_CATALOG);
    assert_eq!(apps.len(), STARTER_CATALOG.len());
}
