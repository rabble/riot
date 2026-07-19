//! Reserved owner-removal: the durable unlisting service and its fair, two-lane
//! admission scheduler.
//!
//! An owner unlisting is a `listed == false` tombstone signed by the site root
//! (or a root-delegated author). Unlike an ordinary listing it never consumes
//! ordinary idempotency-row headroom, admission-work capacity, or the ordinary
//! byte/WAL ceilings — it draws exclusively on the preprovisioned reserved
//! capacity so an owner can always take their community out of the directory even
//! when the anchor is otherwise full. This module has two cooperating parts:
//!
//! 1. [`RemovalScheduler`] — a deterministic, in-memory model of the two reserved
//!    verification lanes. A valid **direct-root** signature enters its own queue
//!    with a protected capacity quarter, one exclusively reserved verification
//!    permit, and one exclusively reserved database writer; **delegated** (or
//!    non-direct) candidates share the remaining three verification permits, the
//!    second writer, and the un-reserved three quarters. Aggregate job/byte caps
//!    (limit IDs 51/52) bound both lanes together. A candidate that cannot be
//!    admitted receives a pre-claim `removal_busy` and leaves no durable trace.
//!    A deficit two-round scheduler guarantees an admitted direct-root candidate
//!    reaches final verification within two rounds regardless of delegated churn.
//!
//! 2. [`ReservedRemovalService`] — the atomic durable unlisting transaction. It
//!    does the SAME bounded decode / canonical re-encode / `control_request_digest`
//!    lookup as an ordinary request FIRST (one global idempotency index spans both
//!    the ordinary and the reserved class), then — for a novel key — verifies
//!    authority and atomically transitions the root's reserved slot to `Terminal`,
//!    deletes visibility, appends one `Removed` inclusion, invalidates the
//!    projection, signs the receipt, and stores the exact reserved result. The
//!    acknowledgement is durable-logical: it never waits for physical compaction.

use std::collections::{BTreeMap, VecDeque};

use riot_anchor_protocol::authority::{admit_public_site_ticket, TicketFloor};
use riot_anchor_protocol::codec::{decode_canonical, CanonicalRecord, CodecError};
use riot_anchor_protocol::control::{
    ControlOutcome, ControlRefusal, ControlResponseV1, ControlSuccess, MAX_CONTROL_FRAME_BYTES,
};
use riot_anchor_protocol::digest::digest_v1;
use riot_anchor_protocol::records::{
    AnchorLimitId, CommunityListingV1, ControlOperationKind, ListingDelegateGrantV1,
    ListingReceiptBodyV1, ListingReceiptV1, OperatorSignedEnvelopeV1,
    RootSignedTicketCoreEnvelopeV2, TransportFloor, IDEMPOTENCY_KEY_BYTES,
    MAX_DELEGATE_GRANT_BYTES, MAX_TICKET_CORE_BYTES,
};

use riot_core::willow::{decode_capability_canonic, is_directory_listing};
use willow25::groupings::{Keylike, Namespaced};

use crate::idempotency::{classify, AdmissionLookup, TERMINAL_RETENTION_SECS};
use crate::repository::{
    AnchorRepository, AnchorRepositoryError, IdempotencyClaimState, RemovalSlot, RemovalSlotState,
    RepoTransaction, SlotReservation,
};
use crate::sync_service::{verify_anchor_item_parts, VerifiedAnchorItem};
use crate::work::OperatorSigner;

/// The `emergency_reserves` row name for the reserved owner-removal verification
/// permits (seeded fixed at 4).
pub const OWNER_REMOVAL_VERIFICATION_PERMITS_RESERVE: &str = "owner_removal_verification_permits";
/// The `emergency_reserves` row name for the valid-removal database writer permits
/// (seeded fixed at 2).
pub const VALID_REMOVAL_WRITER_PERMITS_RESERVE: &str = "valid_removal_writer_permits";

/// The signing domain for a directory-inclusion body (operator-signed); shared
/// byte-for-byte with the ordinary listing feed so removals extend the same chain.
const DIRECTORY_INCLUSION_SIGNING_DOMAIN: &[u8] = b"riot/directory-inclusion/v1";
/// The `digest_v1` label for a signed directory-inclusion record.
const DIRECTORY_INCLUSION_ENVELOPE_LABEL: &[u8] = b"riot/directory-inclusion-envelope/v1";
/// Inclusion body wire version.
const DIRECTORY_INCLUSION_VERSION: u8 = 1;

// ===========================================================================
// Part 1: the fair, two-lane reserved-removal admission scheduler
// ===========================================================================

/// Which reserved lane a candidate belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemovalLane {
    /// An authenticated direct-root tombstone (root authority, no delegate walk).
    DirectRoot,
    /// A delegated or otherwise non-direct candidate.
    Delegated,
}

