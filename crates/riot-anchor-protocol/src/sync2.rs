//! WU-005: the `riot/sync/2` routed paginated reconciliation protocol — the
//! bounded `OpenNamespace` routing frame and its three modes (`ReadCommitted`,
//! `HostReconcileStaged`, `ReplicaIntoStaged`), the paginated immutable-snapshot
//! inventory frame set, the closed sync refusal matrix, the page digest, and the
//! transport-independent per-direction sender/receiver finite state machine.
//!
//! Every layout transcribes the design's "`riot/sync/2`: Routed Paginated
//! Reconciliation", "Composite transaction", and "TDD Slice 2" sections. As with
//! WU-004, optional slots are `null`-or-value, closed enums are `snake_case`, sum
//! variants encode as `[variant_name, ...fields]`, and `*_bytes` fields carry
//! separately-canonical bytes. The FSM is transport-independent: it consumes
//! [`Sync2Frame`]s and yields [`Sync2Action`]s and never performs IO, so it obeys
//! the crate's `dependency_boundary` contract.
//!
//! Under-specified byte layouts (the design gives field lists but no positional
//! encoding for the frames) are chosen per the WU-002 canonical conventions and
//! flagged in the WU-005 report as INVENTED — confirm before WU-006. The sync
//! module is version-scoped `v2`: per-frame version integers are implicit (only
//! `OpenNamespace` carries an explicit negotiated `protocol_version`).

use minicbor::{Decoder, Encoder};

use crate::authority::TicketReason;
use crate::codec::{
    definite_array, expect_array, peek_null, read_bytes_max, read_discriminant, read_fixed_bytes,
    read_null, CanonicalRecord, CodecError,
};
use crate::control::{PeerContextReason, PeerSide, TransportMode};
use crate::digest::{digest_v1, label, sync_snapshot_digest};
use crate::records::{AnchorLimitId, LimitValue, MAX_TICKET_CORE_BYTES};

// ---------------------------------------------------------------------------
// Bounds. The design fixes the pagination and bundle ceilings; the identifier
// and session bounds are INVENTED per WU-002 conventions (flagged in the report).
// ---------------------------------------------------------------------------

/// Maximum accepted `OpenNamespace.session_id` bytes (INVENTED bound).
pub const MAX_SESSION_ID_BYTES: usize = 32;
/// Maximum accepted full canonical entry-ID bytes (INVENTED bound).
pub const MAX_ENTRY_ID_BYTES: usize = 128;
/// A single `IdsPage` carries at most 256 sorted entry IDs (design).
pub const MAX_IDS_PER_PAGE: usize = 256;
/// A receiver may send at most four `NeedEntries` frames per page (design).
pub const MAX_NEEDS_PER_PAGE: usize = 4;
/// A single `NeedEntries` requests at most 64 entry IDs (design).
pub const MAX_IDS_PER_NEED: usize = 64;
/// A single `EntriesChunk` bundle holds at most 64 entries (design).
pub const MAX_ENTRIES_PER_CHUNK: usize = 64;
/// A single `EntriesChunk` bundle is at most 8 MiB (design).
pub const MAX_CHUNK_BUNDLE_BYTES: usize = 8 * 1024 * 1024;
/// An admitted anchor-profile encoded item including proofs is at most 2 MiB
/// (design) — so at least one item always fits a chunk.
pub const MAX_ANCHOR_ITEM_BYTES: usize = 2 * 1024 * 1024;
/// The maximum accepted `sync/2` frame size: an 8 MiB bundle plus framing slack.
pub const MAX_SYNC2_FRAME_BYTES: usize = MAX_CHUNK_BUNDLE_BYTES + 64 * 1024;
/// Cap on the `unsupported_version` / `stale_source` inline vectors.
const MAX_INLINE_VECTOR: usize = 16;

const MAX_TOKEN_BYTES: usize = 48;

// ---------------------------------------------------------------------------
// Closed textual token vocabularies specific to `sync/2`. The shared transport,
// peer-side, and peer-context enums are reused from `control`.
// ---------------------------------------------------------------------------

