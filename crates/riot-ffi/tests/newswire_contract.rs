//! Newswire FFI contract: create a space, post, editorial action, and
//! project the collective view — all through the UniFFI boundary.
//!
//! The projection is the product surface. Every field a post SIGNS must
//! survive the trip to a native app, and every field an editor's action
//! carries must be readable in the editorial history — otherwise a client
//! cannot derive the same front page as its peers, and the signed record is
//! a promise the app cannot keep.

use riot_ffi::open_local_profile_with_database;
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

// --- Reader-side writes: the path every real person actually takes ----------
//
// Every other test in this file has ONE profile that CREATES the space and then
// writes into its own store. Nobody uses Riot that way. A person joins a
// community someone else published, receives its descriptor and posts over
// sync, and then reacts and replies to other people's reports.
//
// That path had no test at all, which is how a build shipped where reading
// worked and every write came back "Reactions aren't available for this post"
// and "That reply was not accepted."

/// Carries a signed record from one profile into another exactly as sync does:
/// inspect the bytes, plan, accept. Nothing is injected into the reader's store.
fn carry(reader: &riot_ffi::MobileProfile, signed_bytes: Vec<u8>) {
    let preview = reader
        .inspect_bytes(signed_bytes, "test-sync".into())
        .expect("inspect carried bytes");
    preview
        .create_plan(Vec::new())
        .expect("plan carried bytes")
        .accept()
        .expect("accept carried bytes");
}

#[test]
fn a_reader_who_joined_a_community_can_reply_to_someone_elses_post() {
    let author = open_local_profile().expect("author profile");
    let space = author
        .create_newswire_space(space_input("River City Wire"))
        .expect("create space");
    let post = author
        .create_newswire_post(post_input(&space.entry_id, "Free breakfast at the church"))
        .expect("create post");

    // The reader JOINS the published community, exactly as the app does with a
    // riot:// share reference, before any of its records are carried across.
    let reference = author
        .newswire_share_reference(space.entry_id.clone())
        .expect("share reference");
    let reader = open_local_profile().expect("reader profile");
    reader
        .join_newswire_community(
            riot_ffi::PublicSpace {
                namespace_id: reference.namespace_id.clone(),
                title: "River City Wire".into(),
                is_public: true,
            },
            reference.descriptor_entry_id.clone(),
            Vec::new(),
        )
        .expect("join community");
    carry(&reader, space.signed_bytes.clone());
    carry(&reader, post.signed_bytes.clone());

    let reply = reader.create_newswire_comment(
        space.entry_id.clone(),
        post.entry_id.clone(),
        "I can bring bread.".into(),
        "en".into(),
    );
    assert!(
        reply.is_ok(),
        "a joined reader must be able to reply to another author's post, got {:?}",
        reply.err()
    );
}

#[test]
fn a_reader_who_joined_a_community_can_react_to_someone_elses_post() {
    let author = open_local_profile().expect("author profile");
    let space = author
        .create_newswire_space(space_input("River City Wire"))
        .expect("create space");
    let post = author
        .create_newswire_post(post_input(&space.entry_id, "Rent strike meeting"))
        .expect("create post");

    let reference = author
        .newswire_share_reference(space.entry_id.clone())
        .expect("share reference");
    let reader = open_local_profile().expect("reader profile");
    reader
        .join_newswire_community(
            riot_ffi::PublicSpace {
                namespace_id: reference.namespace_id.clone(),
                title: "River City Wire".into(),
                is_public: true,
            },
            reference.descriptor_entry_id.clone(),
            Vec::new(),
        )
        .expect("join community");
    carry(&reader, space.signed_bytes.clone());
    carry(&reader, post.signed_bytes.clone());

    let reaction = reader.toggle_newswire_reaction(
        space.entry_id.clone(),
        post.entry_id.clone(),
        "solidarity".into(),
        true,
    );
    assert!(
        reaction.is_ok(),
        "a joined reader must be able to react to another author's post, got {:?}",
        reaction.err()
    );
}

