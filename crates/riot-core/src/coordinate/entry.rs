//! Signed Coordinate records and hostile structural inspection.
//!
//! WU-2 lands the [`CoordinateItemV1`] object-kind family: a signing factory
//! ([`create_signed_coordinate_item`]) that binds an item to a newswire space
//! descriptor exactly as a news post binds, and the canonical communal
//! admission gate ([`inspect_coordinate_record`]) — a **verbatim copy** of
//! `crate::newswire::entry::inspect_news_record` (the closed communal-cap check
//! at `newswire/entry.rs:433-441`), so a Coordinate record is admitted by the
//! same rule as every other communal record and no partial gate is hand-rolled.

use willow25::prelude::*;

use crate::import::{MAX_CAPABILITY_BYTES, MAX_ENTRY_BYTES};
use crate::newswire::{NewswirePayload, SpaceDescriptorV1, VerifiedNewswireRecord};
use crate::willow::{
    authorise_entry, decode_capability_canonic, decode_entry_canonic, encode_capability,
    encode_entry, entry_id, system_snapshot, verify_entry, william3_digest, AuthorisationToken,
    ClockSnapshot, Entry, EntryId, EvidenceAuthor, SignedWillowEntry,
};

use super::path::{classify_coordinate_path, coordinate_path, CoordinatePathKind};
use super::{
    decode_coordinate_item, encode_coordinate_item, CoordinateError, CoordinateItemV1,
    MAX_COORDINATE_PAYLOAD_BYTES,
};

/// The Coordinate object-kind payloads carried under `coordinate/v1/…`. WU-2
/// introduces only the item; status/verification/action variants are added in
/// later work units and will compiler-force new arms in [`encode_payload`],
/// [`payload_path_kind`], and [`inspect_verified_components_bounded`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoordinatePayload {
    Item(CoordinateItemV1),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedCoordinateRecord {
    pub signed: SignedWillowEntry,
    pub entry_id: EntryId,
    pub snapshot: ClockSnapshot,
    pub payload: CoordinatePayload,
}

/// A structurally verified Coordinate record. Callers can read every fact but
/// cannot forge the wrapper — the fields are private and there is no public
/// constructor (mirror of `VerifiedNewswireRecord`).
///
/// ```compile_fail
/// use riot_core::coordinate::VerifiedCoordinateRecord;
/// let _forged = VerifiedCoordinateRecord {
///     entry_id: [0; 32],
///     namespace_id: [0; 32],
///     signer_id: [0; 32],
///     tai_j2000_micros: 0,
///     payload: todo!(),
/// };
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedCoordinateRecord {
    entry_id: EntryId,
    namespace_id: [u8; 32],
    signer_id: [u8; 32],
    tai_j2000_micros: u64,
    payload: CoordinatePayload,
}

impl VerifiedCoordinateRecord {
    pub fn entry_id(&self) -> EntryId {
        self.entry_id
    }

    pub fn namespace_id(&self) -> [u8; 32] {
        self.namespace_id
    }

    pub fn signer_id(&self) -> [u8; 32] {
        self.signer_id
    }

    pub fn tai_j2000_micros(&self) -> u64 {
        self.tai_j2000_micros
    }

    pub fn payload(&self) -> &CoordinatePayload {
        &self.payload
    }
}

fn encode_payload(payload: &CoordinatePayload) -> Result<Vec<u8>, CoordinateError> {
    match payload {
        CoordinatePayload::Item(value) => encode_coordinate_item(value),
    }
    .map_err(|_| CoordinateError::ModelInvalid)
}

fn payload_path_kind(payload: &CoordinatePayload) -> CoordinatePathKind {
    match payload {
        CoordinatePayload::Item(item) => CoordinatePathKind::Item {
            space_descriptor_entry_id: item.space_descriptor_entry_id,
        },
    }
}

