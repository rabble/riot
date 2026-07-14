use riot_core::apps::bundle::{
    decode_app_bundle, encode_app_bundle, AppBundle, AppResource, MAX_BUNDLE_RESOURCES,
    MAX_BUNDLE_TOTAL_BYTES, MAX_RESOURCE_CONTENT_TYPE_BYTES, MAX_RESOURCE_PATH_BYTES,
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

#[test]
fn bundle_digest_is_deterministic_and_content_sensitive() {
    use riot_core::apps::bundle::app_bundle_digest;
    let bytes_a = encode_app_bundle(&sample_bundle()).expect("encode");
    let mut other = sample_bundle();
    other.resources[1].bytes = b"console.log('bye')".to_vec();
    let bytes_b = encode_app_bundle(&other).expect("encode");

    assert_eq!(app_bundle_digest(&bytes_a), app_bundle_digest(&bytes_a));
    assert_ne!(app_bundle_digest(&bytes_a), app_bundle_digest(&bytes_b));
}

#[test]
fn bundle_validation_enforces_each_resource_shape_and_count_boundary() {
    let mut bundle = sample_bundle();
    bundle.resources.clear();
    assert_eq!(
        encode_app_bundle(&bundle),
        Err(AppsError::BundleFieldInvalid)
    );

    let resource = sample_bundle().resources.remove(0);
    let mut bundle = sample_bundle();
    bundle.resources = vec![resource; MAX_BUNDLE_RESOURCES + 1];
    assert_eq!(
        encode_app_bundle(&bundle),
        Err(AppsError::BundleFieldInvalid)
    );

    for path in [String::new(), "p".repeat(MAX_RESOURCE_PATH_BYTES + 1)] {
        let mut bundle = sample_bundle();
        bundle.resources[0].path = path;
        assert_eq!(
            encode_app_bundle(&bundle),
            Err(AppsError::BundleFieldInvalid)
        );
    }

    for content_type in [
        String::new(),
        "t".repeat(MAX_RESOURCE_CONTENT_TYPE_BYTES + 1),
    ] {
        let mut bundle = sample_bundle();
        bundle.resources[0].content_type = content_type;
        assert_eq!(
            encode_app_bundle(&bundle),
            Err(AppsError::BundleFieldInvalid)
        );
    }
}

#[test]
fn payload_at_raw_ceiling_is_rejected_when_canonical_framing_crosses_ceiling() {
    let bundle = AppBundle {
        entry_point: "index.html".to_string(),
        resources: vec![AppResource {
            path: "index.html".to_string(),
            content_type: "text/html".to_string(),
            bytes: vec![0; MAX_BUNDLE_TOTAL_BYTES],
        }],
    };
    assert_eq!(encode_app_bundle(&bundle), Err(AppsError::BundleTooLarge));
}

#[expect(
    clippy::too_many_arguments,
    reason = "each argument selects one independent malformed-CBOR axis"
)]
fn raw_bundle(
    top_pairs: u64,
    top_keys: [u64; 2],
    resource_count: u64,
    resource_pairs: u64,
    resource_keys: [u64; 3],
    path: Option<&str>,
    content_type: Option<&str>,
    bytes_are_bytes: bool,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = minicbor::Encoder::new(&mut bytes);
    encoder.map(top_pairs).unwrap();
    encoder.u64(top_keys[0]).unwrap().str("index.html").unwrap();
    encoder
        .u64(top_keys[1])
        .unwrap()
        .array(resource_count)
        .unwrap();
    encoder.map(resource_pairs).unwrap();
    encoder.u64(resource_keys[0]).unwrap();
    match path {
        Some(path) => {
            encoder.str(path).unwrap();
        }
        None => {
            encoder.bytes(b"index.html").unwrap();
        }
    }
    encoder.u64(resource_keys[1]).unwrap();
    match content_type {
        Some(content_type) => {
            encoder.str(content_type).unwrap();
        }
        None => {
            encoder.bytes(b"text/html").unwrap();
        }
    }
    encoder.u64(resource_keys[2]).unwrap();
    if bytes_are_bytes {
        encoder.bytes(b"<html></html>").unwrap();
    } else {
        encoder.str("<html></html>").unwrap();
    }
    bytes
}