/// DURABLE reader. The in-memory twin of this test passes, and so does the
/// durable self-authored journey — the untested intersection was a reader who
/// holds someone else's community in a REAL SQLite store and then writes into
/// it. That is the configuration a shipped app is always in.
#[test]
fn a_durable_reader_can_reply_to_a_carried_post() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir.path().join("reader.db").to_string_lossy().to_string();

    let author = open_local_profile().expect("author profile");
    let space = author
        .create_newswire_space(space_input("River City Wire"))
        .expect("create space");
    let post = author
        .create_newswire_post(post_input(&space.entry_id, "Free breakfast"))
        .expect("create post");
    let reference = author
        .newswire_share_reference(space.entry_id.clone())
        .expect("share reference");

    let reader = open_local_profile_with_database(db_path).expect("durable reader profile");
    reader
        .join_newswire_community(
            riot_ffi::PublicSpace {
                namespace_id: reference.namespace_id.clone(),
                title: "River City Wire".into(),
                is_public: true,
            },
            reference.descriptor_entry_id.clone(),
            Vec::new(),
        )
        .expect("join community");
    carry(&reader, space.signed_bytes.clone());
    carry(&reader, post.signed_bytes.clone());

    let reply = reader.create_newswire_comment(
        space.entry_id.clone(),
        post.entry_id.clone(),
        "I can bring bread.".into(),
        "en".into(),
    );
    assert!(
        reply.is_ok(),
        "a durable reader must be able to reply to a carried post, got {:?}",
        reply.err()
    );

    let reaction =
        reader.toggle_newswire_reaction(space.entry_id, post.entry_id, "solidarity".into(), true);
    assert!(
        reaction.is_ok(),
        "a durable reader must be able to react to a carried post, got {:?}",
        reaction.err()
    );
}

/// A community joined by reference whose descriptor has NOT yet arrived — the
/// state the sidebar calls "Not synced yet". Writing into it cannot succeed,
/// but it must fail HONESTLY: the person is told what is wrong, not handed
/// `Internal`, which the UI maps into the same bucket as a permissions problem
/// and renders as "Reactions aren't available for this post".
#[test]
fn writing_into_a_community_whose_descriptor_has_not_arrived_fails_honestly() {
    let author = open_local_profile().expect("author profile");
    let space = author
        .create_newswire_space(space_input("River City Wire"))
        .expect("create space");
    let post = author
        .create_newswire_post(post_input(&space.entry_id, "Free breakfast"))
        .expect("create post");
    let reference = author
        .newswire_share_reference(space.entry_id.clone())
        .expect("share reference");

    // Joined, but nothing carried across yet.
    let reader = open_local_profile().expect("reader profile");
    reader
        .join_newswire_community(
            riot_ffi::PublicSpace {
                namespace_id: reference.namespace_id.clone(),
                title: "River City Wire".into(),
                is_public: true,
            },
            reference.descriptor_entry_id.clone(),
            Vec::new(),
        )
        .expect("join community");

    let error = reader
        .create_newswire_comment(
            space.entry_id.clone(),
            post.entry_id.clone(),
            "I can bring bread.".into(),
            "en".into(),
        )
        .expect_err("a reply into an unsynced community cannot be admitted");

    assert!(
        !matches!(error, riot_ffi::MobileError::Internal),
        "an unsynced community must not report Internal — the UI maps that to \
         an authority failure and tells the person reactions are unavailable, \
         which is neither true nor actionable. Got {error:?}"
    );
}

// --- RELAUNCH. The one configuration no test ever covered --------------------
//
// Every durable test above builds the community and writes into it inside a
// SINGLE profile lifetime, which is the only lifetime in which
// `profile.sync_inventory` is populated — it lives in memory and is never
// persisted or rebuilt from the store. A real person quits the app. On the next
// launch the store still holds every entry and the registry still lists the
// community, but the inventory comes back EMPTY, so
// `ensure_complete_sync_inventory` sees `[] != live_ids` and refuses every
// newswire write with `Internal`, which Swift renders as "Reactions aren't
// available for this post." Reads keep working, which is why this looks like an
// authority bug instead of a lost session field.

#[test]
fn a_durable_reader_can_still_react_after_the_app_is_relaunched() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir.path().join("relaunch.db").to_string_lossy().to_string();

    let author = open_local_profile().expect("author profile");
    let space = author
        .create_newswire_space(space_input("River City Wire"))
        .expect("create space");
    let post = author
        .create_newswire_post(post_input(&space.entry_id, "Free breakfast"))
        .expect("create post");
    let reference = author
        .newswire_share_reference(space.entry_id.clone())
        .expect("share reference");

    // First launch: join, receive the community over sync, react successfully.
    {
        let reader =
            open_local_profile_with_database(db_path.clone()).expect("durable reader profile");
        reader
            .join_newswire_community(
                riot_ffi::PublicSpace {
                    namespace_id: reference.namespace_id.clone(),
                    title: "River City Wire".into(),
                    is_public: true,
                },
                reference.descriptor_entry_id.clone(),
                vec![7u8; 32],
            )
            .expect("join community");
        carry(&reader, space.signed_bytes.clone());
        carry(&reader, post.signed_bytes.clone());
        reader
            .toggle_newswire_reaction(
                space.entry_id.clone(),
                post.entry_id.clone(),
                "solidarity".into(),
                true,
            )
            .expect("react during the first launch");
        drop(reader);
    }

    // Second launch: same database, same store, same registry.
    let reader =
        riot_ffi::open_local_profile_with_database_for_starter_catalog_generation(db_path, None)
            .expect("reopen durable profile");
    // The registry is durable, so the community the app reopens into is ALREADY
    // active: this call takes the re-select path, which is the whole point.
    assert_eq!(
        reader
            .active_community()
            .expect("active community")
            .map(|row| row.namespace_id),
        Some(reference.namespace_id.clone()),
        "the relaunched profile must reopen into the community it left"
    );
    reader
        .switch_community(reference.namespace_id.clone(), vec![7u8; 32])
        .expect("switch to the community the registry already lists");

    let projection = reader
        .project_newswire_space(space.entry_id.clone())
        .expect("the community still reads after relaunch");
    assert!(
        !projection.open_wire.is_empty(),
        "the post must still be readable after relaunch"
    );

    let reaction =
        reader.toggle_newswire_reaction(space.entry_id, post.entry_id, "important".into(), true);
    assert!(
        reaction.is_ok(),
        "a reaction must survive an app relaunch — the store and the registry \
         both persist, so the session-only sync inventory must be rebuilt from \
         them rather than refusing every write. Got {:?}",
        reaction.err()
    );
}

