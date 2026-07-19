//! WU-004: the `riot/anchor/1` control plane — the nine control operations, their
//! exact request/success/refusal envelopes, the closed refusal matrix, the
//! control-request / work-target digests, the descriptor-chain verifier, and the
//! directory snapshot cursor.
//!
//! Every layout transcribes the design's "Canonical Anchor Records" +
//! "`riot/anchor/1`: Control Plane" + "Refusals" sections. Optional slots are
//! `null`-or-value; closed enums are `snake_case`; sum variants encode as
//! `[variant_name, ...fields]`; `*_bytes` fields carry separately-canonical bytes.
//!
//! Where the design gives a field *name* but no inner structure — the directory
//! feed inclusions, checkpoint, and snapshot-record payloads, and the
//! `ordered_namespace_host_plan` entries — this module carries the payload as an
//! opaque canonical byte string (its `_bytes`/plan-entry owner is WU-005/006).
//! Those choices are flagged in the doc-comments and the WU-004 report.

use minicbor::{Decoder, Encoder};

use crate::codec::{
    definite_array, expect_array, peek_null, read_bytes_max, read_discriminant, read_fixed_bytes,
    read_null, read_version, CanonicalRecord, CodecError,
};
use crate::digest::{digest_v1, label, snapshot_cursor_hmac_input};
use crate::records::{
    AnchorDescriptorBodyV1, AnchorLimitId, AnchorLimitProfileV1, ControlOperationKind,
    DescriptorEnvelopeV1, DescriptorFloor, HostingReceiptV1, LimitValue, ListingReceiptV1,
    ReplicaPrepareChallengeV1, ReplicaSourceAttestationV1, RootSignedTicketCoreEnvelopeV2,
    WorkChallengeV1, WorkStampV1, ALL_LIMIT_IDS, IDEMPOTENCY_KEY_BYTES,
};

const MAX_TOKEN_BYTES: usize = 48;

/// A control frame is at most 64 KiB (design "The control ALPN carries canonical
/// CBOR frames no larger than 64 KiB").
pub const MAX_CONTROL_FRAME_BYTES: usize = 64 * 1024;

// ---------------------------------------------------------------------------
// Shared token-enum machinery for the closed nested vocabularies.
// ---------------------------------------------------------------------------

macro_rules! token_enum {
    (
        $(#[$outer:meta])*
        $name:ident { $($variant:ident => $tok:literal),+ $(,)? }
    ) => {
        $(#[$outer])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $name {
            $(
                #[doc = $tok]
                $variant,
            )+
        }

        impl $name {
            /// The exact `snake_case` wire token.
            pub fn token(self) -> &'static str {
                match self { $($name::$variant => $tok,)+ }
            }
            /// Parse a wire token, or `None` for anything unrecognized.
            pub fn from_token(token: &str) -> Option<Self> {
                match token { $($tok => Some($name::$variant),)+ _ => None }
            }
            /// Encode this token as a `snake_case` text string.
            pub fn encode(self, e: &mut Encoder<&mut Vec<u8>>) -> Result<(), CodecError> {
                e.str(self.token()).map_err(|_| CodecError::Malformed)?;
                Ok(())
            }
            /// Decode a `snake_case` token, rejecting anything unrecognized.
            pub fn decode(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
                let token = read_discriminant(d, MAX_TOKEN_BYTES)?;
                $name::from_token(&token).ok_or(CodecError::UnknownVariant)
            }
        }
    };
}

token_enum! {
    /// The refusal subject (design "Refusals").
    RefusalSubject {
        Ticket => "ticket",
        Manifest => "manifest",
        Listing => "listing",
        Namespace => "namespace",
        Capacity => "capacity",
        Version => "version",
        Transport => "transport",
        Operation => "operation",
        Work => "work",
        Peer => "peer",
    }
}

token_enum! {
    /// The normative client retry action derived from a refusal row (design
    /// "Retry scope"). Not a wire field — derived from the code.
    RetryScope {
        Never => "never",
        SameRequestAfterDelay => "same_request_after_delay",
        SameOperationNewCommitKey => "same_operation_new_commit_key",
        HostThenNewIdempotencyKey => "host_then_new_idempotency_key",
        NewOperation => "new_operation",
        SameIdempotencyWithNewWorkStamp => "same_idempotency_with_new_work_stamp",
        NewIdempotencyKeyAfterDelay => "new_idempotency_key_after_delay",
        NewCheckpoint => "new_checkpoint",
        RefreshDescriptor => "refresh_descriptor",
    }
}

token_enum! {
    /// Transport mode nested enum (design "transport mode is `require_none |
    /// require_arti | unsupported_other`").
    TransportMode {
        RequireNone => "require_none",
        RequireArti => "require_arti",
        UnsupportedOther => "unsupported_other",
    }
}

token_enum! {
    /// `site_too_large` storage class.
    StorageClass {
        ProfileTotal => "profile_total",
        SiteLogicalBytes => "site_logical_bytes",
        EntriesPerNamespace => "entries_per_namespace",
        ItemPayload => "item_payload",
        Bundle => "bundle",
    }
}

token_enum! {
    /// Peer side.
    PeerSide {
        Source => "source",
        Destination => "destination",
    }
}

token_enum! {
    /// `peer_context_changed` reason.
    PeerContextReason {
        DescriptorRotation => "descriptor_rotation",
        ConfigurationRotation => "configuration_rotation",
        TransportLoss => "transport_loss",
        OrderlyShutdown => "orderly_shutdown",
        ProcessRestart => "process_restart",
    }
}

token_enum! {
    /// `checkpoint_unavailable` reason.
    CheckpointReason {
        Unknown => "unknown",
        Reclaimed => "reclaimed",
        SnapshotMissing => "snapshot_missing",
        DigestMismatch => "digest_mismatch",
    }
}

token_enum! {
    /// Cursor kind.
    CursorKind {
        Feed => "feed",
        Snapshot => "snapshot",
    }
}

token_enum! {
    /// `cursor_invalid` reason.
    CursorReason {
        Malformed => "malformed",
        AfterHead => "after_head",
        WrongCheckpoint => "wrong_checkpoint",
        WrongGeneration => "wrong_generation",
        Regressed => "regressed",
        Expired => "expired",
    }
}

token_enum! {
    /// `peer_auth_failed` stage.
    PeerAuthStage {
        DescriptorExchange => "descriptor_exchange",
        HelloValidation => "hello_validation",
        ChannelBinding => "channel_binding",
        InitiatorProof => "initiator_proof",
        ResponderProof => "responder_proof",
        ConfiguredRule => "configured_rule",
    }
}

// ---------------------------------------------------------------------------
// Small codec helpers.
// ---------------------------------------------------------------------------

fn expect_token(d: &mut Decoder<'_>, expected: &str) -> Result<(), CodecError> {
    let token = read_discriminant(d, MAX_TOKEN_BYTES)?;
    if token == expected {
        Ok(())
    } else {
        // A code paired with the wrong subject / detail tag is a closed-matrix
        // violation: the design says unknown code/detail pairings fail decoding.
        Err(CodecError::UnknownVariant)
    }
}

fn expect_bool(d: &mut Decoder<'_>, expected: bool) -> Result<(), CodecError> {
    let value = d.bool().map_err(|_| CodecError::Malformed)?;
    if value == expected {
        Ok(())
    } else {
        Err(CodecError::NonCanonical)
    }
}

/// Read a `retry_after_seconds` slot that MUST be `null` for this code.
fn read_retry_null(d: &mut Decoder<'_>) -> Result<(), CodecError> {
    if peek_null(d)? {
        read_null(d)
    } else {
        Err(CodecError::NonCanonical)
    }
}

/// Read a `retry_after_seconds` slot that MUST be a nonzero `uint` for this code.
fn read_retry_required(d: &mut Decoder<'_>) -> Result<u64, CodecError> {
    if peek_null(d)? {
        return Err(CodecError::NonCanonical);
    }
    let value = d.u64().map_err(|_| CodecError::Malformed)?;
    if value == 0 {
        return Err(CodecError::NonCanonical);
    }
    Ok(value)
}

fn read_digest(d: &mut Decoder<'_>) -> Result<[u8; 32], CodecError> {
    read_fixed_bytes::<32>(d)
}

fn read_opt_digest(d: &mut Decoder<'_>) -> Result<Option<[u8; 32]>, CodecError> {
    if peek_null(d)? {
        read_null(d)?;
        Ok(None)
    } else {
        Ok(Some(read_fixed_bytes::<32>(d)?))
    }
}

fn read_opt_u64(d: &mut Decoder<'_>) -> Result<Option<u64>, CodecError> {
    if peek_null(d)? {
        read_null(d)?;
        Ok(None)
    } else {
        Ok(Some(d.u64().map_err(|_| CodecError::Malformed)?))
    }
}

fn encode_limit_id(e: &mut Encoder<&mut Vec<u8>>, id: AnchorLimitId) -> Result<(), CodecError> {
    e.u64(id.id()).map_err(|_| CodecError::Malformed)?;
    Ok(())
}

fn decode_limit_id(d: &mut Decoder<'_>) -> Result<AnchorLimitId, CodecError> {
    let raw = d.u64().map_err(|_| CodecError::Malformed)?;
    AnchorLimitId::from_id(raw).ok_or(CodecError::UnknownVariant)
}

fn encode_opt_digest(
    e: &mut Encoder<&mut Vec<u8>>,
    value: &Option<[u8; 32]>,
) -> Result<(), CodecError> {
    match value {
        Some(d) => e.bytes(d).map_err(|_| CodecError::Malformed)?,
        None => e.null().map_err(|_| CodecError::Malformed)?,
    };
    Ok(())
}

fn encode_opt_u64(e: &mut Encoder<&mut Vec<u8>>, value: &Option<u64>) -> Result<(), CodecError> {
    match value {
        Some(v) => e.u64(*v).map_err(|_| CodecError::Malformed)?,
        None => e.null().map_err(|_| CodecError::Malformed)?,
    };
    Ok(())
}

fn read_triple_digests(d: &mut Decoder<'_>) -> Result<[[u8; 32]; 3], CodecError> {
    expect_array(d, 3)?;
    let mut out = [[0u8; 32]; 3];
    for slot in out.iter_mut() {
        *slot = read_fixed_bytes::<32>(d)?;
    }
    Ok(out)
}

fn encode_triple_digests(
    e: &mut Encoder<&mut Vec<u8>>,
    digests: &[[u8; 32]; 3],
) -> Result<(), CodecError> {
    e.array(3).map_err(|_| CodecError::Malformed)?;
    for digest in digests {
        e.bytes(digest).map_err(|_| CodecError::Malformed)?;
    }
    Ok(())
}

// ===========================================================================
// ControlRefusal — the closed refusal matrix.
//
// Wire: [code, subject, retryable, retry_after_seconds|null, details]. The
// subject, retryability, retry-after nullness, retry scope, and details shape are
// all a closed function of `code`; decode rejects any cross-pairing.
// ===========================================================================

