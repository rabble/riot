//! FFI contract for minimal profiles: claiming a display name, rendering it,
//! and resolving any subspace id to something showable — end-to-end through the
//! UniFFI layer, in-process, same as `apps_contract.rs`.
//!
//! Two properties here are load-bearing beyond the obvious ones:
//!
//! * **No bare claimed name ever crosses the boundary.** A self-claimed name
//!   shown without its key tag is precisely the impersonation the tag exists to
//!   blunt, so every rendered string is asserted to carry one.
//! * **The id is what's stable, not the name.** `whoami().id` survives a rename;
//!   the name does not. Apps store the id — a stored name is a snapshot that a
//!   later rename can never repair.

use riot_ffi::{
    open_local_profile, MobileError, MobileProfile, MobileSyncSession, SyncOutcomeKind,
};
use std::sync::Arc;

use riot_core::profile::card::MAX_DISPLAY_NAME_BYTES;
use riot_core::profile::resolver::key_tag;

fn subspace_id(profile: &MobileProfile) -> [u8; 32] {
    profile
        .profile()
        .whoami()
        .expect("whoami")
        .id
        .try_into()
        .expect("32-byte subspace id")
}

/// The tag every rendered name must carry, derived the one sanctioned way.
fn tag_of(profile: &MobileProfile) -> String {
    key_tag(&subspace_id(profile))
}

fn take_frame(session: &MobileSyncSession) -> Vec<u8> {
    session
        .take_outbound_frame()
        .expect("take outbound frame")
        .expect("sync outcome queued a frame")
}

fn sync_to_review(
    receiver: &Arc<MobileProfile>,
    sender: &Arc<MobileProfile>,
) -> (
    Arc<MobileSyncSession>,
    Arc<MobileSyncSession>,
    riot_ffi::SyncOutcome,
) {
    let initiator = receiver.open_sync_session().expect("receiver sync");
    let responder = sender.open_sync_session().expect("sender sync");
    initiator.begin().expect("begin");
    responder
        .receive_frame(take_frame(&initiator))
        .expect("receive hello");
    initiator
        .receive_frame(take_frame(&responder))
        .expect("receive summary");
    responder
        .receive_frame(take_frame(&initiator))
        .expect("receive request");
    let review = initiator
        .receive_frame(take_frame(&responder))
        .expect("receive entries");
    (initiator, responder, review)
}

fn accept_and_finish(initiator: &MobileSyncSession, responder: &MobileSyncSession) {
    assert_eq!(
        initiator.accept_import().expect("accept import").kind,
        SyncOutcomeKind::FrameReady
    );
    let responder_outcome = responder
        .receive_frame(take_frame(initiator))
        .expect("receive post-import summary");
    if !responder_outcome.terminal {
        initiator
            .receive_frame(take_frame(responder))
            .expect("send reverse entries");
        let reverse_review = responder
            .receive_frame(take_frame(initiator))
            .expect("review reverse entries");
        assert_eq!(reverse_review.kind, SyncOutcomeKind::ReviewImport);
        assert_eq!(
            responder
                .accept_import()
                .expect("accept reverse import")
                .kind,
            SyncOutcomeKind::FrameReady
        );
    }
    assert_eq!(
        initiator
            .receive_frame(take_frame(responder))
            .expect("receive complete")
            .kind,
        SyncOutcomeKind::Complete
    );
}

#[test]
fn a_claimed_name_renders_with_its_key_tag_never_bare() {
    let profile = open_local_profile().expect("profile");
    let session = profile.profile();
    session.set_display_name("Ana".into()).expect("set name");

    let tag = tag_of(&profile);
    assert_eq!(session.my_display_name().unwrap(), format!("Ana · {tag}"));
    // The bare claim is never the whole answer.
    assert_ne!(session.my_display_name().unwrap(), "Ana");
}

#[test]
fn a_person_with_no_name_renders_in_the_same_shape_as_everyone_else() {
    let profile = open_local_profile().expect("profile");
    let tag = tag_of(&profile);
    assert_eq!(
        profile.profile().my_display_name().unwrap(),
        format!("member · {tag}")
    );
}