/// The RELAUNCH PATH THE APP ACTUALLY TAKES. iOS/macOS do not call
/// `switch_community` on launch — `ProfileRepository.restoreSpace` re-joins the
/// persisted space with `join_public_space`, which lands on its own
/// already-active idempotent early return. A fix that only covered
/// `switch_community` would leave every shipped build exactly as broken.
#[test]
fn a_relaunch_that_rejoins_its_persisted_space_can_still_react() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir.path().join("rejoin.db").to_string_lossy().to_string();
    let key = vec![9u8; 32];

    let author = open_local_profile().expect("author profile");
    let space = author
        .create_newswire_space(space_input("River City Wire"))
        .expect("create space");
    let post = author
        .create_newswire_post(post_input(&space.entry_id, "Free breakfast"))
        .expect("create post");
    let reference = author
        .newswire_share_reference(space.entry_id.clone())
        .expect("share reference");

    {
        let reader =
            open_local_profile_with_database(db_path.clone()).expect("durable reader profile");
        reader
            .join_newswire_community(
                riot_ffi::PublicSpace {
                    namespace_id: reference.namespace_id.clone(),
                    title: "River City Wire".into(),
                    is_public: true,
                },
                reference.descriptor_entry_id.clone(),
                key.clone(),
            )
            .expect("join community");
        carry(&reader, space.signed_bytes.clone());
        carry(&reader, post.signed_bytes.clone());
        reader
            .toggle_newswire_reaction(
                space.entry_id.clone(),
                post.entry_id.clone(),
                "solidarity".into(),
                true,
            )
            .expect("react during the first launch");
        drop(reader);
    }

    let reader =
        riot_ffi::open_local_profile_with_database_for_starter_catalog_generation(db_path, None)
            .expect("reopen durable profile");
    // Exactly what `restoreSpace` does on launch.
    reader
        .join_public_space(
            riot_ffi::PublicSpace {
                namespace_id: reference.namespace_id.clone(),
                title: "River City Wire".into(),
                is_public: true,
            },
            key,
        )
        .expect("rejoin the persisted space");

    let reaction = reader.toggle_newswire_reaction(
        space.entry_id.clone(),
        post.entry_id.clone(),
        "important".into(),
        true,
    );
    assert!(
        reaction.is_ok(),
        "the app's own relaunch path must leave the community writable, got {:?}",
        reaction.err()
    );

    let reply = reader.create_newswire_comment(
        space.entry_id,
        post.entry_id,
        "I can bring bread.".into(),
        "en".into(),
    );
    assert!(
        reply.is_ok(),
        "replies must survive the relaunch too, got {:?}",
        reply.err()
    );
}

// --- The store-size ceiling: writes die as a community FILLS UP ------------
//
// `EvidenceStore::persist` estimates its WAL cost from `mutation.live`, which is
// the WHOLE live set after the join (`live: next_join.live_records()`), not the
// delta being written. So the estimate grows with the size of the community,
// and once it crosses `checkpoint_hard_pages` (1024) `admit_write` answers
// `BusyRetryable` -> `SessionError::StalePreview` -> `MobileError::Internal` ->
// "Reactions aren't available for this post."
//
// Nothing about the reaction is wrong. The community is simply too big, and it
// gets worse with every post that syncs in — a device that worked yesterday
// stops writing today, permanently, with no message that says so.