/// The configurable caps and permit counts of the reserved-removal lanes. The
/// permit counts come from the seeded `emergency_reserves` partitions
/// ([`load_lane_limits`]); the aggregate caps come from limit IDs 51/52.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemovalLaneLimits {
    /// Aggregate in-flight job cap across both lanes (limit ID 51).
    pub aggregate_jobs: u32,
    /// Aggregate in-flight canonical-byte cap across both lanes (limit ID 52).
    pub aggregate_canonical_bytes: u64,
    /// Maximum delegated candidates per root (design's "eight per root").
    pub delegated_jobs_per_root: u32,
    /// Verification permits exclusively reserved for the direct-root lane.
    pub direct_verification_permits: u32,
    /// Verification permits available to the delegated lane.
    pub delegated_verification_permits: u32,
    /// Database writer permits exclusively reserved for the direct-root lane.
    pub direct_writer_permits: u32,
    /// Database writer permits available to the delegated lane.
    pub delegated_writer_permits: u32,
}

impl RemovalLaneLimits {
    /// The design's default lane limits: aggregate 51-job / 52-MiB caps (limit IDs
    /// 51 and 52), 8 delegated per root, the 4 verification permits split 1
    /// (direct-exclusive) + 3 (delegated), and the 2 writers split 1 + 1.
    #[must_use]
    pub const fn defaults() -> Self {
        Self {
            aggregate_jobs: 256,
            aggregate_canonical_bytes: 4 * 1024 * 1024,
            delegated_jobs_per_root: 8,
            direct_verification_permits: 1,
            delegated_verification_permits: 3,
            direct_writer_permits: 1,
            delegated_writer_permits: 1,
        }
    }

    /// The protected direct-root quarter of the aggregate job cap. Delegates can
    /// never consume it; a valid direct-root candidate can always borrow it.
    #[must_use]
    pub const fn direct_job_quarter(&self) -> u32 {
        self.aggregate_jobs / 4
    }

    /// The protected direct-root quarter of the aggregate byte cap.
    #[must_use]
    pub const fn direct_byte_quarter(&self) -> u64 {
        self.aggregate_canonical_bytes / 4
    }
}

/// Read the reserved verification / writer permit counts from the durable
/// `emergency_reserves` partitions and combine them with the aggregate caps.
pub fn load_lane_limits(
    repo: &mut AnchorRepository,
    aggregate_jobs: u32,
    aggregate_canonical_bytes: u64,
) -> Result<RemovalLaneLimits, AnchorRepositoryError> {
    let tx = repo.begin()?;
    let verification = tx
        .emergency_reserve_value(OWNER_REMOVAL_VERIFICATION_PERMITS_RESERVE)?
        .unwrap_or(4) as u32;
    let writers = tx
        .emergency_reserve_value(VALID_REMOVAL_WRITER_PERMITS_RESERVE)?
        .unwrap_or(2) as u32;
    drop(tx);
    // One of each is exclusively reserved for the authenticated direct-root lane.
    let direct_verification_permits = 1.min(verification);
    let direct_writer_permits = 1.min(writers);
    Ok(RemovalLaneLimits {
        aggregate_jobs,
        aggregate_canonical_bytes,
        delegated_jobs_per_root: 8,
        direct_verification_permits,
        delegated_verification_permits: verification - direct_verification_permits,
        direct_writer_permits,
        delegated_writer_permits: writers - direct_writer_permits,
    })
}

/// A pre-claim admission refusal. It maps to a `removal_busy` control refusal with
/// the saturated limit; it creates NO durable index/slot/result row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemovalOverload {
    /// The saturated limit that forced the refusal.
    pub limit_id: AnchorLimitId,
}

/// An opaque proof that a candidate is admitted and holds its lane's reserved
/// verification permit. It must be released (or completed) to free the permit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdmitTicket {
    lane: RemovalLane,
    root: [u8; 32],
    source: [u8; 32],
    canonical_bytes: u64,
    writer_granted: bool,
}

impl AdmitTicket {
    /// The lane this admitted candidate belongs to.
    #[must_use]
    pub fn lane(&self) -> RemovalLane {
        self.lane
    }

    /// Whether this candidate has been granted a database writer permit (i.e. has
    /// reached final verification via a scheduler round).
    #[must_use]
    pub fn writer_granted(&self) -> bool {
        self.writer_granted
    }
}

/// Per-root delegated in-flight accounting.
#[derive(Debug, Default, Clone)]
struct DelegatedRoot {
    sources: VecDeque<[u8; 32]>,
}

