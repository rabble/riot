//! the community property — an organizer approves once, and every member
//! (including one who joins later) gets the app, with no install step.
use riot_ffi::{open_local_profile, MobileProfile, MobileSyncSession, SyncOutcomeKind};
use std::sync::Arc;

fn checklist() -> (Vec<u8>, Vec<u8>) {
    let m = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/apps/checklist.manifest.cbor")).unwrap();
    let b = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/apps/checklist.bundle.cbor")).unwrap();
    (m, b)
}
fn take_frame(s: &MobileSyncSession) -> Vec<u8> { s.take_outbound_frame().unwrap().unwrap() }

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
            if rev.kind == SyncOutcomeKind::ReviewImport { r.accept_import().unwrap(); }
            i.receive_frame(take_frame(&r)).unwrap();
        }
    }


}

#[test]
fn organizer_approval_covers_a_member_who_joins_later() {
    let organizer = open_local_profile().expect("A");
    let space = organizer.create_public_space("Berlin Mutual Aid".into()).expect("space");
    let (m, b) = checklist();
    let app = organizer.app_runtime().install_app(m.clone(), b.clone()).expect("install");
    organizer.app_runtime().trust_app(app.app_id.clone()).expect("organizer approves");
    organizer.app_runtime().app_data_put(app.app_id.clone(), "items/a".into(), b"{\"text\":\"Bring water\"}".to_vec()).expect("put");

    // B joins the same space and installs the same built-in checklist.
    let member = open_local_profile().expect("B");
    member.join_public_space(space).expect("join");
    let app_b = member.app_runtime().install_app(m, b).expect("install on B");
    assert_eq!(app_b.app_id, app.app_id);

    sync(&member, &organizer);

    let trusted = member.app_runtime().is_app_trusted(app_b.app_id.clone()).unwrap();
    println!("member_trusted_after_sync={trusted}");
    assert!(trusted, "organizer's single approval must cover a member — no install step");

    let data = member.app_runtime().app_data_get(app_b.app_id, "items/a".into()).unwrap();
    println!("member_sees_data={:?}", data.as_ref().map(|d| String::from_utf8_lossy(d).to_string()));
    assert!(data.is_some(), "member must also see the organizer's checklist item");
}

#[test]
fn a_member_cannot_self_approve_an_app() {
    let organizer = open_local_profile().expect("A");
    let space = organizer.create_public_space("Berlin Mutual Aid".into()).expect("space");
    let (m, b) = checklist();
    let member = open_local_profile().expect("B");
    member.join_public_space(space).expect("join");
    let app = member.app_runtime().install_app(m, b).expect("install");
    let self_approve = member.app_runtime().trust_app(app.app_id.clone());
    let trusted = member.app_runtime().is_app_trusted(app.app_id).unwrap_or(false);
    println!("self_approve_err={:?} trusted={trusted}", self_approve.is_err());
    assert!(self_approve.is_err(), "a member is not an organizer and must not be able to approve");
    assert!(!trusted, "self-approval must not grant trust");
}
