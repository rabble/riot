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

/// The person's relationship to a community, in plain product terms. Derived
/// from the held author, never caller-asserted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum CommunityRelationship {
    /// Their subspace key is the community namespace — they can approve tools.
    Organizer,
    /// They hold a distinct author in the community and can post.
    Member,
    /// They carry the community but hold no author — read only.
    PublicReader,
    /// A composite indymedia site the user follows (read-mostly).
    Following,
    /// The user's own personal home space.
    Personal,
}

/// One row in the community chooser. Plain-language fields only; the full
/// namespace and descriptor identifiers are technical disclosure the chooser
/// does not lead with.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CommunityRow {
    pub namespace_id: String,
    pub title: String,
    pub relationship: CommunityRelationship,
    /// Pinned `SpaceDescriptorV1` EntryId (hex) — the handle Home uses to
    /// reproject a loaded/joined community's newswire. `None` for a legacy space.
    pub descriptor_entry_id: Option<String>,
    /// Most recent local content time; drives the chooser's "recent activity".
    pub recent_activity_unix_seconds: Option<u64>,
    /// Most recent exchange time; drives the chooser's "sync freshness".
    pub sync_freshness_unix_seconds: Option<u64>,
    pub archived: bool,
    /// A corrupt/incompatible at-rest author was preserved for recovery; the
    /// community is shown but cannot be opened until repaired.
    pub quarantined: bool,
    /// True when the community can be opened right now (loadable author, not
    /// archived, not quarantined). False → the chooser offers recovery, never
    /// silently drops the row.
    pub available: bool,
}

/// One row in the followed-sites list: a composite indymedia site the user
/// follows. Distinct from `CommunityRow` because a followed site is **author-less**
/// (the user holds no posting author there); it carries only public identifiers —
/// there is NO secret / `Vec<u8>` field on this boundary (Security S2).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FollowedSiteRow {
    /// Owned site root (namespace id), lowercase hex (64 chars).
    pub root: String,
    /// Core-resolved title (from the resolved manifest, or a placeholder token
    /// until first resolve). Never caller-asserted.
    pub title: String,
    /// Honest row-state token, aligned to spec §3.1 where meaningful for a
    /// followed site: "available" / "pending-first-sync" / "transport-blocked"
    /// / "degraded". (syncing/quarantined are community-only.)
    /// NOTE: in Rung 1 the seam persists a plain Following record, so rows carry
    /// the default ("pending-first-sync") — the true transport-blocked/degraded
    /// PATH is exercised in Rung 5 (real `follow_site(ticket)` transport parsing).
    /// Rung 1 lands the FIELDS + default only.
    pub state: String,
    /// True iff the site requires an unavailable transport (`require:arti`) — the
    /// row shows "requires Tor — unavailable" without drilling in (S1). Field
    /// lands here; its true-path is set in Rung 5 (see `state` note).
    pub transport_blocked: bool,
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
    AppRejected,
    /// Approving an app was refused because this profile is a **member** of the
    /// space, not its organizer. Honest and expected: only the organizer decides
    /// what the space runs. The surface says "only the organizer can turn an app
    /// on here" — it must NOT offer a way around it.
    NotSpaceOrganizer,
    /// Approving an app was refused because this profile predates space
    /// organizers: its author is not organizer-shaped (`subspace != namespace`),
    /// so it cannot prove it created its own space and can never approve an app
    /// for it. There is no migration — under the old scheme a creator and a
    /// joiner are byte-identical, so letting this profile approve would let ANY
    /// member self-approve and would gut the one human review gate in the design.
    /// The only true remedy is a new profile, and the surface says exactly that.
    LegacyProfileCannotOrganize,
    /// The durable SQLite database could not be opened or written. The path
    /// may not exist, the file may be locked by another process, or the
    /// schema may be corrupt.
    Database,
    /// A community could not be opened: it is unknown, archived, holds no
    /// loadable author, or its at-rest author failed to open and was quarantined
    /// for recovery. The chooser preserves the row and offers recovery — it is
    /// never dropped, and a switch never silently lands on a different community.
    CommunityUnavailable,
    /// `publish_site_manifest` refused: the incoming `version` is below the
    /// durable per-root floor (rollback). No store write occurred — the prior
    /// manifest, if any, is unchanged.
    ManifestRollback,
    /// `publish_site_manifest` refused: a higher `version` lowered the mandatory
    /// transport `require` floor below the durable per-root floor (a distinct
    /// attack — it passes the version check yet strips a privacy guarantee). No
    /// store write occurred.
    ManifestRequireDowngrade,
    /// `publish_site_manifest` refused: a conflicting owner signature was seen
    /// at the SAME version as the durable floor — a compromise signal (e.g. a
    /// second device holding the masthead secret), surfaced as a distinct alarm
    /// rather than folded into a routine `InvalidInput`. No store write occurred.
    ManifestEquivocation,
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
            Self::AppRejected => "APP_REJECTED",
            Self::NotSpaceOrganizer => "NOT_SPACE_ORGANIZER",
            Self::LegacyProfileCannotOrganize => "LEGACY_PROFILE_CANNOT_ORGANIZE",
            Self::Database => "DATABASE_ERROR",
            Self::CommunityUnavailable => "COMMUNITY_UNAVAILABLE",
            Self::ManifestRollback => "MANIFEST_ROLLBACK",
            Self::ManifestRequireDowngrade => "MANIFEST_REQUIRE_DOWNGRADE",
            Self::ManifestEquivocation => "MANIFEST_EQUIVOCATION",
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