macro_rules! sync_token_enum {
    (
        $(#[$outer:meta])*
        $name:ident { $($variant:ident => $tok:literal),+ $(,)? }
    ) => {
        $(#[$outer])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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

sync_token_enum! {
    /// The direction phase discriminant (design: `AnchorToClient`, `ClientToAnchor`,
    /// `SourceToDestination`).
    Sync2Phase {
        AnchorToClient => "anchor_to_client",
        ClientToAnchor => "client_to_anchor",
        SourceToDestination => "source_to_destination",
    }
}

sync_token_enum! {
    /// The routed mode discriminant (tag only; the `OpenNamespace` mode carries the
    /// staged fields).
    Sync2ModeTag {
        ReadCommitted => "read_committed",
        HostReconcileStaged => "host_reconcile_staged",
        ReplicaIntoStaged => "replica_into_staged",
    }
}

sync_token_enum! {
    /// The `admission_failed` subject (design: `authority | bundle | entry`).
    AdmissionSubject {
        Authority => "authority",
        Bundle => "bundle",
        Entry => "entry",
    }
}

sync_token_enum! {
    /// The closed set of `sync/2` frame names — the exact discriminants declared by
    /// the FSM, used for dispatch and for `unexpected_frame` details.
    Sync2FrameName {
        OpenNamespace => "open_namespace",
        SnapshotStart => "snapshot_start",
        IdsPage => "ids_page",
        NeedEntries => "need_entries",
        PageNeedsComplete => "page_needs_complete",
        EntriesChunk => "entries_chunk",
        PageComplete => "page_complete",
        DirectionComplete => "direction_complete",
        NamespaceComplete => "namespace_complete",
        Refuse => "refuse",
    }
}

// The `invalid_ticket` reason has no shared token method (it is an authority-layer
// enum); map its exact `signature | root | structure` tokens locally.
fn ticket_reason_token(reason: TicketReason) -> &'static str {
    match reason {
        TicketReason::Signature => "signature",
        TicketReason::Root => "root",
        TicketReason::Structure => "structure",
    }
}

fn ticket_reason_from_token(token: &str) -> Option<TicketReason> {
    match token {
        "signature" => Some(TicketReason::Signature),
        "root" => Some(TicketReason::Root),
        "structure" => Some(TicketReason::Structure),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Small codec helpers (module-local; the `control` equivalents are private).
// ---------------------------------------------------------------------------

fn encode_bool(e: &mut Encoder<&mut Vec<u8>>, value: bool) -> Result<(), CodecError> {
    e.bool(value).map_err(|_| CodecError::Malformed)?;
    Ok(())
}

fn read_bool(d: &mut Decoder<'_>) -> Result<bool, CodecError> {
    d.bool().map_err(|_| CodecError::Malformed)
}

fn expect_token(d: &mut Decoder<'_>, expected: &str) -> Result<(), CodecError> {
    let token = read_discriminant(d, MAX_TOKEN_BYTES)?;
    if token == expected {
        Ok(())
    } else {
        Err(CodecError::UnknownVariant)
    }
}

fn expect_bool(d: &mut Decoder<'_>, expected: bool) -> Result<(), CodecError> {
    if read_bool(d)? == expected {
        Ok(())
    } else {
        Err(CodecError::NonCanonical)
    }
}

fn read_u64(d: &mut Decoder<'_>) -> Result<u64, CodecError> {
    d.u64().map_err(|_| CodecError::Malformed)
}

fn encode_u64(e: &mut Encoder<&mut Vec<u8>>, v: u64) -> Result<(), CodecError> {
    e.u64(v).map_err(|_| CodecError::Malformed)?;
    Ok(())
}

fn read_digest(d: &mut Decoder<'_>) -> Result<[u8; 32], CodecError> {
    read_fixed_bytes::<32>(d)
}

fn encode_digest(e: &mut Encoder<&mut Vec<u8>>, digest: &[u8; 32]) -> Result<(), CodecError> {
    e.bytes(digest).map_err(|_| CodecError::Malformed)?;
    Ok(())
}

fn read_retry_null(d: &mut Decoder<'_>) -> Result<(), CodecError> {
    if peek_null(d)? {
        read_null(d)
    } else {
        Err(CodecError::NonCanonical)
    }
}

fn read_retry_required(d: &mut Decoder<'_>) -> Result<u64, CodecError> {
    if peek_null(d)? {
        return Err(CodecError::NonCanonical);
    }
    let value = read_u64(d)?;
    if value == 0 {
        return Err(CodecError::NonCanonical);
    }
    Ok(value)
}

fn encode_opt_id(e: &mut Encoder<&mut Vec<u8>>, id: &Option<Vec<u8>>) -> Result<(), CodecError> {
    match id {
        Some(bytes) => {
            if bytes.len() > MAX_ENTRY_ID_BYTES {
                return Err(CodecError::LengthOutOfRange);
            }
            e.bytes(bytes).map_err(|_| CodecError::Malformed)?;
        }
        None => {
            e.null().map_err(|_| CodecError::Malformed)?;
        }
    }
    Ok(())
}

fn read_opt_id(d: &mut Decoder<'_>) -> Result<Option<Vec<u8>>, CodecError> {
    if peek_null(d)? {
        read_null(d)?;
        Ok(None)
    } else {
        Ok(Some(read_bytes_max(d, MAX_ENTRY_ID_BYTES)?))
    }
}

fn encode_id_list(
    e: &mut Encoder<&mut Vec<u8>>,
    ids: &[Vec<u8>],
    max: usize,
) -> Result<(), CodecError> {
    if ids.len() > max {
        return Err(CodecError::LengthOutOfRange);
    }
    e.array(ids.len() as u64)
        .map_err(|_| CodecError::Malformed)?;
    for id in ids {
        if id.len() > MAX_ENTRY_ID_BYTES {
            return Err(CodecError::LengthOutOfRange);
        }
        e.bytes(id).map_err(|_| CodecError::Malformed)?;
    }
    Ok(())
}

fn read_id_list(d: &mut Decoder<'_>, max: usize) -> Result<Vec<Vec<u8>>, CodecError> {
    let count = definite_array(d)?;
    if count as usize > max {
        return Err(CodecError::LengthOutOfRange);
    }
    let mut ids = Vec::with_capacity(count as usize);
    for _ in 0..count {
        ids.push(read_bytes_max(d, MAX_ENTRY_ID_BYTES)?);
    }
    Ok(ids)
}

fn encode_limit_id(e: &mut Encoder<&mut Vec<u8>>, id: AnchorLimitId) -> Result<(), CodecError> {
    e.u64(id.id()).map_err(|_| CodecError::Malformed)?;
    Ok(())
}

fn decode_limit_id(d: &mut Decoder<'_>) -> Result<AnchorLimitId, CodecError> {
    let raw = read_u64(d)?;
    AnchorLimitId::from_id(raw).ok_or(CodecError::UnknownVariant)
}

// ---------------------------------------------------------------------------
// The routing frame and its mode.
// ---------------------------------------------------------------------------

/// The routed reconciliation mode. `HostReconcileStaged` is a bidirectional
/// organizer reconciliation; `ReplicaIntoStaged` is one-way source→destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sync2Mode {
    /// The public follow/read path: one-way committed snapshot, no operation.
    ReadCommitted,
    /// A staged organizer host reconciliation bound to an operation + token.
    HostReconcileStaged {
        /// The stable operation ID.
        operation_id: [u8; 32],
        /// The unguessable per-namespace 256-bit token.
        namespace_token: [u8; 32],
    },
    /// A staged one-way replication into a destination stage.
    ReplicaIntoStaged {
        /// The stable operation ID.
        operation_id: [u8; 32],
        /// The unguessable per-namespace 256-bit token.
        namespace_token: [u8; 32],
    },
}

impl Sync2Mode {
    /// The tag-only discriminant for this mode.
    pub fn tag(&self) -> Sync2ModeTag {
        match self {
            Sync2Mode::ReadCommitted => Sync2ModeTag::ReadCommitted,
            Sync2Mode::HostReconcileStaged { .. } => Sync2ModeTag::HostReconcileStaged,
            Sync2Mode::ReplicaIntoStaged { .. } => Sync2ModeTag::ReplicaIntoStaged,
        }
    }

    fn encode(&self, e: &mut Encoder<&mut Vec<u8>>) -> Result<(), CodecError> {
        match self {
            Sync2Mode::ReadCommitted => {
                e.array(1).map_err(|_| CodecError::Malformed)?;
                Sync2ModeTag::ReadCommitted.encode(e)?;
            }
            Sync2Mode::HostReconcileStaged {
                operation_id,
                namespace_token,
            } => {
                e.array(3).map_err(|_| CodecError::Malformed)?;
                Sync2ModeTag::HostReconcileStaged.encode(e)?;
                encode_digest(e, operation_id)?;
                encode_digest(e, namespace_token)?;
            }
            Sync2Mode::ReplicaIntoStaged {
                operation_id,
                namespace_token,
            } => {
                e.array(3).map_err(|_| CodecError::Malformed)?;
                Sync2ModeTag::ReplicaIntoStaged.encode(e)?;
                encode_digest(e, operation_id)?;
                encode_digest(e, namespace_token)?;
            }
        }
        Ok(())
    }

    fn decode(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        let count = definite_array(d)?;
        let tag = Sync2ModeTag::decode(d)?;
        match tag {
            Sync2ModeTag::ReadCommitted => {
                if count != 1 {
                    return Err(CodecError::WrongArrayLength {
                        expected: 1,
                        actual: count,
                    });
                }
                Ok(Sync2Mode::ReadCommitted)
            }
            Sync2ModeTag::HostReconcileStaged | Sync2ModeTag::ReplicaIntoStaged => {
                if count != 3 {
                    return Err(CodecError::WrongArrayLength {
                        expected: 3,
                        actual: count,
                    });
                }
                let operation_id = read_digest(d)?;
                let namespace_token = read_digest(d)?;
                if tag == Sync2ModeTag::HostReconcileStaged {
                    Ok(Sync2Mode::HostReconcileStaged {
                        operation_id,
                        namespace_token,
                    })
                } else {
                    Ok(Sync2Mode::ReplicaIntoStaged {
                        operation_id,
                        namespace_token,
                    })
                }
            }
        }
    }
}

/// The bounded routing frame the responder reads before constructing a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenNamespace {
    /// The negotiated protocol version (must be 2).
    pub protocol_version: u64,
    /// The connection's session identifier (INVENTED bound: at most 32 bytes).
    pub session_id: Vec<u8>,
    /// The separately-canonical `RootSignedTicketCoreEnvelopeV2` bytes.
    pub ticket_core_bytes: Vec<u8>,
    /// The requested namespace ID.
    pub namespace_id: [u8; 32],
    /// The routed mode.
    pub mode: Sync2Mode,
}

// ---------------------------------------------------------------------------
// The inventory frame set.
// ---------------------------------------------------------------------------

/// `SnapshotStart { phase, namespace_id, snapshot_digest, entry_count, logical_bytes }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotStart {
    /// Direction phase.
    pub phase: Sync2Phase,
    /// Namespace ID.
    pub namespace_id: [u8; 32],
    /// The immutable sender snapshot digest.
    pub snapshot_digest: [u8; 32],
    /// The snapshot's entry count.
    pub entry_count: u64,
    /// The snapshot's exact logical byte sum.
    pub logical_bytes: u64,
}

/// `IdsPage { phase, snapshot_digest, after_exclusive?, entry_ids: <=256, done }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdsPage {
    /// Direction phase.
    pub phase: Sync2Phase,
    /// The advertised snapshot digest (must equal `SnapshotStart`).
    pub snapshot_digest: [u8; 32],
    /// The exclusive cursor: the last ID of the previous page, or `None` first.
    pub after_exclusive: Option<Vec<u8>>,
    /// The strictly ascending page of full entry IDs (at most 256).
    pub entry_ids: Vec<Vec<u8>>,
    /// Whether this is the final page of the inventory.
    pub done: bool,
}

/// `NeedEntries { phase, page_digest, request_id, entry_ids: <=64 }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NeedEntries {
    /// Direction phase.
    pub phase: Sync2Phase,
    /// The digest of the page these IDs were drawn from.
    pub page_digest: [u8; 32],
    /// The receiver-chosen distinct request ID.
    pub request_id: u64,
    /// The requested IDs (each present in the page once; at most 64).
    pub entry_ids: Vec<Vec<u8>>,
}