/// The deterministic two-lane reserved-removal admission + scheduling model.
///
/// It is pure and in-memory: durable state lives in the repository. Admission
/// enforces the protected direct quarter, the aggregate caps, the per-root /
/// per-source structural limits, and the reserved verification permits;
/// [`RemovalScheduler::run_round`] grants the reserved database writers with a
/// deficit two-round fairness guarantee for admitted direct-root work.
#[derive(Debug, Clone)]
pub struct RemovalScheduler {
    limits: RemovalLaneLimits,
    direct_jobs: BTreeMap<[u8; 32], u64>,
    delegated: BTreeMap<[u8; 32], DelegatedRoot>,
    delegated_bytes: u64,
    direct_bytes: u64,
    direct_verify_in_use: u32,
    delegated_verify_in_use: u32,
    direct_writer_in_use: u32,
    delegated_writer_in_use: u32,
    // Round-robin queues of admitted candidates awaiting a writer permit.
    direct_queue: VecDeque<AdmitTicket>,
    delegated_queue: VecDeque<AdmitTicket>,
    // Rotating cursor for delegated fairness.
    delegated_cursor: usize,
}

impl RemovalScheduler {
    /// Construct a scheduler with the given lane limits.
    #[must_use]
    pub fn new(limits: RemovalLaneLimits) -> Self {
        Self {
            limits,
            direct_jobs: BTreeMap::new(),
            delegated: BTreeMap::new(),
            delegated_bytes: 0,
            direct_bytes: 0,
            direct_verify_in_use: 0,
            delegated_verify_in_use: 0,
            direct_writer_in_use: 0,
            delegated_writer_in_use: 0,
            direct_queue: VecDeque::new(),
            delegated_queue: VecDeque::new(),
            delegated_cursor: 0,
        }
    }

    /// The lane limits this scheduler was built with.
    #[must_use]
    pub fn limits(&self) -> RemovalLaneLimits {
        self.limits
    }

    /// Total in-flight admitted jobs across both lanes.
    #[must_use]
    pub fn in_flight_jobs(&self) -> u32 {
        self.direct_jobs.len() as u32 + self.delegated_job_count()
    }

    fn delegated_job_count(&self) -> u32 {
        self.delegated
            .values()
            .map(|root| root.sources.len() as u32)
            .sum()
    }

    /// Attempt to admit a valid candidate into its lane, acquiring a reserved
    /// verification permit. On success the candidate is queued for a writer permit.
    /// On failure the caller must return `removal_busy` and persist nothing.
    pub fn try_admit(
        &mut self,
        lane: RemovalLane,
        root: [u8; 32],
        source: [u8; 32],
        canonical_bytes: u64,
    ) -> Result<AdmitTicket, RemovalOverload> {
        match lane {
            RemovalLane::DirectRoot => self.try_admit_direct(root, source, canonical_bytes),
            RemovalLane::Delegated => self.try_admit_delegated(root, source, canonical_bytes),
        }
    }

    fn try_admit_direct(
        &mut self,
        root: [u8; 32],
        source: [u8; 32],
        canonical_bytes: u64,
    ) -> Result<AdmitTicket, RemovalOverload> {
        // At most one direct candidate per root; a second serializes → busy.
        if self.direct_jobs.contains_key(&root) {
            return Err(RemovalOverload {
                limit_id: AnchorLimitId::QueuedReservedRemovalJobs,
            });
        }
        // Aggregate job cap. The direct quarter is always available to direct work
        // as long as the aggregate is not entirely full.
        if self.in_flight_jobs() >= self.limits.aggregate_jobs {
            return Err(RemovalOverload {
                limit_id: AnchorLimitId::QueuedReservedRemovalJobs,
            });
        }
        // Aggregate byte cap.
        if self.direct_bytes + self.delegated_bytes + canonical_bytes
            > self.limits.aggregate_canonical_bytes
        {
            return Err(RemovalOverload {
                limit_id: AnchorLimitId::QueuedReservedRemovalCanonicalBytes,
            });
        }
        // Direct-exclusive verification permit (never exceeded thanks to
        // one-per-root, but checked for completeness).
        if self.direct_verify_in_use >= self.limits.direct_verification_permits {
            return Err(RemovalOverload {
                limit_id: AnchorLimitId::ReservedOwnerRemovalVerificationPermits,
            });
        }
        self.direct_jobs.insert(root, canonical_bytes);
        self.direct_bytes += canonical_bytes;
        self.direct_verify_in_use += 1;
        let ticket = AdmitTicket {
            lane: RemovalLane::DirectRoot,
            root,
            source,
            canonical_bytes,
            writer_granted: false,
        };
        self.direct_queue.push_back(ticket);
        Ok(ticket)
    }