/// Fills a durable community with `posts` reports carried from another author,
/// then answers whether a reaction into it is still accepted.
fn reaction_into_a_durable_community_of(posts: usize) -> Result<(), riot_ffi::MobileError> {
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir.path().join("capacity.db").to_string_lossy().to_string();

    let author = open_local_profile().expect("author profile");
    let space = author
        .create_newswire_space(space_input("River City Wire"))
        .expect("create space");
    let reference = author
        .newswire_share_reference(space.entry_id.clone())
        .expect("share reference");

    let reader = open_local_profile_with_database(db_path).expect("durable reader profile");
    reader
        .join_newswire_community(
            riot_ffi::PublicSpace {
                namespace_id: reference.namespace_id.clone(),
                title: "River City Wire".into(),
                is_public: true,
            },
            reference.descriptor_entry_id.clone(),
            vec![5u8; 32],
        )
        .expect("join community");
    carry(&reader, space.signed_bytes.clone());

    let mut first_post = None;
    for index in 0..posts {
        // The AUTHOR hits the ceiling first — its own sync inventory fills at
        // MAX_SYNC_IDS — so this error is part of what the test measures and
        // must not be unwrapped away.
        let post = author.create_newswire_post(post_input(
            &space.entry_id,
            &format!("Report number {index} from the open wire"),
        ))?;
        carry(&reader, post.signed_bytes.clone());
        if first_post.is_none() {
            first_post = Some(post.entry_id);
        }
    }

    reader
        .toggle_newswire_reaction(
            space.entry_id,
            first_post.expect("at least one post"),
            "solidarity".into(),
            true,
        )
        .map(|_| ())
}

/// A community with an ordinary amount of content in it must stay writable.
/// Rabble's own device held 44 live entries when every reaction started coming
/// back "Reactions aren't available for this post."
#[test]
fn a_durable_community_with_forty_reports_is_still_writable() {
    assert!(
        reaction_into_a_durable_community_of(40).is_ok(),
        "a 40-report community must still accept a reaction"
    );
}

/// The wall a growing community hits next. `MAX_SYNC_IDS` is 64, so a
/// community physically cannot hold more than 64 live entries — and the sync
/// inventory refuses before the store does. This test does not endorse that
/// number; it pins it, so the limit is a decision someone made rather than a
/// surprise a city runs into mid-protest. When it is raised, this test is the
/// thing that says so out loud.
#[test]
fn the_sync_inventory_ceiling_is_sixty_four_live_entries() {
    assert_eq!(
        riot_core::sync::MAX_SYNC_IDS,
        64,
        "raising this ceiling is a wire-format decision, not a tuning tweak"
    );
    assert!(
        matches!(
            reaction_into_a_durable_community_of(80),
            Err(riot_ffi::MobileError::SessionLimit)
        ),
        "past the ceiling a community must refuse with a CAPACITY error, which \
         the app renders as \"can\u{2019}t hold another reaction right now\" — never \
         as the authority-or-input bucket that says reactions are unavailable"
    );
}

// --- Sync freshness: the sidebar's "Not synced yet" ------------------------
//
// `CommunityRow.sync_freshness_unix_seconds` is what the community sidebar
// renders as "Synced 5 minutes ago" or, when it is `None`, "Not synced yet".
// It is written in exactly one place in the whole crate — `site_ffi.rs`, the
// followed-SITE path. No community sync path ever stamped it, so every
// community on every device read "Not synced yet" permanently, no matter how
// many successful syncs had landed. Meanwhile the Home header renders a
// SEPARATE, session-only Swift dictionary, so one screen said "Synced just
// now" and "Not synced yet" about the same community at the same moment.

#[test]
fn a_relay_pull_stamps_the_communitys_sync_freshness() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir
        .path()
        .join("freshness.db")
        .to_string_lossy()
        .to_string();
    let key = vec![3u8; 32];

    let author = open_local_profile().expect("author profile");
    let space = author
        .create_newswire_space(space_input("River City Wire"))
        .expect("create space");
    let post = author
        .create_newswire_post(post_input(&space.entry_id, "Free breakfast"))
        .expect("create post");
    let reference = author
        .newswire_share_reference(space.entry_id.clone())
        .expect("share reference");

    let reader = open_local_profile_with_database(db_path).expect("durable reader");
    reader
        .join_newswire_community(
            riot_ffi::PublicSpace {
                namespace_id: reference.namespace_id.clone(),
                title: "River City Wire".into(),
                is_public: true,
            },
            reference.descriptor_entry_id.clone(),
            key,
        )
        .expect("join community");

    let before = reader
        .list_communities()
        .expect("communities")
        .into_iter()
        .find(|row| row.namespace_id == reference.namespace_id)
        .expect("the joined community is listed");
    assert_eq!(
        before.sync_freshness_unix_seconds, None,
        "a community that has never synced has no freshness stamp"
    );

    carry(&reader, space.signed_bytes.clone());
    carry(&reader, post.signed_bytes.clone());

    let after = reader
        .list_communities()
        .expect("communities")
        .into_iter()
        .find(|row| row.namespace_id == reference.namespace_id)
        .expect("the joined community is still listed");
    assert!(
        after.sync_freshness_unix_seconds.is_some(),
        "content arriving over sync must stamp the community's freshness — \
         without it the sidebar reads \"Not synced yet\" forever, on every \
         device, however many syncs have succeeded"
    );
}

