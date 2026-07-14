//! the community property — an organizer approves once, and every member
//! (including one who joins later) gets the app, with no install step.
use riot_ffi::{
    open_local_profile, MobileError, MobileProfile, MobileSyncSession, SyncOutcomeKind,
};
use std::sync::Arc;

fn checklist() -> (Vec<u8>, Vec<u8>) {
    let m = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/apps/checklist.manifest.cbor"
    ))
    .unwrap();
    let b = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/apps/checklist.bundle.cbor"
    ))
    .unwrap();
    (m, b)
}
fn take_frame(s: &MobileSyncSession) -> Vec<u8> {
    s.take_outbound_frame().unwrap().unwrap()
}

fn sync(receiver: &Arc<MobileProfile>, sender: &Arc<MobileProfile>) {
    let i = receiver.open_sync_session().expect("i");
    let r = sender.open_sync_session().expect("r");
    i.begin().unwrap();
    r.receive_frame(take_frame(&i)).unwrap();
    i.receive_frame(take_frame(&r)).unwrap();
    r.receive_frame(take_frame(&i)).unwrap();
    let review = i.receive_frame(take_frame(&r)).unwrap();
    if review.kind == SyncOutcomeKind::ReviewImport {
        i.accept_import().unwrap();
        let ro = r.receive_frame(take_frame(&i)).unwrap();
        if !ro.terminal {
            i.receive_frame(take_frame(&r)).unwrap();
            let rev = r.receive_frame(take_frame(&i)).unwrap();
            if rev.kind == SyncOutcomeKind::ReviewImport {
                r.accept_import().unwrap();
            }
            i.receive_frame(take_frame(&r)).unwrap();
        }
    }
}

#[test]
fn organizer_approval_covers_a_member_who_joins_later() {
    let organizer = open_local_profile().expect("A");
    let space = organizer
        .create_public_space("Berlin Mutual Aid".into())
        .expect("space");
    let (m, b) = checklist();
    let app = organizer
        .app_runtime()
        .install_app(m.clone(), b.clone())
        .expect("install");
    organizer
        .app_runtime()
        .trust_app(app.app_id.clone())
        .expect("organizer approves");
    organizer
        .app_runtime()
        .app_data_put(
            app.app_id.clone(),
            "items/a".into(),
            b"{\"text\":\"Bring water\"}".to_vec(),
        )
        .expect("put");

    // B joins the same space and installs the same built-in checklist.
    let member = open_local_profile().expect("B");
    member.join_public_space(space).expect("join");
    let app_b = member
        .app_runtime()
        .install_app(m, b)
        .expect("install on B");
    assert_eq!(app_b.app_id, app.app_id);

    sync(&member, &organizer);

    let trusted = member
        .app_runtime()
        .is_app_trusted(app_b.app_id.clone())
        .unwrap();
    println!("member_trusted_after_sync={trusted}");
    assert!(
        trusted,
        "organizer's single approval must cover a member — no install step"
    );

    let data = member
        .app_runtime()
        .app_data_get(app_b.app_id, "items/a".into())
        .unwrap();
    println!(
        "member_sees_data={:?}",
        data.as_ref()
            .map(|d| String::from_utf8_lossy(d).to_string())
    );
    assert!(
        data.is_some(),
        "member must also see the organizer's checklist item"
    );
}

/// A profile made BEFORE spaces had organizers, reconstructed the way the app
/// reconstructs one: an old-scheme author (random namespace, random subspace —
/// not organizer-shaped) sealed and reopened. This is rabble's profile.
fn open_legacy_profile() -> Arc<MobileProfile> {
    let author = riot_core::willow::generate_communal_author().expect("legacy author");
    // The pre-organizer author is exactly what `is_space_organizer` cannot vouch
    // for, and what the old `InvalidInput` refusal hid behind a silent no-op.
    assert_ne!(
        author.subspace_id().as_bytes(),
        &author.identity().namespace_id,
        "a legacy author must NOT be organizer-shaped, or this test proves nothing"
    );
    let key = [7u8; 32];
    let sealed = author.seal_identity(&key).expect("seal");
    riot_ffi::open_profile_from_sealed_identity(key.to_vec(), sealed).expect("open legacy")
}

