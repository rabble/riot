use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex, PoisonError};

use zeroize::{Zeroize, Zeroizing};

use riot_core::import::{decode_bundle, encode_bundle, BundleDecodeOutcome, ItemStatus};
use riot_core::model::{decode_alert, encode_alert, AlertPayload, Certainty, Severity, Urgency};
use riot_core::session::{
    public_entry_identity, CommitOutcome, EvidenceStore, ImportContext, ImportPlan, ImportPreview,
    ImportSelection, InspectOutcome, RiotSession,
};
use riot_core::sync::{ByteSyncOutcome, ByteSyncSession, SyncError, MAX_SYNC_IDS};
use riot_core::willow::{
    alert_entry_path_matches_payload, create_signed_alert, entry_id, generate_communal_author,
    generate_communal_author_for_namespace, system_snapshot, AlertDraft, EvidenceAuthor,
    SignedAlert as CoreSignedAlert, SignedWillowEntry, WillowError,
};

use crate::mobile_api::{
    AlertCertainty, AlertDraftInput, AlertDraftRecord, AlertFreshness, AlertSeverity, AlertUrgency,
    CurrentEntry, ImportAcceptance, MobileError, MobileImportPlan, MobileImportPreview,
    MobileProfile, MobileSyncSession, PublicIdentity, PublicSpace, SignedAlert, SyncOutcome,
    SyncOutcomeKind,
};

pub(crate) enum ProfileState {
    Active(Box<LocalProfile>),
    Failed,
}

const MAX_RETAINED_DRAFTS: usize = 64;
const MAX_SELECTED_ENTRY_IDS: usize = 64;

pub(crate) struct LocalProfile {
    store: EvidenceStore,
    author: EvidenceAuthor,
    space: Option<PublicSpace>,
    drafts: Vec<StoredDraft>,
    preview: Option<StoredPreview>,
    plan: Option<StoredPlan>,
    entries: Vec<CurrentEntry>,
    sync_inventory: Vec<SignedWillowEntry>,
    sync_session: Option<StoredSyncSession>,
    next_handle_id: u64,
}

struct StoredDraft {
    id: u64,
    draft: AlertDraft,
}

struct StoredPreview {
    id: u64,
    preview: ImportPreview,
    entries: Vec<CurrentEntry>,
    sync_entries: Vec<SignedWillowEntry>,
}

struct StoredPlan {
    id: u64,
    plan: ImportPlan,
    entries: Vec<CurrentEntry>,
    sync_entries: Vec<SignedWillowEntry>,
}

struct StoredSyncSession {
    id: u64,
    bridge: ByteSyncSession,
    pending: Option<StoredSyncImport>,
}

struct StoredSyncImport {
    preview: ImportPreview,
    entries: Vec<CurrentEntry>,
    sync_entries: Vec<SignedWillowEntry>,
}

struct InspectableAlert {
    current: CurrentEntry,
    signed: SignedWillowEntry,
}

pub(crate) fn open_local_profile() -> Result<Arc<MobileProfile>, MobileError> {
    match catch_unwind(AssertUnwindSafe(|| {
        let session = RiotSession::open().map_err(|_| MobileError::Internal)?;
        let store = session.create_store().map_err(|_| MobileError::Internal)?;
        let author = generate_communal_author().map_err(map_author_error)?;
        Ok(profile_with_author(store, author))
    })) {
        Ok(result) => result,
        Err(_) => Err(MobileError::Internal),
    }
}

pub(crate) fn open_profile_from_sealed_identity(
    mut wrapping_key: Vec<u8>,
    sealed_identity: Vec<u8>,
) -> Result<Arc<MobileProfile>, MobileError> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let key = exact_wrapping_key(&wrapping_key)?;
        let author = EvidenceAuthor::open_sealed_identity(&key, &sealed_identity)
            .map_err(map_author_error)?;
        let session = RiotSession::open().map_err(|_| MobileError::Internal)?;
        let store = session.create_store().map_err(|_| MobileError::Internal)?;
        Ok(profile_with_author(store, author))
    }));
    wrapping_key.zeroize();
    match result {
        Ok(result) => result,
        Err(_) => Err(MobileError::Internal),
    }
}

fn profile_with_author(store: EvidenceStore, author: EvidenceAuthor) -> Arc<MobileProfile> {
    Arc::new(MobileProfile {
        inner: Arc::new(Mutex::new(ProfileState::Active(Box::new(LocalProfile {
            store,
            author,
            space: None,
            drafts: Vec::new(),
            preview: None,
            plan: None,
            entries: Vec::new(),
            sync_inventory: Vec::new(),
            sync_session: None,
            next_handle_id: 1,
        })))),
    })
}

