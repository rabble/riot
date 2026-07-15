//! Public conformance proof for the Known-contributors projection (Unit 1C).
//!
//! A community's People surface is derived, deterministically, from the signed
//! records it already holds — never from a membership roster or presence. A
//! contributor is any distinct author of a signed newswire record in the space
//! (a news post or an editorial action). The recognized organizer is marked
//! ONLY by the coordinate rule `author_id == namespace_id`; nothing a member
//! self-claims can promote them.

use riot_core::newswire::{
    contributors, create_signed_editorial_action_with_clock, create_signed_news_post_with_clock,
    create_signed_space_descriptor_with_clock, inspect_news_record, project, ContributorRowV1,
    EditorialActionKind, EditorialActionV1, NewsPostV1, ProjectionClockV1, SpaceDescriptorV1,
    VerifiedNewswireRecord,
};
use riot_core::willow::{ClockSnapshot, ClockSource, EvidenceAuthor, WillowError};

#[derive(Clone, Copy)]
struct FixedClock(ClockSnapshot);

impl ClockSource for FixedClock {
    fn snapshot(&self) -> Result<ClockSnapshot, WillowError> {
        Ok(self.0)
    }
}

fn founder(mut secret: [u8; 32]) -> EvidenceAuthor {
    loop {
        let subspace_secret = willow25::entry::SubspaceSecret::from_bytes(&secret);
        let subspace_id = subspace_secret.corresponding_subspace_id();
        let namespace_id = willow25::entry::NamespaceId::from_bytes(subspace_id.as_bytes());
        if namespace_id.is_communal() {
            return EvidenceAuthor::from_parts_for_tests(namespace_id, &secret);
        }
        secret[0] = secret[0].wrapping_add(1);
    }
}

fn member(namespace_id: [u8; 32], secret: [u8; 32]) -> EvidenceAuthor {
    EvidenceAuthor::from_parts_for_tests(
        willow25::entry::NamespaceId::from_bytes(&namespace_id),
        &secret,
    )
}

fn fixed_clock(clock: ProjectionClockV1, offset_micros: u64) -> FixedClock {
    let unix_seconds: u64 = clock.unix_seconds();
    FixedClock(ClockSnapshot {
        unix_seconds,
        tai_j2000_micros: clock.tai_j2000_micros() + offset_micros,
        uncertainty_seconds: 0,
    })
}

struct Community {
    clock: ProjectionClockV1,
    organizer: EvidenceAuthor,
    namespace_id: [u8; 32],
    descriptor: VerifiedNewswireRecord,
}

/// A newswire space whose founding roster is the organizer plus any supplied
/// extra editors, so an editorial action from any of them is recognized by the
/// projection.
fn community(extra_editors: &[&EvidenceAuthor]) -> Community {
    let clock = ProjectionClockV1::from_unix_seconds(1_800_000_000).unwrap();
    let organizer = founder([0x41; 32]);
    let namespace_id = *organizer.namespace_id().as_bytes();
    let mut editorial_roster = vec![*organizer.subspace_id().as_bytes()];
    editorial_roster.extend(
        extra_editors
            .iter()
            .map(|author| *author.subspace_id().as_bytes()),
    );
    let descriptor = create_signed_space_descriptor_with_clock(
        &organizer,
        &fixed_clock(clock, 0),
        SpaceDescriptorV1 {
            namespace_id,
            name: "Riverside Newswire".into(),
            summary: "Human-published neighborhood reporting.".into(),
            languages: vec!["en".into()],
            geographic_tags: vec!["riverside".into()],
            topic_tags: vec!["local".into()],
            editorial_roster,
            predecessor: None,
            successor: None,
        },
    )
    .unwrap();
    let descriptor = inspect_news_record(&descriptor.signed).unwrap();
    Community {
        clock,
        organizer,
        namespace_id,
        descriptor,
    }
}

fn signed_post(
    community: &Community,
    author: &EvidenceAuthor,
    offset: u64,
    headline: &str,
) -> VerifiedNewswireRecord {
    let post = create_signed_news_post_with_clock(
        author,
        &community.descriptor,
        &fixed_clock(community.clock, offset),
        NewsPostV1 {
            space_descriptor_entry_id: community.descriptor.entry_id(),
            headline: headline.into(),
            body: "A human-authored report.".into(),
            language: "en".into(),
            event_time_unix_seconds: None,
            expires_at_unix_seconds: None,
            coarse_location: None,
            source_claims: vec![],
            operational_profile: None,
            ai_assisted: false,
        },
    )
    .unwrap();
    inspect_news_record(&post.signed).unwrap()
}

fn signed_feature(
    community: &Community,
    editor: &EvidenceAuthor,
    offset: u64,
    target: &VerifiedNewswireRecord,
) -> VerifiedNewswireRecord {
    let action = create_signed_editorial_action_with_clock(
        editor,
        &community.descriptor,
        &fixed_clock(community.clock, offset),
        EditorialActionV1 {
            space_descriptor_entry_id: community.descriptor.entry_id(),
            target_entry_id: target.entry_id(),
            kind: EditorialActionKind::Feature,
            reason: None,
            correction_text: None,
        },
    )
    .unwrap();
    inspect_news_record(&action.signed).unwrap()
}

