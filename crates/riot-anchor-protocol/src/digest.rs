//! Protocol identity digests and domain-separation labels.
//!
//! Every digest used as a signature coordinate, continuity link, cursor, receipt
//! field, or replay binding is defined by the design's normative table or an
//! adjacent specialized formula. The general construction is [`digest_v1`]:
//!
//! ```text
//! digest_v1(label, canonical_bytes) =
//!   BLAKE3(
//!     u16be(byte_length(label)) ||
//!     label ||
//!     u64be(byte_length(canonical_bytes)) ||
//!     canonical_bytes
//!   )
//! ```
//!
//! Some preimages deliberately do NOT use the `digest_v1` framing (they prepend a
//! bare label with no length prefixes, or hardcode a label length): those are the
//! `operator_key_id`, `AnchorId`, descriptor/receipt signatures, sync-snapshot,
//! work-proof, peer-proof, and the keyed HMAC inputs (namespace token, cursor,
//! pilot). The distinction is load-bearing — this module keeps the two families
//! visibly separate so no caller can substitute one for the other.

/// Exact ASCII domain-separation labels from the design's digest/preimage tables.
///
/// The first block are `digest_v1` labels; the second block are the bare-label /
/// keyed-preimage labels. Byte strings, never re-encoded.
pub mod label {
    // --- digest_v1 labels (design "Protocol identity digests" table) ---
    /// `descriptor_digest` over a complete `DescriptorEnvelopeV1`.
    pub const DESCRIPTOR_ENVELOPE: &[u8] = b"riot/anchor-descriptor-envelope/v1";
    /// `limit_profile_digest` over a complete canonical `AnchorLimitProfileV1` body.
    pub const LIMIT_PROFILE: &[u8] = b"riot/anchor-limit-profile/v1";
    /// `inclusion_digest` over `OperatorSignedEnvelopeV1<DirectoryInclusionBodyV1>`.
    pub const DIRECTORY_INCLUSION_ENVELOPE: &[u8] = b"riot/directory-inclusion-envelope/v1";
    /// `checkpoint_digest` over `OperatorSignedEnvelopeV1<DirectoryCheckpointBodyV1>`.
    pub const DIRECTORY_CHECKPOINT_ENVELOPE: &[u8] = b"riot/directory-checkpoint-envelope/v1";
    /// `snapshot_record_digest` over `OperatorSignedEnvelopeV1<DirectorySnapshotRecordBodyV1>`.
    pub const DIRECTORY_SNAPSHOT_ENVELOPE: &[u8] = b"riot/directory-snapshot-envelope/v1";
    /// `listing_digest` over an `AdmittedListingEnvelopeV1`.
    pub const ADMITTED_LISTING_ENVELOPE: &[u8] = b"riot/admitted-listing-envelope/v1";
    /// `root_signed_ticket_core_digest` over `RootSignedTicketCoreEnvelopeV2` (excl. hints).
    pub const PUBLIC_SITE_TICKET_SIGNED_CORE: &[u8] = b"riot/public-site-ticket-signed-core/v2";
    /// `work_challenge_digest` over `OperatorSignedEnvelopeV1<WorkChallengeBodyV1>`.
    pub const WORK_CHALLENGE_ENVELOPE: &[u8] = b"riot/anchor-work-challenge-envelope/v1";
    /// `control_request_digest` over `ControlDigestBodyV1` including `work_stamp`.
    pub const CONTROL_REQUEST_BODY: &[u8] = b"riot/anchor-control-request-body/v1";
    /// `work_target_digest` over `ControlDigestBodyV1` with the `work_stamp` slot null.
    pub const WORK_TARGET: &[u8] = b"riot/anchor-work-target/v1";
    /// `page_digest` over a complete canonical `IdsPage` frame.
    pub const SYNC_IDS_PAGE: &[u8] = b"riot/sync-ids-page/v2";
    /// `peer_transcript_digest` over a complete canonical `PeerTranscriptV1` array.
    pub const PEER_TRANSCRIPT: &[u8] = b"riot/anchor-peer-transcript/v1";
    /// `replica_source_attestation_digest` over its operator-signed envelope.
    pub const REPLICA_SOURCE_ATTESTATION_ENVELOPE: &[u8] =
        b"riot/replica-source-attestation-envelope/v1";