/// A closed control refusal. Each variant is exactly one design matrix row; the
/// subject, retryability, and retry scope are derived from the variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlRefusal {
    /// `invalid_ticket_authority`.
    InvalidTicketAuthority,
    /// `invalid_manifest_authority`.
    InvalidManifestAuthority,
    /// `invalid_listing_authority`.
    InvalidListingAuthority,
    /// `invalid_operation_authority`.
    InvalidOperationAuthority,
    /// `unsupported_version` — `["versions", supported_versions]`.
    UnsupportedVersion {
        /// The supported control/sync versions (sorted ascending set).
        supported_versions: Vec<u64>,
    },
    /// `admission_over_quota` — `["quota", limit_id, effective, observed]`.
    AdmissionOverQuota {
        /// The limit that was exceeded.
        limit_id: AnchorLimitId,
        /// The effective ceiling.
        effective_value: LimitValue,
        /// The observed value.
        observed_value: LimitValue,
        /// Required nonzero retry delay.
        retry_after_seconds: u64,
    },
    /// `commit_over_quota` — `["quota", limit_id, effective, observed]`.
    CommitOverQuota {
        /// The limit that was exceeded.
        limit_id: AnchorLimitId,
        /// The effective ceiling.
        effective_value: LimitValue,
        /// The observed value.
        observed_value: LimitValue,
        /// Required nonzero retry delay.
        retry_after_seconds: u64,
    },
    /// `unsupported_transport` — `["transport", required_mode, observed_mode]`.
    UnsupportedTransport {
        /// The mode the site requires.
        required_mode: TransportMode,
        /// The mode the anchor observed.
        observed_mode: TransportMode,
    },
    /// `manifest_transport_mismatch` — `["digests", expected, observed]`.
    ManifestTransportMismatch {
        /// Expected digest.
        expected_digest: [u8; 32],
        /// Observed digest.
        observed_digest: [u8; 32],
    },
    /// `not_hosted`.
    NotHosted,
    /// `listing_manifest_mismatch` — `["digests", expected, observed]`.
    ListingManifestMismatch {
        /// Expected digest.
        expected_digest: [u8; 32],
        /// Observed digest.
        observed_digest: [u8; 32],
    },
    /// `commit_manifest_mismatch` — `["digests", expected, observed]`.
    CommitManifestMismatch {
        /// Expected digest.
        expected_digest: [u8; 32],
        /// Observed digest.
        observed_digest: [u8; 32],
    },
    /// `snapshot_mismatch` — `["snapshot", expected, observed]`.
    SnapshotMismatch {
        /// Expected snapshot digest.
        expected_snapshot_digest: [u8; 32],
        /// Observed snapshot digest.
        observed_snapshot_digest: [u8; 32],
    },
    /// `ticket_expired` — `["expiry", expires_at, observed_at]`.
    TicketExpired {
        /// Ticket expiry (Unix seconds).
        expires_at: u64,
        /// Observation time (Unix seconds).
        observed_at: u64,
    },
    /// `listing_expired` — `["expiry", expires_at, observed_at]`.
    ListingExpired {
        /// Listing expiry (Unix seconds).
        expires_at: u64,
        /// Observation time (Unix seconds).
        observed_at: u64,
    },
    /// `work_expired` — `["expiry", expires_at, observed_at]`.
    WorkExpired {
        /// Challenge expiry (Unix seconds).
        expires_at: u64,
        /// Observation time (Unix seconds).
        observed_at: u64,
    },
    /// `listing_equivocation` — `["equivocation", first, second]`.
    ListingEquivocation {
        /// First conflicting digest.
        first_digest: [u8; 32],
        /// Second conflicting digest.
        second_digest: [u8; 32],
    },
    /// `manifest_equivocation` — `["equivocation", first, second]`.
    ManifestEquivocation {
        /// First conflicting digest.
        first_digest: [u8; 32],
        /// Second conflicting digest.
        second_digest: [u8; 32],
    },
    /// `anchor_profile_oversize` — `["encoded_size", observed, maximum]`.
    AnchorProfileOversize {
        /// Observed encoded bytes.
        observed_bytes: u64,
        /// Maximum encoded bytes.
        maximum_bytes: u64,
    },
    /// `site_too_large` — `["storage", required_class, advertised, local_limit]`.
    SiteTooLarge {
        /// The storage class exceeded.
        required_class: StorageClass,
        /// Advertised byte requirement.
        advertised_bytes: u64,
        /// Local limit bytes.
        local_limit_bytes: u64,
    },
    /// `work_required` — `["work", policy_epoch, difficulty]`.
    WorkRequired {
        /// The pressure-policy epoch.
        policy_epoch: u64,
        /// The required difficulty (`0..=24`).
        difficulty: u64,
    },
    /// `stale_base` — `["site_state", current_generation, digests]`.
    StaleBase {
        /// Current site generation.
        current_generation: u64,
        /// Ordered `O`, `C`, `W` snapshot digests.
        ordered_namespace_snapshot_digests: [[u8; 32]; 3],
    },
    /// `stale_source` — `["source_state", attested, observed, digests]`.
    StaleSource {
        /// Attested source generation.
        attested_generation: u64,
        /// Observed source generation.
        observed_generation: u64,
        /// Ordered observed `O`, `C`, `W` snapshot digests.
        ordered_observed_namespace_snapshot_digests: [[u8; 32]; 3],
    },
    /// `attestation_consumed` — `["attestation", digest]`.
    AttestationConsumed {
        /// The consumed attestation digest.
        replica_source_attestation_digest: [u8; 32],
    },
    /// `already_unlisted` — `["listing_state", "already_unlisted"]`.
    AlreadyUnlisted,
    /// `removal_replay_window` — `["relist_window", earliest_retry_at]`.
    RemovalReplayWindow {
        /// The earliest retry time (Unix seconds).
        earliest_retry_at: u64,
        /// Required nonzero retry delay.
        retry_after_seconds: u64,
    },
    /// `idempotency_conflict`.
    IdempotencyConflict,
    /// `operation_not_found` — `["operation", operation_id]`.
    OperationNotFound {
        /// The unknown operation id.
        operation_id: [u8; 32],
    },
    /// `operation_expired` — `["operation_expiry", operation_id, expires_at]`.
    OperationExpired {
        /// The expired operation id.
        operation_id: [u8; 32],
        /// The operation expiry (Unix seconds).
        expires_at: u64,
    },
    /// `checkpoint_unavailable` — `["checkpoint", digest, reason]`.
    CheckpointUnavailable {
        /// The unavailable checkpoint digest.
        checkpoint_digest: [u8; 32],
        /// The unavailability reason.
        reason: CheckpointReason,
    },
    /// `cursor_invalid` — `["cursor", kind, reason, opt_digest, opt_floor, opt_head]`.
    CursorInvalid {
        /// Feed or snapshot cursor.
        cursor_kind: CursorKind,
        /// The invalidity reason.
        reason: CursorReason,
        /// Optional checkpoint digest.
        checkpoint_digest: Option<[u8; 32]>,
        /// Optional floor sequence.
        floor_sequence: Option<u64>,
        /// Optional head sequence.
        head_sequence: Option<u64>,
    },
    /// `peer_context_changed` — `["peer_context", side, prior, opt_latest, reason]`.
    PeerContextChanged {
        /// The side that changed.
        side: PeerSide,
        /// The prior descriptor digest.
        prior_descriptor_digest: [u8; 32],
        /// The optional latest known descriptor digest.
        latest_descriptor_digest: Option<[u8; 32]>,
        /// The context-change reason.
        reason: PeerContextReason,
    },
    /// `admission_busy` — `["capacity", limit_id]`.
    AdmissionBusy {
        /// The saturated limit.
        limit_id: AnchorLimitId,
        /// Required nonzero retry delay.
        retry_after_seconds: u64,
    },
    /// `removal_busy` — `["capacity", limit_id]`.
    RemovalBusy {
        /// The saturated limit.
        limit_id: AnchorLimitId,
        /// Required nonzero retry delay.
        retry_after_seconds: u64,
    },
    /// `commit_busy` — `["capacity", limit_id]`.
    CommitBusy {
        /// The saturated limit.
        limit_id: AnchorLimitId,
        /// Required nonzero retry delay.
        retry_after_seconds: u64,
    },
    /// `peer_auth_failed` — `["peer_auth", stage]`.
    PeerAuthFailed {
        /// The handshake stage that failed.
        stage: PeerAuthStage,
    },
}