pub(crate) fn identity(inner: &Arc<Mutex<ProfileState>>) -> Result<PublicIdentity, MobileError> {
    with_active(inner, |profile| {
        let identity = profile.author.identity();
        Ok(PublicIdentity {
            namespace_id: hex(&identity.namespace_id),
            signing_key_id: hex(&identity.signing_key_id),
        })
    })
}

pub(crate) fn seal_identity(
    inner: &Arc<Mutex<ProfileState>>,
    mut wrapping_key: Vec<u8>,
) -> Result<Vec<u8>, MobileError> {
    let result = with_active(inner, |profile| {
        let key = exact_wrapping_key(&wrapping_key)?;
        profile.author.seal_identity(&key).map_err(map_author_error)
    });
    wrapping_key.zeroize();
    result
}

pub(crate) fn create_public_space(
    inner: &Arc<Mutex<ProfileState>>,
    title: String,
) -> Result<PublicSpace, MobileError> {
    with_active(inner, |profile| {
        if sync_session_is_active(profile) {
            return Err(MobileError::InvalidInput);
        }
        if title.trim().is_empty() || title.len() > 512 {
            return Err(MobileError::InvalidInput);
        }
        let space = PublicSpace {
            namespace_id: hex(&profile.author.identity().namespace_id),
            title,
            is_public: true,
        };
        profile.space = Some(space.clone());
        Ok(space)
    })
}

pub(crate) fn join_public_space(
    inner: &Arc<Mutex<ProfileState>>,
    space: PublicSpace,
) -> Result<PublicSpace, MobileError> {
    with_active(inner, |profile| {
        if sync_session_is_active(profile) {
            return Err(MobileError::InvalidInput);
        }
        if !space.is_public
            || space.title.trim().is_empty()
            || space.title.len() > 512
            || profile.space.is_some()
            || !profile.drafts.is_empty()
            || !profile.entries.is_empty()
            || profile.preview.is_some()
            || profile.plan.is_some()
        {
            return Err(MobileError::InvalidInput);
        }
        let namespace_id = parse_entry_id(&space.namespace_id)?;
        if profile.author.identity().namespace_id != namespace_id {
            profile.author =
                generate_communal_author_for_namespace(namespace_id).map_err(map_author_error)?;
        }
        let joined = PublicSpace {
            namespace_id: hex(&namespace_id),
            title: space.title,
            is_public: true,
        };
        profile.space = Some(joined.clone());
        Ok(joined)
    })
}

pub(crate) fn create_draft_alert(
    inner: &Arc<Mutex<ProfileState>>,
    input: AlertDraftInput,
) -> Result<AlertDraftRecord, MobileError> {
    with_active(inner, |profile| {
        if profile.space.is_none() {
            return Err(MobileError::InvalidInput);
        }
        let ai_assisted = input.ai_assisted;
        let draft = AlertDraft {
            valid_from: input.valid_from,
            expires_at: input.expires_at,
            language: input.language,
            urgency: urgency_from_ffi(input.urgency),
            severity: severity_from_ffi(input.severity),
            certainty: certainty_from_ffi(input.certainty),
            headline: input.headline,
            description: input.description,
            affected_area_claim: input.affected_area_claim,
            source_claims: input.source_claims,
            ai_assisted,
        };
        validate_draft(&draft)?;
        if profile.drafts.len() >= MAX_RETAINED_DRAFTS {
            return Err(MobileError::SessionLimit);
        }
        let id = profile.alloc_handle_id()?;
        profile.drafts.push(StoredDraft { id, draft });
        Ok(AlertDraftRecord {
            draft_id: id,
            ai_assisted,
        })
    })
}

pub(crate) fn sign_draft(
    inner: &Arc<Mutex<ProfileState>>,
    draft_id: u64,
) -> Result<SignedAlert, MobileError> {
    with_active(inner, |profile| {
        if sync_session_is_active(profile) {
            return Err(MobileError::InvalidInput);
        }
        let draft_index = profile
            .drafts
            .iter()
            .position(|draft| draft.id == draft_id)
            .ok_or(MobileError::DraftNotFound)?;
        let core_signed =
            create_signed_alert(&profile.author, profile.drafts[draft_index].draft.clone())
                .map_err(map_author_error)?;
        let bundle_bytes = encode_bundle(std::slice::from_ref(&core_signed.signed))
            .map_err(|_| MobileError::Internal)?;

        // Signing enters the same inspect/plan/commit core path as portable
        // imports, so current-entry state remains authoritative in the
        // session arbiter.
        profile.preview = None;
        profile.plan = None;
        let preview = inspect_core(&profile.store, &bundle_bytes, "local-sign")?;
        let plan = preview.plan_all().map_err(map_core_error)?;
        match plan.commit().map_err(map_core_error)? {
            CommitOutcome::Committed(_) | CommitOutcome::NoChanges(_) => {}
        }

        let entry = current_entry_from_signed(&core_signed)?;
        remember_entry(&mut profile.entries, entry.clone());
        remember_sync_entries(profile, vec![core_signed.signed.clone()])?;
        profile.drafts.remove(draft_index);
        Ok(SignedAlert {
            entry,
            bundle_bytes,
        })
    })
}

