//! The `sync/2` anchor adapter: [`Sync2Repository`] backed by
//! [`AnchorRepository`] direction-private staging.
//!
//! The responder routes an inbound `OpenNamespace` through
//! [`AnchorSyncRepository::open_namespace`], which verifies the operation is
//! actively prepared and unexpired and that the presented per-namespace token
//! deterministically re-derives, then returns this endpoint's phase plan. A host
//! reconciliation streams the anchor's committed base ([`AnchorSnapshot`]) to the
//! client, then admits the client's entries into an operation-private stage
//! ([`AnchorStage`]).
//!
//! # The anchor is the trust root
//!
//! Every entry admitted into staging passes riot-core's REAL Meadowcap +
//! signature check ([`verify_anchor_item`] → [`riot_core::willow::verify_entry`])
//! plus anchor-profile byte bounds. An item that fails is refused
//! (`AdmissionSubject::Entry`) and never enters staging — the anchor never
//! constructs an admitted record from unverified client bytes. The composite
//! Commit ([`crate::hosting`]) re-verifies every staged entry before promotion as
//! defense in depth.

use std::cell::RefCell;
use std::rc::Rc;

use riot_anchor_protocol::codec::decode_canonical;
use riot_anchor_protocol::control::{
    ControlOutcome, ControlResponseV1, ControlSuccess, PrepareSuccessV1, MAX_CONTROL_FRAME_BYTES,
};
use riot_anchor_protocol::sync2::{
    compute_snapshot_digest, AdmissionSubject, OpenNamespace, OpenedNamespace, PhaseParty,
    Sync2DirectionStage, Sync2Mode, Sync2ModeTag, Sync2Phase, Sync2Refusal, Sync2Repository,
    Sync2Snapshot,
};

use riot_core::willow::{
    decode_capability_canonic, decode_entry_canonic, entry_id as canonical_entry_id, verify_entry,
    william3_digest, AuthorisationToken,
};
use willow25::entry::{Entrylike, SubspaceSignature};
use willow25::groupings::{Coordinatelike, Keylike, Namespaced};

use crate::repository::{AnchorRepository, OperationStatus, StagedEntry};
use crate::work::TokenSecretRing;

/// The anchor-profile encoded item ceiling (design: at most 2 MiB including
/// proofs, so at least one always fits a chunk).
pub const MAX_ANCHOR_ITEM_BYTES: usize = 2 * 1024 * 1024;
/// The per-item payload ceiling (design: 1 MiB).
pub const MAX_ITEM_PAYLOAD_BYTES: usize = 1024 * 1024;
/// The anchor-profile item wire version.
const ITEM_VERSION: u8 = 1;
const SIGNATURE_BYTES: usize = 64;

/// Why a candidate anchor-profile item was refused. Every variant maps to a
/// `sync/2` admission subject: a structural framing failure is `Bundle`; a
/// verification / bounds failure is `Entry`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemReject {
    /// The item envelope did not decode (framing/bounds/trailing bytes).
    Malformed,
    /// The item exceeded the anchor-profile item or payload byte ceiling.
    Oversize,
    /// The canonical `Entry` did not decode.
    NonCanonicalEntry,
    /// The canonical `WriteCapability` did not decode.
    NonCanonicalCapability,
    /// The payload length or WILLIAM3 digest did not match the entry.
    PayloadMismatch,
    /// The capability + signature did not authorise the entry (Meadowcap fail).
    DoesNotAuthorise,
}

impl ItemReject {
    /// The `sync/2` admission subject this rejection surfaces.
    pub fn subject(self) -> AdmissionSubject {
        match self {
            ItemReject::Malformed | ItemReject::Oversize => AdmissionSubject::Bundle,
            _ => AdmissionSubject::Entry,
        }
    }
}