impl ControlRefusal {
    /// The refusal's closed `code` token.
    pub fn code(&self) -> &'static str {
        match self {
            ControlRefusal::InvalidTicketAuthority => "invalid_ticket_authority",
            ControlRefusal::InvalidManifestAuthority => "invalid_manifest_authority",
            ControlRefusal::InvalidListingAuthority => "invalid_listing_authority",
            ControlRefusal::InvalidOperationAuthority => "invalid_operation_authority",
            ControlRefusal::UnsupportedVersion { .. } => "unsupported_version",
            ControlRefusal::AdmissionOverQuota { .. } => "admission_over_quota",
            ControlRefusal::CommitOverQuota { .. } => "commit_over_quota",
            ControlRefusal::UnsupportedTransport { .. } => "unsupported_transport",
            ControlRefusal::ManifestTransportMismatch { .. } => "manifest_transport_mismatch",
            ControlRefusal::NotHosted => "not_hosted",
            ControlRefusal::ListingManifestMismatch { .. } => "listing_manifest_mismatch",
            ControlRefusal::CommitManifestMismatch { .. } => "commit_manifest_mismatch",
            ControlRefusal::SnapshotMismatch { .. } => "snapshot_mismatch",
            ControlRefusal::TicketExpired { .. } => "ticket_expired",
            ControlRefusal::ListingExpired { .. } => "listing_expired",
            ControlRefusal::WorkExpired { .. } => "work_expired",
            ControlRefusal::ListingEquivocation { .. } => "listing_equivocation",
            ControlRefusal::ManifestEquivocation { .. } => "manifest_equivocation",
            ControlRefusal::AnchorProfileOversize { .. } => "anchor_profile_oversize",
            ControlRefusal::SiteTooLarge { .. } => "site_too_large",
            ControlRefusal::WorkRequired { .. } => "work_required",
            ControlRefusal::StaleBase { .. } => "stale_base",
            ControlRefusal::StaleSource { .. } => "stale_source",
            ControlRefusal::AttestationConsumed { .. } => "attestation_consumed",
            ControlRefusal::AlreadyUnlisted => "already_unlisted",
            ControlRefusal::RemovalReplayWindow { .. } => "removal_replay_window",
            ControlRefusal::IdempotencyConflict => "idempotency_conflict",
            ControlRefusal::OperationNotFound { .. } => "operation_not_found",
            ControlRefusal::OperationExpired { .. } => "operation_expired",
            ControlRefusal::CheckpointUnavailable { .. } => "checkpoint_unavailable",
            ControlRefusal::CursorInvalid { .. } => "cursor_invalid",
            ControlRefusal::PeerContextChanged { .. } => "peer_context_changed",
            ControlRefusal::AdmissionBusy { .. } => "admission_busy",
            ControlRefusal::RemovalBusy { .. } => "removal_busy",
            ControlRefusal::CommitBusy { .. } => "commit_busy",
            ControlRefusal::PeerAuthFailed { .. } => "peer_auth_failed",
        }
    }

    /// The refusal's derived `subject`.
    pub fn subject(&self) -> RefusalSubject {
        match self {
            ControlRefusal::InvalidTicketAuthority | ControlRefusal::TicketExpired { .. } => {
                RefusalSubject::Ticket
            }
            ControlRefusal::InvalidManifestAuthority
            | ControlRefusal::ManifestTransportMismatch { .. }
            | ControlRefusal::ManifestEquivocation { .. } => RefusalSubject::Manifest,
            ControlRefusal::InvalidListingAuthority
            | ControlRefusal::NotHosted
            | ControlRefusal::ListingManifestMismatch { .. }
            | ControlRefusal::ListingExpired { .. }
            | ControlRefusal::ListingEquivocation { .. }
            | ControlRefusal::AlreadyUnlisted
            | ControlRefusal::RemovalReplayWindow { .. } => RefusalSubject::Listing,
            ControlRefusal::InvalidOperationAuthority
            | ControlRefusal::CommitManifestMismatch { .. }
            | ControlRefusal::StaleBase { .. }
            | ControlRefusal::StaleSource { .. }
            | ControlRefusal::AttestationConsumed { .. }
            | ControlRefusal::IdempotencyConflict
            | ControlRefusal::OperationNotFound { .. }
            | ControlRefusal::OperationExpired { .. }
            | ControlRefusal::CheckpointUnavailable { .. }
            | ControlRefusal::CursorInvalid { .. } => RefusalSubject::Operation,
            ControlRefusal::UnsupportedVersion { .. } => RefusalSubject::Version,
            ControlRefusal::UnsupportedTransport { .. } => RefusalSubject::Transport,
            ControlRefusal::SnapshotMismatch { .. } => RefusalSubject::Namespace,
            ControlRefusal::WorkExpired { .. } | ControlRefusal::WorkRequired { .. } => {
                RefusalSubject::Work
            }
            ControlRefusal::AdmissionOverQuota { .. }
            | ControlRefusal::CommitOverQuota { .. }
            | ControlRefusal::AnchorProfileOversize { .. }
            | ControlRefusal::SiteTooLarge { .. }
            | ControlRefusal::AdmissionBusy { .. }
            | ControlRefusal::RemovalBusy { .. }
            | ControlRefusal::CommitBusy { .. } => RefusalSubject::Capacity,
            ControlRefusal::PeerContextChanged { .. } | ControlRefusal::PeerAuthFailed { .. } => {
                RefusalSubject::Peer
            }
        }
    }

    /// The refusal's derived `retryable` flag.
    pub fn retryable(&self) -> bool {
        !matches!(self.retry_scope(), RetryScope::Never)
    }

    /// The refusal's `retry_after_seconds`, if the row requires a nonzero delay.
    pub fn retry_after_seconds(&self) -> Option<u64> {
        match self {
            ControlRefusal::AdmissionOverQuota {
                retry_after_seconds,
                ..
            }
            | ControlRefusal::CommitOverQuota {
                retry_after_seconds,
                ..
            }
            | ControlRefusal::RemovalReplayWindow {
                retry_after_seconds,
                ..
            }
            | ControlRefusal::AdmissionBusy {
                retry_after_seconds,
                ..
            }
            | ControlRefusal::RemovalBusy {
                retry_after_seconds,
                ..
            }
            | ControlRefusal::CommitBusy {
                retry_after_seconds,
                ..
            } => Some(*retry_after_seconds),
            _ => None,
        }
    }

    /// The refusal's derived `retry_scope` (normative client action).
    pub fn retry_scope(&self) -> RetryScope {
        match self {
            ControlRefusal::InvalidTicketAuthority
            | ControlRefusal::InvalidManifestAuthority
            | ControlRefusal::InvalidListingAuthority
            | ControlRefusal::InvalidOperationAuthority
            | ControlRefusal::UnsupportedVersion { .. }
            | ControlRefusal::UnsupportedTransport { .. }
            | ControlRefusal::ManifestTransportMismatch { .. }
            | ControlRefusal::TicketExpired { .. }
            | ControlRefusal::ListingExpired { .. }
            | ControlRefusal::ListingEquivocation { .. }
            | ControlRefusal::ManifestEquivocation { .. }
            | ControlRefusal::AnchorProfileOversize { .. }
            | ControlRefusal::SiteTooLarge { .. }
            | ControlRefusal::AlreadyUnlisted
            | ControlRefusal::IdempotencyConflict
            | ControlRefusal::OperationNotFound { .. } => RetryScope::Never,
            ControlRefusal::AdmissionOverQuota { .. }
            | ControlRefusal::AdmissionBusy { .. }
            | ControlRefusal::RemovalBusy { .. } => RetryScope::SameRequestAfterDelay,
            ControlRefusal::CommitOverQuota { .. } | ControlRefusal::CommitBusy { .. } => {
                RetryScope::SameOperationNewCommitKey
            }
            ControlRefusal::NotHosted | ControlRefusal::ListingManifestMismatch { .. } => {
                RetryScope::HostThenNewIdempotencyKey
            }
            ControlRefusal::CommitManifestMismatch { .. }
            | ControlRefusal::SnapshotMismatch { .. }
            | ControlRefusal::StaleBase { .. }
            | ControlRefusal::StaleSource { .. }
            | ControlRefusal::AttestationConsumed { .. }
            | ControlRefusal::OperationExpired { .. }
            | ControlRefusal::PeerContextChanged { .. } => RetryScope::NewOperation,
            ControlRefusal::WorkExpired { .. } | ControlRefusal::WorkRequired { .. } => {
                RetryScope::SameIdempotencyWithNewWorkStamp
            }
            ControlRefusal::RemovalReplayWindow { .. } => RetryScope::NewIdempotencyKeyAfterDelay,
            ControlRefusal::CheckpointUnavailable { .. } | ControlRefusal::CursorInvalid { .. } => {
                RetryScope::NewCheckpoint
            }
            ControlRefusal::PeerAuthFailed { .. } => RetryScope::RefreshDescriptor,
        }
    }

    fn encode_details(&self, e: &mut Encoder<&mut Vec<u8>>) -> Result<(), CodecError> {
        match self {
            ControlRefusal::InvalidTicketAuthority
            | ControlRefusal::InvalidManifestAuthority
            | ControlRefusal::InvalidListingAuthority
            | ControlRefusal::InvalidOperationAuthority
            | ControlRefusal::NotHosted
            | ControlRefusal::IdempotencyConflict => {
                e.array(1).map_err(|_| CodecError::Malformed)?;
                e.str("none").map_err(|_| CodecError::Malformed)?;
            }
            ControlRefusal::UnsupportedVersion { supported_versions } => {
                e.array(2).map_err(|_| CodecError::Malformed)?;
                e.str("versions").map_err(|_| CodecError::Malformed)?;
                e.array(supported_versions.len() as u64)
                    .map_err(|_| CodecError::Malformed)?;
                let mut previous: Option<u64> = None;
                for v in supported_versions {
                    if let Some(p) = previous {
                        if *v <= p {
                            return Err(CodecError::UnsortedSet);
                        }
                    }
                    previous = Some(*v);
                    e.u64(*v).map_err(|_| CodecError::Malformed)?;
                }
            }
            ControlRefusal::AdmissionOverQuota {
                limit_id,
                effective_value,
                observed_value,
                ..
            }
            | ControlRefusal::CommitOverQuota {
                limit_id,
                effective_value,
                observed_value,
                ..
            } => {
                e.array(4).map_err(|_| CodecError::Malformed)?;
                e.str("quota").map_err(|_| CodecError::Malformed)?;
                encode_limit_id(e, *limit_id)?;
                effective_value.encode(e)?;
                observed_value.encode(e)?;
            }
            ControlRefusal::UnsupportedTransport {
                required_mode,
                observed_mode,
            } => {
                e.array(3).map_err(|_| CodecError::Malformed)?;
                e.str("transport").map_err(|_| CodecError::Malformed)?;
                required_mode.encode(e)?;
                observed_mode.encode(e)?;
            }
            ControlRefusal::ManifestTransportMismatch {
                expected_digest,
                observed_digest,
            }
            | ControlRefusal::ListingManifestMismatch {
                expected_digest,
                observed_digest,
            }
            | ControlRefusal::CommitManifestMismatch {
                expected_digest,
                observed_digest,
            } => {
                e.array(3).map_err(|_| CodecError::Malformed)?;
                e.str("digests").map_err(|_| CodecError::Malformed)?;
                e.bytes(expected_digest)
                    .map_err(|_| CodecError::Malformed)?;
                e.bytes(observed_digest)
                    .map_err(|_| CodecError::Malformed)?;
            }
            ControlRefusal::SnapshotMismatch {
                expected_snapshot_digest,
                observed_snapshot_digest,
            } => {
                e.array(3).map_err(|_| CodecError::Malformed)?;
                e.str("snapshot").map_err(|_| CodecError::Malformed)?;
                e.bytes(expected_snapshot_digest)
                    .map_err(|_| CodecError::Malformed)?;
                e.bytes(observed_snapshot_digest)
                    .map_err(|_| CodecError::Malformed)?;
            }
            ControlRefusal::TicketExpired {
                expires_at,
                observed_at,
            }
            | ControlRefusal::ListingExpired {
                expires_at,
                observed_at,
            }
            | ControlRefusal::WorkExpired {
                expires_at,
                observed_at,
            } => {
                e.array(3).map_err(|_| CodecError::Malformed)?;
                e.str("expiry").map_err(|_| CodecError::Malformed)?;
                e.u64(*expires_at).map_err(|_| CodecError::Malformed)?;
                e.u64(*observed_at).map_err(|_| CodecError::Malformed)?;
            }
            ControlRefusal::ListingEquivocation {
                first_digest,
                second_digest,
            }
            | ControlRefusal::ManifestEquivocation {
                first_digest,
                second_digest,
            } => {
                e.array(3).map_err(|_| CodecError::Malformed)?;
                e.str("equivocation").map_err(|_| CodecError::Malformed)?;
                e.bytes(first_digest).map_err(|_| CodecError::Malformed)?;
                e.bytes(second_digest).map_err(|_| CodecError::Malformed)?;
            }
            ControlRefusal::AnchorProfileOversize {
                observed_bytes,
                maximum_bytes,
            } => {
                e.array(3).map_err(|_| CodecError::Malformed)?;
                e.str("encoded_size").map_err(|_| CodecError::Malformed)?;
                e.u64(*observed_bytes).map_err(|_| CodecError::Malformed)?;
                e.u64(*maximum_bytes).map_err(|_| CodecError::Malformed)?;
            }
            ControlRefusal::SiteTooLarge {
                required_class,
                advertised_bytes,
                local_limit_bytes,
            } => {
                e.array(4).map_err(|_| CodecError::Malformed)?;
                e.str("storage").map_err(|_| CodecError::Malformed)?;
                required_class.encode(e)?;
                e.u64(*advertised_bytes)
                    .map_err(|_| CodecError::Malformed)?;
                e.u64(*local_limit_bytes)
                    .map_err(|_| CodecError::Malformed)?;
            }
            ControlRefusal::WorkRequired {
                policy_epoch,
                difficulty,
            } => {
                e.array(3).map_err(|_| CodecError::Malformed)?;
                e.str("work").map_err(|_| CodecError::Malformed)?;
                e.u64(*policy_epoch).map_err(|_| CodecError::Malformed)?;
                e.u64(*difficulty).map_err(|_| CodecError::Malformed)?;
            }
            ControlRefusal::StaleBase {
                current_generation,
                ordered_namespace_snapshot_digests,
            } => {
                e.array(3).map_err(|_| CodecError::Malformed)?;
                e.str("site_state").map_err(|_| CodecError::Malformed)?;
                e.u64(*current_generation)
                    .map_err(|_| CodecError::Malformed)?;
                encode_triple_digests(e, ordered_namespace_snapshot_digests)?;
            }
            ControlRefusal::StaleSource {
                attested_generation,
                observed_generation,
                ordered_observed_namespace_snapshot_digests,
            } => {
                e.array(4).map_err(|_| CodecError::Malformed)?;
                e.str("source_state").map_err(|_| CodecError::Malformed)?;
                e.u64(*attested_generation)
                    .map_err(|_| CodecError::Malformed)?;
                e.u64(*observed_generation)
                    .map_err(|_| CodecError::Malformed)?;
                encode_triple_digests(e, ordered_observed_namespace_snapshot_digests)?;
            }
            ControlRefusal::AttestationConsumed {
                replica_source_attestation_digest,
            } => {
                e.array(2).map_err(|_| CodecError::Malformed)?;
                e.str("attestation").map_err(|_| CodecError::Malformed)?;
                e.bytes(replica_source_attestation_digest)
                    .map_err(|_| CodecError::Malformed)?;
            }
            ControlRefusal::AlreadyUnlisted => {
                e.array(2).map_err(|_| CodecError::Malformed)?;
                e.str("listing_state").map_err(|_| CodecError::Malformed)?;
                e.str("already_unlisted")
                    .map_err(|_| CodecError::Malformed)?;
            }
            ControlRefusal::RemovalReplayWindow {
                earliest_retry_at, ..
            } => {
                e.array(2).map_err(|_| CodecError::Malformed)?;
                e.str("relist_window").map_err(|_| CodecError::Malformed)?;
                e.u64(*earliest_retry_at)
                    .map_err(|_| CodecError::Malformed)?;
            }
            ControlRefusal::OperationNotFound { operation_id } => {
                e.array(2).map_err(|_| CodecError::Malformed)?;
                e.str("operation").map_err(|_| CodecError::Malformed)?;
                e.bytes(operation_id).map_err(|_| CodecError::Malformed)?;
            }
            ControlRefusal::OperationExpired {
                operation_id,
                expires_at,
            } => {
                e.array(3).map_err(|_| CodecError::Malformed)?;
                e.str("operation_expiry")
                    .map_err(|_| CodecError::Malformed)?;
                e.bytes(operation_id).map_err(|_| CodecError::Malformed)?;
                e.u64(*expires_at).map_err(|_| CodecError::Malformed)?;
            }
            ControlRefusal::CheckpointUnavailable {
                checkpoint_digest,
                reason,
            } => {
                e.array(3).map_err(|_| CodecError::Malformed)?;
                e.str("checkpoint").map_err(|_| CodecError::Malformed)?;
                e.bytes(checkpoint_digest)
                    .map_err(|_| CodecError::Malformed)?;
                reason.encode(e)?;
            }
            ControlRefusal::CursorInvalid {
                cursor_kind,
                reason,
                checkpoint_digest,
                floor_sequence,
                head_sequence,
            } => {
                e.array(6).map_err(|_| CodecError::Malformed)?;
                e.str("cursor").map_err(|_| CodecError::Malformed)?;
                cursor_kind.encode(e)?;
                reason.encode(e)?;
                encode_opt_digest(e, checkpoint_digest)?;
                encode_opt_u64(e, floor_sequence)?;
                encode_opt_u64(e, head_sequence)?;
            }
            ControlRefusal::PeerContextChanged {
                side,
                prior_descriptor_digest,
                latest_descriptor_digest,
                reason,
            } => {
                e.array(5).map_err(|_| CodecError::Malformed)?;
                e.str("peer_context").map_err(|_| CodecError::Malformed)?;
                side.encode(e)?;
                e.bytes(prior_descriptor_digest)
                    .map_err(|_| CodecError::Malformed)?;
                encode_opt_digest(e, latest_descriptor_digest)?;
                reason.encode(e)?;
            }
            ControlRefusal::AdmissionBusy { limit_id, .. }
            | ControlRefusal::RemovalBusy { limit_id, .. }
            | ControlRefusal::CommitBusy { limit_id, .. } => {
                e.array(2).map_err(|_| CodecError::Malformed)?;
                e.str("capacity").map_err(|_| CodecError::Malformed)?;
                encode_limit_id(e, *limit_id)?;
            }
            ControlRefusal::PeerAuthFailed { stage } => {
                e.array(2).map_err(|_| CodecError::Malformed)?;
                e.str("peer_auth").map_err(|_| CodecError::Malformed)?;
                stage.encode(e)?;
            }
        }
        Ok(())
    }
}

impl CanonicalRecord for ControlRefusal {
    fn encode_canonical(&self) -> Result<Vec<u8>, CodecError> {
        let mut buf = Vec::new();
        {
            let mut e = Encoder::new(&mut buf);
            e.array(5).map_err(|_| CodecError::Malformed)?;
            e.str(self.code()).map_err(|_| CodecError::Malformed)?;
            e.str(self.subject().token())
                .map_err(|_| CodecError::Malformed)?;
            e.bool(self.retryable())
                .map_err(|_| CodecError::Malformed)?;
            match self.retry_after_seconds() {
                Some(secs) => {
                    if secs == 0 {
                        return Err(CodecError::NonCanonical);
                    }
                    e.u64(secs).map_err(|_| CodecError::Malformed)?;
                }
                None => {
                    e.null().map_err(|_| CodecError::Malformed)?;
                }
            }
            self.encode_details(&mut e)?;
        }
        Ok(buf)
    }