fn build_signed(
    author: &EvidenceAuthor,
    snapshot: ClockSnapshot,
    payload: CoordinatePayload,
) -> Result<SignedCoordinateRecord, CoordinateError> {
    let payload_bytes = encode_payload(&payload)?;
    let digest = william3_digest(&payload_bytes);
    let path = coordinate_path(
        payload_path_kind(&payload),
        snapshot.tai_j2000_micros,
        &digest,
    )?;
    let entry = Entry::builder()
        .namespace_id(author.namespace_id().clone())
        .subspace_id(author.subspace_id())
        .path(path)
        .timestamp(snapshot.tai_j2000_micros)
        .payload(&payload_bytes)
        .build();
    let authorised = authorise_entry(author, entry).map_err(|_| CoordinateError::SigningFailed)?;
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    let entry_bytes = encode_entry(authorised.entry());
    let entry_id = entry_id(&entry_bytes);
    Ok(SignedCoordinateRecord {
        signed: SignedWillowEntry {
            entry_bytes,
            capability_bytes: encode_capability(token.capability()),
            signature: signature.to_bytes(),
            payload_bytes,
        },
        entry_id,
        snapshot,
        payload,
    })
}

/// A Coordinate item is admitted exactly like a news post: the author must be a
/// communal member of the descriptor's namespace, and the item must name that
/// descriptor. There is NO roster requirement and NO owned capability — a
/// Coordinate record is communal.
fn descriptor_context(
    descriptor: &VerifiedNewswireRecord,
) -> Result<&SpaceDescriptorV1, CoordinateError> {
    let NewswirePayload::SpaceDescriptor(payload) = descriptor.payload() else {
        return Err(CoordinateError::AuthorityInvalid);
    };
    if payload.namespace_id != descriptor.namespace_id()
        || payload.namespace_id != descriptor.signer_id()
    {
        return Err(CoordinateError::AuthorityInvalid);
    }
    Ok(payload)
}

fn require_item_authority(
    author: &EvidenceAuthor,
    descriptor: &VerifiedNewswireRecord,
    item: &CoordinateItemV1,
) -> Result<(), CoordinateError> {
    let descriptor_payload = descriptor_context(descriptor)?;
    if item.space_descriptor_entry_id != descriptor.entry_id() {
        return Err(CoordinateError::DuplicatedFieldMismatch);
    }
    if author.namespace_id().as_bytes() != &descriptor_payload.namespace_id {
        return Err(CoordinateError::AuthorityInvalid);
    }
    Ok(())
}

pub fn create_signed_coordinate_item(
    author: &EvidenceAuthor,
    descriptor: &VerifiedNewswireRecord,
    item: CoordinateItemV1,
) -> Result<SignedCoordinateRecord, CoordinateError> {
    require_item_authority(author, descriptor, &item)?;
    let snapshot = system_snapshot().map_err(|_| CoordinateError::ClockUnavailable)?;
    build_signed(author, snapshot, CoordinatePayload::Item(item))
}

#[cfg(feature = "conformance")]
pub fn create_signed_coordinate_item_with_clock(
    author: &EvidenceAuthor,
    descriptor: &VerifiedNewswireRecord,
    clock: &dyn crate::willow::ClockSource,
    item: CoordinateItemV1,
) -> Result<SignedCoordinateRecord, CoordinateError> {
    require_item_authority(author, descriptor, &item)?;
    let snapshot = clock
        .snapshot()
        .map_err(|_| CoordinateError::ClockUnavailable)?;
    build_signed(author, snapshot, CoordinatePayload::Item(item))
}

/// The canonical communal admission gate — a verbatim copy of
/// `newswire::entry::inspect_news_record`, including the closed communal-cap
/// check (owned / delegated / wrong-namespace / wrong-receiver / non-including
/// capabilities are all refused). Any change here must track the newswire gate.
pub fn inspect_coordinate_record(
    signed: &SignedWillowEntry,
) -> Result<VerifiedCoordinateRecord, CoordinateError> {
    if signed.entry_bytes.len() > MAX_ENTRY_BYTES {
        return Err(CoordinateError::EntryBytesExceeded);
    }
    if signed.capability_bytes.len() > MAX_CAPABILITY_BYTES {
        return Err(CoordinateError::CapabilityBytesExceeded);
    }
    if signed.payload_bytes.len() > MAX_COORDINATE_PAYLOAD_BYTES {
        return Err(CoordinateError::PayloadBytesExceeded);
    }

    let entry = decode_entry_canonic(&signed.entry_bytes)
        .map_err(|_| CoordinateError::CanonicalEntryInvalid)?;
    let capability = decode_capability_canonic(&signed.capability_bytes)
        .map_err(|_| CoordinateError::CanonicalCapabilityInvalid)?;

    if capability.is_owned()
        || !capability.delegations().is_empty()
        || capability.granted_namespace() != entry.namespace_id()
        || capability.receiver() != entry.subspace_id()
        || !capability.includes(&entry)
    {
        return Err(CoordinateError::CapabilityInvalid);
    }

    let token = AuthorisationToken::new(capability, signed.signature.into());
    if !verify_entry(&entry, &token) {
        return Err(CoordinateError::SignatureInvalid);
    }
    inspect_verified_components(&entry, &signed.payload_bytes)
}

