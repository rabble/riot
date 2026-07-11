use riot_core::apps::trust::{is_trusted, TrustMarker, TrustMarkerKind};
use riot_core::willow::identity::EvidenceAuthor;

fn subspace_of(author: &EvidenceAuthor) -> [u8; 32] {
    *author.subspace_id().as_bytes()
}

#[test]
fn no_markers_means_not_trusted() {
    let organizer = riot_core::willow::generate_communal_author().expect("author");
    let app_id = [1u8; 32];
    assert!(!is_trusted(&app_id, &[], &[subspace_of(&organizer)]));
}

#[test]
fn organizer_trust_marker_grants_trust() {
    let organizer = riot_core::willow::generate_communal_author().expect("author");
    let app_id = [1u8; 32];
    let markers = vec![TrustMarker {
        app_id,
        author_subspace_id: subspace_of(&organizer),
        kind: TrustMarkerKind::Trust,
        timestamp_micros: 10,
    }];
    assert!(is_trusted(&app_id, &markers, &[subspace_of(&organizer)]));
}

#[test]
fn non_organizer_trust_marker_is_ignored() {
    let non_organizer = riot_core::willow::generate_communal_author().expect("author");
    let organizer = riot_core::willow::generate_communal_author().expect("author");
    let app_id = [1u8; 32];
    let markers = vec![TrustMarker {
        app_id,
        author_subspace_id: subspace_of(&non_organizer),
        kind: TrustMarkerKind::Trust,
        timestamp_micros: 10,
    }];
    assert!(!is_trusted(&app_id, &markers, &[subspace_of(&organizer)]));
}

#[test]
fn duplicate_same_organizer_markers_fail_closed_with_different_timestamps() {
    let organizer = riot_core::willow::generate_communal_author().expect("author");
    let app_id = [1u8; 32];
    let mut markers = vec![
        TrustMarker {
            app_id,
            author_subspace_id: subspace_of(&organizer),
            kind: TrustMarkerKind::Trust,
            timestamp_micros: 20,
        },
        TrustMarker {
            app_id,
            author_subspace_id: subspace_of(&organizer),
            kind: TrustMarkerKind::Revoke,
            timestamp_micros: 10,
        },
    ];
    assert!(!is_trusted(&app_id, &markers, &[subspace_of(&organizer)]));
    markers.reverse();
    assert!(!is_trusted(&app_id, &markers, &[subspace_of(&organizer)]));
}

#[test]
fn newer_trust_from_one_organizer_beats_older_revoke_from_another() {
    let trusting_organizer = riot_core::willow::generate_communal_author().expect("author");
    let revoking_organizer = riot_core::willow::generate_communal_author().expect("author");
    let app_id = [1u8; 32];
    let markers = vec![
        TrustMarker {
            app_id,
            author_subspace_id: subspace_of(&revoking_organizer),
            kind: TrustMarkerKind::Revoke,
            timestamp_micros: 10,
        },
        TrustMarker {
            app_id,
            author_subspace_id: subspace_of(&trusting_organizer),
            kind: TrustMarkerKind::Trust,
            timestamp_micros: 20,
        },
    ];
    assert!(is_trusted(
        &app_id,
        &markers,
        &[
            subspace_of(&trusting_organizer),
            subspace_of(&revoking_organizer),
        ]
    ));
}

#[test]
fn revoke_wins_equal_timestamp_across_different_organizers() {
    let trusting_organizer = riot_core::willow::generate_communal_author().expect("author");
    let revoking_organizer = riot_core::willow::generate_communal_author().expect("author");
    let app_id = [1u8; 32];
    let trust = TrustMarker {
        app_id,
        author_subspace_id: subspace_of(&trusting_organizer),
        kind: TrustMarkerKind::Trust,
        timestamp_micros: 10,
    };
    let revoke = TrustMarker {
        app_id,
        author_subspace_id: subspace_of(&revoking_organizer),
        kind: TrustMarkerKind::Revoke,
        timestamp_micros: 10,
    };
    assert!(!is_trusted(
        &app_id,
        &[trust, revoke],
        &[
            subspace_of(&trusting_organizer),
            subspace_of(&revoking_organizer),
        ]
    ));
    assert!(!is_trusted(
        &app_id,
        &[revoke, trust],
        &[
            subspace_of(&trusting_organizer),
            subspace_of(&revoking_organizer),
        ]
    ));
}

#[test]
fn duplicate_same_organizer_equal_timestamp_fails_closed_in_either_order() {
    let organizer = riot_core::willow::generate_communal_author().expect("author");
    let app_id = [1u8; 32];
    let trust = TrustMarker {
        app_id,
        author_subspace_id: subspace_of(&organizer),
        kind: TrustMarkerKind::Trust,
        timestamp_micros: 10,
    };
    let revoke = TrustMarker {
        kind: TrustMarkerKind::Revoke,
        ..trust
    };
    assert!(!is_trusted(
        &app_id,
        &[trust, revoke],
        &[subspace_of(&organizer)]
    ));
    assert!(!is_trusted(
        &app_id,
        &[revoke, trust],
        &[subspace_of(&organizer)]
    ));
}

#[test]
fn irrelevant_and_unrecognized_duplicates_do_not_poison_valid_trust() {
    let organizer = riot_core::willow::generate_communal_author().expect("author");
    let outsider = riot_core::willow::generate_communal_author().expect("author");
    let app_id = [1u8; 32];
    let valid = TrustMarker {
        app_id,
        author_subspace_id: subspace_of(&organizer),
        kind: TrustMarkerKind::Trust,
        timestamp_micros: 10,
    };
    let outsider_marker = TrustMarker {
        author_subspace_id: subspace_of(&outsider),
        ..valid
    };
    let irrelevant = TrustMarker {
        app_id: [2u8; 32],
        ..valid
    };

    assert!(is_trusted(
        &app_id,
        &[
            valid,
            outsider_marker,
            outsider_marker,
            irrelevant,
            irrelevant
        ],
        &[subspace_of(&organizer)]
    ));
}