#[test]
fn the_codec_is_the_single_enforcement_point_for_a_names_bounds() {
    let profile = open_local_profile().expect("profile");
    let session = profile.profile();

    assert!(matches!(
        session.set_display_name(String::new()),
        Err(MobileError::InvalidInput)
    ));
    let too_long = "a".repeat(MAX_DISPLAY_NAME_BYTES + 1);
    assert!(matches!(
        session.set_display_name(too_long),
        Err(MobileError::InvalidInput)
    ));

    // Exactly at the cap is fine — the boundary is inclusive.
    let at_cap = "a".repeat(MAX_DISPLAY_NAME_BYTES);
    session.set_display_name(at_cap.clone()).expect("at cap");
    assert!(session.my_display_name().unwrap().starts_with(&at_cap));

    // A rejected name changed nothing.
    assert!(matches!(
        session.set_display_name(String::new()),
        Err(MobileError::InvalidInput)
    ));
    assert!(session.my_display_name().unwrap().starts_with(&at_cap));
}

/// The test that proves the id-not-name design. An app that stores `whoami().id`
/// keeps working across a rename; one that stored the name would be frozen at
/// the old one forever.
#[test]
fn whoami_id_is_stable_across_a_rename_while_the_name_changes() {
    let profile = open_local_profile().expect("profile");
    let session = profile.profile();

    let before = session.whoami().expect("whoami before");
    assert_eq!(before.display_name, "member", "no name claimed yet");

    session.set_display_name("Ana".into()).expect("set Ana");
    let as_ana = session.whoami().expect("whoami as Ana");

    session.set_display_name("Ana R".into()).expect("rename");
    let renamed = session.whoami().expect("whoami renamed");

    // The id never moves — through the first claim and through the rename.
    assert_eq!(before.id, as_ana.id);
    assert_eq!(as_ana.id, renamed.id);
    assert_eq!(before.tag, renamed.tag);
    assert_eq!(renamed.id.len(), 32);

    // The name does.
    assert_eq!(as_ana.display_name, "Ana");
    assert_eq!(renamed.display_name, "Ana R");

    // And resolving the SAME id now yields the new name — which is exactly what
    // lets an app repair every historical row it ever attributed to this id.
    let resolved = session.profile_for(renamed.id.clone()).expect("resolve");
    assert_eq!(resolved.display_name, "Ana R");
}

#[test]
fn an_unknown_id_resolves_to_the_fallback_and_a_malformed_one_is_an_error() {
    let profile = open_local_profile().expect("profile");
    let session = profile.profile();
    session.set_display_name("Ana".into()).expect("set name");

    // A well-formed id this device has never seen a card for. An app has to be
    // able to draw a row authored by someone whose profile has not synced yet,
    // so this is NOT an error.
    let stranger = vec![7u8; 32];
    let resolved = session
        .profile_for(stranger.clone())
        .expect("unknown id resolves, never errors");
    assert_eq!(resolved.display_name, "member");
    assert_eq!(resolved.id, stranger);
    assert_eq!(resolved.tag, "07070707");

    // A wrong-length id is a caller bug and IS an error.
    assert!(matches!(
        session.profile_for(vec![7u8; 8]),
        Err(MobileError::InvalidInput)
    ));
    assert!(matches!(
        session.profile_for(Vec::new()),
        Err(MobileError::InvalidInput)
    ));
    assert!(matches!(
        session.profile_for(vec![7u8; 33]),
        Err(MobileError::InvalidInput)
    ));
}

/// A profile write commits through `store.inspect`, which replaces the
/// session-wide preview slot an in-flight sync review holds. It carries the same
/// guard `app_data_put` does — and, critically, refusing the write must leave
/// sync working rather than wedging it.
#[test]
fn a_name_cannot_be_claimed_mid_sync_and_refusing_it_does_not_brick_sync() {
    let profile = open_local_profile().expect("profile");
    profile.create_public_space("Guard".into()).expect("space");
    let session = profile.profile();

    let sync = profile.open_sync_session().expect("open sync");
    assert!(matches!(
        session.set_display_name("Ana".into()),
        Err(MobileError::InvalidInput)
    ));
    sync.cancel().expect("cancel");

    // Sync still opens afterwards — the refused write left no wreckage.
    profile
        .open_sync_session()
        .expect("sync still opens after a refused profile write")
        .cancel()
        .expect("cancel");

    // And the name can be claimed once sync is closed.
    session.set_display_name("Ana".into()).expect("set name");
    let tag = tag_of(&profile);
    assert_eq!(session.my_display_name().unwrap(), format!("Ana · {tag}"));

    // A committed profile write ALSO leaves sync openable: the entry must be in
    // the sync inventory, or `ensure_complete_sync_inventory` fails forever.
    profile
        .open_sync_session()
        .expect("sync still opens after a committed profile write")
        .cancel()
        .expect("cancel");
}

