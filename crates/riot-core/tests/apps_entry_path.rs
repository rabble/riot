use riot_core::apps::entry::{app_data_path, build_app_data_entry, APPS_COMPONENT};
use riot_core::apps::AppsError;
use riot_core::willow::{generate_communal_author, Path};
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
fn build_app_data_entry_signs_under_authors_own_namespace_and_subspace() {
    let author = generate_communal_author().expect("author");
    let app_id = [9u8; 32];
    let entry = build_app_data_entry(&author, &app_id, "items/x", 1, b"{}").expect("entry");
    assert_eq!(entry.namespace_id(), author.namespace_id());
    assert_eq!(*entry.subspace_id(), author.subspace_id());
}