/// The exact bug rabble hit: he tapped "Let everyone here use this" on a space he
/// created, and it silently did nothing. The refusal is CORRECT (a legacy author
/// cannot prove it created the space), but it must not masquerade as a malformed
/// input — that is what left him with no idea why the app never appeared.
#[test]
fn a_legacy_profile_gets_a_distinct_error_never_invalid_input() {
    let legacy = open_legacy_profile();
    legacy
        .create_public_space("Riverside Tenants".into())
        .expect("space");
    let (m, b) = checklist();
    let app = legacy.app_runtime().install_app(m, b).expect("install");

    let err = legacy
        .app_runtime()
        .trust_app(app.app_id.clone())
        .expect_err("a legacy profile cannot approve — but it must SAY SO");
    println!("legacy_err={err:?}");

    assert!(
        matches!(err, MobileError::LegacyProfileCannotOrganize),
        "legacy profile must get its own error, got {err:?}"
    );
    assert!(
        !matches!(err, MobileError::InvalidInput),
        "InvalidInput is the overloaded code that made this failure silent"
    );
    // The gate still holds: a distinct message is not a way in.
    assert!(!legacy.app_runtime().is_app_trusted(app.app_id).unwrap());

    // And the UI can know NOT to offer the button in the first place.
    assert!(!legacy.app_runtime().is_organizer().unwrap());
    assert!(
        !legacy.app_runtime().can_organize().unwrap(),
        "a legacy profile can never organize any space — the honest advice is a new profile"
    );
}

/// The gate must not have been weakened while making it talk.
#[test]
fn an_organizer_shaped_profile_still_approves() {
    let organizer = open_local_profile().expect("A");
    organizer
        .create_public_space("Berlin Mutual Aid".into())
        .expect("space");
    let (m, b) = checklist();
    let app = organizer.app_runtime().install_app(m, b).expect("install");

    assert!(organizer.app_runtime().is_organizer().unwrap());
    assert!(organizer.app_runtime().can_organize().unwrap());
    organizer
        .app_runtime()
        .trust_app(app.app_id.clone())
        .expect("organizer still approves");
    assert!(organizer.app_runtime().is_app_trusted(app.app_id).unwrap());
}

/// A member is a DIFFERENT case from a legacy profile and needs opposite advice:
/// ask the organizer (which works) vs start a new profile (the only thing that
/// works). They are byte-identical in the author, so this pins the split.
#[test]
fn a_member_is_told_they_are_not_the_organizer_not_that_they_are_legacy() {
    let organizer = open_local_profile().expect("A");
    let space = organizer
        .create_public_space("Berlin Mutual Aid".into())
        .expect("space");
    let (m, b) = checklist();
    let member = open_local_profile().expect("B");
    member.join_public_space(space).expect("join");
    let app = member.app_runtime().install_app(m, b).expect("install");

    let err = member
        .app_runtime()
        .trust_app(app.app_id)
        .expect_err("member cannot approve");
    println!("member_err={err:?}");
    assert!(
        matches!(err, MobileError::NotSpaceOrganizer),
        "a member is not a legacy profile — telling them to start a new profile is a lie, got {err:?}"
    );

    assert!(
        !member.app_runtime().is_organizer().unwrap(),
        "button must not be offered"
    );
    assert!(
        member.app_runtime().can_organize().unwrap(),
        "a member's profile is perfectly capable of organizing — just not THIS space"
    );
}

/// An organizer who relaunches is restored through `join_public_space` (iOS
/// re-joins every persisted space, created or not). Keying member-ness off that
/// CALL rather than off the author regeneration would silently demote them.
#[test]
fn an_organizer_survives_the_relaunch_restore_path() {
    let organizer = open_local_profile().expect("A");
    let space = organizer
        .create_public_space("Berlin Mutual Aid".into())
        .expect("space");
    let key = [3u8; 32];
    let sealed = organizer.seal_identity(key.to_vec()).expect("seal");

    // The relaunch: a fresh profile off the sealed identity, then the persisted
    // space restored the ONLY way ProfileRepository restores one — join_public_space,
    // for a space it CREATED just the same as for one it joined.
    let relaunched =
        riot_ffi::open_profile_from_sealed_identity(key.to_vec(), sealed).expect("reopen");
    relaunched
        .join_public_space(space)
        .expect("restore own space");

    let (m, b) = checklist();
    let app = relaunched.app_runtime().install_app(m, b).expect("install");
    assert!(
        relaunched.app_runtime().is_organizer().unwrap(),
        "restoring your own space must not turn you into a member of it"
    );
    relaunched
        .app_runtime()
        .trust_app(app.app_id.clone())
        .expect("still the organizer after relaunch");
    assert!(relaunched.app_runtime().is_app_trusted(app.app_id).unwrap());
}

#[test]
fn a_member_cannot_self_approve_an_app() {
    let organizer = open_local_profile().expect("A");
    let space = organizer
        .create_public_space("Berlin Mutual Aid".into())
        .expect("space");
    let (m, b) = checklist();
    let member = open_local_profile().expect("B");
    member.join_public_space(space).expect("join");
    let app = member.app_runtime().install_app(m, b).expect("install");
    let self_approve = member.app_runtime().trust_app(app.app_id.clone());
    let trusted = member
        .app_runtime()
        .is_app_trusted(app.app_id)
        .unwrap_or(false);
    println!(
        "self_approve_err={:?} trusted={trusted}",
        self_approve.is_err()
    );
    assert!(
        self_approve.is_err(),
        "a member is not an organizer and must not be able to approve"
    );
    assert!(!trusted, "self-approval must not grant trust");
}