pub(crate) fn list_current_entries(
    inner: &Arc<Mutex<ProfileState>>,
) -> Result<Vec<CurrentEntry>, MobileError> {
    with_active(inner, |profile| {
        let namespace_id = &profile
            .space
            .as_ref()
            .ok_or(MobileError::InvalidInput)?
            .namespace_id;
        let live_ids = profile.store.live_entry_ids().map_err(map_core_error)?;
        let mut entries = Vec::with_capacity(live_ids.len());
        for live_id in live_ids {
            let live_id = hex(&live_id);
            let entry = profile
                .entries
                .iter()
                .find(|entry| entry.entry_id == live_id)
                .cloned()
                .ok_or(MobileError::Internal)?;
            if entry.namespace_id != *namespace_id {
                return Err(MobileError::Internal);
            }
            entries.push(entry);
        }
        entries.sort_unstable_by(|left, right| left.entry_id.cmp(&right.entry_id));
        Ok(entries)
    })
}

pub(crate) fn inspect_bytes(
    inner: &Arc<Mutex<ProfileState>>,
    bytes: Vec<u8>,
    route: String,
) -> Result<Arc<MobileImportPreview>, MobileError> {
    with_active(inner, |profile| {
        if sync_session_is_active(profile) {
            return Err(MobileError::InvalidInput);
        }
        if route.trim().is_empty() || route.len() > 256 {
            return Err(MobileError::InvalidInput);
        }
        let namespace_id = &profile
            .space
            .as_ref()
            .ok_or(MobileError::InvalidInput)?
            .namespace_id;
        let inspectable = inspectable_alert_entries(&bytes, namespace_id)?;
        let entries: Vec<_> = inspectable
            .iter()
            .map(|item| item.current.clone())
            .collect();
        let sync_entries = inspectable.into_iter().map(|item| item.signed).collect();
        profile.ensure_handle_capacity()?;
        profile.preview = None;
        profile.plan = None;
        let preview = inspect_core(&profile.store, &bytes, &route)?;
        if preview.eligible_count().map_err(map_core_error)? != entries.len() {
            return Err(MobileError::ImportRejected);
        }
        let preview_id = profile.alloc_handle_id()?;
        profile.preview = Some(StoredPreview {
            id: preview_id,
            preview,
            entries,
            sync_entries,
        });
        Ok(Arc::new(MobileImportPreview {
            inner: Arc::clone(inner),
            preview_id,
        }))
    })
}

pub(crate) fn eligible_entries(
    inner: &Arc<Mutex<ProfileState>>,
    preview_id: u64,
) -> Result<Vec<CurrentEntry>, MobileError> {
    with_active(inner, |profile| {
        profile
            .preview
            .as_ref()
            .filter(|preview| preview.id == preview_id)
            .map(|preview| preview.entries.clone())
            .ok_or(MobileError::PreviewConsumed)
    })
}

pub(crate) fn create_plan(
    inner: &Arc<Mutex<ProfileState>>,
    preview_id: u64,
    selected_entry_ids: Vec<String>,
) -> Result<Arc<MobileImportPlan>, MobileError> {
    with_active(inner, |profile| {
        if selected_entry_ids.is_empty() {
            return Err(MobileError::InvalidInput);
        }
        if selected_entry_ids.len() > MAX_SELECTED_ENTRY_IDS {
            return Err(MobileError::SessionLimit);
        }
        if selected_entry_ids
            .iter()
            .enumerate()
            .any(|(index, id)| selected_entry_ids[..index].contains(id))
        {
            return Err(MobileError::InvalidInput);
        }
        profile.ensure_handle_capacity()?;
        let (selection, selected_entries, selected_sync_entries, plan) = {
            let preview = profile
                .preview
                .as_ref()
                .filter(|preview| preview.id == preview_id)
                .ok_or(MobileError::PreviewConsumed)?;
            let mut selection = Vec::with_capacity(selected_entry_ids.len());
            let mut selected_entries = Vec::with_capacity(selected_entry_ids.len());
            let mut selected_sync_entries = Vec::with_capacity(selected_entry_ids.len());
            for selected_id in &selected_entry_ids {
                let entry_id = parse_entry_id(selected_id)?;
                let entry_index = preview
                    .entries
                    .iter()
                    .position(|entry| entry.entry_id == *selected_id)
                    .ok_or(MobileError::InvalidInput)?;
                selection.push(entry_id);
                selected_entries.push(preview.entries[entry_index].clone());
                selected_sync_entries.push(preview.sync_entries[entry_index].clone());
            }
            let plan = preview
                .preview
                .plan(ImportSelection::new(selection))
                .map_err(map_core_error)?;
            (
                selected_entry_ids,
                selected_entries,
                selected_sync_entries,
                plan,
            )
        };
        if selection.len() != selected_entries.len() {
            return Err(MobileError::Internal);
        }
        let plan_id = profile.alloc_handle_id()?;
        profile.plan = Some(StoredPlan {
            id: plan_id,
            plan,
            entries: selected_entries,
            sync_entries: selected_sync_entries,
        });
        Ok(Arc::new(MobileImportPlan {
            inner: Arc::clone(inner),
            plan_id,
        }))
    })
}

