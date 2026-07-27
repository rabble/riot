//! The Coordinate ledger store scan — the design's silent **5th registration
//! site**.
//!
//! A Coordinate item is admitted through the import boundary and RETAINED, but
//! it only reaches a projection if a prefix scan discovers it. This module owns
//! that scan: [`load_ledger_records`] builds the exact
//! `coordinate/v1/<descriptor>/items` prefix and reads every retained item back
//! out of the store, mirroring `newswire::store::load_space_records`. A new
//! Coordinate family (status/verify/action, later work units) MUST add its own
//! prefix here or its entries admit, persist, and are never scanned — invisible.
//!
//! The Coordinate ledger has no descriptor of its own: every item binds to a
//! newswire space descriptor, so the scan first resolves that descriptor (for
//! its namespace) via [`crate::newswire::load_space_descriptor`], then scans the
//! Coordinate prefix inside that namespace.

use std::collections::BTreeMap;

use crate::import::join::PrefixedEntry;
use crate::newswire::load_space_descriptor;
use crate::session::EvidenceStore;
use crate::willow::{encode_entry, EntryId, Path};

use super::{inspect_verified_components, VerifiedCoordinateRecord};

/// Ceiling on distinct Coordinate records folded from one scan, matching the
/// newswire projection ceiling (`newswire::MAX_PROJECTED_RECORDS`).
const MAX_LEDGER_RECORDS: usize = 1_024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinateStoreError {
    DescriptorNotFound,
    MissingRetainedPayload,
    MalformedRetainedRecord,
    EntryIdMismatch,
    DescriptorMismatch,
    NamespaceMismatch,
    ConflictingDuplicate,
    ProjectionLimitExceeded,
    StoreQueryFailed,
}

impl std::fmt::Display for CoordinateStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let code = match self {
            Self::DescriptorNotFound => "DESCRIPTOR_NOT_FOUND",
            Self::MissingRetainedPayload => "MISSING_RETAINED_PAYLOAD",
            Self::MalformedRetainedRecord => "MALFORMED_RETAINED_RECORD",
            Self::EntryIdMismatch => "ENTRY_ID_MISMATCH",
            Self::DescriptorMismatch => "DESCRIPTOR_MISMATCH",
            Self::NamespaceMismatch => "NAMESPACE_MISMATCH",
            Self::ConflictingDuplicate => "CONFLICTING_DUPLICATE",
            Self::ProjectionLimitExceeded => "PROJECTION_LIMIT_EXCEEDED",
            Self::StoreQueryFailed => "STORE_QUERY_FAILED",
        };
        f.write_str(code)
    }
}

impl std::error::Error for CoordinateStoreError {}

/// Every Coordinate item currently retained for a community room, discovered by
/// scanning the `coordinate/v1/<descriptor>/items` prefix inside the room's
/// namespace. The bound newswire descriptor is resolved first so the scan is
/// namespace-scoped exactly like the newswire wire scan.
pub fn load_ledger_records(
    store: &EvidenceStore,
    descriptor_id: EntryId,
) -> Result<Vec<VerifiedCoordinateRecord>, CoordinateStoreError> {
    let descriptor = load_space_descriptor(store, descriptor_id)
        .map_err(|_| CoordinateStoreError::DescriptorNotFound)?;
    let namespace_id = descriptor.namespace_id();
    let items = Path::from_slices(&[b"coordinate", b"v1", &descriptor_id, b"items"])
        .map_err(|_| CoordinateStoreError::StoreQueryFailed)?;
    let entries = store
        .entries_with_prefix_in_namespace(&namespace_id, &items)
        .map_err(|_| CoordinateStoreError::StoreQueryFailed)?;
    let records = decode_scanned_coordinate_entries(descriptor_id, entries)?;
    if records
        .iter()
        .any(|record| record.namespace_id() != namespace_id)
    {
        return Err(CoordinateStoreError::NamespaceMismatch);
    }
    Ok(records)
}

fn decode_scanned_coordinate_entries(
    descriptor_id: EntryId,
    entries: Vec<PrefixedEntry>,
) -> Result<Vec<VerifiedCoordinateRecord>, CoordinateStoreError> {
    let mut distinct = BTreeMap::<EntryId, PrefixedEntry>::new();
    for entry in entries {
        if let Some(existing) = distinct.get(&entry.0) {
            if encode_entry(&existing.1) != encode_entry(&entry.1) || existing.2 != entry.2 {
                return Err(CoordinateStoreError::ConflictingDuplicate);
            }
            continue;
        }
        distinct.insert(entry.0, entry);
        if distinct.len() > MAX_LEDGER_RECORDS {
            return Err(CoordinateStoreError::ProjectionLimitExceeded);
        }
    }

    let mut records = Vec::with_capacity(distinct.len());
    for entry in distinct.into_values() {
        let record = decode_scanned_coordinate_entry(entry)?;
        let super::CoordinatePayload::Item(item) = record.payload();
        if item.space_descriptor_entry_id != descriptor_id {
            return Err(CoordinateStoreError::DescriptorMismatch);
        }
        records.push(record);
    }
    records.sort_by_key(|record| (record.tai_j2000_micros(), record.entry_id()));
    Ok(records)
}

