//! Writing a profile card, resolving everyone's names back out of the store,
//! and the one sanctioned rendering rule: a self-claimed name is never shown
//! bare — it always carries its key-derived suffix.

use riot_core::import::encode_bundle;
use riot_core::profile::card::ProfileCard;
use riot_core::profile::resolver::{
    render_display_name, resolve_display_names, write_profile_card,
};
use riot_core::profile::ProfileError;
use riot_core::session::{
    CommitOutcome, EvidenceStore, ImportContext, InspectOutcome, RiotSession,
};
use riot_core::willow::{
    alert_entry_path_matches_payload, authorise_entry, encode_capability, encode_entry,
    entry_timestamp_micros, generate_communal_author, Entry, EvidenceAuthor, SignedWillowEntry,
};

fn signed_profile(
    author: &EvidenceAuthor,
    display_name: &str,
    timestamp: u64,
) -> SignedWillowEntry {
    let payload = riot_core::profile::card::encode_profile_card(&ProfileCard {
        display_name: display_name.to_string(),
    })
    .expect("encode card");
    let path = riot_core::profile::path::profile_card_path(author.subspace_id().as_bytes())
        .expect("profile path");
    let entry = Entry::builder()
        .namespace_id(author.namespace_id().clone())
        .subspace_id(author.subspace_id())
        .path(path)
        .timestamp(timestamp)
        .payload(&payload)
        .build();
    let authorised = authorise_entry(author, entry).expect("authorise");
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    SignedWillowEntry {
        entry_bytes: encode_entry(authorised.entry()),
        capability_bytes: encode_capability(token.capability()),
        signature: signature.to_bytes(),
        payload_bytes: payload,
    }
}

fn commit_signed(store: &EvidenceStore, signed: &SignedWillowEntry) {
    let bundle = encode_bundle(std::slice::from_ref(signed)).expect("valid bundle");
    let preview = match store
        .inspect(&bundle, ImportContext::new("profile-resolver-test"))
        .expect("inspect")
    {
        InspectOutcome::Preview(preview) => preview,
        InspectOutcome::Rejected(rejection) => panic!("unexpected rejection: {rejection:?}"),
    };
    match preview.plan_all().expect("plan").commit().expect("commit") {
        CommitOutcome::Committed(_) | CommitOutcome::NoChanges(_) => {}
    }
}

#[test]
fn render_always_appends_the_key_suffix() {
    let subspace = [
        0xa3, 0xf9, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    assert_eq!(
        render_display_name(Some("Ana"), &subspace),
        "Ana · a3f91122"
    );
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
        '\u{2069}', '\u{00ad}', '\u{feff}', '\u{200b}', '\u{fff9}',
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

#[test]
fn resolver_returns_local_and_foreign_profiles_by_full_subspace_id() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let local = generate_communal_author().expect("local author");
    let foreign = generate_communal_author().expect("foreign author");

    commit_signed(&store, &signed_profile(&local, "Ana", 1));
    commit_signed(&store, &signed_profile(&foreign, "Beatriz", 1));

    let names = resolve_display_names(&store).expect("resolve");
    assert_eq!(names.len(), 2);
    assert_eq!(
        names
            .get(local.subspace_id().as_bytes())
            .map(String::as_str),
        Some("Ana")
    );
    assert_eq!(
        names
            .get(foreign.subspace_id().as_bytes())
            .map(String::as_str),
        Some("Beatriz")
    );
}

#[test]
fn invalid_signature_and_capability_never_reach_the_resolver() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let valid_author = generate_communal_author().expect("valid author");
    let attacker = generate_communal_author().expect("attacker");

    commit_signed(&store, &signed_profile(&valid_author, "Ana", 1));

    let mut bad_signature = signed_profile(&attacker, "Forged signature", 1);
    bad_signature.signature[0] ^= 0x80;
    assert!(encode_bundle(&[bad_signature]).is_err());

    let mut bad_capability = signed_profile(&attacker, "Wrong capability", 2);
    bad_capability.capability_bytes = signed_profile(&valid_author, "unused", 2).capability_bytes;
    assert!(encode_bundle(&[bad_capability]).is_err());

    let names = resolve_display_names(&store).expect("resolve");
    assert_eq!(names.len(), 1);
    assert_eq!(
        names
            .get(valid_author.subspace_id().as_bytes())
            .map(String::as_str),
        Some("Ana")
    );
}

#[test]
fn newer_wins_older_is_rejected_and_equal_timestamp_is_deterministic() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let author = generate_communal_author().expect("author");

    write_profile_card(
        &store,
        &author,
        &ProfileCard {
            display_name: "newer".into(),
        },
        20,
    )
    .expect("newer write");
    assert_eq!(
        write_profile_card(
            &store,
            &author,
            &ProfileCard {
                display_name: "older".into(),
            },
            10,
        ),
        Err(ProfileError::StoreRejected)
    );
    assert_eq!(
        resolve_display_names(&store)
            .expect("resolve")
            .get(author.subspace_id().as_bytes())
            .map(String::as_str),
        Some("newer")
    );

    let a = signed_profile(&author, "tie-a", 30);
    let b = signed_profile(&author, "tie-b", 30);
    commit_signed(&store, &a);
    commit_signed(&store, &b);
    let first_winner = resolve_display_names(&store)
        .expect("resolve")
        .get(author.subspace_id().as_bytes())
        .cloned()
        .expect("winner");
    assert!(first_winner == "tie-a" || first_winner == "tie-b");

    let second_session = RiotSession::open().expect("session");
    let second_store = second_session.create_store().expect("store");
    commit_signed(&second_store, &b);
    commit_signed(&second_store, &a);
    assert_eq!(
        resolve_display_names(&second_store)
            .expect("resolve")
            .get(author.subspace_id().as_bytes())
            .map(String::as_str),
        Some(first_winner.as_str()),
        "equal-timestamp winner must not depend on arrival order"
    );
}

#[test]
fn closed_store_maps_to_profile_store_rejected() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    store.close().expect("close store");

    assert_eq!(
        resolve_display_names(&store),
        Err(ProfileError::StoreRejected)
    );
    assert_eq!(
        write_profile_card(
            &store,
            &generate_communal_author().expect("author"),
            &ProfileCard {
                display_name: "Ana".into(),
            },
            1,
        ),
        Err(ProfileError::StoreRejected)
    );
}

#[test]
fn invalid_card_is_rejected_before_profile_path_or_store_work() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let author = generate_communal_author().expect("author");

    assert_eq!(
        write_profile_card(
            &store,
            &author,
            &ProfileCard {
                display_name: String::new(),
            },
            1,
        ),
        Err(ProfileError::FieldInvalid)
    );
    assert_eq!(store.live_count().expect("live count"), 0);
}

#[test]
fn willow_entry_timestamp_and_invalid_binding_input_are_explicit() {
    let author = generate_communal_author().expect("author");
    let signed = signed_profile(&author, "Ana", 42);

    assert_eq!(entry_timestamp_micros(&signed.entry_bytes), Ok(42));
    assert!(entry_timestamp_micros(b"not an entry").is_err());
    assert!(alert_entry_path_matches_payload(b"not an entry", &[1; 16], &[2; 16]).is_err());
}
