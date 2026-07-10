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
use crate::import::join::{plan_join, JoinEffect, JoinState};
use crate::willow::{decode_capability_canonic, decode_entry_canonic, AuthorisationToken, EntryId};
use willow25::authorisation::PossiblyAuthorisedEntry;

/// Ceilings from fixtures/manifest.json.
const MAX_RECEIPTS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    SessionLimit,
    ObjectClosed,
    WrongSession,
    StalePreview,
    PlanConsumed,
    PreviewConsumed,
    NoEligibleEntries,
    StoreFull,
    /// Test-only injected pre-swap failure (proves rollback).
    Injected,
    Internal,
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
}

/// A verified, ready-to-commit entry captured at inspection time. Only
/// entries whose signature and capability already verified reach here, so
/// carrying the authorised entry is sufficient.
#[derive(Clone)]
struct VerifiedEntry {
    authorised: willow25::authorisation::AuthorisedEntry,
}

struct PreviewState {
    preview_id: u64,
    base_generation: u64,
    entries: Vec<VerifiedEntry>,
    route: String,
}

struct PlanState {
    plan_id: u64,
    entries: Vec<VerifiedEntry>,
    route: String,
    consumed: bool,
}

struct SessionState {
    session_id: u64,
    closed: bool,
    store: Option<StoreState>,
    store_closed: bool,
    preview: Option<PreviewState>,
    plan: Option<PlanState>,
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

    pub fn receipt_count(&self) -> Result<usize, SessionError> {
        let st = self.inner.lock().map_err(|_| SessionError::Internal)?;
        st.require_store(self.store_id)?;
        Ok(st.store.as_ref().unwrap().receipts.len())
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
        let mut st = self.inner.lock().map_err(|_| SessionError::Internal)?;
        st.require_store(self.store_id)?;

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
                let _ = valid.entry_id; // verified id available; join recomputes it
                verified.push(VerifiedEntry { authorised });
            }
            // Ineligible items are simply not carried into the preview.
        }

        let store = st.store.as_ref().unwrap();
        let base_generation = store.generation;
        let preview_id = st.alloc_id();
        st.preview = Some(PreviewState {
            preview_id,
            base_generation,
            entries: verified.clone(),
            route: context.route.clone(),
        });
        // A new inspection supersedes any prior plan.
        st.plan = None;

        Ok(InspectOutcome::Preview(ImportPreview {
            inner: Arc::clone(&self.inner),
            preview_id,
            session_id: st.session_id,
            base_generation,
            eligible: verified.len(),
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

pub struct ImportPreview {
    inner: Arc<Mutex<SessionState>>,
    preview_id: u64,
    session_id: u64,
    base_generation: u64,
    eligible: usize,
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

    /// Plan to apply every eligible entry. Fails StalePreview if the store
    /// generation advanced, or if this preview was superseded.
    pub fn plan_all(&self) -> Result<ImportPlan, SessionError> {
        let mut st = self.inner.lock().map_err(|_| SessionError::Internal)?;
        st.require_open_store()?;
        // Generation guard first: any commit since inspection makes us stale.
        if st.store.as_ref().unwrap().generation != self.base_generation {
            return Err(SessionError::StalePreview);
        }
        match &st.preview {
            Some(p) if p.preview_id == self.preview_id => {}
            _ => return Err(SessionError::StalePreview),
        }
        if self.eligible == 0 {
            return Err(SessionError::NoEligibleEntries);
        }
        let (entries, route, base_generation) = {
            let p = st.preview.as_ref().unwrap();
            (p.entries.clone(), p.route.clone(), p.base_generation)
        };
        let plan_id = st.alloc_id();
        let _ = base_generation;
        st.plan = Some(PlanState {
            plan_id,
            entries,
            route,
            consumed: false,
        });
        Ok(ImportPlan {
            inner: Arc::clone(&self.inner),
            plan_id,
            base_generation,
        })
    }
}

pub struct ImportPlan {
    inner: Arc<Mutex<SessionState>>,
    plan_id: u64,
    base_generation: u64,
}

impl ImportPlan {
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
        match &st.plan {
            Some(p) if p.plan_id == self.plan_id => {
                if p.consumed {
                    return Err(SessionError::PlanConsumed);
                }
            }
            _ => return Err(SessionError::PlanConsumed),
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

        let batch = entries.iter().map(|v| v.authorised.clone()).collect();
        let join_plan = plan_join(&pre_join, batch);

        // Any entry whose id was not previously seen makes this a real change.
        let any_new = join_plan
            .effects
            .iter()
            .any(|(_, e)| !matches!(e, JoinEffect::AlreadyPresent));

        if !any_new {
            // Duplicate-only: no swap, no generation change, no receipt. The
            // plan is consumed regardless.
            mark_plan_consumed(&mut st, self.plan_id);
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
            mark_plan_consumed(&mut st, self.plan_id);
            return Err(SessionError::Injected);
        }

        // Commit: one pointer swap installs the new live set, generation,
        // receipt, and first-receipt records.
        let store = st.store.as_mut().unwrap();
        if store.receipts.len() >= MAX_RECEIPTS {
            mark_plan_consumed(&mut st, self.plan_id);
            return Err(SessionError::StoreFull);
        }
        store.join = join_plan.next;
        store.generation = after_generation;
        for rec in newly_first {
            if !store.first_receipt.iter().any(|(id, _, _)| *id == rec.0) {
                store.first_receipt.push(rec);
            }
        }
        store.receipts.push(receipt.clone());
        store.next_receipt_id += 1;

        mark_plan_consumed(&mut st, self.plan_id);
        Ok(CommitOutcome::Committed(receipt))
    }
}

fn mark_plan_consumed(st: &mut SessionState, plan_id: u64) {
    if let Some(p) = st.plan.as_mut() {
        if p.plan_id == plan_id {
            p.consumed = true;
        }
    }
}
