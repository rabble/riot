use riot_core::profile::path::{
    classify_profile_path, profile_card_path, PROFILE_COMPONENT, PROFILE_PREFIX_COMPONENT_COUNT,
};
use riot_core::willow::Path;

#[test]
fn card_path_has_expected_shape() {
    let subspace = [7u8; 32];
    let path = profile_card_path(&subspace).expect("path");
    assert_eq!(
        path,
        Path::from_slices(&[PROFILE_COMPONENT, &subspace, b"card"]).expect("path")
    );
}

#[test]
fn classifier_accepts_exactly_the_card_slot() {
    let subspace = [7u8; 32];
    let path = profile_card_path(&subspace).expect("path");
    assert_eq!(classify_profile_path(&path), Some(subspace));
}

#[test]
fn classifier_rejects_every_malformed_shape() {
    let subspace = [7u8; 32];
    let short_id = [7u8; 31];

    // Bare prefix, no subspace.
    let bare = Path::from_slices(&[PROFILE_COMPONENT]).expect("path");
    assert_eq!(classify_profile_path(&bare), None);

    // Subspace but no slot.
    let no_slot = Path::from_slices(&[PROFILE_COMPONENT, &subspace]).expect("path");
    assert_eq!(classify_profile_path(&no_slot), None);

    // Wrong-length subspace.
    let short = Path::from_slices(&[PROFILE_COMPONENT, &short_id, b"card"]).expect("path");
    assert_eq!(classify_profile_path(&short), None);

    // Unknown slot name.
    let unknown = Path::from_slices(&[PROFILE_COMPONENT, &subspace, b"avatar"]).expect("path");
    assert_eq!(classify_profile_path(&unknown), None);

    // Extra trailing component.
    let extra =
        Path::from_slices(&[PROFILE_COMPONENT, &subspace, b"card", b"extra"]).expect("path");
    assert_eq!(classify_profile_path(&extra), None);

    // Different top-level family entirely.
    let other = Path::from_slices(&[b"apps", &subspace, b"card"]).expect("path");
    assert_eq!(classify_profile_path(&other), None);
}

#[test]
fn prefix_component_count_matches_the_built_path() {
    let path = profile_card_path(&[7u8; 32]).expect("path");
    assert_eq!(path.components().count(), PROFILE_PREFIX_COMPONENT_COUNT + 1);
}