    fn try_admit_delegated(
        &mut self,
        root: [u8; 32],
        source: [u8; 32],
        canonical_bytes: u64,
    ) -> Result<AdmitTicket, RemovalOverload> {
        let entry = self.delegated.get(&root);
        // At most one per (root, source).
        if entry.is_some_and(|root| root.sources.contains(&source)) {
            return Err(RemovalOverload {
                limit_id: AnchorLimitId::QueuedReservedRemovalJobs,
            });
        }
        // At most eight per root.
        let per_root = entry.map_or(0, |root| root.sources.len() as u32);
        if per_root >= self.limits.delegated_jobs_per_root {
            return Err(RemovalOverload {
                limit_id: AnchorLimitId::QueuedReservedRemovalJobs,
            });
        }
        // Delegates are confined to the un-reserved three quarters of the job cap;
        // the protected direct quarter stays available.
        let delegated_ceiling = self.limits.aggregate_jobs - self.limits.direct_job_quarter();
        if self.delegated_job_count() + 1 > delegated_ceiling {
            return Err(RemovalOverload {
                limit_id: AnchorLimitId::QueuedReservedRemovalJobs,
            });
        }
        // Delegates are likewise confined to the un-reserved byte capacity.
        let delegated_byte_ceiling =
            self.limits.aggregate_canonical_bytes - self.limits.direct_byte_quarter();
        if self.delegated_bytes + canonical_bytes > delegated_byte_ceiling {
            return Err(RemovalOverload {
                limit_id: AnchorLimitId::QueuedReservedRemovalCanonicalBytes,
            });
        }
        // Delegated verification permits (never the direct-exclusive one).
        if self.delegated_verify_in_use >= self.limits.delegated_verification_permits {
            return Err(RemovalOverload {
                limit_id: AnchorLimitId::ReservedOwnerRemovalVerificationPermits,
            });
        }
        self.delegated
            .entry(root)
            .or_default()
            .sources
            .push_back(source);
        self.delegated_bytes += canonical_bytes;
        self.delegated_verify_in_use += 1;
        let ticket = AdmitTicket {
            lane: RemovalLane::Delegated,
            root,
            source,
            canonical_bytes,
            writer_granted: false,
        };
        self.delegated_queue.push_back(ticket);
        Ok(ticket)
    }

    /// Release an admitted candidate, freeing its verification permit, any writer
    /// permit it holds, and its job/byte accounting.
    pub fn release(&mut self, ticket: &AdmitTicket) {
        match ticket.lane {
            RemovalLane::DirectRoot => {
                if self.direct_jobs.remove(&ticket.root).is_some() {
                    self.direct_bytes = self.direct_bytes.saturating_sub(ticket.canonical_bytes);
                    self.direct_verify_in_use = self.direct_verify_in_use.saturating_sub(1);
                    if ticket.writer_granted {
                        self.direct_writer_in_use = self.direct_writer_in_use.saturating_sub(1);
                    }
                }
                self.direct_queue
                    .retain(|queued| queued.root != ticket.root);
            }
            RemovalLane::Delegated => {
                if let Some(root) = self.delegated.get_mut(&ticket.root) {
                    if let Some(position) = root.sources.iter().position(|s| *s == ticket.source) {
                        root.sources.remove(position);
                        self.delegated_bytes =
                            self.delegated_bytes.saturating_sub(ticket.canonical_bytes);
                        self.delegated_verify_in_use =
                            self.delegated_verify_in_use.saturating_sub(1);
                        if ticket.writer_granted {
                            self.delegated_writer_in_use =
                                self.delegated_writer_in_use.saturating_sub(1);
                        }
                    }
                    if root.sources.is_empty() {
                        self.delegated.remove(&ticket.root);
                    }
                }
                self.delegated_queue.retain(|queued| {
                    !(queued.root == ticket.root && queued.source == ticket.source)
                });
            }
        }
    }

    /// Run one fair scheduler round, granting available database writer permits to
    /// queued admitted candidates. The direct-root lane is served first from its
    /// exclusively reserved writer; the delegated lane is served round-robin from
    /// its own writer. Returns the tickets that reached final verification (a
    /// granted writer) this round. An admitted direct-root candidate is guaranteed
    /// a writer within two rounds.
    pub fn run_round(&mut self) -> Vec<AdmitTicket> {
        let mut granted = Vec::new();
        // Direct-root first: exclusive writer permit.
        while self.direct_writer_in_use < self.limits.direct_writer_permits {
            let Some(mut ticket) = self.direct_queue.pop_front() else {
                break;
            };
            self.direct_writer_in_use += 1;
            ticket.writer_granted = true;
            granted.push(ticket);
        }
        // Delegated lane: its own writer permit(s), round-robin.
        while self.delegated_writer_in_use < self.limits.delegated_writer_permits {
            let Some(mut ticket) = self.delegated_queue.pop_front() else {
                break;
            };
            self.delegated_writer_in_use += 1;
            ticket.writer_granted = true;
            self.delegated_cursor = self.delegated_cursor.wrapping_add(1);
            granted.push(ticket);
        }
        granted
    }
}

// ===========================================================================
// Part 2: the atomic durable reserved-removal (unlisting) service
// ===========================================================================

/// A failpoint hook (mirrors [`crate::listing`]): the service calls it before each
/// durable mutation with a stable label; returning `true` aborts before commit so
/// the whole transaction rolls back. Production passes [`no_failpoint`].
pub type Failpoint<'a> = &'a mut dyn FnMut(&str) -> bool;

/// A failpoint hook that never trips (production).
pub fn no_failpoint(_: &str) -> bool {
    false
}

