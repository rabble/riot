// Reproduces the stage scenario: the organizer is auto-connected to a peer
// (a sync session is open) and taps "Let everyone in this space use this".
use riot_ffi::open_local_profile;

#[test]
fn organizer_can_approve_an_app_while_connected_to_a_peer() {
    let profile = open_local_profile().expect("profile");
    profile.create_public_space("Riverside".into()).expect("space");
    let runtime = profile.app_runtime();
    let app = runtime
        .directory_listings()
        .expect("listings")
        .into_iter()
        .next()
        .expect("the built-in checklist is listed");
    let app_id_hex: String = app.app_id.iter().map(|b| format!("{b:02x}")).collect();

    // Auto-connect: a peer is found, so a sync session is open.
    let _sync = profile.open_sync_session().expect("sync session");

    // The organizer approves. This must not fail just because a peer is nearby.
    runtime
        .trust_app(app_id_hex.clone())
        .expect("organizer approves the app while connected to a peer");
    assert!(runtime.is_app_trusted(app_id_hex).expect("trusted"));
}
