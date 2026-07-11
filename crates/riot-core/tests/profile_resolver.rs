//! Writing a profile card, resolving everyone's names back out of the store,
//! and the one sanctioned rendering rule: a self-claimed name is never shown
//! bare — it always carries its key-derived suffix.

use riot_core::profile::card::ProfileCard;
use riot_core::profile::resolver::{render_display_name, resolve_display_names, write_profile_card};
use riot_core::session::RiotSession;
use riot_core::willow::generate_communal_author;

#[test]
fn render_always_appends_the_key_suffix() {
    let subspace = [
        0xa3, 0xf9, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    assert_eq!(render_display_name(Some("Ana"), &subspace), "Ana · a3f91122");
}

#[test]
fn render_falls_back_to_member_for_an_unknown_subspace() {
    let subspace = [
        0xa3, 0xf9, 0x11, 0x22, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0,
    ];
    assert_eq!(render_display_name(None, &subspace), "member · a3f91122");
}

#[test]
fn same_name_different_key_never_collides_in_rendering() {
    let a = [0xaa; 32];
    let b = [0xbb; 32];
    assert_ne!(
        render_display_name(Some("Ana"), &a),
        render_display_name(Some("Ana"), &b)
    );
}

#[test]
fn write_then_resolve_round_trips() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let author = generate_communal_author().expect("author");

    write_profile_card(
        &store,
        &author,
        &ProfileCard {
            display_name: "Ana".into(),
        },
        1,
    )
    .expect("write");

    let names = resolve_display_names(&store).expect("resolve");
    let subspace = *author.subspace_id().as_bytes();
    assert_eq!(names.get(&subspace).map(String::as_str), Some("Ana"));
}

#[test]
fn a_later_write_replaces_the_earlier_name_in_the_same_slot() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let author = generate_communal_author().expect("author");

    write_profile_card(
        &store,
        &author,
        &ProfileCard {
            display_name: "Ana".into(),
        },
        1,
    )
    .expect("write");
    write_profile_card(
        &store,
        &author,
        &ProfileCard {
            display_name: "Ana R.".into(),
        },
        2,
    )
    .expect("rewrite");

    let names = resolve_display_names(&store).expect("resolve");
    let subspace = *author.subspace_id().as_bytes();
    assert_eq!(names.len(), 1, "one slot per person, last write wins");
    assert_eq!(names.get(&subspace).map(String::as_str), Some("Ana R."));
}

#[test]
fn resolve_is_empty_when_nobody_has_a_profile() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    assert!(resolve_display_names(&store).expect("resolve").is_empty());
}