// --- Carrying every community you hold, not just the one on screen ---------
//
// `open_sync_session` scopes itself to `profile.space` — the ACTIVE community —
// and nothing iterates the joined ones. So a phone holding a tenants union, a
// mutual aid network and a protest wire passes along only whichever happens to
// be selected when it meets someone. For a tool whose reach depends on devices
// carrying content to each other during a shutdown, that is most of the
// propagation not happening.

/// Joins `title` and carries its descriptor + post in, leaving it ACTIVE.
fn join_and_carry(
    reader: &riot_ffi::MobileProfile,
    reference: &riot_ffi::NewswireShareReference,
    title: &str,
    key: Vec<u8>,
    bytes: &[Vec<u8>],
) {
    reader
        .join_newswire_community(
            riot_ffi::PublicSpace {
                namespace_id: reference.namespace_id.clone(),
                title: title.into(),
                is_public: true,
            },
            reference.descriptor_entry_id.clone(),
            key,
        )
        .expect("join community");
    for signed in bytes {
        carry(reader, signed.clone());
    }
}

#[test]
fn a_device_can_offer_a_community_that_is_not_the_active_one() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir.path().join("carrier.db").to_string_lossy().to_string();
    let key = vec![11u8; 32];

    // Two separate communities, each with a post, published by two authors.
    let union_author = open_local_profile().expect("author profile");
    let union_space = union_author
        .create_newswire_space(space_input("Riverside Tenants Union"))
        .expect("create space");
    let union_post = union_author
        .create_newswire_post(post_input(&union_space.entry_id, "Rent strike Thursday"))
        .expect("create post");
    let union_ref = union_author
        .newswire_share_reference(union_space.entry_id.clone())
        .expect("share reference");

    let wire_author = open_local_profile().expect("author profile");
    let wire_space = wire_author
        .create_newswire_space(space_input("River City Wire"))
        .expect("create space");
    let wire_post = wire_author
        .create_newswire_post(post_input(&wire_space.entry_id, "Free breakfast"))
        .expect("create post");
    let wire_ref = wire_author
        .newswire_share_reference(wire_space.entry_id.clone())
        .expect("share reference");

    // One carrier device holds BOTH. The wire is joined second, so it is active.
    let carrier = open_local_profile_with_database(db_path).expect("carrier profile");
    join_and_carry(
        &carrier,
        &union_ref,
        "Riverside Tenants Union",
        key.clone(),
        &[
            union_space.signed_bytes.clone(),
            union_post.signed_bytes.clone(),
        ],
    );
    join_and_carry(
        &carrier,
        &wire_ref,
        "River City Wire",
        key,
        &[
            wire_space.signed_bytes.clone(),
            wire_post.signed_bytes.clone(),
        ],
    );

    let active = carrier
        .active_community()
        .expect("active community")
        .expect("a community is active");
    assert_eq!(
        active.namespace_id, wire_ref.namespace_id,
        "the wire was joined last, so it is the one on screen"
    );

    // The tenants union is held but NOT on screen. A carrier must still be able
    // to hand it onward — that is the whole sneakernet property.
    let offer = carrier
        .sync_offer_for_community(union_ref.namespace_id.clone())
        .expect("a held community can be offered even when it is not active");
    assert!(
        !offer.is_empty(),
        "a device carrying a community must be able to offer it without the \
         person selecting it first"
    );
}