fn rows(community: &Community, records: &[VerifiedNewswireRecord]) -> Vec<ContributorRowV1> {
    let projection = project(&community.descriptor, records, community.clock).unwrap();
    contributors(&projection, community.namespace_id)
}

#[test]
fn no_records_yields_no_contributors() {
    let community = community(&[]);
    assert!(rows(&community, &[]).is_empty());
}

#[test]
fn each_distinct_author_becomes_exactly_one_row() {
    let community = community(&[]);
    let member_one = member(community.namespace_id, [0x52; 32]);
    let member_two = member(community.namespace_id, [0x53; 32]);
    let records = vec![
        signed_post(&community, &community.organizer, 1, "Organizer note"),
        signed_post(&community, &member_one, 2, "First report"),
        signed_post(&community, &member_one, 3, "Second report"),
        signed_post(&community, &member_two, 4, "Another report"),
    ];
    let rows = rows(&community, &records);

    // Three DISTINCT authors, one row apiece — three posts by two members do
    // not become three member rows.
    assert_eq!(rows.len(), 3);
    let ids: Vec<[u8; 32]> = rows.iter().map(|row| row.author_id).collect();
    assert_eq!(
        ids.iter().collect::<std::collections::BTreeSet<_>>().len(),
        3
    );
    assert!(ids.contains(&community.namespace_id));
    assert!(ids.contains(&*member_one.subspace_id().as_bytes()));
    assert!(ids.contains(&*member_two.subspace_id().as_bytes()));

    // Content-derived, not a roster: the count reflects the signed records.
    let member_one_row = rows
        .iter()
        .find(|row| row.author_id == *member_one.subspace_id().as_bytes())
        .unwrap();
    assert_eq!(member_one_row.contribution_count, 2);
}

#[test]
fn organizer_flag_comes_only_from_the_recognized_coordinate() {
    let community = community(&[]);
    let member_one = member(community.namespace_id, [0x52; 32]);
    let records = vec![
        signed_post(&community, &community.organizer, 1, "Organizer note"),
        signed_post(&community, &member_one, 2, "Member report"),
    ];
    let rows = rows(&community, &records);

    let organizer_row = rows
        .iter()
        .find(|row| row.author_id == community.namespace_id)
        .unwrap();
    assert!(organizer_row.is_organizer);

    // A member who posts is never promoted — the flag is the coordinate, not
    // activity or any self-claim.
    let member_row = rows
        .iter()
        .find(|row| row.author_id == *member_one.subspace_id().as_bytes())
        .unwrap();
    assert!(!member_row.is_organizer);

    // Exactly one recognized organizer, and it is the namespace coordinate.
    assert_eq!(rows.iter().filter(|row| row.is_organizer).count(), 1);
}

#[test]
fn organizer_sorts_first_then_by_author_id() {
    let community = community(&[]);
    let member_one = member(community.namespace_id, [0x52; 32]);
    let member_two = member(community.namespace_id, [0x53; 32]);
    let records = vec![
        signed_post(&community, &member_two, 1, "Two"),
        signed_post(&community, &member_one, 2, "One"),
        signed_post(&community, &community.organizer, 3, "Organizer"),
    ];
    let rows = rows(&community, &records);
    assert!(rows[0].is_organizer);
    assert_eq!(rows[0].author_id, community.namespace_id);
    // The remaining rows are ordered deterministically by author id.
    let tail: Vec<[u8; 32]> = rows[1..].iter().map(|row| row.author_id).collect();
    let mut sorted = tail.clone();
    sorted.sort();
    assert_eq!(tail, sorted);
}

#[test]
fn an_editor_who_only_acts_is_still_a_contributor() {
    // A recognized editor who never files a post but signs an editorial action
    // has contributed to the community's signed record, and appears.
    let member_one = member(*founder([0x41; 32]).namespace_id().as_bytes(), [0x52; 32]);
    let community = community(&[&member_one]);
    let post = signed_post(&community, &community.organizer, 1, "Report");
    let feature = signed_feature(&community, &member_one, 2, &post);
    let records = vec![post, feature];
    let rows = rows(&community, &records);

    let editor_row = rows
        .iter()
        .find(|row| row.author_id == *member_one.subspace_id().as_bytes())
        .expect("an acting editor is a contributor");
    assert!(!editor_row.is_organizer);
    assert_eq!(editor_row.contribution_count, 1);
}

#[test]
fn a_hidden_posts_author_is_still_counted() {
    // Redaction suppresses a post's plaintext but keeps its author accountable,
    // so the author of a hidden post remains a known contributor.
    let community = community(&[]);
    let member_one = member(community.namespace_id, [0x52; 32]);
    let post = signed_post(&community, &member_one, 1, "Sensitive report");
    let hide = {
        let action = create_signed_editorial_action_with_clock(
            &community.organizer,
            &community.descriptor,
            &fixed_clock(community.clock, 2),
            EditorialActionV1 {
                space_descriptor_entry_id: community.descriptor.entry_id(),
                target_entry_id: post.entry_id(),
                kind: EditorialActionKind::Hide,
                reason: Some("doxxing".into()),
                correction_text: None,
            },
        )
        .unwrap();
        inspect_news_record(&action.signed).unwrap()
    };
    let records = vec![post, hide];
    let rows = rows(&community, &records);
    assert!(rows
        .iter()
        .any(|row| row.author_id == *member_one.subspace_id().as_bytes()));
}
