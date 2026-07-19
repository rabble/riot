use std::collections::BTreeMap;

use crate::import::join::PrefixedEntry;
use crate::session::EvidenceStore;
use crate::willow::{encode_entry, EntryId, Path};

use super::{
    contributors, inspect_verified_components, project, ContributorRowV1, NewswirePayload,
    NewswireProjection, NewswireProjectionError, ProjectionClockV1, VerifiedNewswireRecord,
    MAX_PROJECTED_RECORDS,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewswireStoreError {
    DescriptorNotFound,
    DuplicateDescriptor,
    MissingRetainedPayload,
    MalformedRetainedRecord,
    EntryIdMismatch,
    DescriptorMismatch,
    NamespaceMismatch,
    ConflictingDuplicate,
    ProjectionLimitExceeded,
    StoreQueryFailed,
    DescriptorInvalid,
    ClockUnavailable,
    ClockOutOfRange,
}

impl std::fmt::Display for NewswireStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let code = match self {
            Self::DescriptorNotFound => "DESCRIPTOR_NOT_FOUND",
            Self::DuplicateDescriptor => "DUPLICATE_DESCRIPTOR",
            Self::MissingRetainedPayload => "MISSING_RETAINED_PAYLOAD",
            Self::MalformedRetainedRecord => "MALFORMED_RETAINED_RECORD",
            Self::EntryIdMismatch => "ENTRY_ID_MISMATCH",
            Self::DescriptorMismatch => "DESCRIPTOR_MISMATCH",
            Self::NamespaceMismatch => "NAMESPACE_MISMATCH",
            Self::ConflictingDuplicate => "CONFLICTING_DUPLICATE",
            Self::ProjectionLimitExceeded => "PROJECTION_LIMIT_EXCEEDED",
            Self::StoreQueryFailed => "STORE_QUERY_FAILED",
            Self::DescriptorInvalid => "DESCRIPTOR_INVALID",
            Self::ClockUnavailable => "CLOCK_UNAVAILABLE",
            Self::ClockOutOfRange => "CLOCK_OUT_OF_RANGE",
        };
        f.write_str(code)
    }
}

impl std::error::Error for NewswireStoreError {}

impl From<NewswireProjectionError> for NewswireStoreError {
    fn from(error: NewswireProjectionError) -> Self {
        match error {
            NewswireProjectionError::DescriptorInvalid => Self::DescriptorInvalid,
            NewswireProjectionError::ConflictingDuplicate => Self::ConflictingDuplicate,
            NewswireProjectionError::ProjectionLimitExceeded => Self::ProjectionLimitExceeded,
            NewswireProjectionError::ClockUnavailable => Self::ClockUnavailable,
            NewswireProjectionError::ClockOutOfRange => Self::ClockOutOfRange,
        }
    }
}

pub fn load_space_descriptor(
    store: &EvidenceStore,
    descriptor_id: EntryId,
) -> Result<VerifiedNewswireRecord, NewswireStoreError> {
    let prefix = Path::from_slices(&[b"newswire", b"v1", b"descriptors"])
        .map_err(|_| NewswireStoreError::StoreQueryFailed)?;
    let matches = store
        .entries_with_prefix(&prefix)
        .map_err(|_| NewswireStoreError::StoreQueryFailed)?
        .into_iter()
        .filter(|(stored_id, _, _)| *stored_id == descriptor_id)
        .collect();
    decode_descriptor_entries(descriptor_id, matches)
}

