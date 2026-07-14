use riot_core::apps::manifest::{
    app_id_for, decode_manifest, encode_manifest, AppManifest, MAX_APP_DESCRIPTION_BYTES,
    MAX_APP_ENTRY_POINT_BYTES, MAX_APP_NAME_BYTES, MAX_APP_PERMISSIONS, MAX_APP_PERMISSION_BYTES,
    MAX_APP_VERSION_BYTES,
};
use riot_core::apps::AppsError;
use riot_core::willow::{generate_communal_author, NamespaceKind};

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

#[test]
fn app_id_rejects_an_invalid_manifest() {
    let author = generate_communal_author().expect("author");
    let mut manifest = sample_manifest(author.identity());
    manifest.name.clear();
    assert_eq!(
        app_id_for(&manifest, &[1; 32]),
        Err(AppsError::ManifestFieldInvalid)
    );
}

#[test]
fn manifest_validation_enforces_every_text_and_permission_boundary() {
    let author = generate_communal_author().expect("author");
    let base = sample_manifest(author.identity());

    let mut invalid = base.clone();
    invalid.name.clear();
    assert_eq!(
        encode_manifest(&invalid),
        Err(AppsError::ManifestFieldInvalid)
    );
    invalid = base.clone();
    invalid.name = "x".repeat(MAX_APP_NAME_BYTES + 1);
    assert_eq!(
        encode_manifest(&invalid),
        Err(AppsError::ManifestFieldInvalid)
    );

    invalid = base.clone();
    invalid.description.clear();
    assert_eq!(
        encode_manifest(&invalid),
        Err(AppsError::ManifestFieldInvalid)
    );

    invalid = base.clone();
    invalid.version.clear();
    assert_eq!(
        encode_manifest(&invalid),
        Err(AppsError::ManifestFieldInvalid)
    );
    invalid = base.clone();
    invalid.version = "x".repeat(MAX_APP_VERSION_BYTES + 1);
    assert_eq!(
        encode_manifest(&invalid),
        Err(AppsError::ManifestFieldInvalid)
    );

    invalid = base.clone();
    invalid.entry_point.clear();
    assert_eq!(
        encode_manifest(&invalid),
        Err(AppsError::ManifestFieldInvalid)
    );
    invalid = base.clone();
    invalid.entry_point = "x".repeat(MAX_APP_ENTRY_POINT_BYTES + 1);
    assert_eq!(
        encode_manifest(&invalid),
        Err(AppsError::ManifestFieldInvalid)
    );

    invalid = base.clone();
    invalid.permissions = vec!["p".to_string(); MAX_APP_PERMISSIONS + 1];
    assert_eq!(
        encode_manifest(&invalid),
        Err(AppsError::ManifestFieldInvalid)
    );
    invalid = base.clone();
    invalid.permissions = vec![String::new()];
    assert_eq!(
        encode_manifest(&invalid),
        Err(AppsError::ManifestFieldInvalid)
    );
    invalid.permissions = vec!["p".repeat(MAX_APP_PERMISSION_BYTES + 1)];
    assert_eq!(
        encode_manifest(&invalid),
        Err(AppsError::ManifestFieldInvalid)
    );

    let mut exact = base;
    exact.name = "n".repeat(MAX_APP_NAME_BYTES);
    exact.description = "d".repeat(MAX_APP_DESCRIPTION_BYTES);
    exact.version = "v".repeat(MAX_APP_VERSION_BYTES);
    exact.entry_point = "e".repeat(MAX_APP_ENTRY_POINT_BYTES);
    exact.permissions = vec!["p".repeat(MAX_APP_PERMISSION_BYTES); MAX_APP_PERMISSIONS];
    assert_eq!(
        decode_manifest(&encode_manifest(&exact).unwrap()).unwrap(),
        exact
    );
}