    // --- bare-label / specialized preimages (NOT digest_v1 framing) ---
    /// `operator_key_id = BLAKE3(label || canonical_cbor(verification_key))`.
    pub const OPERATOR_KEY_ID: &[u8] = b"riot/anchor-operator-key-id/v1";
    /// `AnchorId = BLAKE3(label || genesis_operator_public_key || genesis_random_256)`.
    pub const ANCHOR_ID: &[u8] = b"riot/anchor-id/v1";
    /// Descriptor current-signature domain: `Sign(label || canonical_cbor(body))`.
    pub const DESCRIPTOR_SIG: &[u8] = b"riot/anchor-descriptor/v1";
    /// Descriptor predecessor-transition domain: `Sign(label || BLAKE3(canonical_cbor(body)))`.
    pub const DESCRIPTOR_TRANSITION_SIG: &[u8] = b"riot/anchor-descriptor-transition/v1";
    /// Hosting-receipt signature domain.
    pub const HOSTING_RECEIPT: &[u8] = b"riot/hosting-receipt/v1";
    /// Listing-receipt signature domain: `Sign(label || canonical_cbor(receipt_body))`.
    pub const LISTING_RECEIPT: &[u8] = b"riot/listing-receipt/v1";
    /// Replica-source-attestation signature domain:
    /// `Sign(label || canonical_cbor(ReplicaSourceAttestationBodyV1))`.
    pub const REPLICA_SOURCE_ATTESTATION_SIG: &[u8] = b"riot/replica-source-attestation/v1";
    /// Sync-snapshot digest domain (per-field length prefixes).
    pub const SYNC_SNAPSHOT: &[u8] = b"riot/sync-snapshot/v2";
    /// Namespace-token HMAC domain (label length hardcoded as `u16be(23)`).
    pub const NAMESPACE_TOKEN: &[u8] = b"riot/namespace-token/v1";
    /// Directory snapshot-cursor HMAC domain (label length hardcoded as `u16be(33)`).
    pub const DIRECTORY_SNAPSHOT_CURSOR: &[u8] = b"riot/directory-snapshot-cursor/v1";
    /// Work-challenge signature domain.
    pub const WORK_CHALLENGE_SIG: &[u8] = b"riot/anchor-work-challenge/v1";
    /// Work-proof BLAKE3 domain.
    pub const WORK_PROOF: &[u8] = b"riot/anchor-work-proof/v1";
    /// Peer-proof signature domain (label length hardcoded as `u16be(25)`).
    pub const PEER_PROOF: &[u8] = b"riot/anchor-peer-proof/v1";

    // Compile-time guards for the hardcoded label lengths the design pins as
    // literals in HMAC/preimage constructions. If a label is ever edited these
    // fail the build rather than silently shifting a preimage.
    const _: () = assert!(NAMESPACE_TOKEN.len() == 23);
    const _: () = assert!(DIRECTORY_SNAPSHOT_CURSOR.len() == 33);
    const _: () = assert!(PEER_PROOF.len() == 25);
}

/// The general protocol identity digest.
///
/// `BLAKE3(u16be(len(label)) || label || u64be(len(canonical)) || canonical)`.
/// The label must be at most `u16::MAX` bytes (all protocol labels are short
/// ASCII constants).
pub fn digest_v1(label: &[u8], canonical: &[u8]) -> [u8; 32] {
    let label_len = u16::try_from(label.len()).expect("digest_v1 label exceeds u16::MAX bytes");
    let mut hasher = blake3::Hasher::new();
    hasher.update(&label_len.to_be_bytes());
    hasher.update(label);
    hasher.update(&(canonical.len() as u64).to_be_bytes());
    hasher.update(canonical);
    *hasher.finalize().as_bytes()
}

/// `operator_key_id = BLAKE3("riot/anchor-operator-key-id/v1" || canonical_cbor(verification_key))`.
/// Bare label: no length prefixes.
pub fn operator_key_id(verification_key_canonical: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(label::OPERATOR_KEY_ID);
    hasher.update(verification_key_canonical);
    *hasher.finalize().as_bytes()
}

/// `AnchorId = BLAKE3("riot/anchor-id/v1" || genesis_operator_public_key || genesis_random_256_bits)`.
pub fn anchor_id(
    genesis_operator_public_key: &[u8; 32],
    genesis_random_256: &[u8; 32],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(label::ANCHOR_ID);
    hasher.update(genesis_operator_public_key);
    hasher.update(genesis_random_256);
    *hasher.finalize().as_bytes()
}