/// The immutable coordinates the removal service stamps into every receipt.
#[derive(Debug, Clone)]
pub struct RemovalContext {
    /// Stable anchor id.
    pub anchor_id: [u8; 32],
    /// Current signing operator key id.
    pub operator_key_id: [u8; 32],
    /// Current descriptor epoch.
    pub descriptor_epoch: u64,
    /// Current descriptor digest.
    pub descriptor_digest: [u8; 32],
}

/// A root-signed delegate grant received SEPARATELY from the tombstone item.
#[derive(Debug, Clone)]
pub struct RawDelegateGrant {
    /// Canonical [`ListingDelegateGrantV1`] body bytes.
    pub grant_bytes: Vec<u8>,
    /// The `O`-root Ed25519 signature over the grant.
    pub signature: [u8; 64],
}

/// The raw, untrusted materials an owner submits to unlist: the full signed Willow
/// tombstone entry (a `listed == false` [`CommunityListingV1`]) plus, for a
/// delegated removal, the separately supplied root-signed grant.
#[derive(Debug, Clone)]
pub struct RawRemovalSubmission {
    /// The complete signed Willow tombstone entry in anchor-item format.
    pub tombstone_item_bytes: Vec<u8>,
    /// `None` = root-owned (direct-root lane); `Some` = delegated lane.
    pub delegate_grant: Option<RawDelegateGrant>,
}

/// An error that prevents the removal service from producing any control result.
#[derive(Debug)]
#[non_exhaustive]
pub enum RemovalError {
    /// A durable-store error.
    Repository(AnchorRepositoryError),
    /// A canonical-encoding error building the receipt/response.
    Codec(CodecError),
    /// An injected failpoint tripped before commit (test-only).
    Failpoint(&'static str),
}

impl core::fmt::Display for RemovalError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Repository(error) => write!(formatter, "removal repository error: {error}"),
            Self::Codec(error) => write!(formatter, "removal codec error: {error:?}"),
            Self::Failpoint(label) => write!(formatter, "removal failpoint tripped: {label}"),
        }
    }
}

impl std::error::Error for RemovalError {}

impl From<AnchorRepositoryError> for RemovalError {
    fn from(error: AnchorRepositoryError) -> Self {
        Self::Repository(error)
    }
}

impl From<CodecError> for RemovalError {
    fn from(error: CodecError) -> Self {
        Self::Codec(error)
    }
}

/// The verified, admit-ready tombstone produced by `verify_removal`.
struct VerifiedRemoval {
    listing: CommunityListingV1,
    listing_digest: [u8; 32],
    community_id: [u8; 32],
    root_id: [u8; 32],
    lane: RemovalLane,
    source: [u8; 32],
}

/// The reserved owner-removal service.
pub struct ReservedRemovalService<S: OperatorSigner> {
    context: RemovalContext,
    signer: S,
    retry_after_seconds: u64,
}

impl<S: OperatorSigner> ReservedRemovalService<S> {
    /// Construct a removal service. `retry_after_seconds` is the nonzero delay
    /// stamped into every `removal_busy` / `removal_replay_window` refusal.
    pub fn new(context: RemovalContext, signer: S, retry_after_seconds: u64) -> Self {
        Self {
            context,
            signer,
            retry_after_seconds: retry_after_seconds.max(1),
        }
    }