fn raw_manifest(
    keys: [u64; 9],
    kind: u8,
    id_len: usize,
    permissions_count: u64,
    name: Option<&str>,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = minicbor::Encoder::new(&mut bytes);
    encoder.map(9).unwrap();
    encoder.u64(keys[0]).unwrap();
    match name {
        Some(name) => {
            encoder.str(name).unwrap();
        }
        None => {
            encoder.bytes(b"not text").unwrap();
        }
    }
    encoder.u64(keys[1]).unwrap().str("description").unwrap();
    encoder.u64(keys[2]).unwrap().str("1.0.0").unwrap();
    encoder
        .u64(keys[3])
        .unwrap()
        .bytes(&vec![2; id_len])
        .unwrap();
    encoder
        .u64(keys[4])
        .unwrap()
        .bytes(&vec![3; id_len])
        .unwrap();
    encoder.u64(keys[5]).unwrap().u8(kind).unwrap();
    encoder
        .u64(keys[6])
        .unwrap()
        .bytes(&vec![4; id_len])
        .unwrap();
    encoder
        .u64(keys[7])
        .unwrap()
        .array(permissions_count)
        .unwrap();
    for _ in 0..permissions_count.min(MAX_APP_PERMISSIONS as u64) {
        encoder.str("own-app-data").unwrap();
    }
    encoder.u64(keys[8]).unwrap().str("index.html").unwrap();
    bytes
}

#[test]
fn manifest_decoder_rejects_order_unknown_kind_id_and_permission_count_errors() {
    let invalid = [
        raw_manifest([0, 0, 2, 3, 4, 5, 6, 7, 8], 0, 32, 1, Some("name")),
        raw_manifest([0, 1, 2, 3, 4, 5, 6, 7, 9], 0, 32, 1, Some("name")),
        raw_manifest([0, 1, 2, 3, 4, 5, 6, 7, 8], 2, 32, 1, Some("name")),
        raw_manifest([0, 1, 2, 3, 4, 5, 6, 7, 8], 0, 31, 1, Some("name")),
        raw_manifest(
            [0, 1, 2, 3, 4, 5, 6, 7, 8],
            0,
            32,
            MAX_APP_PERMISSIONS as u64 + 1,
            Some("name"),
        ),
        raw_manifest([0, 1, 2, 3, 4, 5, 6, 7, 8], 0, 32, 1, None),
    ];
    for bytes in invalid {
        assert_eq!(
            decode_manifest(&bytes),
            Err(AppsError::ManifestFieldInvalid)
        );
    }
}

#[test]
fn manifest_decoder_rejects_empty_overlong_and_noncanonical_text() {
    for name in ["", &"n".repeat(MAX_APP_NAME_BYTES + 1)] {
        let bytes = raw_manifest([0, 1, 2, 3, 4, 5, 6, 7, 8], 0, 32, 1, Some(name));
        assert_eq!(
            decode_manifest(&bytes),
            Err(AppsError::ManifestFieldInvalid)
        );
    }

    let author = generate_communal_author().expect("author");
    let mut noncanonical = encode_manifest(&sample_manifest(author.identity())).unwrap();
    noncanonical.splice(1..2, [0x18, 0x00]);
    assert_eq!(
        decode_manifest(&noncanonical),
        Err(AppsError::ManifestFieldInvalid)
    );
}

#[test]
fn manifest_round_trips_owned_namespace_kind() {
    let author = generate_communal_author().expect("author");
    let mut manifest = sample_manifest(author.identity());
    manifest.author.namespace_kind = NamespaceKind::Owned;
    assert_eq!(
        decode_manifest(&encode_manifest(&manifest).unwrap()).unwrap(),
        manifest
    );
}

#[test]
fn manifest_decoder_rejects_an_indefinite_permissions_array() {
    let encoded = raw_manifest([0, 1, 2, 3, 4, 5, 6, 7, 8], 0, 32, 1, Some("name"));
    let array_header = encoded
        .windows(2)
        .position(|window| window == [0x07, 0x81])
        .expect("permissions array header")
        + 1;
    let mut indefinite = encoded[..=array_header].to_vec();
    indefinite[array_header] = 0x9f;
    assert_eq!(
        decode_manifest(&indefinite),
        Err(AppsError::ManifestFieldInvalid)
    );
}
