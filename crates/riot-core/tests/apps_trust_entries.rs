use riot_core::apps::trust::{
    decode_trust_marker, encode_trust_marker, trust_markers_for, write_trust_marker, TrustMarker,
    TrustMarkerKind,
};
use riot_core::session::RiotSession;
use riot_core::willow::generate_communal_author;

fn payload_marker(app_id: [u8; 32], kind: TrustMarkerKind) -> TrustMarker {
    TrustMarker {
        app_id,
        author_subspace_id: [0u8; 32],
        kind,
        timestamp_micros: 0,
    }
}

#[test]
fn trust_marker_codec_round_trips_trust_and_revoke() {
    for kind in [TrustMarkerKind::Trust, TrustMarkerKind::Revoke] {
        let marker = payload_marker([7u8; 32], kind);
        let encoded = encode_trust_marker(&marker).expect("encode");
        let decoded = decode_trust_marker(&encoded).expect("decode");
        assert_eq!(decoded.app_id, marker.app_id);
        assert_eq!(decoded.kind, marker.kind);
    }
}

#[test]
fn trust_marker_codec_rejects_tampering_truncation_and_trailing_bytes() {
    let encoded =
        encode_trust_marker(&payload_marker([7u8; 32], TrustMarkerKind::Trust)).expect("encode");

    let mut tampered = encoded.clone();
    tampered[1] = 2; // canonical key 0 becomes an unknown key
    assert!(decode_trust_marker(&tampered).is_err());
    assert!(decode_trust_marker(&encoded[..encoded.len() - 1]).is_err());

    let mut trailing = encoded;
    trailing.push(0);
    assert!(decode_trust_marker(&trailing).is_err());
}

#[test]
fn trust_marker_codec_rejects_unknown_kind() {
    let mut encoded =
        encode_trust_marker(&payload_marker([7u8; 32], TrustMarkerKind::Trust)).expect("encode");
    *encoded.last_mut().expect("kind byte") = 2;
    assert!(decode_trust_marker(&encoded).is_err());
}

#[test]
fn write_then_scan_derives_author_and_timestamp_from_the_entry() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let organizer = generate_communal_author().expect("organizer");
    let app_id = [7u8; 32];

    write_trust_marker(&store, &organizer, &app_id, TrustMarkerKind::Trust, 123)
        .expect("write trust");

    assert_eq!(
        trust_markers_for(&store, &app_id).expect("scan"),
        vec![TrustMarker {
            app_id,
            author_subspace_id: *organizer.subspace_id().as_bytes(),
            kind: TrustMarkerKind::Trust,
            timestamp_micros: 123,
        }]
    );
}

#[test]
fn later_revoke_replaces_trust_in_the_organizers_lww_slot() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let organizer = generate_communal_author().expect("organizer");
    let app_id = [7u8; 32];

    write_trust_marker(&store, &organizer, &app_id, TrustMarkerKind::Trust, 123)
        .expect("write trust");
    write_trust_marker(&store, &organizer, &app_id, TrustMarkerKind::Revoke, 124)
        .expect("write revoke");

    assert_eq!(
        trust_markers_for(&store, &app_id).expect("scan"),
        vec![TrustMarker {
            app_id,
            author_subspace_id: *organizer.subspace_id().as_bytes(),
            kind: TrustMarkerKind::Revoke,
            timestamp_micros: 124,
        }]
    );
}