pub fn load_space_records(
    store: &EvidenceStore,
    descriptor_id: EntryId,
) -> Result<Vec<VerifiedNewswireRecord>, NewswireStoreError> {
    let descriptor = load_space_descriptor(store, descriptor_id)?;
    let namespace_id = descriptor.namespace_id();
    let posts = Path::from_slices(&[b"newswire", b"v1", &descriptor_id, b"posts"])
        .map_err(|_| NewswireStoreError::StoreQueryFailed)?;
    let actions = Path::from_slices(&[b"newswire", b"v1", &descriptor_id, b"actions"])
        .map_err(|_| NewswireStoreError::StoreQueryFailed)?;
    let comments = Path::from_slices(&[b"newswire", b"v1", &descriptor_id, b"comments"])
        .map_err(|_| NewswireStoreError::StoreQueryFailed)?;
    let mut entries = store
        .entries_with_prefix_in_namespace(&namespace_id, &posts)
        .map_err(|_| NewswireStoreError::StoreQueryFailed)?;
    entries.extend(
        store
            .entries_with_prefix_in_namespace(&namespace_id, &actions)
            .map_err(|_| NewswireStoreError::StoreQueryFailed)?,
    );
    entries.extend(
        store
            .entries_with_prefix_in_namespace(&namespace_id, &comments)
            .map_err(|_| NewswireStoreError::StoreQueryFailed)?,
    );
    let records = decode_scanned_entries(descriptor_id, entries)?;
    if records
        .iter()
        .any(|record| record.namespace_id() != namespace_id)
    {
        return Err(NewswireStoreError::NamespaceMismatch);
    }
    Ok(records)
}

pub fn project_space(
    store: &EvidenceStore,
    descriptor_id: EntryId,
    clock: ProjectionClockV1,
) -> Result<NewswireProjection, NewswireStoreError> {
    let descriptor = load_space_descriptor(store, descriptor_id)?;
    let records = load_space_records(store, descriptor_id)?;
    project(&descriptor, &records, clock).map_err(Into::into)
}

/// The Known-contributors surface for a space: every distinct author of a
/// signed record it holds, with the recognized organizer marked by the
/// namespace coordinate. Derived from the same descriptor + records the
/// collective projection uses, so it is deterministic across clients.
pub fn contributors_for_space(
    store: &EvidenceStore,
    descriptor_id: EntryId,
    clock: ProjectionClockV1,
) -> Result<Vec<ContributorRowV1>, NewswireStoreError> {
    let descriptor = load_space_descriptor(store, descriptor_id)?;
    let namespace_id = descriptor.namespace_id();
    let records = load_space_records(store, descriptor_id)?;
    let projection = project(&descriptor, &records, clock)?;
    Ok(contributors(&projection, namespace_id))
}

fn decode_scanned_entries(
    descriptor_id: EntryId,
    entries: Vec<PrefixedEntry>,
) -> Result<Vec<VerifiedNewswireRecord>, NewswireStoreError> {
    let mut distinct = BTreeMap::<EntryId, PrefixedEntry>::new();
    for entry in entries {
        if let Some(existing) = distinct.get(&entry.0) {
            if encode_entry(&existing.1) != encode_entry(&entry.1) || existing.2 != entry.2 {
                return Err(NewswireStoreError::ConflictingDuplicate);
            }
            continue;
        }
        distinct.insert(entry.0, entry);
        if distinct.len() > MAX_PROJECTED_RECORDS {
            return Err(NewswireStoreError::ProjectionLimitExceeded);
        }
    }

    let mut records = Vec::with_capacity(distinct.len());
    for entry in distinct.into_values() {
        let record = decode_scanned_entry(entry)?;
        let pinned_descriptor = match record.payload() {
            NewswirePayload::NewsPost(post) => post.space_descriptor_entry_id,
            NewswirePayload::EditorialAction(action) => action.space_descriptor_entry_id,
            NewswirePayload::NewsComment(comment) => comment.space_descriptor_entry_id,
            NewswirePayload::SpaceDescriptor(_) => {
                return Err(NewswireStoreError::DescriptorMismatch);
            }
        };
        if pinned_descriptor != descriptor_id {
            return Err(NewswireStoreError::DescriptorMismatch);
        }
        records.push(record);
    }
    records.sort_by_key(|record| (record.tai_j2000_micros(), record.entry_id()));
    Ok(records)
}

