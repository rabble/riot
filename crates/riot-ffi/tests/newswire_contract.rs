//! Newswire FFI contract: create a space, post, editorial action, and
//! project the collective view — all through the UniFFI boundary.

use riot_ffi::{
    open_local_profile, NewswireEditorialActionInput, NewswireEditorialActionKind,
    NewswirePostInput, NewswirePostTreatment, NewswireSpaceInput,
};

#[test]
fn create_space_and_project_empty_newswire() {
    let profile = open_local_profile().expect("profile");

    let space = profile
        .create_newswire_space(NewswireSpaceInput {
            name: "Harbor District".into(),
            summary: "Community emergency newswire for the harbor district.".into(),
            languages: vec!["en".into()],
            geographic_tags: vec!["harbor".into()],
            topic_tags: vec!["emergency".into()],
        })
        .expect("create space");

    assert!(!space.entry_id.is_empty());
    assert!(!space.signed_bytes.is_empty());

    // A fresh space has no posts yet.
    let projection = profile
        .project_newswire_space(space.entry_id.clone())
        .expect("project empty");
    assert!(projection.open_wire.is_empty());
    assert!(projection.front_page.is_empty());
}

#[test]
fn create_post_and_project_it() {
    let profile = open_local_profile().expect("profile");

    let space = profile
        .create_newswire_space(NewswireSpaceInput {
            name: "Waterfront".into(),
            summary: "Waterfront mutual aid newswire.".into(),
            languages: vec!["en".into()],
            geographic_tags: vec![],
            topic_tags: vec![],
        })
        .expect("create space");

    let post = profile
        .create_newswire_post(NewswirePostInput {
            space_descriptor_entry_id: space.entry_id.clone(),
            headline: "Shelter open at community center".into(),
            body: "The west hall is receiving arrivals. Blankets needed.".into(),
            language: "en".into(),
            coarse_location: Some("Harbor District, west side".into()),
            source_claims: vec!["Field observer".into()],
            ai_assisted: false,
        })
        .expect("create post");

    assert!(!post.entry_id.is_empty());

    // The post should appear in the projection.
    let projection = profile
        .project_newswire_space(space.entry_id)
        .expect("project");
    assert!(
        projection
            .open_wire
            .iter()
            .any(|p| p.entry_id == post.entry_id),
        "open wire must contain the created post"
    );
}

#[test]
fn editorial_action_hides_a_post() {
    let profile = open_local_profile().expect("profile");

    let space = profile
        .create_newswire_space(NewswireSpaceInput {
            name: "Test Space".into(),
            summary: "Testing editorial actions.".into(),
            languages: vec!["en".into()],
            geographic_tags: vec![],
            topic_tags: vec![],
        })
        .expect("create space");

    let post = profile
        .create_newswire_post(NewswirePostInput {
            space_descriptor_entry_id: space.entry_id.clone(),
            headline: "Unverified rumor".into(),
            body: "This post will be hidden by an editor.".into(),
            language: "en".into(),
            coarse_location: None,
            source_claims: vec![],
            ai_assisted: false,
        })
        .expect("create post");

    // The founding organizer is in the editorial roster, so they can act.
    let action = profile
        .create_newswire_editorial_action(NewswireEditorialActionInput {
            space_descriptor_entry_id: space.entry_id.clone(),
            target_entry_id: post.entry_id.clone(),
            kind: NewswireEditorialActionKind::Hide,
            reason: Some("Unverified — pending confirmation.".into()),
            correction_text: None,
        })
        .expect("editorial action");

    assert!(!action.entry_id.is_empty());

    // After hiding, the post's treatment should reflect it.
    let projection = profile
        .project_newswire_space(space.entry_id)
        .expect("project after hide");

    let projected = projection
        .open_wire
        .iter()
        .find(|p| p.entry_id == post.entry_id)
        .expect("hidden post should still be in the projection");
    assert_eq!(
        projected.treatment,
        NewswirePostTreatment::Hidden,
        "post should be marked hidden after the editorial action"
    );
}

#[test]
fn editorial_action_from_non_editor_fails() {
    // A fresh profile creates a space. A *different* fresh profile is NOT
    // in the editorial roster and cannot author actions (it can still post
    // freely in the communal namespace).
    let organizer = open_local_profile().expect("organizer");

    let space = organizer
        .create_newswire_space(NewswireSpaceInput {
            name: "Organized".into(),
            summary: "An organized space with an editorial roster.".into(),
            languages: vec!["en".into()],
            geographic_tags: vec![],
            topic_tags: vec![],
        })
        .expect("create space");

    // The organizer creates a post.
    let post = organizer
        .create_newswire_post(NewswirePostInput {
            space_descriptor_entry_id: space.entry_id.clone(),
            headline: "Base post".into(),
            body: "Target for a rogue editorial action.".into(),
            language: "en".into(),
            coarse_location: None,
            source_claims: vec![],
            ai_assisted: false,
        })
        .expect("post");

    // A different profile is not in the roster.
    let outsider = open_local_profile().expect("outsider");
    let result = outsider.create_newswire_editorial_action(NewswireEditorialActionInput {
        space_descriptor_entry_id: space.entry_id,
        target_entry_id: post.entry_id,
        kind: NewswireEditorialActionKind::Feature,
        reason: None,
        correction_text: None,
    });
    assert!(
        result.is_err(),
        "a non-editor must not be able to author editorial actions"
    );
}
