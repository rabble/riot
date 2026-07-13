//! Public conformance proof for deterministic Newswire projection.

use riot_core::newswire::{
    create_signed_editorial_action_with_clock, create_signed_news_post_with_clock,
    create_signed_space_descriptor_with_clock, inspect_news_record, project, EditorialActionKind,
    EditorialActionV1, NewsPostV1, NewswireProjection, ProjectionClockV1, SpaceDescriptorV1,
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
    FixedClock(ClockSnapshot {
        unix_seconds: clock.unix_seconds() as u64,
        tai_j2000_micros: clock.tai_j2000_micros() + offset_micros,
        uncertainty_seconds: 0,
    })
}

#[test]
fn no_posts_produces_empty_wire_and_front_page() {
    let clock = ProjectionClockV1::from_unix_seconds(1_800_000_000).unwrap();
    let organizer = founder([0x41; 32]);
    let namespace_id = *organizer.namespace_id().as_bytes();
    let descriptor = create_signed_space_descriptor_with_clock(
        &organizer,
        &fixed_clock(clock, 0),
        SpaceDescriptorV1 {
            namespace_id,
            name: "Harbor Newswire".into(),
            summary: "Human-published neighborhood reporting.".into(),
            languages: vec!["en".into()],
            geographic_tags: vec!["harbor".into()],
            topic_tags: vec!["local".into()],
            editorial_roster: vec![],
            predecessor: None,
            successor: None,
        },
    )
    .unwrap();
    let descriptor = inspect_news_record(&descriptor.signed).unwrap();

    assert_eq!(
        project(&descriptor, &[], clock).unwrap(),
        NewswireProjection {
            open_wire: vec![],
            front_page: vec![],
            earlier: vec![],
            future_quarantine: vec![],
            editorial_history: vec![],
        }
    );
}

#[test]
fn inspected_signed_records_feed_the_public_projection_api() {
    let clock = ProjectionClockV1::from_unix_seconds(1_800_000_000).unwrap();
    let organizer = founder([0x51; 32]);
    let namespace_id = *organizer.namespace_id().as_bytes();
    let editor = member(namespace_id, [0x52; 32]);
    let descriptor = create_signed_space_descriptor_with_clock(
        &organizer,
        &fixed_clock(clock, 0),
        SpaceDescriptorV1 {
            namespace_id,
            name: "Harbor Newswire".into(),
            summary: "Human-published neighborhood reporting.".into(),
            languages: vec!["en".into()],
            geographic_tags: vec!["harbor".into()],
            topic_tags: vec!["local".into()],
            editorial_roster: vec![*editor.subspace_id().as_bytes()],
            predecessor: None,
            successor: None,
        },
    )
    .unwrap();
    let descriptor = inspect_news_record(&descriptor.signed).unwrap();
    let post = create_signed_news_post_with_clock(
        &editor,
        &descriptor,
        &fixed_clock(clock, 1),
        NewsPostV1 {
            space_descriptor_entry_id: descriptor.entry_id(),
            headline: "Harbor update".into(),
            body: "A human-authored report.".into(),
            language: "en".into(),
            event_time_unix_seconds: Some(clock.unix_seconds() as u64),
            expires_at_unix_seconds: None,
            coarse_location: Some("north pier".into()),
            source_claims: vec!["eyewitness".into()],
            operational_profile: None,
            ai_assisted: false,
        },
    )
    .unwrap();
    let post_id = post.entry_id;
    let post = inspect_news_record(&post.signed).unwrap();
    let feature = create_signed_editorial_action_with_clock(
        &editor,
        &descriptor,
        &fixed_clock(clock, 2),
        EditorialActionV1 {
            space_descriptor_entry_id: descriptor.entry_id(),
            target_entry_id: post_id,
            kind: EditorialActionKind::Feature,
            reason: None,
            correction_text: None,
        },
    )
    .unwrap();
    let feature = inspect_news_record(&feature.signed).unwrap();

    let view = project(&descriptor, &[feature, post], clock).unwrap();
    assert_eq!(view.open_wire.len(), 1);
    assert_eq!(view.open_wire[0].entry_id, post_id);
    assert_eq!(view.front_page, view.open_wire);
    assert_eq!(view.editorial_history.len(), 1);
    assert!(view.editorial_history[0].active);
}