    fn decode_fields(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        expect_array(d, 5)?;
        let code = read_discriminant(d, MAX_TOKEN_BYTES)?;
        // Each arm fixes subject, retryable, retry-after nullness, and the exact
        // details shape; any cross-pairing fails decode (closed matrix).
        let refusal = match code.as_str() {
            "invalid_ticket_authority" => {
                decode_none_row(d, "ticket", false)?;
                ControlRefusal::InvalidTicketAuthority
            }
            "invalid_manifest_authority" => {
                decode_none_row(d, "manifest", false)?;
                ControlRefusal::InvalidManifestAuthority
            }
            "invalid_listing_authority" => {
                decode_none_row(d, "listing", false)?;
                ControlRefusal::InvalidListingAuthority
            }
            "invalid_operation_authority" => {
                decode_none_row(d, "operation", false)?;
                ControlRefusal::InvalidOperationAuthority
            }
            "not_hosted" => {
                decode_none_row(d, "listing", true)?;
                ControlRefusal::NotHosted
            }
            "idempotency_conflict" => {
                decode_none_row(d, "operation", false)?;
                ControlRefusal::IdempotencyConflict
            }
            "unsupported_version" => {
                expect_token(d, "version")?;
                expect_bool(d, false)?;
                read_retry_null(d)?;
                expect_array(d, 2)?;
                expect_token(d, "versions")?;
                let count = definite_array(d)?;
                if count as usize > 16 {
                    return Err(CodecError::LengthOutOfRange);
                }
                let mut supported_versions = Vec::with_capacity(count as usize);
                let mut previous: Option<u64> = None;
                for _ in 0..count {
                    let v = d.u64().map_err(|_| CodecError::Malformed)?;
                    if let Some(p) = previous {
                        if v <= p {
                            return Err(CodecError::UnsortedSet);
                        }
                    }
                    previous = Some(v);
                    supported_versions.push(v);
                }
                ControlRefusal::UnsupportedVersion { supported_versions }
            }
            "admission_over_quota" | "commit_over_quota" => {
                expect_token(d, "capacity")?;
                expect_bool(d, true)?;
                let retry_after_seconds = read_retry_required(d)?;
                expect_array(d, 4)?;
                expect_token(d, "quota")?;
                let limit_id = decode_limit_id(d)?;
                let effective_value = LimitValue::decode(d)?;
                let observed_value = LimitValue::decode(d)?;
                if code == "admission_over_quota" {
                    ControlRefusal::AdmissionOverQuota {
                        limit_id,
                        effective_value,
                        observed_value,
                        retry_after_seconds,
                    }
                } else {
                    ControlRefusal::CommitOverQuota {
                        limit_id,
                        effective_value,
                        observed_value,
                        retry_after_seconds,
                    }
                }
            }
            "unsupported_transport" => {
                expect_token(d, "transport")?;
                expect_bool(d, false)?;
                read_retry_null(d)?;
                expect_array(d, 3)?;
                expect_token(d, "transport")?;
                let required_mode = TransportMode::decode(d)?;
                let observed_mode = TransportMode::decode(d)?;
                ControlRefusal::UnsupportedTransport {
                    required_mode,
                    observed_mode,
                }
            }
            "manifest_transport_mismatch" => {
                let (expected_digest, observed_digest) = decode_digests_row(d, "manifest", false)?;
                ControlRefusal::ManifestTransportMismatch {
                    expected_digest,
                    observed_digest,
                }
            }
            "listing_manifest_mismatch" => {
                let (expected_digest, observed_digest) = decode_digests_row(d, "listing", true)?;
                ControlRefusal::ListingManifestMismatch {
                    expected_digest,
                    observed_digest,
                }
            }
            "commit_manifest_mismatch" => {
                let (expected_digest, observed_digest) = decode_digests_row(d, "operation", true)?;
                ControlRefusal::CommitManifestMismatch {
                    expected_digest,
                    observed_digest,
                }
            }
            "snapshot_mismatch" => {
                expect_token(d, "namespace")?;
                expect_bool(d, true)?;
                read_retry_null(d)?;
                expect_array(d, 3)?;
                expect_token(d, "snapshot")?;
                let expected_snapshot_digest = read_digest(d)?;
                let observed_snapshot_digest = read_digest(d)?;
                ControlRefusal::SnapshotMismatch {
                    expected_snapshot_digest,
                    observed_snapshot_digest,
                }
            }
            "ticket_expired" => {
                let (expires_at, observed_at) = decode_expiry_row(d, "ticket", false)?;
                ControlRefusal::TicketExpired {
                    expires_at,
                    observed_at,
                }
            }
            "listing_expired" => {
                let (expires_at, observed_at) = decode_expiry_row(d, "listing", false)?;
                ControlRefusal::ListingExpired {
                    expires_at,
                    observed_at,
                }
            }
            "work_expired" => {
                let (expires_at, observed_at) = decode_expiry_row(d, "work", true)?;
                ControlRefusal::WorkExpired {
                    expires_at,
                    observed_at,
                }
            }
            "listing_equivocation" => {
                let (first_digest, second_digest) = decode_equivocation_row(d, "listing")?;
                ControlRefusal::ListingEquivocation {
                    first_digest,
                    second_digest,
                }
            }
            "manifest_equivocation" => {
                let (first_digest, second_digest) = decode_equivocation_row(d, "manifest")?;
                ControlRefusal::ManifestEquivocation {
                    first_digest,
                    second_digest,
                }
            }
            "anchor_profile_oversize" => {
                expect_token(d, "capacity")?;
                expect_bool(d, false)?;
                read_retry_null(d)?;
                expect_array(d, 3)?;
                expect_token(d, "encoded_size")?;
                let observed_bytes = d.u64().map_err(|_| CodecError::Malformed)?;
                let maximum_bytes = d.u64().map_err(|_| CodecError::Malformed)?;
                ControlRefusal::AnchorProfileOversize {
                    observed_bytes,
                    maximum_bytes,
                }
            }
            "site_too_large" => {
                expect_token(d, "capacity")?;
                expect_bool(d, false)?;
                read_retry_null(d)?;
                expect_array(d, 4)?;
                expect_token(d, "storage")?;
                let required_class = StorageClass::decode(d)?;
                let advertised_bytes = d.u64().map_err(|_| CodecError::Malformed)?;
                let local_limit_bytes = d.u64().map_err(|_| CodecError::Malformed)?;
                ControlRefusal::SiteTooLarge {
                    required_class,
                    advertised_bytes,
                    local_limit_bytes,
                }
            }
            "work_required" => {
                expect_token(d, "work")?;
                expect_bool(d, true)?;
                read_retry_null(d)?;
                expect_array(d, 3)?;
                expect_token(d, "work")?;
                let policy_epoch = d.u64().map_err(|_| CodecError::Malformed)?;
                let difficulty = d.u64().map_err(|_| CodecError::Malformed)?;
                ControlRefusal::WorkRequired {
                    policy_epoch,
                    difficulty,
                }
            }
            "stale_base" => {
                expect_token(d, "operation")?;
                expect_bool(d, true)?;
                read_retry_null(d)?;
                expect_array(d, 3)?;
                expect_token(d, "site_state")?;
                let current_generation = d.u64().map_err(|_| CodecError::Malformed)?;
                let ordered_namespace_snapshot_digests = read_triple_digests(d)?;
                ControlRefusal::StaleBase {
                    current_generation,
                    ordered_namespace_snapshot_digests,
                }
            }
            "stale_source" => {
                expect_token(d, "operation")?;
                expect_bool(d, true)?;
                read_retry_null(d)?;
                expect_array(d, 4)?;
                expect_token(d, "source_state")?;
                let attested_generation = d.u64().map_err(|_| CodecError::Malformed)?;
                let observed_generation = d.u64().map_err(|_| CodecError::Malformed)?;
                let ordered_observed_namespace_snapshot_digests = read_triple_digests(d)?;
                ControlRefusal::StaleSource {
                    attested_generation,
                    observed_generation,
                    ordered_observed_namespace_snapshot_digests,
                }
            }
            "attestation_consumed" => {
                expect_token(d, "operation")?;
                expect_bool(d, true)?;
                read_retry_null(d)?;
                expect_array(d, 2)?;
                expect_token(d, "attestation")?;
                let replica_source_attestation_digest = read_digest(d)?;
                ControlRefusal::AttestationConsumed {
                    replica_source_attestation_digest,
                }
            }
            "already_unlisted" => {
                expect_token(d, "listing")?;
                expect_bool(d, false)?;
                read_retry_null(d)?;
                expect_array(d, 2)?;
                expect_token(d, "listing_state")?;
                expect_token(d, "already_unlisted")?;
                ControlRefusal::AlreadyUnlisted
            }
            "removal_replay_window" => {
                expect_token(d, "listing")?;
                expect_bool(d, true)?;
                let retry_after_seconds = read_retry_required(d)?;
                expect_array(d, 2)?;
                expect_token(d, "relist_window")?;
                let earliest_retry_at = d.u64().map_err(|_| CodecError::Malformed)?;
                ControlRefusal::RemovalReplayWindow {
                    earliest_retry_at,
                    retry_after_seconds,
                }
            }
            "operation_not_found" => {
                expect_token(d, "operation")?;
                expect_bool(d, false)?;
                read_retry_null(d)?;
                expect_array(d, 2)?;
                expect_token(d, "operation")?;
                let operation_id = read_digest(d)?;
                ControlRefusal::OperationNotFound { operation_id }
            }
            "operation_expired" => {
                expect_token(d, "operation")?;
                expect_bool(d, true)?;
                read_retry_null(d)?;
                expect_array(d, 3)?;
                expect_token(d, "operation_expiry")?;
                let operation_id = read_digest(d)?;
                let expires_at = d.u64().map_err(|_| CodecError::Malformed)?;
                ControlRefusal::OperationExpired {
                    operation_id,
                    expires_at,
                }
            }
            "checkpoint_unavailable" => {
                expect_token(d, "operation")?;
                expect_bool(d, true)?;
                read_retry_null(d)?;
                expect_array(d, 3)?;
                expect_token(d, "checkpoint")?;
                let checkpoint_digest = read_digest(d)?;
                let reason = CheckpointReason::decode(d)?;
                ControlRefusal::CheckpointUnavailable {
                    checkpoint_digest,
                    reason,
                }
            }
            "cursor_invalid" => {
                expect_token(d, "operation")?;
                expect_bool(d, true)?;
                read_retry_null(d)?;
                expect_array(d, 6)?;
                expect_token(d, "cursor")?;
                let cursor_kind = CursorKind::decode(d)?;
                let reason = CursorReason::decode(d)?;
                let checkpoint_digest = read_opt_digest(d)?;
                let floor_sequence = read_opt_u64(d)?;
                let head_sequence = read_opt_u64(d)?;
                ControlRefusal::CursorInvalid {
                    cursor_kind,
                    reason,
                    checkpoint_digest,
                    floor_sequence,
                    head_sequence,
                }
            }
            "peer_context_changed" => {
                expect_token(d, "peer")?;
                expect_bool(d, true)?;
                read_retry_null(d)?;
                expect_array(d, 5)?;
                expect_token(d, "peer_context")?;
                let side = PeerSide::decode(d)?;
                let prior_descriptor_digest = read_digest(d)?;
                let latest_descriptor_digest = read_opt_digest(d)?;
                let reason = PeerContextReason::decode(d)?;
                ControlRefusal::PeerContextChanged {
                    side,
                    prior_descriptor_digest,
                    latest_descriptor_digest,
                    reason,
                }
            }
            "admission_busy" | "removal_busy" | "commit_busy" => {
                expect_token(d, "capacity")?;
                expect_bool(d, true)?;
                let retry_after_seconds = read_retry_required(d)?;
                expect_array(d, 2)?;
                expect_token(d, "capacity")?;
                let limit_id = decode_limit_id(d)?;
                match code.as_str() {
                    "admission_busy" => ControlRefusal::AdmissionBusy {
                        limit_id,
                        retry_after_seconds,
                    },
                    "removal_busy" => ControlRefusal::RemovalBusy {
                        limit_id,
                        retry_after_seconds,
                    },
                    _ => ControlRefusal::CommitBusy {
                        limit_id,
                        retry_after_seconds,
                    },
                }
            }
            "peer_auth_failed" => {
                expect_token(d, "peer")?;
                expect_bool(d, true)?;
                read_retry_null(d)?;
                expect_array(d, 2)?;
                expect_token(d, "peer_auth")?;
                let stage = PeerAuthStage::decode(d)?;
                ControlRefusal::PeerAuthFailed { stage }
            }
            _ => return Err(CodecError::UnknownVariant),
        };
        Ok(refusal)
    }
}

fn decode_none_row(d: &mut Decoder<'_>, subject: &str, retryable: bool) -> Result<(), CodecError> {
    expect_token(d, subject)?;
    expect_bool(d, retryable)?;
    read_retry_null(d)?;
    expect_array(d, 1)?;
    expect_token(d, "none")?;
    Ok(())
}

fn decode_digests_row(
    d: &mut Decoder<'_>,
    subject: &str,
    retryable: bool,
) -> Result<([u8; 32], [u8; 32]), CodecError> {
    expect_token(d, subject)?;
    expect_bool(d, retryable)?;
    read_retry_null(d)?;
    expect_array(d, 3)?;
    expect_token(d, "digests")?;
    let expected_digest = read_digest(d)?;
    let observed_digest = read_digest(d)?;
    Ok((expected_digest, observed_digest))
}

fn decode_expiry_row(
    d: &mut Decoder<'_>,
    subject: &str,
    retryable: bool,
) -> Result<(u64, u64), CodecError> {
    expect_token(d, subject)?;
    expect_bool(d, retryable)?;
    read_retry_null(d)?;
    expect_array(d, 3)?;
    expect_token(d, "expiry")?;
    let expires_at = d.u64().map_err(|_| CodecError::Malformed)?;
    let observed_at = d.u64().map_err(|_| CodecError::Malformed)?;
    Ok((expires_at, observed_at))
}

fn decode_equivocation_row(
    d: &mut Decoder<'_>,
    subject: &str,
) -> Result<([u8; 32], [u8; 32]), CodecError> {
    expect_token(d, subject)?;
    expect_bool(d, false)?;
    read_retry_null(d)?;
    expect_array(d, 3)?;
    expect_token(d, "equivocation")?;
    let first_digest = read_digest(d)?;
    let second_digest = read_digest(d)?;
    Ok((first_digest, second_digest))
}

// ===========================================================================
// Control operations: request semantic bodies, ControlRequestV1, and digests.
// ===========================================================================

/// Maximum `PullDirectoryFeed` page size (design: "limit: at most 32").
pub const MAX_FEED_PULL_LIMIT: u64 = 32;
/// Maximum accepted directory snapshot cursor byte string.
pub const MAX_SNAPSHOT_CURSOR_BYTES: usize = 4 * 1024;

fn embed_value<R: CanonicalRecord>(buf: &mut Vec<u8>, record: &R) -> Result<(), CodecError> {
    buf.extend_from_slice(&record.encode_canonical()?);
    Ok(())
}

/// `describe` request (empty semantic body `[1]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DescribeV1;

/// `get_work_challenge` request semantic body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetWorkChallengeV1 {
    /// The operation the caller intends to run.
    pub intended_operation_kind: ControlOperationKind,
    /// The intended 128-bit idempotency key.
    pub intended_idempotency_key: [u8; IDEMPOTENCY_KEY_BYTES],
    /// The community root.
    pub community_root: [u8; 32],
    /// `work_target_digest` of the intended request (work-stamp slot `null`).
    pub work_target_digest: [u8; 32],
}