pub(crate) fn accept_plan(
    inner: &Arc<Mutex<ProfileState>>,
    plan_id: u64,
) -> Result<ImportAcceptance, MobileError> {
    with_active(inner, |profile| {
        let (entries, sync_entries) = profile
            .plan
            .as_ref()
            .filter(|plan| plan.id == plan_id)
            .map(|plan| (plan.entries.clone(), plan.sync_entries.clone()))
            .ok_or(MobileError::PlanConsumed)?;
        let outcome = profile
            .plan
            .as_ref()
            .expect("checked plan")
            .plan
            .commit()
            .map_err(map_core_error)?;
        match outcome {
            CommitOutcome::Committed(_) | CommitOutcome::NoChanges(_) => {
                profile.plan = None;
                profile.preview = None;
                for entry in &entries {
                    remember_entry(&mut profile.entries, entry.clone());
                }
                remember_sync_entries(profile, sync_entries)?;
                Ok(ImportAcceptance {
                    accepted_entry_ids: entries.into_iter().map(|entry| entry.entry_id).collect(),
                })
            }
        }
    })
}

pub(crate) fn open_sync_session(
    inner: &Arc<Mutex<ProfileState>>,
) -> Result<Arc<MobileSyncSession>, MobileError> {
    with_active(inner, |profile| {
        if profile.preview.is_some() || profile.plan.is_some() {
            return Err(MobileError::InvalidInput);
        }
        if profile
            .sync_session
            .as_ref()
            .is_some_and(|session| session.pending.is_some())
        {
            return Err(MobileError::InvalidInput);
        }
        let namespace_id = parse_entry_id(
            &profile
                .space
                .as_ref()
                .ok_or(MobileError::InvalidInput)?
                .namespace_id,
        )?;
        ensure_complete_sync_inventory(profile)?;
        let bridge = ByteSyncSession::new(namespace_id, profile.sync_inventory.clone())
            .map_err(map_sync_error)?;
        let sync_id = profile.alloc_handle_id()?;
        profile.sync_session = Some(StoredSyncSession {
            id: sync_id,
            bridge,
            pending: None,
        });
        Ok(Arc::new(MobileSyncSession {
            inner: Arc::clone(inner),
            sync_id,
        }))
    })
}

pub(crate) fn sync_begin(
    inner: &Arc<Mutex<ProfileState>>,
    sync_id: u64,
) -> Result<SyncOutcome, MobileError> {
    with_active(inner, |profile| {
        let session = active_sync_mut(profile, sync_id)?;
        let outcome = session.bridge.begin().map_err(map_sync_error)?;
        outcome_without_import(outcome, session.bridge.is_terminal())
    })
}

pub(crate) fn sync_receive_frame(
    inner: &Arc<Mutex<ProfileState>>,
    sync_id: u64,
    frame_bytes: Vec<u8>,
) -> Result<SyncOutcome, MobileError> {
    with_active(inner, |profile| {
        let outcome = active_sync_mut(profile, sync_id)?
            .bridge
            .receive_bytes(&frame_bytes)
            .map_err(map_sync_error)?;
        match outcome {
            ByteSyncOutcome::ImportBundle(bundle_bytes) => {
                match prepare_sync_import(profile, sync_id, &bundle_bytes) {
                    Ok(outcome) => Ok(outcome),
                    Err(error) => {
                        let code = if matches!(
                            error,
                            MobileError::StoreFull | MobileError::SessionLimit
                        ) {
                            2
                        } else {
                            1
                        };
                        let session = active_sync_mut(profile, sync_id)?;
                        let outcome = session
                            .bridge
                            .import_rejected(code)
                            .map_err(map_sync_error)?;
                        outcome_without_import(outcome, session.bridge.is_terminal())
                    }
                }
            }
            other => {
                let terminal = active_sync_mut(profile, sync_id)?.bridge.is_terminal();
                let terminal_without_frame = terminal && !matches!(other, ByteSyncOutcome::FrameReady);
                let result = outcome_without_import(other, terminal);
                if terminal_without_frame {
                    profile.sync_session = None;
                }
                result
            }
        }
    })
}