#[test]
fn app_display_name_is_the_rendered_name() {
    let profile = open_local_profile().expect("profile");
    let runtime = profile.app_runtime();
    let tag = tag_of(&profile);

    // Before any claim: the same shape as everyone else, not a `member-<hex>` label.
    assert_eq!(
        runtime.app_display_name().unwrap(),
        format!("member · {tag}")
    );

    profile
        .profile()
        .set_display_name("Ana".into())
        .expect("set name");

    // This is what `riot.whoami()` reads — a real name, rendered.
    assert_eq!(runtime.app_display_name().unwrap(), format!("Ana · {tag}"));
    assert_eq!(
        runtime.app_display_name().unwrap(),
        profile.profile().my_display_name().unwrap()
    );
}

/// One slot per person, last-write-wins. Two claims in immediate succession land
/// in the same wall-clock second, so this also pins the write floor: without it
/// the second claim would carry an equal Willow timestamp, lose to the first,
/// and the rename would silently do nothing.
#[test]
fn a_second_claim_replaces_the_first_rather_than_adding_a_slot() {
    let profile = open_local_profile().expect("profile");
    let session = profile.profile();

    session.set_display_name("Ana".into()).expect("first");
    session.set_display_name("Bea".into()).expect("second");
    session.set_display_name("Cy".into()).expect("third");

    let tag = tag_of(&profile);
    assert_eq!(session.my_display_name().unwrap(), format!("Cy · {tag}"));

    let names = session.display_names().expect("display names");
    assert_eq!(names.len(), 1, "one slot per person, not three");
    assert_eq!(names[0].rendered, format!("Cy · {tag}"));
    assert_eq!(names[0].subspace_id, subspace_id(&profile).to_vec());
}

/// The payoff: a name claimed on one device reaches another over nearby sync and
/// resolves there. This is what the sync-inventory discipline in
/// `set_display_name` buys — a profile card written outside that pipeline would
/// sit in the local store and never be offered to a peer at all.
#[test]
fn a_claimed_name_syncs_to_a_peer_and_resolves_there() {
    let sender = open_local_profile().expect("sender");
    let space = sender.create_public_space("Names".into()).expect("space");
    sender
        .profile()
        .set_display_name("Ana".into())
        .expect("set name");
    let ana_id = subspace_id(&sender).to_vec();

    let receiver = open_local_profile().expect("receiver");
    receiver.join_public_space(space, Vec::new()).expect("join");

    // Before sync the receiver has never heard of Ana — and still renders her.
    let unknown = receiver
        .profile()
        .profile_for(ana_id.clone())
        .expect("unsynced author still resolves");
    assert_eq!(unknown.display_name, "member");

    let (initiator, responder, _) = sync_to_review(&receiver, &sender);
    accept_and_finish(&initiator, &responder);

    let resolved = receiver
        .profile()
        .profile_for(ana_id.clone())
        .expect("resolve after sync");
    assert_eq!(resolved.display_name, "Ana");
    assert_eq!(resolved.id, ana_id);

    let rendered = format!("Ana · {}", key_tag(&subspace_id(&sender)));
    let names = receiver.profile().display_names().expect("display names");
    assert!(
        names
            .iter()
            .any(|row| row.subspace_id == ana_id && row.rendered == rendered),
        "the peer's name is listed, rendered: {names:?}"
    );

    // The receiver's own name is untouched by someone else's card.
    assert_eq!(receiver.profile().whoami().unwrap().display_name, "member");
}

/// A profile card is a live entry that is not an alert. The alert listing must
/// skip it — otherwise a live entry with no match in the alert table bricks the
/// whole feed with `Internal` the moment anyone claims a name.
#[test]
fn claiming_a_name_does_not_brick_the_alert_listing() {
    let profile = open_local_profile().expect("profile");
    profile.create_public_space("Feed".into()).expect("space");

    assert!(profile
        .list_current_entries()
        .expect("listing before")
        .is_empty());

    profile
        .profile()
        .set_display_name("Ana".into())
        .expect("set name");

    // The profile card is NOT an alert, and must not appear as a phantom row.
    assert!(
        profile
            .list_current_entries()
            .expect("the alert listing survives a profile write")
            .is_empty(),
        "a profile card must never surface as an alert row"
    );
}