fn decode_scanned_coordinate_entry(
    (stored_id, entry, payload): PrefixedEntry,
) -> Result<VerifiedCoordinateRecord, CoordinateStoreError> {
    let payload = payload.ok_or(CoordinateStoreError::MissingRetainedPayload)?;
    let record = inspect_verified_components(&entry, &payload)
        .map_err(|_| CoordinateStoreError::MalformedRetainedRecord)?;
    if record.entry_id() != stored_id {
        return Err(CoordinateStoreError::EntryIdMismatch);
    }
    Ok(record)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coordinate::{
        create_signed_coordinate_item, inspect_coordinate_record, CoordinateItemV1, CoordinateKind,
        CoordinatePayload, SignedCoordinateRecord,
    };
    use crate::newswire::{
        create_signed_space_descriptor, inspect_news_record, SpaceDescriptorV1,
        VerifiedNewswireRecord,
    };
    use crate::willow::{
        decode_entry_canonic, generate_communal_author_for_namespace,
        generate_space_organizer_author, EvidenceAuthor,
    };

    fn tuple(record: &SignedCoordinateRecord) -> PrefixedEntry {
        (
            record.entry_id,
            decode_entry_canonic(&record.signed.entry_bytes).expect("entry"),
            Some(record.signed.payload_bytes.clone()),
        )
    }

    fn room(organizer: &EvidenceAuthor) -> (VerifiedNewswireRecord, EntryId, [u8; 32]) {
        let namespace_id = *organizer.namespace_id().as_bytes();
        let descriptor = create_signed_space_descriptor(
            organizer,
            SpaceDescriptorV1 {
                namespace_id,
                name: "Ledger Room".into(),
                summary: "Coordinate scan fixtures.".into(),
                languages: vec!["en".into()],
                geographic_tags: vec![],
                topic_tags: vec![],
                editorial_roster: vec![],
                predecessor: None,
                successor: None,
            },
        )
        .expect("descriptor");
        let entry_id = descriptor.entry_id;
        let verified = inspect_news_record(&descriptor.signed).expect("verified descriptor");
        (verified, entry_id, namespace_id)
    }

    fn item(space_descriptor_entry_id: EntryId, title: &str) -> CoordinateItemV1 {
        CoordinateItemV1 {
            space_descriptor_entry_id,
            kind: CoordinateKind::Need,
            title: title.into(),
            body: "Human ask.".into(),
            language: "en".into(),
            category_tags: vec![],
            coarse_location: None,
            capacity: None,
            needed_by_unix_seconds: None,
            expires_at_unix_seconds: None,
            contact_instructions: String::new(),
            source_claims: vec![],
            ai_assisted: true,
        }
    }

    fn signed_item(title: &str) -> (SignedCoordinateRecord, EntryId) {
        let organizer = generate_space_organizer_author().expect("organizer");
        let (descriptor, descriptor_id, namespace_id) = room(&organizer);
        let member = generate_communal_author_for_namespace(namespace_id).expect("member");
        let record =
            create_signed_coordinate_item(&member, &descriptor, item(descriptor_id, title))
                .expect("item");
        (record, descriptor_id)
    }

    #[test]
    fn scan_returns_the_admitted_item_pinned_to_its_descriptor() {
        let (record, descriptor_id) = signed_item("Need a ride");
        // The record admits through the canonical gate before it is ever scanned.
        assert!(inspect_coordinate_record(&record.signed).is_ok());

        let decoded = decode_scanned_coordinate_entries(descriptor_id, vec![tuple(&record)])
            .expect("decoded");
        assert_eq!(decoded.len(), 1);
        assert!(matches!(decoded[0].payload(), CoordinatePayload::Item(_)));
        assert_eq!(decoded[0].entry_id(), record.entry_id);
    }

    #[test]
    fn scan_rejects_an_item_pinned_to_a_foreign_descriptor() {
        let (record, _descriptor_id) = signed_item("Need a ride");
        // Scanning under the WRONG descriptor id: the item's own binding does
        // not match, so the scan refuses it rather than silently mixing rooms.
        assert_eq!(
            decode_scanned_coordinate_entries([0x91; 32], vec![tuple(&record)]),
            Err(CoordinateStoreError::DescriptorMismatch)
        );
    }

    #[test]
    fn scan_rejects_missing_payload_and_entry_id_mismatch() {
        let (record, descriptor_id) = signed_item("Sort donations");

        let mut no_payload = tuple(&record);
        no_payload.2 = None;
        assert_eq!(
            decode_scanned_coordinate_entries(descriptor_id, vec![no_payload]),
            Err(CoordinateStoreError::MissingRetainedPayload)
        );

        let mut wrong_id = tuple(&record);
        wrong_id.0 = [0x44; 32];
        assert_eq!(
            decode_scanned_coordinate_entries(descriptor_id, vec![wrong_id]),
            Err(CoordinateStoreError::EntryIdMismatch)
        );

        let mut malformed = tuple(&record);
        malformed.2 = Some(b"not the retained payload".to_vec());
        assert_eq!(
            decode_scanned_coordinate_entries(descriptor_id, vec![malformed]),
            Err(CoordinateStoreError::MalformedRetainedRecord)
        );
    }

    #[test]
    fn scan_rejects_a_conflicting_duplicate_id() {
        let (first, descriptor_id) = signed_item("First");
        let (second, _) = signed_item("Second");
        let mut conflicting = tuple(&second);
        conflicting.0 = first.entry_id;
        assert_eq!(
            decode_scanned_coordinate_entries(descriptor_id, vec![tuple(&first), conflicting]),
            Err(CoordinateStoreError::ConflictingDuplicate)
        );
    }
}
