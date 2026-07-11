//! Session arbiter and preview-first atomic import.
//!
//! All session, store, preview, and plan state lives behind one
//! `Arc<Mutex<SessionState>>` — the single linearization point. Handles
//! (`EvidenceStore`, `ImportPreview`, `ImportPlan`) carry only IDs plus that
//! `Arc`; every method acquires the arbiter before any admission check or
//! mutation. Import is copy-on-write: the join plan is computed against a
//! clone and installed with one pointer swap, so a fault before the swap
//! leaves all observable state unchanged.

use std::sync::{Arc, Mutex};

use crate::import::bundle::{decode_bundle, BundleDecodeOutcome, BundleRejection, ItemStatus};
use crate::import::join::{
    plan_join_with_payloads, JoinEffect, JoinState, STORE_CHARGE_ENTRY_BYTES,
};
use crate::model::decode_alert;
use crate::willow::{
    alert_entry_path_matches_payload, decode_capability_canonic, decode_entry_canonic,
    AuthorisationToken, EntryId,
};
use willow25::authorisation::PossiblyAuthorisedEntry;

/// Ceilings from fixtures/manifest.json.
const MAX_RECEIPTS: usize = 256;
/// A live preview can issue at most this many plans.
const MAX_PLANS_PER_PREVIEW: usize = 64;

/// Fixed accounting charges and the hard retained-store budget from
/// fixtures/manifest.json. These bound the store's *total* retained byte
/// footprint (live entries plus permanent receipt/reference/namespace/index
/// history), which no single count-based ceiling above composes to
/// guarantee on its own.
const STORE_CHARGE_NAMESPACE_BYTES: u64 = 256;
/// Charged once per retained `DispositionRow`, matching the actual
/// per-row growth of a receipt's retained `Vec<DispositionRow>` — a receipt
/// with more rows genuinely retains more bytes, so a flat per-receipt charge
/// would undercount.
const STORE_CHARGE_RECEIPT_BYTES: u64 = 256;
const STORE_CHARGE_DIGEST_REFERENCE_BYTES: u64 = 32;
const RETAINED_STORE_BUDGET_BYTES: u64 = 16_777_216;
/// `namespace_views` from fixtures/manifest.json: the most distinct Willow
/// namespaces one store may ever observe.
const MAX_NAMESPACE_VIEWS: usize = 64;
/// `retained_preview_output_bytes` from fixtures/manifest.json: bounds a
/// live preview and its active plan's own retained bytes for their
/// lifetime — a *separate* budget from the store's permanent one, since a
/// caller can `inspect()`/`plan()` without ever committing.
const PREVIEW_OUTPUT_BUDGET_BYTES: u64 = 2_097_152;
/// `plan_tombstone_bytes` from fixtures/manifest.json: charged once per
/// retained terminal record in `plan_tombstones`.
const PLAN_TOMBSTONE_BYTES: u64 = 256;

/// Retained bytes for one set of preview/plan entries plus their route: the
/// same per-entry index charge used by the store (there is no separate
/// manifest constant for preview entries), on top of each entry's actual
/// canonical size, plus the route string's own bytes.
fn preview_output_entries_charge_bytes(entries: &[VerifiedEntry], route_len: usize) -> u64 {
    entries
        .iter()
        .map(|v| {
            STORE_CHARGE_ENTRY_BYTES
                + v.entry_bytes_len as u64
                + v.payload.as_ref().map_or(0, |p| p.len()) as u64
        })
        .sum::<u64>()
        + route_len as u64
}

/// Retained bytes added by a plan on top of its still-live preview. Entry
/// metadata and the route are cloned into `PlanState`, but immutable payload
/// bytes are `Arc`-shared with `PreviewState`, so charging them a second time
/// would count an allocation that does not exist.
fn plan_output_entries_charge_bytes(entries: &[VerifiedEntry], route_len: usize) -> u64 {
    entries
        .iter()
        .map(|v| STORE_CHARGE_ENTRY_BYTES + v.entry_bytes_len as u64)
        .sum::<u64>()
        + route_len as u64
}

/// Would a preview/plan pair with this much combined retained-entry/route
/// charge, plus `plan_tombstones.len() * PLAN_TOMBSTONE_BYTES`, exceed the
/// 2 MiB preview-output budget? Pure arithmetic, exactly testable at the
/// boundary — same pattern as `store_charge_exceeds_budget`.
fn preview_output_exceeds_budget(charge_bytes: u64) -> bool {
    charge_bytes > PREVIEW_OUTPUT_BUDGET_BYTES
}

/// Would committing a receipt that charges `receipt_charge_delta` bytes
/// (per-row overhead, digest references, and the receipt's own route bytes),
/// against a store already carrying `retained_receipt_charge_bytes` of
/// permanent receipt history, `seen_index_charge_bytes` of permanent
/// per-seen-entry index overhead, `live_entry_bytes` of current live-entry
/// canonical bytes, and `namespace_charge_bytes` for every distinct
/// namespace ever observed, push the store over its frozen retained-byte
/// budget? Pure arithmetic: exactly testable at the boundary without
/// needing to legitimately construct 16 MiB of retained state, which the
/// tighter per-unit ceilings already prevent under the current
/// fixed-length path scheme (see the core_import_charge_budget integration
/// tests).
fn store_charge_exceeds_budget(
    retained_receipt_charge_bytes: u64,
    receipt_charge_delta: u64,
    seen_index_charge_bytes: u64,
    live_entry_bytes: u64,
    namespace_charge_bytes: u64,
) -> bool {
    retained_receipt_charge_bytes
        .saturating_add(receipt_charge_delta)
        .saturating_add(seen_index_charge_bytes)
        .saturating_add(live_entry_bytes)
        .saturating_add(namespace_charge_bytes)
        > RETAINED_STORE_BUDGET_BYTES
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    SessionLimit,
    ObjectClosed,
    WrongSession,
    StalePreview,
    PlanSuperseded,
    PlanConsumed,
    PlanClosed,
    PreviewConsumed,
    NoEligibleEntries,
    EmptySelection,
    DuplicateSelection,
    UnknownSelection,
    StoreFull,
    /// Test-only injected pre-swap failure (proves rollback).
    Injected,
    Internal,
}