/// Carrying more communities must never widen what any ONE peer is offered.
/// This is the isolation property `install_sync_inventory` guards for the
/// active community, asserted here per namespace: a carrier holding two
/// communities offers each separately, and neither offer contains the other's
/// entries.
#[test]
fn carrying_two_communities_never_mixes_them_into_one_offer() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir
        .path()
        .join("isolation.db")
        .to_string_lossy()
        .to_string();
    let key = vec![13u8; 32];

    let a_author = open_local_profile().expect("author profile");
    let a_space = a_author
        .create_newswire_space(space_input("Riverside Tenants Union"))
        .expect("create space");
    let a_post = a_author
        .create_newswire_post(post_input(&a_space.entry_id, "Rent strike Thursday"))
        .expect("create post");
    let a_ref = a_author
        .newswire_share_reference(a_space.entry_id.clone())
        .expect("share reference");

    let b_author = open_local_profile().expect("author profile");
    let b_space = b_author
        .create_newswire_space(space_input("River City Wire"))
        .expect("create space");
    let b_post = b_author
        .create_newswire_post(post_input(&b_space.entry_id, "Free breakfast"))
        .expect("create post");
    let b_ref = b_author
        .newswire_share_reference(b_space.entry_id.clone())
        .expect("share reference");

    let carrier = open_local_profile_with_database(db_path).expect("carrier profile");
    join_and_carry(
        &carrier,
        &a_ref,
        "Riverside Tenants Union",
        key.clone(),
        &[a_space.signed_bytes.clone(), a_post.signed_bytes.clone()],
    );
    join_and_carry(
        &carrier,
        &b_ref,
        "River City Wire",
        key,
        &[b_space.signed_bytes.clone(), b_post.signed_bytes.clone()],
    );

    let a_offer = carrier
        .sync_offer_for_community(a_ref.namespace_id.clone())
        .expect("offer for A");
    let b_offer = carrier
        .sync_offer_for_community(b_ref.namespace_id.clone())
        .expect("offer for B");

    // Compared by the post's own text, which rides in the payload. The
    // `signed_bytes` of a single-entry bundle carry their own framing and do
    // not appear verbatim inside a multi-entry bundle, so byte-containment on
    // those would be vacuous.
    let a_bytes = a_offer.concat();
    let b_bytes = b_offer.concat();
    let a_text = b"Rent strike Thursday";
    let b_text = b"Free breakfast";
    let _ = (&a_post, &b_post);

    let contains = |haystack: &[u8], needle: &[u8]| -> bool {
        needle.len() <= haystack.len()
            && haystack
                .windows(needle.len())
                .any(|window| window == needle)
    };

    assert!(contains(&a_bytes, a_text), "A's offer carries A's post");
    assert!(contains(&b_bytes, b_text), "B's offer carries B's post");
    assert!(
        !contains(&a_bytes, b_text),
        "A's offer must NOT leak B's entries — carrying two communities cannot \
         widen what a single peer is offered"
    );
    assert!(
        !contains(&b_bytes, a_text),
        "B's offer must NOT leak A's entries"
    );
}

/// A namespace this device has not joined is never offered, even if its store
/// holds entries for it (a followed site, a stale import). Held-community check
/// before any offer is built.
#[test]
fn a_community_this_device_has_not_joined_is_never_offered() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir.path().join("unjoined.db").to_string_lossy().to_string();

    let author = open_local_profile().expect("author profile");
    let space = author
        .create_newswire_space(space_input("River City Wire"))
        .expect("create space");
    let reference = author
        .newswire_share_reference(space.entry_id.clone())
        .expect("share reference");

    let device = open_local_profile_with_database(db_path).expect("device profile");
    assert!(
        matches!(
            device.sync_offer_for_community(reference.namespace_id),
            Err(riot_ffi::MobileError::CommunityUnavailable)
        ),
        "a namespace this device never joined must not be offerable"
    );
}

// --- Per-community carry policy -------------------------------------------
//
// Carrying every community a device holds is what makes content saturate during
// a shutdown. Working out which communities two devices share means disclosing
// membership — information ABOUT a person, unlike the content they chose to
// publish. A public wire wants maximum spread; a legal-support group does not,
// and no cryptography changes that. So it is a per-community choice.
// See docs/decisions/2026-08-04-membership-disclosure-note.md.

#[test]
fn a_newly_joined_community_carries_by_default_and_can_be_made_manual() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir.path().join("carry.db").to_string_lossy().to_string();

    let author = open_local_profile().expect("author profile");
    let space = author
        .create_newswire_space(space_input("River City Wire"))
        .expect("create space");
    let reference = author
        .newswire_share_reference(space.entry_id.clone())
        .expect("share reference");

    let device = open_local_profile_with_database(db_path).expect("device profile");
    join_and_carry(
        &device,
        &reference,
        "River City Wire",
        vec![21u8; 32],
        &[space.signed_bytes.clone()],
    );

    // Public broadcast: a newly joined community spreads unless told otherwise.
    assert_eq!(
        device
            .communities_to_carry_automatically()
            .expect("carry list"),
        vec![reference.namespace_id.clone()]
    );

    device
        .set_community_carry_policy(reference.namespace_id.clone(), false)
        .expect("mark manual");
    assert!(
        device
            .communities_to_carry_automatically()
            .expect("carry list")
            .is_empty(),
        "a community marked manual is not offered without being asked"
    );

    // Marking it manual must NOT break a deliberate exchange — the person can
    // still choose to hand it over.
    assert!(
        device
            .sync_offer_for_community(reference.namespace_id.clone())
            .expect("a manual community can still be offered deliberately")
            .len()
            > 0
    );

    device
        .set_community_carry_policy(reference.namespace_id.clone(), true)
        .expect("back to automatic");
    assert_eq!(
        device
            .communities_to_carry_automatically()
            .expect("carry list"),
        vec![reference.namespace_id]
    );
}

