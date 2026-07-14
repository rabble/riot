//! Public conformance proof for descriptor-bound Newswire records.

use riot_core::import::join::{plan_join, JoinState};
use riot_core::import::{MAX_CAPABILITY_BYTES, MAX_ENTRY_BYTES};
use riot_core::newswire::{
    create_signed_editorial_action_with_clock, create_signed_news_post_with_clock,
    create_signed_space_descriptor_with_clock, inspect_news_record, EditorialActionKind,
    EditorialActionV1, NewsPostV1, NewswireError, NewswirePathKind, NewswirePayload,
    SignedNewswireRecord, SpaceDescriptorV1, MAX_NEWSWIRE_PAYLOAD_BYTES,
};
use riot_core::willow::{
    authorise_entry, decode_capability_canonic, decode_entry_canonic, encode_capability,
    encode_entry, entry_id, verify_entry, william3_digest, AuthorisationToken, ClockSnapshot,
    ClockSource, Entry, EntryFacts, EvidenceAuthor, Path, SignedWillowEntry, WillowError,
};
use willow25::authorisation::PossiblyAuthorisedEntry;
use willow25::prelude::*;

#[derive(Clone, Copy)]
struct FixedClock(ClockSnapshot);

impl ClockSource for FixedClock {
    fn snapshot(&self) -> Result<ClockSnapshot, WillowError> {
        Ok(self.0)
    }
}