/// `prepare_host` request semantic body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrepareHostV1 {
    /// The root-signed composite ticket-core envelope.
    pub root_signed_ticket_core: RootSignedTicketCoreEnvelopeV2,
    /// The client-observed ordered `O`, `C`, `W` snapshot digests.
    pub ordered_namespace_snapshot_digests: [[u8; 32]; 3],
    /// An optional valid admission work stamp.
    pub work_stamp: Option<WorkStampV1>,
}

/// `commit_host` request semantic body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitHostV1 {
    /// The stable operation id.
    pub operation_id: [u8; 32],
    /// The final ordered `O`, `C`, `W` snapshot digests.
    pub ordered_namespace_snapshot_digests: [[u8; 32]; 3],
}

/// `submit_listing` request semantic body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitListingV1 {
    /// The complete canonical `AdmittedListingEnvelopeV1` bytes.
    pub admitted_listing_envelope_bytes: Vec<u8>,
    /// An optional valid admission work stamp.
    pub work_stamp: Option<WorkStampV1>,
}

/// `prepare_replica` request semantic body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrepareReplicaV1 {
    /// The destination-issued prepare challenge.
    pub replica_prepare_challenge: ReplicaPrepareChallengeV1,
    /// The source-signed attestation envelope.
    pub replica_source_attestation: ReplicaSourceAttestationV1,
    /// The same complete root-signed ticket-core envelope.
    pub root_signed_ticket_core: RootSignedTicketCoreEnvelopeV2,
    /// The desired ordered `O`, `C`, `W` snapshot digests.
    pub ordered_namespace_snapshot_digests: [[u8; 32]; 3],
}

/// `pull_directory_feed` request semantic body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PullDirectoryFeedV1 {
    /// The cursor: return inclusions after this sequence.
    pub after_sequence: u64,
    /// Page size (`<= 32`).
    pub limit: u64,
}

/// `pull_directory_snapshot` request semantic body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullDirectorySnapshotV1 {
    /// The verified checkpoint digest.
    pub checkpoint_digest: [u8; 32],
    /// The optional opaque `SnapshotCursorV1` bytes (`null` starts at ordinal 0).
    pub snapshot_cursor_bytes: Option<Vec<u8>>,
}

/// `get_operation` request semantic body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GetOperationV1 {
    /// The 256-bit operation id.
    pub operation_id: [u8; 32],
}

/// A decoded control-plane request operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlOperation {
    /// `describe`
    Describe(DescribeV1),
    /// `get_work_challenge`
    GetWorkChallenge(GetWorkChallengeV1),
    /// `prepare_host`
    PrepareHost(Box<PrepareHostV1>),
    /// `commit_host`
    CommitHost(CommitHostV1),
    /// `submit_listing`
    SubmitListing(SubmitListingV1),
    /// `prepare_replica`
    PrepareReplica(Box<PrepareReplicaV1>),
    /// `pull_directory_feed`
    PullDirectoryFeed(PullDirectoryFeedV1),
    /// `pull_directory_snapshot`
    PullDirectorySnapshot(PullDirectorySnapshotV1),
    /// `get_operation`
    GetOperation(GetOperationV1),
}

impl ControlOperation {
    /// The operation kind.
    pub fn kind(&self) -> ControlOperationKind {
        match self {
            ControlOperation::Describe(_) => ControlOperationKind::Describe,
            ControlOperation::GetWorkChallenge(_) => ControlOperationKind::GetWorkChallenge,
            ControlOperation::PrepareHost(_) => ControlOperationKind::PrepareHost,
            ControlOperation::CommitHost(_) => ControlOperationKind::CommitHost,
            ControlOperation::SubmitListing(_) => ControlOperationKind::SubmitListing,
            ControlOperation::PrepareReplica(_) => ControlOperationKind::PrepareReplica,
            ControlOperation::PullDirectoryFeed(_) => ControlOperationKind::PullDirectoryFeed,
            ControlOperation::PullDirectorySnapshot(_) => {
                ControlOperationKind::PullDirectorySnapshot
            }
            ControlOperation::GetOperation(_) => ControlOperationKind::GetOperation,
        }
    }

    /// Encode the operation's canonical `semantic_body`. When `with_work_stamp`
    /// is false, the fixed optional work-stamp slot is forced to `null` (this is
    /// the `work_target_digest` preimage form).
    pub fn encode_semantic_body(&self, with_work_stamp: bool) -> Result<Vec<u8>, CodecError> {
        let mut buf = Vec::new();
        match self {
            ControlOperation::Describe(_) => {
                let mut e = Encoder::new(&mut buf);
                e.array(1).map_err(|_| CodecError::Malformed)?;
                e.u64(1).map_err(|_| CodecError::Malformed)?;
            }
            ControlOperation::GetWorkChallenge(body) => {
                let mut e = Encoder::new(&mut buf);
                e.array(5).map_err(|_| CodecError::Malformed)?;
                e.u64(1).map_err(|_| CodecError::Malformed)?;
                body.intended_operation_kind.encode(&mut e)?;
                e.bytes(&body.intended_idempotency_key)
                    .map_err(|_| CodecError::Malformed)?;
                e.bytes(&body.community_root)
                    .map_err(|_| CodecError::Malformed)?;
                e.bytes(&body.work_target_digest)
                    .map_err(|_| CodecError::Malformed)?;
            }
            ControlOperation::PrepareHost(body) => {
                {
                    let mut e = Encoder::new(&mut buf);
                    e.array(4).map_err(|_| CodecError::Malformed)?;
                    e.u64(1).map_err(|_| CodecError::Malformed)?;
                }
                embed_value(&mut buf, &body.root_signed_ticket_core)?;
                {
                    let mut e = Encoder::new(&mut buf);
                    encode_triple_digests(&mut e, &body.ordered_namespace_snapshot_digests)?;
                }
                encode_optional_work_stamp(&mut buf, &body.work_stamp, with_work_stamp)?;
            }
            ControlOperation::CommitHost(body) => {
                let mut e = Encoder::new(&mut buf);
                e.array(3).map_err(|_| CodecError::Malformed)?;
                e.u64(1).map_err(|_| CodecError::Malformed)?;
                e.bytes(&body.operation_id)
                    .map_err(|_| CodecError::Malformed)?;
                encode_triple_digests(&mut e, &body.ordered_namespace_snapshot_digests)?;
            }
            ControlOperation::SubmitListing(body) => {
                {
                    let mut e = Encoder::new(&mut buf);
                    e.array(3).map_err(|_| CodecError::Malformed)?;
                    e.u64(1).map_err(|_| CodecError::Malformed)?;
                    e.bytes(&body.admitted_listing_envelope_bytes)
                        .map_err(|_| CodecError::Malformed)?;
                }
                encode_optional_work_stamp(&mut buf, &body.work_stamp, with_work_stamp)?;
            }
            ControlOperation::PrepareReplica(body) => {
                {
                    let mut e = Encoder::new(&mut buf);
                    e.array(5).map_err(|_| CodecError::Malformed)?;
                    e.u64(1).map_err(|_| CodecError::Malformed)?;
                }
                embed_value(&mut buf, &body.replica_prepare_challenge)?;
                embed_value(&mut buf, &body.replica_source_attestation)?;
                embed_value(&mut buf, &body.root_signed_ticket_core)?;
                {
                    let mut e = Encoder::new(&mut buf);
                    encode_triple_digests(&mut e, &body.ordered_namespace_snapshot_digests)?;
                }
            }
            ControlOperation::PullDirectoryFeed(body) => {
                if body.limit > MAX_FEED_PULL_LIMIT {
                    return Err(CodecError::LengthOutOfRange);
                }
                let mut e = Encoder::new(&mut buf);
                e.array(3).map_err(|_| CodecError::Malformed)?;
                e.u64(1).map_err(|_| CodecError::Malformed)?;
                e.u64(body.after_sequence)
                    .map_err(|_| CodecError::Malformed)?;
                e.u64(body.limit).map_err(|_| CodecError::Malformed)?;
            }
            ControlOperation::PullDirectorySnapshot(body) => {
                let mut e = Encoder::new(&mut buf);
                e.array(3).map_err(|_| CodecError::Malformed)?;
                e.u64(1).map_err(|_| CodecError::Malformed)?;
                e.bytes(&body.checkpoint_digest)
                    .map_err(|_| CodecError::Malformed)?;
                match &body.snapshot_cursor_bytes {
                    Some(bytes) => {
                        if bytes.len() > MAX_SNAPSHOT_CURSOR_BYTES {
                            return Err(CodecError::LengthOutOfRange);
                        }
                        e.bytes(bytes).map_err(|_| CodecError::Malformed)?;
                    }
                    None => {
                        e.null().map_err(|_| CodecError::Malformed)?;
                    }
                }
            }
            ControlOperation::GetOperation(body) => {
                let mut e = Encoder::new(&mut buf);
                e.array(2).map_err(|_| CodecError::Malformed)?;
                e.u64(1).map_err(|_| CodecError::Malformed)?;
                e.bytes(&body.operation_id)
                    .map_err(|_| CodecError::Malformed)?;
            }
        }
        Ok(buf)
    }

    fn decode_semantic_body(
        kind: ControlOperationKind,
        d: &mut Decoder<'_>,
    ) -> Result<Self, CodecError> {
        Ok(match kind {
            ControlOperationKind::Describe => {
                expect_array(d, 1)?;
                read_version(d, 1)?;
                ControlOperation::Describe(DescribeV1)
            }
            ControlOperationKind::GetWorkChallenge => {
                expect_array(d, 5)?;
                read_version(d, 1)?;
                let intended_operation_kind = ControlOperationKind::decode(d)?;
                let intended_idempotency_key = read_fixed_bytes::<IDEMPOTENCY_KEY_BYTES>(d)?;
                let community_root = read_fixed_bytes::<32>(d)?;
                let work_target_digest = read_fixed_bytes::<32>(d)?;
                ControlOperation::GetWorkChallenge(GetWorkChallengeV1 {
                    intended_operation_kind,
                    intended_idempotency_key,
                    community_root,
                    work_target_digest,
                })
            }
            ControlOperationKind::PrepareHost => {
                expect_array(d, 4)?;
                read_version(d, 1)?;
                let root_signed_ticket_core = RootSignedTicketCoreEnvelopeV2::decode_fields(d)?;
                let ordered_namespace_snapshot_digests = read_triple_digests(d)?;
                let work_stamp = decode_optional_work_stamp(d)?;
                ControlOperation::PrepareHost(Box::new(PrepareHostV1 {
                    root_signed_ticket_core,
                    ordered_namespace_snapshot_digests,
                    work_stamp,
                }))
            }
            ControlOperationKind::CommitHost => {
                expect_array(d, 3)?;
                read_version(d, 1)?;
                let operation_id = read_fixed_bytes::<32>(d)?;
                let ordered_namespace_snapshot_digests = read_triple_digests(d)?;
                ControlOperation::CommitHost(CommitHostV1 {
                    operation_id,
                    ordered_namespace_snapshot_digests,
                })
            }
            ControlOperationKind::SubmitListing => {
                expect_array(d, 3)?;
                read_version(d, 1)?;
                let admitted_listing_envelope_bytes =
                    read_bytes_max(d, crate::records::MAX_LISTING_ENVELOPE_BYTES)?;
                let work_stamp = decode_optional_work_stamp(d)?;
                ControlOperation::SubmitListing(SubmitListingV1 {
                    admitted_listing_envelope_bytes,
                    work_stamp,
                })
            }
            ControlOperationKind::PrepareReplica => {
                expect_array(d, 5)?;
                read_version(d, 1)?;
                let replica_prepare_challenge = ReplicaPrepareChallengeV1::decode_fields(d)?;
                let replica_source_attestation = ReplicaSourceAttestationV1::decode_fields(d)?;
                let root_signed_ticket_core = RootSignedTicketCoreEnvelopeV2::decode_fields(d)?;
                let ordered_namespace_snapshot_digests = read_triple_digests(d)?;
                ControlOperation::PrepareReplica(Box::new(PrepareReplicaV1 {
                    replica_prepare_challenge,
                    replica_source_attestation,
                    root_signed_ticket_core,
                    ordered_namespace_snapshot_digests,
                }))
            }
            ControlOperationKind::PullDirectoryFeed => {
                expect_array(d, 3)?;
                read_version(d, 1)?;
                let after_sequence = d.u64().map_err(|_| CodecError::Malformed)?;
                let limit = d.u64().map_err(|_| CodecError::Malformed)?;
                if limit > MAX_FEED_PULL_LIMIT {
                    return Err(CodecError::LengthOutOfRange);
                }
                ControlOperation::PullDirectoryFeed(PullDirectoryFeedV1 {
                    after_sequence,
                    limit,
                })
            }
            ControlOperationKind::PullDirectorySnapshot => {
                expect_array(d, 3)?;
                read_version(d, 1)?;
                let checkpoint_digest = read_fixed_bytes::<32>(d)?;
                let snapshot_cursor_bytes = if peek_null(d)? {
                    read_null(d)?;
                    None
                } else {
                    Some(read_bytes_max(d, MAX_SNAPSHOT_CURSOR_BYTES)?)
                };
                ControlOperation::PullDirectorySnapshot(PullDirectorySnapshotV1 {
                    checkpoint_digest,
                    snapshot_cursor_bytes,
                })
            }
            ControlOperationKind::GetOperation => {
                expect_array(d, 2)?;
                read_version(d, 1)?;
                let operation_id = read_fixed_bytes::<32>(d)?;
                ControlOperation::GetOperation(GetOperationV1 { operation_id })
            }
        })
    }