/// Opens a local profile backed by a durable SQLite database at `db_path`.
/// Spaces and accepted imports survive the handle being dropped and the
/// database being reopened. Use this for production app launches; use
/// `open_local_profile` for in-memory (demo/test) sessions.
#[uniffi::export]
pub fn open_local_profile_with_database(
    db_path: String,
) -> Result<Arc<MobileProfile>, MobileError> {
    crate::mobile_state::open_local_profile_with_database(db_path)
}

/// Restores a profile from a sealed identity into a durable SQLite database
/// at `db_path`. Combines identity restore with persistent storage.
#[uniffi::export]
pub fn open_profile_from_sealed_identity_with_database(
    db_path: String,
    wrapping_key: Vec<u8>,
    sealed_identity: Vec<u8>,
) -> Result<Arc<MobileProfile>, MobileError> {
    crate::mobile_state::open_profile_from_sealed_identity_with_database(
        db_path,
        wrapping_key,
        sealed_identity,
    )
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

    /// Joins a public space. When the join displaces a held community's author
    /// (adopting someone else's space while already in one), that outgoing author
    /// is sealed INLINE into its registry row under `wrapping_key` — so no author
    /// is ever parked unsealed in RAM (Risk 13). Pass the secure-store key; an
    /// empty slice is the keyless path for ephemeral in-memory profiles. The key
    /// is zeroized before return.
    pub fn join_public_space(
        &self,
        space: PublicSpace,
        wrapping_key: Vec<u8>,
    ) -> Result<PublicSpace, MobileError> {
        crate::mobile_state::join_public_space(&self.inner, space, wrapping_key)
    }

    /// Join a newswire community by the descriptor handle from a 1E share
    /// reference, so the joined community's registry row carries that handle and
    /// its Home reprojects once sync delivers the descriptor + posts (Risk 15).
    /// Distinct from `join_public_space` so the nearby-adopt path stays
    /// single-community and untouched.
    pub fn join_newswire_community(
        &self,
        space: PublicSpace,
        descriptor_entry_id: String,
        wrapping_key: Vec<u8>,
    ) -> Result<PublicSpace, MobileError> {
        crate::mobile_state::join_newswire_community(
            &self.inner,
            space,
            descriptor_entry_id,
            wrapping_key,
        )
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

    // --- Multiple communities (Unit 3) ---------------------------------------

    /// Every held community for the chooser, most-recently-active first. Reads
    /// metadata only — it never unseals any community's author.
    pub fn list_communities(&self) -> Result<Vec<CommunityRow>, MobileError> {
        crate::mobile_state::list_communities(&self.inner)
    }

    /// Every composite indymedia site the user follows, as author-less rows —
    /// distinct from `list_communities` (which surfaces only author-bearing
    /// communities). Reads registry metadata only; never unseals anything.
    pub fn list_followed_sites(&self) -> Result<Vec<FollowedSiteRow>, MobileError> {
        crate::mobile_state::list_followed_sites(&self.inner)
    }

    /// The currently selected community, or `None` before any is chosen. This is
    /// what "returning opens the last available community directly" reads.
    pub fn active_community(&self) -> Result<Option<CommunityRow>, MobileError> {
        crate::mobile_state::active_community(&self.inner)
    }

    /// Switches the active community: re-seals the outgoing author, cancels
    /// in-flight work, unseals the target with `wrapping_key`, and reprojects.
    /// A write or import in flight across this call fails closed. If the target's
    /// at-rest author fails to open it is quarantined (never dropped) and this
    /// returns `CommunityUnavailable` without leaving the old community.
    pub fn switch_community(
        &self,
        namespace_id: String,
        wrapping_key: Vec<u8>,
    ) -> Result<CommunityRow, MobileError> {
        crate::mobile_state::switch_community(&self.inner, namespace_id, wrapping_key)
    }

    /// Archives a community: it stays in the registry (never dropped) but leaves
    /// the primary chooser until restored.
    pub fn archive_community(&self, namespace_id: String) -> Result<(), MobileError> {
        crate::mobile_state::archive_community(&self.inner, namespace_id)
    }

    /// Restores an archived community to the chooser. Does not switch to it.
    pub fn restore_community(&self, namespace_id: String) -> Result<CommunityRow, MobileError> {
        crate::mobile_state::restore_community(&self.inner, namespace_id)
    }

    /// Seals every session-held community author under `wrapping_key` and flushes
    /// the registry, so the held communities survive a reopen. The native host
    /// calls this whenever it has the wrapping key (alongside `seal_identity`).
    pub fn persist_communities(&self, wrapping_key: Vec<u8>) -> Result<(), MobileError> {
        crate::mobile_state::persist_communities(&self.inner, wrapping_key)
    }

    /// True when the persisted registry failed to decode and was quarantined for
    /// recovery — the chooser shows a recovery state rather than an empty list.
    pub fn community_registry_quarantined(&self) -> Result<bool, MobileError> {
        crate::mobile_state::community_registry_quarantined(&self.inner)
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