/// `PageNeedsComplete { phase, page_digest }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageNeedsComplete {
    /// Direction phase.
    pub phase: Sync2Phase,
    /// The page digest.
    pub page_digest: [u8; 32],
}

/// `EntriesChunk { phase, page_digest, request_id, chunk_index, done, bundle }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntriesChunk {
    /// Direction phase.
    pub phase: Sync2Phase,
    /// The page digest.
    pub page_digest: [u8; 32],
    /// The request this chunk answers.
    pub request_id: u64,
    /// The chunk index (starts at 0, no gaps, only the last has `done`).
    pub chunk_index: u64,
    /// Whether this is the last chunk of the request.
    pub done: bool,
    /// The separately-canonical bundle bytes (at most 64 entries, 8 MiB): a
    /// canonical CBOR array of per-item canonical byte strings.
    pub bundle_bytes: Vec<u8>,
}

/// `PageComplete { phase, page_digest }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageComplete {
    /// Direction phase.
    pub phase: Sync2Phase,
    /// The page digest.
    pub page_digest: [u8; 32],
}

/// `DirectionComplete { phase, sender_snapshot_digest }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectionComplete {
    /// Direction phase.
    pub phase: Sync2Phase,
    /// The sender's advertised snapshot digest, echoed by the receiver.
    pub sender_snapshot_digest: [u8; 32],
}

/// `NamespaceComplete { mode, final_snapshot_digest }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamespaceComplete {
    /// The routed mode tag.
    pub mode: Sync2ModeTag,
    /// The terminal committed/staged snapshot digest.
    pub final_snapshot_digest: [u8; 32],
}

/// The complete closed set of `sync/2` frames.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sync2Frame {
    /// The bounded routing frame.
    OpenNamespace(OpenNamespace),
    /// The immutable snapshot header.
    SnapshotStart(SnapshotStart),
    /// One inventory ID page.
    IdsPage(IdsPage),
    /// A receiver request for entries within a page.
    NeedEntries(NeedEntries),
    /// End of a page's requests.
    PageNeedsComplete(PageNeedsComplete),
    /// One bundle chunk answering a request.
    EntriesChunk(EntriesChunk),
    /// End of a page's chunks.
    PageComplete(PageComplete),
    /// End of a direction.
    DirectionComplete(DirectionComplete),
    /// End of the namespace session.
    NamespaceComplete(NamespaceComplete),
    /// A terminal (or the sole retryable `busy`) refusal.
    Refuse(Sync2Refusal),
}

impl Sync2Frame {
    /// The frame's closed name discriminant.
    pub fn name(&self) -> Sync2FrameName {
        match self {
            Sync2Frame::OpenNamespace(_) => Sync2FrameName::OpenNamespace,
            Sync2Frame::SnapshotStart(_) => Sync2FrameName::SnapshotStart,
            Sync2Frame::IdsPage(_) => Sync2FrameName::IdsPage,
            Sync2Frame::NeedEntries(_) => Sync2FrameName::NeedEntries,
            Sync2Frame::PageNeedsComplete(_) => Sync2FrameName::PageNeedsComplete,
            Sync2Frame::EntriesChunk(_) => Sync2FrameName::EntriesChunk,
            Sync2Frame::PageComplete(_) => Sync2FrameName::PageComplete,
            Sync2Frame::DirectionComplete(_) => Sync2FrameName::DirectionComplete,
            Sync2Frame::NamespaceComplete(_) => Sync2FrameName::NamespaceComplete,
            Sync2Frame::Refuse(_) => Sync2FrameName::Refuse,
        }
    }
}

impl CanonicalRecord for Sync2Frame {
    fn encode_canonical(&self) -> Result<Vec<u8>, CodecError> {
        let mut buf = Vec::new();
        {
            let mut e = Encoder::new(&mut buf);
            match self {
                Sync2Frame::OpenNamespace(f) => {
                    e.array(6).map_err(|_| CodecError::Malformed)?;
                    Sync2FrameName::OpenNamespace.encode(&mut e)?;
                    if f.protocol_version == 0 {
                        return Err(CodecError::NonCanonical);
                    }
                    encode_u64(&mut e, f.protocol_version)?;
                    if f.session_id.len() > MAX_SESSION_ID_BYTES {
                        return Err(CodecError::LengthOutOfRange);
                    }
                    e.bytes(&f.session_id).map_err(|_| CodecError::Malformed)?;
                    if f.ticket_core_bytes.len() > MAX_TICKET_CORE_BYTES + 128 {
                        return Err(CodecError::LengthOutOfRange);
                    }
                    e.bytes(&f.ticket_core_bytes)
                        .map_err(|_| CodecError::Malformed)?;
                    encode_digest(&mut e, &f.namespace_id)?;
                    f.mode.encode(&mut e)?;
                }
                Sync2Frame::SnapshotStart(f) => {
                    e.array(6).map_err(|_| CodecError::Malformed)?;
                    Sync2FrameName::SnapshotStart.encode(&mut e)?;
                    f.phase.encode(&mut e)?;
                    encode_digest(&mut e, &f.namespace_id)?;
                    encode_digest(&mut e, &f.snapshot_digest)?;
                    encode_u64(&mut e, f.entry_count)?;
                    encode_u64(&mut e, f.logical_bytes)?;
                }
                Sync2Frame::IdsPage(f) => {
                    e.array(6).map_err(|_| CodecError::Malformed)?;
                    Sync2FrameName::IdsPage.encode(&mut e)?;
                    f.phase.encode(&mut e)?;
                    encode_digest(&mut e, &f.snapshot_digest)?;
                    encode_opt_id(&mut e, &f.after_exclusive)?;
                    encode_sorted_ids(&mut e, &f.entry_ids, MAX_IDS_PER_PAGE)?;
                    encode_bool(&mut e, f.done)?;
                }
                Sync2Frame::NeedEntries(f) => {
                    e.array(5).map_err(|_| CodecError::Malformed)?;
                    Sync2FrameName::NeedEntries.encode(&mut e)?;
                    f.phase.encode(&mut e)?;
                    encode_digest(&mut e, &f.page_digest)?;
                    encode_u64(&mut e, f.request_id)?;
                    encode_id_list(&mut e, &f.entry_ids, MAX_IDS_PER_NEED)?;
                }
                Sync2Frame::PageNeedsComplete(f) => {
                    e.array(3).map_err(|_| CodecError::Malformed)?;
                    Sync2FrameName::PageNeedsComplete.encode(&mut e)?;
                    f.phase.encode(&mut e)?;
                    encode_digest(&mut e, &f.page_digest)?;
                }
                Sync2Frame::EntriesChunk(f) => {
                    e.array(7).map_err(|_| CodecError::Malformed)?;
                    Sync2FrameName::EntriesChunk.encode(&mut e)?;
                    f.phase.encode(&mut e)?;
                    encode_digest(&mut e, &f.page_digest)?;
                    encode_u64(&mut e, f.request_id)?;
                    encode_u64(&mut e, f.chunk_index)?;
                    encode_bool(&mut e, f.done)?;
                    if f.bundle_bytes.len() > MAX_CHUNK_BUNDLE_BYTES {
                        return Err(CodecError::LengthOutOfRange);
                    }
                    e.bytes(&f.bundle_bytes)
                        .map_err(|_| CodecError::Malformed)?;
                }
                Sync2Frame::PageComplete(f) => {
                    e.array(3).map_err(|_| CodecError::Malformed)?;
                    Sync2FrameName::PageComplete.encode(&mut e)?;
                    f.phase.encode(&mut e)?;
                    encode_digest(&mut e, &f.page_digest)?;
                }
                Sync2Frame::DirectionComplete(f) => {
                    e.array(3).map_err(|_| CodecError::Malformed)?;
                    Sync2FrameName::DirectionComplete.encode(&mut e)?;
                    f.phase.encode(&mut e)?;
                    encode_digest(&mut e, &f.sender_snapshot_digest)?;
                }
                Sync2Frame::NamespaceComplete(f) => {
                    e.array(3).map_err(|_| CodecError::Malformed)?;
                    Sync2FrameName::NamespaceComplete.encode(&mut e)?;
                    f.mode.encode(&mut e)?;
                    encode_digest(&mut e, &f.final_snapshot_digest)?;
                }
                Sync2Frame::Refuse(refusal) => {
                    e.array(5).map_err(|_| CodecError::Malformed)?;
                    Sync2FrameName::Refuse.encode(&mut e)?;
                    e.str(refusal.code()).map_err(|_| CodecError::Malformed)?;
                    encode_bool(&mut e, refusal.retryable())?;
                    match refusal.retry_after_seconds() {
                        Some(secs) => {
                            if secs == 0 {
                                return Err(CodecError::NonCanonical);
                            }
                            encode_u64(&mut e, secs)?;
                        }
                        None => {
                            e.null().map_err(|_| CodecError::Malformed)?;
                        }
                    }
                    refusal.encode_details(&mut e)?;
                }
            }
        }
        Ok(buf)
    }

