//! Newswire FFI contract: create a space, post, editorial action, and
//! project the collective view — all through the UniFFI boundary.
//!
//! The projection is the product surface. Every field a post SIGNS must
//! survive the trip to a native app, and every field an editor's action
//! carries must be readable in the editorial history — otherwise a client
//! cannot derive the same front page as its peers, and the signed record is
//! a promise the app cannot keep.

use riot_ffi::NewswireProjectedComment;
use riot_ffi::{
    open_local_profile, AlertCertainty, AlertSeverity, AlertUrgency, NewswireAlertProfile,
    NewswireEditorialActionInput, NewswireEditorialActionKind, NewswireOperationalProfile,
    NewswirePostInput, NewswirePostTreatment, NewswireProjectedPost, NewswireRequestKind,
    NewswireRequestProfile, NewswireSpaceInput,
};

/// Well past: any projection clock is later than this, so a post carrying it
/// as an expiry is expired and belongs in `earlier`.
const EXPIRED_UNIX_SECONDS: u64 = 1_600_000_000;
/// Well future: a post carrying this as an expiry stays on the open wire.
const LIVE_UNIX_SECONDS: u64 = 4_000_000_000;
const EVENT_UNIX_SECONDS: u64 = 1_700_000_000;

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

fn find<'a>(posts: &'a [NewswireProjectedPost], entry_id: &str) -> &'a NewswireProjectedPost {
    posts
        .iter()
        .find(|post| post.entry_id == entry_id)
        .expect("projected post")
}

#[test]
fn create_space_and_project_empty_newswire() {
    let profile = open_local_profile().expect("profile");

    let space = profile
        .create_newswire_space(space_input("Harbor District"))
        .expect("create space");

    assert!(!space.entry_id.is_empty());
    assert!(!space.signed_bytes.is_empty());

    // A fresh space has no posts yet — and every collection is empty, not just
    // the two the projection used to expose.
    let projection = profile
        .project_newswire_space(space.entry_id.clone())
        .expect("project empty");
    assert!(projection.open_wire.is_empty());
    assert!(projection.front_page.is_empty());
    assert!(projection.earlier.is_empty());
    assert!(projection.comments.is_empty());
    assert!(projection.editorial_history.is_empty());
    assert!(projection.future_quarantine.is_empty());
}

fn find_comment<'a>(
    comments: &'a [NewswireProjectedComment],
    entry_id: &str,
) -> &'a NewswireProjectedComment {
    comments
        .iter()
        .find(|comment| comment.entry_id == entry_id)
        .expect("projected comment")
}

/// A communal reply is created through the boundary and reaches the projection
/// grouped under its parent post — carrying its body, parent id, and a rendered
/// author, exactly like a post row.
#[test]
fn a_comment_is_created_and_projected_under_its_parent_post() {
    let profile = open_local_profile().expect("profile");
    let session = profile.profile();
    session
        .set_display_name("Bo".into())
        .expect("set display name");

    let space = profile
        .create_newswire_space(space_input("Discussion"))
        .expect("create space");
    let post = profile
        .create_newswire_post(post_input(&space.entry_id, "What did you see?"))
        .expect("create post");

    let comment = profile
        .create_newswire_comment(
            space.entry_id.clone(),
            post.entry_id.clone(),
            "I was on the east side when it started.".into(),
            "en".into(),
        )
        .expect("create comment");
    assert!(!comment.entry_id.is_empty());
    assert!(!comment.signed_bytes.is_empty());

    let projection = profile
        .project_newswire_space(space.entry_id)
        .expect("project");
    let projected = find_comment(&projection.comments, &comment.entry_id);

    assert_eq!(projected.parent_entry_id, post.entry_id);
    assert_eq!(
        projected.body.as_deref(),
        Some("I was on the east side when it started.")
    );
    assert_eq!(projected.language, "en");
    assert_eq!(projected.treatment, NewswirePostTreatment::Ordinary);
    assert!(projected.tai_j2000_micros > 0);
    // The author is rendered by the same sanctioned path as a post author.
    assert_eq!(projected.author.display_name, "Bo");
    assert_eq!(
        projected.author.rendered,
        format!("Bo · {}", projected.author.tag)
    );
}