    /// `ControlDigestBodyV1 = [1, operation_kind, semantic_body]`. With
    /// `with_work_stamp = false` the work-stamp slot is `null` (work-target form).
    pub fn control_digest_body(&self, with_work_stamp: bool) -> Result<Vec<u8>, CodecError> {
        let semantic = self.encode_semantic_body(with_work_stamp)?;
        let mut buf = Vec::new();
        {
            let mut e = Encoder::new(&mut buf);
            e.array(3).map_err(|_| CodecError::Malformed)?;
            e.u64(1).map_err(|_| CodecError::Malformed)?;
            self.kind().encode(&mut e)?;
        }
        buf.extend_from_slice(&semantic);
        Ok(buf)
    }

    /// `control_request_digest = digest_v1("riot/anchor-control-request-body/v1",
    /// ControlDigestBodyV1)` including the real work stamp.
    pub fn control_request_digest(&self) -> Result<[u8; 32], CodecError> {
        Ok(digest_v1(
            label::CONTROL_REQUEST_BODY,
            &self.control_digest_body(true)?,
        ))
    }

    /// `work_target_digest = digest_v1("riot/anchor-work-target/v1",
    /// ControlDigestBodyV1)` with the work-stamp slot `null`.
    pub fn work_target_digest(&self) -> Result<[u8; 32], CodecError> {
        Ok(digest_v1(
            label::WORK_TARGET,
            &self.control_digest_body(false)?,
        ))
    }
}

fn encode_optional_work_stamp(
    buf: &mut Vec<u8>,
    work_stamp: &Option<WorkStampV1>,
    with_work_stamp: bool,
) -> Result<(), CodecError> {
    match (with_work_stamp, work_stamp) {
        (true, Some(stamp)) => embed_value(buf, stamp)?,
        _ => {
            let mut e = Encoder::new(&mut *buf);
            e.null().map_err(|_| CodecError::Malformed)?;
        }
    }
    Ok(())
}

fn decode_optional_work_stamp(d: &mut Decoder<'_>) -> Result<Option<WorkStampV1>, CodecError> {
    if peek_null(d)? {
        read_null(d)?;
        Ok(None)
    } else {
        Ok(Some(WorkStampV1::decode_fields(d)?))
    }
}

/// `ControlRequestV1 = [1, operation_kind, idempotency_key, semantic_body]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlRequestV1 {
    /// The request's random 128-bit idempotency key.
    pub idempotency_key: [u8; IDEMPOTENCY_KEY_BYTES],
    /// The operation and its semantic body.
    pub operation: ControlOperation,
}

impl CanonicalRecord for ControlRequestV1 {
    fn encode_canonical(&self) -> Result<Vec<u8>, CodecError> {
        let semantic = self.operation.encode_semantic_body(true)?;
        let mut buf = Vec::new();
        {
            let mut e = Encoder::new(&mut buf);
            e.array(4).map_err(|_| CodecError::Malformed)?;
            e.u64(1).map_err(|_| CodecError::Malformed)?;
            self.operation.kind().encode(&mut e)?;
            e.bytes(&self.idempotency_key)
                .map_err(|_| CodecError::Malformed)?;
        }
        buf.extend_from_slice(&semantic);
        Ok(buf)
    }

    fn decode_fields(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        expect_array(d, 4)?;
        read_version(d, 1)?;
        let kind = ControlOperationKind::decode(d)?;
        let idempotency_key = read_fixed_bytes::<IDEMPOTENCY_KEY_BYTES>(d)?;
        let operation = ControlOperation::decode_semantic_body(kind, d)?;
        Ok(ControlRequestV1 {
            idempotency_key,
            operation,
        })
    }
}

// ===========================================================================
// Effective operation limits + control success payloads + ControlResponseV1.
// ===========================================================================

/// Maximum canonical bytes for one directory-feed inclusion (design "Each
/// inclusion is at most 48 KiB").
pub const MAX_INCLUSION_BYTES: usize = 48 * 1024;
/// Maximum canonical bytes for a directory checkpoint / snapshot-record payload.
pub const MAX_DIRECTORY_FRAME_BYTES: usize = 60 * 1024;

/// `effective_operation_limits` — the 82 `[limit_id, effective_value]` rows in
/// strictly ascending ID order, drawn byte-identically from the limit profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveOperationLimits(pub Vec<(AnchorLimitId, LimitValue)>);

impl EffectiveOperationLimits {
    /// Project the effective values out of a full limit profile.
    pub fn from_profile(profile: &AnchorLimitProfileV1) -> Self {
        EffectiveOperationLimits(
            profile
                .entries
                .iter()
                .map(|entry| (entry.id, entry.effective))
                .collect(),
        )
    }

    fn encode(&self, buf: &mut Vec<u8>) -> Result<(), CodecError> {
        if self.0.len() != ALL_LIMIT_IDS.len() {
            return Err(CodecError::WrongArrayLength {
                expected: ALL_LIMIT_IDS.len() as u64,
                actual: self.0.len() as u64,
            });
        }
        let mut e = Encoder::new(&mut *buf);
        e.array(self.0.len() as u64)
            .map_err(|_| CodecError::Malformed)?;
        for (index, (id, value)) in self.0.iter().enumerate() {
            if id.id() != (index as u64) + 1 {
                return Err(CodecError::NonCanonical);
            }
            e.array(2).map_err(|_| CodecError::Malformed)?;
            encode_limit_id(&mut e, *id)?;
            value.encode(&mut e)?;
        }
        Ok(())
    }

    fn decode(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        let count = definite_array(d)?;
        if count != ALL_LIMIT_IDS.len() as u64 {
            return Err(CodecError::WrongArrayLength {
                expected: ALL_LIMIT_IDS.len() as u64,
                actual: count,
            });
        }
        let mut rows = Vec::with_capacity(count as usize);
        for index in 0..count {
            expect_array(d, 2)?;
            let id = decode_limit_id(d)?;
            if id.id() != index + 1 {
                return Err(CodecError::NonCanonical);
            }
            let value = LimitValue::decode(d)?;
            rows.push((id, value));
        }
        Ok(EffectiveOperationLimits(rows))
    }
}

/// `describe` success payload `[1, descriptor_envelope, anchor_limit_profile]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescribeSuccessV1 {
    /// The current signed descriptor envelope.
    pub descriptor: DescriptorEnvelopeV1,
    /// The advertised limit profile.
    pub limit_profile: AnchorLimitProfileV1,
}

/// Prepare success payload, shared by `prepare_host` and `prepare_replica`.
///
/// LAYOUT DECISION: `ordered_namespace_host_plan` is modelled as the ordered
/// `O`, `C`, `W` namespace-id triple (the design names the plan but gives no
/// inner structure); confirm before WU-006 freezes vectors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrepareSuccessV1 {
    /// The stable operation id.
    pub operation_id: [u8; 32],
    /// The captured base site generation.
    pub base_site_generation: u64,
    /// The ordered `O`, `C`, `W` namespace host plan (namespace ids).
    pub ordered_namespace_host_plan: [[u8; 32]; 3],
    /// The ordered `O`, `C`, `W` namespace tokens.
    pub ordered_namespace_tokens: [[u8; 32]; 3],
    /// The ordered `O`, `C`, `W` retained snapshot digests.
    pub ordered_retained_snapshot_digests: [[u8; 32]; 3],
    /// The sync version.
    pub sync_version: u64,
    /// The effective operation limits (82 rows).
    pub effective_operation_limits: EffectiveOperationLimits,
    /// The operation expiry (Unix seconds).
    pub operation_expiry: u64,
}

impl PrepareSuccessV1 {
    fn encode(&self, buf: &mut Vec<u8>) -> Result<(), CodecError> {
        {
            let mut e = Encoder::new(&mut *buf);
            e.array(9).map_err(|_| CodecError::Malformed)?;
            e.u64(1).map_err(|_| CodecError::Malformed)?;
            e.bytes(&self.operation_id)
                .map_err(|_| CodecError::Malformed)?;
            e.u64(self.base_site_generation)
                .map_err(|_| CodecError::Malformed)?;
            encode_triple_digests(&mut e, &self.ordered_namespace_host_plan)?;
            encode_triple_digests(&mut e, &self.ordered_namespace_tokens)?;
            encode_triple_digests(&mut e, &self.ordered_retained_snapshot_digests)?;
            e.u64(self.sync_version)
                .map_err(|_| CodecError::Malformed)?;
        }
        self.effective_operation_limits.encode(buf)?;
        {
            let mut e = Encoder::new(&mut *buf);
            e.u64(self.operation_expiry)
                .map_err(|_| CodecError::Malformed)?;
        }
        Ok(())
    }

    fn decode(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        expect_array(d, 9)?;
        read_version(d, 1)?;
        let operation_id = read_fixed_bytes::<32>(d)?;
        let base_site_generation = d.u64().map_err(|_| CodecError::Malformed)?;
        let ordered_namespace_host_plan = read_triple_digests(d)?;
        let ordered_namespace_tokens = read_triple_digests(d)?;
        let ordered_retained_snapshot_digests = read_triple_digests(d)?;
        let sync_version = d.u64().map_err(|_| CodecError::Malformed)?;
        let effective_operation_limits = EffectiveOperationLimits::decode(d)?;
        let operation_expiry = d.u64().map_err(|_| CodecError::Malformed)?;
        Ok(PrepareSuccessV1 {
            operation_id,
            base_site_generation,
            ordered_namespace_host_plan,
            ordered_namespace_tokens,
            ordered_retained_snapshot_digests,
            sync_version,
            effective_operation_limits,
            operation_expiry,
        })
    }
}

/// `pull_directory_feed` success payload: a page or a checkpoint-required signal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedPullSuccessV1 {
    /// `["page", inclusions, floor_sequence, head_sequence, head_digest, done]`.
    Page {
        /// Ordered opaque canonical inclusion bytes.
        inclusions: Vec<Vec<u8>>,
        /// The retained floor sequence.
        floor_sequence: u64,
        /// The head sequence.
        head_sequence: u64,
        /// The `inclusion_digest` at the head sequence.
        head_digest: [u8; 32],
        /// Whether the feed is exhausted.
        done: bool,
    },
    /// `["checkpoint_required", checkpoint, snapshot_cursor_bytes]`.
    CheckpointRequired {
        /// Opaque canonical checkpoint envelope bytes.
        checkpoint_bytes: Vec<u8>,
        /// Opaque `SnapshotCursorV1` bytes.
        snapshot_cursor_bytes: Vec<u8>,
    },
}

impl FeedPullSuccessV1 {
    fn encode(&self, buf: &mut Vec<u8>) -> Result<(), CodecError> {
        let mut e = Encoder::new(&mut *buf);
        match self {
            FeedPullSuccessV1::Page {
                inclusions,
                floor_sequence,
                head_sequence,
                head_digest,
                done,
            } => {
                e.array(6).map_err(|_| CodecError::Malformed)?;
                e.str("page").map_err(|_| CodecError::Malformed)?;
                e.array(inclusions.len() as u64)
                    .map_err(|_| CodecError::Malformed)?;
                for inclusion in inclusions {
                    if inclusion.len() > MAX_INCLUSION_BYTES {
                        return Err(CodecError::LengthOutOfRange);
                    }
                    e.bytes(inclusion).map_err(|_| CodecError::Malformed)?;
                }
                e.u64(*floor_sequence).map_err(|_| CodecError::Malformed)?;
                e.u64(*head_sequence).map_err(|_| CodecError::Malformed)?;
                e.bytes(head_digest).map_err(|_| CodecError::Malformed)?;
                e.bool(*done).map_err(|_| CodecError::Malformed)?;
            }
            FeedPullSuccessV1::CheckpointRequired {
                checkpoint_bytes,
                snapshot_cursor_bytes,
            } => {
                if checkpoint_bytes.len() > MAX_DIRECTORY_FRAME_BYTES
                    || snapshot_cursor_bytes.len() > MAX_SNAPSHOT_CURSOR_BYTES
                {
                    return Err(CodecError::LengthOutOfRange);
                }
                e.array(3).map_err(|_| CodecError::Malformed)?;
                e.str("checkpoint_required")
                    .map_err(|_| CodecError::Malformed)?;
                e.bytes(checkpoint_bytes)
                    .map_err(|_| CodecError::Malformed)?;
                e.bytes(snapshot_cursor_bytes)
                    .map_err(|_| CodecError::Malformed)?;
            }
        }
        Ok(())
    }

    fn decode(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        let len = definite_array(d)?;
        let tag = read_discriminant(d, MAX_TOKEN_BYTES)?;
        match tag.as_str() {
            "page" => {
                if len != 6 {
                    return Err(CodecError::WrongArrayLength {
                        expected: 6,
                        actual: len,
                    });
                }
                let inclusion_count = definite_array(d)?;
                if inclusion_count > MAX_FEED_PULL_LIMIT {
                    return Err(CodecError::LengthOutOfRange);
                }
                let mut inclusions = Vec::with_capacity(inclusion_count as usize);
                for _ in 0..inclusion_count {
                    inclusions.push(read_bytes_max(d, MAX_INCLUSION_BYTES)?);
                }
                let floor_sequence = d.u64().map_err(|_| CodecError::Malformed)?;
                let head_sequence = d.u64().map_err(|_| CodecError::Malformed)?;
                let head_digest = read_fixed_bytes::<32>(d)?;
                let done = d.bool().map_err(|_| CodecError::Malformed)?;
                Ok(FeedPullSuccessV1::Page {
                    inclusions,
                    floor_sequence,
                    head_sequence,
                    head_digest,
                    done,
                })
            }
            "checkpoint_required" => {
                if len != 3 {
                    return Err(CodecError::WrongArrayLength {
                        expected: 3,
                        actual: len,
                    });
                }
                let checkpoint_bytes = read_bytes_max(d, MAX_DIRECTORY_FRAME_BYTES)?;
                let snapshot_cursor_bytes = read_bytes_max(d, MAX_SNAPSHOT_CURSOR_BYTES)?;
                Ok(FeedPullSuccessV1::CheckpointRequired {
                    checkpoint_bytes,
                    snapshot_cursor_bytes,
                })
            }
            _ => Err(CodecError::UnknownVariant),
        }
    }
}