    fn decode_fields(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        let count = definite_array(d)?;
        let name = Sync2FrameName::decode(d)?;
        let expect_len = |want: u64| -> Result<(), CodecError> {
            if count == want {
                Ok(())
            } else {
                Err(CodecError::WrongArrayLength {
                    expected: want,
                    actual: count,
                })
            }
        };
        match name {
            Sync2FrameName::OpenNamespace => {
                expect_len(6)?;
                let protocol_version = read_u64(d)?;
                if protocol_version == 0 {
                    return Err(CodecError::NonCanonical);
                }
                let session_id = read_bytes_max(d, MAX_SESSION_ID_BYTES)?;
                let ticket_core_bytes = read_bytes_max(d, MAX_TICKET_CORE_BYTES + 128)?;
                let namespace_id = read_digest(d)?;
                let mode = Sync2Mode::decode(d)?;
                Ok(Sync2Frame::OpenNamespace(OpenNamespace {
                    protocol_version,
                    session_id,
                    ticket_core_bytes,
                    namespace_id,
                    mode,
                }))
            }
            Sync2FrameName::SnapshotStart => {
                expect_len(6)?;
                let phase = Sync2Phase::decode(d)?;
                let namespace_id = read_digest(d)?;
                let snapshot_digest = read_digest(d)?;
                let entry_count = read_u64(d)?;
                let logical_bytes = read_u64(d)?;
                Ok(Sync2Frame::SnapshotStart(SnapshotStart {
                    phase,
                    namespace_id,
                    snapshot_digest,
                    entry_count,
                    logical_bytes,
                }))
            }
            Sync2FrameName::IdsPage => {
                expect_len(6)?;
                let phase = Sync2Phase::decode(d)?;
                let snapshot_digest = read_digest(d)?;
                let after_exclusive = read_opt_id(d)?;
                let entry_ids = read_sorted_ids(d, MAX_IDS_PER_PAGE)?;
                let done = read_bool(d)?;
                Ok(Sync2Frame::IdsPage(IdsPage {
                    phase,
                    snapshot_digest,
                    after_exclusive,
                    entry_ids,
                    done,
                }))
            }
            Sync2FrameName::NeedEntries => {
                expect_len(5)?;
                let phase = Sync2Phase::decode(d)?;
                let page_digest = read_digest(d)?;
                let request_id = read_u64(d)?;
                let entry_ids = read_id_list(d, MAX_IDS_PER_NEED)?;
                Ok(Sync2Frame::NeedEntries(NeedEntries {
                    phase,
                    page_digest,
                    request_id,
                    entry_ids,
                }))
            }
            Sync2FrameName::PageNeedsComplete => {
                expect_len(3)?;
                let phase = Sync2Phase::decode(d)?;
                let page_digest = read_digest(d)?;
                Ok(Sync2Frame::PageNeedsComplete(PageNeedsComplete {
                    phase,
                    page_digest,
                }))
            }
            Sync2FrameName::EntriesChunk => {
                expect_len(7)?;
                let phase = Sync2Phase::decode(d)?;
                let page_digest = read_digest(d)?;
                let request_id = read_u64(d)?;
                let chunk_index = read_u64(d)?;
                let done = read_bool(d)?;
                let bundle_bytes = read_bytes_max(d, MAX_CHUNK_BUNDLE_BYTES)?;
                Ok(Sync2Frame::EntriesChunk(EntriesChunk {
                    phase,
                    page_digest,
                    request_id,
                    chunk_index,
                    done,
                    bundle_bytes,
                }))
            }
            Sync2FrameName::PageComplete => {
                expect_len(3)?;
                let phase = Sync2Phase::decode(d)?;
                let page_digest = read_digest(d)?;
                Ok(Sync2Frame::PageComplete(PageComplete {
                    phase,
                    page_digest,
                }))
            }
            Sync2FrameName::DirectionComplete => {
                expect_len(3)?;
                let phase = Sync2Phase::decode(d)?;
                let sender_snapshot_digest = read_digest(d)?;
                Ok(Sync2Frame::DirectionComplete(DirectionComplete {
                    phase,
                    sender_snapshot_digest,
                }))
            }
            Sync2FrameName::NamespaceComplete => {
                expect_len(3)?;
                let mode = Sync2ModeTag::decode(d)?;
                let final_snapshot_digest = read_digest(d)?;
                Ok(Sync2Frame::NamespaceComplete(NamespaceComplete {
                    mode,
                    final_snapshot_digest,
                }))
            }
            Sync2FrameName::Refuse => {
                expect_len(5)?;
                Ok(Sync2Frame::Refuse(Sync2Refusal::decode_fields(d)?))
            }
        }
    }
}

fn encode_sorted_ids(
    e: &mut Encoder<&mut Vec<u8>>,
    ids: &[Vec<u8>],
    max: usize,
) -> Result<(), CodecError> {
    if ids.len() > max {
        return Err(CodecError::LengthOutOfRange);
    }
    // Entry IDs on a page are a strictly ascending set.
    for pair in ids.windows(2) {
        match pair[0].as_slice().cmp(pair[1].as_slice()) {
            core::cmp::Ordering::Less => {}
            core::cmp::Ordering::Equal => return Err(CodecError::DuplicateSetMember),
            core::cmp::Ordering::Greater => return Err(CodecError::UnsortedSet),
        }
    }
    e.array(ids.len() as u64)
        .map_err(|_| CodecError::Malformed)?;
    for id in ids {
        if id.len() > MAX_ENTRY_ID_BYTES {
            return Err(CodecError::LengthOutOfRange);
        }
        e.bytes(id).map_err(|_| CodecError::Malformed)?;
    }
    Ok(())
}

fn read_sorted_ids(d: &mut Decoder<'_>, max: usize) -> Result<Vec<Vec<u8>>, CodecError> {
    let count = definite_array(d)?;
    if count as usize > max {
        return Err(CodecError::LengthOutOfRange);
    }
    let mut ids: Vec<Vec<u8>> = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let id = read_bytes_max(d, MAX_ENTRY_ID_BYTES)?;
        if let Some(prev) = ids.last() {
            match prev.as_slice().cmp(id.as_slice()) {
                core::cmp::Ordering::Less => {}
                core::cmp::Ordering::Equal => return Err(CodecError::DuplicateSetMember),
                core::cmp::Ordering::Greater => return Err(CodecError::UnsortedSet),
            }
        }
        ids.push(id);
    }
    Ok(ids)
}

