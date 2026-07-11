use riot_core::apps::bundle::{
    decode_app_bundle, encode_app_bundle, AppBundle, AppResource, MAX_BUNDLE_TOTAL_BYTES,
};
use riot_core::apps::AppsError;

fn sample_bundle() -> AppBundle {
    AppBundle {
        entry_point: "index.html".to_string(),
        resources: vec![
            AppResource {
                path: "index.html".to_string(),
                content_type: "text/html".to_string(),
                bytes: b"<html></html>".to_vec(),
            },
            AppResource {
                path: "app.js".to_string(),
                content_type: "text/javascript".to_string(),
                bytes: b"console.log('hi')".to_vec(),
            },
        ],
    }
}

#[test]
fn bundle_round_trips_through_encode_decode() {
    let bundle = sample_bundle();
    let bytes = encode_app_bundle(&bundle).expect("encode");
    let decoded = decode_app_bundle(&bytes).expect("decode");
    assert_eq!(decoded, bundle);
}

#[test]
fn entry_point_not_among_resources_is_rejected() {
    let mut bundle = sample_bundle();
    bundle.entry_point = "missing.html".to_string();
    assert_eq!(
        encode_app_bundle(&bundle),
        Err(AppsError::BundleFieldInvalid)
    );
}

#[test]
fn oversized_bundle_is_rejected() {
    let mut bundle = sample_bundle();
    bundle.resources[0].bytes = vec![0u8; MAX_BUNDLE_TOTAL_BYTES + 1];
    assert_eq!(encode_app_bundle(&bundle), Err(AppsError::BundleTooLarge));
}
