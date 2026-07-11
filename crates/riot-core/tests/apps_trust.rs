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
fn newer_revoke_overrides_older_trust_from_same_organizer() {
    let organizer = riot_core::willow::generate_communal_author().expect("author");
    let app_id = [1u8; 32];
    let markers = vec![
        TrustMarker {
            app_id,
            author_subspace_id: subspace_of(&organizer),
            kind: TrustMarkerKind::Trust,
            timestamp_micros: 10,
        },
        TrustMarker {
            app_id,
            author_subspace_id: subspace_of(&organizer),
            kind: TrustMarkerKind::Revoke,
            timestamp_micros: 20,
        },
    ];
    assert!(!is_trusted(&app_id, &markers, &[subspace_of(&organizer)]));
}

#[test]
fn older_revoke_does_not_override_newer_trust() {
    let organizer = riot_core::willow::generate_communal_author().expect("author");
    let app_id = [1u8; 32];
    let markers = vec![
        TrustMarker {
            app_id,
            author_subspace_id: subspace_of(&organizer),
            kind: TrustMarkerKind::Revoke,
            timestamp_micros: 10,
        },
        TrustMarker {
            app_id,
            author_subspace_id: subspace_of(&organizer),
            kind: TrustMarkerKind::Trust,
            timestamp_micros: 20,
        },
    ];
    assert!(is_trusted(&app_id, &markers, &[subspace_of(&organizer)]));
}
