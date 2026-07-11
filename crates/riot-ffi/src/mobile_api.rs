use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PublicIdentity {
    pub namespace_id: String,
    pub signing_key_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PublicSpace {
    pub namespace_id: String,
    pub title: String,
    pub is_public: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AlertUrgency {
    Immediate,
    Expected,
    Future,
    Past,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AlertSeverity {
    Extreme,
    Severe,
    Moderate,
    Minor,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AlertCertainty {
    Observed,
    Likely,
    Possible,
    Unlikely,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AlertDraftInput {
    pub valid_from: Option<u64>,
    pub expires_at: u64,
    pub language: String,
    pub urgency: AlertUrgency,
    pub severity: AlertSeverity,
    pub certainty: AlertCertainty,
    pub headline: String,
    pub description: String,
    pub affected_area_claim: Option<String>,
    pub source_claims: Vec<String>,
    pub ai_assisted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AlertDraftRecord {
    pub draft_id: u64,
    pub ai_assisted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AlertFreshness {
    pub created_at: u64,
    pub valid_from: Option<u64>,
    pub expires_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CurrentEntry {
    pub entry_id: String,
    pub namespace_id: String,
    pub signer_id: String,
    pub headline: String,
    pub freshness: AlertFreshness,
    pub ai_assisted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SignedAlert {
    pub entry: CurrentEntry,
    pub bundle_bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ImportAcceptance {
    pub accepted_entry_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SyncOutcomeKind {
    FrameReady,
    ReviewImport,
    Complete,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SyncOutcome {
    pub kind: SyncOutcomeKind,
    pub entries: Vec<CurrentEntry>,
    pub rejection_code: Option<u8>,
    pub terminal: bool,
    /// Exact canonical evidence bundle to persist before accepting a reviewed
    /// import. Present only for `ReviewImport`; never a sync protocol frame.
    pub import_bundle_bytes: Option<Vec<u8>>,
}

#[derive(Debug, uniffi::Error)]
pub enum MobileError {
    Internal,
    SessionFailed,
    InvalidInput,
    DraftNotFound,
    ImportRejected,
    StoreFull,
    SessionLimit,
    ObjectClosed,
    PreviewConsumed,
    PlanConsumed,
    StalePreview,
    EntropyUnavailable,
    ClockUnavailable,
}

impl std::fmt::Display for MobileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let code = match self {
            Self::Internal => "INTERNAL_ERROR",
            Self::SessionFailed => "SESSION_FAILED",
            Self::InvalidInput => "INVALID_INPUT",
            Self::DraftNotFound => "DRAFT_NOT_FOUND",
            Self::ImportRejected => "IMPORT_REJECTED",
            Self::StoreFull => "STORE_FULL",
            Self::SessionLimit => "SESSION_LIMIT",
            Self::ObjectClosed => "OBJECT_CLOSED",
            Self::PreviewConsumed => "PREVIEW_CONSUMED",
            Self::PlanConsumed => "PLAN_CONSUMED",
            Self::StalePreview => "STALE_PREVIEW",
            Self::EntropyUnavailable => "ENTROPY_UNAVAILABLE",
            Self::ClockUnavailable => "CLOCK_UNAVAILABLE",
        };
        f.write_str(code)
    }
}

#[derive(uniffi::Object)]
pub struct MobileProfile {
    pub(crate) inner: std::sync::Arc<std::sync::Mutex<crate::mobile_state::ProfileState>>,
}

#[derive(uniffi::Object)]
pub struct MobileImportPreview {
    pub(crate) inner: std::sync::Arc<std::sync::Mutex<crate::mobile_state::ProfileState>>,
    pub(crate) preview_id: u64,
}

#[derive(uniffi::Object)]
pub struct MobileImportPlan {
    pub(crate) inner: std::sync::Arc<std::sync::Mutex<crate::mobile_state::ProfileState>>,
    pub(crate) plan_id: u64,
}

#[derive(uniffi::Object)]
pub struct MobileSyncSession {
    pub(crate) inner: std::sync::Arc<std::sync::Mutex<crate::mobile_state::ProfileState>>,
    pub(crate) sync_id: u64,
}

#[uniffi::export]
pub fn open_local_profile() -> Result<Arc<MobileProfile>, MobileError> {
    crate::mobile_state::open_local_profile()
}

/// Restores only the local signing identity. Content/store persistence is a
/// separate native concern. Both inputs remain opaque byte arrays in UniFFI.
#[uniffi::export]
pub fn open_profile_from_sealed_identity(
    wrapping_key: Vec<u8>,
    sealed_identity: Vec<u8>,
) -> Result<Arc<MobileProfile>, MobileError> {
    crate::mobile_state::open_profile_from_sealed_identity(wrapping_key, sealed_identity)
}

#[uniffi::export]
impl MobileProfile {
    pub fn identity(&self) -> Result<PublicIdentity, MobileError> {
        crate::mobile_state::identity(&self.inner)
    }

    /// Returns authenticated opaque state suitable for Keychain/Keystore
    /// storage. No raw signer or Willow secret type crosses this boundary.
    pub fn seal_identity(&self, wrapping_key: Vec<u8>) -> Result<Vec<u8>, MobileError> {
        crate::mobile_state::seal_identity(&self.inner, wrapping_key)
    }

    pub fn create_public_space(&self, title: String) -> Result<PublicSpace, MobileError> {
        crate::mobile_state::create_public_space(&self.inner, title)
    }

    pub fn join_public_space(&self, space: PublicSpace) -> Result<PublicSpace, MobileError> {
        crate::mobile_state::join_public_space(&self.inner, space)
    }

    pub fn create_draft_alert(
        &self,
        input: AlertDraftInput,
    ) -> Result<AlertDraftRecord, MobileError> {
        crate::mobile_state::create_draft_alert(&self.inner, input)
    }

    pub fn sign_draft(&self, draft_id: u64) -> Result<SignedAlert, MobileError> {
        crate::mobile_state::sign_draft(&self.inner, draft_id)
    }

    pub fn list_current_entries(&self) -> Result<Vec<CurrentEntry>, MobileError> {
        crate::mobile_state::list_current_entries(&self.inner)
    }

    pub fn inspect_bytes(
        &self,
        bytes: Vec<u8>,
        route: String,
    ) -> Result<Arc<MobileImportPreview>, MobileError> {
        crate::mobile_state::inspect_bytes(&self.inner, bytes, route)
    }

    pub fn open_sync_session(&self) -> Result<Arc<MobileSyncSession>, MobileError> {
        crate::mobile_state::open_sync_session(&self.inner)
    }
}

#[uniffi::export]
impl MobileImportPreview {
    pub fn eligible_entries(&self) -> Result<Vec<CurrentEntry>, MobileError> {
        crate::mobile_state::eligible_entries(&self.inner, self.preview_id)
    }

    pub fn create_plan(
        &self,
        selected_entry_ids: Vec<String>,
    ) -> Result<Arc<MobileImportPlan>, MobileError> {
        crate::mobile_state::create_plan(&self.inner, self.preview_id, selected_entry_ids)
    }
}

#[uniffi::export]
impl MobileImportPlan {
    pub fn accept(&self) -> Result<ImportAcceptance, MobileError> {
        crate::mobile_state::accept_plan(&self.inner, self.plan_id)
    }
}

#[uniffi::export]
impl MobileSyncSession {
    pub fn begin(&self) -> Result<SyncOutcome, MobileError> {
        crate::mobile_state::sync_begin(&self.inner, self.sync_id)
    }

    pub fn receive_frame(&self, frame_bytes: Vec<u8>) -> Result<SyncOutcome, MobileError> {
        crate::mobile_state::sync_receive_frame(&self.inner, self.sync_id, frame_bytes)
    }

    pub fn take_outbound_frame(&self) -> Result<Option<Vec<u8>>, MobileError> {
        crate::mobile_state::sync_take_outbound_frame(&self.inner, self.sync_id)
    }

    pub fn accept_import(&self) -> Result<SyncOutcome, MobileError> {
        crate::mobile_state::sync_accept_import(&self.inner, self.sync_id)
    }

    pub fn reject_import(&self, code: u8) -> Result<SyncOutcome, MobileError> {
        crate::mobile_state::sync_reject_import(&self.inner, self.sync_id, code)
    }

    pub fn cancel(&self) -> Result<(), MobileError> {
        // Explicit cancellation is required because the profile owns the
        // pending preview independently of the language handle lifetime.
        crate::mobile_state::sync_cancel(&self.inner, self.sync_id)
    }
}
