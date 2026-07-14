use riot_core::apps::entry::{
    app_data_path, build_app_data_entry, is_app_data_entry, is_app_data_path, APPS_COMPONENT,
};
use riot_core::apps::AppsError;
use riot_core::willow::{generate_communal_author, Entry, Path, WillowError};
use willow25::groupings::{Keylike, Namespaced};

#[test]
fn valid_key_builds_expected_path() {
    let app_id = [7u8; 32];
    let path = app_data_path(&app_id, "items/abc-123").expect("valid path");
    let expected =
        Path::from_slices(&[APPS_COMPONENT, &app_id, b"items", b"abc-123"]).expect("path");
    assert_eq!(path, expected);
}

#[test]
fn empty_key_is_rejected() {
    let app_id = [1u8; 32];
    assert_eq!(app_data_path(&app_id, ""), Err(AppsError::KeyEmpty));
}

#[test]
fn empty_segment_is_rejected() {
    let app_id = [1u8; 32];
    assert_eq!(
        app_data_path(&app_id, "items//x"),
        Err(AppsError::KeySegmentInvalid)
    );
}

#[test]
fn uppercase_or_traversal_like_segment_is_rejected() {
    let app_id = [1u8; 32];
    assert_eq!(
        app_data_path(&app_id, "../secret"),
        Err(AppsError::KeySegmentInvalid)
    );
    assert_eq!(
        app_data_path(&app_id, "Items/abc"),
        Err(AppsError::KeySegmentInvalid)
    );
}

#[test]
fn oversized_key_component_is_rejected() {
    let app_id = [1u8; 32];
    let long = "a".repeat(300);
    assert_eq!(
        app_data_path(&app_id, &long),
        Err(AppsError::PathComponentTooLong)
    );
}

#[test]
fn key_component_at_exactly_the_limit_is_accepted() {
    let app_id = [1u8; 32];
    // MAX_PATH_COMPONENT_BYTES = 256: exactly at the limit is fine.
    let at_limit = "a".repeat(256);
    let path = app_data_path(&app_id, &at_limit).expect("256-byte segment is accepted");
    let expected =
        Path::from_slices(&[APPS_COMPONENT, &app_id, at_limit.as_bytes()]).expect("path");
    assert_eq!(path, expected);
}

#[test]
fn too_many_key_segments_are_rejected() {
    let app_id = [1u8; 32];
    // 2 fixed components (apps, app_id) + 63 segments = 65 > MAX_PATH_COMPONENTS = 64.
    let key = ["a"; 63].join("/");
    assert_eq!(
        app_data_path(&app_id, &key),
        Err(AppsError::TooManyPathComponents)
    );
}

#[test]
fn total_path_bytes_over_the_limit_are_rejected() {
    let app_id = [1u8; 32];
    // Ten 250-byte segments: each under MAX_PATH_COMPONENT_BYTES and only 12
    // components, but 4 + 32 + 2500 = 2536 > MAX_PATH_TOTAL_BYTES = 2048.
    let segment = "a".repeat(250);
    let key = [segment.as_str(); 10].join("/");
    assert_eq!(app_data_path(&app_id, &key), Err(AppsError::PathTooLong));
}

#[test]
fn build_app_data_entry_signs_under_authors_own_namespace_and_subspace() {
    let author = generate_communal_author().expect("author");
    let app_id = [9u8; 32];
    let entry = build_app_data_entry(&author, &app_id, "items/x", 1, b"{}").expect("entry");
    assert_eq!(entry.namespace_id(), author.namespace_id());
    assert_eq!(*entry.subspace_id(), author.subspace_id());
    assert!(is_app_data_entry(&entry));
}

#[test]
fn app_data_classifier_rejects_every_malformed_path_family() {
    let app_id = [7u8; 32];
    let malformed = [
        Path::from_slices(&[]).unwrap(),
        Path::from_slices(&[b"objects"]).unwrap(),
        Path::from_slices(&[APPS_COMPONENT]).unwrap(),
        Path::from_slices(&[APPS_COMPONENT, &[7; 31], b"key"]).unwrap(),
        Path::from_slices(&[APPS_COMPONENT, &app_id]).unwrap(),
        Path::from_slices(&[APPS_COMPONENT, &app_id, b"Uppercase"]).unwrap(),
        Path::from_slices(&[APPS_COMPONENT, &app_id, b""]).unwrap(),
    ];
    for path in malformed {
        assert!(!is_app_data_path(&path), "accepted malformed path {path:?}");
    }
    assert!(is_app_data_path(
        &Path::from_slices(&[APPS_COMPONENT, &app_id, b"items", b"abc-123"]).unwrap()
    ));
}

#[test]
fn app_data_entry_classifier_delegates_to_the_entry_path() {
    let author = generate_communal_author().expect("author");
    let entry = Entry::builder()
        .namespace_id(author.namespace_id().clone())
        .subspace_id(author.subspace_id())
        .path(Path::from_slices(&[b"objects", b"not-app-data"]).unwrap())
        .timestamp(1)
        .payload(b"{}")
        .build();
    assert!(!is_app_data_entry(&entry));
}

#[test]
fn apps_error_display_and_willow_conversion_preserve_exact_variants() {
    let errors = [
        AppsError::KeyEmpty,
        AppsError::KeySegmentInvalid,
        AppsError::TooManyPathComponents,
        AppsError::PathComponentTooLong,
        AppsError::PathTooLong,
        AppsError::PathInvalid,
        AppsError::ManifestFieldInvalid,
        AppsError::BundleFieldInvalid,
        AppsError::BundleTooLarge,
        AppsError::StoreRejected,
        AppsError::StoreBusy,
        AppsError::StaleWrite,
        AppsError::IndexFieldInvalid,
        AppsError::EndorsementFieldInvalid,
        AppsError::IndexEntryMismatch,
    ];
    for error in errors {
        assert_eq!(error.to_string(), format!("{error:?}"));
    }

    let converted = AppsError::from(WillowError::PathInvalid);
    assert_eq!(converted, AppsError::Willow(WillowError::PathInvalid));
    assert_eq!(converted.to_string(), "Willow(PathInvalid)");
}