/// Full public identifiers recovered from canonical entry bytes. Consumers
/// that only need display/provenance facts do not receive a generic Willow
/// value or any signer material.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicEntryIdentity {
    pub namespace_id: [u8; 32],
    pub signer_id: [u8; 32],
}

/// Reads public namespace and signer identifiers from canonical entry bytes.
pub fn public_entry_identity(bytes: &[u8]) -> Result<PublicEntryIdentity, SessionError> {
    use willow25::groupings::{Keylike, Namespaced};

    let entry = decode_entry_canonic(bytes).map_err(|_| SessionError::Internal)?;
    Ok(PublicEntryIdentity {
        namespace_id: *entry.namespace_id().as_bytes(),
        signer_id: *entry.subspace_id().as_bytes(),
    })
}

/// Local import context: the route the bytes arrived by. Receipt time comes
/// from the session clock in a fuller build; Phase 0A records the route.
#[derive(Debug, Clone)]
pub struct ImportContext {
    pub route: String,
}

impl ImportContext {
    pub fn new(route: &str) -> Self {
        Self {
            route: route.to_string(),
        }
    }
}

/// Per-entry disposition recorded immutably in a receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryDisposition {
    AppliedAtCommit { pruned_entry_ids: Vec<EntryId> },
    DominatedAtCommit { dominating_entry_ids: Vec<EntryId> },
    AlreadyPresent { insertion_receipt_id: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispositionRow {
    pub entry_id: EntryId,
    pub disposition: EntryDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportReceipt {
    pub receipt_id: u64,
    pub route: String,
    pub before_generation: u64,
    pub after_generation: u64,
    pub dispositions: Vec<DispositionRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateResult {
    pub unchanged_generation: u64,
    pub entry_ids: Vec<EntryId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitOutcome {
    Committed(ImportReceipt),
    NoChanges(DuplicateResult),
}

/// Current liveness of an accepted entry, separate from its immutable
/// receipt disposition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiveStatus {
    Live,
    NotLive { dominated_on_arrival: bool },
}

/// Provenance separates cryptographic facts and local receipt facts from
/// trust and truth. It asserts no truth claim.
#[derive(Debug, Clone)]
pub struct Provenance {
    pub entry_id: EntryId,
    pub signature_valid: bool,
    pub capability_valid: bool,
    pub live_status: LiveStatus,
    pub import_route: String,
    pub first_receipt_id: u64,
    /// Always false: a valid signature is not a truth claim.
    pub asserts_truth: bool,
}

// ---------------------------------------------------------------------------
// Internal arbiter state.
// ---------------------------------------------------------------------------

struct StoreState {
    store_id: u64,
    generation: u64,
    join: JoinState,
    receipts: Vec<ImportReceipt>,
    /// entry id -> first receipt that accepted it, and its arrival disposition.
    first_receipt: Vec<(EntryId, u64, bool)>, // (id, receipt_id, dominated_on_arrival)
    next_receipt_id: u64,
    /// Permanent charge from every committed receipt's per-row overhead,
    /// digest references, and route bytes. Monotonic — receipt history is
    /// never pruned. Entry-index and live-entry-byte charge are added on top
    /// at read time via `join.seen_index_charge_bytes()` /
    /// `join.live_entry_bytes()`; namespace charge via `seen_namespaces`.
    retained_receipt_charge_bytes: u64,
    /// Distinct Willow namespace IDs ever observed by this store, bounded at
    /// `MAX_NAMESPACE_VIEWS`. `JoinState` can hold entries from more than
    /// one namespace at once (they simply never prune each other), so this
    /// is tracked independently of the join state.
    seen_namespaces: Vec<[u8; 32]>,
}

/// A verified, ready-to-commit entry captured at inspection time. Only
/// entries whose signature and capability already verified reach here, so
/// carrying the authorised entry is sufficient.
#[derive(Clone)]
struct VerifiedEntry {
    authorised: willow25::authorisation::AuthorisedEntry,
    entry_id: EntryId,
    /// Canonical entry byte length, captured once at verification time so
    /// preview-output charging never needs to re-encode.
    entry_bytes_len: usize,
    /// Payload bytes carried through to live retention for app-data and
    /// app-index entries (see `Stored::payload`); `None` for alerts. The
    /// preview owns the allocation and plans share it immutably, so the
    /// preview-output budget charges these bytes exactly once.
    payload: Option<Arc<[u8]>>,
}

struct PreviewState {
    preview_id: u64,
    base_generation: u64,
    entries: Vec<VerifiedEntry>,
    route: String,
    issued_plans: usize,
}

struct PlanState {
    plan_id: u64,
    preview_id: u64,
    entries: Vec<VerifiedEntry>,
    route: String,
}

#[derive(Clone, Copy)]
enum PlanTerminal {
    Superseded,
    Consumed,
    Closed,
}

struct PlanTombstone {
    plan_id: u64,
    terminal: PlanTerminal,
}

struct SessionState {
    session_id: u64,
    closed: bool,
    store: Option<StoreState>,
    store_closed: bool,
    preview: Option<PreviewState>,
    plan: Option<PlanState>,
    /// Terminal records belong only to the live preview, whose issuance
    /// budget bounds this vector at `MAX_PLANS_PER_PREVIEW`.
    plan_tombstones: Vec<PlanTombstone>,
    next_id: u64,
}

impl SessionState {
    fn alloc_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    fn require_open_store(&self) -> Result<(), SessionError> {
        if self.closed {
            return Err(SessionError::ObjectClosed);
        }
        if self.store_closed || self.store.is_none() {
            return Err(SessionError::ObjectClosed);
        }
        Ok(())
    }

    /// Admission for a store handle: the session and store must be open and
    /// the handle's `store_id` must match the live store (a foreign or stale
    /// store handle is `WrongSession`).
    fn require_store(&self, store_id: u64) -> Result<(), SessionError> {
        self.require_open_store()?;
        if self.store.as_ref().unwrap().store_id != store_id {
            return Err(SessionError::WrongSession);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Handles.
// ---------------------------------------------------------------------------

pub struct RiotSession {
    inner: Arc<Mutex<SessionState>>,
}

impl RiotSession {
    /// Opens a session. Phase 0A needs no configuration; a fuller build would
    /// take a `CoreConfig` and could fail on entropy.
    pub fn open() -> Result<Self, SessionError> {
        // Deterministic per-process session id from a monotonic counter kept
        // behind the arbiter; distinct sessions get distinct ids.
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let session_id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(Self {
            inner: Arc::new(Mutex::new(SessionState {
                session_id,
                closed: false,
                store: None,
                store_closed: false,
                preview: None,
                plan: None,
                plan_tombstones: Vec::new(),
                next_id: 0,
            })),
        })
    }

    pub fn create_store(&self) -> Result<EvidenceStore, SessionError> {
        let mut st = self.inner.lock().map_err(|_| SessionError::Internal)?;
        if st.closed {
            return Err(SessionError::ObjectClosed);
        }
        if st.store.is_some() {
            return Err(SessionError::SessionLimit);
        }
        let store_id = st.alloc_id();
        st.store = Some(StoreState {
            store_id,
            generation: 0,
            join: JoinState::new(),
            receipts: Vec::new(),
            first_receipt: Vec::new(),
            next_receipt_id: 1,
            retained_receipt_charge_bytes: 0,
            seen_namespaces: Vec::new(),
        });
        st.store_closed = false;
        Ok(EvidenceStore {
            inner: Arc::clone(&self.inner),
            store_id,
        })
    }
}

pub struct EvidenceStore {
    inner: Arc<Mutex<SessionState>>,
    store_id: u64,
}

pub enum InspectOutcome {
    Preview(ImportPreview),
    Rejected(BundleRejection),
}

enum InspectInnerError {
    Busy,
    Session(SessionError),
}

impl From<SessionError> for InspectInnerError {
    fn from(error: SessionError) -> Self {
        Self::Session(error)
    }
}

impl InspectOutcome {
    /// Test convenience: unwrap a Preview or panic.
    pub fn expect_preview(self) -> ImportPreview {
        match self {
            InspectOutcome::Preview(p) => p,
            InspectOutcome::Rejected(r) => panic!("expected preview, got rejection: {r:?}"),
        }
    }
}

impl EvidenceStore {
    pub fn session_id(&self) -> Result<u64, SessionError> {
        let st = self.inner.lock().map_err(|_| SessionError::Internal)?;
        Ok(st.session_id)
    }

    pub fn generation(&self) -> Result<u64, SessionError> {
        let st = self.inner.lock().map_err(|_| SessionError::Internal)?;
        st.require_store(self.store_id)?;
        Ok(st.store.as_ref().unwrap().generation)
    }

    pub fn live_count(&self) -> Result<usize, SessionError> {
        let st = self.inner.lock().map_err(|_| SessionError::Internal)?;
        st.require_store(self.store_id)?;
        Ok(st.store.as_ref().unwrap().join.live_ids().len())
    }

    /// Complete canonical IDs for the current live view. This keeps callers
    /// on a typed public identity boundary rather than exposing stored Willow
    /// values or signer state.
    pub fn live_entry_ids(&self) -> Result<Vec<EntryId>, SessionError> {
        let st = self.inner.lock().map_err(|_| SessionError::Internal)?;
        st.require_store(self.store_id)?;
        Ok(st.store.as_ref().unwrap().join.live_ids())
    }

    /// Live entries whose path is prefixed by `prefix`, with their canonical
    /// ids and retained payload bytes (`Some` for app-data and app-index
    /// entries).
    /// Same typed boundary as `live_entry_ids`; the returned `Entry` carries
    /// the payload digest/length, never signer or capability state.
    pub fn entries_with_prefix(
        &self,
        prefix: &crate::willow::Path,
    ) -> Result<Vec<crate::import::join::PrefixedEntry>, SessionError> {
        let st = self.inner.lock().map_err(|_| SessionError::Internal)?;
        st.require_store(self.store_id)?;
        Ok(st
            .store
            .as_ref()
            .unwrap()
            .join
            .live_entries_with_prefix(prefix))
    }

    pub fn receipt_count(&self) -> Result<usize, SessionError> {
        let st = self.inner.lock().map_err(|_| SessionError::Internal)?;
        st.require_store(self.store_id)?;
        Ok(st.store.as_ref().unwrap().receipts.len())
    }

    #[cfg(feature = "conformance")]
    #[doc(hidden)]
    pub fn retained_plan_tombstone_count_for_conformance(&self) -> Result<usize, SessionError> {
        let st = self.inner.lock().map_err(|_| SessionError::Internal)?;
        st.require_store(self.store_id)?;
        Ok(st.plan_tombstones.len())
    }

    /// Total retained byte-charge: permanent receipt/reference history plus
    /// currently live entries. See `store_charge_exceeds_budget`.
    #[cfg(feature = "conformance")]
    #[doc(hidden)]
    pub fn retained_store_charge_bytes_for_conformance(&self) -> Result<u64, SessionError> {
        let st = self.inner.lock().map_err(|_| SessionError::Internal)?;
        st.require_store(self.store_id)?;
        let store = st.store.as_ref().unwrap();
        Ok(store.retained_receipt_charge_bytes
            + store.join.seen_index_charge_bytes()
            + store.join.live_entry_bytes()
            + store.seen_namespaces.len() as u64 * STORE_CHARGE_NAMESPACE_BYTES)
    }

    pub fn close(&self) -> Result<(), SessionError> {
        let mut st = self.inner.lock().map_err(|_| SessionError::Internal)?;
        if st.closed {
            return Err(SessionError::ObjectClosed);
        }
        // Closing the store closes its preview and plan in the same section.
        st.store_closed = true;
        st.preview = None;
        st.plan = None;
        Ok(())
    }

    /// Inspect an artifact: decode + verify, and (on success) install a
    /// preview bound to the current store generation.
    pub fn inspect(
        &self,
        bytes: &[u8],
        context: ImportContext,
    ) -> Result<InspectOutcome, SessionError> {
        self.inspect_inner(bytes, context, true)
            .map_err(|error| match error {
                InspectInnerError::Session(error) => error,
                InspectInnerError::Busy => SessionError::Internal,
            })
    }

    /// Local writes must never replace a pending user or sync review.
    fn inspect_local_write(
        &self,
        bytes: &[u8],
        context: ImportContext,
    ) -> Result<InspectOutcome, InspectInnerError> {
        self.inspect_inner(bytes, context, false)
    }

    /// Release only the helper-owned preview; if another inspect replaced it
    /// concurrently, leave that newer review untouched.
    fn finish_local_write(&self, preview_id: u64) -> Result<(), SessionError> {
        let mut st = self.inner.lock().map_err(|_| SessionError::Internal)?;
        st.require_store(self.store_id)?;
        if st
            .preview
            .as_ref()
            .is_some_and(|preview| preview.preview_id == preview_id)
        {
            st.plan = None;
            st.plan_tombstones.clear();
            st.preview = None;
        }
        Ok(())
    }

    fn inspect_inner(
        &self,
        bytes: &[u8],
        context: ImportContext,
        replace_active_review: bool,
    ) -> Result<InspectOutcome, InspectInnerError> {
        let mut st = self.inner.lock().map_err(|_| SessionError::Internal)?;
        st.require_store(self.store_id)?;
        if !replace_active_review && (st.preview.is_some() || st.plan.is_some()) {
            return Err(InspectInnerError::Busy);
        }

        // Decode + verify OUTSIDE any state mutation.
        let decoded = match decode_bundle(bytes) {
            BundleDecodeOutcome::Rejected(rejection) => {
                return Ok(InspectOutcome::Rejected(rejection));
            }
            BundleDecodeOutcome::Decoded(decoded) => decoded,
        };

        let mut verified = Vec::new();
        for item in &decoded.items {
            if let ItemStatus::Valid(valid) = &item.status {
                // Reconstruct the AuthorisedEntry via the checked conversion
                // from the already-verified component bytes.
                let entry = decode_entry_canonic(item.frame.entry_bytes())
                    .map_err(|_| SessionError::Internal)?;
                let capability = decode_capability_canonic(item.frame.capability_bytes())
                    .map_err(|_| SessionError::Internal)?;
                let sig: [u8; 64] = item
                    .frame
                    .signature_bytes()
                    .try_into()
                    .map_err(|_| SessionError::Internal)?;
                let token = AuthorisationToken::new(
                    capability,
                    willow25::entry::SubspaceSignature::from(sig),
                );
                let authorised = PossiblyAuthorisedEntry::new(entry, token)
                    .into_authorised_entry()
                    .map_err(|_| SessionError::Internal)?;
                // A valid signature only proves the author signed these
                // exact bytes, not that the Willow path they chose actually
                // describes the payload underneath it. Without this check a
                // validly signed entry could bind an arbitrary path to
                // content it doesn't describe. App-data entries are exempt
                // from the alert binding: their payload is opaque and embeds
                // no identity a path could contradict — the path itself
                // (already shape-checked in `verify_frame`) is the identity.
                // App-index manifest/bundle slots are likewise exempt (their
                // payload schema was checked in `verify_frame`; their app ID
                // is the path component itself), but endorsement and trust
                // slots belong to exactly the subspace named in their path:
                // the entry's own subspace must equal that identity component,
                // so nobody can write into someone else's slot.
                let is_app_data = crate::apps::entry::is_app_data_path(
                    willow25::groupings::Keylike::path(authorised.entry()),
                );
                let app_index_slot = crate::apps::index::classify_app_index_path(
                    willow25::groupings::Keylike::path(authorised.entry()),
                );
                let path_matches = if is_app_data {
                    true
                } else if let Some(slot) = app_index_slot {
                    match slot {
                        crate::apps::index::AppIndexSlot::Endorsement {
                            endorser_subspace_id,
                            ..
                        } => {
                            *willow25::groupings::Keylike::subspace_id(authorised.entry())
                                .as_bytes()
                                == endorser_subspace_id
                        }
                        crate::apps::index::AppIndexSlot::Trust {
                            organizer_subspace_id,
                            ..
                        } => {
                            *willow25::groupings::Keylike::subspace_id(authorised.entry())
                                .as_bytes()
                                == organizer_subspace_id
                        }
                        crate::apps::index::AppIndexSlot::Manifest { .. }
                        | crate::apps::index::AppIndexSlot::Bundle { .. } => true,
                    }
                } else {
                    decode_alert(item.frame.payload_bytes())
                        .ok()
                        .and_then(|alert| {
                            alert_entry_path_matches_payload(
                                item.frame.entry_bytes(),
                                &alert.object_id,
                                &alert.revision_id,
                            )
                            .ok()
                        })
                        .unwrap_or(false)
                };
                if path_matches {
                    // App-data and app-index payloads are retained with the
                    // live entry (see `Stored::payload`): apps read their
                    // values back, and the directory scan reads manifests/
                    // bundles/endorsements/trust markers back. Alert payloads stay
                    // digest-only.
                    let retain_payload = is_app_data || app_index_slot.is_some();
                    verified.push(VerifiedEntry {
                        authorised,
                        entry_id: valid.entry_id,
                        entry_bytes_len: item.frame.entry_bytes().len(),
                        payload: retain_payload
                            .then(|| Arc::<[u8]>::from(item.frame.payload_bytes())),
                    });
                }
            }
            // Ineligible items are simply not carried into the preview.
        }

        // A live preview retains its verified entries and route for its
        // whole lifetime, independent of whether it is ever committed; that
        // retained cost must be bounded on its own, separately from the
        // store's permanent budget.
        if preview_output_exceeds_budget(preview_output_entries_charge_bytes(
            &verified,
            context.route.len(),
        )) {
            return Err(SessionError::StoreFull.into());
        }

        let store = st.store.as_ref().unwrap();
        let base_generation = store.generation;
        let preview_id = st.alloc_id();
        let eligible = verified.len();
        // Replacing a preview consumes every child plan. Drop the active
        // plan and all outgoing terminal records together; old plan handles
        // detect that their parent preview is no longer live.
        st.plan = None;
        st.plan_tombstones.clear();
        st.preview = Some(PreviewState {
            preview_id,
            base_generation,
            entries: verified,
            route: context.route.clone(),
            issued_plans: 0,
        });

        Ok(InspectOutcome::Preview(ImportPreview {
            inner: Arc::clone(&self.inner),
            preview_id,
            session_id: st.session_id,
            base_generation,
            eligible,
        }))
    }

    /// Provenance for an accepted entry: cryptographic facts + local receipt
    /// facts + current live status. No truth claim.
    pub fn provenance(&self, entry_id: &EntryId) -> Result<Provenance, SessionError> {
        let st = self.inner.lock().map_err(|_| SessionError::Internal)?;
        st.require_store(self.store_id)?;
        let store = st.store.as_ref().unwrap();
        let (first_receipt_id, dominated_on_arrival) = store
            .first_receipt
            .iter()
            .find(|(id, _, _)| id == entry_id)
            .map(|(_, rid, dom)| (*rid, *dom))
            .ok_or(SessionError::Internal)?;
        let live = store.join.is_live_id(entry_id);
        Ok(Provenance {
            entry_id: *entry_id,
            signature_valid: true,
            capability_valid: true,
            live_status: if live {
                LiveStatus::Live
            } else {
                LiveStatus::NotLive {
                    dominated_on_arrival,
                }
            },
            import_route: store
                .receipts
                .iter()
                .find(|r| r.receipt_id == first_receipt_id)
                .map(|r| r.route.clone())
                .unwrap_or_default(),
            first_receipt_id,
            asserts_truth: false,
        })
    }
}

/// Sign and commit one payload at an exact Willow path through the standard
/// admission pipeline. App-index writers share this boundary so local writes
/// cannot bypass the same schema and path-binding gates as synced entries.
pub(crate) fn commit_at(
    store: &EvidenceStore,
    author: &crate::willow::identity::EvidenceAuthor,
    path: &crate::willow::Path,
    payload: &[u8],
    willow_timestamp_micros: u64,
) -> Result<(), crate::apps::AppsError> {
    let entry = crate::willow::Entry::builder()
        .namespace_id(author.namespace_id().clone())
        .subspace_id(author.subspace_id())
        .path(path.clone())
        .timestamp(willow_timestamp_micros)
        .payload(payload)
        .build();
    let authorised = crate::willow::authorise_entry(author, entry)?;
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    let signed = crate::willow::SignedWillowEntry {
        entry_bytes: crate::willow::encode_entry(authorised.entry()),
        capability_bytes: crate::willow::encode_capability(token.capability()),
        signature: signature.to_bytes(),
        payload_bytes: payload.to_vec(),
    };
    let bundle = crate::import::bundle::encode_bundle(std::slice::from_ref(&signed))
        .map_err(|_| crate::apps::AppsError::StoreRejected)?;
    let preview = match store.inspect_local_write(&bundle, ImportContext::new("app-index-write")) {
        Ok(InspectOutcome::Preview(preview)) => preview,
        Ok(InspectOutcome::Rejected(_)) => return Err(crate::apps::AppsError::StoreRejected),
        Err(InspectInnerError::Busy) => return Err(crate::apps::AppsError::StoreBusy),
        Err(InspectInnerError::Session(_)) => return Err(crate::apps::AppsError::StoreRejected),
    };
    let local_preview_id = preview.preview_id;
    let plan = match preview.plan_all() {
        Ok(plan) => plan,
        Err(_) => {
            let _ = store.finish_local_write(local_preview_id);
            return Err(crate::apps::AppsError::StoreRejected);
        }
    };
    let outcome = plan.commit();
    store
        .finish_local_write(local_preview_id)
        .map_err(|_| crate::apps::AppsError::StoreRejected)?;
    match outcome.map_err(|_| crate::apps::AppsError::StoreRejected)? {
        // With one submitted entry, duplicate-only `NoChanges` proves the
        // exact canonical entry id was already present: safe idempotence.
        CommitOutcome::NoChanges(_) => Ok(()),
        CommitOutcome::Committed(receipt) => match receipt.dispositions.as_slice() {
            [DispositionRow {
                disposition: EntryDisposition::AppliedAtCommit { .. },
                ..
            }] => Ok(()),
            [DispositionRow {
                disposition: EntryDisposition::DominatedAtCommit { .. },
                ..
            }] => Err(crate::apps::AppsError::StaleWrite),
            _ => Err(crate::apps::AppsError::StoreRejected),
        },
    }
}

pub struct ImportPreview {
    inner: Arc<Mutex<SessionState>>,
    preview_id: u64,
    session_id: u64,
    base_generation: u64,
    eligible: usize,
}

/// Explicitly names the eligible canonical entry IDs a plan may retain.
/// `all` is provided only for `plan_all`; callers selecting entries use
/// `new` and receive a typed error for empty, duplicate, or unknown IDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportSelection {
    All,
    Entries(Vec<EntryId>),
}

impl ImportSelection {
    pub fn all() -> Self {
        Self::All
    }

    pub fn new(entry_ids: Vec<EntryId>) -> Self {
        Self::Entries(entry_ids)
    }
}

impl ImportPreview {
    pub fn session_id(&self) -> u64 {
        self.session_id
    }

    pub fn eligible_count(&self) -> Result<usize, SessionError> {
        Ok(self.eligible)
    }

    pub fn all_unknown_trust(&self) -> Result<bool, SessionError> {
        // Phase 0A carries no trust set into inspect, so every eligible entry
        // is UnknownTrust. The facts (signature/capability) are still valid.
        Ok(true)
    }

    /// Plans exactly the selected eligible entries. Selection validation has
    /// no state effect, while issuing a valid replacement supersedes the
    /// previously active plan for this preview.
    pub fn plan(&self, selection: ImportSelection) -> Result<ImportPlan, SessionError> {
        let mut st = self.inner.lock().map_err(|_| SessionError::Internal)?;
        st.require_open_store()?;
        match &st.preview {
            Some(p) if p.preview_id == self.preview_id => {}
            _ => return Err(SessionError::PreviewConsumed),
        }
        // Any commit since inspection makes a live preview stale.
        if st.store.as_ref().unwrap().generation != self.base_generation {
            return Err(SessionError::StalePreview);
        }
        let (entries, route, base_generation) = {
            let p = st.preview.as_ref().unwrap();
            let entries = match selection {
                ImportSelection::All => {
                    if p.entries.is_empty() {
                        return Err(SessionError::NoEligibleEntries);
                    }
                    p.entries.clone()
                }
                ImportSelection::Entries(entry_ids) => {
                    if entry_ids.is_empty() {
                        return Err(SessionError::EmptySelection);
                    }
                    if entry_ids
                        .iter()
                        .enumerate()
                        .any(|(index, id)| entry_ids[..index].contains(id))
                    {
                        return Err(SessionError::DuplicateSelection);
                    }
                    let mut entries = Vec::with_capacity(entry_ids.len());
                    for entry_id in entry_ids {
                        let entry = p
                            .entries
                            .iter()
                            .find(|entry| entry.entry_id == entry_id)
                            .ok_or(SessionError::UnknownSelection)?;
                        entries.push(entry.clone());
                    }
                    entries
                }
            };
            if p.issued_plans >= MAX_PLANS_PER_PREVIEW {
                return Err(SessionError::SessionLimit);
            }
            (entries, p.route.clone(), p.base_generation)
        };
        // The new plan retains its own selected-entry metadata and route on
        // top of the still-live preview. Immutable payload allocations stay
        // shared with that preview; superseding the current plan (if any)
        // adds one more tombstone. Every unique retained allocation counts
        // against the same preview-output budget.
        let prospective_tombstones = st.plan_tombstones.len() + usize::from(st.plan.is_some());
        let preview_charge = {
            let p = st.preview.as_ref().unwrap();
            preview_output_entries_charge_bytes(&p.entries, p.route.len())
        };
        let plan_charge = plan_output_entries_charge_bytes(&entries, route.len());
        if preview_output_exceeds_budget(
            preview_charge + plan_charge + prospective_tombstones as u64 * PLAN_TOMBSTONE_BYTES,
        ) {
            return Err(SessionError::StoreFull);
        }
        let plan_id = st.alloc_id();
        st.preview.as_mut().unwrap().issued_plans += 1;
        supersede_active_plan(&mut st);
        st.plan = Some(PlanState {
            plan_id,
            preview_id: self.preview_id,
            entries,
            route,
        });
        Ok(ImportPlan {
            inner: Arc::clone(&self.inner),
            plan_id,
            preview_id: self.preview_id,
            base_generation,
        })
    }

    /// Plan every eligible entry through the same selection path used by
    /// explicit callers.
    pub fn plan_all(&self) -> Result<ImportPlan, SessionError> {
        self.plan(ImportSelection::all())
    }
}

pub struct ImportPlan {
    inner: Arc<Mutex<SessionState>>,
    plan_id: u64,
    preview_id: u64,
    base_generation: u64,
}

impl ImportPlan {
    /// Explicitly terminates the current, unconsumed plan without mutating
    /// the store or creating a receipt. The preview may issue a replacement.
    pub fn close(&self) -> Result<(), SessionError> {
        let mut st = self.inner.lock().map_err(|_| SessionError::Internal)?;
        st.require_open_store()?;
        if !plan_parent_is_live(&st, self.preview_id) {
            return Err(SessionError::PreviewConsumed);
        }
        match &st.plan {
            Some(p) if p.plan_id == self.plan_id && p.preview_id == self.preview_id => {
                terminate_plan(&mut st, self.plan_id, PlanTerminal::Closed);
                Ok(())
            }
            _ => Err(plan_terminal_error(&st, self.plan_id)),
        }
    }

    pub fn commit(&self) -> Result<CommitOutcome, SessionError> {
        self.commit_inner(false)
    }

    /// Test-only: build the next snapshot, then fail before the pointer swap.
    /// Proves logical atomicity — the store is byte-for-byte unchanged.
    pub fn commit_with_injected_failure_for_tests(&self) -> Result<CommitOutcome, SessionError> {
        self.commit_inner(true)
    }

    fn commit_inner(&self, inject_failure: bool) -> Result<CommitOutcome, SessionError> {
        let mut st = self.inner.lock().map_err(|_| SessionError::Internal)?;
        st.require_open_store()?;

        // Admission: plan must be current and unconsumed.
        if !plan_parent_is_live(&st, self.preview_id) {
            return Err(SessionError::PreviewConsumed);
        }
        match &st.plan {
            Some(p) if p.plan_id == self.plan_id && p.preview_id == self.preview_id => {}
            _ => return Err(plan_terminal_error(&st, self.plan_id)),
        }
        if st.store.as_ref().unwrap().generation != self.base_generation {
            return Err(SessionError::StalePreview);
        }

        // Snapshot the inputs, then compute the next join state on a CLONE.
        let (entries, route) = {
            let p = st.plan.as_ref().unwrap();
            (p.entries.clone(), p.route.clone())
        };
        let store = st.store.as_ref().unwrap();
        let before_generation = store.generation;
        let pre_join = store.join.clone();

        let batch = entries
            .iter()
            .map(|v| {
                (
                    v.authorised.clone(),
                    v.payload.as_ref().map(|payload| payload.to_vec()),
                )
            })
            .collect();
        let join_plan =
            plan_join_with_payloads(&pre_join, batch).map_err(|_| SessionError::StoreFull)?;

        // Any entry whose id was not previously seen makes this a real change.
        let any_new = join_plan
            .effects
            .iter()
            .any(|(_, e)| !matches!(e, JoinEffect::AlreadyPresent));

        if !any_new {
            // Duplicate-only: no swap, no generation change, no receipt. The
            // plan is consumed regardless.
            terminate_plan(&mut st, self.plan_id, PlanTerminal::Consumed);
            return Ok(CommitOutcome::NoChanges(DuplicateResult {
                unchanged_generation: before_generation,
                entry_ids: join_plan.effects.iter().map(|(id, _)| *id).collect(),
            }));
        }

        // Build the receipt from the effects (still no mutation).
        let store = st.store.as_ref().unwrap();
        let receipt_id = store.next_receipt_id;
        let after_generation = before_generation + 1;
        let mut dispositions = Vec::with_capacity(join_plan.effects.len());
        let mut newly_first: Vec<(EntryId, u64, bool)> = Vec::new();
        for (entry_id, effect) in &join_plan.effects {
            let disposition = match effect {
                JoinEffect::Winner { pruned_entry_ids } => {
                    newly_first.push((*entry_id, receipt_id, false));
                    EntryDisposition::AppliedAtCommit {
                        pruned_entry_ids: pruned_entry_ids.clone(),
                    }
                }
                JoinEffect::NotLive {
                    dominating_entry_ids,
                } => {
                    newly_first.push((*entry_id, receipt_id, true));
                    EntryDisposition::DominatedAtCommit {
                        dominating_entry_ids: dominating_entry_ids.clone(),
                    }
                }
                JoinEffect::AlreadyPresent => {
                    let prior = store
                        .first_receipt
                        .iter()
                        .find(|(id, _, _)| id == entry_id)
                        .map(|(_, rid, _)| *rid)
                        .unwrap_or(receipt_id);
                    EntryDisposition::AlreadyPresent {
                        insertion_receipt_id: prior,
                    }
                }
            };
            dispositions.push(DispositionRow {
                entry_id: *entry_id,
                disposition,
            });
        }
        let receipt = ImportReceipt {
            receipt_id,
            route,
            before_generation,
            after_generation,
            dispositions,
        };

        // Injected failure: everything above was on the clone; return before
        // touching store state.
        if inject_failure {
            terminate_plan(&mut st, self.plan_id, PlanTerminal::Consumed);
            return Err(SessionError::Injected);
        }

        // Commit: one pointer swap installs the new live set, generation,
        // receipt, and first-receipt records.
        let store = st.store.as_mut().unwrap();
        if store.receipts.len() >= MAX_RECEIPTS {
            terminate_plan(&mut st, self.plan_id, PlanTerminal::Consumed);
            return Err(SessionError::StoreFull);
        }
        let reference_count: u64 = receipt
            .dispositions
            .iter()
            .map(|row| match &row.disposition {
                EntryDisposition::AppliedAtCommit { pruned_entry_ids } => {
                    pruned_entry_ids.len() as u64
                }
                EntryDisposition::DominatedAtCommit {
                    dominating_entry_ids,
                } => dominating_entry_ids.len() as u64,
                EntryDisposition::AlreadyPresent { .. } => 0,
            })
            .sum();
        // Charged once per retained DispositionRow (not once per receipt):
        // a receipt's Vec<DispositionRow> genuinely grows with row count.
        // The route is retained once per receipt, so its bytes are charged
        // once here rather than per row.
        let receipt_charge_delta = receipt.dispositions.len() as u64 * STORE_CHARGE_RECEIPT_BYTES
            + reference_count * STORE_CHARGE_DIGEST_REFERENCE_BYTES
            + receipt.route.len() as u64;

        // Every distinct Willow namespace this batch introduces charges once
        // and counts against the namespace_views ceiling. An AlreadyPresent
        // entry's namespace was necessarily already seen, so scanning the
        // whole batch (not just new winners) cannot spuriously inflate this.
        let new_namespaces: Vec<[u8; 32]> = {
            use willow25::groupings::Namespaced;
            let mut found = Vec::new();
            for v in &entries {
                let namespace_id = *v.authorised.entry().namespace_id().as_bytes();
                if !store.seen_namespaces.contains(&namespace_id) && !found.contains(&namespace_id)
                {
                    found.push(namespace_id);
                }
            }
            found
        };
        if store.seen_namespaces.len() + new_namespaces.len() > MAX_NAMESPACE_VIEWS {
            terminate_plan(&mut st, self.plan_id, PlanTerminal::Consumed);
            return Err(SessionError::StoreFull);
        }
        let namespace_charge_bytes = (store.seen_namespaces.len() + new_namespaces.len()) as u64
            * STORE_CHARGE_NAMESPACE_BYTES;

        if store_charge_exceeds_budget(
            store.retained_receipt_charge_bytes,
            receipt_charge_delta,
            join_plan.next.seen_index_charge_bytes(),
            join_plan.next.live_entry_bytes(),
            namespace_charge_bytes,
        ) {
            terminate_plan(&mut st, self.plan_id, PlanTerminal::Consumed);
            return Err(SessionError::StoreFull);
        }
        store.join = join_plan.next;
        store.generation = after_generation;
        store.retained_receipt_charge_bytes += receipt_charge_delta;
        store.seen_namespaces.extend(new_namespaces);
        for rec in newly_first {
            if !store.first_receipt.iter().any(|(id, _, _)| *id == rec.0) {
                store.first_receipt.push(rec);
            }
        }
        store.receipts.push(receipt.clone());
        store.next_receipt_id += 1;

        terminate_plan(&mut st, self.plan_id, PlanTerminal::Consumed);
        Ok(CommitOutcome::Committed(receipt))
    }
}

fn supersede_active_plan(st: &mut SessionState) {
    if let Some(plan) = st.plan.take() {
        record_plan_terminal(st, plan.plan_id, PlanTerminal::Superseded);
    }
}

fn plan_parent_is_live(st: &SessionState, preview_id: u64) -> bool {
    matches!(st.preview.as_ref(), Some(preview) if preview.preview_id == preview_id)
}

fn terminate_plan(st: &mut SessionState, plan_id: u64, terminal: PlanTerminal) {
    if matches!(st.plan.as_ref(), Some(plan) if plan.plan_id == plan_id) {
        st.plan = None;
        record_plan_terminal(st, plan_id, terminal);
    }
}

fn record_plan_terminal(st: &mut SessionState, plan_id: u64, terminal: PlanTerminal) {
    // Terminal errors remain durable for the current preview. Its issuance
    // budget bounds this vector at `MAX_PLANS_PER_PREVIEW`; replacement
    // clears the old preview's records atomically with its active plan.
    st.plan_tombstones.push(PlanTombstone { plan_id, terminal });
}

fn plan_terminal_error(st: &SessionState, plan_id: u64) -> SessionError {
    match st
        .plan_tombstones
        .iter()
        .rev()
        .find(|tombstone| tombstone.plan_id == plan_id)
        .map(|tombstone| tombstone.terminal)
    {
        Some(PlanTerminal::Superseded) => SessionError::PlanSuperseded,
        Some(PlanTerminal::Closed) => SessionError::PlanClosed,
        Some(PlanTerminal::Consumed) | None => SessionError::PlanConsumed,
    }
}

#[cfg(test)]
mod charge_budget_tests {
    use super::{
        preview_output_exceeds_budget, store_charge_exceeds_budget, PREVIEW_OUTPUT_BUDGET_BYTES,
        RETAINED_STORE_BUDGET_BYTES,
    };

    #[test]
    fn preview_output_exceeds_budget_holds_the_exact_ceiling_and_rejects_one_byte_over() {
        assert!(!preview_output_exceeds_budget(PREVIEW_OUTPUT_BUDGET_BYTES));
        assert!(preview_output_exceeds_budget(
            PREVIEW_OUTPUT_BUDGET_BYTES + 1
        ));
    }

    #[test]
    fn store_charge_exceeds_budget_holds_the_exact_ceiling_and_rejects_one_byte_over() {
        // Exactly at the ceiling: not exceeded.
        assert!(!store_charge_exceeds_budget(
            RETAINED_STORE_BUDGET_BYTES - 100,
            30,
            30,
            20,
            20
        ));
        // One byte over the ceiling: exceeded.
        assert!(store_charge_exceeds_budget(
            RETAINED_STORE_BUDGET_BYTES - 100,
            30,
            30,
            20,
            21
        ));
    }

    #[test]
    fn store_charge_exceeds_budget_sums_all_five_components_without_overflow_panics() {
        assert!(store_charge_exceeds_budget(
            u64::MAX,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            u64::MAX
        ));
        assert!(!store_charge_exceeds_budget(0, 0, 0, 0, 0));
    }
}