    /// Handle one owner-unlisting request against its own idempotency key and the
    /// shared reserved-removal scheduler. The disposition is one transaction:
    /// idempotency replay/conflict, `already_unlisted`, `removal_busy`,
    /// `invalid_listing_authority`, or an atomic terminalisation of the reserved
    /// slot with a signed receipt. The acknowledgement never waits for physical
    /// compaction. `fp` injects crash-safety failpoints.
    #[allow(clippy::too_many_arguments)]
    pub fn submit(
        &self,
        repo: &mut AnchorRepository,
        scheduler: &mut RemovalScheduler,
        idempotency_key: &[u8; IDEMPOTENCY_KEY_BYTES],
        submission: &RawRemovalSubmission,
        control_request_digest: &[u8; 32],
        now: u64,
        fp: Failpoint<'_>,
    ) -> Result<ControlResponseV1, RemovalError> {
        // 1. Global idempotency lookup FIRST — the ONE index spans both the
        //    ordinary and reserved classes. An equal-body retry replays the exact
        //    reserved terminal bytes; a changed body is a conflict with no
        //    disclosure. A key first used by an ordinary op is found here too.
        let tx = repo.begin()?;
        match classify(
            tx.lookup_idempotency(idempotency_key)?.as_ref(),
            control_request_digest,
        ) {
            AdmissionLookup::ReplayEqual { .. } => {
                // Reserved terminal replay; an ordinary key replays ordinary bytes.
                let bytes = match tx.reserved_result(control_request_digest)? {
                    Some(bytes) => bytes,
                    None => tx
                        .ordinary_result(control_request_digest)?
                        .ok_or(RemovalError::Codec(CodecError::Malformed))?,
                };
                drop(tx);
                return Ok(decode_canonical::<ControlResponseV1>(
                    &bytes,
                    MAX_CONTROL_FRAME_BYTES,
                )?);
            }
            AdmissionLookup::Conflict => {
                drop(tx);
                return Ok(refuse(ControlRefusal::IdempotencyConflict));
            }
            AdmissionLookup::Novel => {}
        }
        drop(tx);

        // 2. crypto-before-admit. An invalid signature/authority is refused with
        //    the single closed reason and NEVER enters the reserved queue (it
        //    consumes no permit, slot, or index row).
        let verified = match self.verify_removal(submission, now) {
            Ok(verified) => verified,
            Err(refusal) => return Ok(refuse(refusal)),
        };

        // 3. Locate the root's reserved slot. If the community is already unlisted
        //    (no reserved slot), return the bounded `already_unlisted` floor — no
        //    claim, no slot, no scheduler permit consumed.
        let tx = repo.begin()?;
        let slot = match tx.reserved_slot_for_community(&verified.community_id)? {
            Some(slot) => slot,
            None => {
                drop(tx);
                return Ok(refuse(ControlRefusal::AlreadyUnlisted));
            }
        };
        drop(tx);

        // 4. Reserved fair-lane admission (pre-claim). Overload → `removal_busy`
        //    with retry timing and NO durable trace.
        let ticket = match scheduler.try_admit(
            verified.lane,
            verified.root_id,
            verified.source,
            submission.tombstone_item_bytes.len() as u64,
        ) {
            Ok(ticket) => ticket,
            Err(overload) => {
                return Ok(refuse(ControlRefusal::RemovalBusy {
                    limit_id: overload.limit_id,
                    retry_after_seconds: self.retry_after_seconds,
                }));
            }
        };

        // 5. Atomic terminalisation. On any failure the scheduler permit is
        //    released so a retry can proceed.
        let result = self.terminalize(
            repo,
            idempotency_key,
            control_request_digest,
            &verified,
            &slot,
            now,
            fp,
        );
        scheduler.release(&ticket);
        result
    }

    /// The atomic unlisting transaction: transition the reserved slot to
    /// `Terminal`, delete visibility, append exactly one `Removed` inclusion,
    /// invalidate the projection once, claim the reserved idempotency key, store
    /// the byte-identical reserved result, and stamp the receipt. It never uses
    /// ordinary idempotency capacity, so it survives ordinary-row exhaustion.
    #[allow(clippy::too_many_arguments)]
    fn terminalize(
        &self,
        repo: &mut AnchorRepository,
        idempotency_key: &[u8; IDEMPOTENCY_KEY_BYTES],
        control_request_digest: &[u8; 32],
        verified: &VerifiedRemoval,
        slot: &RemovalSlot,
        now: u64,
        fp: Failpoint<'_>,
    ) -> Result<ControlResponseV1, RemovalError> {
        let mut tx = repo.begin()?;

        // (a) delete visibility (listing/search/current-state), retaining the slot
        //     and the signed feed history.
        if fp("visibility") {
            return Err(RemovalError::Failpoint("visibility"));
        }
        tx.delete_listing(&verified.community_id)?;

        // (b) append EXACTLY ONE signed `Removed` inclusion, advancing the feed.
        if fp("inclusion") {
            return Err(RemovalError::Failpoint("inclusion"));
        }
        let sequence = self.append_removed_inclusion(
            &mut tx,
            &verified.community_id,
            verified.listing_digest,
            now,
        )?;

        // (c) invalidate the directory/search projection EXACTLY ONCE.
        if fp("projection") {
            return Err(RemovalError::Failpoint("projection"));
        }
        tx.invalidate_projection_generation()?;

        // (d) build + sign the receipt and its exact terminal bytes.
        if fp("receipt") {
            return Err(RemovalError::Failpoint("receipt"));
        }
        let response = self.removal_response(verified, sequence, now, idempotency_key)?;
        let response_bytes = response.encode_canonical()?;
        let expires_at = now.saturating_add(TERMINAL_RETENTION_SECS);

        // (e) atomically claim the RESERVED idempotency key (never charges the
        //     ordinary row ceiling), transition the slot to Terminal, and store
        //     the reserved result. This is the ack-durable boundary; physical
        //     compaction is asynchronous and NOT on this path.
        if fp("terminal") {
            return Err(RemovalError::Failpoint("terminal"));
        }
        tx.claim_idempotency_reserved(
            control_request_digest,
            idempotency_key,
            IdempotencyClaimState::Terminal,
            now,
            expires_at,
        )?;
        tx.terminalize_removal_slot(
            slot.slot_index,
            idempotency_key,
            control_request_digest,
            expires_at,
            &response_bytes,
        )?;
        tx.store_reserved_result(control_request_digest, slot.slot_index, &response_bytes)?;

        if fp("commit") {
            return Err(RemovalError::Failpoint("commit"));
        }
        tx.commit()?;
        Ok(response)
    }