fn decode_descriptor_entries(
    descriptor_id: EntryId,
    mut entries: Vec<PrefixedEntry>,
) -> Result<VerifiedNewswireRecord, NewswireStoreError> {
    match entries.len() {
        0 => return Err(NewswireStoreError::DescriptorNotFound),
        1 => {}
        _ => return Err(NewswireStoreError::DuplicateDescriptor),
    }
    let entry = entries
        .pop()
        .ok_or(NewswireStoreError::DescriptorNotFound)?;
    let record = decode_scanned_entry(entry)?;
    if record.entry_id() != descriptor_id
        || !matches!(record.payload(), NewswirePayload::SpaceDescriptor(_))
    {
        return Err(NewswireStoreError::DescriptorMismatch);
    }
    Ok(record)
}

fn decode_scanned_entry(
    (stored_id, entry, payload): PrefixedEntry,
) -> Result<VerifiedNewswireRecord, NewswireStoreError> {
    let payload = payload.ok_or(NewswireStoreError::MissingRetainedPayload)?;
    let record = inspect_verified_components(&entry, &payload)
        .map_err(|_| NewswireStoreError::MalformedRetainedRecord)?;
    if record.entry_id() != stored_id {
        return Err(NewswireStoreError::EntryIdMismatch);
    }
    Ok(record)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::newswire::{
        create_signed_news_comment, create_signed_news_post, create_signed_space_descriptor,
        inspect_news_record, NewsCommentV1, NewsPostV1, SignedNewswireRecord, SpaceDescriptorV1,
    };
    use crate::willow::{
        decode_entry_canonic, generate_communal_author_for_namespace,
        generate_space_organizer_author,
    };

    struct Records {
        descriptor_id: EntryId,
        descriptor: PrefixedEntry,
        post: PrefixedEntry,
        other_post: PrefixedEntry,
    }

    fn tuple(record: &SignedNewswireRecord) -> PrefixedEntry {
        (
            record.entry_id,
            decode_entry_canonic(&record.signed.entry_bytes).expect("entry"),
            Some(record.signed.payload_bytes.clone()),
        )
    }

    fn records() -> Records {
        let organizer = generate_space_organizer_author().expect("organizer");
        let namespace_id = *organizer.namespace_id().as_bytes();
        let writer =
            generate_communal_author_for_namespace(namespace_id).expect("namespace writer");
        let descriptor = create_signed_space_descriptor(
            &organizer,
            SpaceDescriptorV1 {
                namespace_id,
                name: "Unit Newswire".into(),
                summary: "Typed scan fixtures.".into(),
                languages: vec!["en".into()],
                geographic_tags: vec![],
                topic_tags: vec![],
                editorial_roster: vec![],
                predecessor: None,
                successor: None,
            },
        )
        .expect("descriptor");
        let verified = inspect_news_record(&descriptor.signed).expect("verified descriptor");
        let make_post = |headline: &str| NewsPostV1 {
            space_descriptor_entry_id: descriptor.entry_id,
            headline: headline.into(),
            body: "Human report.".into(),
            language: "en".into(),
            event_time_unix_seconds: None,
            expires_at_unix_seconds: None,
            coarse_location: None,
            source_claims: vec!["fixture".into()],
            operational_profile: None,
            ai_assisted: false,
        };
        let post = create_signed_news_post(&writer, &verified, make_post("One")).expect("post");
        let other_post =
            create_signed_news_post(&writer, &verified, make_post("Two")).expect("other post");
        Records {
            descriptor_id: descriptor.entry_id,
            descriptor: tuple(&descriptor),
            post: tuple(&post),
            other_post: tuple(&other_post),
        }
    }

    #[test]
    fn duplicate_descriptor_id_is_rejected() {
        let records = records();
        assert_eq!(
            decode_descriptor_entries(
                records.descriptor_id,
                vec![records.descriptor.clone(), records.descriptor]
            ),
            Err(NewswireStoreError::DuplicateDescriptor)
        );
    }

    #[test]
    fn missing_payload_is_rejected() {
        let records = records();
        let mut post = records.post;
        post.2 = None;
        assert_eq!(
            decode_scanned_entries(records.descriptor_id, vec![post]),
            Err(NewswireStoreError::MissingRetainedPayload)
        );
    }

    #[test]
    fn malformed_payload_is_rejected() {
        let records = records();
        let mut post = records.post;
        post.2 = Some(b"not the retained payload".to_vec());
        assert_eq!(
            decode_scanned_entries(records.descriptor_id, vec![post]),
            Err(NewswireStoreError::MalformedRetainedRecord)
        );
    }

    #[test]
    fn tuple_entry_id_mismatch_is_rejected() {
        let records = records();
        let mut post = records.post;
        post.0 = [0x44; 32];
        assert_eq!(
            decode_scanned_entries(records.descriptor_id, vec![post]),
            Err(NewswireStoreError::EntryIdMismatch)
        );
    }

    #[test]
    fn path_payload_mismatch_is_rejected() {
        let records = records();
        let mut post = records.post;
        post.2 = records.descriptor.2;
        assert_eq!(
            decode_scanned_entries(records.descriptor_id, vec![post]),
            Err(NewswireStoreError::MalformedRetainedRecord)
        );
    }

    #[test]
    fn record_pinned_to_another_descriptor_is_rejected() {
        let records = records();
        assert_eq!(
            decode_scanned_entries([0x91; 32], vec![records.post]),
            Err(NewswireStoreError::DescriptorMismatch)
        );
    }

    #[test]
    fn signed_comment_decodes_and_pins_to_its_descriptor() {
        let organizer = generate_space_organizer_author().expect("organizer");
        let namespace_id = *organizer.namespace_id().as_bytes();
        let writer =
            generate_communal_author_for_namespace(namespace_id).expect("namespace writer");
        let descriptor = create_signed_space_descriptor(
            &organizer,
            SpaceDescriptorV1 {
                namespace_id,
                name: "Unit Newswire".into(),
                summary: "Comment scan fixture.".into(),
                languages: vec!["en".into()],
                geographic_tags: vec![],
                topic_tags: vec![],
                editorial_roster: vec![],
                predecessor: None,
                successor: None,
            },
        )
        .expect("descriptor");
        let verified = inspect_news_record(&descriptor.signed).expect("verified descriptor");
        let comment = create_signed_news_comment(
            &writer,
            &verified,
            NewsCommentV1 {
                space_descriptor_entry_id: descriptor.entry_id,
                parent_entry_id: [0x77; 32],
                body: "Reply on the wire.".into(),
                language: "en".into(),
            },
        )
        .expect("comment");

        let decoded =
            decode_scanned_entries(descriptor.entry_id, vec![tuple(&comment)]).expect("decoded");
        assert_eq!(decoded.len(), 1);
        assert!(matches!(
            decoded[0].payload(),
            NewswirePayload::NewsComment(_)
        ));

        // A comment pinned to a different descriptor is rejected by the scan.
        assert_eq!(
            decode_scanned_entries([0x91; 32], vec![tuple(&comment)]),
            Err(NewswireStoreError::DescriptorMismatch)
        );
    }

    #[test]
    fn conflicting_duplicate_is_rejected() {
        let records = records();
        let mut conflicting = records.other_post;
        conflicting.0 = records.post.0;
        assert_eq!(
            decode_scanned_entries(records.descriptor_id, vec![records.post, conflicting]),
            Err(NewswireStoreError::ConflictingDuplicate)
        );
    }

    #[test]
    fn more_than_1024_distinct_records_is_rejected_before_partial_decode() {
        let records = records();
        let entries = (0..=1_024u64)
            .map(|index| {
                let mut tuple = records.post.clone();
                tuple.0[..8].copy_from_slice(&index.to_be_bytes());
                tuple
            })
            .collect();
        assert_eq!(
            decode_scanned_entries(records.descriptor_id, entries),
            Err(NewswireStoreError::ProjectionLimitExceeded)
        );
    }
}
