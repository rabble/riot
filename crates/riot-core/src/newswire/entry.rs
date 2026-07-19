//! Signed Newswire records and hostile structural inspection.

use willow25::prelude::*;

use crate::import::{MAX_CAPABILITY_BYTES, MAX_ENTRY_BYTES};
use crate::willow::{
    authorise_entry, decode_capability_canonic, decode_entry_canonic, encode_capability,
    encode_entry, entry_id, system_snapshot, verify_entry, william3_digest, AuthorisationToken,
    ClockSnapshot, Entry, EntryId, EvidenceAuthor, SignedWillowEntry,
};

use super::path::{classify_newswire_path, newswire_path, NewswirePathKind};
use super::{
    decode_editorial_action, decode_news_comment, decode_news_post, decode_space_descriptor,
    encode_editorial_action, encode_news_comment, encode_news_post, encode_space_descriptor,
    EditorialActionV1, NewsCommentV1, NewsPostV1, NewswireError, SpaceDescriptorV1,
    MAX_NEWSWIRE_PAYLOAD_BYTES,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewswirePayload {
    SpaceDescriptor(SpaceDescriptorV1),
    NewsPost(NewsPostV1),
    EditorialAction(EditorialActionV1),
    NewsComment(NewsCommentV1),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedNewswireRecord {
    pub signed: SignedWillowEntry,
    pub entry_id: EntryId,
    pub snapshot: ClockSnapshot,
    pub payload: NewswirePayload,
}

/// Descriptor-dependent factories accept this inspection output as authority
/// context. Callers can read every fact, but cannot forge the verified wrapper.
///
/// ```compile_fail
/// use riot_core::newswire::VerifiedNewswireRecord;
/// let _forged = VerifiedNewswireRecord {
///     entry_id: [0; 32],
///     namespace_id: [0; 32],
///     signer_id: [0; 32],
///     tai_j2000_micros: 0,
///     payload: todo!(),
/// };
/// ```
///
/// ```compile_fail
/// use riot_core::newswire::{NewswirePayload, VerifiedNewswireRecord};
/// fn rewrite_verified_context(
///     mut verified: VerifiedNewswireRecord,
///     replacement: NewswirePayload,
/// ) {
///     verified.entry_id = [0; 32];
///     verified.namespace_id = [0; 32];
///     verified.signer_id = [0; 32];
///     verified.tai_j2000_micros = 0;
///     verified.payload = replacement;
/// }
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedNewswireRecord {
    entry_id: EntryId,
    namespace_id: [u8; 32],
    signer_id: [u8; 32],
    tai_j2000_micros: u64,
    payload: NewswirePayload,
}

impl VerifiedNewswireRecord {
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

    pub fn payload(&self) -> &NewswirePayload {
        &self.payload
    }

    #[cfg(test)]
    pub(crate) fn new_for_projection_tests(
        entry_id: EntryId,
        namespace_id: [u8; 32],
        signer_id: [u8; 32],
        tai_j2000_micros: u64,
        payload: NewswirePayload,
    ) -> Self {
        Self {
            entry_id,
            namespace_id,
            signer_id,
            tai_j2000_micros,
            payload,
        }
    }
}

fn encode_payload(payload: &NewswirePayload) -> Result<Vec<u8>, NewswireError> {
    match payload {
        NewswirePayload::SpaceDescriptor(value) => encode_space_descriptor(value),
        NewswirePayload::NewsPost(value) => encode_news_post(value),
        NewswirePayload::EditorialAction(value) => encode_editorial_action(value),
        NewswirePayload::NewsComment(value) => encode_news_comment(value),
    }
    .map_err(|_| NewswireError::ModelInvalid)
}

fn payload_path_kind(payload: &NewswirePayload) -> NewswirePathKind {
    match payload {
        NewswirePayload::SpaceDescriptor(_) => NewswirePathKind::Descriptor,
        NewswirePayload::NewsPost(post) => NewswirePathKind::Post {
            space_descriptor_entry_id: post.space_descriptor_entry_id,
        },
        NewswirePayload::EditorialAction(action) => NewswirePathKind::EditorialAction {
            space_descriptor_entry_id: action.space_descriptor_entry_id,
        },
        NewswirePayload::NewsComment(comment) => NewswirePathKind::Comment {
            space_descriptor_entry_id: comment.space_descriptor_entry_id,
        },
    }
}

fn build_signed(
    author: &EvidenceAuthor,
    snapshot: ClockSnapshot,
    payload: NewswirePayload,
) -> Result<SignedNewswireRecord, NewswireError> {
    let payload_bytes = encode_payload(&payload)?;
    let digest = william3_digest(&payload_bytes);
    let path = newswire_path(
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
    let authorised = authorise_entry(author, entry).map_err(|_| NewswireError::SigningFailed)?;
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    let entry_bytes = encode_entry(authorised.entry());
    let entry_id = entry_id(&entry_bytes);
    Ok(SignedNewswireRecord {
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

fn require_founding_organizer(
    author: &EvidenceAuthor,
    descriptor: &SpaceDescriptorV1,
) -> Result<(), NewswireError> {
    if !author.namespace_id().is_communal() {
        return Err(NewswireError::NonCommunalNamespace);
    }
    let namespace_id = author.namespace_id().as_bytes();
    let signer_id = author.subspace_id();
    if namespace_id != signer_id.as_bytes() || descriptor.namespace_id != *namespace_id {
        return Err(NewswireError::AuthorityInvalid);
    }
    Ok(())
}

fn descriptor_context(
    descriptor: &VerifiedNewswireRecord,
) -> Result<&SpaceDescriptorV1, NewswireError> {
    let NewswirePayload::SpaceDescriptor(payload) = descriptor.payload() else {
        return Err(NewswireError::AuthorityInvalid);
    };
    if payload.namespace_id != descriptor.namespace_id()
        || payload.namespace_id != descriptor.signer_id()
    {
        return Err(NewswireError::AuthorityInvalid);
    }
    Ok(payload)
}

fn require_post_authority(
    author: &EvidenceAuthor,
    descriptor: &VerifiedNewswireRecord,
    post: &NewsPostV1,
) -> Result<(), NewswireError> {
    let descriptor_payload = descriptor_context(descriptor)?;
    if post.space_descriptor_entry_id != descriptor.entry_id() {
        return Err(NewswireError::DuplicatedFieldMismatch);
    }
    if author.namespace_id().as_bytes() != &descriptor_payload.namespace_id {
        return Err(NewswireError::AuthorityInvalid);
    }
    Ok(())
}

/// Whether `subject_id` carries editorial authority in `descriptor`. This is
/// the sole admission rule shared by `require_action_authority` (the signing
/// gate) and the FFI display predicate, so the read-path answer can never
/// diverge from what the write-path enforces. There is no founder
/// special-case: a founder absent from the roster is not an editor.
pub fn is_editorial_authority(descriptor: &SpaceDescriptorV1, subject_id: &[u8; 32]) -> bool {
    descriptor.editorial_roster.contains(subject_id)
}

fn require_action_authority(
    author: &EvidenceAuthor,
    descriptor: &VerifiedNewswireRecord,
    action: &EditorialActionV1,
) -> Result<(), NewswireError> {
    let descriptor_payload = descriptor_context(descriptor)?;
    if action.space_descriptor_entry_id != descriptor.entry_id() {
        return Err(NewswireError::DuplicatedFieldMismatch);
    }
    let signer_id = *author.subspace_id().as_bytes();
    if author.namespace_id().as_bytes() != &descriptor_payload.namespace_id
        || !is_editorial_authority(descriptor_payload, &signer_id)
    {
        return Err(NewswireError::AuthorityInvalid);
    }
    Ok(())
}

/// A comment is admitted exactly like a post: the author must be a communal
/// member of the descriptor's namespace, and the comment must name that
/// descriptor. There is NO roster requirement and NO owned capability — a
/// reply is communal. The `parent_entry_id` is unchecked here (the parent may
/// not be held yet); projection resolves it and drops danglers.
fn require_comment_authority(
    author: &EvidenceAuthor,
    descriptor: &VerifiedNewswireRecord,
    comment: &NewsCommentV1,
) -> Result<(), NewswireError> {
    let descriptor_payload = descriptor_context(descriptor)?;
    if comment.space_descriptor_entry_id != descriptor.entry_id() {
        return Err(NewswireError::DuplicatedFieldMismatch);
    }
    if author.namespace_id().as_bytes() != &descriptor_payload.namespace_id {
        return Err(NewswireError::AuthorityInvalid);
    }
    Ok(())
}

pub fn create_signed_space_descriptor(
    author: &EvidenceAuthor,
    descriptor: SpaceDescriptorV1,
) -> Result<SignedNewswireRecord, NewswireError> {
    require_founding_organizer(author, &descriptor)?;
    let snapshot = system_snapshot().map_err(|_| NewswireError::ClockUnavailable)?;
    build_signed(
        author,
        snapshot,
        NewswirePayload::SpaceDescriptor(descriptor),
    )
}

pub fn create_signed_news_post(
    author: &EvidenceAuthor,
    descriptor: &VerifiedNewswireRecord,
    post: NewsPostV1,
) -> Result<SignedNewswireRecord, NewswireError> {
    require_post_authority(author, descriptor, &post)?;
    let snapshot = system_snapshot().map_err(|_| NewswireError::ClockUnavailable)?;
    build_signed(author, snapshot, NewswirePayload::NewsPost(post))
}

pub fn create_signed_editorial_action(
    author: &EvidenceAuthor,
    descriptor: &VerifiedNewswireRecord,
    action: EditorialActionV1,
) -> Result<SignedNewswireRecord, NewswireError> {
    require_action_authority(author, descriptor, &action)?;
    let snapshot = system_snapshot().map_err(|_| NewswireError::ClockUnavailable)?;
    build_signed(author, snapshot, NewswirePayload::EditorialAction(action))
}

pub fn create_signed_news_comment(
    author: &EvidenceAuthor,
    descriptor: &VerifiedNewswireRecord,
    comment: NewsCommentV1,
) -> Result<SignedNewswireRecord, NewswireError> {
    require_comment_authority(author, descriptor, &comment)?;
    let snapshot = system_snapshot().map_err(|_| NewswireError::ClockUnavailable)?;
    build_signed(author, snapshot, NewswirePayload::NewsComment(comment))
}

#[cfg(feature = "conformance")]
pub fn create_signed_space_descriptor_with_clock(
    author: &EvidenceAuthor,
    clock: &dyn crate::willow::ClockSource,
    descriptor: SpaceDescriptorV1,
) -> Result<SignedNewswireRecord, NewswireError> {
    require_founding_organizer(author, &descriptor)?;
    let snapshot = clock
        .snapshot()
        .map_err(|_| NewswireError::ClockUnavailable)?;
    build_signed(
        author,
        snapshot,
        NewswirePayload::SpaceDescriptor(descriptor),
    )
}

#[cfg(feature = "conformance")]
pub fn create_signed_news_post_with_clock(
    author: &EvidenceAuthor,
    descriptor: &VerifiedNewswireRecord,
    clock: &dyn crate::willow::ClockSource,
    post: NewsPostV1,
) -> Result<SignedNewswireRecord, NewswireError> {
    require_post_authority(author, descriptor, &post)?;
    let snapshot = clock
        .snapshot()
        .map_err(|_| NewswireError::ClockUnavailable)?;
    build_signed(author, snapshot, NewswirePayload::NewsPost(post))
}

#[cfg(feature = "conformance")]
pub fn create_signed_editorial_action_with_clock(
    author: &EvidenceAuthor,
    descriptor: &VerifiedNewswireRecord,
    clock: &dyn crate::willow::ClockSource,
    action: EditorialActionV1,
) -> Result<SignedNewswireRecord, NewswireError> {
    require_action_authority(author, descriptor, &action)?;
    let snapshot = clock
        .snapshot()
        .map_err(|_| NewswireError::ClockUnavailable)?;
    build_signed(author, snapshot, NewswirePayload::EditorialAction(action))
}

#[cfg(feature = "conformance")]
pub fn create_signed_news_comment_with_clock(
    author: &EvidenceAuthor,
    descriptor: &VerifiedNewswireRecord,
    clock: &dyn crate::willow::ClockSource,
    comment: NewsCommentV1,
) -> Result<SignedNewswireRecord, NewswireError> {
    require_comment_authority(author, descriptor, &comment)?;
    let snapshot = clock
        .snapshot()
        .map_err(|_| NewswireError::ClockUnavailable)?;
    build_signed(author, snapshot, NewswirePayload::NewsComment(comment))
}

pub fn inspect_news_record(
    signed: &SignedWillowEntry,
) -> Result<VerifiedNewswireRecord, NewswireError> {
    if signed.entry_bytes.len() > MAX_ENTRY_BYTES {
        return Err(NewswireError::EntryBytesExceeded);
    }
    if signed.capability_bytes.len() > MAX_CAPABILITY_BYTES {
        return Err(NewswireError::CapabilityBytesExceeded);
    }
    if signed.payload_bytes.len() > MAX_NEWSWIRE_PAYLOAD_BYTES {
        return Err(NewswireError::PayloadBytesExceeded);
    }

    let entry = decode_entry_canonic(&signed.entry_bytes)
        .map_err(|_| NewswireError::CanonicalEntryInvalid)?;
    let capability = decode_capability_canonic(&signed.capability_bytes)
        .map_err(|_| NewswireError::CanonicalCapabilityInvalid)?;

    if capability.is_owned()
        || !capability.delegations().is_empty()
        || capability.granted_namespace() != entry.namespace_id()
        || capability.receiver() != entry.subspace_id()
        || !capability.includes(&entry)
    {
        return Err(NewswireError::CapabilityInvalid);
    }

    let token = AuthorisationToken::new(capability, signed.signature.into());
    if !verify_entry(&entry, &token) {
        return Err(NewswireError::SignatureInvalid);
    }
    inspect_verified_components(&entry, &signed.payload_bytes)
}

pub(crate) fn inspect_verified_components(
    entry: &Entry,
    payload_bytes: &[u8],
) -> Result<VerifiedNewswireRecord, NewswireError> {
    if payload_bytes.len() > MAX_NEWSWIRE_PAYLOAD_BYTES {
        return Err(NewswireError::PayloadBytesExceeded);
    }
    inspect_verified_components_bounded(entry, payload_bytes)
}

fn inspect_verified_components_bounded(
    entry: &Entry,
    payload_bytes: &[u8],
) -> Result<VerifiedNewswireRecord, NewswireError> {
    if !entry.namespace_id().is_communal() {
        return Err(NewswireError::NonCommunalNamespace);
    }
    if entry.payload_length() != payload_bytes.len() as u64 {
        return Err(NewswireError::PayloadLengthMismatch);
    }
    let digest = william3_digest(payload_bytes);
    if *entry.payload_digest().as_bytes() != digest {
        return Err(NewswireError::PayloadDigestMismatch);
    }
    let Some((kind, path_time, path_digest)) = classify_newswire_path(entry.path()) else {
        return Err(NewswireError::PathInvalid);
    };
    if path_time != u64::from(entry.timestamp()) {
        return Err(NewswireError::PathTimeMismatch);
    }
    if path_digest != digest {
        return Err(NewswireError::PathDigestMismatch);
    }

    let namespace_id = *entry.namespace_id().as_bytes();
    let signer_id = *entry.subspace_id().as_bytes();
    let payload = match kind {
        NewswirePathKind::Descriptor => {
            let descriptor =
                decode_space_descriptor(payload_bytes).map_err(|_| NewswireError::ModelInvalid)?;
            if descriptor.namespace_id != namespace_id || descriptor.namespace_id != signer_id {
                return Err(NewswireError::DuplicatedFieldMismatch);
            }
            NewswirePayload::SpaceDescriptor(descriptor)
        }
        NewswirePathKind::Post {
            space_descriptor_entry_id,
        } => {
            let post = decode_news_post(payload_bytes).map_err(|_| NewswireError::ModelInvalid)?;
            if post.space_descriptor_entry_id != space_descriptor_entry_id {
                return Err(NewswireError::DuplicatedFieldMismatch);
            }
            NewswirePayload::NewsPost(post)
        }
        NewswirePathKind::EditorialAction {
            space_descriptor_entry_id,
        } => {
            let action =
                decode_editorial_action(payload_bytes).map_err(|_| NewswireError::ModelInvalid)?;
            if action.space_descriptor_entry_id != space_descriptor_entry_id {
                return Err(NewswireError::DuplicatedFieldMismatch);
            }
            NewswirePayload::EditorialAction(action)
        }
        NewswirePathKind::Comment {
            space_descriptor_entry_id,
        } => {
            let comment =
                decode_news_comment(payload_bytes).map_err(|_| NewswireError::ModelInvalid)?;
            if comment.space_descriptor_entry_id != space_descriptor_entry_id {
                return Err(NewswireError::DuplicatedFieldMismatch);
            }
            NewswirePayload::NewsComment(comment)
        }
    };

    Ok(VerifiedNewswireRecord {
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
    use crate::willow::{
        generate_communal_author_for_namespace, generate_space_organizer_author, Path,
    };

    fn descriptor(namespace_id: [u8; 32], roster: Vec<[u8; 32]>) -> SpaceDescriptorV1 {
        SpaceDescriptorV1 {
            namespace_id,
            name: "Test Newswire".into(),
            summary: "A local human newswire.".into(),
            languages: vec!["en".into()],
            geographic_tags: vec![],
            topic_tags: vec![],
            editorial_roster: roster,
            predecessor: None,
            successor: None,
        }
    }

    fn snapshot(time: u64) -> ClockSnapshot {
        ClockSnapshot {
            unix_seconds: 1_800_000_000,
            tai_j2000_micros: time,
            uncertainty_seconds: 0,
        }
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

    #[test]
    fn is_editorial_authority_matches_admission() {
        let organizer = generate_space_organizer_author().unwrap();
        let ns = *organizer.namespace_id().as_bytes();
        let editor = generate_communal_author_for_namespace(ns).unwrap();
        let editor_id = *editor.subspace_id().as_bytes();
        let outsider = generate_communal_author_for_namespace(ns).unwrap();
        let outsider_id = *outsider.subspace_id().as_bytes();
        let d = descriptor(ns, vec![editor_id]);
        assert!(is_editorial_authority(&d, &editor_id));
        assert!(!is_editorial_authority(&d, &outsider_id));
        let empty = descriptor(ns, vec![]);
        assert!(
            !is_editorial_authority(&empty, &ns),
            "founder+empty roster is false (matches admission)"
        );
        assert!(!is_editorial_authority(&empty, &outsider_id));
    }

    #[test]
    fn private_builder_and_inspector_cover_all_record_families() {
        let organizer = generate_space_organizer_author().unwrap();
        let namespace_id = *organizer.namespace_id().as_bytes();
        let editor = generate_communal_author_for_namespace(namespace_id).unwrap();
        let descriptor_record = build_signed(
            &organizer,
            snapshot(10),
            NewswirePayload::SpaceDescriptor(descriptor(
                namespace_id,
                vec![*editor.subspace_id().as_bytes()],
            )),
        )
        .unwrap();
        let verified = inspect_news_record(&descriptor_record.signed).unwrap();
        assert_eq!(verified.entry_id(), descriptor_record.entry_id);

        let post = NewsPostV1 {
            space_descriptor_entry_id: verified.entry_id(),
            headline: "Update".into(),
            body: "Human report".into(),
            language: "en".into(),
            event_time_unix_seconds: None,
            expires_at_unix_seconds: None,
            coarse_location: None,
            source_claims: vec![],
            operational_profile: None,
            ai_assisted: false,
        };
        require_post_authority(&editor, &verified, &post).unwrap();
        let post_record =
            build_signed(&editor, snapshot(11), NewswirePayload::NewsPost(post)).unwrap();
        assert!(matches!(
            inspect_news_record(&post_record.signed).unwrap().payload(),
            NewswirePayload::NewsPost(_)
        ));

        let action = EditorialActionV1 {
            space_descriptor_entry_id: verified.entry_id(),
            target_entry_id: post_record.entry_id,
            kind: super::super::EditorialActionKind::Feature,
            reason: None,
            correction_text: None,
        };
        require_action_authority(&editor, &verified, &action).unwrap();
        let action_record = build_signed(
            &editor,
            snapshot(12),
            NewswirePayload::EditorialAction(action),
        )
        .unwrap();
        assert!(matches!(
            inspect_news_record(&action_record.signed)
                .unwrap()
                .payload(),
            NewswirePayload::EditorialAction(_)
        ));
    }

    #[test]
    fn production_factories_sign_all_record_families_without_injectable_inputs() {
        let organizer = generate_space_organizer_author().unwrap();
        let namespace_id = *organizer.namespace_id().as_bytes();
        let editor = generate_communal_author_for_namespace(namespace_id).unwrap();
        let descriptor_record = create_signed_space_descriptor(
            &organizer,
            descriptor(namespace_id, vec![*editor.subspace_id().as_bytes()]),
        )
        .unwrap();
        let verified = inspect_news_record(&descriptor_record.signed).unwrap();
        let post_record = create_signed_news_post(
            &editor,
            &verified,
            NewsPostV1 {
                space_descriptor_entry_id: verified.entry_id(),
                headline: "Update".into(),
                body: "Human report".into(),
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
        create_signed_editorial_action(
            &editor,
            &verified,
            EditorialActionV1 {
                space_descriptor_entry_id: verified.entry_id(),
                target_entry_id: post_record.entry_id,
                kind: super::super::EditorialActionKind::Feature,
                reason: None,
                correction_text: None,
            },
        )
        .unwrap();
    }

    #[test]
    fn authority_checks_reject_wrong_founder_namespace_member_and_roster() {
        let organizer = generate_space_organizer_author().unwrap();
        let other = generate_space_organizer_author().unwrap();
        let wrong_founder_descriptor = descriptor(*other.namespace_id().as_bytes(), vec![]);
        assert_eq!(
            require_founding_organizer(&organizer, &wrong_founder_descriptor),
            Err(NewswireError::AuthorityInvalid)
        );
        assert_eq!(
            create_signed_space_descriptor(&organizer, wrong_founder_descriptor),
            Err(NewswireError::AuthorityInvalid)
        );

        let namespace_id = *organizer.namespace_id().as_bytes();
        let descriptor_record = build_signed(
            &organizer,
            snapshot(20),
            NewswirePayload::SpaceDescriptor(descriptor(namespace_id, vec![])),
        )
        .unwrap();
        let verified = inspect_news_record(&descriptor_record.signed).unwrap();
        let outsider =
            generate_communal_author_for_namespace(*other.namespace_id().as_bytes()).unwrap();
        let post = NewsPostV1 {
            space_descriptor_entry_id: verified.entry_id(),
            headline: "Update".into(),
            body: "Report".into(),
            language: "en".into(),
            event_time_unix_seconds: None,
            expires_at_unix_seconds: None,
            coarse_location: None,
            source_claims: vec![],
            operational_profile: None,
            ai_assisted: false,
        };
        assert_eq!(
            require_post_authority(&outsider, &verified, &post),
            Err(NewswireError::AuthorityInvalid)
        );
        assert_eq!(
            create_signed_news_post(&outsider, &verified, post.clone()),
            Err(NewswireError::AuthorityInvalid)
        );

        let absent_editor = generate_communal_author_for_namespace(namespace_id).unwrap();
        let action = EditorialActionV1 {
            space_descriptor_entry_id: verified.entry_id(),
            target_entry_id: [7; 32],
            kind: super::super::EditorialActionKind::Feature,
            reason: None,
            correction_text: None,
        };
        assert_eq!(
            require_action_authority(&absent_editor, &verified, &action),
            Err(NewswireError::AuthorityInvalid)
        );
        assert_eq!(
            create_signed_editorial_action(&absent_editor, &verified, action),
            Err(NewswireError::AuthorityInvalid)
        );

        let mut wrong_id = post;
        wrong_id.space_descriptor_entry_id = [8; 32];
        assert_eq!(
            require_post_authority(&absent_editor, &verified, &wrong_id),
            Err(NewswireError::DuplicatedFieldMismatch)
        );

        let mut invalid = descriptor(namespace_id, vec![]);
        invalid.name.clear();
        assert_eq!(
            build_signed(
                &organizer,
                snapshot(21),
                NewswirePayload::SpaceDescriptor(invalid.clone())
            ),
            Err(NewswireError::ModelInvalid)
        );
        assert_eq!(
            create_signed_space_descriptor(&organizer, invalid),
            Err(NewswireError::ModelInvalid)
        );
    }

    #[test]
    fn structural_inspection_reports_corrupt_components_and_bindings() {
        let organizer = generate_space_organizer_author().unwrap();
        let namespace_id = *organizer.namespace_id().as_bytes();
        let record = build_signed(
            &organizer,
            snapshot(30),
            NewswirePayload::SpaceDescriptor(descriptor(namespace_id, vec![])),
        )
        .unwrap();

        let mut entry_trailing = record.signed.clone();
        entry_trailing.entry_bytes.push(0);
        assert_eq!(
            inspect_news_record(&entry_trailing),
            Err(NewswireError::CanonicalEntryInvalid)
        );
        let mut cap_trailing = record.signed.clone();
        cap_trailing.capability_bytes.push(0);
        assert_eq!(
            inspect_news_record(&cap_trailing),
            Err(NewswireError::CanonicalCapabilityInvalid)
        );
        let mut bad_signature = record.signed.clone();
        bad_signature.signature[0] ^= 1;
        assert_eq!(
            inspect_news_record(&bad_signature),
            Err(NewswireError::SignatureInvalid)
        );
        let mut bad_payload = record.signed;
        bad_payload.payload_bytes.push(0);
        assert_eq!(
            inspect_news_record(&bad_payload),
            Err(NewswireError::PayloadLengthMismatch)
        );
    }

    #[test]
    fn structural_inspection_rejects_capability_path_payload_and_founder_mismatches() {
        let organizer = generate_space_organizer_author().unwrap();
        let namespace_id = *organizer.namespace_id().as_bytes();
        let payload = descriptor(namespace_id, vec![]);
        let payload_bytes = encode_space_descriptor(&payload).unwrap();
        let digest = william3_digest(&payload_bytes);

        let wrong_time_path = newswire_path(NewswirePathKind::Descriptor, 41, &digest).unwrap();
        assert_eq!(
            inspect_news_record(&sign_raw(
                &organizer,
                wrong_time_path,
                40,
                payload_bytes.clone()
            )),
            Err(NewswireError::PathTimeMismatch)
        );

        let wrong_digest_path = newswire_path(NewswirePathKind::Descriptor, 40, &[9; 32]).unwrap();
        assert_eq!(
            inspect_news_record(&sign_raw(
                &organizer,
                wrong_digest_path,
                40,
                payload_bytes.clone()
            )),
            Err(NewswireError::PathDigestMismatch)
        );

        let malformed_path = Path::from_slices(&[b"newswire", b"v1", b"unknown"]).unwrap();
        assert_eq!(
            inspect_news_record(&sign_raw(
                &organizer,
                malformed_path,
                40,
                payload_bytes.clone()
            )),
            Err(NewswireError::PathInvalid)
        );

        let malformed = vec![0xff];
        let malformed_path = newswire_path(
            NewswirePathKind::Descriptor,
            40,
            &william3_digest(&malformed),
        )
        .unwrap();
        assert_eq!(
            inspect_news_record(&sign_raw(&organizer, malformed_path, 40, malformed)),
            Err(NewswireError::ModelInvalid)
        );

        let member = generate_communal_author_for_namespace(namespace_id).unwrap();
        let founder_path = newswire_path(NewswirePathKind::Descriptor, 40, &digest).unwrap();
        assert_eq!(
            inspect_news_record(&sign_raw(&member, founder_path, 40, payload_bytes.clone())),
            Err(NewswireError::DuplicatedFieldMismatch)
        );

        let post = NewsPostV1 {
            space_descriptor_entry_id: [1; 32],
            headline: "Update".into(),
            body: "Report".into(),
            language: "en".into(),
            event_time_unix_seconds: None,
            expires_at_unix_seconds: None,
            coarse_location: None,
            source_claims: vec![],
            operational_profile: None,
            ai_assisted: false,
        };
        let post_bytes = encode_news_post(&post).unwrap();
        let wrong_descriptor_path = newswire_path(
            NewswirePathKind::Post {
                space_descriptor_entry_id: [2; 32],
            },
            42,
            &william3_digest(&post_bytes),
        )
        .unwrap();
        assert_eq!(
            inspect_news_record(&sign_raw(&member, wrong_descriptor_path, 42, post_bytes)),
            Err(NewswireError::DuplicatedFieldMismatch)
        );

        let good = build_signed(
            &organizer,
            snapshot(43),
            NewswirePayload::SpaceDescriptor(payload),
        )
        .unwrap();
        let other = generate_space_organizer_author().unwrap();
        let other_good = build_signed(
            &other,
            snapshot(43),
            NewswirePayload::SpaceDescriptor(descriptor(*other.namespace_id().as_bytes(), vec![])),
        )
        .unwrap();
        let mut bad_capability = good.signed.clone();
        bad_capability.capability_bytes = other_good.signed.capability_bytes;
        assert_eq!(
            inspect_news_record(&bad_capability),
            Err(NewswireError::CapabilityInvalid)
        );

        let mut same_length_digest_mismatch = good.signed;
        same_length_digest_mismatch.payload_bytes[0] ^= 1;
        assert_eq!(
            inspect_news_record(&same_length_digest_mismatch),
            Err(NewswireError::PayloadDigestMismatch)
        );
    }

    fn comment(space_descriptor_entry_id: EntryId, parent_entry_id: EntryId) -> NewsCommentV1 {
        NewsCommentV1 {
            space_descriptor_entry_id,
            parent_entry_id,
            body: "Communal reply from the crowd.".into(),
            language: "en".into(),
        }
    }

    #[test]
    fn comment_is_admitted_communally_like_a_post_no_roster_required() {
        let organizer = generate_space_organizer_author().unwrap();
        let namespace_id = *organizer.namespace_id().as_bytes();
        // A community member who is NOT in the editorial roster.
        let member = generate_communal_author_for_namespace(namespace_id).unwrap();
        let descriptor_record = build_signed(
            &organizer,
            snapshot(60),
            NewswirePayload::SpaceDescriptor(descriptor(namespace_id, vec![])),
        )
        .unwrap();
        let verified = inspect_news_record(&descriptor_record.signed).unwrap();

        let value = comment(verified.entry_id(), [0x55; 32]);
        require_comment_authority(&member, &verified, &value).unwrap();
        let record = create_signed_news_comment(&member, &verified, value.clone()).unwrap();
        let inspected = inspect_news_record(&record.signed).unwrap();
        assert!(matches!(
            inspected.payload(),
            NewswirePayload::NewsComment(held) if *held == value
        ));
        assert_eq!(inspected.signer_id(), *member.subspace_id().as_bytes());
    }

    #[test]
    fn comment_authority_rejects_foreign_member_and_wrong_descriptor_binding() {
        let organizer = generate_space_organizer_author().unwrap();
        let other = generate_space_organizer_author().unwrap();
        let namespace_id = *organizer.namespace_id().as_bytes();
        let descriptor_record = build_signed(
            &organizer,
            snapshot(61),
            NewswirePayload::SpaceDescriptor(descriptor(namespace_id, vec![])),
        )
        .unwrap();
        let verified = inspect_news_record(&descriptor_record.signed).unwrap();

        let outsider =
            generate_communal_author_for_namespace(*other.namespace_id().as_bytes()).unwrap();
        assert_eq!(
            require_comment_authority(&outsider, &verified, &comment(verified.entry_id(), [1; 32])),
            Err(NewswireError::AuthorityInvalid)
        );
        assert_eq!(
            create_signed_news_comment(&outsider, &verified, comment(verified.entry_id(), [1; 32])),
            Err(NewswireError::AuthorityInvalid)
        );

        let member = generate_communal_author_for_namespace(namespace_id).unwrap();
        assert_eq!(
            require_comment_authority(&member, &verified, &comment([9; 32], [1; 32])),
            Err(NewswireError::DuplicatedFieldMismatch)
        );
    }

    #[test]
    fn comment_whose_path_space_differs_from_payload_is_rejected() {
        let organizer = generate_space_organizer_author().unwrap();
        let member =
            generate_communal_author_for_namespace(*organizer.namespace_id().as_bytes()).unwrap();
        // Payload names space [1;32] but the path is built for space [2;32].
        let value = comment([1; 32], [3; 32]);
        let payload_bytes = encode_news_comment(&value).unwrap();
        let wrong_space_path = newswire_path(
            NewswirePathKind::Comment {
                space_descriptor_entry_id: [2; 32],
            },
            70,
            &william3_digest(&payload_bytes),
        )
        .unwrap();
        assert_eq!(
            inspect_news_record(&sign_raw(&member, wrong_space_path, 70, payload_bytes)),
            Err(NewswireError::DuplicatedFieldMismatch)
        );
    }

    #[test]
    fn comment_carrying_an_owned_or_delegated_capability_is_rejected() {
        use crate::willow::OwnedMasthead;

        // A comment signed by a site owner's OWNED capability — never communal.
        let masthead = OwnedMasthead::generate().unwrap();
        let value = comment([7; 32], [8; 32]);
        let payload_bytes = encode_news_comment(&value).unwrap();
        let digest = william3_digest(&payload_bytes);
        let path = newswire_path(
            NewswirePathKind::Comment {
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
            payload_bytes: payload_bytes.clone(),
        };
        assert_eq!(
            inspect_news_record(&owned_signed),
            Err(NewswireError::CapabilityInvalid)
        );

        // A comment signed under a communal capability that carries a delegation
        // — the closed inspector admits only zero-delegation communal caps.
        let delegator = generate_space_organizer_author().unwrap();
        let namespace_id = *delegator.namespace_id().as_bytes();
        let receiver = generate_communal_author_for_namespace(namespace_id).unwrap();
        let digest2 = william3_digest(&payload_bytes);
        let path2 = newswire_path(
            NewswirePathKind::Comment {
                space_descriptor_entry_id: [7; 32],
            },
            81,
            &digest2,
        )
        .unwrap();
        let mut delegated = delegator.write_capability();
        // A communal cap is scoped to the delegator's own subspace, so the
        // delegated area is that subspace (never the full area).
        delegated
            .try_delegate(
                delegator.subspace_secret(),
                Area::new_subspace_area(delegator.subspace_id()),
                receiver.subspace_id(),
            )
            .unwrap();
        let entry2 = Entry::builder()
            .namespace_id(delegator.namespace_id().clone())
            .subspace_id(delegator.subspace_id())
            .path(path2)
            .timestamp(81)
            .payload(&payload_bytes)
            .build();
        let authorised2 = entry2
            .into_authorised_entry(&delegated, receiver.subspace_secret())
            .unwrap();
        let token2 = authorised2.authorisation_token();
        let signature2: ed25519_dalek::Signature = token2.signature().clone().into();
        let delegated_signed = SignedWillowEntry {
            entry_bytes: encode_entry(authorised2.entry()),
            capability_bytes: encode_capability(token2.capability()),
            signature: signature2.to_bytes(),
            payload_bytes,
        };
        assert_eq!(
            inspect_news_record(&delegated_signed),
            Err(NewswireError::CapabilityInvalid)
        );
    }

    #[test]
    fn internal_component_inspection_rejects_noncommunal_namespace_exactly() {
        let namespace_id = willow25::entry::NamespaceId::from_bytes(&[1; 32]);
        assert!(!namespace_id.is_communal());
        let entry = Entry::builder()
            .namespace_id(namespace_id)
            .subspace_id(willow25::entry::SubspaceId::from([2; 32]))
            .path(Path::from_slices(&[b"newswire"]).unwrap())
            .timestamp(0)
            .payload(b"")
            .build();
        assert_eq!(
            inspect_verified_components(&entry, b""),
            Err(NewswireError::NonCommunalNamespace)
        );
    }
}
