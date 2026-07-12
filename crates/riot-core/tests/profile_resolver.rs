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

/// The attack the sanitizer exists for, pinned.
///
/// The card codec is policy-free by design, so an attacker may claim the literal
/// name `"Ana · a3f91122"` — honest Ana's ENTIRE rendering. Concatenated naively
/// under the attacker's own key that becomes `"Ana · a3f91122 · deadbeef"`, a
/// string that BEGINS with exactly what honest Ana renders to. Truncate it in a
/// narrow row, or simply read to the first tag as a human does, and the
/// impersonation is perfect and costs nothing.
///
/// The invariant that kills it: the rendered string contains exactly ONE
/// separator, so whatever follows it is the key's and never the name's.
#[test]
fn a_name_containing_the_separator_cannot_forge_a_tag() {
    let honest = [
        0xa3, 0xf9, 0x11, 0x22, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0,
    ];
    let attacker = [
        0xde, 0xad, 0xbe, 0xef, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0,
    ];

    let honest_rendering = render_display_name(Some("Ana"), &honest);
    assert_eq!(honest_rendering, "Ana · a3f91122");

    // The attacker's NAME is honest Ana's whole rendering.
    let spoof = render_display_name(Some("Ana · a3f91122"), &attacker);

    assert!(
        !spoof.starts_with(&honest_rendering),
        "a name must not be able to reproduce another subspace's rendering as a prefix: {spoof:?}"
    );
    assert_eq!(
        spoof.matches('·').count(),
        1,
        "exactly one separator, so the text after it is always the key's: {spoof:?}"
    );
    assert_eq!(spoof, "Ana a3f91122 · deadbeef");
}

/// `U+202E` RIGHT-TO-LEFT OVERRIDE reorders what a reader SEES without changing
/// the bytes — a name can flip the tag that follows it. It never reaches a
/// surface.
#[test]
fn bidi_override_in_a_name_is_stripped() {
    let subspace = [
        0xa3, 0xf9, 0x11, 0x22, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0,
    ];
    let rendered = render_display_name(Some("Ana\u{202e}"), &subspace);
    assert!(
        !rendered.contains('\u{202e}'),
        "bidi override survived: {rendered:?}"
    );
    assert_eq!(rendered, "Ana · a3f91122");

    // The rest of the bidi family, and the invisibles, go the same way.
    for sneaky in [
        '\u{200e}', '\u{200f}', '\u{202a}', '\u{202b}', '\u{202c}', '\u{202d}', '\u{2066}',
        '\u{2069}', '\u{00ad}', '\u{feff}', '\u{200b}',
    ] {
        let rendered = render_display_name(Some(&format!("An{sneaky}a")), &subspace);
        assert_eq!(rendered, "Ana · a3f91122", "{sneaky:?} survived");
    }
}

#[test]
fn control_characters_are_stripped() {
    let subspace = [
        0xa3, 0xf9, 0x11, 0x22, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0,
    ];
    let rendered = render_display_name(Some("An\u{0}a"), &subspace);
    assert!(!rendered.contains('\u{0}'), "NUL survived: {rendered:?}");
    assert_eq!(rendered, "Ana · a3f91122");

    // A newline could otherwise push the tag onto its own line, out of sight.
    assert_eq!(
        render_display_name(Some("Ana\nBeatriz"), &subspace),
        "Ana Beatriz · a3f91122",
        "a line break collapses to a single space rather than hiding the tag"
    );
}

/// A name that sanitizes away to nothing must not render as a blank where a name
/// should be — a nameless-looking row is its own impersonation surface. It takes
/// the same `member` fallback as someone who never claimed a name.
#[test]
fn a_name_that_sanitizes_to_empty_falls_back_to_member() {
    let subspace = [
        0xa3, 0xf9, 0x11, 0x22, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0,
    ];
    for empty in ["·", "\u{202e}", "   ", "\u{0}", " · \u{202e} "] {
        assert_eq!(
            render_display_name(Some(empty), &subspace),
            "member · a3f91122",
            "{empty:?} should render in the nameless shape"
        );
    }
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
