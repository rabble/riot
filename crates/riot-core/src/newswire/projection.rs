//! Deterministic collective views derived from immutable Newswire records.

use std::collections::{BTreeMap, BTreeSet};

use crate::willow::{system_snapshot, ClockSnapshot, EntryId};

use super::{
    EditorialActionKind, EditorialActionV1, NewsCommentV1, NewsPostV1, NewswirePayload,
    OperationalProfileV1, VerifiedNewswireRecord,
};

pub const MAX_PROJECTED_RECORDS: usize = 1_024;
pub const MAX_FUTURE_SKEW_MICROS: u64 = 600_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewswireProjectionError {
    DescriptorInvalid,
    ConflictingDuplicate,
    ProjectionLimitExceeded,
    ClockUnavailable,
    ClockOutOfRange,
}

impl std::fmt::Display for NewswireProjectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let code = match self {
            Self::DescriptorInvalid => "DESCRIPTOR_INVALID",
            Self::ConflictingDuplicate => "CONFLICTING_DUPLICATE",
            Self::ProjectionLimitExceeded => "PROJECTION_LIMIT_EXCEEDED",
            Self::ClockUnavailable => "CLOCK_UNAVAILABLE",
            Self::ClockOutOfRange => "CLOCK_OUT_OF_RANGE",
        };
        f.write_str(code)
    }
}

impl std::error::Error for NewswireProjectionError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectionClockV1 {
    unix_seconds: u64,
    tai_j2000_micros: u64,
}

impl ProjectionClockV1 {
    pub fn system() -> Result<Self, NewswireProjectionError> {
        Ok(Self::from_snapshot(
            system_snapshot().map_err(|_| NewswireProjectionError::ClockUnavailable)?,
        ))
    }

    pub fn unix_seconds(&self) -> u64 {
        self.unix_seconds
    }

    pub fn tai_j2000_micros(&self) -> u64 {
        self.tai_j2000_micros
    }

    fn from_snapshot(snapshot: ClockSnapshot) -> Self {
        Self {
            unix_seconds: snapshot.unix_seconds,
            tai_j2000_micros: snapshot.tai_j2000_micros,
        }
    }