/// Encode an anchor-profile item: version byte then four length-prefixed fields
/// `entry_bytes`, `capability_bytes`, 64-byte `signature`, `payload_bytes`. The
/// layout is canonical (fixed field order, big-endian lengths, no slack).
pub fn encode_item(
    entry_bytes: &[u8],
    capability_bytes: &[u8],
    signature: &[u8; 64],
    payload_bytes: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(
        1 + 12 + entry_bytes.len() + capability_bytes.len() + SIGNATURE_BYTES + payload_bytes.len(),
    );
    out.push(ITEM_VERSION);
    out.extend_from_slice(&(entry_bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(entry_bytes);
    out.extend_from_slice(&(capability_bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(capability_bytes);
    out.extend_from_slice(signature);
    out.extend_from_slice(&(payload_bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(payload_bytes);
    out
}

struct DecodedItem {
    entry_bytes: Vec<u8>,
    capability_bytes: Vec<u8>,
    signature: [u8; 64],
    payload_bytes: Vec<u8>,
}

fn decode_item(bytes: &[u8]) -> Result<DecodedItem, ItemReject> {
    if bytes.len() > MAX_ANCHOR_ITEM_BYTES {
        return Err(ItemReject::Oversize);
    }
    let mut cursor = 0usize;
    let take = |cursor: &mut usize, n: usize| -> Result<&[u8], ItemReject> {
        let end = cursor.checked_add(n).ok_or(ItemReject::Malformed)?;
        if end > bytes.len() {
            return Err(ItemReject::Malformed);
        }
        let slice = &bytes[*cursor..end];
        *cursor = end;
        Ok(slice)
    };
    let read_len = |cursor: &mut usize| -> Result<usize, ItemReject> {
        let raw = take(cursor, 4)?;
        Ok(u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]) as usize)
    };

    let version = take(&mut cursor, 1)?[0];
    if version != ITEM_VERSION {
        return Err(ItemReject::Malformed);
    }
    let entry_len = read_len(&mut cursor)?;
    let entry_bytes = take(&mut cursor, entry_len)?.to_vec();
    let cap_len = read_len(&mut cursor)?;
    let capability_bytes = take(&mut cursor, cap_len)?.to_vec();
    let mut signature = [0u8; 64];
    signature.copy_from_slice(take(&mut cursor, SIGNATURE_BYTES)?);
    let payload_len = read_len(&mut cursor)?;
    if payload_len > MAX_ITEM_PAYLOAD_BYTES {
        return Err(ItemReject::Oversize);
    }
    let payload_bytes = take(&mut cursor, payload_len)?.to_vec();
    if cursor != bytes.len() {
        return Err(ItemReject::Malformed);
    }
    Ok(DecodedItem {
        entry_bytes,
        capability_bytes,
        signature,
        payload_bytes,
    })
}

/// The fully decoded and REAL-Meadowcap-verified parts of one anchor-profile item.
/// Exposed so callers that need the entry's payload or authorising capability
/// (e.g. the listing service's crypto-before-admit) can inspect them after the
/// same checked verification [`verify_anchor_item`] performs.
pub struct VerifiedAnchorItem {
    /// The decoded, verified Willow entry.
    pub entry: riot_core::willow::Entry,
    /// Canonical entry bytes.
    pub entry_bytes: Vec<u8>,
    /// Canonical authorising Meadowcap capability bytes.
    pub capability_bytes: Vec<u8>,
    /// The 64-byte subspace signature.
    pub signature: [u8; 64],
    /// The carried payload bytes (integrity-checked against the entry).
    pub payload_bytes: Vec<u8>,
}

/// Verify one untrusted anchor-profile item end-to-end and return its decoded,
/// verified parts. This is the shared trust boundary: it decodes the entry and
/// capability, rebuilds the authorisation token from the 64-byte signature, checks
/// payload length + WILLIAM3 digest and byte bounds, and runs riot-core's checked
/// [`verify_entry`]. It NEVER trusts the client's assertion of authorisation.
pub fn verify_anchor_item_parts(item_bytes: &[u8]) -> Result<VerifiedAnchorItem, ItemReject> {
    let decoded = decode_item(item_bytes)?;

    let entry =
        decode_entry_canonic(&decoded.entry_bytes).map_err(|_| ItemReject::NonCanonicalEntry)?;
    let capability = decode_capability_canonic(&decoded.capability_bytes)
        .map_err(|_| ItemReject::NonCanonicalCapability)?;

    // Payload integrity: the entry's declared length and WILLIAM3 digest must
    // match the carried payload exactly.
    if entry.payload_length() != decoded.payload_bytes.len() as u64 {
        return Err(ItemReject::PayloadMismatch);
    }
    if *entry.payload_digest().as_bytes() != william3_digest(&decoded.payload_bytes) {
        return Err(ItemReject::PayloadMismatch);
    }

    // REAL Meadowcap + signature verification through riot-core's checked path.
    let token = AuthorisationToken::new(capability, SubspaceSignature::from(decoded.signature));
    if !verify_entry(&entry, &token) {
        return Err(ItemReject::DoesNotAuthorise);
    }

    Ok(VerifiedAnchorItem {
        entry,
        entry_bytes: decoded.entry_bytes,
        capability_bytes: decoded.capability_bytes,
        signature: decoded.signature,
        payload_bytes: decoded.payload_bytes,
    })
}

/// Verify one untrusted anchor-profile item end-to-end and project it to a
/// [`StagedEntry`]. This is the anchor's trust boundary: it decodes the entry and
/// capability, rebuilds the authorisation token from the 64-byte signature, checks
/// payload length + WILLIAM3 digest and byte bounds, and runs riot-core's checked
/// [`verify_entry`] (the willow25 `PossiblyAuthorisedEntry` conversion). It NEVER
/// trusts the client's assertion of authorisation.
pub fn verify_anchor_item(item_bytes: &[u8]) -> Result<StagedEntry, ItemReject> {
    let verified = verify_anchor_item_parts(item_bytes)?;
    let entry = &verified.entry;

    let namespace_id = *entry.namespace_id().as_bytes();
    let subspace_id = *Keylike::subspace_id(entry).as_bytes();
    let entry_id = canonical_entry_id(&verified.entry_bytes);
    let timestamp_be = u64::from(entry.timestamp()).to_be_bytes();
    let payload_digest = *entry.payload_digest().as_bytes();
    let payload_length = entry.payload_length();
    let path_bytes = encode_path(entry);

    Ok(StagedEntry {
        namespace_id,
        entry_id,
        subspace_id,
        path_bytes,
        timestamp_be,
        payload_digest,
        payload_length,
        entry_bytes: verified.entry_bytes,
        item_bytes: item_bytes.to_vec(),
    })
}

/// A deterministic canonical encoding of an entry's path: `u32be(component_count)`
/// then, per component, `u32be(len) || component_bytes`.
fn encode_path(entry: &riot_core::willow::Entry) -> Vec<u8> {
    let path = Keylike::path(entry);
    let mut out = Vec::new();
    out.extend_from_slice(&(path.component_count() as u32).to_be_bytes());
    for component in path.components() {
        out.extend_from_slice(&(component.len() as u32).to_be_bytes());
        out.extend_from_slice(component);
    }
    out
}

// ---------------------------------------------------------------------------
// The Sync2Repository adapter.
// ---------------------------------------------------------------------------

/// A shared, single-writer handle to the durable anchor store. The `sync/2` FSM
/// is single-threaded and transport-independent; the responder shares one
/// `AnchorRepository` across the routed phases through this cell.
pub type SharedRepo = Rc<RefCell<AnchorRepository>>;

/// An immutable committed-base snapshot for one namespace, materialised from the
/// anchor store at routing time. The `sync/2` sender streams from it.
pub struct AnchorSnapshot {
    namespace_id: [u8; 32],
    entries: Vec<(Vec<u8>, Vec<u8>)>,
    logical_bytes: u64,
}

impl AnchorSnapshot {
    /// Materialise the committed base for `namespace_id` from `repo`. A host
    /// operation's stage is initialized from the anchor's currently committed
    /// namespace.
    pub fn from_committed(repo: &AnchorRepository, namespace_id: &[u8; 32]) -> Self {
        let committed = repo.committed_entries(namespace_id).unwrap_or_default();
        let logical_bytes = committed.iter().map(|(_, item)| item.len() as u64).sum();
        AnchorSnapshot {
            namespace_id: *namespace_id,
            entries: committed,
            logical_bytes,
        }
    }
}

impl Sync2Snapshot for AnchorSnapshot {
    fn snapshot_digest(&self) -> [u8; 32] {
        let ids: Vec<Vec<u8>> = self.entries.iter().map(|(id, _)| id.clone()).collect();
        compute_snapshot_digest(&self.namespace_id, self.logical_bytes, &ids)
    }
    fn entry_count(&self) -> u64 {
        self.entries.len() as u64
    }
    fn logical_bytes(&self) -> u64 {
        self.logical_bytes
    }
    fn sorted_entry_ids(&self) -> Vec<Vec<u8>> {
        let mut ids: Vec<Vec<u8>> = self.entries.iter().map(|(id, _)| id.clone()).collect();
        ids.sort_unstable();
        ids
    }
    fn item_bytes(&self, entry_id: &[u8]) -> Option<Vec<u8>> {
        self.entries
            .iter()
            .find(|(id, _)| id.as_slice() == entry_id)
            .map(|(_, item)| item.clone())
    }
}

/// A direction-private staging area for one namespace of a host operation.
///
/// `admit` verifies every item (REAL Meadowcap) and, only for verified entries,
/// writes them into operation-private staging in a short durable transaction. The
/// stage is never query-visible outside the operation; the composite Commit
/// promotes it.
pub struct AnchorStage {
    repo: SharedRepo,
    operation_id: [u8; 32],
    namespace_id: [u8; 32],
    stage_deadline: u64,
    staged_at: u64,
}

impl AnchorStage {
    fn base_ids(&self) -> Vec<Vec<u8>> {
        let repo = self.repo.borrow();
        let committed = repo
            .committed_entries(&self.namespace_id)
            .unwrap_or_default();
        let staged = repo
            .staged_entries(&self.operation_id, &self.namespace_id)
            .unwrap_or_default();
        let mut ids: Vec<Vec<u8>> = committed.into_iter().map(|(id, _)| id).collect();
        ids.extend(staged.into_iter().map(|entry| entry.entry_id.to_vec()));
        ids
    }
}

impl Sync2DirectionStage for AnchorStage {
    fn missing(&self, page_ids: &[Vec<u8>]) -> Vec<Vec<u8>> {
        let have = self.base_ids();
        page_ids
            .iter()
            .filter(|id| !have.iter().any(|h| h.as_slice() == id.as_slice()))
            .cloned()
            .collect()
    }

    fn admit(&mut self, entry_ids: &[Vec<u8>], items: &[Vec<u8>]) -> Result<(), AdmissionSubject> {
        // Verify every item BEFORE opening the staging transaction, so a forged
        // item never touches durable state.
        let mut verified: Vec<StagedEntry> = Vec::with_capacity(items.len());
        for (id, item) in entry_ids.iter().zip(items.iter()) {
            let entry = verify_anchor_item(item).map_err(ItemReject::subject)?;
            // The advertised inventory id must match the verified entry id, and the
            // entry must belong to the routed namespace.
            if entry.entry_id.as_slice() != id.as_slice() || entry.namespace_id != self.namespace_id
            {
                return Err(AdmissionSubject::Entry);
            }
            verified.push(entry);
        }

        let mut repo = self.repo.borrow_mut();
        let mut transaction = repo.begin().map_err(|_| AdmissionSubject::Bundle)?;
        transaction
            .ensure_staged_operation(
                &self.operation_id,
                b"host",
                self.staged_at,
                self.stage_deadline,
            )
            .map_err(|_| AdmissionSubject::Bundle)?;
        for entry in &verified {
            transaction
                .stage_entry(&self.operation_id, entry)
                .map_err(|_| AdmissionSubject::Bundle)?;
        }
        transaction.commit().map_err(|_| AdmissionSubject::Bundle)?;
        Ok(())
    }

    fn resulting_digest(&self, namespace_id: &[u8; 32]) -> [u8; 32] {
        let repo = self.repo.borrow();
        let committed = repo.committed_entries(namespace_id).unwrap_or_default();
        let staged = repo
            .staged_entries(&self.operation_id, namespace_id)
            .unwrap_or_default();
        let mut logical: u64 = committed.iter().map(|(_, item)| item.len() as u64).sum();
        let mut ids: Vec<Vec<u8>> = committed.into_iter().map(|(id, _)| id).collect();
        for entry in staged {
            logical += entry.item_bytes.len() as u64;
            ids.push(entry.entry_id.to_vec());
        }
        compute_snapshot_digest(namespace_id, logical, &ids)
    }

    fn promote(&mut self) {
        // A host operation's staged namespaces are promoted atomically by the
        // composite Commit, not per direction. This marker is a no-op.
    }
}

/// The `sync/2` responder adapter over a shared anchor store. It verifies the
/// operation lifecycle and the per-namespace token before routing writes.
pub struct AnchorSyncRepository {
    repo: SharedRepo,
    token_ring: TokenSecretRing,
    now: u64,
}

impl AnchorSyncRepository {
    /// Construct an adapter over a shared store, its token ring, and the current
    /// time (used for operation-expiry checks).
    pub fn new(repo: SharedRepo, token_ring: TokenSecretRing, now: u64) -> Self {
        Self {
            repo,
            token_ring,
            now,
        }
    }
}

impl Sync2Repository for AnchorSyncRepository {
    type Snapshot = AnchorSnapshot;
    type DirectionStage = AnchorStage;

    fn open_namespace(
        &self,
        request: &OpenNamespace,
    ) -> Result<OpenedNamespace<Self>, Sync2Refusal> {
        let (operation_id, namespace_token) = match &request.mode {
            Sync2Mode::HostReconcileStaged {
                operation_id,
                namespace_token,
            } => (*operation_id, *namespace_token),
            Sync2Mode::ReplicaIntoStaged { .. } | Sync2Mode::ReadCommitted => {
                return Err(Sync2Refusal::InvalidMode {
                    observed_mode: request.mode.tag(),
                });
            }
        };

        let repo = self.repo.borrow();
        let operation = repo
            .load_operation(&operation_id)
            .map_err(|_| Sync2Refusal::OperationNotFound { operation_id })?
            .ok_or(Sync2Refusal::OperationNotFound { operation_id })?;

        // Tokens are accepted only while the operation is actively staged
        // (Prepared) and unexpired: a committed/refused operation's tokens are
        // invalid, and an expired one is `operation_expired`.
        if operation.status != OperationStatus::Prepared {
            return Err(Sync2Refusal::OperationNotFound { operation_id });
        }
        if self.now >= operation.operation_expiry {
            return Err(Sync2Refusal::OperationExpired {
                operation_id,
                expires_at: operation.operation_expiry,
                observed_at: self.now,
            });
        }

        // Re-derive the per-namespace token and compare (constant work over the
        // full 32 bytes): the client cannot present a token for a namespace the
        // anchor did not mint for this operation.
        let expected = self
            .token_ring
            .derive(
                operation.token_secret_epoch,
                &operation_id,
                &request.namespace_id,
                operation.operation_expiry,
            )
            .ok_or(Sync2Refusal::InvalidNamespaceToken {
                namespace_id: request.namespace_id,
            })?;
        if !constant_time_eq(&expected, &namespace_token) {
            return Err(Sync2Refusal::InvalidNamespaceToken {
                namespace_id: request.namespace_id,
            });
        }

        let stage_deadline = operation.operation_expiry;
        let staged_at = self.now;
        drop(repo);

        // A host reconciliation is bidirectional: the anchor sends its committed
        // base, then receives the client's entries into the operation stage.
        let sender = {
            let repo = self.repo.borrow();
            AnchorSnapshot::from_committed(&repo, &request.namespace_id)
        };
        let receiver = AnchorStage {
            repo: Rc::clone(&self.repo),
            operation_id,
            namespace_id: request.namespace_id,
            stage_deadline,
            staged_at,
        };
        let parties = vec![
            (Sync2Phase::AnchorToClient, PhaseParty::Sender(sender)),
            (Sync2Phase::ClientToAnchor, PhaseParty::Receiver(receiver)),
        ];
        Ok(OpenedNamespace {
            namespace_id: request.namespace_id,
            mode: Sync2ModeTag::HostReconcileStaged,
            parties,
            stale_source: None,
        })
    }
}

fn constant_time_eq(a: &[u8; 32], b: &[u8; 32]) -> bool {
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Decode the ordered `O`, `C`, `W` namespace host plan captured in an operation's
/// stored prepared-response bytes.
pub fn ordered_host_plan(prepare_response_bytes: &[u8]) -> Option<PrepareSuccessV1> {
    let response =
        decode_canonical::<ControlResponseV1>(prepare_response_bytes, MAX_CONTROL_FRAME_BYTES)
            .ok()?;
    match response.outcome {
        ControlOutcome::Success(ControlSuccess::PrepareHost(payload))
        | ControlOutcome::Success(ControlSuccess::PrepareReplica(payload)) => Some(*payload),
        _ => None,
    }
}