fn clock(tai_j2000_micros: u64) -> FixedClock {
    FixedClock(ClockSnapshot {
        unix_seconds: 1_800_000_000,
        tai_j2000_micros,
        uncertainty_seconds: 0,
    })
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

fn noncommunal_founder(mut secret: [u8; 32]) -> EvidenceAuthor {
    loop {
        let subspace_secret = willow25::entry::SubspaceSecret::from_bytes(&secret);
        let subspace_id = subspace_secret.corresponding_subspace_id();
        let namespace_id = willow25::entry::NamespaceId::from_bytes(subspace_id.as_bytes());
        if !namespace_id.is_communal() {
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

fn descriptor(namespace_id: [u8; 32], roster: Vec<[u8; 32]>) -> SpaceDescriptorV1 {
    SpaceDescriptorV1 {
        namespace_id,
        name: "Harbor Newswire".into(),
        summary: "Human-published neighborhood reporting.".into(),
        languages: vec!["en".into()],
        geographic_tags: vec!["harbor".into()],
        topic_tags: vec!["local".into()],
        editorial_roster: roster,
        predecessor: None,
        successor: None,
    }
}

fn post(descriptor_id: [u8; 32], body: &str) -> NewsPostV1 {
    NewsPostV1 {
        space_descriptor_entry_id: descriptor_id,
        headline: "Harbor update".into(),
        body: body.into(),
        language: "en".into(),
        event_time_unix_seconds: Some(1_800_000_000),
        expires_at_unix_seconds: None,
        coarse_location: Some("north pier".into()),
        source_claims: vec!["eyewitness".into()],
        operational_profile: None,
        ai_assisted: false,
    }
}

fn action(descriptor_id: [u8; 32], target_entry_id: [u8; 32]) -> EditorialActionV1 {
    EditorialActionV1 {
        space_descriptor_entry_id: descriptor_id,
        target_entry_id,
        kind: EditorialActionKind::Feature,
        reason: None,
        correction_text: None,
    }
}

fn decode_and_verify(record: &SignedNewswireRecord) -> Entry {
    let entry = decode_entry_canonic(&record.signed.entry_bytes).expect("canonical entry");
    let capability =
        decode_capability_canonic(&record.signed.capability_bytes).expect("canonical capability");
    let token = AuthorisationToken::new(capability, record.signed.signature.into());
    assert!(verify_entry(&entry, &token));
    entry
}

fn authorised(record: &SignedNewswireRecord) -> willow25::authorisation::AuthorisedEntry {
    let entry = decode_entry_canonic(&record.signed.entry_bytes).expect("canonical entry");
    let capability =
        decode_capability_canonic(&record.signed.capability_bytes).expect("canonical capability");
    let token = AuthorisationToken::new(capability, record.signed.signature.into());
    PossiblyAuthorisedEntry::new(entry, token)
        .into_authorised_entry()
        .expect("authorised")
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
    let authorised = authorise_entry(author, entry).expect("raw entry authorises");
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
fn exact_paths_bind_time_digest_and_descriptor_identity() {
    let organizer = founder([11; 32]);
    let namespace_id = *organizer.namespace_id().as_bytes();
    let editor = member(namespace_id, [12; 32]);
    let space = descriptor(namespace_id, vec![*editor.subspace_id().as_bytes()]);
    let descriptor_record =
        create_signed_space_descriptor_with_clock(&organizer, &clock(101), space).unwrap();
    let descriptor_entry = decode_and_verify(&descriptor_record);
    let descriptor_digest = william3_digest(&descriptor_record.signed.payload_bytes);
    let expected_descriptor = Path::from_slices(&[
        b"newswire",
        b"v1",
        b"descriptors",
        &101u64.to_be_bytes(),
        &descriptor_digest,
    ])
    .unwrap();
    assert_eq!(descriptor_entry.path(), &expected_descriptor);
    assert_eq!(descriptor_entry.payload_digest_bytes(), descriptor_digest);

    let verified_descriptor = inspect_news_record(&descriptor_record.signed).unwrap();
    assert_eq!(verified_descriptor.entry_id(), descriptor_record.entry_id);
    assert_eq!(verified_descriptor.namespace_id(), namespace_id);
    assert_eq!(
        verified_descriptor.signer_id(),
        *organizer.subspace_id().as_bytes()
    );
    assert_eq!(verified_descriptor.tai_j2000_micros(), 101);
    assert_eq!(verified_descriptor.payload(), &descriptor_record.payload);
    let post_record = create_signed_news_post_with_clock(
        &editor,
        &verified_descriptor,
        &clock(202),
        post(descriptor_record.entry_id, "First report"),
    )
    .unwrap();
    let post_entry = decode_and_verify(&post_record);
    let post_digest = william3_digest(&post_record.signed.payload_bytes);
    let expected_post = Path::from_slices(&[
        b"newswire",
        b"v1",
        &descriptor_record.entry_id,
        b"posts",
        &202u64.to_be_bytes(),
        &post_digest,
    ])
    .unwrap();
    assert_eq!(post_entry.path(), &expected_post);
    assert_eq!(post_entry.payload_digest_bytes(), post_digest);

    let action_record = create_signed_editorial_action_with_clock(
        &editor,
        &verified_descriptor,
        &clock(303),
        action(descriptor_record.entry_id, post_record.entry_id),
    )
    .unwrap();
    let action_entry = decode_and_verify(&action_record);
    let action_digest = william3_digest(&action_record.signed.payload_bytes);
    let expected_action = Path::from_slices(&[
        b"newswire",
        b"v1",
        &descriptor_record.entry_id,
        b"actions",
        &303u64.to_be_bytes(),
        &action_digest,
    ])
    .unwrap();
    assert_eq!(action_entry.path(), &expected_action);
    assert_eq!(action_entry.payload_digest_bytes(), action_digest);

    assert_eq!(
        riot_core::newswire::classify_newswire_path(action_entry.path()),
        Some((
            NewswirePathKind::EditorialAction {
                space_descriptor_entry_id: descriptor_record.entry_id,
            },
            303,
            action_digest,
        ))
    );
}

#[test]
fn deterministic_identity_and_digest_paths_prevent_equal_depth_pruning() {
    let organizer = founder([21; 32]);
    let namespace_id = *organizer.namespace_id().as_bytes();
    let writer = member(namespace_id, [22; 32]);
    let space = descriptor(namespace_id, vec![*writer.subspace_id().as_bytes()]);
    let first =
        create_signed_space_descriptor_with_clock(&organizer, &clock(400), space.clone()).unwrap();
    let repeated =
        create_signed_space_descriptor_with_clock(&organizer, &clock(400), space).unwrap();
    assert_eq!(first.entry_id, repeated.entry_id);

    let verified = inspect_news_record(&first.signed).unwrap();
    let left = create_signed_news_post_with_clock(
        &writer,
        &verified,
        &clock(500),
        post(first.entry_id, "Report A"),
    )
    .unwrap();
    let right = create_signed_news_post_with_clock(
        &writer,
        &verified,
        &clock(500),
        post(first.entry_id, "Report B"),
    )
    .unwrap();
    assert_ne!(
        decode_and_verify(&left).path(),
        decode_and_verify(&right).path()
    );

    let plan = plan_join(
        &JoinState::new(),
        vec![authorised(&left), authorised(&right)],
    )
    .unwrap();
    assert_eq!(plan.next.live_ids().len(), 2);
}

#[test]
fn factories_enforce_founder_namespace_and_fixed_roster_authority() {
    let organizer = founder([31; 32]);
    let namespace_id = *organizer.namespace_id().as_bytes();
    let outsider_founder = founder([32; 32]);
    assert!(create_signed_space_descriptor_with_clock(
        &organizer,
        &clock(600),
        descriptor(*outsider_founder.namespace_id().as_bytes(), vec![]),
    )
    .is_err());

    let editor = member(namespace_id, [33; 32]);
    let outsider = founder([34; 32]);
    let space_record = create_signed_space_descriptor_with_clock(
        &organizer,
        &clock(601),
        descriptor(namespace_id, vec![*editor.subspace_id().as_bytes()]),
    )
    .unwrap();
    let verified = inspect_news_record(&space_record.signed).unwrap();
    assert_eq!(
        create_signed_editorial_action_with_clock(
            &editor,
            &verified,
            &clock(602),
            action([8; 32], [9; 32]),
        ),
        Err(NewswireError::DuplicatedFieldMismatch)
    );
    assert!(create_signed_news_post_with_clock(
        &outsider,
        &verified,
        &clock(602),
        post(space_record.entry_id, "wrong namespace"),
    )
    .is_err());

    let absent_editor = member(namespace_id, [35; 32]);
    assert!(create_signed_editorial_action_with_clock(
        &absent_editor,
        &verified,
        &clock(603),
        action(space_record.entry_id, [9; 32]),
    )
    .is_err());
}

#[test]
fn noncommunal_namespace_never_becomes_verified_newswire_authority() {
    let author = noncommunal_founder([36; 32]);
    let namespace_id = *author.namespace_id().as_bytes();
    let payload = descriptor(namespace_id, vec![]);

    let payload_bytes = riot_core::newswire::encode_space_descriptor(&payload).unwrap();
    let digest = william3_digest(&payload_bytes);
    let path = Path::from_slices(&[
        b"newswire",
        b"v1",
        b"descriptors",
        &604u64.to_be_bytes(),
        &digest,
    ])
    .unwrap();
    let signed = sign_raw(&author, path, 604, payload_bytes);
    assert_eq!(
        inspect_news_record(&signed),
        Err(riot_core::newswire::NewswireError::NonCommunalNamespace)
    );

    assert_eq!(
        create_signed_space_descriptor_with_clock(&author, &clock(604), payload),
        Err(riot_core::newswire::NewswireError::NonCommunalNamespace)
    );
}

#[test]
fn inspection_rejects_every_hostile_envelope_and_binding_mismatch() {
    let organizer = founder([41; 32]);
    let namespace_id = *organizer.namespace_id().as_bytes();
    let descriptor_payload = descriptor(namespace_id, vec![]);
    let good = create_signed_space_descriptor_with_clock(
        &organizer,
        &clock(700),
        descriptor_payload.clone(),
    )
    .unwrap();
    assert!(matches!(
        inspect_news_record(&good.signed).unwrap().payload(),
        NewswirePayload::SpaceDescriptor(_)
    ));

    let mut trailing_entry = good.signed.clone();
    trailing_entry.entry_bytes.push(0);
    assert!(inspect_news_record(&trailing_entry).is_err());
    let mut trailing_capability = good.signed.clone();
    trailing_capability.capability_bytes.push(0);
    assert!(inspect_news_record(&trailing_capability).is_err());
    let mut bad_signature = good.signed.clone();
    bad_signature.signature[0] ^= 0x80;
    assert!(inspect_news_record(&bad_signature).is_err());

    let payload_bytes = good.signed.payload_bytes.clone();
    let digest = william3_digest(&payload_bytes);
    let wrong_time_path = Path::from_slices(&[
        b"newswire",
        b"v1",
        b"descriptors",
        &701u64.to_be_bytes(),
        &digest,
    ])
    .unwrap();
    assert!(inspect_news_record(&sign_raw(
        &organizer,
        wrong_time_path,
        700,
        payload_bytes.clone()
    ))
    .is_err());

    let wrong_digest_path = Path::from_slices(&[
        b"newswire",
        b"v1",
        b"descriptors",
        &700u64.to_be_bytes(),
        &[8; 32],
    ])
    .unwrap();
    assert!(inspect_news_record(&sign_raw(
        &organizer,
        wrong_digest_path,
        700,
        payload_bytes.clone(),
    ))
    .is_err());

    let other = founder([42; 32]);
    let other_digest = william3_digest(&payload_bytes);
    let other_path = Path::from_slices(&[
        b"newswire",
        b"v1",
        b"descriptors",
        &700u64.to_be_bytes(),
        &other_digest,
    ])
    .unwrap();
    assert!(
        inspect_news_record(&sign_raw(&other, other_path, 700, payload_bytes.clone())).is_err()
    );

    let malformed = vec![0xff];
    let malformed_path = Path::from_slices(&[
        b"newswire",
        b"v1",
        b"descriptors",
        &700u64.to_be_bytes(),
        &william3_digest(&malformed),
    ])
    .unwrap();
    assert!(inspect_news_record(&sign_raw(&organizer, malformed_path, 700, malformed)).is_err());

    let verified = inspect_news_record(&good.signed).unwrap();
    let member_author = member(namespace_id, [43; 32]);
    let valid_post = post(good.entry_id, "descriptor binding");
    let valid_post_bytes = riot_core::newswire::encode_news_post(&valid_post).unwrap();
    let wrong_descriptor_path = Path::from_slices(&[
        b"newswire",
        b"v1",
        &[5; 32],
        b"posts",
        &702u64.to_be_bytes(),
        &william3_digest(&valid_post_bytes),
    ])
    .unwrap();
    assert!(inspect_news_record(&sign_raw(
        &member_author,
        wrong_descriptor_path,
        702,
        valid_post_bytes,
    ))
    .is_err());

    let good_post = create_signed_news_post_with_clock(
        &member_author,
        &verified,
        &clock(703),
        post(good.entry_id, "capability binding"),
    )
    .unwrap();
    let mut bad_capability = good_post.signed.clone();
    bad_capability.capability_bytes = good.signed.capability_bytes.clone();
    assert!(inspect_news_record(&bad_capability).is_err());

    let mut length_mismatch = good_post.signed;
    length_mismatch.payload_bytes.push(0);
    assert!(inspect_news_record(&length_mismatch).is_err());

    assert_eq!(entry_id(&good.signed.entry_bytes), good.entry_id);
}

#[test]
fn public_inspection_enforces_component_ceilings_before_decoding_or_hashing() {
    let oversized_entry = SignedWillowEntry {
        entry_bytes: vec![0; MAX_ENTRY_BYTES + 1],
        capability_bytes: vec![],
        signature: [0; 64],
        payload_bytes: vec![],
    };
    assert_eq!(
        inspect_news_record(&oversized_entry),
        Err(NewswireError::EntryBytesExceeded)
    );

    let oversized_capability = SignedWillowEntry {
        entry_bytes: vec![],
        capability_bytes: vec![0; MAX_CAPABILITY_BYTES + 1],
        signature: [0; 64],
        payload_bytes: vec![],
    };
    assert_eq!(
        inspect_news_record(&oversized_capability),
        Err(NewswireError::CapabilityBytesExceeded)
    );

    let oversized_payload = SignedWillowEntry {
        entry_bytes: vec![],
        capability_bytes: vec![],
        signature: [0; 64],
        payload_bytes: vec![0; MAX_NEWSWIRE_PAYLOAD_BYTES + 1],
    };
    assert_eq!(
        inspect_news_record(&oversized_payload),
        Err(NewswireError::PayloadBytesExceeded)
    );

    let organizer = founder([44; 32]);
    let namespace_id = *organizer.namespace_id().as_bytes();
    let good = create_signed_space_descriptor_with_clock(
        &organizer,
        &clock(704),
        descriptor(namespace_id, vec![]),
    )
    .unwrap();

    let mut entry_at_limit = good.signed.clone();
    entry_at_limit.entry_bytes.resize(MAX_ENTRY_BYTES, 0);
    assert_eq!(
        inspect_news_record(&entry_at_limit),
        Err(NewswireError::CanonicalEntryInvalid)
    );

    let mut capability_at_limit = good.signed.clone();
    capability_at_limit
        .capability_bytes
        .resize(MAX_CAPABILITY_BYTES, 0);
    assert_eq!(
        inspect_news_record(&capability_at_limit),
        Err(NewswireError::CanonicalCapabilityInvalid)
    );

    let mut payload_at_limit = good.signed;
    payload_at_limit
        .payload_bytes
        .resize(MAX_NEWSWIRE_PAYLOAD_BYTES, 0);
    assert_eq!(
        inspect_news_record(&payload_at_limit),
        Err(NewswireError::PayloadLengthMismatch)
    );
}