/// The page digest over a complete canonical `IdsPage` frame:
/// `digest_v1("riot/sync-ids-page/v2", canonical_bytes(IdsPage frame))`.
pub fn ids_page_digest(page: &IdsPage) -> [u8; 32] {
    let bytes = Sync2Frame::IdsPage(page.clone())
        .encode_canonical()
        .expect("canonical IdsPage encodes");
    digest_v1(label::SYNC_IDS_PAGE, &bytes)
}

// ---------------------------------------------------------------------------
// The closed sync refusal matrix. `Refuse { code, retryable, retry_after, details }`.
// Only `busy` is retryable (with a nonzero delay); every other code is terminal
// with `retry_after_seconds = null`. Note the sync refuse frame has NO subject
// column (unlike `ControlRefusal`).
// ---------------------------------------------------------------------------

/// A closed `sync/2` refusal. Each variant is exactly one design matrix row; the
/// retryability and retry-after nullness are a closed function of the variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sync2Refusal {
    /// `unsupported_version` — `["version", supported_versions]`.
    UnsupportedVersion {
        /// The supported sync versions (ascending set).
        supported_versions: Vec<u64>,
    },
    /// `invalid_ticket` — `["ticket", reason]`.
    InvalidTicket {
        /// The `signature | root | structure` reason.
        reason: TicketReason,
    },
    /// `expired_ticket` — `["ticket_expiry", expires_at, observed_at]`.
    ExpiredTicket {
        /// Ticket expiry (unix seconds).
        expires_at: u64,
        /// Observed time (unix seconds).
        observed_at: u64,
    },
    /// `transport_mismatch` — `["transport", required_mode, observed_mode]`.
    TransportMismatch {
        /// The mode the site requires.
        required_mode: TransportMode,
        /// The mode observed.
        observed_mode: TransportMode,
    },
    /// `namespace_not_member` — `["namespace", namespace_id]`.
    NamespaceNotMember {
        /// The namespace ID.
        namespace_id: [u8; 32],
    },
    /// `manifest_mismatch` — `["manifest", expected_digest, observed_digest]`.
    ManifestMismatch {
        /// Expected manifest digest.
        expected_digest: [u8; 32],
        /// Observed manifest digest.
        observed_digest: [u8; 32],
    },
    /// `invalid_mode` — `["mode", observed_mode]`.
    InvalidMode {
        /// The observed mode tag.
        observed_mode: Sync2ModeTag,
    },
    /// `operation_not_found` — `["operation", operation_id]`.
    OperationNotFound {
        /// The operation ID.
        operation_id: [u8; 32],
    },
    /// `invalid_namespace_token` — `["namespace_token", namespace_id]`.
    InvalidNamespaceToken {
        /// The namespace ID.
        namespace_id: [u8; 32],
    },
    /// `operation_expired` — `["operation_expiry", operation_id, expires_at, observed_at]`.
    OperationExpired {
        /// The operation ID.
        operation_id: [u8; 32],
        /// Operation expiry (unix seconds).
        expires_at: u64,
        /// Observed time (unix seconds).
        observed_at: u64,
    },
    /// `unexpected_frame` — `["frame", phase, expected_frame_names, observed_frame_name]`.
    UnexpectedFrame {
        /// The current phase.
        phase: Sync2Phase,
        /// The frames legal in this state (ascending set of frame names).
        expected_frame_names: Vec<Sync2FrameName>,
        /// The frame that actually arrived.
        observed_frame_name: Sync2FrameName,
    },
    /// `cursor_regression` — `["cursor", after_exclusive, observed_first_id]`.
    CursorRegression {
        /// The prior page's exclusive cursor (or `None` first page).
        after_exclusive: Option<Vec<u8>>,
        /// The offending first ID observed.
        observed_first_id: Vec<u8>,
    },
    /// `page_mismatch` — `["page", expected_page_digest, observed_page_digest]`.
    PageMismatch {
        /// Expected page digest.
        expected_page_digest: [u8; 32],
        /// Observed page digest.
        observed_page_digest: [u8; 32],
    },
    /// `snapshot_mismatch` — `["snapshot", expected_snapshot_digest, observed_snapshot_digest]`.
    SnapshotMismatch {
        /// Expected snapshot digest.
        expected_snapshot_digest: [u8; 32],
        /// Observed snapshot digest.
        observed_snapshot_digest: [u8; 32],
    },
    /// `stale_source` — `["source_state", attested_generation, observed_generation,
    /// ordered_observed_namespace_snapshot_digests]`.
    StaleSource {
        /// The attested source generation.
        attested_generation: u64,
        /// The observed source generation.
        observed_generation: u64,
        /// The ordered observed O/C/W namespace snapshot digests.
        observed_namespace_snapshot_digests: Vec<[u8; 32]>,
    },
    /// `request_mismatch` — `["request", request_id]`.
    RequestMismatch {
        /// The offending request ID.
        request_id: u64,
    },
    /// `chunk_mismatch` — `["chunk", request_id, expected_index, observed_index]`.
    ChunkMismatch {
        /// The request ID.
        request_id: u64,
        /// The expected chunk index.
        expected_index: u64,
        /// The observed chunk index.
        observed_index: u64,
    },
    /// `frame_oversize` — `["encoded_size", observed_bytes, maximum_bytes]`.
    FrameOversize {
        /// The observed encoded size.
        observed_bytes: u64,
        /// The maximum allowed size.
        maximum_bytes: u64,
    },
    /// `admission_failed` — `["admission", subject]`.
    AdmissionFailed {
        /// The `authority | bundle | entry` subject.
        subject: AdmissionSubject,
    },
    /// `quota_exceeded` — `["quota", limit_id, effective_value, observed_value]`.
    QuotaExceeded {
        /// The limit that was exceeded.
        limit_id: AnchorLimitId,
        /// The effective ceiling.
        effective_value: LimitValue,
        /// The observed value.
        observed_value: LimitValue,
    },
    /// `busy` — `["capacity", limit_id]`. The only retryable sync refusal.
    Busy {
        /// The saturated limit.
        limit_id: AnchorLimitId,
        /// The required nonzero retry delay.
        retry_after_seconds: u64,
    },
    /// `peer_context_changed` — `["peer_context", side, prior_descriptor_digest,
    /// optional_latest_descriptor_digest, reason]`.
    PeerContextChanged {
        /// Which side changed.
        side: PeerSide,
        /// The prior descriptor digest.
        prior_descriptor_digest: [u8; 32],
        /// The latest descriptor digest, if known.
        latest_descriptor_digest: Option<[u8; 32]>,
        /// The reason.
        reason: PeerContextReason,
    },
}