#[test]
fn bundle_decoder_rejects_each_structural_key_count_and_type_error() {
    let invalid = [
        raw_bundle(
            1,
            [0, 1],
            1,
            3,
            [0, 1, 2],
            Some("index.html"),
            Some("text/html"),
            true,
        ),
        raw_bundle(
            2,
            [1, 1],
            1,
            3,
            [0, 1, 2],
            Some("index.html"),
            Some("text/html"),
            true,
        ),
        raw_bundle(
            2,
            [0, 2],
            1,
            3,
            [0, 1, 2],
            Some("index.html"),
            Some("text/html"),
            true,
        ),
        raw_bundle(
            2,
            [0, 1],
            0,
            3,
            [0, 1, 2],
            Some("index.html"),
            Some("text/html"),
            true,
        ),
        raw_bundle(
            2,
            [0, 1],
            MAX_BUNDLE_RESOURCES as u64 + 1,
            3,
            [0, 1, 2],
            Some("index.html"),
            Some("text/html"),
            true,
        ),
        raw_bundle(
            2,
            [0, 1],
            1,
            2,
            [0, 1, 2],
            Some("index.html"),
            Some("text/html"),
            true,
        ),
        raw_bundle(
            2,
            [0, 1],
            1,
            3,
            [1, 1, 2],
            Some("index.html"),
            Some("text/html"),
            true,
        ),
        raw_bundle(
            2,
            [0, 1],
            1,
            3,
            [0, 2, 2],
            Some("index.html"),
            Some("text/html"),
            true,
        ),
        raw_bundle(
            2,
            [0, 1],
            1,
            3,
            [0, 1, 3],
            Some("index.html"),
            Some("text/html"),
            true,
        ),
        raw_bundle(2, [0, 1], 1, 3, [0, 1, 2], None, Some("text/html"), true),
        raw_bundle(2, [0, 1], 1, 3, [0, 1, 2], Some("index.html"), None, true),
        raw_bundle(
            2,
            [0, 1],
            1,
            3,
            [0, 1, 2],
            Some("index.html"),
            Some("text/html"),
            false,
        ),
    ];
    for bytes in invalid {
        assert_eq!(
            decode_app_bundle(&bytes),
            Err(AppsError::BundleFieldInvalid)
        );
    }
}

#[test]
fn bundle_decoder_rejects_empty_and_overlong_text_and_noncanonical_keys() {
    for path in ["", &"p".repeat(MAX_RESOURCE_PATH_BYTES + 1)] {
        let bytes = raw_bundle(
            2,
            [0, 1],
            1,
            3,
            [0, 1, 2],
            Some(path),
            Some("text/html"),
            true,
        );
        assert_eq!(
            decode_app_bundle(&bytes),
            Err(AppsError::BundleFieldInvalid)
        );
    }
    for content_type in ["", &"t".repeat(MAX_RESOURCE_CONTENT_TYPE_BYTES + 1)] {
        let bytes = raw_bundle(
            2,
            [0, 1],
            1,
            3,
            [0, 1, 2],
            Some("index.html"),
            Some(content_type),
            true,
        );
        assert_eq!(
            decode_app_bundle(&bytes),
            Err(AppsError::BundleFieldInvalid)
        );
    }

    let mut noncanonical = encode_app_bundle(&sample_bundle()).unwrap();
    noncanonical.splice(1..2, [0x18, 0x00]);
    assert_eq!(
        decode_app_bundle(&noncanonical),
        Err(AppsError::BundleFieldInvalid)
    );
}

#[test]
fn bundle_decoder_rejects_indefinite_resource_containers() {
    let encoded = encode_app_bundle(&sample_bundle()).unwrap();

    let array_header = encoded
        .windows(2)
        .position(|window| window == [0x01, 0x82])
        .expect("resource array header")
        + 1;
    let mut indefinite_array = encoded[..=array_header].to_vec();
    indefinite_array[array_header] = 0x9f;
    assert_eq!(
        decode_app_bundle(&indefinite_array),
        Err(AppsError::BundleFieldInvalid)
    );

    let map_header = encoded
        .windows(2)
        .position(|window| window == [0x82, 0xa3])
        .expect("resource map header")
        + 1;
    let mut indefinite_map = encoded[..=map_header].to_vec();
    indefinite_map[map_header] = 0xbf;
    assert_eq!(
        decode_app_bundle(&indefinite_map),
        Err(AppsError::BundleFieldInvalid)
    );
}