    /// The crypto-before-admit boundary for a tombstone. Mirrors the ordinary
    /// listing verification but REQUIRES `listed == false`, and classifies the
    /// reserved lane (root-owned → direct-root; delegated → delegated). Every
    /// failure collapses to the single closed `invalid_listing_authority`.
    fn verify_removal(
        &self,
        submission: &RawRemovalSubmission,
        now: u64,
    ) -> Result<VerifiedRemoval, ControlRefusal> {
        let VerifiedAnchorItem {
            entry,
            capability_bytes,
            payload_bytes,
            ..
        } = verify_anchor_item_parts(&submission.tombstone_item_bytes)
            .map_err(|_| ControlRefusal::InvalidListingAuthority)?;

        let listing = decode_canonical::<CommunityListingV1>(
            &payload_bytes,
            riot_anchor_protocol::records::MAX_LISTING_ENVELOPE_BYTES,
        )
        .map_err(|_| ControlRefusal::InvalidListingAuthority)?;

        // The reserved removal path handles ONLY tombstones (listed == false).
        if listing.listed {
            return Err(ControlRefusal::InvalidListingAuthority);
        }

        if !is_directory_listing(Keylike::path(&entry)) {
            return Err(ControlRefusal::InvalidListingAuthority);
        }
        let entry_namespace = *entry.namespace_id().as_bytes();
        if entry_namespace != listing.root_id || entry_namespace != listing.o_namespace_id {
            return Err(ControlRefusal::InvalidListingAuthority);
        }
        let author_subspace = *Keylike::subspace_id(&entry).as_bytes();

        let capability = decode_capability_canonic(&capability_bytes)
            .map_err(|_| ControlRefusal::InvalidListingAuthority)?;
        let zero_delegation = capability.delegations().is_empty();
        let (lane, source) = match &submission.delegate_grant {
            None => {
                // Direct-root lane: only the root secret can mint a zero-delegation
                // owned cap over O.
                if !zero_delegation {
                    return Err(ControlRefusal::InvalidListingAuthority);
                }
                (RemovalLane::DirectRoot, listing.root_id)
            }
            Some(raw_grant) => {
                if zero_delegation {
                    return Err(ControlRefusal::InvalidListingAuthority);
                }
                let grant = decode_canonical::<ListingDelegateGrantV1>(
                    &raw_grant.grant_bytes,
                    MAX_DELEGATE_GRANT_BYTES,
                )
                .map_err(|_| ControlRefusal::InvalidListingAuthority)?;
                riot_anchor_protocol::authority::verify_listing_delegate_grant(
                    &grant,
                    &raw_grant.signature,
                )
                .map_err(|_| ControlRefusal::InvalidListingAuthority)?;
                if grant.root_id != listing.root_id
                    || grant.listing_epoch != listing.listing_epoch
                    || grant.delegate_key != author_subspace
                {
                    return Err(ControlRefusal::InvalidListingAuthority);
                }
                (RemovalLane::Delegated, author_subspace)
            }
        };

        // Internal-consistency self-check: the embedded root-signed ticket must
        // verify AND carry byte-identical coordinates.
        let ticket_envelope = decode_canonical::<RootSignedTicketCoreEnvelopeV2>(
            &listing.ticket_core_bytes,
            MAX_TICKET_CORE_BYTES + 128,
        )
        .map_err(|_| ControlRefusal::InvalidListingAuthority)?;
        let admitted = admit_public_site_ticket(
            &ticket_envelope,
            None,
            &TransportFloor::RequireNone,
            &TicketFloor {
                root_id: listing.root_id,
                highest_transport_epoch: None,
            },
            now,
        )
        .map_err(|_| ControlRefusal::InvalidListingAuthority)?;
        let core = &admitted.core;
        if core.root_id != listing.root_id
            || core.o_namespace_id != listing.o_namespace_id
            || core.c_namespace_id != listing.c_namespace_id
            || core.w_namespace_id != listing.w_namespace_id
            || core.manifest_digest != listing.manifest_digest
            || core.manifest_version != listing.manifest_version
        {
            return Err(ControlRefusal::InvalidListingAuthority);
        }

        let listing_digest = digest_v1(b"riot/community-tombstone/v1", &payload_bytes);
        Ok(VerifiedRemoval {
            community_id: listing.o_namespace_id,
            root_id: listing.root_id,
            listing_digest,
            listing,
            lane,
            source,
        })
    }

    /// Build, sign, and store one `Removed` directory-inclusion; advance the feed
    /// head; return the new monotonic sequence (`feed_coordinate`).
    fn append_removed_inclusion(
        &self,
        tx: &mut RepoTransaction<'_>,
        community_id: &[u8; 32],
        listing_digest: [u8; 32],
        now: u64,
    ) -> Result<u64, RemovalError> {
        let (previous_digest, previous_length) = tx.feed_head()?;
        let sequence = previous_length.saturating_add(1);
        let record_bytes = self.sign_inclusion(
            community_id,
            sequence,
            &previous_digest,
            &listing_digest,
            now,
        );
        let inclusion_digest = digest_v1(DIRECTORY_INCLUSION_ENVELOPE_LABEL, &record_bytes);
        tx.insert_directory_inclusion(&inclusion_digest, community_id, now, &record_bytes)?;
        let advanced = tx.advance_feed_head(&inclusion_digest, now)?;
        debug_assert_eq!(advanced, sequence);
        Ok(advanced)
    }