fn prepare_sync_import(
    profile: &mut LocalProfile,
    sync_id: u64,
    bundle_bytes: &[u8],
) -> Result<SyncOutcome, MobileError> {
    let namespace_id = profile
        .space
        .as_ref()
        .ok_or(MobileError::InvalidInput)?
        .namespace_id
        .clone();
    let inspectable = inspectable_alert_entries(bundle_bytes, &namespace_id)?;
    let entries: Vec<_> = inspectable
        .iter()
        .map(|item| item.current.clone())
        .collect();
    let sync_entries = inspectable.into_iter().map(|item| item.signed).collect();
    profile.preview = None;
    profile.plan = None;
    let preview = inspect_core(&profile.store, bundle_bytes, "conference-sync")?;
    if preview.eligible_count().map_err(map_core_error)? != entries.len() {
        return Err(MobileError::ImportRejected);
    }
    active_sync_mut(profile, sync_id)?.pending = Some(StoredSyncImport {
        preview,
        entries: entries.clone(),
        sync_entries,
    });
    Ok(SyncOutcome {
        kind: SyncOutcomeKind::ReviewImport,
        entries,
        rejection_code: None,
        terminal: false,
        import_bundle_bytes: Some(bundle_bytes.to_vec()),
    })
}

pub(crate) fn sync_take_outbound_frame(
    inner: &Arc<Mutex<ProfileState>>,
    sync_id: u64,
) -> Result<Option<Vec<u8>>, MobileError> {
    with_active(inner, |profile| {
        let (frame, terminal) = {
            let session = active_sync_mut(profile, sync_id)?;
            let frame = session.bridge.take_outbound_frame();
            (frame, session.bridge.is_terminal())
        };
        if terminal && frame.is_some() {
            profile.sync_session = None;
        }
        Ok(frame)
    })
}

pub(crate) fn sync_accept_import(
    inner: &Arc<Mutex<ProfileState>>,
    sync_id: u64,
) -> Result<SyncOutcome, MobileError> {
    with_active(inner, |profile| {
        let (entries, sync_entries) = {
            let pending = active_sync_mut(profile, sync_id)?
                .pending
                .as_ref()
                .ok_or(MobileError::InvalidInput)?;
            let plan = pending.preview.plan_all().map_err(map_core_error)?;
            match plan.commit().map_err(map_core_error)? {
                CommitOutcome::Committed(_) | CommitOutcome::NoChanges(_) => {}
            }
            (pending.entries.clone(), pending.sync_entries.clone())
        };
        active_sync_mut(profile, sync_id)?.pending = None;
        for entry in entries {
            remember_entry(&mut profile.entries, entry);
        }
        remember_sync_entries(profile, sync_entries)?;
        let session = active_sync_mut(profile, sync_id)?;
        let outcome = session.bridge.import_accepted().map_err(map_sync_error)?;
        outcome_without_import(outcome, session.bridge.is_terminal())
    })
}

pub(crate) fn sync_reject_import(
    inner: &Arc<Mutex<ProfileState>>,
    sync_id: u64,
    code: u8,
) -> Result<SyncOutcome, MobileError> {
    with_active(inner, |profile| {
        let session = active_sync_mut(profile, sync_id)?;
        if session.pending.take().is_none() {
            return Err(MobileError::InvalidInput);
        }
        let outcome = session
            .bridge
            .import_rejected(code)
            .map_err(map_sync_error)?;
        outcome_without_import(outcome, session.bridge.is_terminal())
    })
}

pub(crate) fn sync_close(
    inner: &Arc<Mutex<ProfileState>>,
    sync_id: u64,
) -> Result<(), MobileError> {
    with_active(inner, |profile| match profile.sync_session.as_ref() {
        Some(session) if session.id == sync_id => {
            profile.sync_session = None;
            Ok(())
        }
        Some(_) => Err(MobileError::ObjectClosed),
        None => Ok(()),
    })
}

fn active_sync_mut(
    profile: &mut LocalProfile,
    sync_id: u64,
) -> Result<&mut StoredSyncSession, MobileError> {
    profile
        .sync_session
        .as_mut()
        .filter(|session| session.id == sync_id)
        .ok_or(MobileError::ObjectClosed)
}