pub(crate) fn inspect_verified_components(
    entry: &Entry,
    payload_bytes: &[u8],
) -> Result<VerifiedCoordinateRecord, CoordinateError> {
    if payload_bytes.len() > MAX_COORDINATE_PAYLOAD_BYTES {
        return Err(CoordinateError::PayloadBytesExceeded);
    }
    inspect_verified_components_bounded(entry, payload_bytes)
}

fn inspect_verified_components_bounded(
    entry: &Entry,
    payload_bytes: &[u8],
) -> Result<VerifiedCoordinateRecord, CoordinateError> {
    if !entry.namespace_id().is_communal() {
        return Err(CoordinateError::NonCommunalNamespace);
    }
    if entry.payload_length() != payload_bytes.len() as u64 {
        return Err(CoordinateError::PayloadLengthMismatch);
    }
    let digest = william3_digest(payload_bytes);
    if *entry.payload_digest().as_bytes() != digest {
        return Err(CoordinateError::PayloadDigestMismatch);
    }
    let Some((kind, path_time, path_digest)) = classify_coordinate_path(entry.path()) else {
        return Err(CoordinateError::PathInvalid);
    };
    if path_time != u64::from(entry.timestamp()) {
        return Err(CoordinateError::PathTimeMismatch);
    }
    if path_digest != digest {
        return Err(CoordinateError::PathDigestMismatch);
    }

    let namespace_id = *entry.namespace_id().as_bytes();
    let signer_id = *entry.subspace_id().as_bytes();
    let payload = match kind {
        CoordinatePathKind::Item {
            space_descriptor_entry_id,
        } => {
            let item =
                decode_coordinate_item(payload_bytes).map_err(|_| CoordinateError::ModelInvalid)?;
            if item.space_descriptor_entry_id != space_descriptor_entry_id {
                return Err(CoordinateError::DuplicatedFieldMismatch);
            }
            CoordinatePayload::Item(item)
        }
    };

    Ok(VerifiedCoordinateRecord {
        entry_id: entry_id(&encode_entry(entry)),
        namespace_id,
        signer_id,
        tai_j2000_micros: u64::from(entry.timestamp()),
        payload,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coordinate::CoordinateKind;
    use crate::newswire::{create_signed_space_descriptor, inspect_news_record};
    use crate::willow::{
        generate_communal_author_for_namespace, generate_space_organizer_author, Path,
    };

    fn descriptor_record(
        organizer: &EvidenceAuthor,
    ) -> (VerifiedNewswireRecord, EntryId, [u8; 32]) {
        let namespace_id = *organizer.namespace_id().as_bytes();
        let record = create_signed_space_descriptor(
            organizer,
            SpaceDescriptorV1 {
                namespace_id,
                name: "Coordinate Room".into(),
                summary: "A community working ledger.".into(),
                languages: vec!["en".into()],
                geographic_tags: vec![],
                topic_tags: vec![],
                editorial_roster: vec![],
                predecessor: None,
                successor: None,
            },
        )
        .expect("descriptor");
        let entry_id = record.entry_id;
        let verified = inspect_news_record(&record.signed).expect("verified descriptor");
        (verified, entry_id, namespace_id)
    }

    fn item(space_descriptor_entry_id: EntryId) -> CoordinateItemV1 {
        CoordinateItemV1 {
            space_descriptor_entry_id,
            kind: CoordinateKind::Task,
            title: "Sort donations at the church hall".into(),
            body: "Two hours, Saturday morning.".into(),
            language: "en".into(),
            category_tags: vec!["help".into()],
            coarse_location: Some("Riverside".into()),
            capacity: Some(4),
            needed_by_unix_seconds: Some(1_800_000_200),
            expires_at_unix_seconds: Some(1_800_100_000),
            contact_instructions: "Ask for Maria".into(),
            source_claims: vec![],
            ai_assisted: false,
        }
    }

    #[test]
    fn valid_item_signs_and_admits_through_the_canonical_gate() {
        let organizer = generate_space_organizer_author().unwrap();
        let (descriptor, descriptor_id, namespace_id) = descriptor_record(&organizer);
        let member = generate_communal_author_for_namespace(namespace_id).unwrap();

        let value = item(descriptor_id);
        let record = create_signed_coordinate_item(&member, &descriptor, value.clone()).unwrap();
        let inspected = inspect_coordinate_record(&record.signed).unwrap();

        assert_eq!(inspected.entry_id(), record.entry_id);
        assert_eq!(inspected.namespace_id(), namespace_id);
        assert_eq!(inspected.signer_id(), *member.subspace_id().as_bytes());
        assert!(matches!(
            inspected.payload(),
            CoordinatePayload::Item(held) if *held == value
        ));
        // The authoritative record time is the TAI/J2000 micros on the entry —
        // NOT the display *_unix_seconds fields.
        assert_eq!(
            inspected.tai_j2000_micros(),
            record.snapshot.tai_j2000_micros
        );
    }

    #[test]
    fn a_tampered_signature_is_refused_by_the_gate() {
        let organizer = generate_space_organizer_author().unwrap();
        let (descriptor, descriptor_id, namespace_id) = descriptor_record(&organizer);
        let member = generate_communal_author_for_namespace(namespace_id).unwrap();
        let record = create_signed_coordinate_item(&member, &descriptor, item(descriptor_id))
            .expect("record");

        let mut tampered = record.signed.clone();
        tampered.signature[0] ^= 1;
        assert_eq!(
            inspect_coordinate_record(&tampered),
            Err(CoordinateError::SignatureInvalid)
        );

        let mut tampered_payload = record.signed;
        tampered_payload.payload_bytes.push(0);
        assert_eq!(
            inspect_coordinate_record(&tampered_payload),
            Err(CoordinateError::PayloadLengthMismatch)
        );
    }

    #[test]
    fn an_owned_capability_is_refused_by_the_closed_communal_gate() {
        use crate::willow::OwnedMasthead;

        let masthead = OwnedMasthead::generate().unwrap();
        let value = item([7; 32]);
        let payload_bytes = encode_coordinate_item(&value).unwrap();
        let digest = william3_digest(&payload_bytes);
        let path = coordinate_path(
            CoordinatePathKind::Item {
                space_descriptor_entry_id: [7; 32],
            },
            80,
            &digest,
        )
        .unwrap();
        let entry = Entry::builder()
            .namespace_id(masthead.namespace_id().clone())
            .subspace_id(masthead.owner_subspace_id())
            .path(path)
            .timestamp(80)
            .payload(&payload_bytes)
            .build();
        let authorised = masthead.authorise_owner_entry(entry).unwrap();
        let token = authorised.authorisation_token();
        let signature: ed25519_dalek::Signature = token.signature().clone().into();
        let owned_signed = SignedWillowEntry {
            entry_bytes: encode_entry(authorised.entry()),
            capability_bytes: encode_capability(token.capability()),
            signature: signature.to_bytes(),
            payload_bytes,
        };
        assert_eq!(
            inspect_coordinate_record(&owned_signed),
            Err(CoordinateError::CapabilityInvalid)
        );
    }

    #[test]
    fn a_delegated_communal_capability_is_refused_by_the_closed_gate() {
        let delegator = generate_space_organizer_author().unwrap();
        let namespace_id = *delegator.namespace_id().as_bytes();
        let receiver = generate_communal_author_for_namespace(namespace_id).unwrap();
        let value = item([7; 32]);
        let payload_bytes = encode_coordinate_item(&value).unwrap();
        let digest = william3_digest(&payload_bytes);
        let path = coordinate_path(
            CoordinatePathKind::Item {
                space_descriptor_entry_id: [7; 32],
            },
            81,
            &digest,
        )
        .unwrap();
        let mut delegated = delegator.write_capability();
        delegated
            .try_delegate(
                delegator.subspace_secret(),
                Area::new_subspace_area(delegator.subspace_id()),
                receiver.subspace_id(),
            )
            .unwrap();
        let entry = Entry::builder()
            .namespace_id(delegator.namespace_id().clone())
            .subspace_id(delegator.subspace_id())
            .path(path)
            .timestamp(81)
            .payload(&payload_bytes)
            .build();
        let authorised = entry
            .into_authorised_entry(&delegated, receiver.subspace_secret())
            .unwrap();
        let token = authorised.authorisation_token();
        let signature: ed25519_dalek::Signature = token.signature().clone().into();
        let delegated_signed = SignedWillowEntry {
            entry_bytes: encode_entry(authorised.entry()),
            capability_bytes: encode_capability(token.capability()),
            signature: signature.to_bytes(),
            payload_bytes,
        };
        assert_eq!(
            inspect_coordinate_record(&delegated_signed),
            Err(CoordinateError::CapabilityInvalid)
        );
    }

    #[test]
    fn authority_rejects_foreign_member_and_wrong_descriptor_binding() {
        let organizer = generate_space_organizer_author().unwrap();
        let other = generate_space_organizer_author().unwrap();
        let (descriptor, descriptor_id, namespace_id) = descriptor_record(&organizer);

        // A member of a DIFFERENT namespace cannot sign into this room.
        let outsider =
            generate_communal_author_for_namespace(*other.namespace_id().as_bytes()).unwrap();
        assert_eq!(
            create_signed_coordinate_item(&outsider, &descriptor, item(descriptor_id)),
            Err(CoordinateError::AuthorityInvalid)
        );

        // A member of the right namespace but an item pinned to the WRONG
        // descriptor id is refused before signing.
        let member = generate_communal_author_for_namespace(namespace_id).unwrap();
        let mut wrong = item(descriptor_id);
        wrong.space_descriptor_entry_id = [9; 32];
        assert_eq!(
            create_signed_coordinate_item(&member, &descriptor, wrong),
            Err(CoordinateError::DuplicatedFieldMismatch)
        );
    }

    #[test]
    fn structural_inspection_reports_corrupt_path_and_binding_components() {
        let organizer = generate_space_organizer_author().unwrap();
        let (descriptor, descriptor_id, namespace_id) = descriptor_record(&organizer);
        let member = generate_communal_author_for_namespace(namespace_id).unwrap();

        // Sign a raw entry whose PATH time disagrees with the entry timestamp.
        let value = item(descriptor_id);
        let payload_bytes = encode_coordinate_item(&value).unwrap();
        let digest = william3_digest(&payload_bytes);
        let wrong_time_path = coordinate_path(
            CoordinatePathKind::Item {
                space_descriptor_entry_id: descriptor_id,
            },
            41,
            &digest,
        )
        .unwrap();
        assert_eq!(
            inspect_coordinate_record(&sign_raw(
                &member,
                wrong_time_path,
                40,
                payload_bytes.clone()
            )),
            Err(CoordinateError::PathTimeMismatch)
        );

        // A path whose embedded space id differs from the payload's binding.
        let wrong_space_path = coordinate_path(
            CoordinatePathKind::Item {
                space_descriptor_entry_id: [2; 32],
            },
            42,
            &digest,
        )
        .unwrap();
        assert_eq!(
            inspect_coordinate_record(&sign_raw(&member, wrong_space_path, 42, payload_bytes)),
            Err(CoordinateError::DuplicatedFieldMismatch)
        );

        // A structurally valid Coordinate path carrying non-canonical bytes.
        let malformed = vec![0xff];
        let malformed_path = coordinate_path(
            CoordinatePathKind::Item {
                space_descriptor_entry_id: descriptor_id,
            },
            43,
            &william3_digest(&malformed),
        )
        .unwrap();
        assert_eq!(
            inspect_coordinate_record(&sign_raw(&member, malformed_path, 43, malformed)),
            Err(CoordinateError::ModelInvalid)
        );
        let _ = &descriptor;
    }

    fn sign_raw(
        author: &EvidenceAuthor,
        path: Path,
        timestamp: u64,
        payload_bytes: Vec<u8>,
    ) -> SignedWillowEntry {
        let entry = Entry::builder()
            .namespace_id(author.namespace_id().clone())
            .subspace_id(author.subspace_id())
            .path(path)
            .timestamp(timestamp)
            .payload(&payload_bytes)
            .build();
        let authorised = authorise_entry(author, entry).unwrap();
        let token = authorised.authorisation_token();
        let signature: ed25519_dalek::Signature = token.signature().clone().into();
        SignedWillowEntry {
            entry_bytes: encode_entry(authorised.entry()),
            capability_bytes: encode_capability(token.capability()),
            signature: signature.to_bytes(),
            payload_bytes,
        }
    }
}