    /// Deterministically encode and operator-sign one `Removed` inclusion body
    /// (`listed = false`), byte-compatible with the ordinary listing feed chain.
    fn sign_inclusion(
        &self,
        community_id: &[u8; 32],
        sequence: u64,
        previous_inclusion_digest: &[u8; 32],
        listing_digest: &[u8; 32],
        accepted_at: u64,
    ) -> Vec<u8> {
        let mut body = Vec::with_capacity(1 + 32 + 8 + 32 + 32 + 1 + 8);
        body.push(DIRECTORY_INCLUSION_VERSION);
        body.extend_from_slice(community_id);
        body.extend_from_slice(&sequence.to_be_bytes());
        body.extend_from_slice(previous_inclusion_digest);
        body.extend_from_slice(listing_digest);
        body.push(0u8); // listed = false (a Removed inclusion)
        body.extend_from_slice(&accepted_at.to_be_bytes());

        let mut preimage = DIRECTORY_INCLUSION_SIGNING_DOMAIN.to_vec();
        preimage.extend_from_slice(&body);
        let signature = self.signer.sign(&preimage);

        let mut record = body;
        record.extend_from_slice(&signature);
        record
    }

    /// Sign a [`ListingReceiptV1`] for the removal and wrap it in the success.
    fn removal_response(
        &self,
        verified: &VerifiedRemoval,
        feed_coordinate: u64,
        now: u64,
        idempotency_key: &[u8; IDEMPOTENCY_KEY_BYTES],
    ) -> Result<ControlResponseV1, RemovalError> {
        let receipt = self.sign_receipt(ListingReceiptBodyV1 {
            anchor_id: self.context.anchor_id,
            operator_key_id: self.context.operator_key_id,
            descriptor_epoch: self.context.descriptor_epoch,
            descriptor_digest: self.context.descriptor_digest,
            listing_digest: verified.listing_digest,
            full_site_root: verified.root_id,
            accepted_listing_epoch: verified.listing.listing_epoch,
            accepted_listing_revision: verified.listing.listing_revision,
            feed_coordinate,
            accepted_at: now,
            expires_at: verified.listing.expiry_unix_seconds,
            request_idempotency_key: *idempotency_key,
        })?;
        Ok(ControlResponseV1 {
            kind: ControlOperationKind::SubmitListing,
            outcome: ControlOutcome::Success(ControlSuccess::SubmitListing(Box::new(receipt))),
        })
    }

    fn sign_receipt(&self, body: ListingReceiptBodyV1) -> Result<ListingReceiptV1, RemovalError> {
        let mut envelope = OperatorSignedEnvelopeV1 {
            body,
            operator_signature: [0u8; 64],
        };
        let preimage = envelope.signing_preimage()?;
        envelope.operator_signature = self.signer.sign(&preimage);
        Ok(envelope)
    }
}

fn refuse(refusal: ControlRefusal) -> ControlResponseV1 {
    ControlResponseV1 {
        kind: ControlOperationKind::SubmitListing,
        outcome: ControlOutcome::Refused(refusal),
    }
}

/// Reserve one of the `2 * L` slots for a root becoming visible, enforcing the
/// exact per-root two-slot rule; on `Blocked`, the caller returns
/// `removal_replay_window`. This is the reservation primitive an ordinary
/// visibility transition uses; the removal service consumes the slot it reserves.
pub fn reserve_visibility_slot(
    tx: &mut RepoTransaction<'_>,
    community_id: &[u8; 32],
    root_key: &[u8; 32],
    request_digest: &[u8; 32],
    now: u64,
) -> Result<SlotReservation, AnchorRepositoryError> {
    tx.reserve_visibility_slot(community_id, root_key, request_digest, now)
}

/// Idempotent startup cleanup: release every reserved slot whose community no
/// longer has a live listing (an abandoned reservation), matching the design's
/// "abandoned reservations cannot accumulate". Returns how many were released.
pub fn release_abandoned_reservations(
    repo: &mut AnchorRepository,
) -> Result<u64, AnchorRepositoryError> {
    let mut tx = repo.begin()?;
    let abandoned = tx.abandoned_reserved_slots()?;
    for slot in &abandoned {
        tx.release_removal_slot(*slot)?;
    }
    tx.commit()?;
    Ok(abandoned.len() as u64)
}

/// Map a `RemovalSlotState` for readers that need the raw discriminant.
#[must_use]
pub fn slot_state_code(state: RemovalSlotState) -> i64 {
    state.to_code()
}
