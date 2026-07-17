//! Newswire FFI contract for the editor display predicate (Unit 4a).
//!
//! `newswire_is_editor` answers the SAME question the core admission authority
//! (`require_action_authority`) enforces at signing time: both call the shared
//! `is_editorial_authority` predicate, so a display gate can never diverge from
//! what the write path allows. There is no founder special-case — a founder
//! absent from the editorial roster is not an editor, exactly as admission
//! would reject them. Unknown or non-descriptor entry ids resolve to a plain
//! `false`, never an error, so a renderer can gate UI without special-casing.

use riot_ffi::{open_local_profile, NewswirePostInput, NewswireSpaceInput};

fn hex32(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn space_input(name: &str, roster: Vec<String>) -> NewswireSpaceInput {
    NewswireSpaceInput {
        name: name.into(),
        summary: "Editor predicate fixture.".into(),
        languages: vec!["en".into()],
        geographic_tags: vec![],
        topic_tags: vec![],
        editorial_roster: roster,
    }
}

fn post_input(space_entry_id: &str, headline: &str) -> NewswirePostInput {
    NewswirePostInput {
        space_descriptor_entry_id: space_entry_id.into(),
        headline: headline.into(),
        body: "Body of the report.".into(),
        language: "en".into(),
        event_time_unix_seconds: None,
        expires_at_unix_seconds: None,
        coarse_location: None,
        source_claims: vec![],
        operational_profile: None,
        ai_assisted: false,
    }
}

#[test]
fn newswire_is_editor_matches_admission_authority() {
    let profile = open_local_profile().expect("profile");
    let me = profile.profile().whoami().expect("whoami");
    let founder_hex: String = me.id.iter().map(|byte| format!("{byte:02x}")).collect();

    // Editor + outsider are generated communal-author subspace ids under a valid
    // communal namespace. The founder supplies the roster verbatim; here it holds
    // the editor but NOT the founder.
    let namespace = riot_core::willow::generate_space_organizer_author().expect("namespace");
    let ns = *namespace.namespace_id().as_bytes();
    let editor =
        riot_core::willow::generate_communal_author_for_namespace(ns).expect("editor author");
    let editor_hex = hex32(editor.subspace_id().as_bytes());
    let outsider =
        riot_core::willow::generate_communal_author_for_namespace(ns).expect("outsider author");
    let outsider_hex = hex32(outsider.subspace_id().as_bytes());

    let space = profile
        .create_newswire_space(space_input("Harbor District", vec![editor_hex.clone()]))
        .expect("create space");

    // Roster member is an editor.
    assert!(profile
        .newswire_is_editor(space.entry_id.clone(), editor_hex.clone())
        .expect("editor query"));

    // Outsider is not.
    assert!(!profile
        .newswire_is_editor(space.entry_id.clone(), outsider_hex)
        .expect("outsider query"));

    // No founder special-case: the founder, absent from this roster, is NOT an
    // editor — precisely what require_action_authority would reject at signing.
    assert!(!profile
        .newswire_is_editor(space.entry_id.clone(), founder_hex)
        .expect("founder query"));

    // Unknown descriptor id (valid hex, not in the store) resolves to false.
    assert!(!profile
        .newswire_is_editor(hex32(&[9u8; 32]), editor_hex.clone())
        .expect("unknown descriptor query"));

    // A non-descriptor entry id (a signed post) resolves to false, not an error.
    let post = profile
        .create_newswire_post(post_input(&space.entry_id, "First report"))
        .expect("create post");
    assert!(!profile
        .newswire_is_editor(post.entry_id, editor_hex)
        .expect("non-descriptor entry query"));
}