    #[cfg(feature = "conformance")]
    pub fn from_unix_seconds(unix_seconds: i64) -> Result<Self, NewswireProjectionError> {
        let snapshot = crate::willow::snapshot_from_unix_seconds(unix_seconds, 0)
            .map_err(|_| NewswireProjectionError::ClockOutOfRange)?;
        Ok(Self::from_snapshot(snapshot))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PostTreatment {
    Ordinary,
    Hidden { actions: Vec<EntryId> },
    Tombstoned { actions: Vec<EntryId> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedPost {
    pub entry_id: EntryId,
    pub author_id: [u8; 32],
    pub tai_j2000_micros: u64,
    /// The plaintext content half. `headline`, `body`, `coarse_location`,
    /// `source_claims` and `operational_profile` are all REDACTED — set to
    /// `None`/empty — when the post is Hidden or Tombstoned. `language`,
    /// `event_time_unix_seconds`, `expires_at_unix_seconds` and `ai_assisted`
    /// survive redaction: they are metadata, not the content a hide suppresses,
    /// and the expiry is what buckets a row into `earlier` versus the open wire.
    pub headline: Option<String>,
    pub body: Option<String>,
    pub language: String,
    pub coarse_location: Option<String>,
    pub event_time_unix_seconds: Option<u64>,
    pub expires_at_unix_seconds: Option<u64>,
    pub source_claims: Vec<String>,
    pub operational_profile: Option<OperationalProfileV1>,
    pub ai_assisted: bool,
    pub verification_ids: Vec<EntryId>,
    pub correction_ids: Vec<EntryId>,
    pub treatment: PostTreatment,
}

/// A projected communal reply, grouped under its parent post by
/// `parent_entry_id`. `body` is `None` when the comment is Hidden or
/// Tombstoned by an active editorial action; `language`, identity and ordering
/// survive redaction so the row stays accountable — the same discipline as a
/// redacted post. Only comments whose parent is an eligible post appear here;
/// a reply to a comment or to an unheld/expired post is dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedComment {
    pub entry_id: EntryId,
    pub parent_entry_id: EntryId,
    pub author_id: [u8; 32],
    pub tai_j2000_micros: u64,
    pub body: Option<String>,
    pub language: String,
    pub treatment: PostTreatment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedEditorialAction {
    pub entry_id: EntryId,
    pub signer_id: [u8; 32],
    pub tai_j2000_micros: u64,
    pub target_entry_id: EntryId,
    pub kind: super::EditorialActionKind,
    pub reason: Option<String>,
    pub correction_text: Option<String>,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewswireProjection {
    pub open_wire: Vec<ProjectedPost>,
    pub front_page: Vec<ProjectedPost>,
    pub earlier: Vec<ProjectedPost>,
    /// Communal replies across the whole space, each carrying its
    /// `parent_entry_id`. Flat and time-sorted within each parent; the client
    /// groups them under the post rows. Dangling replies are already dropped.
    pub comments: Vec<ProjectedComment>,
    pub future_quarantine: Vec<EntryId>,
    pub editorial_history: Vec<ProjectedEditorialAction>,
}

type ProjectionKey = (u64, EntryId);

/// The plaintext half of a projected post, either carried through verbatim or
/// wholly redacted when the post is Hidden or Tombstoned. Keeping this in one
/// place means an ordinary row and a redacted one differ in exactly these
/// fields and nothing else — a hide can never accidentally leave one behind.
struct ProjectedContent {
    headline: Option<String>,
    body: Option<String>,
    coarse_location: Option<String>,
    source_claims: Vec<String>,
    operational_profile: Option<OperationalProfileV1>,
}

impl ProjectedContent {
    fn from(post: &NewsPostV1, is_ordinary: bool) -> Self {
        if is_ordinary {
            Self {
                headline: Some(post.headline.clone()),
                body: Some(post.body.clone()),
                coarse_location: post.coarse_location.clone(),
                source_claims: post.source_claims.clone(),
                operational_profile: post.operational_profile.clone(),
            }
        } else {
            Self {
                headline: None,
                body: None,
                coarse_location: None,
                source_claims: Vec::new(),
                operational_profile: None,
            }
        }
    }
}

struct EligiblePost<'a> {
    key: ProjectionKey,
    record: &'a VerifiedNewswireRecord,
    post: &'a NewsPostV1,
}

struct EligibleAction<'a> {
    key: ProjectionKey,
    record: &'a VerifiedNewswireRecord,
    action: &'a EditorialActionV1,
    active: bool,
}

struct EligibleComment<'a> {
    record: &'a VerifiedNewswireRecord,
    comment: &'a NewsCommentV1,
}

pub fn project(
    descriptor: &VerifiedNewswireRecord,
    records: &[VerifiedNewswireRecord],
    clock: ProjectionClockV1,
) -> Result<NewswireProjection, NewswireProjectionError> {
    let NewswirePayload::SpaceDescriptor(payload) = descriptor.payload() else {
        return Err(NewswireProjectionError::DescriptorInvalid);
    };
    if payload.namespace_id != descriptor.namespace_id()
        || payload.namespace_id != descriptor.signer_id()
    {
        return Err(NewswireProjectionError::DescriptorInvalid);
    }
    let future_cutoff = clock
        .tai_j2000_micros
        .checked_add(MAX_FUTURE_SKEW_MICROS)
        .ok_or(NewswireProjectionError::ClockOutOfRange)?;

    let mut distinct = BTreeMap::new();
    for record in records {
        match distinct.get(&record.entry_id()) {
            Some(existing) if *existing != record => {
                return Err(NewswireProjectionError::ConflictingDuplicate);
            }
            Some(_) => {}
            None => {
                distinct.insert(record.entry_id(), record);
                if distinct.len() > MAX_PROJECTED_RECORDS {
                    return Err(NewswireProjectionError::ProjectionLimitExceeded);
                }
            }
        }
    }

    let descriptor_id = descriptor.entry_id();
    let namespace_id = descriptor.namespace_id();
    let roster = payload
        .editorial_roster
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut posts = Vec::new();
    let mut actions = Vec::new();
    let mut comments = Vec::new();
    let mut future = Vec::new();

    for record in distinct.into_values() {
        if record.namespace_id() != namespace_id {
            continue;
        }
        let key = (record.tai_j2000_micros(), record.entry_id());
        match record.payload() {
            NewswirePayload::NewsPost(post) if post.space_descriptor_entry_id == descriptor_id => {
                if record.tai_j2000_micros() > future_cutoff {
                    future.push(key);
                } else {
                    posts.push(EligiblePost { key, record, post });
                }
            }
            NewswirePayload::EditorialAction(action)
                if action.space_descriptor_entry_id == descriptor_id
                    && roster.contains(&record.signer_id()) =>
            {
                if record.tai_j2000_micros() > future_cutoff {
                    future.push(key);
                } else {
                    actions.push(EligibleAction {
                        key,
                        record,
                        action,
                        active: action.kind != EditorialActionKind::Retract,
                    });
                }
            }
            NewswirePayload::NewsComment(comment)
                if comment.space_descriptor_entry_id == descriptor_id =>
            {
                if record.tai_j2000_micros() > future_cutoff {
                    future.push(key);
                } else {
                    comments.push(EligibleComment { record, comment });
                }
            }
            _ => {}
        }
    }

    posts.sort_by_key(|post| std::cmp::Reverse(post.key));
    actions.sort_by_key(|action| action.key);
    future.sort_unstable();

    let eligible_post_ids = posts
        .iter()
        .map(|post| post.record.entry_id())
        .collect::<BTreeSet<_>>();
    let eligible_comment_ids = comments
        .iter()
        .map(|comment| comment.record.entry_id())
        .collect::<BTreeSet<_>>();
    // An editorial action is active when it targets an eligible post OR an
    // eligible comment — moderation reaches communal replies exactly as it
    // reaches posts (per content, never per person).
    for action in &mut actions {
        if action.action.kind != EditorialActionKind::Retract {
            action.active = eligible_post_ids.contains(&action.action.target_entry_id)
                || eligible_comment_ids.contains(&action.action.target_entry_id);
        }
    }

    let action_indexes = actions
        .iter()
        .enumerate()
        .map(|(index, action)| (action.record.entry_id(), index))
        .collect::<BTreeMap<_, _>>();
    let mut retracted = BTreeSet::new();
    // Retractions are evaluated against pre-retraction target eligibility. This
    // keeps every strictly later valid suppressing act active, independent of
    // input order, while applying their shared suppression after the scan.
    for index in 0..actions.len() {
        if actions[index].action.kind != EditorialActionKind::Retract {
            continue;
        }
        let Some(&target_index) = action_indexes.get(&actions[index].action.target_entry_id) else {
            continue;
        };
        if actions[target_index].action.kind == EditorialActionKind::Retract
            || !actions[target_index].active
            || actions[target_index].key >= actions[index].key
        {
            continue;
        }
        actions[index].active = true;
        retracted.insert(actions[target_index].record.entry_id());
    }
    for action in &mut actions {
        if action.action.kind != EditorialActionKind::Retract {
            action.active &= !retracted.contains(&action.record.entry_id());
        }
    }

    // Every entry id — post OR comment — an active hide/tombstone redacts.
    let plaintext_redacted_target_ids = actions
        .iter()
        .filter(|action| {
            action.active
                && matches!(
                    action.action.kind,
                    EditorialActionKind::Hide | EditorialActionKind::Tombstone
                )
        })
        .map(|action| action.action.target_entry_id)
        .collect::<BTreeSet<_>>();

    let editorial_history = actions
        .iter()
        .map(|action| ProjectedEditorialAction {
            entry_id: action.record.entry_id(),
            signer_id: action.record.signer_id(),
            tai_j2000_micros: action.record.tai_j2000_micros(),
            target_entry_id: action.action.target_entry_id,
            kind: action.action.kind,
            reason: action.action.reason.clone(),
            correction_text: if action.action.kind == EditorialActionKind::Correct
                && plaintext_redacted_target_ids.contains(&action.action.target_entry_id)
            {
                None
            } else {
                action.action.correction_text.clone()
            },
            active: action.active,
        })
        .collect();

    let mut open_wire = Vec::new();
    let mut earlier = Vec::new();
    let mut featured = Vec::new();
    for eligible in posts {
        let targeted = actions
            .iter()
            .filter(|action| {
                action.active
                    && action.action.kind != EditorialActionKind::Retract
                    && action.action.target_entry_id == eligible.record.entry_id()
            })
            .collect::<Vec<_>>();
        let verification_ids = targeted
            .iter()
            .filter(|action| action.action.kind == EditorialActionKind::Verify)
            .map(|action| action.record.entry_id())
            .collect();
        let hide_ids = targeted
            .iter()
            .filter(|action| action.action.kind == EditorialActionKind::Hide)
            .map(|action| action.record.entry_id())
            .collect::<Vec<_>>();
        let tombstone_ids = targeted
            .iter()
            .filter(|action| action.action.kind == EditorialActionKind::Tombstone)
            .map(|action| action.record.entry_id())
            .collect::<Vec<_>>();
        let correction_ids = if tombstone_ids.is_empty() {
            targeted
                .iter()
                .filter(|action| action.action.kind == EditorialActionKind::Correct)
                .map(|action| action.record.entry_id())
                .collect()
        } else {
            Vec::new()
        };
        let feature_key = targeted
            .iter()
            .filter(|action| action.action.kind == EditorialActionKind::Feature)
            .map(|action| action.key)
            .max();

        let treatment = if !tombstone_ids.is_empty() {
            PostTreatment::Tombstoned {
                actions: tombstone_ids,
            }
        } else if !hide_ids.is_empty() {
            PostTreatment::Hidden { actions: hide_ids }
        } else {
            PostTreatment::Ordinary
        };
        let is_ordinary = treatment == PostTreatment::Ordinary;
        // Redacted rows keep their identity, ordering, and freshness metadata
        // but surrender every plaintext field a hide is meant to suppress.
        let content = ProjectedContent::from(eligible.post, is_ordinary);
        let projected = ProjectedPost {
            entry_id: eligible.record.entry_id(),
            author_id: eligible.record.signer_id(),
            tai_j2000_micros: eligible.record.tai_j2000_micros(),
            headline: content.headline,
            body: content.body,
            language: eligible.post.language.clone(),
            coarse_location: content.coarse_location,
            event_time_unix_seconds: eligible.post.event_time_unix_seconds,
            expires_at_unix_seconds: eligible.post.expires_at_unix_seconds,
            source_claims: content.source_claims,
            operational_profile: content.operational_profile,
            ai_assisted: eligible.post.ai_assisted,
            verification_ids,
            correction_ids,
            treatment,
        };
        let expired = eligible
            .post
            .expires_at_unix_seconds
            .is_some_and(|expires_at| expires_at <= clock.unix_seconds);
        if expired {
            earlier.push(projected);
        } else {
            if is_ordinary {
                if let Some(key) = feature_key {
                    featured.push((key, projected.clone()));
                }
            }
            open_wire.push(projected);
        }
    }
    featured.sort_by_key(|post| std::cmp::Reverse(post.0));

    let mut projected_comments = Vec::new();
    for eligible in comments {
        // Flat v1: a reply attaches to a POST. A parent that is not an eligible
        // post — unheld, expired, wrong-space, or itself a comment — makes this
        // reply dangling, and it is dropped rather than orphaned or crashed.
        if !eligible_post_ids.contains(&eligible.comment.parent_entry_id) {
            continue;
        }
        let comment_id = eligible.record.entry_id();
        let targeted = actions
            .iter()
            .filter(|action| action.active && action.action.target_entry_id == comment_id)
            .collect::<Vec<_>>();
        let hide_ids = targeted
            .iter()
            .filter(|action| action.action.kind == EditorialActionKind::Hide)
            .map(|action| action.record.entry_id())
            .collect::<Vec<_>>();
        let tombstone_ids = targeted
            .iter()
            .filter(|action| action.action.kind == EditorialActionKind::Tombstone)
            .map(|action| action.record.entry_id())
            .collect::<Vec<_>>();
        let treatment = if !tombstone_ids.is_empty() {
            PostTreatment::Tombstoned {
                actions: tombstone_ids,
            }
        } else if !hide_ids.is_empty() {
            PostTreatment::Hidden { actions: hide_ids }
        } else {
            PostTreatment::Ordinary
        };
        // A hidden or tombstoned reply surrenders its body but keeps identity,
        // ordering and language — the same redaction contract as a post.
        let body = if treatment == PostTreatment::Ordinary {
            Some(eligible.comment.body.clone())
        } else {
            None
        };
        projected_comments.push(ProjectedComment {
            entry_id: comment_id,
            parent_entry_id: eligible.comment.parent_entry_id,
            author_id: eligible.record.signer_id(),
            tai_j2000_micros: eligible.record.tai_j2000_micros(),
            body,
            language: eligible.comment.language.clone(),
            treatment,
        });
    }
    projected_comments.sort_by(|a, b| {
        (a.parent_entry_id, a.tai_j2000_micros, a.entry_id).cmp(&(
            b.parent_entry_id,
            b.tai_j2000_micros,
            b.entry_id,
        ))
    });

    Ok(NewswireProjection {
        open_wire,
        front_page: featured.into_iter().map(|(_, post)| post).collect(),
        earlier,
        comments: projected_comments,
        future_quarantine: future.into_iter().map(|(_, entry_id)| entry_id).collect(),
        editorial_history,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::newswire::{EditorialActionKind, EditorialActionV1, NewsPostV1, SpaceDescriptorV1};

    const NAMESPACE: [u8; 32] = [0x20; 32];
    const AUTHOR: [u8; 32] = [0x30; 32];
    const EDITOR: [u8; 32] = [0x40; 32];
    const EDITOR_TWO: [u8; 32] = [0x41; 32];

    fn id(value: u32) -> EntryId {
        let mut id = [0; 32];
        id[28..].copy_from_slice(&value.to_be_bytes());
        id
    }

    fn spread_id(early: u8, middle: u8, final_byte: u8) -> EntryId {
        let mut id = [0; 32];
        id[0] = early;
        id[15] = middle;
        id[31] = final_byte;
        id
    }

    fn clock() -> ProjectionClockV1 {
        ProjectionClockV1 {
            unix_seconds: 1_800_000_000,
            tai_j2000_micros: 100,
        }
    }

    #[test]
    fn projection_clock_preserves_the_full_unsigned_snapshot_value() {
        let clock = ProjectionClockV1::from_snapshot(ClockSnapshot {
            unix_seconds: u64::MAX,
            tai_j2000_micros: 42,
            uncertainty_seconds: 0,
        });
        let unix_seconds: u64 = clock.unix_seconds();
        assert_eq!(unix_seconds, u64::MAX);
        assert_eq!(clock.tai_j2000_micros(), 42);
    }

    fn record(
        entry_id: EntryId,
        namespace_id: [u8; 32],
        signer_id: [u8; 32],
        tai_j2000_micros: u64,
        payload: NewswirePayload,
    ) -> VerifiedNewswireRecord {
        VerifiedNewswireRecord::new_for_projection_tests(
            entry_id,
            namespace_id,
            signer_id,
            tai_j2000_micros,
            payload,
        )
    }

    fn descriptor_with(
        entry_id: EntryId,
        namespace_id: [u8; 32],
        signer_id: [u8; 32],
        payload_namespace_id: [u8; 32],
        roster: Vec<[u8; 32]>,
    ) -> VerifiedNewswireRecord {
        record(
            entry_id,
            namespace_id,
            signer_id,
            1,
            NewswirePayload::SpaceDescriptor(SpaceDescriptorV1 {
                namespace_id: payload_namespace_id,
                name: "Test Newswire".into(),
                summary: "Local human reporting.".into(),
                languages: vec!["en".into()],
                geographic_tags: vec![],
                topic_tags: vec![],
                editorial_roster: roster,
                predecessor: None,
                successor: None,
            }),
        )
    }

    fn descriptor() -> VerifiedNewswireRecord {
        descriptor_with(
            id(1),
            NAMESPACE,
            NAMESPACE,
            NAMESPACE,
            vec![EDITOR, EDITOR_TWO],
        )
    }

    fn post_bound_to(
        entry_id: EntryId,
        time: u64,
        descriptor_id: EntryId,
        expires_at: Option<u64>,
    ) -> VerifiedNewswireRecord {
        record(
            entry_id,
            NAMESPACE,
            AUTHOR,
            time,
            NewswirePayload::NewsPost(NewsPostV1 {
                space_descriptor_entry_id: descriptor_id,
                headline: format!("Report {entry_id:?}"),
                body: format!("Original body {entry_id:?}"),
                language: "en".into(),
                event_time_unix_seconds: None,
                expires_at_unix_seconds: expires_at,
                coarse_location: Some("north pier".into()),
                source_claims: vec!["eyewitness".into()],
                operational_profile: None,
                ai_assisted: false,
            }),
        )
    }

    fn post(entry_id: EntryId, time: u64, expires_at: Option<u64>) -> VerifiedNewswireRecord {
        post_bound_to(entry_id, time, id(1), expires_at)
    }

    fn action_bound_to(
        entry_id: EntryId,
        time: u64,
        target_entry_id: EntryId,
        kind: EditorialActionKind,
        signer_id: [u8; 32],
        descriptor_id: EntryId,
    ) -> VerifiedNewswireRecord {
        let reason = matches!(
            kind,
            EditorialActionKind::Correct
                | EditorialActionKind::Hide
                | EditorialActionKind::Tombstone
                | EditorialActionKind::Retract
        )
        .then(|| format!("Reason {entry_id:?}"));
        let correction_text =
            (kind == EditorialActionKind::Correct).then(|| format!("Correction {entry_id:?}"));
        record(
            entry_id,
            NAMESPACE,
            signer_id,
            time,
            NewswirePayload::EditorialAction(EditorialActionV1 {
                space_descriptor_entry_id: descriptor_id,
                target_entry_id,
                kind,
                reason,
                correction_text,
            }),
        )
    }

    fn action(
        entry_id: EntryId,
        time: u64,
        target_entry_id: EntryId,
        kind: EditorialActionKind,
    ) -> VerifiedNewswireRecord {
        action_bound_to(entry_id, time, target_entry_id, kind, EDITOR, id(1))
    }

    fn projection(records: &[VerifiedNewswireRecord]) -> NewswireProjection {
        project(&descriptor(), records, clock()).unwrap()
    }

    #[test]
    fn descriptor_requires_consistent_namespace_and_founder_binding() {
        for invalid in [
            descriptor_with(id(1), [9; 32], NAMESPACE, NAMESPACE, vec![EDITOR]),
            descriptor_with(id(1), NAMESPACE, [9; 32], NAMESPACE, vec![EDITOR]),
            descriptor_with(id(1), NAMESPACE, NAMESPACE, [9; 32], vec![EDITOR]),
            post(id(2), 2, None),
        ] {
            assert_eq!(
                project(&invalid, &[], clock()),
                Err(NewswireProjectionError::DescriptorInvalid)
            );
        }
    }

    #[test]
    fn eligible_posts_use_descending_time_and_complete_id_order() {
        let records = vec![
            post(id(2), 200, None),
            post(id(4), 100, None),
            post(id(3), 200, None),
        ];
        let view = projection(&records);
        assert_eq!(
            view.open_wire
                .iter()
                .map(|post| post.entry_id)
                .collect::<Vec<_>>(),
            vec![id(3), id(2), id(4)]
        );
        assert!(view.front_page.is_empty());
    }

    #[test]
    fn equal_time_posts_compare_early_middle_and_final_id_bytes() {
        let early = spread_id(1, 0, 0);
        let middle = spread_id(1, 1, 0);
        let final_byte = spread_id(1, 1, 1);
        let greatest_early = spread_id(2, 0, 0);
        let view = projection(&[
            post(middle, 200, None),
            post(greatest_early, 200, None),
            post(early, 200, None),
            post(final_byte, 200, None),
        ]);
        assert_eq!(
            view.open_wire
                .iter()
                .map(|post| post.entry_id)
                .collect::<Vec<_>>(),
            vec![greatest_early, final_byte, middle, early]
        );
    }

    #[test]
    fn exact_duplicate_collapses_and_conflicting_duplicate_errors() {
        let first = post(id(2), 200, None);
        let duplicate = first.clone();
        assert_eq!(projection(&[first.clone(), duplicate]).open_wire.len(), 1);

        let conflict = post(id(2), 201, None);
        assert_eq!(
            project(&descriptor(), &[first, conflict], clock()),
            Err(NewswireProjectionError::ConflictingDuplicate)
        );
    }

    #[test]
    fn projection_accepts_1024_distinct_records_but_rejects_1025() {
        let mut records = (0..MAX_PROJECTED_RECORDS)
            .map(|index| post(id(index as u32 + 10), index as u64, None))
            .collect::<Vec<_>>();
        records.push(records[0].clone());
        assert_eq!(projection(&records).open_wire.len(), MAX_PROJECTED_RECORDS);

        records.push(post(id(2_000), 2_000, None));
        assert_eq!(
            project(&descriptor(), &records, clock()),
            Err(NewswireProjectionError::ProjectionLimitExceeded)
        );
        assert_eq!(
            NewswireProjectionError::ProjectionLimitExceeded.to_string(),
            "PROJECTION_LIMIT_EXCEEDED"
        );
    }

    #[test]
    fn expired_posts_move_to_earlier_only() {
        let view = projection(&[
            post(id(2), 10, Some(clock().unix_seconds)),
            post(id(3), 20, Some(clock().unix_seconds + 1)),
        ]);
        assert_eq!(view.open_wire[0].entry_id, id(3));
        assert_eq!(view.earlier[0].entry_id, id(2));
        assert!(view.front_page.is_empty());
    }

    #[test]
    fn future_records_go_to_quarantine_only() {
        let cutoff = clock().tai_j2000_micros + MAX_FUTURE_SKEW_MICROS;
        let future_post = post(id(2), cutoff + 1, None);
        let future_action = action(id(3), cutoff + 2, id(2), EditorialActionKind::Feature);
        let view = projection(&[future_action, future_post]);
        assert!(view.open_wire.is_empty());
        assert!(view.front_page.is_empty());
        assert!(view.editorial_history.is_empty());
        assert_eq!(view.future_quarantine, vec![id(2), id(3)]);
    }

    #[test]
    fn future_unknown_editor_action_is_not_collective_quarantine() {
        let future_time = clock().tai_j2000_micros + MAX_FUTURE_SKEW_MICROS + 1;
        let view = projection(&[
            post(id(2), 100, None),
            action_bound_to(
                id(10),
                future_time,
                id(2),
                EditorialActionKind::Feature,
                [0x99; 32],
                id(1),
            ),
        ]);
        assert!(view.future_quarantine.is_empty());
        assert!(view.editorial_history.is_empty());
        assert!(view.front_page.is_empty());
    }

    #[test]
    fn future_cutoff_overflow_returns_stable_clock_error() {
        let overflow = ProjectionClockV1 {
            unix_seconds: 1_800_000_000,
            tai_j2000_micros: u64::MAX,
        };
        assert_eq!(
            project(&descriptor(), &[], overflow),
            Err(NewswireProjectionError::ClockOutOfRange)
        );
        assert_eq!(
            NewswireProjectionError::ClockOutOfRange.to_string(),
            "CLOCK_OUT_OF_RANGE"
        );
    }

    #[test]
    fn front_page_uses_each_posts_greatest_active_feature_key() {
        let records = vec![
            post(id(2), 100, None),
            post(id(3), 200, None),
            action(id(10), 300, id(2), EditorialActionKind::Feature),
            action(id(11), 500, id(2), EditorialActionKind::Feature),
            action(id(12), 400, id(3), EditorialActionKind::Feature),
        ];
        let view = projection(&records);
        assert_eq!(
            view.front_page
                .iter()
                .map(|post| post.entry_id)
                .collect::<Vec<_>>(),
            vec![id(2), id(3)]
        );
    }

    #[test]
    fn equal_time_features_compare_the_complete_action_id() {
        let smaller_feature = spread_id(1, 0xff, 0xff);
        let greater_feature = spread_id(2, 0, 0);
        let view = projection(&[
            post(id(2), 100, None),
            post(id(3), 200, None),
            action_bound_to(
                smaller_feature,
                500,
                id(3),
                EditorialActionKind::Feature,
                EDITOR,
                id(1),
            ),
            action_bound_to(
                greater_feature,
                500,
                id(2),
                EditorialActionKind::Feature,
                EDITOR,
                id(1),
            ),
        ]);
        assert_eq!(
            view.front_page
                .iter()
                .map(|post| post.entry_id)
                .collect::<Vec<_>>(),
            vec![id(2), id(3)]
        );
    }

    #[test]
    fn verification_retains_every_recognized_signer_and_action_without_a_score() {
        let records = vec![
            post(id(2), 100, None),
            action_bound_to(
                id(11),
                300,
                id(2),
                EditorialActionKind::Verify,
                EDITOR_TWO,
                id(1),
            ),
            action_bound_to(
                id(10),
                200,
                id(2),
                EditorialActionKind::Verify,
                EDITOR,
                id(1),
            ),
        ];
        let view = projection(&records);
        assert_eq!(view.open_wire[0].verification_ids, vec![id(10), id(11)]);
        assert_eq!(
            view.editorial_history
                .iter()
                .map(|item| (item.entry_id, item.signer_id, item.active))
                .collect::<Vec<_>>(),
            vec![(id(10), EDITOR, true), (id(11), EDITOR_TWO, true)]
        );
    }

    #[test]
    fn corrections_preserve_original_and_ordered_action_details() {
        let original = post(id(2), 100, None);
        let original_body = match original.payload() {
            NewswirePayload::NewsPost(post) => post.body.clone(),
            _ => unreachable!(),
        };
        let records = vec![
            original,
            action(id(11), 300, id(2), EditorialActionKind::Correct),
            action(id(10), 200, id(2), EditorialActionKind::Correct),
        ];
        let view = projection(&records);
        assert_eq!(
            view.open_wire[0].body.as_deref(),
            Some(original_body.as_str())
        );
        assert_eq!(view.open_wire[0].correction_ids, vec![id(10), id(11)]);
        assert_eq!(
            view.editorial_history
                .iter()
                .map(|item| item.correction_text.clone().unwrap())
                .collect::<Vec<_>>(),
            vec![
                format!("Correction {:?}", id(10)),
                format!("Correction {:?}", id(11))
            ]
        );
    }

    #[test]
    fn ordinary_post_carries_every_signed_content_and_metadata_field() {
        let mut signed = NewsPostV1 {
            space_descriptor_entry_id: id(1),
            headline: "Assembly at the pier".into(),
            body: "Full witness account.".into(),
            language: "en".into(),
            event_time_unix_seconds: Some(1_700_000_000),
            expires_at_unix_seconds: Some(clock().unix_seconds + 10),
            coarse_location: Some("north pier".into()),
            source_claims: vec!["eyewitness".into(), "organizer".into()],
            operational_profile: None,
            ai_assisted: true,
        };
        signed.operational_profile = Some(super::OperationalProfileV1::Request(
            crate::newswire::RequestProfileV1 {
                kind: crate::newswire::RequestKind::Need,
                needed_by_unix_seconds: Some(clock().unix_seconds + 5),
                contact_instructions: "Ask at the desk.".into(),
            },
        ));
        let view = projection(&[record(
            id(2),
            NAMESPACE,
            AUTHOR,
            100,
            NewswirePayload::NewsPost(signed.clone()),
        )]);
        let row = &view.open_wire[0];
        assert_eq!(row.headline.as_deref(), Some("Assembly at the pier"));
        assert_eq!(row.body.as_deref(), Some("Full witness account."));
        assert_eq!(row.language, "en");
        assert_eq!(row.coarse_location.as_deref(), Some("north pier"));
        assert_eq!(row.event_time_unix_seconds, Some(1_700_000_000));
        assert_eq!(row.expires_at_unix_seconds, Some(clock().unix_seconds + 10));
        assert_eq!(row.source_claims, vec!["eyewitness", "organizer"]);
        assert_eq!(row.operational_profile, signed.operational_profile);
        assert!(row.ai_assisted);
    }

    #[test]
    fn redaction_clears_headline_location_and_profile_but_keeps_metadata() {
        let mut signed = NewsPostV1 {
            space_descriptor_entry_id: id(1),
            headline: "Doxxing headline".into(),
            body: "Private content.".into(),
            language: "en".into(),
            event_time_unix_seconds: Some(1_700_000_000),
            expires_at_unix_seconds: None,
            coarse_location: Some("a private address".into()),
            source_claims: vec!["leak".into()],
            operational_profile: None,
            ai_assisted: true,
        };
        signed.operational_profile = Some(super::OperationalProfileV1::Request(
            crate::newswire::RequestProfileV1 {
                kind: crate::newswire::RequestKind::Need,
                needed_by_unix_seconds: None,
                contact_instructions: "Call this private number.".into(),
            },
        ));
        let view = projection(&[
            record(
                id(2),
                NAMESPACE,
                AUTHOR,
                100,
                NewswirePayload::NewsPost(signed),
            ),
            action(id(10), 200, id(2), EditorialActionKind::Hide),
        ]);
        let row = &view.open_wire[0];
        assert_eq!(row.headline, None);
        assert_eq!(row.body, None);
        assert_eq!(row.coarse_location, None);
        assert!(row.source_claims.is_empty());
        assert_eq!(row.operational_profile, None);
        // Metadata that a hide does not suppress survives.
        assert_eq!(row.language, "en");
        assert_eq!(row.event_time_unix_seconds, Some(1_700_000_000));
        assert!(row.ai_assisted);
    }

    #[test]
    fn hide_treatment_removes_body_and_sources_from_ordinary_projection() {
        let view = projection(&[
            post(id(2), 100, None),
            action(id(10), 200, id(2), EditorialActionKind::Hide),
        ]);
        assert_eq!(view.open_wire[0].body, None);
        assert_eq!(
            view.open_wire[0].treatment,
            PostTreatment::Hidden {
                actions: vec![id(10)]
            }
        );
        assert!(view.open_wire[0].source_claims.is_empty());
    }

    #[test]
    fn hidden_row_redacts_sources_and_correction_plaintext_without_detail_api() {
        let view = projection(&[
            post(id(2), 100, None),
            action(id(10), 200, id(2), EditorialActionKind::Correct),
            action(id(11), 300, id(2), EditorialActionKind::Hide),
        ]);
        let row = &view.open_wire[0];
        let correction = view
            .editorial_history
            .iter()
            .find(|action| action.entry_id == id(10))
            .unwrap();
        assert_eq!(
            (
                row.source_claims.clone(),
                correction.correction_text.clone()
            ),
            (Vec::<String>::new(), None)
        );
        assert_eq!(row.body, None);
        assert_eq!(row.correction_ids, vec![id(10)]);
        assert_eq!(correction.entry_id, id(10));
        assert_eq!(correction.signer_id, EDITOR);
        assert_eq!(correction.tai_j2000_micros, 200);
        assert_eq!(correction.kind, EditorialActionKind::Correct);
        assert!(correction.reason.is_some());
        assert!(correction.active);
    }

    #[test]
    fn tombstone_redacts_payload_but_retains_identity_and_history_ids() {
        let view = projection(&[
            post(id(2), 100, None),
            action(id(10), 200, id(2), EditorialActionKind::Correct),
            action(id(11), 300, id(2), EditorialActionKind::Tombstone),
        ]);
        let row = &view.open_wire[0];
        assert_eq!(row.entry_id, id(2));
        assert_eq!(row.author_id, AUTHOR);
        assert_eq!(row.tai_j2000_micros, 100);
        assert_eq!(row.body, None);
        assert!(row.source_claims.is_empty());
        assert!(row.correction_ids.is_empty());
        assert_eq!(
            row.treatment,
            PostTreatment::Tombstoned {
                actions: vec![id(11)]
            }
        );
        assert_eq!(view.editorial_history.len(), 2);
        let correction = view
            .editorial_history
            .iter()
            .find(|action| action.entry_id == id(10))
            .unwrap();
        assert_eq!(correction.signer_id, EDITOR);
        assert_eq!(correction.kind, EditorialActionKind::Correct);
        assert!(correction.reason.is_some());
        assert_eq!(correction.correction_text, None);
    }

    #[test]
    fn later_retract_deactivates_action_and_both_remain_in_history() {
        let view = projection(&[
            post(id(2), 100, None),
            action(id(10), 200, id(2), EditorialActionKind::Feature),
            action(id(11), 300, id(10), EditorialActionKind::Retract),
        ]);
        assert!(view.front_page.is_empty());
        assert_eq!(
            view.editorial_history
                .iter()
                .map(|item| (item.entry_id, item.active))
                .collect::<Vec<_>>(),
            vec![(id(10), false), (id(11), true)]
        );
    }

    #[test]
    fn retract_targeting_effectless_action_is_inactive() {
        let view = projection(&[
            post(id(2), 100, None),
            action(id(20), 200, id(99), EditorialActionKind::Feature),
            action(id(21), 300, id(20), EditorialActionKind::Retract),
        ]);
        assert_eq!(
            view.editorial_history
                .iter()
                .map(|action| (action.entry_id, action.active))
                .collect::<Vec<_>>(),
            vec![(id(20), false), (id(21), false)]
        );
        assert!(view.front_page.is_empty());
    }

    #[test]
    fn all_strictly_later_equal_time_retractions_are_active_and_permutation_stable() {
        let feature_id = spread_id(1, 1, 1);
        let earlier_retract_id = spread_id(1, 1, 0);
        let later_retract_one_id = spread_id(1, 2, 0);
        let later_retract_two_id = spread_id(2, 0, 0);
        let records = vec![
            post(id(2), 100, None),
            action_bound_to(
                feature_id,
                200,
                id(2),
                EditorialActionKind::Feature,
                EDITOR,
                id(1),
            ),
            action_bound_to(
                earlier_retract_id,
                200,
                feature_id,
                EditorialActionKind::Retract,
                EDITOR,
                id(1),
            ),
            action_bound_to(
                later_retract_one_id,
                200,
                feature_id,
                EditorialActionKind::Retract,
                EDITOR,
                id(1),
            ),
            action_bound_to(
                later_retract_two_id,
                200,
                feature_id,
                EditorialActionKind::Retract,
                EDITOR,
                id(1),
            ),
        ];
        let expected = projection(&records);
        assert_eq!(
            expected
                .editorial_history
                .iter()
                .map(|action| (action.entry_id, action.active))
                .collect::<Vec<_>>(),
            vec![
                (earlier_retract_id, false),
                (feature_id, false),
                (later_retract_one_id, true),
                (later_retract_two_id, true),
            ]
        );
        assert!(expected.front_page.is_empty());

        let mut reversed = records.clone();
        reversed.reverse();
        assert_eq!(projection(&reversed), expected);
        let mut rotated = records;
        rotated.rotate_left(2);
        assert_eq!(projection(&rotated), expected);
    }

    #[test]
    fn invalid_retract_targets_have_no_effect() {
        let records = vec![
            post(id(2), 100, None),
            action(id(10), 300, id(2), EditorialActionKind::Feature),
            action(id(11), 200, id(99), EditorialActionKind::Retract),
            action(id(12), 400, id(11), EditorialActionKind::Retract),
            action(id(13), 200, id(10), EditorialActionKind::Retract),
            action(id(14), 500, id(98), EditorialActionKind::Retract),
            action_bound_to(
                id(15),
                600,
                id(10),
                EditorialActionKind::Retract,
                EDITOR,
                id(999),
            ),
        ];
        let view = projection(&records);
        assert_eq!(view.front_page[0].entry_id, id(2));
        assert!(
            view.editorial_history
                .iter()
                .find(|item| item.entry_id == id(10))
                .unwrap()
                .active
        );
        assert!(view
            .editorial_history
            .iter()
            .filter(|item| item.kind == EditorialActionKind::Retract)
            .all(|item| !item.active));
        assert!(!view
            .editorial_history
            .iter()
            .any(|item| item.entry_id == id(15)));
    }

    #[test]
    fn in_space_retract_targeting_wrong_space_action_is_inactive_history_only() {
        let wrong_space_action = record(
            id(30),
            [0x22; 32],
            EDITOR,
            200,
            NewswirePayload::EditorialAction(EditorialActionV1 {
                space_descriptor_entry_id: id(1),
                target_entry_id: id(2),
                kind: EditorialActionKind::Feature,
                reason: None,
                correction_text: None,
            }),
        );
        let view = projection(&[
            post(id(2), 100, None),
            wrong_space_action,
            action(id(31), 300, id(30), EditorialActionKind::Retract),
        ]);
        assert!(view.front_page.is_empty());
        assert_eq!(view.open_wire[0].treatment, PostTreatment::Ordinary);
        assert_eq!(
            view.editorial_history
                .iter()
                .map(|action| (action.entry_id, action.active))
                .collect::<Vec<_>>(),
            vec![(id(31), false)]
        );
    }

    #[test]
    fn non_retraction_invalid_targets_stay_history_only() {
        let wrong_space_post = record(
            id(4),
            [0x22; 32],
            AUTHOR,
            120,
            NewswirePayload::NewsPost(NewsPostV1 {
                space_descriptor_entry_id: id(1),
                headline: "Wrong space".into(),
                body: "Not eligible here".into(),
                language: "en".into(),
                event_time_unix_seconds: None,
                expires_at_unix_seconds: None,
                coarse_location: None,
                source_claims: vec![],
                operational_profile: None,
                ai_assisted: false,
            }),
        );
        let records = vec![
            post(id(2), 100, None),
            post_bound_to(id(3), 110, id(999), None),
            wrong_space_post,
            action(id(20), 200, id(21), EditorialActionKind::Feature),
            action(id(21), 210, id(99), EditorialActionKind::Verify),
            action(id(22), 220, id(1), EditorialActionKind::Correct),
            action(id(23), 230, id(3), EditorialActionKind::Hide),
            action(id(24), 240, id(20), EditorialActionKind::Tombstone),
            action(id(25), 250, id(4), EditorialActionKind::Verify),
        ];
        let view = projection(&records);
        assert_eq!(view.open_wire.len(), 1);
        assert_eq!(view.open_wire[0].treatment, PostTreatment::Ordinary);
        assert!(view.front_page.is_empty());
        assert_eq!(view.editorial_history.len(), 6);
        assert!(view.editorial_history.iter().all(|item| !item.active));
    }

    #[test]
    fn unknown_editor_and_wrong_descriptor_have_no_collective_effect() {
        let records = vec![
            post(id(2), 100, None),
            action_bound_to(
                id(10),
                200,
                id(2),
                EditorialActionKind::Feature,
                [0x99; 32],
                id(1),
            ),
            action_bound_to(
                id(11),
                300,
                id(2),
                EditorialActionKind::Tombstone,
                EDITOR,
                id(999),
            ),
        ];
        let view = projection(&records);
        assert_eq!(view.open_wire[0].treatment, PostTreatment::Ordinary);
        assert!(view.front_page.is_empty());
        assert!(view.editorial_history.is_empty());
    }

    fn comment_bound_to(
        entry_id: EntryId,
        time: u64,
        parent_entry_id: EntryId,
        descriptor_id: EntryId,
        signer_id: [u8; 32],
    ) -> VerifiedNewswireRecord {
        record(
            entry_id,
            NAMESPACE,
            signer_id,
            time,
            NewswirePayload::NewsComment(NewsCommentV1 {
                space_descriptor_entry_id: descriptor_id,
                parent_entry_id,
                body: format!("Reply {entry_id:?}"),
                language: "en".into(),
            }),
        )
    }

    fn comment(entry_id: EntryId, time: u64, parent_entry_id: EntryId) -> VerifiedNewswireRecord {
        comment_bound_to(entry_id, time, parent_entry_id, id(1), AUTHOR)
    }

    #[test]
    fn comments_group_under_parent_flat_and_time_sorted() {
        let view = projection(&[
            post(id(2), 100, None),
            post(id(3), 110, None),
            comment(id(20), 300, id(2)),
            comment(id(21), 200, id(2)),
            comment(id(22), 250, id(3)),
        ]);
        assert_eq!(
            view.comments
                .iter()
                .map(|c| (c.entry_id, c.parent_entry_id, c.body.clone()))
                .collect::<Vec<_>>(),
            vec![
                (id(21), id(2), Some(format!("Reply {:?}", id(21)))),
                (id(20), id(2), Some(format!("Reply {:?}", id(20)))),
                (id(22), id(3), Some(format!("Reply {:?}", id(22)))),
            ]
        );
        assert_eq!(view.comments[0].author_id, AUTHOR);
        assert_eq!(view.comments[0].language, "en");
        assert_eq!(view.comments[0].treatment, PostTreatment::Ordinary);
    }

    #[test]
    fn dangling_and_reply_to_comment_are_dropped_without_crashing() {
        let view = projection(&[
            post(id(2), 100, Some(clock().unix_seconds)), // expired -> in `earlier`, still eligible
            comment(id(20), 200, id(2)),                  // parent expired-but-eligible -> KEPT
            comment(id(30), 210, id(999)),                // unknown parent -> dropped
            comment(id(31), 220, id(20)),                 // reply to a comment -> dropped
            comment_bound_to(id(32), 230, id(2), id(777), AUTHOR), // wrong space -> dropped
        ]);
        assert_eq!(
            view.comments.iter().map(|c| c.entry_id).collect::<Vec<_>>(),
            vec![id(20)]
        );
        // The parent post is off the open wire (expired) but the reply survives.
        assert_eq!(view.earlier[0].entry_id, id(2));
    }

    #[test]
    fn editor_tombstone_redacts_a_comment_body_keeping_identity() {
        let view = projection(&[
            post(id(2), 100, None),
            comment(id(20), 200, id(2)),
            action(id(30), 300, id(20), EditorialActionKind::Tombstone),
        ]);
        let row = &view.comments[0];
        assert_eq!(row.entry_id, id(20));
        assert_eq!(row.parent_entry_id, id(2));
        assert_eq!(row.author_id, AUTHOR);
        assert_eq!(row.tai_j2000_micros, 200);
        assert_eq!(row.body, None);
        assert_eq!(row.language, "en");
        assert_eq!(
            row.treatment,
            PostTreatment::Tombstoned {
                actions: vec![id(30)]
            }
        );
        // The moderation act is honestly recorded and active.
        let mod_action = view
            .editorial_history
            .iter()
            .find(|a| a.entry_id == id(30))
            .unwrap();
        assert!(mod_action.active);
        assert_eq!(mod_action.target_entry_id, id(20));
    }

    #[test]
    fn hide_by_non_editor_leaves_comment_ordinary() {
        // A hide signed by someone NOT in the roster has no collective effect.
        let view = projection(&[
            post(id(2), 100, None),
            comment(id(20), 200, id(2)),
            action_bound_to(
                id(30),
                300,
                id(20),
                EditorialActionKind::Hide,
                [0x99; 32],
                id(1),
            ),
        ]);
        assert_eq!(view.comments[0].treatment, PostTreatment::Ordinary);
        assert_eq!(view.comments[0].body, Some(format!("Reply {:?}", id(20))));
        assert!(view.editorial_history.is_empty());
    }

    #[test]
    fn future_comment_is_quarantined_like_a_post() {
        let cutoff = clock().tai_j2000_micros + MAX_FUTURE_SKEW_MICROS;
        let view = projection(&[post(id(2), 100, None), comment(id(20), cutoff + 1, id(2))]);
        assert!(view.comments.is_empty());
        assert_eq!(view.future_quarantine, vec![id(20)]);
    }

    #[test]
    fn comment_permutations_produce_equal_projection() {
        let records = vec![
            post(id(2), 100, None),
            post(id(3), 110, None),
            comment(id(20), 300, id(2)),
            comment(id(21), 200, id(2)),
            comment(id(22), 250, id(3)),
            action(id(30), 400, id(20), EditorialActionKind::Hide),
        ];
        let expected = projection(&records);
        assert_eq!(expected.comments.len(), 3);
        let mut reversed = records.clone();
        reversed.reverse();
        assert_eq!(projection(&reversed), expected);
        let mut rotated = records;
        rotated.rotate_left(2);
        assert_eq!(projection(&rotated), expected);
    }

    #[test]
    fn arrival_permutations_produce_byte_for_byte_equal_projection() {
        let records = vec![
            post(id(2), 100, None),
            post(id(3), 90, Some(clock().unix_seconds)),
            action(id(10), 200, id(2), EditorialActionKind::Feature),
            action(id(11), 210, id(2), EditorialActionKind::Verify),
            action(id(12), 220, id(2), EditorialActionKind::Correct),
            action(id(13), 230, id(2), EditorialActionKind::Hide),
            action(id(14), 240, id(13), EditorialActionKind::Retract),
        ];
        let expected = projection(&records);
        let mut reversed = records.clone();
        reversed.reverse();
        assert_eq!(projection(&reversed), expected);
        let mut rotated = records;
        rotated.rotate_left(3);
        assert_eq!(projection(&rotated), expected);
    }
}