/// The policy survives a relaunch — it is a durable safety choice, not a
/// session preference. A person who marks their legal-support group manual
/// before an action must not have it silently revert when they reopen the app.
#[test]
fn the_carry_policy_survives_a_relaunch() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir
        .path()
        .join("carry-relaunch.db")
        .to_string_lossy()
        .to_string();
    let key = vec![23u8; 32];

    let author = open_local_profile().expect("author profile");
    let space = author
        .create_newswire_space(space_input("Legal Support"))
        .expect("create space");
    let reference = author
        .newswire_share_reference(space.entry_id.clone())
        .expect("share reference");

    {
        let device = open_local_profile_with_database(db_path.clone()).expect("device profile");
        join_and_carry(
            &device,
            &reference,
            "Legal Support",
            key.clone(),
            &[space.signed_bytes.clone()],
        );
        device
            .set_community_carry_policy(reference.namespace_id.clone(), false)
            .expect("mark manual");
        drop(device);
    }

    let device =
        riot_ffi::open_local_profile_with_database_for_starter_catalog_generation(db_path, None)
            .expect("reopen");
    assert!(
        device
            .communities_to_carry_automatically()
            .expect("carry list")
            .is_empty(),
        "a manual community must stay manual across a relaunch"
    );
    assert!(
        !device
            .list_communities()
            .expect("communities")
            .into_iter()
            .find(|row| row.namespace_id == reference.namespace_id)
            .expect("listed")
            .carry_automatically,
        "the row a person sees reflects the stored choice"
    );
}

// --- Finding shared communities without disclosing the rest ---------------

/// Joins `title` into `device` without carrying any content — enough to list it.
fn join_only(
    device: &riot_ffi::MobileProfile,
    reference: &riot_ffi::NewswireShareReference,
    title: &str,
    key: Vec<u8>,
) {
    device
        .join_newswire_community(
            riot_ffi::PublicSpace {
                namespace_id: reference.namespace_id.clone(),
                title: title.into(),
                is_public: true,
            },
            reference.descriptor_entry_id.clone(),
            key,
        )
        .expect("join community");
}

fn published(title: &str) -> riot_ffi::NewswireShareReference {
    let author = open_local_profile().expect("author profile");
    let space = author
        .create_newswire_space(space_input(title))
        .expect("create space");
    author
        .newswire_share_reference(space.entry_id)
        .expect("share reference")
}

#[test]
fn two_devices_learn_the_community_they_share_and_nothing_else() {
    let shared = published("River City Wire");
    let only_a = published("Riverside Tenants Union");
    let only_b = published("Legal Support");

    let dir = tempfile::tempdir().expect("temp dir");
    let a = open_local_profile_with_database(dir.path().join("a.db").to_string_lossy().to_string())
        .expect("device A");
    let b = open_local_profile_with_database(dir.path().join("b.db").to_string_lossy().to_string())
        .expect("device B");

    join_only(&a, &shared, "River City Wire", vec![31u8; 32]);
    join_only(&a, &only_a, "Riverside Tenants Union", vec![31u8; 32]);
    join_only(&b, &shared, "River City Wire", vec![32u8; 32]);
    join_only(&b, &only_b, "Legal Support", vec![32u8; 32]);

    // One session nonce, derived from both sides in the real exchange.
    let nonce = vec![0xAB; 32];
    let a_advert = a.carry_advertisement(nonce.clone()).expect("A advertises");
    let b_advert = b.carry_advertisement(nonce.clone()).expect("B advertises");

    let a_sees = a
        .communities_shared_with_peer(nonce.clone(), b_advert.clone())
        .expect("A matches B");
    let b_sees = b
        .communities_shared_with_peer(nonce.clone(), a_advert.clone())
        .expect("B matches A");

    assert_eq!(
        a_sees,
        vec![shared.namespace_id.clone()],
        "A learns the shared one"
    );
    assert_eq!(
        b_sees,
        vec![shared.namespace_id.clone()],
        "B learns the shared one"
    );

    // Neither advertisement discloses a namespace id in the clear.
    for digest in a_advert.iter().chain(b_advert.iter()) {
        assert_ne!(digest, &shared.namespace_id);
        assert_ne!(digest, &only_a.namespace_id);
        assert_ne!(digest, &only_b.namespace_id);
    }
    // And B never learns that A carries the tenants union.
    assert!(!b_sees.contains(&only_a.namespace_id));
    assert!(!a_sees.contains(&only_b.namespace_id));
}