fn sync_session_is_active(profile: &LocalProfile) -> bool {
    profile.sync_session.is_some()
}

fn outcome_without_import(
    outcome: ByteSyncOutcome,
    terminal: bool,
) -> Result<SyncOutcome, MobileError> {
    let (kind, rejection_code) = match outcome {
        ByteSyncOutcome::FrameReady => (SyncOutcomeKind::FrameReady, None),
        ByteSyncOutcome::Rejected(code) => (SyncOutcomeKind::Rejected, Some(code)),
        ByteSyncOutcome::Complete => (SyncOutcomeKind::Complete, None),
        ByteSyncOutcome::ImportBundle(_) => return Err(MobileError::Internal),
    };
    Ok(SyncOutcome {
        kind,
        entries: Vec::new(),
        rejection_code,
        terminal,
        import_bundle_bytes: None,
    })
}

fn with_active<T>(
    inner: &Arc<Mutex<ProfileState>>,
    action: impl FnOnce(&mut LocalProfile) -> Result<T, MobileError>,
) -> Result<T, MobileError> {
    match catch_unwind(AssertUnwindSafe(|| {
        let mut state = lock_unpoisoned(inner);
        match &mut *state {
            ProfileState::Active(profile) => action(profile),
            ProfileState::Failed => Err(MobileError::SessionFailed),
        }
    })) {
        Ok(result) => result,
        Err(_) => {
            *lock_unpoisoned(inner) = ProfileState::Failed;
            Err(MobileError::Internal)
        }
    }
}

fn lock_unpoisoned(inner: &Arc<Mutex<ProfileState>>) -> std::sync::MutexGuard<'_, ProfileState> {
    inner.lock().unwrap_or_else(PoisonError::into_inner)
}

fn inspect_core(
    store: &EvidenceStore,
    bytes: &[u8],
    route: &str,
) -> Result<ImportPreview, MobileError> {
    match store
        .inspect(bytes, ImportContext::new(route))
        .map_err(map_core_error)?
    {
        InspectOutcome::Preview(preview) => Ok(preview),
        InspectOutcome::Rejected(_) => Err(MobileError::ImportRejected),
    }
}

fn inspectable_alert_entries(
    bytes: &[u8],
    expected_namespace_id: &str,
) -> Result<Vec<InspectableAlert>, MobileError> {
    let decoded = match decode_bundle(bytes) {
        BundleDecodeOutcome::Decoded(decoded) => decoded,
        BundleDecodeOutcome::Rejected(_) => return Err(MobileError::ImportRejected),
    };
    let mut entries = Vec::new();
    for item in decoded.items {
        let ItemStatus::Valid(valid) = item.status else {
            continue;
        };
        let alert =
            decode_alert(item.frame.payload_bytes()).map_err(|_| MobileError::ImportRejected)?;
        if !alert_entry_path_matches_payload(
            item.frame.entry_bytes(),
            &alert.object_id,
            &alert.revision_id,
        )
        .map_err(|_| MobileError::ImportRejected)?
        {
            return Err(MobileError::ImportRejected);
        }
        let identity = public_entry_identity(item.frame.entry_bytes())
            .map_err(|_| MobileError::ImportRejected)?;
        let namespace_id = hex(&identity.namespace_id);
        if namespace_id != expected_namespace_id {
            return Err(MobileError::ImportRejected);
        }
        entries.push(InspectableAlert {
            current: CurrentEntry {
                entry_id: hex(&valid.entry_id),
                namespace_id,
                signer_id: hex(&identity.signer_id),
                headline: alert.headline,
                freshness: AlertFreshness {
                    created_at: alert.created_at,
                    valid_from: alert.valid_from,
                    expires_at: alert.expires_at,
                },
                ai_assisted: alert.ai_assisted,
            },
            signed: SignedWillowEntry {
                entry_bytes: item.frame.entry_bytes().to_vec(),
                capability_bytes: item.frame.capability_bytes().to_vec(),
                signature: item
                    .frame
                    .signature_bytes()
                    .try_into()
                    .map_err(|_| MobileError::ImportRejected)?,
                payload_bytes: item.frame.payload_bytes().to_vec(),
            },
        });
    }
    if entries.is_empty() {
        return Err(MobileError::ImportRejected);
    }
    Ok(entries)
}

fn current_entry_from_signed(signed: &CoreSignedAlert) -> Result<CurrentEntry, MobileError> {
    let identity =
        public_entry_identity(&signed.signed.entry_bytes).map_err(|_| MobileError::Internal)?;
    Ok(CurrentEntry {
        entry_id: hex(&entry_id(&signed.signed.entry_bytes)),
        namespace_id: hex(&identity.namespace_id),
        signer_id: hex(&identity.signer_id),
        headline: signed.payload.headline.clone(),
        freshness: AlertFreshness {
            created_at: signed.payload.created_at,
            valid_from: signed.payload.valid_from,
            expires_at: signed.payload.expires_at,
        },
        ai_assisted: signed.payload.ai_assisted,
    })
}