/// `pull_directory_snapshot` success payload
/// `[1, checkpoint, optional_snapshot_record, optional_next_cursor_bytes, done]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotPullSuccessV1 {
    /// Opaque canonical checkpoint envelope bytes.
    pub checkpoint_bytes: Vec<u8>,
    /// Optional opaque canonical snapshot-record bytes.
    pub snapshot_record_bytes: Option<Vec<u8>>,
    /// Optional next opaque `SnapshotCursorV1` bytes.
    pub next_cursor_bytes: Option<Vec<u8>>,
    /// Whether the snapshot traversal is done.
    pub done: bool,
}

impl SnapshotPullSuccessV1 {
    fn encode(&self, buf: &mut Vec<u8>) -> Result<(), CodecError> {
        if self.checkpoint_bytes.len() > MAX_DIRECTORY_FRAME_BYTES {
            return Err(CodecError::LengthOutOfRange);
        }
        let mut e = Encoder::new(&mut *buf);
        e.array(5).map_err(|_| CodecError::Malformed)?;
        e.u64(1).map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.checkpoint_bytes)
            .map_err(|_| CodecError::Malformed)?;
        match &self.snapshot_record_bytes {
            Some(bytes) => {
                if bytes.len() > MAX_DIRECTORY_FRAME_BYTES {
                    return Err(CodecError::LengthOutOfRange);
                }
                e.bytes(bytes).map_err(|_| CodecError::Malformed)?;
            }
            None => {
                e.null().map_err(|_| CodecError::Malformed)?;
            }
        }
        match &self.next_cursor_bytes {
            Some(bytes) => {
                if bytes.len() > MAX_SNAPSHOT_CURSOR_BYTES {
                    return Err(CodecError::LengthOutOfRange);
                }
                e.bytes(bytes).map_err(|_| CodecError::Malformed)?;
            }
            None => {
                e.null().map_err(|_| CodecError::Malformed)?;
            }
        }
        e.bool(self.done).map_err(|_| CodecError::Malformed)?;
        Ok(())
    }

    fn decode(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        expect_array(d, 5)?;
        read_version(d, 1)?;
        let checkpoint_bytes = read_bytes_max(d, MAX_DIRECTORY_FRAME_BYTES)?;
        let snapshot_record_bytes = if peek_null(d)? {
            read_null(d)?;
            None
        } else {
            Some(read_bytes_max(d, MAX_DIRECTORY_FRAME_BYTES)?)
        };
        let next_cursor_bytes = if peek_null(d)? {
            read_null(d)?;
            None
        } else {
            Some(read_bytes_max(d, MAX_SNAPSHOT_CURSOR_BYTES)?)
        };
        let done = d.bool().map_err(|_| CodecError::Malformed)?;
        Ok(SnapshotPullSuccessV1 {
            checkpoint_bytes,
            snapshot_record_bytes,
            next_cursor_bytes,
            done,
        })
    }
}

/// The originating Prepare kind of a `get_operation` result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrepareKind {
    /// `prepare_host`.
    PrepareHost,
    /// `prepare_replica`.
    PrepareReplica,
}

impl PrepareKind {
    fn token(self) -> &'static str {
        match self {
            PrepareKind::PrepareHost => "prepare_host",
            PrepareKind::PrepareReplica => "prepare_replica",
        }
    }
    fn from_token(token: &str) -> Option<Self> {
        match token {
            "prepare_host" => Some(PrepareKind::PrepareHost),
            "prepare_replica" => Some(PrepareKind::PrepareReplica),
            _ => None,
        }
    }
}

/// A terminal operation lifecycle outcome
/// `["committed", hosting_receipt] | ["refused", ControlRefusal]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalOperationOutcome {
    /// The operation committed with this signed hosting receipt.
    Committed(Box<HostingReceiptV1>),
    /// The operation terminally refused.
    Refused(ControlRefusal),
}

/// The `get_operation` operation state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GetOperationState {
    /// `["prepared", operation_expiry, prepare_success_payload]`.
    Prepared {
        /// The operation expiry (Unix seconds).
        operation_expiry: u64,
        /// The byte-identical embedded Prepare success payload.
        prepare_success: Box<PrepareSuccessV1>,
    },
    /// `["terminal", terminal_operation_outcome]`.
    Terminal {
        /// The terminal operation outcome.
        outcome: TerminalOperationOutcome,
    },
}

/// `get_operation` success payload
/// `[1, operation_id, originating_prepare_kind, prepared|terminal]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetOperationSuccessV1 {
    /// The operation id.
    pub operation_id: [u8; 32],
    /// The originating Prepare kind.
    pub originating_prepare_kind: PrepareKind,
    /// The current state.
    pub state: GetOperationState,
}

impl GetOperationSuccessV1 {
    fn encode(&self, buf: &mut Vec<u8>) -> Result<(), CodecError> {
        {
            let mut e = Encoder::new(&mut *buf);
            e.array(4).map_err(|_| CodecError::Malformed)?;
            e.u64(1).map_err(|_| CodecError::Malformed)?;
            e.bytes(&self.operation_id)
                .map_err(|_| CodecError::Malformed)?;
            e.str(self.originating_prepare_kind.token())
                .map_err(|_| CodecError::Malformed)?;
        }
        match &self.state {
            GetOperationState::Prepared {
                operation_expiry,
                prepare_success,
            } => {
                {
                    let mut e = Encoder::new(&mut *buf);
                    e.array(3).map_err(|_| CodecError::Malformed)?;
                    e.str("prepared").map_err(|_| CodecError::Malformed)?;
                    e.u64(*operation_expiry)
                        .map_err(|_| CodecError::Malformed)?;
                }
                prepare_success.encode(buf)?;
            }
            GetOperationState::Terminal { outcome } => {
                {
                    let mut e = Encoder::new(&mut *buf);
                    e.array(2).map_err(|_| CodecError::Malformed)?;
                    e.str("terminal").map_err(|_| CodecError::Malformed)?;
                }
                match outcome {
                    TerminalOperationOutcome::Committed(receipt) => {
                        {
                            let mut e = Encoder::new(&mut *buf);
                            e.array(2).map_err(|_| CodecError::Malformed)?;
                            e.str("committed").map_err(|_| CodecError::Malformed)?;
                        }
                        embed_value(buf, receipt.as_ref())?;
                    }
                    TerminalOperationOutcome::Refused(refusal) => {
                        {
                            let mut e = Encoder::new(&mut *buf);
                            e.array(2).map_err(|_| CodecError::Malformed)?;
                            e.str("refused").map_err(|_| CodecError::Malformed)?;
                        }
                        embed_value(buf, refusal)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn decode(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        expect_array(d, 4)?;
        read_version(d, 1)?;
        let operation_id = read_fixed_bytes::<32>(d)?;
        let kind_token = read_discriminant(d, MAX_TOKEN_BYTES)?;
        let originating_prepare_kind =
            PrepareKind::from_token(&kind_token).ok_or(CodecError::UnknownVariant)?;
        let state_len = definite_array(d)?;
        let state_tag = read_discriminant(d, MAX_TOKEN_BYTES)?;
        let state = match state_tag.as_str() {
            "prepared" => {
                if state_len != 3 {
                    return Err(CodecError::WrongArrayLength {
                        expected: 3,
                        actual: state_len,
                    });
                }
                let operation_expiry = d.u64().map_err(|_| CodecError::Malformed)?;
                let prepare_success = Box::new(PrepareSuccessV1::decode(d)?);
                GetOperationState::Prepared {
                    operation_expiry,
                    prepare_success,
                }
            }
            "terminal" => {
                if state_len != 2 {
                    return Err(CodecError::WrongArrayLength {
                        expected: 2,
                        actual: state_len,
                    });
                }
                expect_array(d, 2)?;
                let outcome_tag = read_discriminant(d, MAX_TOKEN_BYTES)?;
                let outcome = match outcome_tag.as_str() {
                    "committed" => TerminalOperationOutcome::Committed(Box::new(
                        HostingReceiptV1::decode_fields(d)?,
                    )),
                    "refused" => {
                        TerminalOperationOutcome::Refused(ControlRefusal::decode_fields(d)?)
                    }
                    _ => return Err(CodecError::UnknownVariant),
                };
                GetOperationState::Terminal { outcome }
            }
            _ => return Err(CodecError::UnknownVariant),
        };
        Ok(GetOperationSuccessV1 {
            operation_id,
            originating_prepare_kind,
            state,
        })
    }
}

/// A control success payload, one per operation kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlSuccess {
    /// `describe` success.
    Describe(Box<DescribeSuccessV1>),
    /// `get_work_challenge` success (`[1, work_challenge_envelope]`).
    GetWorkChallenge(Box<WorkChallengeV1>),
    /// `prepare_host` success.
    PrepareHost(Box<PrepareSuccessV1>),
    /// `commit_host` success (`[1, hosting_receipt]`).
    CommitHost(Box<HostingReceiptV1>),
    /// `submit_listing` success (`[1, listing_receipt]`).
    SubmitListing(Box<ListingReceiptV1>),
    /// `prepare_replica` success.
    PrepareReplica(Box<PrepareSuccessV1>),
    /// `pull_directory_feed` success.
    PullDirectoryFeed(FeedPullSuccessV1),
    /// `pull_directory_snapshot` success.
    PullDirectorySnapshot(SnapshotPullSuccessV1),
    /// `get_operation` success.
    GetOperation(Box<GetOperationSuccessV1>),
}

impl ControlSuccess {
    /// The operation kind this success payload belongs to.
    pub fn kind(&self) -> ControlOperationKind {
        match self {
            ControlSuccess::Describe(_) => ControlOperationKind::Describe,
            ControlSuccess::GetWorkChallenge(_) => ControlOperationKind::GetWorkChallenge,
            ControlSuccess::PrepareHost(_) => ControlOperationKind::PrepareHost,
            ControlSuccess::CommitHost(_) => ControlOperationKind::CommitHost,
            ControlSuccess::SubmitListing(_) => ControlOperationKind::SubmitListing,
            ControlSuccess::PrepareReplica(_) => ControlOperationKind::PrepareReplica,
            ControlSuccess::PullDirectoryFeed(_) => ControlOperationKind::PullDirectoryFeed,
            ControlSuccess::PullDirectorySnapshot(_) => ControlOperationKind::PullDirectorySnapshot,
            ControlSuccess::GetOperation(_) => ControlOperationKind::GetOperation,
        }
    }

    fn encode(&self, buf: &mut Vec<u8>) -> Result<(), CodecError> {
        match self {
            ControlSuccess::Describe(payload) => {
                {
                    let mut e = Encoder::new(&mut *buf);
                    e.array(3).map_err(|_| CodecError::Malformed)?;
                    e.u64(1).map_err(|_| CodecError::Malformed)?;
                }
                embed_value(buf, &payload.descriptor)?;
                embed_value(buf, &payload.limit_profile)?;
            }
            ControlSuccess::GetWorkChallenge(challenge) => {
                {
                    let mut e = Encoder::new(&mut *buf);
                    e.array(2).map_err(|_| CodecError::Malformed)?;
                    e.u64(1).map_err(|_| CodecError::Malformed)?;
                }
                embed_value(buf, challenge.as_ref())?;
            }
            ControlSuccess::PrepareHost(payload) | ControlSuccess::PrepareReplica(payload) => {
                payload.encode(buf)?;
            }
            ControlSuccess::CommitHost(receipt) => {
                {
                    let mut e = Encoder::new(&mut *buf);
                    e.array(2).map_err(|_| CodecError::Malformed)?;
                    e.u64(1).map_err(|_| CodecError::Malformed)?;
                }
                embed_value(buf, receipt.as_ref())?;
            }
            ControlSuccess::SubmitListing(receipt) => {
                {
                    let mut e = Encoder::new(&mut *buf);
                    e.array(2).map_err(|_| CodecError::Malformed)?;
                    e.u64(1).map_err(|_| CodecError::Malformed)?;
                }
                embed_value(buf, receipt.as_ref())?;
            }
            ControlSuccess::PullDirectoryFeed(payload) => {
                {
                    let mut e = Encoder::new(&mut *buf);
                    e.array(2).map_err(|_| CodecError::Malformed)?;
                    e.u64(1).map_err(|_| CodecError::Malformed)?;
                }
                payload.encode(buf)?;
            }
            ControlSuccess::PullDirectorySnapshot(payload) => {
                payload.encode(buf)?;
            }
            ControlSuccess::GetOperation(payload) => {
                payload.encode(buf)?;
            }
        }
        Ok(())
    }

    fn decode(kind: ControlOperationKind, d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        Ok(match kind {
            ControlOperationKind::Describe => {
                expect_array(d, 3)?;
                read_version(d, 1)?;
                let descriptor = DescriptorEnvelopeV1::decode_fields(d)?;
                let limit_profile = AnchorLimitProfileV1::decode_fields(d)?;
                ControlSuccess::Describe(Box::new(DescribeSuccessV1 {
                    descriptor,
                    limit_profile,
                }))
            }
            ControlOperationKind::GetWorkChallenge => {
                expect_array(d, 2)?;
                read_version(d, 1)?;
                ControlSuccess::GetWorkChallenge(Box::new(WorkChallengeV1::decode_fields(d)?))
            }
            ControlOperationKind::PrepareHost => {
                ControlSuccess::PrepareHost(Box::new(PrepareSuccessV1::decode(d)?))
            }
            ControlOperationKind::CommitHost => {
                expect_array(d, 2)?;
                read_version(d, 1)?;
                ControlSuccess::CommitHost(Box::new(HostingReceiptV1::decode_fields(d)?))
            }
            ControlOperationKind::SubmitListing => {
                expect_array(d, 2)?;
                read_version(d, 1)?;
                ControlSuccess::SubmitListing(Box::new(ListingReceiptV1::decode_fields(d)?))
            }
            ControlOperationKind::PrepareReplica => {
                ControlSuccess::PrepareReplica(Box::new(PrepareSuccessV1::decode(d)?))
            }
            ControlOperationKind::PullDirectoryFeed => {
                expect_array(d, 2)?;
                read_version(d, 1)?;
                ControlSuccess::PullDirectoryFeed(FeedPullSuccessV1::decode(d)?)
            }
            ControlOperationKind::PullDirectorySnapshot => {
                ControlSuccess::PullDirectorySnapshot(SnapshotPullSuccessV1::decode(d)?)
            }
            ControlOperationKind::GetOperation => {
                ControlSuccess::GetOperation(Box::new(GetOperationSuccessV1::decode(d)?))
            }
        })
    }
}

/// A control outcome: `["success", payload] | ["refused", ControlRefusal]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlOutcome {
    /// A successful outcome with its operation-specific payload.
    Success(ControlSuccess),
    /// A refused outcome.
    Refused(ControlRefusal),
}

/// `ControlResponseV1 = [1, operation_kind, outcome]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlResponseV1 {
    /// The responding operation kind.
    pub kind: ControlOperationKind,
    /// The outcome.
    pub outcome: ControlOutcome,
}

impl CanonicalRecord for ControlResponseV1 {
    fn encode_canonical(&self) -> Result<Vec<u8>, CodecError> {
        if let ControlOutcome::Success(success) = &self.outcome {
            if success.kind() != self.kind {
                return Err(CodecError::NonCanonical);
            }
        }
        let mut buf = Vec::new();
        {
            let mut e = Encoder::new(&mut buf);
            e.array(3).map_err(|_| CodecError::Malformed)?;
            e.u64(1).map_err(|_| CodecError::Malformed)?;
            self.kind.encode(&mut e)?;
        }
        match &self.outcome {
            ControlOutcome::Success(success) => {
                {
                    let mut e = Encoder::new(&mut buf);
                    e.array(2).map_err(|_| CodecError::Malformed)?;
                    e.str("success").map_err(|_| CodecError::Malformed)?;
                }
                success.encode(&mut buf)?;
            }
            ControlOutcome::Refused(refusal) => {
                {
                    let mut e = Encoder::new(&mut buf);
                    e.array(2).map_err(|_| CodecError::Malformed)?;
                    e.str("refused").map_err(|_| CodecError::Malformed)?;
                }
                embed_value(&mut buf, refusal)?;
            }
        }
        Ok(buf)
    }

    fn decode_fields(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        expect_array(d, 3)?;
        read_version(d, 1)?;
        let kind = ControlOperationKind::decode(d)?;
        expect_array(d, 2)?;
        let outcome_tag = read_discriminant(d, MAX_TOKEN_BYTES)?;
        let outcome = match outcome_tag.as_str() {
            "success" => ControlOutcome::Success(ControlSuccess::decode(kind, d)?),
            "refused" => ControlOutcome::Refused(ControlRefusal::decode_fields(d)?),
            _ => return Err(CodecError::UnknownVariant),
        };
        Ok(ControlResponseV1 { kind, outcome })
    }
}

// ===========================================================================
// Directory snapshot cursor.
// ===========================================================================

/// `SnapshotCursorBodyV1` — the authenticated snapshot pagination cursor body.
///
/// LAYOUT DECISION: `NameVn -> [1, ...]`; `snapshot_generation_id` is modelled as
/// a `u64` immutable-generation counter. The cursor tag is
/// `HMAC-SHA256(cursor_secret, snapshot_cursor_hmac_input(canonical(body)))`; this
/// crate builds the HMAC input only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotCursorBodyV1 {
    /// The bound checkpoint digest.
    pub checkpoint_digest: [u8; 32],
    /// The immutable snapshot generation id.
    pub snapshot_generation_id: u64,
    /// The next member ordinal.
    pub next_ordinal: u64,
    /// The member immediately before `next_ordinal` (`null` only at ordinal 0).
    pub previous_root: Option<[u8; 32]>,
    /// Issue time (Unix seconds).
    pub issued_at: u64,
    /// Expiry (Unix seconds).
    pub expires_at: u64,
    /// The cursor-secret epoch.
    pub cursor_secret_epoch: u32,
}

impl SnapshotCursorBodyV1 {
    /// The HMAC input the anchor's cursor secret is keyed over.
    pub fn cursor_tag_hmac_input(&self) -> Result<Vec<u8>, CodecError> {
        Ok(snapshot_cursor_hmac_input(&self.encode_canonical()?))
    }
}

impl CanonicalRecord for SnapshotCursorBodyV1 {
    fn encode_canonical(&self) -> Result<Vec<u8>, CodecError> {
        let mut buf = Vec::new();
        let mut e = Encoder::new(&mut buf);
        e.array(8).map_err(|_| CodecError::Malformed)?;
        e.u64(1).map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.checkpoint_digest)
            .map_err(|_| CodecError::Malformed)?;
        e.u64(self.snapshot_generation_id)
            .map_err(|_| CodecError::Malformed)?;
        e.u64(self.next_ordinal)
            .map_err(|_| CodecError::Malformed)?;
        match &self.previous_root {
            Some(root) => e.bytes(root).map_err(|_| CodecError::Malformed)?,
            None => e.null().map_err(|_| CodecError::Malformed)?,
        };
        e.u64(self.issued_at).map_err(|_| CodecError::Malformed)?;
        e.u64(self.expires_at).map_err(|_| CodecError::Malformed)?;
        e.u32(self.cursor_secret_epoch)
            .map_err(|_| CodecError::Malformed)?;
        Ok(buf)
    }

