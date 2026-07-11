use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex, PoisonError};

use riot_core::import::{decode_bundle, encode_bundle, BundleDecodeOutcome, ItemStatus};
use riot_core::model::{decode_alert, encode_alert, AlertPayload, Certainty, Severity, Urgency};
use riot_core::session::{
    public_entry_identity, CommitOutcome, EvidenceStore, ImportContext, ImportPlan, ImportPreview,
    ImportSelection, InspectOutcome, RiotSession,
};
use riot_core::willow::{
    alert_entry_path_matches_payload, create_signed_alert, entry_id, generate_communal_author,
    generate_communal_author_for_namespace, system_snapshot, AlertDraft, EvidenceAuthor,
    SignedAlert as CoreSignedAlert, WillowError,
};

use crate::mobile_api::{
    AlertCertainty, AlertDraftInput, AlertDraftRecord, AlertFreshness, AlertSeverity, AlertUrgency,
    CurrentEntry, ImportAcceptance, MobileError, MobileImportPlan, MobileImportPreview,
    MobileProfile, PublicIdentity, PublicSpace, SignedAlert,
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
}

struct StoredPlan {
    id: u64,
    plan: ImportPlan,
    entries: Vec<CurrentEntry>,
}

pub(crate) fn open_local_profile() -> Result<Arc<MobileProfile>, MobileError> {
    match catch_unwind(AssertUnwindSafe(|| {
        let session = RiotSession::open().map_err(|_| MobileError::Internal)?;
        let store = session.create_store().map_err(|_| MobileError::Internal)?;
        let author = generate_communal_author().map_err(map_author_error)?;
        Ok(Arc::new(MobileProfile {
            inner: Arc::new(Mutex::new(ProfileState::Active(Box::new(LocalProfile {
                store,
                author,
                space: None,
                drafts: Vec::new(),
                preview: None,
                plan: None,
                entries: Vec::new(),
                next_handle_id: 1,
            })))),
        }))
    })) {
        Ok(result) => result,
        Err(_) => Err(MobileError::Internal),
    }
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

pub(crate) fn create_public_space(
    inner: &Arc<Mutex<ProfileState>>,
    title: String,
) -> Result<PublicSpace, MobileError> {
    with_active(inner, |profile| {
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
        profile.author =
            generate_communal_author_for_namespace(namespace_id).map_err(map_author_error)?;
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
        if route.trim().is_empty() || route.len() > 256 {
            return Err(MobileError::InvalidInput);
        }
        let namespace_id = &profile
            .space
            .as_ref()
            .ok_or(MobileError::InvalidInput)?
            .namespace_id;
        let entries = inspectable_alert_entries(&bytes, namespace_id)?;
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
        let (selection, selected_entries, plan) = {
            let preview = profile
                .preview
                .as_ref()
                .filter(|preview| preview.id == preview_id)
                .ok_or(MobileError::PreviewConsumed)?;
            let mut selection = Vec::with_capacity(selected_entry_ids.len());
            let mut selected_entries = Vec::with_capacity(selected_entry_ids.len());
            for selected_id in &selected_entry_ids {
                let entry_id = parse_entry_id(selected_id)?;
                let entry = preview
                    .entries
                    .iter()
                    .find(|entry| entry.entry_id == *selected_id)
                    .cloned()
                    .ok_or(MobileError::InvalidInput)?;
                selection.push(entry_id);
                selected_entries.push(entry);
            }
            let plan = preview
                .preview
                .plan(ImportSelection::new(selection))
                .map_err(map_core_error)?;
            (selected_entry_ids, selected_entries, plan)
        };
        if selection.len() != selected_entries.len() {
            return Err(MobileError::Internal);
        }
        let plan_id = profile.alloc_handle_id()?;
        profile.plan = Some(StoredPlan {
            id: plan_id,
            plan,
            entries: selected_entries,
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
        let entries = profile
            .plan
            .as_ref()
            .filter(|plan| plan.id == plan_id)
            .map(|plan| plan.entries.clone())
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
                Ok(ImportAcceptance {
                    accepted_entry_ids: entries.into_iter().map(|entry| entry.entry_id).collect(),
                })
            }
        }
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
) -> Result<Vec<CurrentEntry>, MobileError> {
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
        entries.push(CurrentEntry {
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
        WillowError::PathInvalid
        | WillowError::DoesNotAuthorise
        | WillowError::DecodeFailed
        | WillowError::TrailingBytes => MobileError::Internal,
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