/// `proof = BLAKE3("riot/anchor-work-proof/v1" || work_challenge_digest || u64be(counter))`.
pub fn work_proof(work_challenge_digest: &[u8; 32], counter: u64) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(label::WORK_PROOF);
    hasher.update(work_challenge_digest);
    hasher.update(&counter.to_be_bytes());
    *hasher.finalize().as_bytes()
}

/// The sync-snapshot digest.
///
/// ```text
/// BLAKE3(
///   "riot/sync-snapshot/v2" ||
///   u32be(len(namespace_id)) || namespace_id ||
///   u64be(entry_count) ||
///   u64be(logical_bytes) ||
///   for each sorted entry_id: u32be(len(entry_id)) || entry_id
/// )
/// ```
///
/// `sorted_entry_ids` must already be in the snapshot's canonical order; this
/// function hashes them positionally and does not reorder.
pub fn sync_snapshot_digest(
    namespace_id: &[u8],
    entry_count: u64,
    logical_bytes: u64,
    sorted_entry_ids: &[&[u8]],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(label::SYNC_SNAPSHOT);
    hasher.update(&(namespace_id.len() as u32).to_be_bytes());
    hasher.update(namespace_id);
    hasher.update(&entry_count.to_be_bytes());
    hasher.update(&logical_bytes.to_be_bytes());
    for id in sorted_entry_ids {
        hasher.update(&(id.len() as u32).to_be_bytes());
        hasher.update(id);
    }
    *hasher.finalize().as_bytes()
}

/// The namespace-token HMAC input (the preimage passed to HMAC-SHA256 under the
/// anchor operation secret). This module builds the input bytes only; the keyed
/// MAC lives with the anchor server that holds the secret.
///
/// ```text
/// u16be(23) || "riot/namespace-token/v1" ||
/// u16be(len(operation_id)) || operation_id ||
/// u16be(len(namespace_id)) || namespace_id ||
/// u64be(operation_expiry_unix_seconds) ||
/// u32be(token_secret_epoch)
/// ```
pub fn namespace_token_hmac_input(
    operation_id: &[u8],
    namespace_id: &[u8],
    operation_expiry_unix_seconds: u64,
    token_secret_epoch: u32,
) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&23u16.to_be_bytes());
    out.extend_from_slice(label::NAMESPACE_TOKEN);
    out.extend_from_slice(&(operation_id.len() as u16).to_be_bytes());
    out.extend_from_slice(operation_id);
    out.extend_from_slice(&(namespace_id.len() as u16).to_be_bytes());
    out.extend_from_slice(namespace_id);
    out.extend_from_slice(&operation_expiry_unix_seconds.to_be_bytes());
    out.extend_from_slice(&token_secret_epoch.to_be_bytes());
    out
}

/// The directory snapshot-cursor HMAC input (the preimage passed to HMAC-SHA256
/// under the anchor's cursor secret). This module builds the input bytes only;
/// the keyed MAC lives with the anchor server that holds the secret.
///
/// ```text
/// u16be(33) || "riot/directory-snapshot-cursor/v1" ||
/// u64be(byte_length(canonical_cbor(cursor_body))) ||
/// canonical_cbor(cursor_body)
/// ```
pub fn snapshot_cursor_hmac_input(cursor_body_canonical: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&33u16.to_be_bytes());
    out.extend_from_slice(label::DIRECTORY_SNAPSHOT_CURSOR);
    out.extend_from_slice(&(cursor_body_canonical.len() as u64).to_be_bytes());
    out.extend_from_slice(cursor_body_canonical);
    out
}

/// The role-prefixed peer-proof signature preimage.
///
/// ```text
/// u16be(25) || "riot/anchor-peer-proof/v1" ||
/// u16be(len(role)) || role ||
/// peer_transcript_digest
/// ```
///
/// `role` is the exact lowercase ASCII `initiator` or `responder`.
pub fn peer_proof_signature_preimage(role: &[u8], peer_transcript_digest: &[u8; 32]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&25u16.to_be_bytes());
    out.extend_from_slice(label::PEER_PROOF);
    out.extend_from_slice(&(role.len() as u16).to_be_bytes());
    out.extend_from_slice(role);
    out.extend_from_slice(peer_transcript_digest);
    out
}