    fn decode_fields(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        expect_array(d, 8)?;
        read_version(d, 1)?;
        let checkpoint_digest = read_fixed_bytes::<32>(d)?;
        let snapshot_generation_id = d.u64().map_err(|_| CodecError::Malformed)?;
        let next_ordinal = d.u64().map_err(|_| CodecError::Malformed)?;
        let previous_root = if peek_null(d)? {
            read_null(d)?;
            None
        } else {
            Some(read_fixed_bytes::<32>(d)?)
        };
        let issued_at = d.u64().map_err(|_| CodecError::Malformed)?;
        let expires_at = d.u64().map_err(|_| CodecError::Malformed)?;
        let cursor_secret_epoch = d.u32().map_err(|_| CodecError::Malformed)?;
        Ok(SnapshotCursorBodyV1 {
            checkpoint_digest,
            snapshot_generation_id,
            next_ordinal,
            previous_root,
            issued_at,
            expires_at,
            cursor_secret_epoch,
        })
    }
}

/// `SnapshotCursorV1 = [1, SnapshotCursorBodyV1, exactly_32_byte_cursor_tag]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotCursorV1 {
    /// The cursor body.
    pub body: SnapshotCursorBodyV1,
    /// The 32-byte HMAC cursor tag.
    pub cursor_tag: [u8; 32],
}

impl CanonicalRecord for SnapshotCursorV1 {
    fn encode_canonical(&self) -> Result<Vec<u8>, CodecError> {
        let mut buf = Vec::new();
        {
            let mut e = Encoder::new(&mut buf);
            e.array(3).map_err(|_| CodecError::Malformed)?;
            e.u64(1).map_err(|_| CodecError::Malformed)?;
        }
        buf.extend_from_slice(&self.body.encode_canonical()?);
        {
            let mut e = Encoder::new(&mut buf);
            e.bytes(&self.cursor_tag)
                .map_err(|_| CodecError::Malformed)?;
        }
        Ok(buf)
    }

    fn decode_fields(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        expect_array(d, 3)?;
        read_version(d, 1)?;
        let body = SnapshotCursorBodyV1::decode_fields(d)?;
        let cursor_tag = read_fixed_bytes::<32>(d)?;
        Ok(SnapshotCursorV1 { body, cursor_tag })
    }
}

// ===========================================================================
// verify_descriptor_chain
// ===========================================================================

/// Why a descriptor-chain traversal failed. Any variant maps the client to the
/// design's `descriptor_chain_unavailable` outcome (app update / trust reset).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DescriptorError {
    /// A page envelope failed canonical decode / re-encode.
    Codec(CodecError),
    /// A body's recomputed `AnchorId` did not equal its stated `anchor_id`.
    AnchorIdRecomputeMismatch,
    /// A successor's `anchor_id` was not the pinned stable id.
    AnchorIdMismatch,
    /// `current_operator_key_id` did not equal the key-ID of the carried key.
    KeyIdMismatch,
    /// A successor epoch was not exactly `predecessor_epoch + 1`.
    EpochNotIncrementing,
    /// `previous_descriptor_digest` did not equal the pinned floor digest.
    WrongPredecessorDigest,
    /// The current-operator signature did not verify.
    BadCurrentSignature,
    /// A successor omitted its mandatory predecessor signature.
    MissingPredecessorSignature,
    /// A successor omitted / changed its predecessor verification key.
    PredecessorKeyMismatch,
    /// The predecessor-transition signature did not verify.
    BadPredecessorSignature,
    /// Successive descriptor validity windows did not overlap as signed.
    TimeOverlapViolation,
    /// The resulting head descriptor is not currently valid.
    HeadExpired,
    /// The traversal exceeded 32 hops.
    HopsCapExceeded,
    /// The traversal exceeded 256 KiB of canonical bytes.
    BytesCapExceeded,
}

impl From<CodecError> for DescriptorError {
    fn from(err: CodecError) -> Self {
        DescriptorError::Codec(err)
    }
}

impl core::fmt::Display for DescriptorError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for DescriptorError {}

/// Verify a descriptor chain from a pinned `floor` forward through `pages`,
/// returning the newest authenticated floor.
///
/// Each successor must recompute the stable `AnchorId`, carry `epoch =
/// predecessor_epoch + 1`, name the pinned digest as its previous, verify its
/// current- and predecessor-operator signatures (the predecessor key equal to the
/// pinned floor key), preserve issued/expiry overlap, and only the resulting head
/// must be currently valid. Traversal is capped at 32 hops and 256 KiB. An empty
/// iterator leaves the floor unchanged.
pub fn verify_descriptor_chain(
    floor: DescriptorFloor,
    pages: impl Iterator<Item = DescriptorEnvelopeV1>,
    now: u64,
) -> Result<DescriptorFloor, DescriptorError> {
    let mut current = floor;
    let mut hops = 0usize;
    let mut total_bytes = 0usize;
    let mut previous_body: Option<AnchorDescriptorBodyV1> = None;
    let mut head_times: Option<(u64, u64)> = None;

    for envelope in pages {
        hops += 1;
        if hops > crate::records::MAX_DESCRIPTOR_CHAIN_HOPS {
            return Err(DescriptorError::HopsCapExceeded);
        }
        let encoded = envelope.encode_canonical()?;
        total_bytes = total_bytes.saturating_add(encoded.len());
        if total_bytes > crate::records::MAX_DESCRIPTOR_CHAIN_BYTES {
            return Err(DescriptorError::BytesCapExceeded);
        }
        let body = envelope.body.clone();

        if body.recomputed_anchor_id() != body.anchor_id {
            return Err(DescriptorError::AnchorIdRecomputeMismatch);
        }
        if body.anchor_id != current.anchor_id {
            return Err(DescriptorError::AnchorIdMismatch);
        }
        if body.current_operator_verification_key.operator_key_id()? != body.current_operator_key_id
        {
            return Err(DescriptorError::KeyIdMismatch);
        }
        if body.descriptor_epoch != current.descriptor_epoch + 1 {
            return Err(DescriptorError::EpochNotIncrementing);
        }
        if body.previous_descriptor_digest != Some(current.descriptor_digest) {
            return Err(DescriptorError::WrongPredecessorDigest);
        }
        let predecessor_signature = envelope
            .predecessor_signature
            .ok_or(DescriptorError::MissingPredecessorSignature)?;
        match &body.predecessor_operator_verification_key {
            Some(key) if *key == current.operator_verification_key => {}
            _ => return Err(DescriptorError::PredecessorKeyMismatch),
        }
        envelope
            .verify_current()
            .map_err(|_| DescriptorError::BadCurrentSignature)?;
        let predecessor_preimage = envelope.predecessor_signing_preimage()?;
        if !crate::records::verify_ed25519_strict(
            &current.operator_verification_key.public_key,
            &predecessor_preimage,
            &predecessor_signature,
        ) {
            return Err(DescriptorError::BadPredecessorSignature);
        }
        if body.issued_at >= body.expires_at {
            return Err(DescriptorError::TimeOverlapViolation);
        }
        if let Some(prev) = &previous_body {
            if body.issued_at < prev.issued_at || body.issued_at > prev.expires_at {
                return Err(DescriptorError::TimeOverlapViolation);
            }
        }

        let descriptor_digest = envelope.descriptor_digest()?;
        current = DescriptorFloor {
            anchor_id: body.anchor_id,
            descriptor_epoch: body.descriptor_epoch,
            descriptor_digest,
            operator_verification_key: body.current_operator_verification_key,
        };
        head_times = Some((body.issued_at, body.expires_at));
        previous_body = Some(body);
    }

    if let Some((issued_at, expires_at)) = head_times {
        if now < issued_at || now >= expires_at {
            return Err(DescriptorError::HeadExpired);
        }
    }
    Ok(current)
}
