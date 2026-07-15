//! Newswire FFI contract for the Known-contributors surface (Unit 1C).
//!
//! The People surface is derived from the space's signed records and crosses
//! the UniFFI boundary as rendered authors — never a raw key posing as a name,
//! never a membership roster, never presence. The organizer-vs-member
//! discrimination is a core coordinate rule proven in
//! `crates/riot-core/tests/newswire_contributors.rs`; here we prove the FFI
//! surfaces distinct rows, the rendered author, the content-derived count, the
//! recognized organizer, and an empty space as an empty surface.

use riot_ffi::{open_local_profile, NewswirePostInput, NewswireSpaceInput};

fn space_input(name: &str) -> NewswireSpaceInput {
    NewswireSpaceInput {
        name: name.into(),
        summary: "Community newswire fixture.".into(),
        languages: vec!["en".into()],
        geographic_tags: vec![],
        topic_tags: vec![],
        editorial_roster: vec![],
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
fn an_empty_space_has_no_contributors() {
    let profile = open_local_profile().expect("profile");
    let space = profile
        .create_newswire_space(space_input("Harbor District"))
        .expect("create space");

    let contributors = profile
        .project_newswire_contributors(space.entry_id)
        .expect("project contributors");
    assert!(contributors.is_empty());
}

#[test]
fn a_posting_founder_is_a_rendered_organizer_contributor() {
    let profile = open_local_profile().expect("profile");
    let session = profile.profile();
    session
        .set_display_name("Ana".into())
        .expect("set display name");
    let me = session.whoami().expect("whoami");

    let space = profile
        .create_newswire_space(space_input("Waterfront"))
        .expect("create space");
    profile
        .create_newswire_post(post_input(&space.entry_id, "First report"))
        .expect("first post");
    profile
        .create_newswire_post(post_input(&space.entry_id, "Second report"))
        .expect("second post");

    let contributors = profile
        .project_newswire_contributors(space.entry_id)
        .expect("project contributors");

    // Two posts by one author collapse to a single distinct contributor row.
    assert_eq!(contributors.len(), 1);
    let row = &contributors[0];

    // The founder of a communal space is its recognized organizer, by the
    // namespace coordinate alone.
    assert!(row.is_organizer);

    // Content-derived, not a roster: the count is the two signed posts.
    assert_eq!(row.contribution_count, 2);

    // The author crosses the boundary RENDERED — name + key tag — and never as
    // a raw hex key posing as a name.
    let expected_id_hex = me
        .id
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(row.author.display_name, "Ana");
    assert_eq!(row.author.tag, me.tag);
    assert_eq!(row.author.rendered, format!("Ana · {}", me.tag));
    assert_eq!(row.author.id, expected_id_hex);
    assert_ne!(row.author.rendered, row.author.id);
}