impl Sync2Refusal {
    /// The exact lowercase textual refusal code.
    pub fn code(&self) -> &'static str {
        match self {
            Sync2Refusal::UnsupportedVersion { .. } => "unsupported_version",
            Sync2Refusal::InvalidTicket { .. } => "invalid_ticket",
            Sync2Refusal::ExpiredTicket { .. } => "expired_ticket",
            Sync2Refusal::TransportMismatch { .. } => "transport_mismatch",
            Sync2Refusal::NamespaceNotMember { .. } => "namespace_not_member",
            Sync2Refusal::ManifestMismatch { .. } => "manifest_mismatch",
            Sync2Refusal::InvalidMode { .. } => "invalid_mode",
            Sync2Refusal::OperationNotFound { .. } => "operation_not_found",
            Sync2Refusal::InvalidNamespaceToken { .. } => "invalid_namespace_token",
            Sync2Refusal::OperationExpired { .. } => "operation_expired",
            Sync2Refusal::UnexpectedFrame { .. } => "unexpected_frame",
            Sync2Refusal::CursorRegression { .. } => "cursor_regression",
            Sync2Refusal::PageMismatch { .. } => "page_mismatch",
            Sync2Refusal::SnapshotMismatch { .. } => "snapshot_mismatch",
            Sync2Refusal::StaleSource { .. } => "stale_source",
            Sync2Refusal::RequestMismatch { .. } => "request_mismatch",
            Sync2Refusal::ChunkMismatch { .. } => "chunk_mismatch",
            Sync2Refusal::FrameOversize { .. } => "frame_oversize",
            Sync2Refusal::AdmissionFailed { .. } => "admission_failed",
            Sync2Refusal::QuotaExceeded { .. } => "quota_exceeded",
            Sync2Refusal::Busy { .. } => "busy",
            Sync2Refusal::PeerContextChanged { .. } => "peer_context_changed",
        }
    }

    /// Whether the refusal is retryable. Only `busy` is.
    pub fn retryable(&self) -> bool {
        matches!(self, Sync2Refusal::Busy { .. })
    }

    /// The required nonzero retry delay (only `busy`).
    pub fn retry_after_seconds(&self) -> Option<u64> {
        match self {
            Sync2Refusal::Busy {
                retry_after_seconds,
                ..
            } => Some(*retry_after_seconds),
            _ => None,
        }
    }

    fn encode_details(&self, e: &mut Encoder<&mut Vec<u8>>) -> Result<(), CodecError> {
        match self {
            Sync2Refusal::UnsupportedVersion { supported_versions } => {
                e.array(2).map_err(|_| CodecError::Malformed)?;
                e.str("version").map_err(|_| CodecError::Malformed)?;
                if supported_versions.len() > MAX_INLINE_VECTOR {
                    return Err(CodecError::LengthOutOfRange);
                }
                e.array(supported_versions.len() as u64)
                    .map_err(|_| CodecError::Malformed)?;
                let mut prev: Option<u64> = None;
                for v in supported_versions {
                    if let Some(p) = prev {
                        if *v <= p {
                            return Err(CodecError::UnsortedSet);
                        }
                    }
                    prev = Some(*v);
                    encode_u64(e, *v)?;
                }
            }
            Sync2Refusal::InvalidTicket { reason } => {
                e.array(2).map_err(|_| CodecError::Malformed)?;
                e.str("ticket").map_err(|_| CodecError::Malformed)?;
                e.str(ticket_reason_token(*reason))
                    .map_err(|_| CodecError::Malformed)?;
            }
            Sync2Refusal::ExpiredTicket {
                expires_at,
                observed_at,
            } => {
                e.array(3).map_err(|_| CodecError::Malformed)?;
                e.str("ticket_expiry").map_err(|_| CodecError::Malformed)?;
                encode_u64(e, *expires_at)?;
                encode_u64(e, *observed_at)?;
            }
            Sync2Refusal::TransportMismatch {
                required_mode,
                observed_mode,
            } => {
                e.array(3).map_err(|_| CodecError::Malformed)?;
                e.str("transport").map_err(|_| CodecError::Malformed)?;
                required_mode.encode(e)?;
                observed_mode.encode(e)?;
            }
            Sync2Refusal::NamespaceNotMember { namespace_id } => {
                e.array(2).map_err(|_| CodecError::Malformed)?;
                e.str("namespace").map_err(|_| CodecError::Malformed)?;
                encode_digest(e, namespace_id)?;
            }
            Sync2Refusal::ManifestMismatch {
                expected_digest,
                observed_digest,
            } => {
                e.array(3).map_err(|_| CodecError::Malformed)?;
                e.str("manifest").map_err(|_| CodecError::Malformed)?;
                encode_digest(e, expected_digest)?;
                encode_digest(e, observed_digest)?;
            }
            Sync2Refusal::InvalidMode { observed_mode } => {
                e.array(2).map_err(|_| CodecError::Malformed)?;
                e.str("mode").map_err(|_| CodecError::Malformed)?;
                observed_mode.encode(e)?;
            }
            Sync2Refusal::OperationNotFound { operation_id } => {
                e.array(2).map_err(|_| CodecError::Malformed)?;
                e.str("operation").map_err(|_| CodecError::Malformed)?;
                encode_digest(e, operation_id)?;
            }
            Sync2Refusal::InvalidNamespaceToken { namespace_id } => {
                e.array(2).map_err(|_| CodecError::Malformed)?;
                e.str("namespace_token")
                    .map_err(|_| CodecError::Malformed)?;
                encode_digest(e, namespace_id)?;
            }
            Sync2Refusal::OperationExpired {
                operation_id,
                expires_at,
                observed_at,
            } => {
                e.array(4).map_err(|_| CodecError::Malformed)?;
                e.str("operation_expiry")
                    .map_err(|_| CodecError::Malformed)?;
                encode_digest(e, operation_id)?;
                encode_u64(e, *expires_at)?;
                encode_u64(e, *observed_at)?;
            }
            Sync2Refusal::UnexpectedFrame {
                phase,
                expected_frame_names,
                observed_frame_name,
            } => {
                e.array(4).map_err(|_| CodecError::Malformed)?;
                e.str("frame").map_err(|_| CodecError::Malformed)?;
                phase.encode(e)?;
                encode_frame_name_set(e, expected_frame_names)?;
                observed_frame_name.encode(e)?;
            }
            Sync2Refusal::CursorRegression {
                after_exclusive,
                observed_first_id,
            } => {
                e.array(3).map_err(|_| CodecError::Malformed)?;
                e.str("cursor").map_err(|_| CodecError::Malformed)?;
                encode_opt_id(e, after_exclusive)?;
                if observed_first_id.len() > MAX_ENTRY_ID_BYTES {
                    return Err(CodecError::LengthOutOfRange);
                }
                e.bytes(observed_first_id)
                    .map_err(|_| CodecError::Malformed)?;
            }
            Sync2Refusal::PageMismatch {
                expected_page_digest,
                observed_page_digest,
            } => {
                e.array(3).map_err(|_| CodecError::Malformed)?;
                e.str("page").map_err(|_| CodecError::Malformed)?;
                encode_digest(e, expected_page_digest)?;
                encode_digest(e, observed_page_digest)?;
            }
            Sync2Refusal::SnapshotMismatch {
                expected_snapshot_digest,
                observed_snapshot_digest,
            } => {
                e.array(3).map_err(|_| CodecError::Malformed)?;
                e.str("snapshot").map_err(|_| CodecError::Malformed)?;
                encode_digest(e, expected_snapshot_digest)?;
                encode_digest(e, observed_snapshot_digest)?;
            }
            Sync2Refusal::StaleSource {
                attested_generation,
                observed_generation,
                observed_namespace_snapshot_digests,
            } => {
                e.array(4).map_err(|_| CodecError::Malformed)?;
                e.str("source_state").map_err(|_| CodecError::Malformed)?;
                encode_u64(e, *attested_generation)?;
                encode_u64(e, *observed_generation)?;
                if observed_namespace_snapshot_digests.len() > MAX_INLINE_VECTOR {
                    return Err(CodecError::LengthOutOfRange);
                }
                e.array(observed_namespace_snapshot_digests.len() as u64)
                    .map_err(|_| CodecError::Malformed)?;
                for digest in observed_namespace_snapshot_digests {
                    encode_digest(e, digest)?;
                }
            }
            Sync2Refusal::RequestMismatch { request_id } => {
                e.array(2).map_err(|_| CodecError::Malformed)?;
                e.str("request").map_err(|_| CodecError::Malformed)?;
                encode_u64(e, *request_id)?;
            }
            Sync2Refusal::ChunkMismatch {
                request_id,
                expected_index,
                observed_index,
            } => {
                e.array(4).map_err(|_| CodecError::Malformed)?;
                e.str("chunk").map_err(|_| CodecError::Malformed)?;
                encode_u64(e, *request_id)?;
                encode_u64(e, *expected_index)?;
                encode_u64(e, *observed_index)?;
            }
            Sync2Refusal::FrameOversize {
                observed_bytes,
                maximum_bytes,
            } => {
                e.array(3).map_err(|_| CodecError::Malformed)?;
                e.str("encoded_size").map_err(|_| CodecError::Malformed)?;
                encode_u64(e, *observed_bytes)?;
                encode_u64(e, *maximum_bytes)?;
            }
            Sync2Refusal::AdmissionFailed { subject } => {
                e.array(2).map_err(|_| CodecError::Malformed)?;
                e.str("admission").map_err(|_| CodecError::Malformed)?;
                subject.encode(e)?;
            }
            Sync2Refusal::QuotaExceeded {
                limit_id,
                effective_value,
                observed_value,
            } => {
                e.array(4).map_err(|_| CodecError::Malformed)?;
                e.str("quota").map_err(|_| CodecError::Malformed)?;
                encode_limit_id(e, *limit_id)?;
                effective_value.encode(e)?;
                observed_value.encode(e)?;
            }
            Sync2Refusal::Busy { limit_id, .. } => {
                e.array(2).map_err(|_| CodecError::Malformed)?;
                e.str("capacity").map_err(|_| CodecError::Malformed)?;
                encode_limit_id(e, *limit_id)?;
            }
            Sync2Refusal::PeerContextChanged {
                side,
                prior_descriptor_digest,
                latest_descriptor_digest,
                reason,
            } => {
                e.array(5).map_err(|_| CodecError::Malformed)?;
                e.str("peer_context").map_err(|_| CodecError::Malformed)?;
                side.encode(e)?;
                encode_digest(e, prior_descriptor_digest)?;
                match latest_descriptor_digest {
                    Some(d) => encode_digest(e, d)?,
                    None => {
                        e.null().map_err(|_| CodecError::Malformed)?;
                    }
                }
                reason.encode(e)?;
            }
        }
        Ok(())
    }

    // Decode the 4 trailing slots `[retryable, retry_after, details]` after the
    // code has been read, fixing retryability and details shape per the code.
    fn decode_fields(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        let code = read_discriminant(d, MAX_TOKEN_BYTES)?;
        match code.as_str() {
            "unsupported_version" => {
                expect_bool(d, false)?;
                read_retry_null(d)?;
                expect_array(d, 2)?;
                expect_token(d, "version")?;
                let count = definite_array(d)?;
                if count as usize > MAX_INLINE_VECTOR {
                    return Err(CodecError::LengthOutOfRange);
                }
                let mut supported_versions = Vec::with_capacity(count as usize);
                let mut prev: Option<u64> = None;
                for _ in 0..count {
                    let v = read_u64(d)?;
                    if let Some(p) = prev {
                        if v <= p {
                            return Err(CodecError::UnsortedSet);
                        }
                    }
                    prev = Some(v);
                    supported_versions.push(v);
                }
                Ok(Sync2Refusal::UnsupportedVersion { supported_versions })
            }
            "invalid_ticket" => {
                expect_bool(d, false)?;
                read_retry_null(d)?;
                expect_array(d, 2)?;
                expect_token(d, "ticket")?;
                let token = read_discriminant(d, MAX_TOKEN_BYTES)?;
                let reason = ticket_reason_from_token(&token).ok_or(CodecError::UnknownVariant)?;
                Ok(Sync2Refusal::InvalidTicket { reason })
            }
            "expired_ticket" => {
                expect_bool(d, false)?;
                read_retry_null(d)?;
                expect_array(d, 3)?;
                expect_token(d, "ticket_expiry")?;
                let expires_at = read_u64(d)?;
                let observed_at = read_u64(d)?;
                Ok(Sync2Refusal::ExpiredTicket {
                    expires_at,
                    observed_at,
                })
            }
            "transport_mismatch" => {
                expect_bool(d, false)?;
                read_retry_null(d)?;
                expect_array(d, 3)?;
                expect_token(d, "transport")?;
                let required_mode = TransportMode::decode(d)?;
                let observed_mode = TransportMode::decode(d)?;
                Ok(Sync2Refusal::TransportMismatch {
                    required_mode,
                    observed_mode,
                })
            }
            "namespace_not_member" => {
                expect_bool(d, false)?;
                read_retry_null(d)?;
                expect_array(d, 2)?;
                expect_token(d, "namespace")?;
                let namespace_id = read_digest(d)?;
                Ok(Sync2Refusal::NamespaceNotMember { namespace_id })
            }
            "manifest_mismatch" => {
                expect_bool(d, false)?;
                read_retry_null(d)?;
                expect_array(d, 3)?;
                expect_token(d, "manifest")?;
                let expected_digest = read_digest(d)?;
                let observed_digest = read_digest(d)?;
                Ok(Sync2Refusal::ManifestMismatch {
                    expected_digest,
                    observed_digest,
                })
            }
            "invalid_mode" => {
                expect_bool(d, false)?;
                read_retry_null(d)?;
                expect_array(d, 2)?;
                expect_token(d, "mode")?;
                let observed_mode = Sync2ModeTag::decode(d)?;
                Ok(Sync2Refusal::InvalidMode { observed_mode })
            }
            "operation_not_found" => {
                expect_bool(d, false)?;
                read_retry_null(d)?;
                expect_array(d, 2)?;
                expect_token(d, "operation")?;
                let operation_id = read_digest(d)?;
                Ok(Sync2Refusal::OperationNotFound { operation_id })
            }
            "invalid_namespace_token" => {
                expect_bool(d, false)?;
                read_retry_null(d)?;
                expect_array(d, 2)?;
                expect_token(d, "namespace_token")?;
                let namespace_id = read_digest(d)?;
                Ok(Sync2Refusal::InvalidNamespaceToken { namespace_id })
            }
            "operation_expired" => {
                expect_bool(d, false)?;
                read_retry_null(d)?;
                expect_array(d, 4)?;
                expect_token(d, "operation_expiry")?;
                let operation_id = read_digest(d)?;
                let expires_at = read_u64(d)?;
                let observed_at = read_u64(d)?;
                Ok(Sync2Refusal::OperationExpired {
                    operation_id,
                    expires_at,
                    observed_at,
                })
            }
            "unexpected_frame" => {
                expect_bool(d, false)?;
                read_retry_null(d)?;
                expect_array(d, 4)?;
                expect_token(d, "frame")?;
                let phase = Sync2Phase::decode(d)?;
                let expected_frame_names = decode_frame_name_set(d)?;
                let observed_frame_name = Sync2FrameName::decode(d)?;
                Ok(Sync2Refusal::UnexpectedFrame {
                    phase,
                    expected_frame_names,
                    observed_frame_name,
                })
            }
            "cursor_regression" => {
                expect_bool(d, false)?;
                read_retry_null(d)?;
                expect_array(d, 3)?;
                expect_token(d, "cursor")?;
                let after_exclusive = read_opt_id(d)?;
                let observed_first_id = read_bytes_max(d, MAX_ENTRY_ID_BYTES)?;
                Ok(Sync2Refusal::CursorRegression {
                    after_exclusive,
                    observed_first_id,
                })
            }
            "page_mismatch" => {
                expect_bool(d, false)?;
                read_retry_null(d)?;
                expect_array(d, 3)?;
                expect_token(d, "page")?;
                let expected_page_digest = read_digest(d)?;
                let observed_page_digest = read_digest(d)?;
                Ok(Sync2Refusal::PageMismatch {
                    expected_page_digest,
                    observed_page_digest,
                })
            }
            "snapshot_mismatch" => {
                expect_bool(d, false)?;
                read_retry_null(d)?;
                expect_array(d, 3)?;
                expect_token(d, "snapshot")?;
                let expected_snapshot_digest = read_digest(d)?;
                let observed_snapshot_digest = read_digest(d)?;
                Ok(Sync2Refusal::SnapshotMismatch {
                    expected_snapshot_digest,
                    observed_snapshot_digest,
                })
            }
            "stale_source" => {
                expect_bool(d, false)?;
                read_retry_null(d)?;
                expect_array(d, 4)?;
                expect_token(d, "source_state")?;
                let attested_generation = read_u64(d)?;
                let observed_generation = read_u64(d)?;
                let count = definite_array(d)?;
                if count as usize > MAX_INLINE_VECTOR {
                    return Err(CodecError::LengthOutOfRange);
                }
                let mut observed_namespace_snapshot_digests = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    observed_namespace_snapshot_digests.push(read_digest(d)?);
                }
                Ok(Sync2Refusal::StaleSource {
                    attested_generation,
                    observed_generation,
                    observed_namespace_snapshot_digests,
                })
            }
            "request_mismatch" => {
                expect_bool(d, false)?;
                read_retry_null(d)?;
                expect_array(d, 2)?;
                expect_token(d, "request")?;
                let request_id = read_u64(d)?;
                Ok(Sync2Refusal::RequestMismatch { request_id })
            }
            "chunk_mismatch" => {
                expect_bool(d, false)?;
                read_retry_null(d)?;
                expect_array(d, 4)?;
                expect_token(d, "chunk")?;
                let request_id = read_u64(d)?;
                let expected_index = read_u64(d)?;
                let observed_index = read_u64(d)?;
                Ok(Sync2Refusal::ChunkMismatch {
                    request_id,
                    expected_index,
                    observed_index,
                })
            }
            "frame_oversize" => {
                expect_bool(d, false)?;
                read_retry_null(d)?;
                expect_array(d, 3)?;
                expect_token(d, "encoded_size")?;
                let observed_bytes = read_u64(d)?;
                let maximum_bytes = read_u64(d)?;
                Ok(Sync2Refusal::FrameOversize {
                    observed_bytes,
                    maximum_bytes,
                })
            }
            "admission_failed" => {
                expect_bool(d, false)?;
                read_retry_null(d)?;
                expect_array(d, 2)?;
                expect_token(d, "admission")?;
                let subject = AdmissionSubject::decode(d)?;
                Ok(Sync2Refusal::AdmissionFailed { subject })
            }
            "quota_exceeded" => {
                expect_bool(d, false)?;
                read_retry_null(d)?;
                expect_array(d, 4)?;
                expect_token(d, "quota")?;
                let limit_id = decode_limit_id(d)?;
                let effective_value = LimitValue::decode(d)?;
                let observed_value = LimitValue::decode(d)?;
                Ok(Sync2Refusal::QuotaExceeded {
                    limit_id,
                    effective_value,
                    observed_value,
                })
            }
            "busy" => {
                expect_bool(d, true)?;
                let retry_after_seconds = read_retry_required(d)?;
                expect_array(d, 2)?;
                expect_token(d, "capacity")?;
                let limit_id = decode_limit_id(d)?;
                Ok(Sync2Refusal::Busy {
                    limit_id,
                    retry_after_seconds,
                })
            }
            "peer_context_changed" => {
                expect_bool(d, false)?;
                read_retry_null(d)?;
                expect_array(d, 5)?;
                expect_token(d, "peer_context")?;
                let side = PeerSide::decode(d)?;
                let prior_descriptor_digest = read_digest(d)?;
                let latest_descriptor_digest = if peek_null(d)? {
                    read_null(d)?;
                    None
                } else {
                    Some(read_digest(d)?)
                };
                let reason = PeerContextReason::decode(d)?;
                Ok(Sync2Refusal::PeerContextChanged {
                    side,
                    prior_descriptor_digest,
                    latest_descriptor_digest,
                    reason,
                })
            }
            _ => Err(CodecError::UnknownVariant),
        }
    }
}