/// A reply whose parent post is not held is dropped from the projection — the
/// flat list never carries an orphan. A comment is still communal, so any
/// profile may post one (no editorial role required).
#[test]
fn a_comment_with_no_held_parent_is_dropped_from_the_projection() {
    let profile = open_local_profile().expect("profile");
    let space = profile
        .create_newswire_space(space_input("Danglers"))
        .expect("create space");

    // A well-formed entry id that names no post this store holds.
    let ghost_parent = "ab".repeat(32);
    let comment = profile
        .create_newswire_comment(
            space.entry_id.clone(),
            ghost_parent,
            "Reply into the void.".into(),
            "en".into(),
        )
        .expect("create comment");

    let projection = profile
        .project_newswire_space(space.entry_id)
        .expect("project");
    assert!(
        projection
            .comments
            .iter()
            .all(|c| c.entry_id != comment.entry_id),
        "a reply with no held parent must not appear in the projection"
    );
}

/// An editor tombstoning a comment redacts its body while keeping identity and
/// ordering — the same per-content moderation a post receives, never per person.
#[test]
fn an_editor_tombstone_redacts_a_comment_body() {
    let profile = open_local_profile().expect("profile");
    let space = profile
        .create_newswire_space(space_input("Moderated replies"))
        .expect("create space");
    let post = profile
        .create_newswire_post(post_input(&space.entry_id, "Open thread"))
        .expect("create post");
    let comment = profile
        .create_newswire_comment(
            space.entry_id.clone(),
            post.entry_id.clone(),
            "Content that names a private individual.".into(),
            "en".into(),
        )
        .expect("create comment");

    // The founding organizer is the default editor and can moderate the reply.
    profile
        .create_newswire_editorial_action(NewswireEditorialActionInput {
            space_descriptor_entry_id: space.entry_id.clone(),
            target_entry_id: comment.entry_id.clone(),
            kind: NewswireEditorialActionKind::Tombstone,
            reason: Some("Names a private individual.".into()),
            correction_text: None,
        })
        .expect("tombstone the comment");

    let projection = profile
        .project_newswire_space(space.entry_id)
        .expect("project");
    let projected = find_comment(&projection.comments, &comment.entry_id);
    assert_eq!(projected.treatment, NewswirePostTreatment::Tombstoned);
    assert_eq!(projected.body, None);
    // Identity survives: the row is still accountable.
    assert_eq!(projected.parent_entry_id, post.entry_id);
    assert!(projected.tai_j2000_micros > 0);
}