/// A fresh nonce per session is what stops two encounters by the same person
/// being correlated. The same community must advertise differently each time.
#[test]
fn a_different_session_nonce_produces_an_unlinkable_advertisement() {
    let wire = published("River City Wire");
    let dir = tempfile::tempdir().expect("temp dir");
    let device =
        open_local_profile_with_database(dir.path().join("d.db").to_string_lossy().to_string())
            .expect("device");
    join_only(&device, &wire, "River City Wire", vec![33u8; 32]);

    let monday = device.carry_advertisement(vec![1u8; 32]).expect("monday");
    let tuesday = device.carry_advertisement(vec![2u8; 32]).expect("tuesday");

    assert_eq!(monday.len(), 1);
    assert_ne!(
        monday, tuesday,
        "the same community must not advertise identically across sessions, or \
         two encounters can be linked to the same person"
    );

    // A nonce-less advertisement would be identical forever; refuse it.
    assert!(device.carry_advertisement(Vec::new()).is_err());
    assert!(device
        .communities_shared_with_peer(Vec::new(), monday)
        .is_err());
}

/// A community marked manual is not in the advertisement at all — it is never
/// named to a peer, not even blinded, without a deliberate act.
#[test]
fn a_manual_community_is_not_advertised() {
    let wire = published("River City Wire");
    let legal = published("Legal Support");
    let dir = tempfile::tempdir().expect("temp dir");
    let device =
        open_local_profile_with_database(dir.path().join("m.db").to_string_lossy().to_string())
            .expect("device");
    join_only(&device, &wire, "River City Wire", vec![34u8; 32]);
    join_only(&device, &legal, "Legal Support", vec![34u8; 32]);

    let nonce = vec![7u8; 32];
    assert_eq!(
        device
            .carry_advertisement(nonce.clone())
            .expect("both")
            .len(),
        2
    );

    device
        .set_community_carry_policy(legal.namespace_id.clone(), false)
        .expect("mark manual");

    let advert = device.carry_advertisement(nonce.clone()).expect("one");
    assert_eq!(advert.len(), 1, "the manual community is not advertised");

    // Even a peer that HOLDS the manual community cannot learn this device does.
    let peer_dir = tempfile::tempdir().expect("temp dir");
    let peer = open_local_profile_with_database(
        peer_dir.path().join("p.db").to_string_lossy().to_string(),
    )
    .expect("peer");
    join_only(&peer, &legal, "Legal Support", vec![35u8; 32]);
    let peer_advert = peer
        .carry_advertisement(nonce.clone())
        .expect("peer advert");

    assert!(
        device
            .communities_shared_with_peer(nonce, peer_advert)
            .expect("match")
            .is_empty(),
        "a manual community must not surface as shared, in either direction"
    );
}

/// Correcting the record is in this app's purpose, and right now there is no way
/// to do it. `Retract` and `Tombstone` exist in the core; nothing surfaces them,
/// and the open question is whether an author may withdraw their OWN words in a
/// community where they hold no editorial role. They must: a person who wrote
/// something has standing over that record regardless of who runs the wire.
#[test]
#[ignore = "RED: unmet requirement, not a flake. An author cannot retract their \
own words without an editorial role — verified 2026-08-02, fails with InvalidInput. \
create_signed_editorial_action takes (author, descriptor, action) and NO store \
handle, so it cannot check who wrote the target entry; allowing this means either \
passing the target's authorship into the core or gating it in the FFI where the \
store is reachable. That is a permissions-boundary decision, so it is recorded \
here rather than guessed at."]
fn an_author_can_retract_their_own_reply_without_an_editorial_role() {
    let author = open_local_profile().expect("author profile");
    let space = author
        .create_newswire_space(space_input("River City Wire"))
        .expect("create space");
    let post = author
        .create_newswire_post(post_input(&space.entry_id, "Free breakfast"))
        .expect("create post");
    let reference = author
        .newswire_share_reference(space.entry_id.clone())
        .expect("share reference");

    // A member with no editorial role at all.
    let member = open_local_profile().expect("member profile");
    member
        .join_newswire_community(
            riot_ffi::PublicSpace {
                namespace_id: reference.namespace_id.clone(),
                title: "River City Wire".into(),
                is_public: true,
            },
            reference.descriptor_entry_id.clone(),
            Vec::new(),
        )
        .expect("join community");
    carry(&member, space.signed_bytes.clone());
    carry(&member, post.signed_bytes.clone());

    let reply = member
        .create_newswire_comment(
            space.entry_id.clone(),
            post.entry_id.clone(),
            "I said something I want to take back.".into(),
            "en".into(),
        )
        .expect("member replies");

    let retraction = member.create_newswire_editorial_action(NewswireEditorialActionInput {
        space_descriptor_entry_id: space.entry_id.clone(),
        target_entry_id: reply.entry_id.clone(),
        kind: NewswireEditorialActionKind::Retract,
        reason: Some("I was wrong.".into()),
        correction_text: None,
    });
    assert!(
        retraction.is_ok(),
        "an author must be able to withdraw their own words without an editorial \
         role, got {:?}",
        retraction.err()
    );
}