fn remember_entry(entries: &mut Vec<CurrentEntry>, entry: CurrentEntry) {
    if !entries.iter().any(|known| known.entry_id == entry.entry_id) {
        entries.push(entry);
    }
}

fn remember_sync_entries(
    profile: &mut LocalProfile,
    incoming: Vec<SignedWillowEntry>,
) -> Result<(), MobileError> {
    for signed in incoming {
        let id = entry_id(&signed.entry_bytes);
        if !profile
            .sync_inventory
            .iter()
            .any(|known| entry_id(&known.entry_bytes) == id)
        {
            profile.sync_inventory.push(signed);
        }
    }
    let live_ids = profile.store.live_entry_ids().map_err(map_core_error)?;
    profile
        .sync_inventory
        .retain(|signed| live_ids.contains(&entry_id(&signed.entry_bytes)));
    profile
        .sync_inventory
        .sort_unstable_by_key(|signed| entry_id(&signed.entry_bytes));
    profile.sync_inventory.truncate(MAX_SYNC_IDS);
    Ok(())
}

fn ensure_complete_sync_inventory(profile: &LocalProfile) -> Result<(), MobileError> {
    let mut live_ids = profile.store.live_entry_ids().map_err(map_core_error)?;
    live_ids.sort_unstable();
    if live_ids.len() > MAX_SYNC_IDS {
        return Err(MobileError::SessionLimit);
    }
    let inventory_ids: Vec<_> = profile
        .sync_inventory
        .iter()
        .map(|signed| entry_id(&signed.entry_bytes))
        .collect();
    if inventory_ids != live_ids {
        return Err(MobileError::Internal);
    }
    Ok(())
}

impl LocalProfile {
    fn ensure_handle_capacity(&self) -> Result<(), MobileError> {
        if self.next_handle_id == u64::MAX {
            Err(MobileError::SessionLimit)
        } else {
            Ok(())
        }
    }

    fn alloc_handle_id(&mut self) -> Result<u64, MobileError> {
        self.ensure_handle_capacity()?;
        let id = self.next_handle_id;
        self.next_handle_id = self
            .next_handle_id
            .checked_add(1)
            .ok_or(MobileError::SessionLimit)?;
        Ok(id)
    }
}

fn validate_draft(draft: &AlertDraft) -> Result<(), MobileError> {
    let created_at = system_snapshot().map_err(map_author_error)?.unix_seconds;
    encode_alert(&AlertPayload {
        object_id: [0; 16],
        revision_id: [0; 16],
        created_at,
        valid_from: draft.valid_from,
        expires_at: draft.expires_at,
        language: draft.language.clone(),
        urgency: draft.urgency,
        severity: draft.severity,
        certainty: draft.certainty,
        headline: draft.headline.clone(),
        description: draft.description.clone(),
        affected_area_claim: draft.affected_area_claim.clone(),
        source_claims: draft.source_claims.clone(),
        ai_assisted: draft.ai_assisted,
    })
    .map(|_| ())
    .map_err(|_| MobileError::InvalidInput)
}

fn urgency_from_ffi(value: AlertUrgency) -> Urgency {
    match value {
        AlertUrgency::Immediate => Urgency::Immediate,
        AlertUrgency::Expected => Urgency::Expected,
        AlertUrgency::Future => Urgency::Future,
        AlertUrgency::Past => Urgency::Past,
        AlertUrgency::Unknown => Urgency::Unknown,
    }
}

fn severity_from_ffi(value: AlertSeverity) -> Severity {
    match value {
        AlertSeverity::Extreme => Severity::Extreme,
        AlertSeverity::Severe => Severity::Severe,
        AlertSeverity::Moderate => Severity::Moderate,
        AlertSeverity::Minor => Severity::Minor,
        AlertSeverity::Unknown => Severity::Unknown,
    }
}

fn certainty_from_ffi(value: AlertCertainty) -> Certainty {
    match value {
        AlertCertainty::Observed => Certainty::Observed,
        AlertCertainty::Likely => Certainty::Likely,
        AlertCertainty::Possible => Certainty::Possible,
        AlertCertainty::Unlikely => Certainty::Unlikely,
        AlertCertainty::Unknown => Certainty::Unknown,
    }
}