fn encode_frame_name_set(
    e: &mut Encoder<&mut Vec<u8>>,
    names: &[Sync2FrameName],
) -> Result<(), CodecError> {
    // The expected-frame set is canonical: sorted ascending by token bytes, no
    // duplicates.
    let mut tokens: Vec<&'static str> = names.iter().map(|n| n.token()).collect();
    tokens.sort_unstable();
    for pair in tokens.windows(2) {
        if pair[0] == pair[1] {
            return Err(CodecError::DuplicateSetMember);
        }
    }
    e.array(tokens.len() as u64)
        .map_err(|_| CodecError::Malformed)?;
    for token in tokens {
        e.str(token).map_err(|_| CodecError::Malformed)?;
    }
    Ok(())
}

fn decode_frame_name_set(d: &mut Decoder<'_>) -> Result<Vec<Sync2FrameName>, CodecError> {
    let count = definite_array(d)?;
    if count as usize > 10 {
        return Err(CodecError::LengthOutOfRange);
    }
    let mut names = Vec::with_capacity(count as usize);
    let mut prev: Option<String> = None;
    for _ in 0..count {
        let token = read_discriminant(d, MAX_TOKEN_BYTES)?;
        if let Some(p) = &prev {
            match p.as_str().cmp(token.as_str()) {
                core::cmp::Ordering::Less => {}
                core::cmp::Ordering::Equal => return Err(CodecError::DuplicateSetMember),
                core::cmp::Ordering::Greater => return Err(CodecError::UnsortedSet),
            }
        }
        let name = Sync2FrameName::from_token(&token).ok_or(CodecError::UnknownVariant)?;
        prev = Some(token);
        names.push(name);
    }
    Ok(names)
}