/// The heart of Unit 1A: a post signs headline, body, language, location,
/// event time, expiry, source claims, an operational profile and the
/// AI-assistance flag. Every one of them must arrive at the native app,
/// alongside a rendered author and the ordering key the wire is sorted by.
#[test]
fn projection_carries_every_signed_field_of_a_post() {
    let profile = open_local_profile().expect("profile");
    let session = profile.profile();
    session
        .set_display_name("Ana".into())
        .expect("set display name");
    let me = session.whoami().expect("whoami");

    let space = profile
        .create_newswire_space(space_input("Waterfront"))
        .expect("create space");

    let post = profile
        .create_newswire_post(NewswirePostInput {
            space_descriptor_entry_id: space.entry_id.clone(),
            headline: "Shelter open at community center".into(),
            body: "The west hall is receiving arrivals. Blankets needed.".into(),
            language: "en".into(),
            event_time_unix_seconds: Some(EVENT_UNIX_SECONDS),
            expires_at_unix_seconds: Some(LIVE_UNIX_SECONDS),
            coarse_location: Some("Harbor District, west side".into()),
            source_claims: vec!["Field observer".into(), "Shelter coordinator".into()],
            operational_profile: Some(NewswireOperationalProfile::Alert {
                profile: NewswireAlertProfile {
                    urgency: AlertUrgency::Immediate,
                    severity: AlertSeverity::Severe,
                    certainty: AlertCertainty::Observed,
                    valid_from_unix_seconds: Some(EVENT_UNIX_SECONDS),
                },
            }),
            ai_assisted: true,
        })
        .expect("create post");

    let projection = profile
        .project_newswire_space(space.entry_id)
        .expect("project");
    let projected = find(&projection.open_wire, &post.entry_id);

    assert_eq!(
        projected.headline.as_deref(),
        Some("Shelter open at community center")
    );
    assert_eq!(
        projected.body.as_deref(),
        Some("The west hall is receiving arrivals. Blankets needed.")
    );
    assert_eq!(projected.language, "en");
    assert_eq!(
        projected.coarse_location.as_deref(),
        Some("Harbor District, west side")
    );
    assert_eq!(projected.event_time_unix_seconds, Some(EVENT_UNIX_SECONDS));
    assert_eq!(projected.expires_at_unix_seconds, Some(LIVE_UNIX_SECONDS));
    // Source claims keep the order they were signed in — a projection may not
    // reorder a claim list the author committed to.
    assert_eq!(
        projected.source_claims,
        vec![
            "Field observer".to_string(),
            "Shelter coordinator".to_string()
        ]
    );
    assert!(projected.ai_assisted);
    assert_eq!(
        projected.operational_profile,
        Some(NewswireOperationalProfile::Alert {
            profile: NewswireAlertProfile {
                urgency: AlertUrgency::Immediate,
                severity: AlertSeverity::Severe,
                certainty: AlertCertainty::Observed,
                valid_from_unix_seconds: Some(EVENT_UNIX_SECONDS),
            },
        })
    );
    assert_eq!(projected.treatment, NewswirePostTreatment::Ordinary);
    assert!(projected.verification_ids.is_empty());
    assert!(projected.correction_ids.is_empty());

    // The ordering key the open wire is sorted by, surfaced so a client can
    // merge two projections without re-deriving it.
    assert!(projected.tai_j2000_micros > 0);

    // A real creation instant (UTC Unix seconds) is recovered from the entry
    // timestamp so the client can render "2h ago". It is a plausible present-day
    // second (post signed just now) and inverts the same converter the signer
    // used — NOT the raw micros value, and never the 1970 fallback.
    let created = projected
        .created_at_unix_seconds
        .expect("a freshly signed post must carry a recovered creation time");
    assert!(
        (1_700_000_000..4_000_000_000).contains(&created),
        "created_at_unix_seconds must be a present-day Unix second, got {created}"
    );
    assert!(
        created < projected.tai_j2000_micros,
        "created seconds ({created}) must be far below the micros ordering value ({}) — unit guard",
        projected.tai_j2000_micros
    );

    // The author is RENDERED, never a raw key posing as a name.
    let expected_tag = me
        .id
        .iter()
        .take(4)
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(projected.author.display_name, "Ana");
    assert_eq!(projected.author.tag, expected_tag);
    assert_eq!(projected.author.tag, me.tag);
    assert_eq!(projected.author.rendered, format!("Ana · {expected_tag}"));
    assert_eq!(
        projected.author.id,
        me.id
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
}

/// A person who never claimed a name renders in the same shape as everyone
/// else — `member · <tag>` — so no surface needs a second layout for them.
#[test]
fn an_author_with_no_profile_card_still_renders_as_a_name_and_tag() {
    let profile = open_local_profile().expect("profile");
    let space = profile
        .create_newswire_space(space_input("Unnamed"))
        .expect("create space");
    let post = profile
        .create_newswire_post(post_input(&space.entry_id, "Report from a nameless author"))
        .expect("create post");

    let projection = profile
        .project_newswire_space(space.entry_id)
        .expect("project");
    let author = &find(&projection.open_wire, &post.entry_id).author;

    assert_eq!(author.display_name, "member");
    assert_eq!(author.rendered, format!("member · {}", author.tag));
    assert_eq!(author.tag.len(), 8);
}

/// An expired post leaves the open wire for `earlier` — the bucket the core
/// projection has always computed and the FFI has always thrown away.
#[test]
fn an_expired_post_moves_to_earlier_and_leaves_the_open_wire() {
    let profile = open_local_profile().expect("profile");
    let space = profile
        .create_newswire_space(space_input("Expiring"))
        .expect("create space");

    let live = profile
        .create_newswire_post(NewswirePostInput {
            expires_at_unix_seconds: Some(LIVE_UNIX_SECONDS),
            ..post_input(&space.entry_id, "Still current")
        })
        .expect("live post");
    let expired = profile
        .create_newswire_post(NewswirePostInput {
            expires_at_unix_seconds: Some(EXPIRED_UNIX_SECONDS),
            ..post_input(&space.entry_id, "Long past")
        })
        .expect("expired post");

    let projection = profile
        .project_newswire_space(space.entry_id)
        .expect("project");

    assert_eq!(
        projection
            .open_wire
            .iter()
            .map(|post| post.entry_id.as_str())
            .collect::<Vec<_>>(),
        vec![live.entry_id.as_str()]
    );
    assert_eq!(
        projection
            .earlier
            .iter()
            .map(|post| post.entry_id.as_str())
            .collect::<Vec<_>>(),
        vec![expired.entry_id.as_str()]
    );
    assert_eq!(
        find(&projection.earlier, &expired.entry_id)
            .headline
            .as_deref(),
        Some("Long past")
    );
}

/// A featured post reaches the front page carrying the same complete row the
/// open wire carries, and the feature action itself is readable in the
/// editorial history.
#[test]
fn a_featured_post_reaches_the_front_page_and_its_action_the_history() {
    let profile = open_local_profile().expect("profile");
    let space = profile
        .create_newswire_space(space_input("Featured"))
        .expect("create space");
    let post = profile
        .create_newswire_post(post_input(&space.entry_id, "Assembly reconvenes Friday"))
        .expect("create post");

    let action = profile
        .create_newswire_editorial_action(NewswireEditorialActionInput {
            space_descriptor_entry_id: space.entry_id.clone(),
            target_entry_id: post.entry_id.clone(),
            kind: NewswireEditorialActionKind::Feature,
            reason: None,
            correction_text: None,
        })
        .expect("feature");

    let projection = profile
        .project_newswire_space(space.entry_id)
        .expect("project");

    let featured = find(&projection.front_page, &post.entry_id);
    assert_eq!(
        featured.headline.as_deref(),
        Some("Assembly reconvenes Friday")
    );

    let history = projection
        .editorial_history
        .iter()
        .find(|item| item.entry_id == action.entry_id)
        .expect("the feature action is in the editorial history");
    assert_eq!(history.target_entry_id, post.entry_id);
    assert_eq!(history.kind, NewswireEditorialActionKind::Feature);
    assert_eq!(history.reason, None);
    assert_eq!(history.correction_text, None);
    assert!(history.active);
    assert!(history.tai_j2000_micros > 0);
    // The acting editor is rendered by the same sanctioned path as an author.
    assert_eq!(history.signer.rendered, featured.author.rendered);
}

/// A correction leaves the original body standing and adds its replacement
/// text to the history — the whole point of a correction rather than an edit.
#[test]
fn a_correction_preserves_the_original_and_surfaces_its_replacement_text() {
    let profile = open_local_profile().expect("profile");
    let space = profile
        .create_newswire_space(space_input("Corrections"))
        .expect("create space");
    let post = profile
        .create_newswire_post(post_input(&space.entry_id, "Original headline"))
        .expect("create post");

    let action = profile
        .create_newswire_editorial_action(NewswireEditorialActionInput {
            space_descriptor_entry_id: space.entry_id.clone(),
            target_entry_id: post.entry_id.clone(),
            kind: NewswireEditorialActionKind::Correct,
            reason: Some("The time was wrong.".into()),
            correction_text: Some("The assembly reconvenes Friday, not Thursday.".into()),
        })
        .expect("correct");

    let projection = profile
        .project_newswire_space(space.entry_id)
        .expect("project");

    let corrected = find(&projection.open_wire, &post.entry_id);
    assert_eq!(corrected.headline.as_deref(), Some("Original headline"));
    assert_eq!(corrected.body.as_deref(), Some("Body of the report."));
    assert_eq!(corrected.correction_ids, vec![action.entry_id.clone()]);
    assert_eq!(corrected.treatment, NewswirePostTreatment::Ordinary);

    let history = projection
        .editorial_history
        .iter()
        .find(|item| item.entry_id == action.entry_id)
        .expect("correction in history");
    assert_eq!(history.reason.as_deref(), Some("The time was wrong."));
    assert_eq!(
        history.correction_text.as_deref(),
        Some("The assembly reconvenes Friday, not Thursday.")
    );
}

/// A verification is a signed act, not a score: the post carries the id of
/// every action that verified it.
#[test]
fn a_verification_is_carried_as_an_action_id_on_the_post() {
    let profile = open_local_profile().expect("profile");
    let space = profile
        .create_newswire_space(space_input("Verified"))
        .expect("create space");
    let post = profile
        .create_newswire_post(post_input(&space.entry_id, "Confirmed at the pier"))
        .expect("create post");

    let action = profile
        .create_newswire_editorial_action(NewswireEditorialActionInput {
            space_descriptor_entry_id: space.entry_id.clone(),
            target_entry_id: post.entry_id.clone(),
            kind: NewswireEditorialActionKind::Verify,
            reason: None,
            correction_text: None,
        })
        .expect("verify");

    let projection = profile
        .project_newswire_space(space.entry_id)
        .expect("project");
    let verified = find(&projection.open_wire, &post.entry_id);
    assert_eq!(verified.verification_ids, vec![action.entry_id]);
}

#[test]
fn editorial_action_hides_a_post() {
    let profile = open_local_profile().expect("profile");

    let space = profile
        .create_newswire_space(space_input("Test Space"))
        .expect("create space");

    let post = profile
        .create_newswire_post(NewswirePostInput {
            coarse_location: Some("north pier".into()),
            source_claims: vec!["rumor".into()],
            ..post_input(&space.entry_id, "Unverified rumor")
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

    let projection = profile
        .project_newswire_space(space.entry_id)
        .expect("project after hide");

    let projected = find(&projection.open_wire, &post.entry_id);
    assert_eq!(
        projected.treatment,
        NewswirePostTreatment::Hidden,
        "post should be marked hidden after the editorial action"
    );

    // Hiding redacts the plaintext the reader would otherwise see. The
    // headline is the MOST visible half of a post — a hide that leaves it
    // standing has hidden nothing.
    assert_eq!(projected.headline, None);
    assert_eq!(projected.body, None);
    assert_eq!(projected.coarse_location, None);
    assert!(projected.source_claims.is_empty());
    assert_eq!(projected.operational_profile, None);

    // Identity and ordering survive: the row is still accountable.
    assert_eq!(projected.author.display_name, "member");
    assert!(projected.tai_j2000_micros > 0);
}

/// Tombstoning redacts the same plaintext as a hide, and the history keeps
/// the acting editor's reason so the act itself stays accountable.
#[test]
fn a_tombstone_redacts_the_payload_and_keeps_the_act_in_history() {
    let profile = open_local_profile().expect("profile");
    let space = profile
        .create_newswire_space(space_input("Tombstones"))
        .expect("create space");
    let post = profile
        .create_newswire_post(post_input(&space.entry_id, "Doxxing content"))
        .expect("create post");

    let action = profile
        .create_newswire_editorial_action(NewswireEditorialActionInput {
            space_descriptor_entry_id: space.entry_id.clone(),
            target_entry_id: post.entry_id.clone(),
            kind: NewswireEditorialActionKind::Tombstone,
            reason: Some("Names a private individual.".into()),
            correction_text: None,
        })
        .expect("tombstone");

    let projection = profile
        .project_newswire_space(space.entry_id)
        .expect("project");
    let projected = find(&projection.open_wire, &post.entry_id);

    assert_eq!(projected.treatment, NewswirePostTreatment::Tombstoned);
    assert_eq!(projected.headline, None);
    assert_eq!(projected.body, None);

    let history = projection
        .editorial_history
        .iter()
        .find(|item| item.entry_id == action.entry_id)
        .expect("tombstone in history");
    assert_eq!(
        history.reason.as_deref(),
        Some("Names a private individual.")
    );
    assert!(history.active);
}

/// A retraction is a first-class signed act: it reaches the editorial history
/// carrying its reason and the id of the action it targets, so the record of
/// an editor changing their mind is itself public.
///
/// The *effect* of a retraction — deactivating a strictly-later target and
/// clearing the front page — depends on the two acts landing at distinct
/// Willow timestamps, which the real system clock does not guarantee for two
/// signs microseconds apart. That timing-dependent semantic is proven
/// deterministically against explicit clocks in
/// `newswire::projection::tests::later_retract_deactivates_action_and_both_remain_in_history`;
/// here we assert only what the boundary is responsible for surfacing.
#[test]
fn a_retraction_reaches_the_history_with_its_reason_and_target() {
    let profile = open_local_profile().expect("profile");
    let space = profile
        .create_newswire_space(space_input("Retractions"))
        .expect("create space");
    let post = profile
        .create_newswire_post(post_input(&space.entry_id, "Briefly featured"))
        .expect("create post");

    let feature = profile
        .create_newswire_editorial_action(NewswireEditorialActionInput {
            space_descriptor_entry_id: space.entry_id.clone(),
            target_entry_id: post.entry_id.clone(),
            kind: NewswireEditorialActionKind::Feature,
            reason: None,
            correction_text: None,
        })
        .expect("feature");
    let retract = profile
        .create_newswire_editorial_action(NewswireEditorialActionInput {
            space_descriptor_entry_id: space.entry_id.clone(),
            target_entry_id: feature.entry_id.clone(),
            kind: NewswireEditorialActionKind::Retract,
            reason: Some("Featured in error.".into()),
            correction_text: None,
        })
        .expect("retract");

    let projection = profile
        .project_newswire_space(space.entry_id)
        .expect("project");

    // Both the feature and the retraction that targets it are in the history.
    let ids = projection
        .editorial_history
        .iter()
        .map(|item| item.entry_id.as_str())
        .collect::<Vec<_>>();
    assert!(ids.contains(&feature.entry_id.as_str()));

    let retraction = projection
        .editorial_history
        .iter()
        .find(|item| item.entry_id == retract.entry_id)
        .expect("the retraction is in the editorial history");
    assert_eq!(retraction.kind, NewswireEditorialActionKind::Retract);
    assert_eq!(retraction.target_entry_id, feature.entry_id);
    assert_eq!(retraction.reason.as_deref(), Some("Featured in error."));
    assert_eq!(retraction.correction_text, None);
}

#[test]
fn editorial_action_from_non_editor_fails() {
    // A fresh profile creates a space. A *different* fresh profile is NOT
    // in the editorial roster and cannot author actions (it can still post
    // freely in the communal namespace).
    let organizer = open_local_profile().expect("organizer");

    let space = organizer
        .create_newswire_space(space_input("Organized"))
        .expect("create space");

    // The organizer creates a post.
    let post = organizer
        .create_newswire_post(post_input(&space.entry_id, "Base post"))
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

/// An empty founding roster keeps the behaviour every existing caller relies
/// on: the founder is the sole editor.
#[test]
fn an_empty_founding_roster_leaves_the_founder_as_the_sole_editor() {
    let profile = open_local_profile().expect("profile");
    let space = profile
        .create_newswire_space(space_input("Default roster"))
        .expect("create space");
    let post = profile
        .create_newswire_post(post_input(&space.entry_id, "Base post"))
        .expect("post");

    profile
        .create_newswire_editorial_action(NewswireEditorialActionInput {
            space_descriptor_entry_id: space.entry_id,
            target_entry_id: post.entry_id,
            kind: NewswireEditorialActionKind::Feature,
            reason: None,
            correction_text: None,
        })
        .expect("the founder is the default editor");
}

/// The founding collective chooses its editors. A roster that does not name
/// the founder means the founder cannot act editorially — proof the roster is
/// the descriptor's, not a hardcoded `vec![signer_id]`.
#[test]
fn a_founding_roster_that_excludes_the_founder_denies_them_editorial_authority() {
    let profile = open_local_profile().expect("profile");
    let stranger_key = "11".repeat(32);

    let space = profile
        .create_newswire_space(NewswireSpaceInput {
            editorial_roster: vec![stranger_key],
            ..space_input("Delegated roster")
        })
        .expect("create space");
    let post = profile
        .create_newswire_post(post_input(&space.entry_id, "Base post"))
        .expect("post");

    let result = profile.create_newswire_editorial_action(NewswireEditorialActionInput {
        space_descriptor_entry_id: space.entry_id,
        target_entry_id: post.entry_id,
        kind: NewswireEditorialActionKind::Feature,
        reason: None,
        correction_text: None,
    });
    assert!(
        result.is_err(),
        "a founder outside the roster they signed has no editorial authority"
    );
}

/// A founder who names themselves alongside others keeps their own authority.
#[test]
fn a_founding_roster_naming_the_founder_and_others_keeps_the_founder_editing() {
    let profile = open_local_profile().expect("profile");
    let me = profile.profile().whoami().expect("whoami");
    let my_key = me
        .id
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();

    let space = profile
        .create_newswire_space(NewswireSpaceInput {
            editorial_roster: vec![my_key, "22".repeat(32)],
            ..space_input("Shared roster")
        })
        .expect("create space");
    let post = profile
        .create_newswire_post(post_input(&space.entry_id, "Base post"))
        .expect("post");

    profile
        .create_newswire_editorial_action(NewswireEditorialActionInput {
            space_descriptor_entry_id: space.entry_id,
            target_entry_id: post.entry_id,
            kind: NewswireEditorialActionKind::Verify,
            reason: None,
            correction_text: None,
        })
        .expect("a founder inside the roster keeps editorial authority");
}

/// A roster key that is not 32 bytes of hex is refused at the boundary — a
/// malformed editor key must never reach the signed descriptor.
#[test]
fn a_malformed_roster_key_is_rejected_at_the_boundary() {
    let profile = open_local_profile().expect("profile");
    for bad_key in ["not hex", "aabb", &"zz".repeat(32)] {
        let result = profile.create_newswire_space(NewswireSpaceInput {
            editorial_roster: vec![bad_key.to_string()],
            ..space_input("Malformed roster")
        });
        assert!(
            result.is_err(),
            "roster key {bad_key:?} must be rejected before signing"
        );
    }
}

/// A duplicated roster key is refused by the signed model, and the failure
/// crosses the boundary as a stable input error rather than a panic.
#[test]
fn a_duplicated_roster_key_is_rejected() {
    let profile = open_local_profile().expect("profile");
    let key = "33".repeat(32);
    let result = profile.create_newswire_space(NewswireSpaceInput {
        editorial_roster: vec![key.clone(), key],
        ..space_input("Duplicate roster")
    });
    assert!(result.is_err(), "a duplicated editor key must be rejected");
}

/// A request profile signs contact instructions and a needed-by time, and the
/// model requires an expiry and a location alongside it. Both halves must
/// survive the round trip.
#[test]
fn a_request_profile_round_trips_through_the_projection() {
    let profile = open_local_profile().expect("profile");
    let space = profile
        .create_newswire_space(space_input("Mutual aid"))
        .expect("create space");

    let post = profile
        .create_newswire_post(NewswirePostInput {
            expires_at_unix_seconds: Some(LIVE_UNIX_SECONDS),
            coarse_location: Some("community kitchen".into()),
            operational_profile: Some(NewswireOperationalProfile::Request {
                profile: NewswireRequestProfile {
                    kind: NewswireRequestKind::Need,
                    needed_by_unix_seconds: Some(LIVE_UNIX_SECONDS),
                    contact_instructions: "Ask for Ana at the kitchen door.".into(),
                },
            }),
            ..post_input(&space.entry_id, "Blankets needed")
        })
        .expect("create request post");

    let projection = profile
        .project_newswire_space(space.entry_id)
        .expect("project");
    let projected = find(&projection.open_wire, &post.entry_id);

    assert_eq!(
        projected.operational_profile,
        Some(NewswireOperationalProfile::Request {
            profile: NewswireRequestProfile {
                kind: NewswireRequestKind::Need,
                needed_by_unix_seconds: Some(LIVE_UNIX_SECONDS),
                contact_instructions: "Ask for Ana at the kitchen door.".into(),
            },
        })
    );
}

/// Every closed alert and request enum variant survives the round trip
/// through the boundary — the to-core mapping on create and the from-core
/// mapping on projection are exercised for each.
#[test]
fn every_operational_enum_variant_round_trips() {
    let profile = open_local_profile().expect("profile");
    let space = profile
        .create_newswire_space(space_input("Every variant"))
        .expect("create space");

    let urgencies = [
        AlertUrgency::Immediate,
        AlertUrgency::Expected,
        AlertUrgency::Future,
        AlertUrgency::Past,
        AlertUrgency::Unknown,
    ];
    let severities = [
        AlertSeverity::Extreme,
        AlertSeverity::Severe,
        AlertSeverity::Moderate,
        AlertSeverity::Minor,
        AlertSeverity::Unknown,
    ];
    let certainties = [
        AlertCertainty::Observed,
        AlertCertainty::Likely,
        AlertCertainty::Possible,
        AlertCertainty::Unlikely,
        AlertCertainty::Unknown,
    ];

    for index in 0..urgencies.len() {
        let alert = NewswireAlertProfile {
            urgency: urgencies[index],
            severity: severities[index],
            certainty: certainties[index],
            valid_from_unix_seconds: None,
        };
        let post = profile
            .create_newswire_post(NewswirePostInput {
                expires_at_unix_seconds: Some(LIVE_UNIX_SECONDS),
                coarse_location: Some("somewhere".into()),
                source_claims: vec!["a source".into()],
                operational_profile: Some(NewswireOperationalProfile::Alert {
                    profile: alert.clone(),
                }),
                ..post_input(&space.entry_id, &format!("Alert {index}"))
            })
            .expect("alert post");
        let projection = profile
            .project_newswire_space(space.entry_id.clone())
            .expect("project");
        assert_eq!(
            find(&projection.open_wire, &post.entry_id).operational_profile,
            Some(NewswireOperationalProfile::Alert { profile: alert })
        );
    }

    for kind in [NewswireRequestKind::Need, NewswireRequestKind::Offer] {
        let request = NewswireRequestProfile {
            kind,
            needed_by_unix_seconds: None,
            contact_instructions: "the public desk".into(),
        };
        let post = profile
            .create_newswire_post(NewswirePostInput {
                expires_at_unix_seconds: Some(LIVE_UNIX_SECONDS),
                coarse_location: Some("somewhere".into()),
                operational_profile: Some(NewswireOperationalProfile::Request {
                    profile: request.clone(),
                }),
                ..post_input(&space.entry_id, "Request")
            })
            .expect("request post");
        let projection = profile
            .project_newswire_space(space.entry_id.clone())
            .expect("project");
        assert_eq!(
            find(&projection.open_wire, &post.entry_id).operational_profile,
            Some(NewswireOperationalProfile::Request { profile: request })
        );
    }
}

/// The signed model refuses an alert with no expiry. That refusal must reach
/// the caller as an input error — never a panic, never a silent post.
#[test]
fn an_alert_profile_without_an_expiry_is_rejected() {
    let profile = open_local_profile().expect("profile");
    let space = profile
        .create_newswire_space(space_input("Alerts"))
        .expect("create space");

    let result = profile.create_newswire_post(NewswirePostInput {
        coarse_location: Some("north pier".into()),
        source_claims: vec!["eyewitness".into()],
        operational_profile: Some(NewswireOperationalProfile::Alert {
            profile: NewswireAlertProfile {
                urgency: AlertUrgency::Immediate,
                severity: AlertSeverity::Extreme,
                certainty: AlertCertainty::Observed,
                valid_from_unix_seconds: None,
            },
        }),
        ..post_input(&space.entry_id, "Evacuate the north pier")
    });
    assert!(
        result.is_err(),
        "an alert with no expiry must be refused before signing"
    );
}

/// Every entry-id argument crossing the boundary is hex-decoded. A malformed
/// one is a stable input error, not an internal failure.
#[test]
fn malformed_entry_ids_are_refused_at_the_boundary() {
    let profile = open_local_profile().expect("profile");
    assert!(profile.project_newswire_space("nonsense".into()).is_err());
    assert!(profile
        .create_newswire_post(post_input("aabb", "Headline"))
        .is_err());
    assert!(profile
        .create_newswire_editorial_action(NewswireEditorialActionInput {
            space_descriptor_entry_id: "zz".repeat(32),
            target_entry_id: "00".repeat(32),
            kind: NewswireEditorialActionKind::Feature,
            reason: None,
            correction_text: None,
        })
        .is_err());
}

/// Projecting a descriptor that is not in the store fails cleanly.
#[test]
fn projecting_an_unknown_space_fails_cleanly() {
    let profile = open_local_profile().expect("profile");
    assert!(profile.project_newswire_space("ab".repeat(32)).is_err());
}

fn find_post<'a>(
    projection: &'a [NewswireProjectedPost],
    entry_id: &str,
) -> &'a NewswireProjectedPost {
    projection
        .iter()
        .find(|post| post.entry_id == entry_id)
        .expect("projected post")
}

/// A communal reaction is signed through the boundary and reaches the projected
/// post as a tally: one entry for the reacted kind with a distinct-author count
/// of one, keyed by the stable lowercase kind name.
#[test]
fn a_reaction_is_signed_and_tallied_on_its_post() {
    let profile = open_local_profile().expect("profile");
    let space = profile
        .create_newswire_space(space_input("Reactions"))
        .expect("create space");
    let post = profile
        .create_newswire_post(post_input(&space.entry_id, "Solidarity now"))
        .expect("create post");

    let reaction = profile
        .toggle_newswire_reaction(
            space.entry_id.clone(),
            post.entry_id.clone(),
            "solidarity".into(),
            true,
        )
        .expect("react");
    assert!(!reaction.entry_id.is_empty());
    assert!(!reaction.signed_bytes.is_empty());

    let projection = profile
        .project_newswire_space(space.entry_id)
        .expect("project");
    let projected = find_post(&projection.open_wire, &post.entry_id);
    assert_eq!(projected.reactions.len(), 1);
    assert_eq!(projected.reactions[0].kind, "solidarity");
    assert_eq!(projected.reactions[0].count, 1);
}

/// Toggling the same reaction off (active = false) retracts it: the author drops
/// out of the tally, and with no one left the kind disappears from the post.
#[test]
fn toggling_a_reaction_off_retracts_it_from_the_tally() {
    let profile = open_local_profile().expect("profile");
    let space = profile
        .create_newswire_space(space_input("Toggle"))
        .expect("create space");
    let post = profile
        .create_newswire_post(post_input(&space.entry_id, "Toggle me"))
        .expect("create post");

    profile
        .toggle_newswire_reaction(
            space.entry_id.clone(),
            post.entry_id.clone(),
            "support".into(),
            true,
        )
        .expect("react on");
    profile
        .toggle_newswire_reaction(
            space.entry_id.clone(),
            post.entry_id.clone(),
            "support".into(),
            false,
        )
        .expect("react off");

    let projection = profile
        .project_newswire_space(space.entry_id)
        .expect("project");
    let projected = find_post(&projection.open_wire, &post.entry_id);
    assert!(
        projected
            .reactions
            .iter()
            .all(|tally| tally.kind != "support"),
        "a retracted reaction must leave no tally behind"
    );
}

/// An unknown reaction kind is refused at the boundary rather than silently
/// coerced to a default — the closed set is enforced in one place.
#[test]
fn an_unknown_reaction_kind_is_refused() {
    let profile = open_local_profile().expect("profile");
    let space = profile
        .create_newswire_space(space_input("Closed set"))
        .expect("create space");
    let post = profile
        .create_newswire_post(post_input(&space.entry_id, "Only four kinds"))
        .expect("create post");

    assert!(profile
        .toggle_newswire_reaction(space.entry_id, post.entry_id, "party".into(), true)
        .is_err());
}