fn parse_entry_id(value: &str) -> Result<[u8; 32], MobileError> {
    if value.len() != 64 {
        return Err(MobileError::InvalidInput);
    }
    let mut id = [0u8; 32];
    for (index, byte) in id.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|_| MobileError::InvalidInput)?;
    }
    Ok(id)
}

fn exact_wrapping_key(value: &[u8]) -> Result<Zeroizing<[u8; 32]>, MobileError> {
    value
        .try_into()
        .map(Zeroizing::new)
        .map_err(|_| MobileError::InvalidInput)
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        value.push(HEX[(byte >> 4) as usize] as char);
        value.push(HEX[(byte & 0x0f) as usize] as char);
    }
    value
}

fn map_core_error(error: riot_core::session::SessionError) -> MobileError {
    use riot_core::session::SessionError;

    match error {
        SessionError::StoreFull => MobileError::StoreFull,
        SessionError::SessionLimit => MobileError::SessionLimit,
        SessionError::ObjectClosed => MobileError::ObjectClosed,
        SessionError::PreviewConsumed => MobileError::PreviewConsumed,
        SessionError::PlanSuperseded | SessionError::PlanConsumed | SessionError::PlanClosed => {
            MobileError::PlanConsumed
        }
        SessionError::StalePreview => MobileError::StalePreview,
        SessionError::NoEligibleEntries
        | SessionError::EmptySelection
        | SessionError::DuplicateSelection
        | SessionError::UnknownSelection => MobileError::InvalidInput,
        SessionError::WrongSession | SessionError::Injected | SessionError::Internal => {
            MobileError::Internal
        }
    }
}

fn map_author_error(error: WillowError) -> MobileError {
    match error {
        WillowError::EntropyUnavailable => MobileError::EntropyUnavailable,
        WillowError::ClockUnavailable => MobileError::ClockUnavailable,
        WillowError::InvalidAlert(_) | WillowError::NamespaceNotCommunal => {
            MobileError::InvalidInput
        }
        WillowError::SealedIdentityInvalid => MobileError::InvalidInput,
        WillowError::IdentitySealFailed => MobileError::Internal,
        WillowError::PathInvalid
        | WillowError::DoesNotAuthorise
        | WillowError::DecodeFailed
        | WillowError::TrailingBytes => MobileError::Internal,
    }
}

fn map_sync_error(error: SyncError) -> MobileError {
    match error {
        SyncError::FrameTooLarge | SyncError::TooManyEntryIds | SyncError::BundleTooLarge => {
            MobileError::SessionLimit
        }
        SyncError::MalformedFrame
        | SyncError::NonCanonicalFrame
        | SyncError::UnsupportedCodec
        | SyncError::DuplicateEntryId
        | SyncError::EntryIdsNotSorted
        | SyncError::NamespaceMismatch
        | SyncError::UnexpectedFrame
        | SyncError::UnknownEntryId
        | SyncError::InvalidBundle => MobileError::InvalidInput,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobile_api::{
        AlertCertainty, AlertDraftInput, AlertSeverity, AlertUrgency, MobileError,
    };

    fn valid_input() -> AlertDraftInput {
        AlertDraftInput {
            valid_from: None,
            expires_at: u64::MAX - 1,
            language: "en".into(),
            urgency: AlertUrgency::Immediate,
            severity: AlertSeverity::Severe,
            certainty: AlertCertainty::Observed,
            headline: "Bounded handle".into(),
            description: "Checked allocation fixture.".into(),
            affected_area_claim: None,
            source_claims: vec!["fixture".into()],
            ai_assisted: false,
        }
    }

    #[test]
    fn exhausted_handle_counter_returns_session_limit_without_retention() {
        let profile = open_local_profile().unwrap();
        create_public_space(&profile.inner, "Handle fixture".into()).unwrap();
        {
            let mut state = lock_unpoisoned(&profile.inner);
            let ProfileState::Active(local) = &mut *state else {
                panic!("profile active");
            };
            local.next_handle_id = u64::MAX;
        }

        assert!(matches!(
            create_draft_alert(&profile.inner, valid_input()),
            Err(MobileError::SessionLimit)
        ));
        let state = lock_unpoisoned(&profile.inner);
        let ProfileState::Active(local) = &*state else {
            panic!("profile active");
        };
        assert!(local.drafts.is_empty());
    }

    #[test]
    fn boundary_panic_quarantines_profile_for_later_calls() {
        let profile = open_local_profile().unwrap();
        let result = with_active(&profile.inner, |_profile| -> Result<(), MobileError> {
            panic!("injected boundary panic")
        });
        assert!(matches!(result, Err(MobileError::Internal)));
        assert!(matches!(
            identity(&profile.inner),
            Err(MobileError::SessionFailed)
        ));
    }
}