// ---------------------------------------------------------------------------
// The bundle payload codec: a canonical CBOR array of per-item byte strings.
// ---------------------------------------------------------------------------

/// Encode a chunk bundle as a canonical CBOR array of per-item byte strings.
pub fn encode_bundle(items: &[Vec<u8>]) -> Result<Vec<u8>, CodecError> {
    if items.len() > MAX_ENTRIES_PER_CHUNK {
        return Err(CodecError::LengthOutOfRange);
    }
    let mut buf = Vec::new();
    {
        let mut e = Encoder::new(&mut buf);
        e.array(items.len() as u64)
            .map_err(|_| CodecError::Malformed)?;
        for item in items {
            if item.len() > MAX_ANCHOR_ITEM_BYTES {
                return Err(CodecError::LengthOutOfRange);
            }
            e.bytes(item).map_err(|_| CodecError::Malformed)?;
        }
    }
    Ok(buf)
}

/// Decode a chunk bundle produced by [`encode_bundle`].
pub fn decode_bundle(bytes: &[u8]) -> Result<Vec<Vec<u8>>, CodecError> {
    if bytes.len() > MAX_CHUNK_BUNDLE_BYTES {
        return Err(CodecError::TooLarge {
            limit: MAX_CHUNK_BUNDLE_BYTES,
            actual: bytes.len(),
        });
    }
    let mut d = Decoder::new(bytes);
    let count = definite_array(&mut d)?;
    if count as usize > MAX_ENTRIES_PER_CHUNK {
        return Err(CodecError::LengthOutOfRange);
    }
    let mut items = Vec::with_capacity(count as usize);
    for _ in 0..count {
        items.push(read_bytes_max(&mut d, MAX_ANCHOR_ITEM_BYTES)?);
    }
    if d.position() != bytes.len() {
        return Err(CodecError::TrailingBytes);
    }
    Ok(items)
}

// ---------------------------------------------------------------------------
// The transport-independent repository + FSM.
// ---------------------------------------------------------------------------

mod fsm;
pub use fsm::{
    OpenedNamespace, PhaseParty, PhasePlan, Sync2Action, Sync2DirectionStage, Sync2Repository,
    Sync2Session, Sync2Snapshot,
};

// Re-export the snapshot digest for callers building `Sync2Snapshot` impls.
pub use crate::digest::sync_snapshot_digest as snapshot_digest_of;

/// Compute the immutable snapshot digest for a set of full entry IDs; the IDs are
/// sorted lexicographically first (design "sorts full canonical entry-ID bytes
/// lexicographically").
pub fn compute_snapshot_digest(
    namespace_id: &[u8; 32],
    logical_bytes: u64,
    entry_ids: &[Vec<u8>],
) -> [u8; 32] {
    let mut sorted: Vec<&[u8]> = entry_ids.iter().map(|v| v.as_slice()).collect();
    sorted.sort_unstable();
    sync_snapshot_digest(namespace_id, sorted.len() as u64, logical_bytes, &sorted)
}
