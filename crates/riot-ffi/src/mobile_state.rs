use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex, PoisonError};

use ed25519_dalek::Signature;
use willow25::entry::EntrylikeExt;
use willow25::groupings::Keylike;
use zeroize::{Zeroize, Zeroizing};

use riot_core::import::{
    decode_bundle, decode_bundle_with_root, encode_bundle, BundleDecodeOutcome, ItemStatus,
    MAX_BUNDLE_BYTES,
};
use riot_core::model::{decode_alert, encode_alert, AlertPayload, Certainty, Severity, Urgency};
use riot_core::profile::card::{encode_profile_card, ProfileCard};
use riot_core::profile::path::{is_profile_prefixed, profile_card_path, SUBSPACE_ID_BYTES};
use riot_core::profile::resolver::{
    key_tag, render_display_name, resolve_display_names, sanitize_display_name,
};
use riot_core::profile::ProfileError;
use riot_core::session::{
    public_entry_identity, CommitOutcome, EvidenceStore, ImportContext, ImportPlan, ImportPreview,
    ImportSelection, InspectOutcome, RiotSession,
};
use riot_core::sync::{ByteSyncOutcome, ByteSyncSession, SyncError, MAX_SYNC_IDS};
use riot_core::willow::{
    alert_entry_path_matches_payload, create_signed_alert, entry_id,
    generate_communal_author_for_namespace, generate_space_organizer_author, system_snapshot,
    AlertDraft, EvidenceAuthor, SignedAlert as CoreSignedAlert, SignedWillowEntry, WillowError,
};

use crate::community_registry::{CommunityRecord, Relationship, REGISTRY_KEY};
use crate::mobile_api::{
    AlertCertainty, AlertDraftInput, AlertDraftRecord, AlertFreshness, AlertSeverity, AlertUrgency,
    CommunityRelationship, CommunityRow, CurrentEntry, FollowedSiteRow, ImportAcceptance,
    MobileError, MobileImportPlan, MobileImportPreview, MobileProfile, MobileSyncSession,
    PublicIdentity, PublicSpace, SignedAlert, SyncOutcome, SyncOutcomeKind,
};

pub(crate) enum ProfileState {
    Active(Box<LocalProfile>),
    Failed,
}

const MAX_RETAINED_DRAFTS: usize = 64;
const MAX_SELECTED_ENTRY_IDS: usize = 64;
const MAX_INSTALLED_APPS: usize = 16;
const MAX_APP_TRUST_MARKERS: usize = 256;
/// The complete retained inventory must fit one protocol bundle. This caps
/// aggregate proof/payload retention at 8 MiB before any session clones it.
///
/// CAUTION: this is currently an *alias* for `MAX_BUNDLE_BYTES`, so it bounds
/// nothing on its own. `encode_bundle` already fails at exactly this threshold,
/// which means both `if encoded.len() > MAX_SYNC_INVENTORY_BYTES` guards below
/// (in `prospective_sync_inventory` and the inventory revalidation) are
/// unreachable — the `encode_bundle(..)?` on the preceding line always fires
/// first. Do not read this constant as a separate, tighter sync bound: it is
/// not one today.
///
/// OPEN QUESTION (needs an owner decision, do not guess): was a tighter
/// sync-specific inventory ceiling intended here? This is the bound on how much
/// a peer can make us buffer during reconciliation, so the answer is
/// security-relevant. Either give it a real value below `MAX_BUNDLE_BYTES` (a
/// protocol change — the guards then become live and must be tested), or drop
/// the constant and the two dead guards and rely on `encode_bundle` alone.
/// Left as-is deliberately rather than silently changing peer-facing limits.
const MAX_SYNC_INVENTORY_BYTES: usize = MAX_BUNDLE_BYTES;

pub(crate) struct LocalProfile {
    pub(crate) store: EvidenceStore,
    pub(crate) author: EvidenceAuthor,
    pub(crate) space: Option<PublicSpace>,
    drafts: Vec<StoredDraft>,
    pub(crate) preview: Option<StoredPreview>,
    pub(crate) plan: Option<StoredPlan>,
    entries: Vec<CurrentEntry>,
    // pub(crate) so the site-follow-import test can assert the isolation invariant:
    // an owned-namespace bundle import must leave the community sync inventory
    // UNCHANGED. Only mutated via prospective/install_sync_inventory.
    pub(crate) sync_inventory: Vec<SignedWillowEntry>,
    sync_session: Option<StoredSyncSession>,
    /// A SECOND, independent sync slot for followed-site sync (Option C).
    /// Physically separate from `sync_session` so the community session, its
    /// generation guard, and the isolation-critical `sync_inventory` equality
    /// check are never entangled with owned-namespace delivery. Its offer is the
    /// just-in-time `build_followed_site_offer(root)` — never stored.
    // WU2 lands the mechanism + in-crate tests; the FFI handle that reads this
    // slot in production is WU4 (`follow_site`), so it is dead outside tests today.
    #[allow(dead_code)]
    followed_site_session: Option<StoredSyncSession>,
    next_handle_id: u64,
    /// Installed apps with their canonical manifest/bundle bytes (dedup +
    /// cap accounting; the display record is returned to the caller at
    /// install time). Retaining the bytes is what lets `share_app` publish
    /// an installed app into the Willow app-index; the memory ceiling is
    /// `MAX_INSTALLED_APPS` × `MAX_BUNDLE_TOTAL_BYTES`.
    installed_apps: Vec<StoredInstalledApp>,
    /// Profile-local trust markers, evaluated by `riot_core::apps::trust`
    /// with this profile's own subspace as the sole recognized organizer.
    /// Syncing markers as Willow entries is the app-directory follow-up.
    app_trust_markers: Vec<riot_core::apps::trust::TrustMarker>,
    /// Floor guaranteeing strictly increasing Willow timestamps for
    /// same-profile app-data writes, so a rapid overwrite of the same key
    /// within one clock second still prunes deterministically.
    app_data_timestamp_floor_micros: u64,
    /// Set exactly while the seeded demo space is the listed space. Its presence
    /// is the ONLY marker of demo mode — `hide_demo_space` refuses to un-list a
    /// space it did not itself list.
    demo_mode: Option<Box<DemoModeState>>,
    /// True once this profile joined a space that was NOT its own — the author
    /// had to be regenerated into a stranger's namespace (`join_public_space`).
    ///
    /// It exists ONLY to tell an honest refusal from a misleading one. A member
    /// and a pre-organizer ("legacy") profile are byte-identical — both have
    /// `subspace != namespace` — so nothing in the author can separate them, and
    /// each needs a different thing said to it: a member should ask the
    /// organizer, a legacy profile must start a new one. Neither may approve.
    ///
    /// Session-scoped, and safe to be: it never widens what a profile may do
    /// (every path it selects between still REFUSES), it only picks the sentence.
    /// It resets on relaunch, where a member is currently told the legacy line —
    /// the wording is wrong, the refusal is not. Fixing that means persisting
    /// provenance in the profile record, which is iOS's side of the boundary.
    joined_others_space: bool,
    /// Monotonic app-execution generation. Bumped on any app-trust change and
    /// any namespace swap. `AppExecutionSession` (Unit 0C) captures this at open
    /// and revalidates it on every read/commit, so a re-approval — which returns
    /// trust to TRUE — still invalidates every session opened before it. This is
    /// what makes containment a mechanism, not a native-host policy: a stale
    /// session fails *before* it can touch data, with no way to assume authority.
    app_execution_generation: u64,
    /// The durable database handle — a cheap `Arc` clone; the session owns a twin
    /// sharing the same connection, lease, and reader pool. `None` for in-memory
    /// profiles, whose registry then lives only in this struct for the session.
    db: Option<riot_core::store::RiotDatabase>,
    /// The held communities and which one is active (Unit 3). Source of truth in
    /// memory; mirrored to `local_state` on every mutation when `db` is `Some`.
    // pub(crate) so the site-follow importer can read the Following gate and stamp
    // last-sync; only ever mutated via registry methods + persist_registry.
    pub(crate) registry: crate::community_registry::CommunityRegistry,
    /// Inactive per-community authors, unsealed and parked by namespace. The
    /// ACTIVE community's author is `self.author`, never duplicated here (the
    /// author is deliberately not `Clone`). A community's sealed author is
    /// un-loadable from at rest EXCEPT by a deliberate `switch_community` with the
    /// wrapping key; listing never unseals. This is the isolation the registry
    /// guarantees.
    ///
    /// AT REST (on disk) only sealed authors are ever written. IN RAM, with a
    /// wrapping key present, ONLY THE ACTIVE author is ever unsealed: both
    /// `switch_community` and `join_public_space` seal the outgoing author inline
    /// into its registry row (Risk 13 closed — seal-inline-on-join) rather than
    /// parking it here unsealed. This map holds an unsealed author only on the
    /// keyless path — an ephemeral `open_local_profile()` profile that carries no
    /// key — or transiently if a seal fails (entropy) and the author is parked
    /// rather than lost.
    community_authors: std::collections::HashMap<[u8; 32], EvidenceAuthor>,
    /// Bumped on every community switch. Every in-flight preview/plan/sync handle
    /// captures it at creation and revalidates before commit, so a write or import
    /// in flight across a switch fails closed rather than landing in the wrong
    /// community's store.
    community_generation: u64,
    /// Parked board projections for inactive communities, keyed by namespace. The
    /// store retains payloads only for IMPORTED entries, so a locally-signed alert
    /// cannot be rebuilt from the store after a switch; parking the projection
    /// preserves it in-session. A freshly loaded/joined community has no parked
    /// projection and is reprojected from the store's imported content instead.
    community_entries: std::collections::HashMap<[u8; 32], Vec<CurrentEntry>>,
    /// Parked sync inventories for inactive communities, keyed by namespace. The
    /// sync inventory is what the active community offers peers; it MUST stay
    /// scoped to the active community (a shared inventory would leak one
    /// community's entries to another's peers), so a switch parks the outgoing
    /// one and restores the target's rather than reusing it.
    community_sync_inventory: std::collections::HashMap<[u8; 32], Vec<SignedWillowEntry>>,
    /// True when the persisted registry blob failed to decode and was quarantined
    /// for recovery (a bad migration): the session runs with an empty registry,
    /// but the raw undecodable bytes are preserved under the quarantine key and
    /// never discarded.
    registry_quarantined: bool,
}

/// What the profile was before demo mode was switched on.
///
/// Only the author is remembered, because the space slot it replaced was
/// necessarily empty (`load_demo_space` refuses to displace a listed space).
struct DemoModeState {
    /// The author held before the profile moved into the demo namespace, put
    /// back by `hide_demo_space`. `None` only if the profile was somehow already
    /// in that namespace and no move was needed.
    previous_author: Option<EvidenceAuthor>,
}

struct StoredDraft {
    id: u64,
    draft: AlertDraft,
}

struct StoredInstalledApp {
    app_id: [u8; 32],
    manifest_bytes: Vec<u8>,
    bundle_bytes: Vec<u8>,
}

pub(crate) struct StoredPreview {
    id: u64,
    /// The community generation when this handle was created. A switch advances
    /// the generation, so a preview created before a switch fails its generation
    /// check and cannot commit into the wrong community (the fail-closed guard).
    community_generation: u64,
    preview: ImportPreview,
    entries: Vec<CurrentEntry>,
    sync_entries: Vec<SignedWillowEntry>,
}

pub(crate) struct StoredPlan {
    id: u64,
    community_generation: u64,
    plan: ImportPlan,
    entries: Vec<CurrentEntry>,
    sync_entries: Vec<SignedWillowEntry>,
}

struct StoredSyncSession {
    id: u64,
    community_generation: u64,
    bridge: ByteSyncSession,
    pending: Option<StoredSyncImport>,
    /// `None` for a community sync session (the active-community path, byte
    /// identical to before this field existed). `Some(root)` marks a
    /// FOLLOWED-SITE session: its import admits owned records under
    /// `followed_root = root` (the site), family-gated to /mod + /articles, and
    /// its accept commits WITHOUT touching `sync_inventory` (owned-namespace
    /// records must never enter the active community's peer offer set).
    // Read only through the followed-site drive fns, which are FFI-wired in WU4.
    #[allow(dead_code)]
    followed_root: Option<[u8; 32]>,
}

struct StoredSyncImport {
    preview: ImportPreview,
    entries: Vec<CurrentEntry>,
    sync_entries: Vec<SignedWillowEntry>,
}

struct InspectableEntry {
    current: Option<CurrentEntry>,
    signed: SignedWillowEntry,
}

/// The community-generation guard (Unit 3): a preview/plan/sync handle captured
/// in one community must never act on another. A switch advances
/// `community_generation`, so a handle whose captured generation no longer
/// matches is stale — its operation fails closed rather than committing into,
/// or reading from, the wrong community's store. This is the mechanism that
/// makes a switch/write race safe even for a handle that outlives the switch;
/// the profile `Mutex` already serializes the operations themselves.
fn handle_generation_is_current(profile: &LocalProfile, captured_generation: u64) -> bool {
    captured_generation == profile.community_generation
}

/// Drop any preview/plan/sync handle left over from a previous community (its
/// captured generation no longer matches the current one). A direct use of such
/// a handle already fails closed via its generation check; this clears a dead
/// handle out of the way so it cannot block a fresh operation in the new
/// community. Never masks a live handle — only a stale one is dropped.
fn drop_stale_handles(profile: &mut LocalProfile) {
    let generation = profile.community_generation;
    if profile
        .preview
        .as_ref()
        .is_some_and(|preview| preview.community_generation != generation)
    {
        profile.preview = None;
    }
    if profile
        .plan
        .as_ref()
        .is_some_and(|plan| plan.community_generation != generation)
    {
        profile.plan = None;
    }
    if profile
        .sync_session
        .as_ref()
        .is_some_and(|session| session.community_generation != generation)
    {
        profile.sync_session = None;
    }
}

pub(crate) fn open_local_profile() -> Result<Arc<MobileProfile>, MobileError> {
    match catch_unwind(AssertUnwindSafe(|| {
        let session = RiotSession::open().map_err(|_| MobileError::Internal)?;
        let store = session.create_store().map_err(|_| MobileError::Internal)?;
        // Organizer-shaped from birth: the namespace ID is this profile's own
        // subspace key, so if this person creates a space they are derivably its
        // organizer — and the identity is fixed before it is sealed, so creating a
        // space never rotates the signing key.
        let author = generate_space_organizer_author().map_err(map_author_error)?;
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

/// Opens a local profile backed by a durable SQLite database at `db_path`.
/// Spaces, entries, and accepted imports written through this profile
/// survive the handle being dropped and the session being reopened.
///
/// The path must be a filesystem location the native host can read and
/// write. `DatabaseConfig::default()` is used (WAL journaling, default
/// busy timeout and reader pool).
pub(crate) fn open_local_profile_with_database(
    db_path: String,
) -> Result<Arc<MobileProfile>, MobileError> {
    match catch_unwind(AssertUnwindSafe(|| {
        let database = riot_core::store::RiotDatabase::open(
            &db_path,
            riot_core::store::DatabaseConfig::default(),
        )
        .map_err(map_database_error)?;
        // Keep a cheap Arc-clone handle for registry persistence; the session
        // owns a twin sharing the same connection, lease, and reader pool.
        let db_handle = database.clone();
        let session = RiotSession::open_sqlite(database).map_err(|_| MobileError::Internal)?;
        let store = session.create_store().map_err(|_| MobileError::Internal)?;
        let author = generate_space_organizer_author().map_err(map_author_error)?;
        Ok(profile_with_author_and_db(store, author, Some(db_handle)))
    })) {
        Ok(result) => result,
        Err(_) => Err(MobileError::Internal),
    }
}

/// Restores a profile from a sealed identity, backed by a durable SQLite
/// database at `db_path`. Combines `open_profile_from_sealed_identity`
/// semantics with the durable store from `open_local_profile_with_database`.
pub(crate) fn open_profile_from_sealed_identity_with_database(
    db_path: String,
    mut wrapping_key: Vec<u8>,
    sealed_identity: Vec<u8>,
) -> Result<Arc<MobileProfile>, MobileError> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let database = riot_core::store::RiotDatabase::open(
            &db_path,
            riot_core::store::DatabaseConfig::default(),
        )
        .map_err(map_database_error)?;
        let db_handle = database.clone();
        let key = exact_wrapping_key(&wrapping_key)?;
        let author = EvidenceAuthor::open_sealed_identity(&key, &sealed_identity)
            .map_err(map_author_error)?;
        let session = RiotSession::open_sqlite(database).map_err(|_| MobileError::Internal)?;
        let store = session.create_store().map_err(|_| MobileError::Internal)?;
        let profile = profile_with_author_and_db(store, author, Some(db_handle));
        // Durable multi-community restore: swap the active community's own sealed
        // author in over the just-restored primary identity, and reproject its
        // content, so a reopen lands on the same community with the same Home.
        restore_active_community(&profile, &key)?;
        Ok(profile)
    }));
    wrapping_key.zeroize();
    match result {
        Ok(result) => result,
        Err(_) => Err(MobileError::Internal),
    }
}

/// Maps a `DatabaseError` to the FFI error enum. Database open failures
/// (missing directory, locked file, corrupt schema) surface as a typed
/// `Database` error rather than a generic `Internal`.
fn map_database_error(error: riot_core::store::DatabaseError) -> MobileError {
    let _ = error; // Avoid unused warnings if the variant is narrowed later.
    MobileError::Database
}

fn profile_with_author(store: EvidenceStore, author: EvidenceAuthor) -> Arc<MobileProfile> {
    profile_with_author_and_db(store, author, None)
}

fn profile_with_author_and_db(
    store: EvidenceStore,
    author: EvidenceAuthor,
    db: Option<riot_core::store::RiotDatabase>,
) -> Arc<MobileProfile> {
    let (registry, registry_quarantined) = load_registry(db.as_ref());
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
            followed_site_session: None,
            next_handle_id: 1,
            installed_apps: Vec::new(),
            app_trust_markers: Vec::new(),
            app_data_timestamp_floor_micros: 0,
            demo_mode: None,
            joined_others_space: false,
            app_execution_generation: 0,
            db,
            registry,
            community_authors: std::collections::HashMap::new(),
            community_generation: 0,
            community_entries: std::collections::HashMap::new(),
            community_sync_inventory: std::collections::HashMap::new(),
            registry_quarantined,
        })))),
    })
}

/// Loads the persisted community registry from `local_state`. A blob that fails
/// to decode is a bad migration: its raw bytes are copied to the quarantine key
/// (never overwritten, never discarded) and an empty registry is returned with
/// the quarantine flag set, so the person can recover rather than losing every
/// community. An in-memory profile (`db` is `None`) starts with an empty registry.
fn load_registry(
    db: Option<&riot_core::store::RiotDatabase>,
) -> (crate::community_registry::CommunityRegistry, bool) {
    use crate::community_registry::{CommunityRegistry, REGISTRY_KEY, REGISTRY_QUARANTINE_KEY};
    let Some(db) = db else {
        return (CommunityRegistry::default(), false);
    };
    let Ok(Some(bytes)) = db.local_state(REGISTRY_KEY) else {
        return (CommunityRegistry::default(), false);
    };
    match CommunityRegistry::decode(&bytes) {
        Ok(registry) => (registry, false),
        Err(_) => {
            // Preserve the undecodable blob for recovery before anything can
            // overwrite the primary key. Only copy if a prior quarantine is not
            // already present, so repeated opens do not clobber the first capture.
            if matches!(db.local_state(REGISTRY_QUARANTINE_KEY), Ok(None)) {
                let _ = db.set_local_state(REGISTRY_QUARANTINE_KEY, &bytes);
            }
            (CommunityRegistry::default(), true)
        }
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
        // Register this as the active community (organizer): the author's own
        // namespace, so `is_space_organizer` is true.
        register_active_community(profile, None)?;
        Ok(space)
    })
}

/// A wrapping key that MAY be absent: an empty slice means keyless (an ephemeral
/// in-memory/test profile that carries no secure-store key), 32 bytes is a real
/// key, any other length is malformed. Callers on real devices always pass a
/// key; the keyless arm exists only so `open_local_profile()` profiles keep
/// working without one.
fn optional_wrapping_key(value: &[u8]) -> Result<Option<Zeroizing<[u8; 32]>>, MobileError> {
    if value.is_empty() {
        Ok(None)
    } else {
        exact_wrapping_key(value).map(Some)
    }
}

/// Seal the OUTGOING community's author into its registry row and drop the
/// plaintext copy — or, keyless / seal failure / no row to seal into, park it
/// unsealed so the author is never lost. Sealing inline is what keeps a
/// joined-away or switched-away author from lingering unsealed in RAM (Risk 13);
/// with the key its row is fully recoverable, so nothing but the ACTIVE author
/// stays unsealed. Shared by `join_public_space` and `switch_community` so the
/// two paths seal identically.
fn seal_or_park_outgoing(
    profile: &mut LocalProfile,
    active_ns: [u8; 32],
    outgoing: EvidenceAuthor,
    key: Option<&[u8; 32]>,
) {
    if let Some(key) = key {
        if let Ok(sealed) = outgoing.seal_identity(key) {
            if let Some(record) = profile.registry.find_mut(&active_ns) {
                record.sealed_author = Some(sealed);
                return;
            }
        }
    }
    profile.community_authors.insert(active_ns, outgoing);
}

pub(crate) fn join_public_space(
    inner: &Arc<Mutex<ProfileState>>,
    space: PublicSpace,
    wrapping_key: Vec<u8>,
) -> Result<PublicSpace, MobileError> {
    join_community_impl(inner, space, None, wrapping_key)
}

/// Join a newswire community by the descriptor handle a 1E share reference
/// supplies, so the joined community's registry row CARRIES that handle (Risk
/// 15). Without it a joined community is a "dead follow" — Home can never
/// reproject its newswire even after sync delivers the descriptor + posts,
/// because there is no descriptor id to project from and no discovery accessor.
pub(crate) fn join_newswire_community(
    inner: &Arc<Mutex<ProfileState>>,
    space: PublicSpace,
    descriptor_entry_id: String,
    mut wrapping_key: Vec<u8>,
) -> Result<PublicSpace, MobileError> {
    let descriptor = match parse_entry_id(&descriptor_entry_id) {
        Ok(descriptor) => Some(descriptor),
        Err(error) => {
            wrapping_key.zeroize();
            return Err(error);
        }
    };
    join_community_impl(inner, space, descriptor, wrapping_key)
}

fn join_community_impl(
    inner: &Arc<Mutex<ProfileState>>,
    space: PublicSpace,
    descriptor_entry_id: Option<[u8; 32]>,
    mut wrapping_key: Vec<u8>,
) -> Result<PublicSpace, MobileError> {
    let result = with_active(inner, |profile| {
        // Validate the key shape up front so a malformed key fails the join
        // before any author is minted or moved.
        let key = optional_wrapping_key(&wrapping_key)?;
        if sync_session_is_active(profile) {
            return Err(MobileError::InvalidInput);
        }
        if !space.is_public || space.title.trim().is_empty() || space.title.len() > 512 {
            return Err(MobileError::InvalidInput);
        }
        let namespace_id = parse_entry_id(&space.namespace_id)?;
        let joined = PublicSpace {
            namespace_id: hex(&namespace_id),
            title: space.title,
            is_public: true,
        };

        // Re-selecting the already-active community is idempotent.
        if profile.registry.active == Some(namespace_id) {
            return Ok(joined);
        }

        // Adopting or restoring the person's OWN space (namespace already matches
        // the current author): keep the author — an organizer restoring their own
        // space must stay the organizer — do not mint or park. On relaunch iOS
        // restores every persisted space through this function, a created one
        // exactly like a joined one, and this branch keeps a creator a creator.
        if profile.author.identity().namespace_id == namespace_id {
            profile.space = Some(joined.clone());
            register_active_community(profile, descriptor_entry_id)?;
            return Ok(joined);
        }

        // Joining SOMEONE ELSE'S space as a member. A held-but-inactive community
        // must be re-entered through `switch_community` (which has the wrapping
        // key to unseal its own author); minting a second author here would fork
        // an unlinkable pseudonym and orphan the first.
        if profile.registry.find(&namespace_id).is_some() {
            return Err(MobileError::CommunityUnavailable);
        }

        // Cancel any in-flight work bound to the outgoing community.
        profile.preview = None;
        profile.plan = None;
        profile.sync_session = None;
        profile.drafts.clear();
        profile.sync_inventory.clear();
        profile.app_trust_markers.clear();

        // Mint a fresh, unlinkable communal author for the target namespace — the
        // only moment we learn for certain this space belongs to someone else.
        let joined_author =
            generate_communal_author_for_namespace(namespace_id).map_err(map_author_error)?;
        // Seal-inline-on-join (Risk 13): the outgoing active author (not `Clone`)
        // is sealed into its registry row immediately, so no author is ever
        // parked unsealed in RAM when a real key is present. Keyless profiles
        // fall back to parking. A fresh profile with no active community simply
        // drops its bootstrap author.
        let outgoing_ns = profile.registry.active;
        let outgoing = std::mem::replace(&mut profile.author, joined_author);
        if let Some(active_ns) = outgoing_ns {
            seal_or_park_outgoing(profile, active_ns, outgoing, key.as_deref());
        }
        profile.joined_others_space = true;
        profile.space = Some(joined.clone());
        // The namespace moved: every app-execution session and any in-flight
        // handle bound to the old namespace is now stale and must fail closed.
        bump_app_execution_generation(profile);
        profile.community_generation = profile.community_generation.wrapping_add(1);
        register_active_community(profile, descriptor_entry_id)?;
        reproject_active(profile)?;
        Ok(joined)
    });
    wrapping_key.zeroize();
    result
}

/// The title the seeded demo space is listed under. A bundle carries signed
/// entries, not a space title, so this is the one place it is written down.
pub(crate) const DEMO_SPACE_TITLE: &str = "Riverside Tenants Union";

/// The import route recorded for demo bytes, alongside `local-sign` and
/// `local-app-write`. It names where the bytes came from; it grants nothing.
const DEMO_IMPORT_ROUTE: &str = "demo-space";

/// Loads the seeded demo space from a signed bundle and lists its namespace.
///
/// The bytes go through the ORDINARY `inspect → plan → commit` pipeline — the
/// same one a peer's bundle goes through. There is no privileged demo import:
/// every entry is verified, admitted, and committed exactly as if it had arrived
/// over sync, which is the whole point of shipping the demo as a real signed
/// bundle rather than as native fixtures.
///
/// **Additive.** It refuses (leaving the store bit-for-bit untouched) if any
/// OTHER space is listed. That is not squeamishness: the FFI store is
/// single-namespace in practice — `open_sync_session` builds its inventory from
/// every live entry and `ByteSyncSession` rejects an entry outside its namespace
/// — so mixing the demo into somebody's real space would silently take their
/// sync away. A person who wants the demo starts from a profile with no space.
///
/// **Idempotent.** Entries are content-addressed, so a second load finds every
/// one of them already present, commits nothing, and re-lists the same space.
///
/// Fails with `InvalidInput` while a sync session is open, for the same reason
/// `app_data_put` and `set_display_name` do: the commit runs through
/// `store.inspect`, which replaces the session-wide preview slot an in-flight
/// sync review is holding.
pub(crate) fn load_demo_space(
    inner: &Arc<Mutex<ProfileState>>,
    bytes: Vec<u8>,
) -> Result<PublicSpace, MobileError> {
    with_active(inner, |profile| {
        if sync_session_is_active(profile) {
            return Err(MobileError::InvalidInput);
        }
        if profile.preview.is_some() || profile.plan.is_some() {
            return Err(MobileError::InvalidInput);
        }

        // The namespace is the BUNDLE's own, read back from its signed entries.
        // The demo space is not a special kind of space; it is just one this
        // device did not author.
        let namespace_id = whole_bundle_namespace_id(&bytes)?;
        let namespace_hex = hex(&namespace_id);

        let already_listed = profile
            .space
            .as_ref()
            .is_some_and(|space| space.namespace_id == namespace_hex);
        // The hard additive rule. Nothing below this line runs while a space
        // that is not the demo is listed, so a real space cannot be displaced
        // and its entries cannot be touched.
        if !already_listed && profile.space.is_some() {
            return Err(MobileError::ImportRejected);
        }

        let inspectable = inspectable_entries(&bytes, &namespace_hex)?;
        if inspectable.is_empty() {
            return Err(MobileError::ImportRejected);
        }
        let entries: Vec<_> = inspectable
            .iter()
            .filter_map(|item| item.current.clone())
            .collect();
        let sync_entries: Vec<_> = inspectable.into_iter().map(|item| item.signed).collect();
        let next_inventory = prospective_sync_inventory(profile, &sync_entries)?;

        // Every fallible step that would move the profile's identity happens
        // BEFORE the commit, so a rejected bundle can never leave a half-moved
        // profile behind.
        let demo_author = if already_listed
            || profile.author.identity().namespace_id == namespace_id
        {
            None
        } else {
            Some(generate_communal_author_for_namespace(namespace_id).map_err(map_author_error)?)
        };

        let preview = inspect_core(&profile.store, &bytes, DEMO_IMPORT_ROUTE)?;
        let eligible = preview.eligible_count().map_err(map_core_error)?;
        if eligible > sync_entries.len() {
            return Err(MobileError::ImportRejected);
        }
        if eligible > 0 {
            let plan = preview.plan_all().map_err(map_core_error)?;
            match plan.commit().map_err(map_core_error)? {
                CommitOutcome::Committed(_) | CommitOutcome::NoChanges(_) => {}
            }
        }
        // `eligible == 0` is the idempotent re-load: the join already holds every
        // one of these entries, so there is nothing to commit and no duplicate to
        // create. Planning an empty selection would be an error, so don't.

        for entry in entries {
            remember_entry(&mut profile.entries, entry);
        }
        install_sync_inventory(profile, next_inventory)?;
        advance_app_write_floor(profile, &sync_entries)?;

        if !already_listed {
            // The same move `join_public_space` makes: a communal namespace you
            // did not create needs a subspace key IN it, or this person could
            // never write to the space they are looking at. The SEALED identity
            // on disk is not rewritten, and `hide_demo_space` puts this author
            // back.
            let previous_author =
                demo_author.map(|author| std::mem::replace(&mut profile.author, author));
            profile.demo_mode = Some(Box::new(DemoModeState { previous_author }));
            // Entering demo mode may swap the author's namespace; invalidate any
            // app-execution session bound to the pre-demo namespace.
            bump_app_execution_generation(profile);
        }

        let space = PublicSpace {
            namespace_id: namespace_hex,
            title: DEMO_SPACE_TITLE.to_string(),
            is_public: true,
        };
        profile.space = Some(space.clone());
        // After the author moved: trust markers are read against whichever
        // namespace the profile is now in.
        refresh_app_trust_markers(profile)?;
        Ok(space)
    })
}

/// Stops listing the demo space, and puts the pre-demo author back.
///
/// **This does not delete anything, and cannot.** Willow is append-only: there
/// is no delete primitive here and this does not invent one. The demo's entries
/// stay in the local store, inert and unreachable from the UI — no space lists
/// their namespace, so nothing resolves them. The bytes come back only with a
/// profile reset, which is the escape hatch that already exists.
///
/// A no-op (not an error) if demo mode was never on, and it will never un-list a
/// space it did not itself list.
pub(crate) fn hide_demo_space(inner: &Arc<Mutex<ProfileState>>) -> Result<(), MobileError> {
    with_active(inner, |profile| {
        if sync_session_is_active(profile) {
            return Err(MobileError::InvalidInput);
        }
        let Some(state) = profile.demo_mode.take() else {
            return Ok(());
        };
        if let Some(author) = state.previous_author {
            profile.author = author;
            // Leaving demo mode restores the pre-demo author, another namespace
            // swap; invalidate any session bound to the demo namespace.
            bump_app_execution_generation(profile);
        }
        profile.space = None;
        profile.preview = None;
        profile.plan = None;
        refresh_app_trust_markers(profile)?;
        Ok(())
    })
}

/// The namespace of a demo bundle whose EVERY item is valid.
///
/// The ordinary import path tolerates a bundle with some bad items: it drops
/// them and admits the rest, which is right for a peer's bundle picked up in the
/// wild. It is wrong here. The demo bundle is one known artifact, and a copy of
/// it with a flipped byte is not "the demo minus one alert" — it is damaged, and
/// admitting the surviving 18 of its 19 entries would be exactly the
/// half-imported state demo mode promises never to leave behind. So: all items
/// valid, or nothing at all.
///
/// `inspectable_entries` then proves every frame names this same namespace, so a
/// bundle that straddles namespaces is refused rather than partly imported.
fn whole_bundle_namespace_id(bytes: &[u8]) -> Result<[u8; 32], MobileError> {
    let decoded = match decode_bundle(bytes) {
        BundleDecodeOutcome::Decoded(decoded) => decoded,
        BundleDecodeOutcome::Rejected(_) => return Err(MobileError::ImportRejected),
    };
    if decoded.items.is_empty()
        || decoded
            .items
            .iter()
            .any(|item| !matches!(item.status, ItemStatus::Valid(_)))
    {
        return Err(MobileError::ImportRejected);
    }
    let first = &decoded.items[0];
    let identity = public_entry_identity(first.frame.entry_bytes())
        .map_err(|_| MobileError::ImportRejected)?;
    Ok(identity.namespace_id)
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
        let next_inventory =
            prospective_sync_inventory(profile, std::slice::from_ref(&core_signed.signed))?;
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
        install_sync_inventory(profile, next_inventory)?;
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
        // Alerts only. App-data (`apps/<app_id>/...`), app-index
        // (`app-index/<app_id>/...`), profile (`profile/<subspace>/card`), and
        // newswire (`newswire/v1/...`) entries share this store but are not
        // alerts, so exclude them the same way `ensure_complete_sync_inventory`
        // does — otherwise a single local `app_data_put`, `set_display_name`, or
        // `create_newswire_post`, or its replay on the next open, leaves a live
        // non-alert entry with no match in `profile.entries` and bricks this
        // listing with `Internal`.
        // Scope the whole listing to the ACTIVE namespace. One store holds every
        // held community's entries (Unit 3), so an unscoped scan would surface
        // another community's alert — which has no row in this community's cache
        // and would brick the board with `Internal`. Namespace-scoping is the
        // isolation boundary for the board: a switch reprojects the active
        // namespace's cache, and this only ever consults that namespace.
        let active_namespace = parse_entry_id(namespace_id)?;
        let app_index_prefix =
            riot_core::willow::Path::from_slices(&[riot_core::apps::index::APP_INDEX_COMPONENT])
                .map_err(|_| MobileError::Internal)?;
        let app_index_ids: std::collections::BTreeSet<_> = profile
            .store
            .entries_with_prefix_in_namespace(&active_namespace, &app_index_prefix)
            .map_err(map_core_error)?
            .into_iter()
            .map(|(id, _, _)| id)
            .collect();
        let all_prefix =
            riot_core::willow::Path::from_slices(&[]).map_err(|_| MobileError::Internal)?;
        let alert_ids: Vec<_> = profile
            .store
            .entries_with_prefix_in_namespace(&active_namespace, &all_prefix)
            .map_err(map_core_error)?
            .into_iter()
            .filter(|(id, entry, _)| {
                !riot_core::apps::entry::is_app_data_entry(entry)
                    && !app_index_ids.contains(id)
                    && !is_profile_prefixed(entry.path())
                    && !riot_core::newswire::is_newswire_prefix(entry.path())
                    && !riot_core::willow::site_paths::is_owned_editorial_entry(entry)
                    && !riot_core::willow::site_paths::is_owned_moderation_entry(entry)
            })
            .map(|(id, _, _)| id)
            .collect();
        let mut entries = Vec::with_capacity(alert_ids.len());
        for live_id in alert_ids {
            let live_id = hex(&live_id);
            // The store retains payloads only for imported entries, so a
            // locally-authored alert from a previous session cannot be rebuilt
            // after a reopen and simply has no projected row. Skip it rather than
            // bricking the whole board — the alert families are already excluded
            // above, so a miss here is unretained payload, not a leaked non-alert.
            if let Some(entry) = profile
                .entries
                .iter()
                .find(|entry| entry.entry_id == live_id)
            {
                entries.push(entry.clone());
            }
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
        let inspectable = inspectable_entries(&bytes, namespace_id)?;
        let entries: Vec<_> = inspectable
            .iter()
            .filter_map(|item| item.current.clone())
            .collect();
        let sync_entries: Vec<_> = inspectable.into_iter().map(|item| item.signed).collect();
        profile.ensure_handle_capacity()?;
        profile.preview = None;
        profile.plan = None;
        let preview = inspect_core(&profile.store, &bytes, &route)?;
        if preview.eligible_count().map_err(map_core_error)? != sync_entries.len() {
            return Err(MobileError::ImportRejected);
        }
        let preview_id = profile.alloc_handle_id()?;
        profile.preview = Some(StoredPreview {
            id: preview_id,
            community_generation: profile.community_generation,
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
        let found = profile
            .preview
            .as_ref()
            .filter(|preview| preview.id == preview_id)
            .map(|preview| (preview.community_generation, preview.entries.clone()))
            .ok_or(MobileError::PreviewConsumed)?;
        if !handle_generation_is_current(profile, found.0) {
            // Created in another community, before a switch — fail closed.
            profile.preview = None;
            return Err(MobileError::ObjectClosed);
        }
        Ok(found.1)
    })
}

pub(crate) fn create_plan(
    inner: &Arc<Mutex<ProfileState>>,
    preview_id: u64,
    selected_entry_ids: Vec<String>,
) -> Result<Arc<MobileImportPlan>, MobileError> {
    with_active(inner, |profile| {
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
        // A preview created before a community switch is stale — planning from it
        // would carry another community's selection into this one. Fail closed.
        let preview_generation = profile
            .preview
            .as_ref()
            .filter(|preview| preview.id == preview_id)
            .map(|preview| preview.community_generation);
        if let Some(generation) = preview_generation {
            if !handle_generation_is_current(profile, generation) {
                profile.preview = None;
                return Err(MobileError::ObjectClosed);
            }
        }
        let (selection, selected_entries, selected_sync_entries, plan) = {
            let preview = profile
                .preview
                .as_ref()
                .filter(|preview| preview.id == preview_id)
                .ok_or(MobileError::PreviewConsumed)?;
            let mut selection = Vec::with_capacity(selected_entry_ids.len());
            let mut selected_entries = Vec::with_capacity(selected_entry_ids.len());
            let mut selected_sync_entries = Vec::with_capacity(preview.sync_entries.len());
            for selected_id in &selected_entry_ids {
                let parsed_id = parse_entry_id(selected_id)?;
                let entry_index = preview
                    .entries
                    .iter()
                    .position(|entry| entry.entry_id == *selected_id)
                    .ok_or(MobileError::InvalidInput)?;
                selection.push(parsed_id);
                selected_entries.push(preview.entries[entry_index].clone());
                let signed = preview
                    .sync_entries
                    .iter()
                    .find(|signed| hex(&entry_id(&signed.entry_bytes)) == *selected_id)
                    .ok_or(MobileError::Internal)?;
                selected_sync_entries.push(signed.clone());
            }
            for signed in &preview.sync_entries {
                let id = hex(&entry_id(&signed.entry_bytes));
                if !preview.entries.iter().any(|entry| entry.entry_id == id) {
                    selection.push(entry_id(&signed.entry_bytes));
                    selected_sync_entries.push(signed.clone());
                }
            }
            if selection.is_empty() {
                return Err(MobileError::InvalidInput);
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
            community_generation: profile.community_generation,
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
        let (plan_generation, entries, sync_entries) = profile
            .plan
            .as_ref()
            .filter(|plan| plan.id == plan_id)
            .map(|plan| {
                (
                    plan.community_generation,
                    plan.entries.clone(),
                    plan.sync_entries.clone(),
                )
            })
            .ok_or(MobileError::PlanConsumed)?;
        // The community-generation guard: a plan created before a switch must
        // never commit into the current community. Drop it and fail closed —
        // this is what makes a write in flight across a switch land nowhere.
        if !handle_generation_is_current(profile, plan_generation) {
            profile.plan = None;
            profile.preview = None;
            return Err(MobileError::ObjectClosed);
        }
        let next_inventory = prospective_sync_inventory(profile, &sync_entries)?;
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
                install_sync_inventory(profile, next_inventory)?;
                advance_app_write_floor(profile, &sync_entries)?;
                refresh_app_trust_markers(profile)?;
                Ok(ImportAcceptance {
                    accepted_entry_ids: sync_entries
                        .into_iter()
                        .map(|signed| hex(&entry_id(&signed.entry_bytes)))
                        .collect(),
                })
            }
        }
    })
}

pub(crate) fn open_sync_session(
    inner: &Arc<Mutex<ProfileState>>,
) -> Result<Arc<MobileSyncSession>, MobileError> {
    with_active(inner, |profile| {
        // Clear any handle left behind by a previous community so a stale preview,
        // plan, or coordinator cannot block opening one here.
        drop_stale_handles(profile);
        if profile.preview.is_some() || profile.plan.is_some() {
            return Err(MobileError::InvalidInput);
        }
        if profile.sync_session.is_some() {
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
            community_generation: profile.community_generation,
            bridge,
            pending: None,
            followed_root: None,
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
                let terminal_without_frame =
                    terminal && !matches!(other, ByteSyncOutcome::FrameReady);
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
    let inspectable = inspectable_entries(bundle_bytes, &namespace_id)?;
    let entries: Vec<_> = inspectable
        .iter()
        .filter_map(|item| item.current.clone())
        .collect();
    let sync_entries: Vec<_> = inspectable.into_iter().map(|item| item.signed).collect();
    profile.preview = None;
    profile.plan = None;
    // The synced namespace is the followed root: admit owned editorial in
    // lockstep with `inspectable_entries` above so the eligible-count check
    // below holds (a divergence would reject the whole owned bundle).
    let followed_root = parse_entry_id(&namespace_id)?;
    let preview = inspect_core_with_root(
        &profile.store,
        bundle_bytes,
        "conference-sync",
        Some(followed_root),
    )?;
    if preview.eligible_count().map_err(map_core_error)? != sync_entries.len() {
        return Err(MobileError::ImportRejected);
    }
    prospective_sync_inventory(profile, &sync_entries)?;
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
            (pending.entries.clone(), pending.sync_entries.clone())
        };
        let next_inventory = prospective_sync_inventory(profile, &sync_entries)?;
        {
            let pending = active_sync_mut(profile, sync_id)?
                .pending
                .as_ref()
                .ok_or(MobileError::InvalidInput)?;
            let plan = pending.preview.plan_all().map_err(map_core_error)?;
            match plan.commit().map_err(map_core_error)? {
                CommitOutcome::Committed(_) | CommitOutcome::NoChanges(_) => {}
            }
        }
        active_sync_mut(profile, sync_id)?.pending = None;
        for entry in entries {
            remember_entry(&mut profile.entries, entry);
        }
        install_sync_inventory(profile, next_inventory)?;
        advance_app_write_floor(profile, &sync_entries)?;
        refresh_app_trust_markers(profile)?;
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

pub(crate) fn sync_cancel(
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
    // A sync coordinator created before a community switch is stale: the
    // generation guard drops it and reports it closed, so no sync operation
    // (and no buffered import review) can act across a switch. This is the
    // "coordinator bound to A does not carry into B" isolation, enforced at the
    // one accessor every sync operation goes through.
    let captured = match profile.sync_session.as_ref() {
        Some(session) if session.id == sync_id => Some(session.community_generation),
        _ => None,
    };
    if let Some(generation) = captured {
        if !handle_generation_is_current(profile, generation) {
            profile.sync_session = None;
            return Err(MobileError::ObjectClosed);
        }
    }
    profile
        .sync_session
        .as_mut()
        .filter(|session| session.id == sync_id)
        .ok_or(MobileError::ObjectClosed)
}

fn sync_session_is_active(profile: &LocalProfile) -> bool {
    // A sync coordinator from a previous community (stale generation) is not
    // active here — it has been left behind by a switch and will be dropped on
    // the next access. Treating it as active would wrongly block operations in
    // the newly selected community.
    let Some(generation) = profile
        .sync_session
        .as_ref()
        .map(|session| session.community_generation)
    else {
        return false;
    };
    handle_generation_is_current(profile, generation)
}

// ---------------------------------------------------------------------------
// Followed-site sync (Option C, WU2): a SECOND, independent sync session keyed
// on a followed composite site's owned namespace `root`. It reuses the
// ByteSyncSession frame protocol and the proven owned-admission core, but on a
// SEPARATE slot with two deliberate divergences from the community path:
//   1. import admits under `followed_root = root` (the SITE, not the active
//      community) and family-gates to owned /mod + /articles only (least
//      privilege, mirrors Option B's import_followed_site_bundle);
//   2. accept commits WITHOUT touching `sync_inventory` — owned-namespace
//      records must never enter the active community's peer offer set.
// The offer is `build_followed_site_offer(root)`, computed just-in-time and
// never stored (the C1a isolation property: no second unstamped set exists).
// The community drive fns above are untouched. Not yet FFI-exported; WU4 wraps
// this in a uniffi handle behind `follow_site(ticket)`.
// ---------------------------------------------------------------------------

#[allow(dead_code)] // FFI caller is WU4; exercised by in-crate tests today
pub(crate) fn open_followed_site_sync_session(
    inner: &Arc<Mutex<ProfileState>>,
    root: [u8; 32],
) -> Result<u64, MobileError> {
    with_active(inner, |profile| {
        drop_stale_handles(profile);
        // Drop a followed-site session left over from a previous community
        // (WU5 will refine the switch/lifetime interaction; invalidating on a
        // stale generation is the safe-closed default for WU2).
        if profile
            .followed_site_session
            .as_ref()
            .is_some_and(|session| session.community_generation != profile.community_generation)
        {
            profile.followed_site_session = None;
        }
        if profile.preview.is_some() || profile.plan.is_some() {
            return Err(MobileError::InvalidInput);
        }
        if profile.followed_site_session.is_some() {
            return Err(MobileError::InvalidInput);
        }
        // FOLLOWING GATE (R3): the central security check — refuse to open a
        // followed-site session for a root this profile does not follow, so a
        // hostile ticket can never make the app sync an unfollowed owned
        // namespace. Mirrors Option B's import_followed_site_bundle gate.
        let following = profile
            .registry
            .find(&root)
            .is_some_and(|record| record.relationship == Relationship::Following);
        if !following {
            return Err(MobileError::ImportRejected);
        }
        let offer = build_followed_site_offer(profile, &root)?;
        let bridge = ByteSyncSession::new(root, offer).map_err(map_sync_error)?;
        let sync_id = profile.alloc_handle_id()?;
        profile.followed_site_session = Some(StoredSyncSession {
            id: sync_id,
            community_generation: profile.community_generation,
            bridge,
            pending: None,
            followed_root: Some(root),
        });
        Ok(sync_id)
    })
}

#[allow(dead_code)] // FFI caller is WU4; exercised by in-crate tests today
fn active_followed_mut(
    profile: &mut LocalProfile,
    sync_id: u64,
) -> Result<&mut StoredSyncSession, MobileError> {
    let captured = match profile.followed_site_session.as_ref() {
        Some(session) if session.id == sync_id => Some(session.community_generation),
        _ => None,
    };
    if let Some(generation) = captured {
        if !handle_generation_is_current(profile, generation) {
            profile.followed_site_session = None;
            return Err(MobileError::ObjectClosed);
        }
    }
    profile
        .followed_site_session
        .as_mut()
        .filter(|session| session.id == sync_id)
        .ok_or(MobileError::ObjectClosed)
}

#[allow(dead_code)] // FFI caller is WU4; exercised by in-crate tests today
pub(crate) fn followed_sync_begin(
    inner: &Arc<Mutex<ProfileState>>,
    sync_id: u64,
) -> Result<SyncOutcome, MobileError> {
    with_active(inner, |profile| {
        let session = active_followed_mut(profile, sync_id)?;
        let outcome = session.bridge.begin().map_err(map_sync_error)?;
        outcome_without_import(outcome, session.bridge.is_terminal())
    })
}

#[allow(dead_code)] // FFI caller is WU4; exercised by in-crate tests today
pub(crate) fn followed_sync_receive_frame(
    inner: &Arc<Mutex<ProfileState>>,
    sync_id: u64,
    frame_bytes: Vec<u8>,
) -> Result<SyncOutcome, MobileError> {
    with_active(inner, |profile| {
        let outcome = active_followed_mut(profile, sync_id)?
            .bridge
            .receive_bytes(&frame_bytes)
            .map_err(map_sync_error)?;
        match outcome {
            ByteSyncOutcome::ImportBundle(bundle_bytes) => {
                match prepare_followed_site_import(profile, sync_id, &bundle_bytes) {
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
                        let session = active_followed_mut(profile, sync_id)?;
                        let outcome = session
                            .bridge
                            .import_rejected(code)
                            .map_err(map_sync_error)?;
                        outcome_without_import(outcome, session.bridge.is_terminal())
                    }
                }
            }
            other => {
                let terminal = active_followed_mut(profile, sync_id)?.bridge.is_terminal();
                let terminal_without_frame =
                    terminal && !matches!(other, ByteSyncOutcome::FrameReady);
                let result = outcome_without_import(other, terminal);
                if terminal_without_frame {
                    profile.followed_site_session = None;
                }
                result
            }
        }
    })
}

#[allow(dead_code)] // FFI caller is WU4; exercised by in-crate tests today
fn prepare_followed_site_import(
    profile: &mut LocalProfile,
    sync_id: u64,
    bundle_bytes: &[u8],
) -> Result<SyncOutcome, MobileError> {
    let root = active_followed_mut(profile, sync_id)?
        .followed_root
        .ok_or(MobileError::Internal)?;
    let namespace_hex = hex(&root);
    let inspectable = inspectable_entries(bundle_bytes, &namespace_hex)?;
    // FAMILY GATE (R2): a followed-site bundle may carry ONLY owned /mod +
    // /articles. Any other family (a communal alert/newswire entry, a profile
    // card, a third namespace's entry) makes the whole bundle untrustworthy —
    // reject it all-or-nothing. Routed through the SAME canonical predicate the
    // manual (Option B) and transport (WU3) paths use, so the gate cannot drift.
    for item in &inspectable {
        let entry = riot_core::willow::decode_entry_canonic(&item.signed.entry_bytes)
            .map_err(|_| MobileError::ImportRejected)?;
        if !riot_core::site::is_followed_site_family(&entry) {
            return Err(MobileError::ImportRejected);
        }
    }
    let entries: Vec<_> = inspectable
        .iter()
        .filter_map(|item| item.current.clone())
        .collect();
    let sync_entries: Vec<_> = inspectable.into_iter().map(|item| item.signed).collect();
    profile.preview = None;
    profile.plan = None;
    // Admit under the SITE root (not the active community): owned /mod +
    // /articles are admitted under a cap rooted at `root`. This is the same
    // owned-admission core Option B proved; only the root wiring differs from
    // the community path's `followed_root = active namespace`.
    let preview =
        inspect_core_with_root(&profile.store, bundle_bytes, "site-follow-sync", Some(root))?;
    if preview.eligible_count().map_err(map_core_error)? != sync_entries.len() {
        return Err(MobileError::ImportRejected);
    }
    // DELIBERATELY no prospective_sync_inventory: owned-namespace records must
    // never enter the community offer set.
    active_followed_mut(profile, sync_id)?.pending = Some(StoredSyncImport {
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

#[allow(dead_code)] // FFI caller is WU4; exercised by in-crate tests today
pub(crate) fn followed_sync_take_outbound_frame(
    inner: &Arc<Mutex<ProfileState>>,
    sync_id: u64,
) -> Result<Option<Vec<u8>>, MobileError> {
    with_active(inner, |profile| {
        let (frame, terminal) = {
            let session = active_followed_mut(profile, sync_id)?;
            let frame = session.bridge.take_outbound_frame();
            (frame, session.bridge.is_terminal())
        };
        if terminal && frame.is_some() {
            profile.followed_site_session = None;
        }
        Ok(frame)
    })
}

#[allow(dead_code)] // FFI caller is WU4; exercised by in-crate tests today
pub(crate) fn followed_sync_accept_import(
    inner: &Arc<Mutex<ProfileState>>,
    sync_id: u64,
) -> Result<SyncOutcome, MobileError> {
    with_active(inner, |profile| {
        let entries = {
            let pending = active_followed_mut(profile, sync_id)?
                .pending
                .as_ref()
                .ok_or(MobileError::InvalidInput)?;
            pending.entries.clone()
        };
        {
            let pending = active_followed_mut(profile, sync_id)?
                .pending
                .as_ref()
                .ok_or(MobileError::InvalidInput)?;
            let plan = pending.preview.plan_all().map_err(map_core_error)?;
            match plan.commit().map_err(map_core_error)? {
                CommitOutcome::Committed(_) | CommitOutcome::NoChanges(_) => {}
            }
        }
        active_followed_mut(profile, sync_id)?.pending = None;
        for entry in entries {
            remember_entry(&mut profile.entries, entry);
        }
        // DELIBERATELY NOT install_sync_inventory: this is the one divergence
        // from the community accept. Owned-namespace records committed here must
        // not enter the active community's peer offer set — the isolation
        // invariant the community `install_sync_inventory` equality guard
        // protects. `track_committed_entry` is likewise not called.
        let session = active_followed_mut(profile, sync_id)?;
        let outcome = session.bridge.import_accepted().map_err(map_sync_error)?;
        outcome_without_import(outcome, session.bridge.is_terminal())
    })
}

#[allow(dead_code)] // FFI caller is WU4; exercised by in-crate tests today
pub(crate) fn followed_sync_reject_import(
    inner: &Arc<Mutex<ProfileState>>,
    sync_id: u64,
    code: u8,
) -> Result<SyncOutcome, MobileError> {
    with_active(inner, |profile| {
        let session = active_followed_mut(profile, sync_id)?;
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

#[allow(dead_code)] // FFI caller is WU4; exercised by in-crate tests today
pub(crate) fn followed_sync_cancel(
    inner: &Arc<Mutex<ProfileState>>,
    sync_id: u64,
) -> Result<(), MobileError> {
    with_active(inner, |profile| {
        match profile.followed_site_session.as_ref() {
            Some(session) if session.id == sync_id => {
                profile.followed_site_session = None;
                Ok(())
            }
            Some(_) => Err(MobileError::ObjectClosed),
            None => Ok(()),
        }
    })
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

pub(crate) fn with_active<T>(
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

pub(crate) fn inspect_core(
    store: &EvidenceStore,
    bytes: &[u8],
    route: &str,
) -> Result<ImportPreview, MobileError> {
    inspect_core_with_root(store, bytes, route, None)
}

/// `inspect_core` for an admission path that knows the owned site it follows.
/// The sync commit path passes the synced namespace so owned editorial entries
/// are admitted in lockstep with `inspectable_entries` (both keyed on that same
/// namespace); local/self routes pass `None` and stay fail-closed for owned.
pub(crate) fn inspect_core_with_root(
    store: &EvidenceStore,
    bytes: &[u8],
    route: &str,
    followed_site_root: Option<[u8; 32]>,
) -> Result<ImportPreview, MobileError> {
    let context = match followed_site_root {
        Some(root) => ImportContext::with_followed_root(route, root),
        None => ImportContext::new(route),
    };
    match store.inspect(bytes, context).map_err(map_core_error)? {
        InspectOutcome::Preview(preview) => Ok(preview),
        InspectOutcome::Rejected(_) => Err(MobileError::ImportRejected),
    }
}

fn inspectable_entries(
    bytes: &[u8],
    expected_namespace_id: &str,
) -> Result<Vec<InspectableEntry>, MobileError> {
    // Every entry must already be in `expected_namespace_id`, so for an owned
    // site that namespace IS the followed root: decoding with it admits owned
    // editorial entries here exactly as the commit path (`inspect_core`) does,
    // keeping the two in lockstep (a divergence would fail the eligible-count
    // check in `prepare_sync_import` and reject the whole bundle). Communal
    // namespaces ignore the root.
    let followed_root = parse_entry_id(expected_namespace_id)?;
    let decoded = match decode_bundle_with_root(bytes, Some(followed_root)) {
        BundleDecodeOutcome::Decoded(decoded) => decoded,
        BundleDecodeOutcome::Rejected(_) => return Err(MobileError::ImportRejected),
    };
    let mut entries = Vec::new();
    for item in decoded.items {
        let ItemStatus::Valid(valid) = item.status else {
            continue;
        };
        let decoded_entry = riot_core::willow::decode_entry_canonic(item.frame.entry_bytes())
            .map_err(|_| MobileError::ImportRejected)?;
        let identity = public_entry_identity(item.frame.entry_bytes())
            .map_err(|_| MobileError::ImportRejected)?;
        let namespace_id = hex(&identity.namespace_id);
        if namespace_id != expected_namespace_id {
            return Err(MobileError::ImportRejected);
        }
        // App, profile, and newswire entries sync and commit like any other, but
        // they are not alerts and carry no alert row. Anything else must decode
        // AS an alert — so a payload that is not one is rejected outright.
        //
        // Profile cards must be listed here explicitly. Without it a synced card
        // falls into the alert branch below, `decode_alert` fails on a
        // profile-card payload, and the ENTIRE import is rejected — which would
        // mean a display name could never reach another device at all.
        let is_non_alert = riot_core::apps::entry::is_app_data_entry(&decoded_entry)
            || riot_core::apps::index::classify_app_index_path(decoded_entry.path()).is_some()
            || is_profile_prefixed(decoded_entry.path())
            || riot_core::newswire::is_newswire_prefix(decoded_entry.path())
            || riot_core::willow::site_paths::is_owned_editorial_entry(&decoded_entry)
            || riot_core::willow::site_paths::is_owned_moderation_entry(&decoded_entry);
        let current = if is_non_alert {
            None
        } else {
            let alert = decode_alert(item.frame.payload_bytes())
                .map_err(|_| MobileError::ImportRejected)?;
            if !alert_entry_path_matches_payload(
                item.frame.entry_bytes(),
                &alert.object_id,
                &alert.revision_id,
            )
            .map_err(|_| MobileError::ImportRejected)?
            {
                return Err(MobileError::ImportRejected);
            }
            Some(CurrentEntry {
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
            })
        };
        entries.push(InspectableEntry {
            current,
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

/// Track a locally-committed signed entry in the ACTIVE community's sync
/// inventory so it can traverse the nearby bridge (Risk 16 — the newswire
/// create/post path, which previously committed without tracking, so newswire
/// content could never be shared). Mirrors how the alert sign/import paths keep
/// the inventory complete. A newswire entry IS a live entry in the active
/// namespace, so this PRESERVES the load-bearing
/// `inventory == active_namespace_live_ids` invariant (the isolation guarantee):
/// it only ever adds an entry that already belongs to the active namespace, and
/// `install_sync_inventory` re-checks that equality and fails closed otherwise.
pub(crate) fn track_committed_entry(
    profile: &mut LocalProfile,
    signed: &SignedWillowEntry,
) -> Result<(), MobileError> {
    let next = prospective_sync_inventory(profile, std::slice::from_ref(signed))?;
    install_sync_inventory(profile, next)
}

fn prospective_sync_inventory(
    profile: &LocalProfile,
    incoming: &[SignedWillowEntry],
) -> Result<Vec<SignedWillowEntry>, MobileError> {
    let mut candidates = profile.sync_inventory.clone();
    for signed in incoming {
        let id = entry_id(&signed.entry_bytes);
        if !candidates
            .iter()
            .any(|known| entry_id(&known.entry_bytes) == id)
        {
            candidates.push(signed.clone());
        }
    }

    // Simulate Willow's full prefix-pruning relation so overwritten proofs
    // leave the candidate set before count/byte accounting. Proofs are never
    // rebuilt from store metadata.
    let decoded: Vec<_> = candidates
        .iter()
        .map(|signed| {
            riot_core::willow::decode_entry_canonic(&signed.entry_bytes)
                .map_err(|_| MobileError::Internal)
        })
        .collect::<Result<_, _>>()?;
    let ids: Vec<_> = candidates
        .iter()
        .map(|signed| entry_id(&signed.entry_bytes))
        .collect();
    let keep: Vec<_> = decoded
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            !decoded.iter().enumerate().any(|(other_index, other)| {
                ids[other_index] != ids[index] && other.prunes(candidate)
            })
        })
        .collect();
    candidates = candidates
        .into_iter()
        .zip(keep)
        .filter_map(|(signed, keep)| keep.then_some(signed))
        .collect();
    candidates.sort_unstable_by_key(|signed| entry_id(&signed.entry_bytes));
    if candidates.len() > MAX_SYNC_IDS {
        return Err(MobileError::SessionLimit);
    }
    let encoded = encode_bundle(&candidates).map_err(|_| MobileError::SessionLimit)?;
    if encoded.len() > MAX_SYNC_INVENTORY_BYTES {
        return Err(MobileError::SessionLimit);
    }
    Ok(candidates)
}

/// The live entry ids in the ACTIVE community's namespace. The store holds every
/// held community's entries (Unit 3), but sync — like the board — is scoped to
/// the selected community, so the inventory is built and checked against exactly
/// this namespace, never the whole store.
fn active_namespace_live_ids(
    profile: &LocalProfile,
) -> Result<Vec<riot_core::willow::EntryId>, MobileError> {
    let Some(space) = profile.space.as_ref() else {
        // No community is listed yet — e.g. mid-`load_demo_space`, which installs
        // the inventory before it lists the demo space. The store is single
        // namespace at that point, so the whole store is the right inventory.
        //
        // ⚠️ LOAD-BEARING for isolation: this whole-store fallback is only safe
        // because `space == None` occurs solely in single-namespace contexts (a
        // switch always sets `space`). If a future flow ever holds MULTI-community
        // data while `space` is `None`, this silently WIDENS the sync inventory to
        // the whole store — leaking one community's entries to another's peers.
        // Any such flow must set `space` first (or scope this call explicitly).
        return profile.store.live_entry_ids().map_err(map_core_error);
    };
    let namespace_id = parse_entry_id(&space.namespace_id)?;
    namespace_live_ids(profile, &namespace_id)
}

/// The live entry ids in `namespace_id` — the active-community scoping of
/// `active_namespace_live_ids` generalized to ANY namespace, so a followed
/// site's owned namespace can be queried the same way. It is a namespace-scoped
/// prefix query and therefore can never return another namespace's ids: the
/// same offer-isolation property the community inventory relies on, applied per
/// namespace. Reads only; the community `sync_inventory` and its equality guard
/// are never touched by this helper. `active_namespace_live_ids` delegates here
/// for the active namespace, so that path stays byte-identical.
fn namespace_live_ids(
    profile: &LocalProfile,
    namespace_id: &[u8; 32],
) -> Result<Vec<riot_core::willow::EntryId>, MobileError> {
    let all_prefix =
        riot_core::willow::Path::from_slices(&[]).map_err(|_| MobileError::Internal)?;
    Ok(profile
        .store
        .entries_with_prefix_in_namespace(namespace_id, &all_prefix)
        .map_err(map_core_error)?
        .into_iter()
        .map(|(id, _, _)| id)
        .collect())
}

/// The just-in-time followed-site OFFER for `namespace_id`: exactly that
/// namespace's live entries in their full signed form, read verbatim from the
/// durable store — the entries a followed-site sync session hands to a peer.
///
/// This is the C1a primitive that keeps the community-isolation invariant
/// untouched: the offer is DERIVED here and RETURNED, never stored in
/// `profile.sync_inventory`. So no second unstamped set exists that a later
/// community switch could offer to the wrong peer. It applies the SAME
/// discipline as `install_sync_inventory`, but per namespace: the offer must
/// equal EXACTLY `namespace_live_ids(namespace_id)` (fail closed otherwise), and
/// the same `MAX_SYNC_IDS` / `MAX_SYNC_INVENTORY_BYTES` ceilings.
///
/// Fails closed on a memory-backed store: `signed_entries_in_namespace` returns
/// `None` (the join drops cap/sig), so an offer cannot be built — followed-site
/// sync requires a durable profile.
// Consumed by the followed-site drive fns below (FFI-wired in WU4).
#[allow(dead_code)]
pub(crate) fn build_followed_site_offer(
    profile: &LocalProfile,
    namespace_id: &[u8; 32],
) -> Result<Vec<SignedWillowEntry>, MobileError> {
    // Durable-only: `None` is the explicit "no signed form on a memory store"
    // signal — never treat it as an empty offer.
    let signed = profile
        .store
        .signed_entries_in_namespace(namespace_id)
        .map_err(map_core_error)?
        .ok_or(MobileError::Internal)?;

    let mut live_ids = namespace_live_ids(profile, namespace_id)?;
    live_ids.sort_unstable();

    let mut offer = signed;
    offer.retain(|entry| {
        live_ids
            .binary_search(&entry_id(&entry.entry_bytes))
            .is_ok()
    });
    offer.sort_unstable_by_key(|entry| entry_id(&entry.entry_bytes));
    let offer_ids: Vec<_> = offer
        .iter()
        .map(|entry| entry_id(&entry.entry_bytes))
        .collect();
    // The per-namespace analog of the `install_sync_inventory` equality guard.
    // The offer MUST be exactly this namespace's live set — no more (would leak),
    // no less (would silently under-serve). It is returned, not stored.
    if offer_ids != live_ids {
        return Err(MobileError::Internal);
    }
    if offer.len() > MAX_SYNC_IDS {
        return Err(MobileError::SessionLimit);
    }
    let encoded = encode_bundle(&offer).map_err(|_| MobileError::SessionLimit)?;
    if encoded.len() > MAX_SYNC_INVENTORY_BYTES {
        return Err(MobileError::SessionLimit);
    }
    Ok(offer)
}

fn install_sync_inventory(
    profile: &mut LocalProfile,
    mut inventory: Vec<SignedWillowEntry>,
) -> Result<(), MobileError> {
    let mut live_ids = active_namespace_live_ids(profile)?;
    live_ids.sort_unstable();
    inventory.retain(|signed| live_ids.contains(&entry_id(&signed.entry_bytes)));
    inventory.sort_unstable_by_key(|signed| entry_id(&signed.entry_bytes));
    let inventory_ids: Vec<_> = inventory
        .iter()
        .map(|signed| entry_id(&signed.entry_bytes))
        .collect();
    // ⚠️ LOAD-BEARING for isolation: the inventory (which is unstamped, unlike the
    // generation-guarded handles) MUST equal exactly the active namespace's live
    // ids before it can reach a peer. This equality is what keeps a switch's
    // re-scoping the sole thing standing between one community's inventory and
    // another's peers — do not relax it to a subset/superset check.
    if inventory_ids != live_ids {
        return Err(MobileError::Internal);
    }
    profile.sync_inventory = inventory;
    Ok(())
}

fn advance_app_write_floor(
    profile: &mut LocalProfile,
    entries: &[SignedWillowEntry],
) -> Result<(), MobileError> {
    for signed in entries {
        let entry = riot_core::willow::decode_entry_canonic(&signed.entry_bytes)
            .map_err(|_| MobileError::Internal)?;
        // Profile cards ride this floor too. They are last-write-wins on ONE
        // coordinate, so a rename must land at a strictly later timestamp than
        // the name it replaces. Without this, two `set_display_name` calls in
        // the same wall-clock second both get `now * 1e6`, the second is an
        // equal-timestamp write, and Willow keeps the OLD name — a rename that
        // silently does nothing.
        let is_local_write = riot_core::apps::entry::is_app_data_entry(&entry)
            || riot_core::apps::index::classify_app_index_path(entry.path()).is_some()
            || is_profile_prefixed(entry.path());
        if is_local_write {
            let timestamp = riot_core::willow::entry_timestamp_micros(&signed.entry_bytes)
                .map_err(|_| MobileError::Internal)?;
            profile.app_data_timestamp_floor_micros =
                profile.app_data_timestamp_floor_micros.max(timestamp);
        }
    }
    Ok(())
}

fn ensure_complete_sync_inventory(profile: &LocalProfile) -> Result<(), MobileError> {
    let mut live_ids = active_namespace_live_ids(profile)?;
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
    let encoded = encode_bundle(&profile.sync_inventory).map_err(|_| MobileError::SessionLimit)?;
    if encoded.len() > MAX_SYNC_INVENTORY_BYTES {
        return Err(MobileError::SessionLimit);
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

pub(crate) fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        value.push(HEX[(byte >> 4) as usize] as char);
        value.push(HEX[(byte & 0x0f) as usize] as char);
    }
    value
}

pub(crate) fn map_core_error(error: riot_core::session::SessionError) -> MobileError {
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
        WillowError::InvalidAlert(_)
        | WillowError::NamespaceNotCommunal
        | WillowError::DelegationAreaEscapesArticles
        | WillowError::DelegationAreaEscapesMod => MobileError::InvalidInput,
        WillowError::SealedIdentityInvalid | WillowError::SealedMastheadInvalid => {
            MobileError::InvalidInput
        }
        WillowError::IdentitySealFailed => MobileError::Internal,
        WillowError::PathInvalid
        | WillowError::DoesNotAuthorise
        | WillowError::DecodeFailed
        | WillowError::TrailingBytes => MobileError::Internal,
    }
}

// ---------------------------------------------------------------------------
// Signed-JS-apps runtime surface (see apps_ffi.rs).
// ---------------------------------------------------------------------------

pub(crate) fn install_app(
    inner: &Arc<Mutex<ProfileState>>,
    manifest_bytes: Vec<u8>,
    bundle_bytes: Vec<u8>,
) -> Result<crate::apps_ffi::InstalledAppRecord, MobileError> {
    with_active(inner, |profile| {
        install_pair(profile, manifest_bytes, bundle_bytes)
    })
}

/// Install an app this profile already holds — one that arrived over nearby
/// sync, was published here, or is built into Riot. The manifest and bundle
/// bytes are resolved locally rather than passed in, which is what lets a
/// neighbour's app be *opened* and not merely listed.
///
/// It goes through the same `install_pair` as a direct `install_app`, so a
/// carried or built-in app can never enter the runtime on weaker terms than one
/// installed from bytes the caller supplied.
pub(crate) fn install_from_directory(
    inner: &Arc<Mutex<ProfileState>>,
    app_id: Vec<u8>,
) -> Result<crate::apps_ffi::InstalledAppRecord, MobileError> {
    let app_id = exact_app_id(&app_id)?;
    with_active(inner, |profile| {
        // `AppRejected` covers every way an app can be un-openable from here:
        // never arrived, bundle still in flight, or nothing local but copies
        // that fail to re-derive this id. None of them is a distinct outcome
        // to the caller — the app simply cannot be opened yet.
        let pair = resolve_app_payload_bytes(profile, &app_id)?;
        install_pair(profile, pair.manifest_bytes, pair.bundle_bytes)
    })
}

/// The manifest and bundle bytes a native host needs to *serve* a held app's
/// pages and re-admit it after a relaunch. `install_from_directory` admits an
/// app into the runtime, but the WebView still has to render it, and a carried
/// app has no local file to read — the store holds the only copy. Resolved from
/// the same three sources, so a built-in serves its pages exactly as a carried
/// app does.
///
/// Both halves come from ONE verified resolution, so they can never disagree.
pub(crate) fn app_pair_bytes(
    inner: &Arc<Mutex<ProfileState>>,
    app_id: Vec<u8>,
) -> Result<crate::apps_ffi::AppPairBytes, MobileError> {
    let app_id = exact_app_id(&app_id)?;
    with_active(inner, |profile| {
        let pair = resolve_app_payload_bytes(profile, &app_id)?;
        Ok(crate::apps_ffi::AppPairBytes {
            manifest_bytes: pair.manifest_bytes,
            bundle_bytes: pair.bundle_bytes,
        })
    })
}

/// The one install path into the runtime. Verifies the canonical pair
/// invariant, then records the app against this profile under the install cap.
fn install_pair(
    profile: &mut LocalProfile,
    manifest_bytes: Vec<u8>,
    bundle_bytes: Vec<u8>,
) -> Result<crate::apps_ffi::InstalledAppRecord, MobileError> {
    use riot_core::apps::index::verify_app_pair;
    use riot_core::apps::manifest::decode_manifest;

    // The single canonical pair invariant; the manifest re-decode below
    // only extracts display fields from bytes verify_app_pair accepted.
    let app_id = verify_app_pair(&manifest_bytes, &bundle_bytes).map_err(map_apps_error)?;
    let manifest = decode_manifest(&manifest_bytes).map_err(map_apps_error)?;

    if !profile
        .installed_apps
        .iter()
        .any(|app| app.app_id == app_id)
    {
        if profile.installed_apps.len() >= MAX_INSTALLED_APPS {
            return Err(MobileError::SessionLimit);
        }
        profile.installed_apps.push(StoredInstalledApp {
            app_id,
            manifest_bytes,
            bundle_bytes,
        });
    }
    Ok(crate::apps_ffi::InstalledAppRecord {
        app_id: hex(&app_id),
        app_id_bytes: app_id.to_vec(),
        name: manifest.name,
        description: manifest.description,
        version: manifest.version,
        entry_point: manifest.entry_point,
        permissions: manifest.permissions,
    })
}

/// The subspace recognized as this space's organizer. A space's namespace ID is
/// its creator's subspace key (`generate_space_organizer_author`), so the
/// organizer is derivable by every member from the space alone — no extra field,
/// no key exchange.
fn space_organizer_subspace_id(profile: &LocalProfile) -> [u8; 32] {
    profile.author.identity().namespace_id
}

/// True when this profile is the space's organizer (its subspace key is the
/// namespace). Only the organizer may approve apps for the space.
fn is_space_organizer(profile: &LocalProfile) -> bool {
    *profile.author.subspace_id().as_bytes() == space_organizer_subspace_id(profile)
}

/// Why this profile may not approve an app here — or `None` when it may.
///
/// The refusal is the SAME in both arms (the organizer gate does not move); only
/// the sentence differs, because the two people need opposite advice. Splitting
/// them is the whole point: `InvalidInput` used to cover both, which is how a
/// silent, unexplained failure reached someone who had done nothing wrong.
fn organizer_refusal(profile: &LocalProfile) -> Option<MobileError> {
    if is_space_organizer(profile) {
        return None;
    }
    if profile.joined_others_space {
        // A member of someone else's space. Working as designed.
        Some(MobileError::NotSpaceOrganizer)
    } else {
        // Not organizer-shaped, yet never joined anyone: a profile minted before
        // organizers existed, sitting in a space it created and cannot prove it
        // created. No migration exists — see `LegacyProfileCannotOrganize`.
        Some(MobileError::LegacyProfileCannotOrganize)
    }
}

/// Whether this profile may approve apps for its space — the question the review
/// sheet asks BEFORE offering "Let everyone here use this".
///
/// A button that cannot succeed should not be drawn. This is what lets the sheet
/// show the honest alternative instead, and it is deliberately a plain query: it
/// grants nothing, and the gate in `set_app_trust` is still enforced independently.
pub(crate) fn is_organizer(inner: &Arc<Mutex<ProfileState>>) -> Result<bool, MobileError> {
    with_active(inner, |profile| Ok(is_space_organizer(profile)))
}

/// Whether this profile could EVER organize a space (its author is
/// organizer-shaped). False only for pre-organizer "legacy" profiles, and the
/// signal the UI uses to say "start a new profile" rather than "ask the organizer".
pub(crate) fn can_organize(inner: &Arc<Mutex<ProfileState>>) -> Result<bool, MobileError> {
    with_active(inner, |profile| {
        Ok(!matches!(
            organizer_refusal(profile),
            Some(MobileError::LegacyProfileCannotOrganize)
        ))
    })
}

// ===========================================================================
// Multiple communities (Unit 3): a durable, isolated, per-community-identity
// registry. Each community keeps its OWN author (unlinkable), sealed at rest
// through the existing `seal_identity` mechanism; a deliberate switch is the
// only path that unseals another community, and it re-seals the outgoing one
// and reprojects on the way. Communities are isolated: an entry, an approval,
// or in-flight work in one never leaks into another, and a write or import in
// flight across a switch fails closed.
// ===========================================================================

fn relationship_to_ffi(relationship: Relationship) -> CommunityRelationship {
    match relationship {
        Relationship::Organizer => CommunityRelationship::Organizer,
        Relationship::Member => CommunityRelationship::Member,
        Relationship::PublicReader => CommunityRelationship::PublicReader,
        Relationship::Following => CommunityRelationship::Following,
        Relationship::Personal => CommunityRelationship::Personal,
    }
}

/// Build the FFI row for one community. `available` answers "can this open right
/// now": not archived, not quarantined, and its author is loadable (sealed at
/// rest, parked in this session, or currently active). Listing never unseals.
fn community_row(profile: &LocalProfile, record: &CommunityRecord) -> CommunityRow {
    let loadable = record.sealed_author.is_some()
        || profile.community_authors.contains_key(&record.namespace_id)
        || profile.registry.active == Some(record.namespace_id);
    CommunityRow {
        namespace_id: hex(&record.namespace_id),
        title: record.title.clone(),
        relationship: relationship_to_ffi(record.relationship),
        descriptor_entry_id: record.descriptor_entry_id.as_ref().map(|id| hex(id)),
        recent_activity_unix_seconds: record.last_activity_unix_seconds,
        sync_freshness_unix_seconds: record.last_sync_unix_seconds,
        archived: record.archived,
        quarantined: record.quarantined,
        available: !record.archived && !record.quarantined && loadable,
    }
}

/// Encode and flush the registry to durable `local_state`. A no-op for an
/// in-memory profile, whose registry lives only for the session.
pub(crate) fn persist_registry(profile: &LocalProfile) -> Result<(), MobileError> {
    if let Some(db) = profile.db.as_ref() {
        db.set_local_state(REGISTRY_KEY, &profile.registry.encode())
            .map_err(map_database_error)?;
    }
    Ok(())
}

/// Register (or refresh the metadata of) the ACTIVE community from the current
/// author + listed space, and mark it active. Called after create/join. The
/// sealed author is added later by `persist_communities`/`switch_community`
/// when the wrapping key is available; metadata persists eagerly so the chooser
/// survives a reopen even before the author is sealed.
pub(crate) fn register_active_community(
    profile: &mut LocalProfile,
    descriptor_entry_id: Option<[u8; 32]>,
) -> Result<(), MobileError> {
    let Some(space) = profile.space.clone() else {
        return Ok(());
    };
    let namespace_id = parse_entry_id(&space.namespace_id)?;
    let relationship = if is_space_organizer(profile) {
        Relationship::Organizer
    } else {
        Relationship::Member
    };
    profile.registry.upsert(CommunityRecord {
        namespace_id,
        title: space.title,
        relationship,
        sealed_author: None,
        descriptor_entry_id,
        archived: false,
        quarantined: false,
        last_activity_unix_seconds: None,
        last_sync_unix_seconds: None,
    });
    profile.registry.active = Some(namespace_id);
    persist_registry(profile)?;
    Ok(())
}

/// Rebuild the active community's in-memory alert projection from the store,
/// scoped to exactly the active namespace. This is the rehydration that closes
/// Risk 11 for the legacy board — after a switch or reopen `list_current_entries`
/// reflects the active community and NOTHING from any other — and it is the
/// isolation guarantee for alert content across a switch.
fn reproject_active(profile: &mut LocalProfile) -> Result<(), MobileError> {
    use willow25::groupings::{Keylike, Namespaced};
    profile.entries.clear();
    let Some(space) = profile.space.clone() else {
        return Ok(());
    };
    let namespace_id = parse_entry_id(&space.namespace_id)?;
    let all_prefix =
        riot_core::willow::Path::from_slices(&[]).map_err(|_| MobileError::Internal)?;
    let prefixed = profile
        .store
        .entries_with_prefix_in_namespace(&namespace_id, &all_prefix)
        .map_err(map_core_error)?;
    let mut newest_activity: Option<u64> = None;
    for (id, entry, payload) in prefixed {
        // Alerts only; app-data, app-index, profile, newswire, and owned
        // editorial entries share the store but carry no alert row (mirrors
        // `list_current_entries`).
        if riot_core::apps::entry::is_app_data_entry(&entry)
            || riot_core::apps::index::classify_app_index_path(entry.path()).is_some()
            || is_profile_prefixed(entry.path())
            || riot_core::newswire::is_newswire_prefix(entry.path())
            || riot_core::willow::site_paths::is_owned_editorial_entry(&entry)
            || riot_core::willow::site_paths::is_owned_moderation_entry(&entry)
        {
            continue;
        }
        let Some(payload) = payload else {
            continue;
        };
        let Ok(alert) = decode_alert(&payload) else {
            continue;
        };
        newest_activity =
            Some(newest_activity.map_or(alert.created_at, |m| m.max(alert.created_at)));
        profile.entries.push(CurrentEntry {
            entry_id: hex(&id),
            namespace_id: hex(entry.namespace_id().as_bytes()),
            signer_id: hex(entry.subspace_id().as_bytes()),
            headline: alert.headline,
            freshness: AlertFreshness {
                created_at: alert.created_at,
                valid_from: alert.valid_from,
                expires_at: alert.expires_at,
            },
            ai_assisted: alert.ai_assisted,
        });
    }
    if let Some(active) = profile.registry.active {
        if let Some(record) = profile.registry.find_mut(&active) {
            if newest_activity.is_some() {
                record.last_activity_unix_seconds = newest_activity;
            }
        }
    }
    Ok(())
}

/// On a durable reopen, swap the active community's OWN sealed author in over
/// the just-restored primary identity and reproject its content, so a reopen
/// lands on the same community with the same Home. A corrupt at-rest author is
/// quarantined (retained for recovery), never dropped.
fn restore_active_community(
    profile_arc: &Arc<MobileProfile>,
    key: &[u8; 32],
) -> Result<(), MobileError> {
    with_active(&profile_arc.inner, |profile| {
        let Some(active_ns) = profile.registry.active else {
            return Ok(());
        };
        let Some(record) = profile.registry.find(&active_ns) else {
            return Ok(());
        };
        if record.archived || record.quarantined {
            return Ok(());
        }
        let Some(sealed) = record.sealed_author.clone() else {
            return Ok(());
        };
        match EvidenceAuthor::open_sealed_identity(key, &sealed) {
            Ok(author) => {
                let title = record.title.clone();
                profile.author = author;
                profile.space = Some(PublicSpace {
                    namespace_id: hex(&active_ns),
                    title,
                    is_public: true,
                });
                profile.joined_others_space = !is_space_organizer(profile);
                reproject_active(profile)?;
            }
            Err(_) => {
                if let Some(record) = profile.registry.find_mut(&active_ns) {
                    record.quarantined = true;
                }
                persist_registry(profile)?;
            }
        }
        Ok(())
    })
}

pub(crate) fn list_communities(
    inner: &Arc<Mutex<ProfileState>>,
) -> Result<Vec<CommunityRow>, MobileError> {
    with_active(inner, |profile| {
        let active = profile.registry.active;
        let mut rows: Vec<CommunityRow> = profile
            .registry
            .communities
            .iter()
            // Followed composite sites are author-less: they are surfaced through
            // `list_followed_sites`, never as a `CommunityRow`. `Personal` stays
            // IN (it is author-bearing and rides the community list).
            .filter(|record| record.relationship != Relationship::Following)
            .map(|record| community_row(profile, record))
            .collect();
        let active_hex = active.map(|ns| hex(&ns));
        // Active community first, then most recent activity, then title — a
        // stable, plain-language order for the chooser.
        rows.sort_by(|a, b| {
            let a_active = active_hex.as_deref() == Some(a.namespace_id.as_str());
            let b_active = active_hex.as_deref() == Some(b.namespace_id.as_str());
            b_active
                .cmp(&a_active)
                .then_with(|| {
                    b.recent_activity_unix_seconds
                        .cmp(&a.recent_activity_unix_seconds)
                })
                .then_with(|| a.title.cmp(&b.title))
        });
        Ok(rows)
    })
}

/// Build the author-less FFI row for one followed composite site. Rung 1 lands
/// the fields + honest defaults: `state` is `"pending-first-sync"` (the post-follow
/// default until a manifest resolves) and `transport_blocked` is `false`; both
/// get their real transport-derived values on the ticket-follow path in Rung 5.
fn followed_site_row(record: &CommunityRecord) -> FollowedSiteRow {
    FollowedSiteRow {
        root: hex(&record.namespace_id),
        title: record.title.clone(),
        state: "pending-first-sync".to_string(),
        transport_blocked: false,
    }
}

/// Every composite indymedia site the user follows, as author-less rows. These
/// are the `Following` registry records — the same records `list_communities`
/// filters OUT — so a followed root surfaces here and nowhere else. Reads
/// metadata only; never unseals anything.
pub(crate) fn list_followed_sites(
    inner: &Arc<Mutex<ProfileState>>,
) -> Result<Vec<FollowedSiteRow>, MobileError> {
    with_active(inner, |profile| {
        Ok(profile
            .registry
            .communities
            .iter()
            .filter(|record| record.relationship == Relationship::Following)
            .map(followed_site_row)
            .collect())
    })
}

/// Test-only seam that persists a `Following` registry record for `root` and
/// returns its lowercase-hex namespace id. It lives in a NON-`#[uniffi::export]`
/// impl block so UniFFI never surfaces a test-only method across the FFI
/// boundary. The production entry point is Rung 5's `follow_site(ticket)` (real
/// ticket + transport parsing); Rung 1 only needs a persisted `Following` record
/// to exercise the `list_followed_sites` / `list_communities`-exclusion paths.
#[cfg(test)]
impl MobileProfile {
    pub(crate) fn follow_site_for_test(&self, root: Vec<u8>) -> Result<String, MobileError> {
        let root: [u8; 32] = root.try_into().map_err(|_| MobileError::InvalidInput)?;
        with_active(&self.inner, |profile| {
            profile.registry.upsert(CommunityRecord {
                namespace_id: root,
                title: "Followed site".to_string(),
                relationship: Relationship::Following,
                sealed_author: None,
                descriptor_entry_id: None,
                archived: false,
                quarantined: false,
                last_activity_unix_seconds: None,
                last_sync_unix_seconds: None,
            });
            persist_registry(profile)?;
            Ok(hex(&root))
        })
    }
}

pub(crate) fn active_community(
    inner: &Arc<Mutex<ProfileState>>,
) -> Result<Option<CommunityRow>, MobileError> {
    with_active(inner, |profile| {
        let Some(active) = profile.registry.active else {
            return Ok(None);
        };
        let Some(record) = profile.registry.find(&active) else {
            return Ok(None);
        };
        Ok(Some(community_row(profile, record)))
    })
}

pub(crate) fn switch_community(
    inner: &Arc<Mutex<ProfileState>>,
    namespace_id: String,
    mut wrapping_key: Vec<u8>,
) -> Result<CommunityRow, MobileError> {
    let result = with_active(inner, |profile| {
        let key = exact_wrapping_key(&wrapping_key)?;
        let target_ns = parse_entry_id(&namespace_id)?;

        // Re-selecting the active community is a no-op: no cancellation, no
        // generation bump, so in-flight work is not disturbed.
        if profile.registry.active == Some(target_ns) {
            let record = profile
                .registry
                .find(&target_ns)
                .ok_or(MobileError::CommunityUnavailable)?;
            return Ok(community_row(profile, record));
        }

        let record = profile
            .registry
            .find(&target_ns)
            .ok_or(MobileError::CommunityUnavailable)?;
        // Archived communities are not selectable (restore first). A QUARANTINED
        // community IS switchable, because this switch is exactly the recovery
        // attempt: it re-tries the unseal, clears the quarantine on success, and
        // re-quarantines only if it still fails. So a transient read failure that
        // once quarantined a community can never leave it permanently dead — a
        // Retry (another switch) recovers it.
        if record.archived {
            return Err(MobileError::CommunityUnavailable);
        }

        // Obtain the target author WITHOUT mutating any state, so a failure to
        // load it leaves the current community fully intact (fail closed — a
        // switch never silently lands on a different community). The sealed
        // at-rest author is preferred: unsealing it (the ONLY path that unseals
        // a community, and it needs the key) means the target is loaded from
        // durable bytes rather than a session cache.
        let target_author = if let Some(sealed) = record.sealed_author.clone() {
            match EvidenceAuthor::open_sealed_identity(&key, &sealed) {
                Ok(author) => {
                    // A stale parked copy is now redundant; drop it so only the
                    // active author is ever held unsealed. Recovery succeeded, so
                    // clear any quarantine — the community is openable again.
                    profile.community_authors.remove(&target_ns);
                    if let Some(record) = profile.registry.find_mut(&target_ns) {
                        record.quarantined = false;
                    }
                    author
                }
                Err(_) => {
                    // The wrapping key is a profile invariant, so a failure to
                    // open means the bytes are corrupt (or a transient read handed
                    // us a wrong key). Quarantine (retain the bytes for recovery),
                    // stay on the current community, fail closed — never drop the
                    // community, never leak a partial key. A later switch retries.
                    if let Some(record) = profile.registry.find_mut(&target_ns) {
                        record.quarantined = true;
                    }
                    persist_registry(profile)?;
                    return Err(MobileError::CommunityUnavailable);
                }
            }
        } else if let Some(author) = profile.community_authors.remove(&target_ns) {
            // Not sealed yet (a keyless join this session): take the parked author.
            // A parked author loaded fine, so clear any quarantine.
            if let Some(record) = profile.registry.find_mut(&target_ns) {
                record.quarantined = false;
            }
            author
        } else {
            // No author to load: a public-reader, or an author neither sealed nor
            // held this session. Cannot post here; the chooser offers recovery.
            return Err(MobileError::CommunityUnavailable);
        };

        // Seal the outgoing active author into its row and DROP it: with the key
        // it is fully recoverable from the registry, so at most the ACTIVE author
        // is ever unsealed in RAM. `join_public_space` now seals inline the same
        // way (Risk 13 closed), so no path leaves a parked author unsealed when a
        // key is present. A seal failure (entropy) falls back to parking so the
        // author is not lost.
        let outgoing_ns = profile.registry.active;
        let outgoing = std::mem::replace(&mut profile.author, target_author);
        if let Some(active_ns) = outgoing_ns {
            seal_or_park_outgoing(profile, active_ns, outgoing, Some(&key));
            // Park the outgoing community's board projection and sync inventory so
            // a locally-authored alert (whose payload the store does not retain)
            // survives the round trip, and the community can still sync when
            // reselected — without either leaking into the new community.
            profile
                .community_entries
                .insert(active_ns, std::mem::take(&mut profile.entries));
            profile
                .community_sync_inventory
                .insert(active_ns, std::mem::take(&mut profile.sync_inventory));
        }

        // Isolation: the community generation is advanced below, which makes every
        // preview/plan/sync coordinator captured in the OUTGOING community stale.
        // Its next use fails closed — it can neither read from nor commit into this
        // community — and `drop_stale_handles` clears it out of the way of a fresh
        // operation. This is the community-generation guard doing the fail-closed
        // work; the profile `Mutex` already serializes the operations themselves.
        // Drafts and the trust cache carry no generation stamp, so drop them now.
        // (The outgoing sync inventory was parked above.)
        profile.drafts.clear();
        // The trust cache is a projection of the active namespace's markers;
        // clearing it reconciles it to the new community (store markers are
        // namespace-scoped, so this can only tighten, never widen, authority).
        profile.app_trust_markers.clear();

        let record = profile
            .registry
            .find(&target_ns)
            .ok_or(MobileError::CommunityUnavailable)?;
        profile.space = Some(PublicSpace {
            namespace_id: hex(&target_ns),
            title: record.title.clone(),
            is_public: true,
        });
        profile.joined_others_space = !is_space_organizer(profile);
        profile.registry.active = Some(target_ns);

        // Fail closed: any preview/plan/sync/app-execution handle captured before
        // this bump can no longer commit into either community.
        bump_app_execution_generation(profile);
        profile.community_generation = profile.community_generation.wrapping_add(1);

        // Restore the target's parked projection (in-session round trip) or, for a
        // freshly loaded/joined community, reproject its imported content.
        if let Some(cached) = profile.community_entries.remove(&target_ns) {
            profile.entries = cached;
        } else {
            reproject_active(profile)?;
        }
        // Restore the target's parked sync inventory (empty for a freshly loaded
        // community until its content is re-imported this session).
        profile.sync_inventory = profile
            .community_sync_inventory
            .remove(&target_ns)
            .unwrap_or_default();
        persist_registry(profile)?;

        let record = profile
            .registry
            .find(&target_ns)
            .ok_or(MobileError::CommunityUnavailable)?;
        Ok(community_row(profile, record))
    });
    wrapping_key.zeroize();
    result
}

pub(crate) fn archive_community(
    inner: &Arc<Mutex<ProfileState>>,
    namespace_id: String,
) -> Result<(), MobileError> {
    with_active(inner, |profile| {
        let target_ns = parse_entry_id(&namespace_id)?;
        let record = profile
            .registry
            .find_mut(&target_ns)
            .ok_or(MobileError::CommunityUnavailable)?;
        record.archived = true;
        // An archived community is not the active selection; native then opens
        // the chooser. The row (and its sealed author) is retained, never dropped.
        if profile.registry.active == Some(target_ns) {
            profile.registry.active = None;
        }
        persist_registry(profile)?;
        Ok(())
    })
}

pub(crate) fn restore_community(
    inner: &Arc<Mutex<ProfileState>>,
    namespace_id: String,
) -> Result<CommunityRow, MobileError> {
    with_active(inner, |profile| {
        let target_ns = parse_entry_id(&namespace_id)?;
        let record = profile
            .registry
            .find_mut(&target_ns)
            .ok_or(MobileError::CommunityUnavailable)?;
        record.archived = false;
        persist_registry(profile)?;
        let record = profile
            .registry
            .find(&target_ns)
            .ok_or(MobileError::CommunityUnavailable)?;
        Ok(community_row(profile, record))
    })
}

pub(crate) fn persist_communities(
    inner: &Arc<Mutex<ProfileState>>,
    mut wrapping_key: Vec<u8>,
) -> Result<(), MobileError> {
    let result = with_active(inner, |profile| {
        let key = exact_wrapping_key(&wrapping_key)?;
        // Seal the active author into its row.
        if let Some(active_ns) = profile.registry.active {
            let sealed = profile.author.seal_identity(&key).ok();
            if let (Some(sealed), Some(record)) = (sealed, profile.registry.find_mut(&active_ns)) {
                record.sealed_author = Some(sealed);
            }
        }
        // Seal every parked author into its row, then drop the plaintext copy —
        // once sealed it is recoverable from the registry, so nothing but the
        // active author remains unsealed in memory.
        let parked: Vec<[u8; 32]> = profile.community_authors.keys().copied().collect();
        for ns in parked {
            let sealed = profile
                .community_authors
                .get(&ns)
                .and_then(|author| author.seal_identity(&key).ok());
            if let Some(sealed) = sealed {
                if let Some(record) = profile.registry.find_mut(&ns) {
                    record.sealed_author = Some(sealed);
                }
                profile.community_authors.remove(&ns);
            }
        }
        persist_registry(profile)?;
        Ok(())
    });
    wrapping_key.zeroize();
    result
}

pub(crate) fn community_registry_quarantined(
    inner: &Arc<Mutex<ProfileState>>,
) -> Result<bool, MobileError> {
    with_active(inner, |profile| Ok(profile.registry_quarantined))
}

pub(crate) fn set_app_trust(
    inner: &Arc<Mutex<ProfileState>>,
    app_id: String,
    trusted: bool,
) -> Result<(), MobileError> {
    use riot_core::apps::index::app_index_trust_path;
    use riot_core::apps::trust::{encode_trust_marker, TrustMarker, TrustMarkerKind};

    with_active(inner, |profile| {
        // A nearby peer must NOT block an approval. Phones auto-connect now, so
        // a sync session is open most of the time an organizer is standing next
        // to someone — and refusing here made "Let everyone in this space use
        // this" fail with an unexplained error exactly when it was tapped.
        //
        // The guard existed because writing the trust marker goes through the
        // store's shared inspect/plan slot, which an in-flight sync review is
        // also using. So drop the sync rather than the approval: the approval is
        // a deliberate human act, the sync is background chatter that reconnects
        // on its own. Anything mid-review is discarded, not half-applied.
        if sync_session_is_active(profile) {
            profile.sync_session = None;
        }
        // Only a space's organizer may approve an app for it. Without this a
        // member could self-approve any app, which would make the trust gate
        // (the one human review moment in the whole design) meaningless.
        //
        // The gate is unchanged; what changed is that it now SAYS WHY. It used to
        // return `InvalidInput` — the same code as a malformed app id — so the
        // sheet closed with nothing to show and the app never appeared. A person
        // who had done nothing wrong was locked out in silence.
        if let Some(refusal) = organizer_refusal(profile) {
            return Err(refusal);
        }
        let app_id = parse_entry_id(&app_id)?;
        if !profile
            .app_trust_markers
            .iter()
            .any(|marker| marker.app_id == app_id)
            && profile.app_trust_markers.len() >= MAX_APP_TRUST_MARKERS
        {
            return Err(MobileError::SessionLimit);
        }
        let timestamp = next_app_write_timestamp(profile)?;
        let kind = if trusted {
            TrustMarkerKind::Trust
        } else {
            TrustMarkerKind::Revoke
        };
        let marker = TrustMarker {
            app_id,
            author_subspace_id: *profile.author.subspace_id().as_bytes(),
            kind,
            timestamp_micros: timestamp,
        };
        let payload = encode_trust_marker(&marker).map_err(map_apps_error)?;
        let path = app_index_trust_path(&app_id, profile.author.subspace_id().as_bytes())
            .map_err(map_apps_error)?;
        let signed = sign_local_app_entry(profile, path, &payload, timestamp)?;
        commit_local_app_entries(profile, vec![signed])?;
        // Any trust change — grant OR revoke — advances the execution
        // generation, invalidating every `AppExecutionSession` opened before it.
        // Re-approval therefore does not silently re-authorize a session that was
        // live across the revoke: a re-approved app runs in a *new* session.
        bump_app_execution_generation(profile);
        Ok(())
    })
}

/// Advance the app-execution generation. Called on every app-trust change and
/// every namespace swap, so a stale `AppExecutionSession` (Unit 0C) fails
/// revalidation on its next read/commit.
fn bump_app_execution_generation(profile: &mut LocalProfile) {
    profile.app_execution_generation = profile.app_execution_generation.wrapping_add(1);
}

pub(crate) fn is_app_trusted(
    inner: &Arc<Mutex<ProfileState>>,
    app_id: String,
) -> Result<bool, MobileError> {
    with_active(inner, |profile| {
        let app_id = parse_entry_id(&app_id)?;
        resolve_is_trusted(profile, &app_id)
    })
}

/// Resolve, from the STORE plus the author's cache, whether `app_id` is trusted
/// by the recognized organizer of the profile's CURRENT namespace. The single
/// trust-evaluation point shared by `is_app_trusted` (the UI query) and
/// `AppExecutionSession` revalidation (the security gate), so a running app and
/// a "Turn off" button can never disagree about whether it is authorized.
///
/// Trust must be read from the STORE, not just the profile-local cache: an
/// organizer's approval reaches a member as a synced trust-marker entry. Reading
/// only the local cache (which `set_app_trust` fills for the author) meant an
/// organizer's decision could never reach anyone else — that is the "one
/// decision covers everyone, no install step" property.
///
/// `is_trusted` fails closed if given two markers for the same coordinate, so the
/// store's Willow-resolved marker and the author's own cached copy are collapsed
/// to one per (app, organizer) before asking. Newest wins, matching Willow's own
/// per-path resolution.
fn resolve_is_trusted(profile: &LocalProfile, app_id: &[u8; 32]) -> Result<bool, MobileError> {
    let organizer = space_organizer_subspace_id(profile);
    let own_namespace_id = profile.author.identity().namespace_id;

    let scanned = riot_core::apps::index::scan_app_index(&profile.store).map_err(map_apps_error)?;
    let mut markers: Vec<riot_core::apps::trust::TrustMarker> = Vec::new();
    let mut push_resolved =
        |marker: riot_core::apps::trust::TrustMarker| match markers.iter_mut().find(|existing| {
            existing.app_id == marker.app_id
                && existing.author_subspace_id == marker.author_subspace_id
        }) {
            Some(existing) if marker.timestamp_micros > existing.timestamp_micros => {
                *existing = marker;
            }
            Some(_) => {}
            None => markers.push(marker),
        };
    for space in scanned.spaces {
        if space.space_namespace_id == own_namespace_id {
            space.markers.into_iter().for_each(&mut push_resolved);
        }
    }
    profile
        .app_trust_markers
        .iter()
        .copied()
        .for_each(&mut push_resolved);

    Ok(riot_core::apps::trust::is_trusted(
        app_id,
        &markers,
        &[organizer],
    ))
}

/// The immutable snapshot an `AppExecutionSession` (Unit 0C) captures at open
/// and presents on every read/commit. The three fields are exactly the state a
/// stale session must be caught disagreeing with: the app it was opened for, the
/// approval generation live at open, and the namespace it was bound to.
#[derive(Debug, Clone, Copy)]
pub(crate) struct AppExecutionSnapshot {
    pub(crate) app_id: [u8; 32],
    pub(crate) generation: u64,
    pub(crate) namespace_id: [u8; 32],
}

/// Open an execution session for a currently-trusted app. This is the launch
/// gate, now enforced in Rust rather than trusted to the native host: an
/// untrusted app cannot obtain a session at all.
pub(crate) fn app_execution_open(
    inner: &Arc<Mutex<ProfileState>>,
    app_id: String,
) -> Result<AppExecutionSnapshot, MobileError> {
    with_active(inner, |profile| {
        let app_id = parse_entry_id(&app_id)?;
        if !resolve_is_trusted(profile, &app_id)? {
            return Err(MobileError::AppRejected);
        }
        Ok(AppExecutionSnapshot {
            app_id,
            generation: profile.app_execution_generation,
            namespace_id: profile.author.identity().namespace_id,
        })
    })
}

/// The security gate run before EVERY execution-session read and commit, under
/// the same lock that then performs the op — no window between check and use.
/// Each clause is an independent invalidation vector; all three are checked
/// (defence in depth), and each on its own denies closed.
fn revalidate_execution(
    profile: &LocalProfile,
    snap: &AppExecutionSnapshot,
) -> Result<(), MobileError> {
    // (2) Namespace replacement: a join/demo swap strands the session in a
    // namespace it can no longer write to. The captured namespace must still be
    // the live one.
    if profile.author.identity().namespace_id != snap.namespace_id {
        return Err(MobileError::AppRejected);
    }
    // (4) Stale approval-generation: trust changes AND namespace swaps advance
    // the generation, so a re-approval — which returns trust to TRUE — still
    // fails a session opened before it. This is the clause a trust-only guard
    // would miss, and the reason (1)/(4) are distinct.
    if profile.app_execution_generation != snap.generation {
        return Err(MobileError::AppRejected);
    }
    // (1) Revoke: the app must STILL be trusted right now, verified against the
    // store, never assumed. Redundant with the generation check by construction,
    // and deliberately kept — the gate VERIFIES authority, it does not assume it.
    if !resolve_is_trusted(profile, &snap.app_id)? {
        return Err(MobileError::AppRejected);
    }
    Ok(())
}

/// Whether the session is still valid RIGHT NOW — the same revalidation the
/// read/commit path runs, exposed as a plain bool so the native bridge can tell
/// an INVALIDATION (revoked / namespace-swapped / stale generation) apart from an
/// ordinary per-op rejection (a malformed key). Both surface as `AppRejected`
/// from a data call; only an invalidation must close the app to a named
/// destination (§4.7). A dead profile or any revalidation failure reads false.
pub(crate) fn app_execution_is_valid(
    inner: &Arc<Mutex<ProfileState>>,
    snap: &AppExecutionSnapshot,
) -> bool {
    with_active(inner, |profile| {
        Ok(revalidate_execution(profile, snap).is_ok())
    })
    .unwrap_or(false)
}

/// Execution-session read: revalidate, then read, under one lock.
pub(crate) fn app_execution_get(
    inner: &Arc<Mutex<ProfileState>>,
    snap: &AppExecutionSnapshot,
    key: String,
) -> Result<Option<Vec<u8>>, MobileError> {
    with_active(inner, |profile| {
        revalidate_execution(profile, snap)?;
        riot_core::apps::bridge::AppDataBridge::get(&profile.store, &snap.app_id, &key)
            .map_err(map_apps_error)
    })
}

/// Execution-session list: revalidate, then list, under one lock.
pub(crate) fn app_execution_list(
    inner: &Arc<Mutex<ProfileState>>,
    snap: &AppExecutionSnapshot,
    prefix: String,
) -> Result<Vec<crate::apps_ffi::AppDataItem>, MobileError> {
    with_active(inner, |profile| {
        revalidate_execution(profile, snap)?;
        let items =
            riot_core::apps::bridge::AppDataBridge::list(&profile.store, &snap.app_id, &prefix)
                .map_err(map_apps_error)?;
        Ok(items
            .into_iter()
            .map(|(key, value)| crate::apps_ffi::AppDataItem { key, value })
            .collect())
    })
}

/// Execution-session commit: revalidate, then commit, under one lock. Returns
/// the canonical signed bundle bytes so a host can persist app data for replay.
/// A committed write does NOT advance the generation — an app writing its own
/// data must not invalidate its own session; only trust and namespace changes do.
pub(crate) fn app_execution_put_with_receipt(
    inner: &Arc<Mutex<ProfileState>>,
    snap: &AppExecutionSnapshot,
    key: String,
    value: Vec<u8>,
) -> Result<Vec<u8>, MobileError> {
    with_active(inner, |profile| {
        revalidate_execution(profile, snap)?;
        // Same preview-slot discipline as `app_data_put_with_receipt`: a
        // store.inspect during an in-flight sync review would clobber it.
        if sync_session_is_active(profile) {
            return Err(MobileError::InvalidInput);
        }
        let timestamp = next_app_write_timestamp(profile)?;
        let path =
            riot_core::apps::entry::app_data_path(&snap.app_id, &key).map_err(map_apps_error)?;
        let signed = sign_local_app_entry(profile, path, &value, timestamp)?;
        let bundle_bytes = commit_local_app_entries(profile, vec![signed])?;
        Ok(bundle_bytes)
    })
}

pub(crate) fn app_data_put(
    inner: &Arc<Mutex<ProfileState>>,
    app_id: String,
    key: String,
    value: Vec<u8>,
) -> Result<(), MobileError> {
    // Native callers that don't need the persistence receipt (Android's
    // RiotJsBridge, iOS' AppRuntimeDataBridge) keep the void signature; the
    // write itself is identical.
    app_data_put_with_receipt(inner, app_id, key, value).map(|_| ())
}

/// `app_data_put` that also returns the canonical signed bundle bytes it
/// committed. The native host persists these across relaunch and replays them
/// into a fresh profile via `replay_app_data_bundle`.
pub(crate) fn app_data_put_with_receipt(
    inner: &Arc<Mutex<ProfileState>>,
    app_id: String,
    key: String,
    value: Vec<u8>,
) -> Result<Vec<u8>, MobileError> {
    with_active(inner, |profile| {
        // Same guard as sign_draft/inspect_bytes: store.inspect replaces the
        // session-wide preview slot, which would clobber an in-flight sync
        // review.
        if sync_session_is_active(profile) {
            return Err(MobileError::InvalidInput);
        }
        let app_id = parse_entry_id(&app_id)?;
        let timestamp = next_app_write_timestamp(profile)?;
        let path = riot_core::apps::entry::app_data_path(&app_id, &key).map_err(map_apps_error)?;
        let signed = sign_local_app_entry(profile, path, &value, timestamp)?;
        let bundle_bytes = commit_local_app_entries(profile, vec![signed])?;
        Ok(bundle_bytes)
    })
}

/// Admits a previously-committed app-data bundle (as returned by
/// `app_data_put_with_receipt`) into this profile's store, so a host that
/// persists app data by saving the signed bytes can rebuild the store on the
/// next open. Strictly app-data-only: the bundle must decode to app-data-path
/// entries and nothing else, so this can never be used to smuggle alert (or
/// any other) entries past the alert review surface. Runs the same
/// inspect/plan/commit admission every synced entry passes through.
pub(crate) fn replay_app_data_bundle(
    inner: &Arc<Mutex<ProfileState>>,
    bytes: Vec<u8>,
) -> Result<(), MobileError> {
    with_active(inner, |profile| {
        // Same preview-slot discipline as app_data_put.
        if sync_session_is_active(profile) {
            return Err(MobileError::InvalidInput);
        }
        let decoded = match decode_bundle(&bytes) {
            BundleDecodeOutcome::Decoded(decoded) => decoded,
            BundleDecodeOutcome::Rejected(_) => return Err(MobileError::ImportRejected),
        };
        let mut saw_entry = false;
        let mut max_replayed_timestamp = 0u64;
        let mut signed_entries = Vec::new();
        for item in &decoded.items {
            let ItemStatus::Valid(_) = &item.status else {
                continue;
            };
            let entry = riot_core::willow::decode_entry_canonic(item.frame.entry_bytes())
                .map_err(|_| MobileError::ImportRejected)?;
            if !riot_core::apps::entry::is_app_data_entry(&entry) {
                return Err(MobileError::ImportRejected);
            }
            let timestamp = riot_core::willow::entry_timestamp_micros(item.frame.entry_bytes())
                .map_err(|_| MobileError::ImportRejected)?;
            max_replayed_timestamp = max_replayed_timestamp.max(timestamp);
            saw_entry = true;
            signed_entries.push(SignedWillowEntry {
                entry_bytes: item.frame.entry_bytes().to_vec(),
                capability_bytes: item.frame.capability_bytes().to_vec(),
                signature: item
                    .frame
                    .signature_bytes()
                    .try_into()
                    .map_err(|_| MobileError::ImportRejected)?,
                payload_bytes: item.frame.payload_bytes().to_vec(),
            });
        }
        if !saw_entry {
            return Err(MobileError::ImportRejected);
        }
        let next_inventory = prospective_sync_inventory(profile, &signed_entries)?;
        profile.preview = None;
        profile.plan = None;
        let preview = inspect_core(&profile.store, &bytes, "app-data-replay")?;
        let plan = preview.plan_all().map_err(map_core_error)?;
        match plan.commit().map_err(map_core_error)? {
            CommitOutcome::Committed(_) | CommitOutcome::NoChanges(_) => {
                // Advance the write floor past every replayed entry, exactly
                // as a live write would (`next_app_write_timestamp`). Without
                // this, a same-key overwrite issued in the same wall-clock
                // second as the original burst gets `now*1e6`, which can be
                // below a replayed `now*1e6 + k` timestamp — cmp_recency would
                // keep the stale replayed value and silently drop the new
                // write. This is the exact invariant the replay path exists
                // to preserve.
                profile.app_data_timestamp_floor_micros = profile
                    .app_data_timestamp_floor_micros
                    .max(max_replayed_timestamp);
                install_sync_inventory(profile, next_inventory)?;
                Ok(())
            }
        }
    })
}

/// The label an app shows for the current person, RENDERED: `"Ana · a3f91122"`,
/// or `"member · a3f91122"` before they have claimed a name.
///
/// This is what `riot.whoami()` reads. It used to be `"member-<hex>"` — a label
/// with nowhere for a real name to go. Identical to `my_display_name`; the two
/// names exist because the app runtime and the profile surface are separate
/// FFI objects, not because the answer differs.
pub(crate) fn app_display_name(inner: &Arc<Mutex<ProfileState>>) -> Result<String, MobileError> {
    my_display_name(inner)
}

// ─── Profiles ────────────────────────────────────────────────────────────────
//
// A profile card is an ordinary signed entry, so it is written through the SAME
// local-write pipeline as an app write (`sign_local_app_entry` +
// `commit_local_app_entries`) — NOT through `riot_core::profile::resolver::
// write_profile_card`.
//
// That is not a style preference. `write_profile_card` takes only an
// `&EvidenceStore`, so an entry it commits lands in the store while staying
// invisible to this profile's `sync_inventory`. Two things break at once:
// `ensure_complete_sync_inventory` requires the inventory to equal the store's
// live ids exactly, so every later `open_sync_session` would fail with
// `Internal` — permanently — and the name would never reach a peer anyway,
// because the inventory IS what sync offers. `commit_local_app_entries` keeps
// the inventory whole, and still commits through inspect → plan → commit, so
// there is no privileged write path here either. The core function remains the
// right API for core-level callers, which carry no such bookkeeping.

fn own_subspace_id(profile: &LocalProfile) -> [u8; SUBSPACE_ID_BYTES] {
    *profile.author.subspace_id().as_bytes()
}

/// A raw 32-byte subspace id as the profile FFI surface carries it.
fn exact_subspace_id(value: &[u8]) -> Result<[u8; SUBSPACE_ID_BYTES], MobileError> {
    value.try_into().map_err(|_| MobileError::InvalidInput)
}

/// The one place a resolved (or missing) name becomes a `WhoAmI`. An id with no
/// card resolves to the `member` fallback rather than an error — see
/// `ProfileSession::profile_for`.
///
/// `display_name` is SANITIZED here, by the same `sanitize_display_name` the
/// rendered form uses, and that is not belt-and-braces. `WhoAmI` hands the name
/// and the tag over as separate fields precisely so a renderer can reassemble
/// them — and reassembling them is `name + " · " + tag`. If the raw claim went
/// out, a name of `"Ana · a3f91122"` would come back together as
/// `"Ana · a3f91122 · deadbeef"`, which begins with exactly what honest Ana
/// renders to. The structure protects nothing once the renderer flattens it, so
/// the field crossing the boundary must already be safe to flatten.
fn who_am_i(
    names: &std::collections::BTreeMap<[u8; SUBSPACE_ID_BYTES], String>,
    subspace_id: [u8; SUBSPACE_ID_BYTES],
) -> crate::profile_ffi::WhoAmI {
    crate::profile_ffi::WhoAmI {
        id: subspace_id.to_vec(),
        display_name: sanitize_display_name(names.get(&subspace_id).map(String::as_str)),
        tag: key_tag(&subspace_id),
    }
}

pub(crate) fn set_display_name(
    inner: &Arc<Mutex<ProfileState>>,
    name: String,
) -> Result<(), MobileError> {
    with_active(inner, |profile| {
        // Same guard as app_data_put/endorse_app: the commit runs through
        // store.inspect, which replaces the session-wide preview slot and would
        // clobber an in-flight sync review.
        if sync_session_is_active(profile) {
            return Err(MobileError::InvalidInput);
        }
        let card = ProfileCard { display_name: name };
        // The codec is the SINGLE enforcement point for the name's bounds —
        // empty and oversized both come back as FieldInvalid from here. Nothing
        // is pre-validated above it, so there is exactly one rule to change.
        let payload = encode_profile_card(&card).map_err(map_profile_error)?;
        let path = profile_card_path(&own_subspace_id(profile)).map_err(map_profile_error)?;
        let timestamp = next_app_write_timestamp(profile)?;
        let signed = sign_local_app_entry(profile, path, &payload, timestamp)?;
        commit_local_app_entries(profile, vec![signed])?;
        Ok(())
    })
}

pub(crate) fn my_display_name(inner: &Arc<Mutex<ProfileState>>) -> Result<String, MobileError> {
    with_active(inner, |profile| {
        let subspace_id = own_subspace_id(profile);
        let names = resolve_display_names(&profile.store).map_err(map_profile_error)?;
        // Rendered, never bare — the raw name never leaves this function.
        Ok(render_display_name(
            names.get(&subspace_id).map(String::as_str),
            &subspace_id,
        ))
    })
}

pub(crate) fn whoami(
    inner: &Arc<Mutex<ProfileState>>,
) -> Result<crate::profile_ffi::WhoAmI, MobileError> {
    with_active(inner, |profile| {
        let subspace_id = own_subspace_id(profile);
        let names = resolve_display_names(&profile.store).map_err(map_profile_error)?;
        Ok(who_am_i(&names, subspace_id))
    })
}

pub(crate) fn profile_for(
    inner: &Arc<Mutex<ProfileState>>,
    id: Vec<u8>,
) -> Result<crate::profile_ffi::WhoAmI, MobileError> {
    // A wrong-length id is a caller bug and IS an error. An unknown but
    // well-formed id is not — it is simply someone whose card has not synced
    // here yet, and an app must still be able to draw their row.
    let subspace_id = exact_subspace_id(&id)?;
    with_active(inner, |profile| {
        let names = resolve_display_names(&profile.store).map_err(map_profile_error)?;
        Ok(who_am_i(&names, subspace_id))
    })
}

pub(crate) fn display_names(
    inner: &Arc<Mutex<ProfileState>>,
) -> Result<Vec<crate::profile_ffi::DisplayNameRecord>, MobileError> {
    with_active(inner, |profile| {
        let names = resolve_display_names(&profile.store).map_err(map_profile_error)?;
        Ok(names
            .into_iter()
            .map(
                |(subspace_id, name)| crate::profile_ffi::DisplayNameRecord {
                    subspace_id: subspace_id.to_vec(),
                    // Every name crossing the boundary is rendered. `resolve_display_names`
                    // hands back the raw claim; this is where it stops being raw.
                    rendered: render_display_name(Some(&name), &subspace_id),
                },
            )
            .collect())
    })
}

fn map_profile_error(error: ProfileError) -> MobileError {
    match error {
        // An empty or oversized display name — the codec's bounds check, which
        // is the only place the name's length is enforced.
        ProfileError::FieldInvalid => MobileError::InvalidInput,
        ProfileError::StoreRejected => MobileError::StoreFull,
        ProfileError::PathInvalid | ProfileError::Willow(_) => MobileError::Internal,
    }
}

pub(crate) fn app_data_get(
    inner: &Arc<Mutex<ProfileState>>,
    app_id: String,
    key: String,
) -> Result<Option<Vec<u8>>, MobileError> {
    with_active(inner, |profile| {
        let app_id = parse_entry_id(&app_id)?;
        riot_core::apps::bridge::AppDataBridge::get(&profile.store, &app_id, &key)
            .map_err(map_apps_error)
    })
}

pub(crate) fn app_data_list(
    inner: &Arc<Mutex<ProfileState>>,
    app_id: String,
    prefix: String,
) -> Result<Vec<crate::apps_ffi::AppDataItem>, MobileError> {
    with_active(inner, |profile| {
        let app_id = parse_entry_id(&app_id)?;
        let items = riot_core::apps::bridge::AppDataBridge::list(&profile.store, &app_id, &prefix)
            .map_err(map_apps_error)?;
        Ok(items
            .into_iter()
            .map(|(key, value)| crate::apps_ffi::AppDataItem { key, value })
            .collect())
    })
}

/// Willow timestamp for the next same-profile app write (app data or
/// app-index): wall-clock micros, floored to stay strictly increasing so a
/// rapid overwrite of the same coordinate still prunes deterministically.
/// Callers store the returned value back into
/// `app_data_timestamp_floor_micros` only after the write succeeds.
fn next_app_write_timestamp(profile: &LocalProfile) -> Result<u64, MobileError> {
    let now_micros = system_snapshot()
        .map_err(map_author_error)?
        .unix_seconds
        .saturating_mul(1_000_000);
    Ok(now_micros.max(
        profile
            .app_data_timestamp_floor_micros
            .checked_add(1)
            .ok_or(MobileError::SessionLimit)?,
    ))
}

/// Raw 32-byte app id as the directory FFI surface carries it.
fn exact_app_id(value: &[u8]) -> Result<[u8; 32], MobileError> {
    value.try_into().map_err(|_| MobileError::InvalidInput)
}

/// Build and authorise one exact app entry without mutating profile state.
fn sign_local_app_entry(
    profile: &LocalProfile,
    path: riot_core::willow::Path,
    payload: &[u8],
    timestamp_micros: u64,
) -> Result<SignedWillowEntry, MobileError> {
    let entry = riot_core::willow::Entry::builder()
        .namespace_id(profile.author.namespace_id().clone())
        .subspace_id(profile.author.subspace_id())
        .path(path)
        .timestamp(timestamp_micros)
        .payload(payload)
        .build();
    let authorised =
        riot_core::willow::authorise_entry(&profile.author, entry).map_err(map_author_error)?;
    let token = authorised.authorisation_token();
    let signature: Signature = token.signature().clone().into();
    Ok(SignedWillowEntry {
        entry_bytes: riot_core::willow::encode_entry(authorised.entry()),
        capability_bytes: riot_core::willow::encode_capability(token.capability()),
        signature: signature.to_bytes(),
        payload_bytes: payload.to_vec(),
    })
}

/// Preflight and commit a complete local app batch through one RIOTE1
/// inspect/plan/commit transaction. Inventory capacity is proven for the
/// whole batch before store mutation, so paired app-index publication cannot
/// leave a manifest without its bundle.
fn commit_local_app_entries(
    profile: &mut LocalProfile,
    signed_entries: Vec<SignedWillowEntry>,
) -> Result<Vec<u8>, MobileError> {
    if signed_entries.is_empty() {
        return Err(MobileError::InvalidInput);
    }
    if profile.preview.is_some() || profile.plan.is_some() {
        return Err(MobileError::InvalidInput);
    }
    let next_inventory = prospective_sync_inventory(profile, &signed_entries)?;
    let bundle_bytes = encode_bundle(&signed_entries).map_err(|_| MobileError::SessionLimit)?;
    let preview = inspect_core(&profile.store, &bundle_bytes, "local-app-write")?;
    if preview.eligible_count().map_err(map_core_error)? != signed_entries.len() {
        return Err(MobileError::AppRejected);
    }
    let plan = preview.plan_all().map_err(map_core_error)?;
    match plan.commit().map_err(map_core_error)? {
        CommitOutcome::Committed(_) | CommitOutcome::NoChanges(_) => {}
    }
    install_sync_inventory(profile, next_inventory)?;
    advance_app_write_floor(profile, &signed_entries)?;
    refresh_app_trust_markers(profile)?;
    Ok(bundle_bytes)
}

fn refresh_app_trust_markers(profile: &mut LocalProfile) -> Result<(), MobileError> {
    let own_namespace_id = profile.author.identity().namespace_id;
    profile.app_trust_markers = riot_core::apps::index::scan_app_index(&profile.store)
        .map_err(map_apps_error)?
        .spaces
        .into_iter()
        .find(|space| space.space_namespace_id == own_namespace_id)
        .map(|space| space.markers)
        .unwrap_or_default();
    if profile.app_trust_markers.len() > MAX_APP_TRUST_MARKERS {
        return Err(MobileError::SessionLimit);
    }
    Ok(())
}

pub(crate) fn directory_listings(
    inner: &Arc<Mutex<ProfileState>>,
) -> Result<Vec<crate::apps_ffi::DirectoryListing>, MobileError> {
    use riot_core::apps::directory::{
        assemble_directory, AppProvenance, DirectoryInputs, SpaceTrust,
    };
    use riot_core::apps::index::scan_app_index;
    use riot_core::apps::starter::{verify_starter_catalog, STARTER_CATALOG};

    with_active(inner, |profile| {
        let scanned = scan_app_index(&profile.store).map_err(map_apps_error)?;
        let mut apps = verify_starter_catalog(STARTER_CATALOG);
        apps.extend(scanned.apps);

        let own_namespace_id = profile.author.identity().namespace_id;
        // Organizer recognition is local policy: the profile's own subspace
        // is the sole recognized organizer, the same source `is_app_trusted`
        // evaluates against. For the profile's own namespace the compacted
        // profile-local marker cache is authoritative (`set_app_trust` keeps
        // exactly one marker per app, satisfying `is_trusted`'s
        // one-marker-per-coordinate input contract); scanned trust entries
        // only speak for other namespaces.
        let mut spaces: Vec<SpaceTrust> = scanned
            .spaces
            .into_iter()
            .filter(|space| space.space_namespace_id != own_namespace_id)
            .map(|mut space| {
                space.organizer_subspace_ids = vec![space.space_namespace_id];
                space
            })
            .collect();
        if !profile.app_trust_markers.is_empty() {
            spaces.push(SpaceTrust {
                space_namespace_id: own_namespace_id,
                markers: profile.app_trust_markers.clone(),
                organizer_subspace_ids: vec![own_namespace_id],
            });
        }

        let listings = assemble_directory(&DirectoryInputs {
            apps,
            endorsements: scanned.endorsements,
            spaces,
            met_subspace_ids: live_entry_subspaces(profile)?,
        });
        listings
            .into_iter()
            .map(|listing| {
                let (built_in, carrier_subspace_id) = match listing.provenance {
                    AppProvenance::BuiltIn => (true, None),
                    AppProvenance::Carried {
                        carrier_subspace_id,
                    } => (false, Some(carrier_subspace_id.to_vec())),
                };
                let installed = profile
                    .installed_apps
                    .iter()
                    .any(|app| app.app_id == listing.app_id);
                Ok(crate::apps_ffi::DirectoryListing {
                    app_id: listing.app_id.to_vec(),
                    name: listing.name,
                    description: listing.description,
                    version: listing.version,
                    author_signing_key_id: listing.author.signing_key_id.to_vec(),
                    permissions: listing.permissions,
                    bundle_present: listing.bundle_present,
                    built_in,
                    installed,
                    carrier_subspace_id,
                    trusted_in_spaces: listing
                        .trusted_in_spaces
                        .iter()
                        .map(|id| id.to_vec())
                        .collect(),
                    endorsing_met_subspaces: listing
                        .endorsements
                        .met_subspace_ids
                        .iter()
                        .map(|id| id.to_vec())
                        .collect(),
                    endorsing_unmet_count: u32::try_from(listing.endorsements.unmet_count)
                        .map_err(|_| MobileError::Internal)?,
                    superseded_by: listing.superseded_by.map(|id| id.to_vec()),
                })
            })
            .collect()
    })
}

/// Documented v1 choice for "met" endorsers: the subspaces present among the
/// store's live entries — every author this profile has actually held bytes
/// from (its own included).
fn live_entry_subspaces(profile: &LocalProfile) -> Result<Vec<[u8; 32]>, MobileError> {
    let all_prefix =
        riot_core::willow::Path::from_slices(&[]).map_err(|_| MobileError::Internal)?;
    let mut subspaces = std::collections::BTreeSet::new();
    for (_, entry, _) in profile
        .store
        .entries_with_prefix(&all_prefix)
        .map_err(map_core_error)?
    {
        let identity = public_entry_identity(&riot_core::willow::encode_entry(&entry))
            .map_err(|_| MobileError::Internal)?;
        subspaces.insert(identity.signer_id);
    }
    Ok(subspaces.into_iter().collect())
}

pub(crate) fn share_app(
    inner: &Arc<Mutex<ProfileState>>,
    app_id: Vec<u8>,
    space: PublicSpace,
) -> Result<(), MobileError> {
    use riot_core::apps::index::{app_index_bundle_path, app_index_manifest_path, verify_app_pair};

    with_active(inner, |profile| {
        // Same guard as app_data_put: a local app-index write must not race
        // an in-flight sync review.
        if sync_session_is_active(profile) {
            return Err(MobileError::InvalidInput);
        }
        let app_id = exact_app_id(&app_id)?;
        // A profile writes with one author bound to one namespace, so the
        // only space it can carry an app into is the one it has joined or
        // created — the same resolution join_public_space established.
        let current = profile.space.as_ref().ok_or(MobileError::InvalidInput)?;
        if !space.is_public || space.namespace_id != current.namespace_id {
            return Err(MobileError::InvalidInput);
        }
        let riot_core::apps::index::AppPairBytes {
            manifest_bytes,
            bundle_bytes,
        } = resolve_app_payload_bytes(profile, &app_id)?;
        if verify_app_pair(&manifest_bytes, &bundle_bytes).map_err(map_apps_error)? != app_id {
            return Err(MobileError::AppRejected);
        }
        let timestamp = next_app_write_timestamp(profile)?;
        let manifest_entry = sign_local_app_entry(
            profile,
            app_index_manifest_path(&app_id).map_err(map_apps_error)?,
            &manifest_bytes,
            timestamp,
        )?;
        let bundle_entry = sign_local_app_entry(
            profile,
            app_index_bundle_path(&app_id).map_err(map_apps_error)?,
            &bundle_bytes,
            timestamp,
        )?;
        commit_local_app_entries(profile, vec![manifest_entry, bundle_entry])?;
        Ok(())
    })
}

/// The canonical manifest/bundle bytes for an app id, from whichever local
/// source holds them: an install on this profile, the built-in starter
/// catalog, or the live app-index in the store. The content-derived id binds
/// the exact bytes, so every verified source yields the identical pair.
///
/// This is the ONE resolver for every path that needs an app's bytes — share
/// it, install it out of the directory, serve its pages. Those paths forked
/// once, and the fork was user-visible: install and page-serving read only the
/// store, but a built-in's bytes live in the binary and are never written to
/// the store or synced, so the directory (which merges the starter catalog into
/// its listings) offered built-ins that could never be opened. Resolution has
/// to consider every source the directory lists from.
///
/// `AppRejected` when no local source holds a pair that re-derives `app_id` —
/// the honest "this app is not all here" outcome the UI reports.
fn resolve_app_payload_bytes(
    profile: &LocalProfile,
    app_id: &[u8; 32],
) -> Result<riot_core::apps::index::AppPairBytes, MobileError> {
    use riot_core::apps::index::{app_pair_bytes as indexed_pair_bytes, starter_pair_bytes};
    use riot_core::apps::starter::STARTER_CATALOG;

    // Verified at install — install_pair derived app_id from these exact
    // bytes; if installed apps ever persist/reload, the reload path must
    // re-verify.
    if let Some(installed) = profile
        .installed_apps
        .iter()
        .find(|app| app.app_id == *app_id)
    {
        return Ok(riot_core::apps::index::AppPairBytes {
            manifest_bytes: installed.manifest_bytes.clone(),
            bundle_bytes: installed.bundle_bytes.clone(),
        });
    }
    // Built into the binary: no store entries exist for these, ever.
    if let Some(pair) = starter_pair_bytes(STARTER_CATALOG, app_id) {
        return Ok(pair);
    }
    // Carried: the store holds the only copy. Both halves come from one
    // verified read of a single carrier's entries.
    indexed_pair_bytes(&profile.store, app_id)
        .map_err(map_apps_error)?
        .ok_or(MobileError::AppRejected)
}

pub(crate) fn endorse_app(
    inner: &Arc<Mutex<ProfileState>>,
    app_id: Vec<u8>,
    note: String,
    retract: bool,
) -> Result<(), MobileError> {
    use riot_core::apps::endorse::{encode_endorsement, EndorsementMarker};
    use riot_core::apps::index::app_index_endorsement_path;

    with_active(inner, |profile| {
        // Same guard as app_data_put/share_app.
        if sync_session_is_active(profile) {
            return Err(MobileError::InvalidInput);
        }
        let app_id = exact_app_id(&app_id)?;
        let marker = EndorsementMarker {
            app_id,
            note,
            retracted: retract,
        };
        let timestamp = next_app_write_timestamp(profile)?;
        let payload = encode_endorsement(&marker).map_err(map_apps_error)?;
        let path = app_index_endorsement_path(&app_id, profile.author.subspace_id().as_bytes())
            .map_err(map_apps_error)?;
        let signed = sign_local_app_entry(profile, path, &payload, timestamp)?;
        commit_local_app_entries(profile, vec![signed])?;
        Ok(())
    })
}

fn map_apps_error(error: riot_core::apps::AppsError) -> MobileError {
    use riot_core::apps::AppsError;
    match error {
        AppsError::StoreRejected => MobileError::StoreFull,
        _ => MobileError::AppRejected,
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

    /// Risk 13: a keyed join must leave NO unsealed author parked in RAM. The
    /// outgoing author is sealed inline into its registry row; only the active
    /// (joined) author stays unsealed.
    #[test]
    fn a_keyed_join_seals_the_outgoing_author_inline_and_parks_nothing_unsealed() {
        const KEY: [u8; 32] = [0x5a; 32];

        // Organizer of A.
        let profile = open_local_profile().unwrap();
        let a = create_public_space(&profile.inner, "Community A".into()).unwrap();
        let a_ns = parse_entry_id(&a.namespace_id).unwrap();

        // A second namespace, minted by a throwaway profile, joined WITH a key.
        let other = open_local_profile().unwrap();
        let b = create_public_space(&other.inner, "Community B".into()).unwrap();
        join_public_space(
            &profile.inner,
            crate::mobile_api::PublicSpace {
                namespace_id: b.namespace_id.clone(),
                title: "Community B".into(),
                is_public: true,
            },
            KEY.to_vec(),
        )
        .unwrap();

        let state = lock_unpoisoned(&profile.inner);
        let ProfileState::Active(local) = &*state else {
            panic!("profile active");
        };
        assert!(
            local.community_authors.is_empty(),
            "no author may sit unsealed in RAM after a keyed join (Risk 13)",
        );
        let a_record = local
            .registry
            .find(&a_ns)
            .expect("A is still a registered community");
        assert!(
            a_record.sealed_author.is_some(),
            "A's outgoing author was sealed inline into its row, not parked",
        );
    }

    /// The keyless join path (ephemeral profiles with no wrapping key) still
    /// parks the outgoing author unsealed — it has no key to seal with, and the
    /// author must not be lost.
    #[test]
    fn a_keyless_join_still_parks_the_outgoing_author() {
        let profile = open_local_profile().unwrap();
        create_public_space(&profile.inner, "Community A".into()).unwrap();
        let other = open_local_profile().unwrap();
        let b = create_public_space(&other.inner, "Community B".into()).unwrap();
        join_public_space(
            &profile.inner,
            crate::mobile_api::PublicSpace {
                namespace_id: b.namespace_id.clone(),
                title: "Community B".into(),
                is_public: true,
            },
            Vec::new(),
        )
        .unwrap();

        let state = lock_unpoisoned(&profile.inner);
        let ProfileState::Active(local) = &*state else {
            panic!("profile active");
        };
        assert_eq!(
            local.community_authors.len(),
            1,
            "keyless join parks the outgoing author (no key to seal it with)",
        );
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

    #[test]
    fn replay_advances_the_write_floor_so_a_same_second_overwrite_is_not_dropped() {
        // Regression: replay must advance app_data_timestamp_floor_micros past
        // every replayed entry. Otherwise a same-key overwrite issued in the
        // same wall-clock second as the original write burst gets a lower
        // timestamp than the replayed value, and recency resolution silently
        // keeps the stale replayed value. Seeding the floor (rather than racing
        // the clock) makes the collision deterministic.
        let app_id = "ab".repeat(32);

        // Original profile: seed the floor far above any real `now * 1e6`
        // (~year 2128), so the receipted write's timestamp is deterministically
        // high regardless of the test clock — emulating a sub-second burst that
        // bumped the floor above wall time.
        let author = open_local_profile().unwrap();
        let space = create_public_space(&author.inner, "Persist".into()).unwrap();
        let seeded_floor = 5_000_000_000_000_000u64;
        {
            let mut state = lock_unpoisoned(&author.inner);
            let ProfileState::Active(local) = &mut *state else {
                panic!("profile active");
            };
            local.app_data_timestamp_floor_micros = seeded_floor;
        }
        let receipt = app_data_put_with_receipt(
            &author.inner,
            app_id.clone(),
            "items/a".into(),
            b"old".to_vec(),
        )
        .unwrap();

        // Fresh profile joins the same space and replays the receipt. Keyless
        // (ephemeral in-memory profile): no outgoing author to seal.
        let fresh = open_local_profile().unwrap();
        join_public_space(&fresh.inner, space, Vec::new()).unwrap();
        replay_app_data_bundle(&fresh.inner, receipt).unwrap();
        assert_eq!(
            app_data_get(&fresh.inner, app_id.clone(), "items/a".into()).unwrap(),
            Some(b"old".to_vec())
        );

        // The replay must have carried the floor to the replayed timestamp
        // (seeded_floor + 1), not left it at zero.
        {
            let state = lock_unpoisoned(&fresh.inner);
            let ProfileState::Active(local) = &*state else {
                panic!("profile active");
            };
            assert!(local.app_data_timestamp_floor_micros > seeded_floor);
        }

        // An immediate same-key overwrite is therefore newer and wins. Without
        // the floor advance the fresh floor would be 0, this write would get
        // `now * 1e6` (far below seeded_floor + 1), and the stale replayed value
        // would win.
        app_data_put(
            &fresh.inner,
            app_id.clone(),
            "items/a".into(),
            b"new".to_vec(),
        )
        .unwrap();
        assert_eq!(
            app_data_get(&fresh.inner, app_id, "items/a".into()).unwrap(),
            Some(b"new".to_vec())
        );
    }

    #[test]
    fn entry_timestamp_micros_rejects_non_canonical_bytes() {
        // The floor advance relies on a *canonical* decode, not a lenient
        // parse: junk bytes must error rather than silently yield a timestamp.
        assert!(riot_core::willow::entry_timestamp_micros(b"garbage").is_err());
    }

    #[test]
    fn list_current_entries_skips_app_data_entries() {
        // Regression: a local `app_data_put` (or its replay on the next open)
        // leaves a live non-alert entry in the store. `list_current_entries`
        // must list alerts only and skip it, rather than fail its "every live
        // id is a known alert" invariant with `Internal` — the bug that left
        // the Tools list empty on every relaunch after using an app.
        let app_id = "ab".repeat(32);
        let profile = open_local_profile().unwrap();
        create_public_space(&profile.inner, "Aid".into()).unwrap();

        // A live app-data entry exists but no alert has been signed.
        app_data_put(
            &profile.inner,
            app_id.clone(),
            "items/a".into(),
            b"hi".to_vec(),
        )
        .unwrap();
        assert!(list_current_entries(&profile.inner).unwrap().is_empty());

        // A signed alert lists, and the app-data entry stays excluded.
        let record = create_draft_alert(&profile.inner, valid_input()).unwrap();
        let signed = sign_draft(&profile.inner, record.draft_id).unwrap();
        let listed = list_current_entries(&profile.inner).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].entry_id, signed.entry.entry_id);
    }

    /// Regression (composite-site Unit 1, #14 follow-up): an owned-namespace
    /// editorial `/articles/...` entry is the opaque owned-editorial family and
    /// MUST be classified as a NON-alert family — exactly like newswire — never
    /// surfaced as an alert. This guards the `is_owned_editorial_entry` clause
    /// the admission commit added to all three FFI classifier sites
    /// (`inspectable_entries`, and in lockstep `list_current_entries` /
    /// `reproject_active`).
    ///
    /// This drives the REAL `inspectable_entries` classifier end-to-end: it
    /// builds a genuine owned masthead site, signs an opaque
    /// `/articles/news/post-1` entry under the owner's owned write capability,
    /// encodes a real bundle, and decodes it with the site's own namespace as
    /// the followed root (exactly what the FFI does — for an owned site the
    /// entry's namespace IS the followed root). We exercise `inspectable_entries`
    /// rather than `list_current_entries` because committing an owned entry into
    /// the active-namespace store requires the followed-site root wired through a
    /// full sync-import session (a known Unit 1 boundary), whereas the bundle
    /// inspector runs the identical classifier and is reachable directly.
    ///
    /// RED without the owned-editorial clause: the opaque payload is not a
    /// decodable alert, so it would fall into the alert branch, `decode_alert`
    /// would fail on it, and the whole bundle would be rejected — turning this
    /// `Ok(non-alert)` into `Err(ImportRejected)`.
    #[test]
    fn inspectable_entries_classifies_owned_articles_as_non_alert() {
        use willow25::prelude::{NamespaceSecret, SubspaceSecret, WriteCapability};

        // An owned masthead site whose secrets the test controls. The namespace
        // id MUST carry the owned marker bit (loop until it does), or the owned
        // admission predicate would never engage.
        let mut seed = [0x30u8; 32];
        let namespace_secret = loop {
            let candidate = NamespaceSecret::from_bytes(&seed);
            if candidate.corresponding_namespace_id().is_owned() {
                break candidate;
            }
            seed[0] = seed[0].wrapping_add(1);
        };
        let namespace_id = namespace_secret.corresponding_namespace_id();
        let owner_secret = SubspaceSecret::from_bytes(&[0x03u8; 32]);
        let owner_id = owner_secret.corresponding_subspace_id();
        let owner_cap = WriteCapability::new_owned(&namespace_secret, owner_id.clone());

        // An opaque editorial article under /articles/ — the path is the
        // identity, the payload is deliberately NOT a decodable alert.
        let payload = b"owned editorial body bytes";
        let entry = riot_core::willow::Entry::builder()
            .namespace_id(namespace_id.clone())
            .subspace_id(owner_id)
            .path(
                riot_core::willow::Path::from_slices(&[
                    riot_core::willow::ARTICLES_COMPONENT,
                    b"news",
                    b"post-1",
                ])
                .expect("article path"),
            )
            .timestamp(100u64)
            .payload(payload)
            .build();
        let authorised = entry
            .into_authorised_entry(&owner_cap, &owner_secret)
            .expect("owner cap authorises its own /articles entry");
        let token = authorised.authorisation_token();
        let signature: Signature = token.signature().clone().into();
        let item = SignedWillowEntry {
            entry_bytes: riot_core::willow::encode_entry(authorised.entry()),
            capability_bytes: riot_core::willow::encode_capability(token.capability()),
            signature: signature.to_bytes(),
            payload_bytes: payload.to_vec(),
        };
        let bundle = encode_bundle(&[item]).expect("owned article bundle encodes");

        // For an owned site the followed root IS the entry's namespace, so the
        // FFI passes exactly that hex — admitting the entry, then classifying it.
        let namespace_hex = hex(namespace_id.as_bytes());
        let inspected = inspectable_entries(&bundle, &namespace_hex)
            .expect("owned /articles entry must be admitted + classified, not rejected");

        assert_eq!(inspected.len(), 1, "the single article is inspectable");
        assert!(
            inspected[0].current.is_none(),
            "owned /articles editorial is the non-alert family: it carries no alert row",
        );
    }

    #[test]
    fn failed_two_entry_share_leaves_store_generation_and_inventory_unchanged() {
        let profile = open_local_profile().unwrap();
        let space = create_public_space(&profile.inner, "Atomic pair".into()).unwrap();
        let (manifest, bundle) = riot_core::apps::starter::STARTER_CATALOG[0];
        let installed = install_app(&profile.inner, manifest.to_vec(), bundle.to_vec()).unwrap();
        let app_id = hex(&installed.app_id_bytes);
        for index in 0..(MAX_SYNC_IDS - 1) {
            app_data_put(
                &profile.inner,
                app_id.clone(),
                format!("items/value-{index}"),
                vec![index as u8],
            )
            .unwrap();
        }
        let before = with_active(&profile.inner, |profile| {
            Ok((
                profile.store.generation().map_err(map_core_error)?,
                profile.sync_inventory.clone(),
            ))
        })
        .unwrap();

        assert!(matches!(
            share_app(&profile.inner, installed.app_id_bytes, space),
            Err(MobileError::SessionLimit)
        ));
        let after = with_active(&profile.inner, |profile| {
            Ok((
                profile.store.generation().map_err(map_core_error)?,
                profile.sync_inventory.clone(),
            ))
        })
        .unwrap();
        assert_eq!(after, before);
    }

    #[test]
    fn successful_share_commits_manifest_and_bundle_in_one_generation() {
        let profile = open_local_profile().unwrap();
        let space = create_public_space(&profile.inner, "Atomic success".into()).unwrap();
        let (manifest, bundle) = riot_core::apps::starter::STARTER_CATALOG[0];
        let installed = install_app(&profile.inner, manifest.to_vec(), bundle.to_vec()).unwrap();
        let before_generation = with_active(&profile.inner, |profile| {
            profile.store.generation().map_err(map_core_error)
        })
        .unwrap();

        share_app(&profile.inner, installed.app_id_bytes, space).unwrap();
        with_active(&profile.inner, |profile| {
            assert_eq!(
                profile.store.generation().map_err(map_core_error)?,
                before_generation + 1
            );
            assert_eq!(profile.store.live_count().map_err(map_core_error)?, 2);
            assert_eq!(profile.sync_inventory.len(), 2);
            let mut slots: Vec<_> = profile
                .sync_inventory
                .iter()
                .map(|signed| {
                    let entry = riot_core::willow::decode_entry_canonic(&signed.entry_bytes)
                        .map_err(|_| MobileError::Internal)?;
                    riot_core::apps::index::classify_app_index_path(entry.path())
                        .ok_or(MobileError::Internal)
                })
                .collect::<Result<_, _>>()?;
            slots.sort_unstable_by_key(|slot| match slot {
                riot_core::apps::index::AppIndexSlot::Manifest { .. } => 0,
                riot_core::apps::index::AppIndexSlot::Bundle { .. } => 1,
                _ => 2,
            });
            assert!(matches!(
                slots.as_slice(),
                [
                    riot_core::apps::index::AppIndexSlot::Manifest { .. },
                    riot_core::apps::index::AppIndexSlot::Bundle { .. }
                ]
            ));
            Ok(())
        })
        .unwrap();
    }

    /// WU1 regression: extracting `namespace_live_ids` must leave the active
    /// community path byte-identical. `active_namespace_live_ids` now delegates
    /// to the generalized helper for the active namespace; the two must agree on
    /// exactly the same live-id set. This is the load-bearing check that the
    /// isolation-critical inventory scoping did not change behavior.
    #[test]
    fn namespace_live_ids_matches_active_scoping_for_the_active_namespace() {
        let profile = open_local_profile().unwrap();
        let space = create_public_space(&profile.inner, "Regression".into()).unwrap();
        let active_ns = parse_entry_id(&space.namespace_id).unwrap();
        let (manifest, bundle) = riot_core::apps::starter::STARTER_CATALOG[0];
        let installed = install_app(&profile.inner, manifest.to_vec(), bundle.to_vec()).unwrap();
        let app_id = hex(&installed.app_id_bytes);
        for index in 0..3u8 {
            app_data_put(
                &profile.inner,
                app_id.clone(),
                format!("items/{index}"),
                vec![index],
            )
            .unwrap();
        }

        with_active(&profile.inner, |profile| {
            let mut via_active = active_namespace_live_ids(profile)?;
            let mut via_generic = namespace_live_ids(profile, &active_ns)?;
            via_active.sort_unstable();
            via_generic.sort_unstable();
            assert!(
                !via_active.is_empty(),
                "the active namespace holds live entries to compare"
            );
            assert_eq!(
                via_active, via_generic,
                "delegation must be byte-identical for the active namespace"
            );
            // A namespace this profile does not hold has no live ids — the
            // per-namespace query cannot reach into the active namespace's set.
            assert!(
                namespace_live_ids(profile, &[0xABu8; 32])?.is_empty(),
                "an unheld namespace yields no live ids"
            );
            Ok(())
        })
        .unwrap();
    }

    /// A durable (SQLite) profile with a few live entries in its active
    /// namespace. Followed-site sync is durable-only, so its tests need this.
    fn durable_profile_with_entries(
        tag: &str,
    ) -> (tempfile::TempDir, Arc<MobileProfile>, [u8; 32]) {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("riot.sqlite");
        let profile =
            open_local_profile_with_database(db_path.to_string_lossy().into_owned()).unwrap();
        let space = create_public_space(&profile.inner, format!("{tag} space")).unwrap();
        let active_ns = parse_entry_id(&space.namespace_id).unwrap();
        let (manifest, bundle) = riot_core::apps::starter::STARTER_CATALOG[0];
        let installed = install_app(&profile.inner, manifest.to_vec(), bundle.to_vec()).unwrap();
        let app_id = hex(&installed.app_id_bytes);
        for index in 0..2u8 {
            app_data_put(
                &profile.inner,
                app_id.clone(),
                format!("items/{index}"),
                vec![index],
            )
            .unwrap();
        }
        (dir, profile, active_ns)
    }

    /// WU2: the followed-site offer is the namespace's live entries in their full
    /// signed form, read verbatim — and byte-identical to what the community
    /// inventory holds for that same namespace (the durable store is the source
    /// for both). Building the offer must NOT mutate `sync_inventory` (the offer
    /// is derived + returned, never stored — the C1a isolation property).
    #[test]
    fn build_followed_site_offer_returns_live_signed_entries_verbatim_without_touching_inventory() {
        let (_dir, profile, active_ns) = durable_profile_with_entries("offer");

        with_active(&profile.inner, |profile| {
            let inventory_before = profile.sync_inventory.clone();
            assert!(
                !inventory_before.is_empty(),
                "the active namespace holds live entries"
            );

            let offer = build_followed_site_offer(profile, &active_ns)?;

            let mut offer_ids: Vec<_> = offer
                .iter()
                .map(|entry| entry_id(&entry.entry_bytes))
                .collect();
            let mut inventory_ids: Vec<_> = inventory_before
                .iter()
                .map(|entry| entry_id(&entry.entry_bytes))
                .collect();
            offer_ids.sort_unstable();
            inventory_ids.sort_unstable();
            assert_eq!(
                offer_ids, inventory_ids,
                "the offer for the active namespace is exactly its live id set"
            );
            for offered in &offer {
                let matching = inventory_before
                    .iter()
                    .find(|held| entry_id(&held.entry_bytes) == entry_id(&offered.entry_bytes))
                    .expect("every offered id is in the inventory");
                assert_eq!(offered.entry_bytes, matching.entry_bytes);
                assert_eq!(offered.capability_bytes, matching.capability_bytes);
                assert_eq!(offered.signature, matching.signature);
                assert_eq!(offered.payload_bytes, matching.payload_bytes);
            }
            assert_eq!(
                profile.sync_inventory, inventory_before,
                "building the offer must not mutate the community sync inventory"
            );
            Ok(())
        })
        .unwrap();
    }

    /// WU2 regression (community path byte-identical): a COMMUNITY sync session
    /// must reproduce today exactly now that `StoredSyncSession` carries a
    /// `followed_root`. It is `None` for the community session, and the community
    /// drive still begins normally. The FULL two-party community drive stays
    /// covered UNCHANGED by the `mobile_contract` integration test — this makes
    /// the "None reproduces today exactly" property unmissable in-crate.
    #[test]
    fn a_community_sync_session_carries_no_followed_root_and_begins_normally() {
        use crate::mobile_api::SyncOutcomeKind;
        let profile = open_local_profile().unwrap();
        create_public_space(&profile.inner, "Community".into()).unwrap();
        let _session = open_sync_session(&profile.inner).unwrap();

        let id = with_active(&profile.inner, |profile| {
            let community = profile
                .sync_session
                .as_ref()
                .expect("community session open");
            assert!(
                community.followed_root.is_none(),
                "a community sync session must carry followed_root = None"
            );
            assert!(
                sync_session_is_active(profile),
                "the community session is active exactly as before"
            );
            Ok(community.id)
        })
        .unwrap();

        // The community drive still starts identically (FrameReady summary).
        assert_eq!(
            sync_begin(&profile.inner, id).unwrap().kind,
            SyncOutcomeKind::FrameReady,
            "the community sync drive is unchanged by the followed_root field"
        );
    }

    /// WU2: on a memory-backed profile the signed form is unavailable
    /// (`signed_entries_in_namespace` -> None), so the offer builder fails closed
    /// rather than returning an empty offer that would silently sync nothing.
    #[test]
    fn build_followed_site_offer_fails_closed_on_a_memory_profile() {
        let profile = open_local_profile().unwrap();
        let space = create_public_space(&profile.inner, "Memory".into()).unwrap();
        let active_ns = parse_entry_id(&space.namespace_id).unwrap();
        let (manifest, bundle) = riot_core::apps::starter::STARTER_CATALOG[0];
        let installed = install_app(&profile.inner, manifest.to_vec(), bundle.to_vec()).unwrap();
        let app_id = hex(&installed.app_id_bytes);
        app_data_put(&profile.inner, app_id, "items/0".into(), vec![0]).unwrap();

        with_active(&profile.inner, |profile| {
            assert!(
                !namespace_live_ids(profile, &active_ns)?.is_empty(),
                "the memory store has live entries, but no signed form",
            );
            assert!(
                build_followed_site_offer(profile, &active_ns).is_err(),
                "a memory profile cannot build a followed-site offer — fail closed"
            );
            Ok(())
        })
        .unwrap();
    }
}

// ===========================================================================
// Spaces-first Rung 1: followed composite sites. These tests exercise the
// `#[cfg(test)] follow_site_for_test` seam, which is INVISIBLE to integration
// tests (they link the lib built without `cfg(test)`), so they must live inline
// here where the seam is compiled in.
// ===========================================================================
#[cfg(test)]
mod spaces_rung1 {
    use crate::mobile_state::open_local_profile;

    #[test]
    fn a_followed_site_is_in_list_followed_sites_and_excluded_from_list_communities() {
        let profile = open_local_profile().unwrap();
        let root_hex = profile.follow_site_for_test(vec![0x11; 32]).unwrap();
        // The followed site appears in the author-less followed list...
        assert!(profile
            .list_followed_sites()
            .unwrap()
            .iter()
            .any(|r| r.root == root_hex));
        // ...and NEVER as a CommunityRow (author-less → filtered out).
        assert!(profile
            .list_communities()
            .unwrap()
            .iter()
            .all(|c| c.namespace_id != root_hex));
    }

    // Task 4 — exposure-boundary guard (Security S2): a followed-site row carries
    // only public identifiers, never key material.
    #[test]
    fn followed_site_row_exposes_only_public_identifiers() {
        let profile = open_local_profile().unwrap();
        let root_hex = profile.follow_site_for_test(vec![0x22; 32]).unwrap();
        let row = profile
            .list_followed_sites()
            .unwrap()
            .into_iter()
            .find(|r| r.root == root_hex)
            .unwrap();
        assert_eq!(row.root.len(), 64);
        assert!(row.root.chars().all(|c| c.is_ascii_hexdigit()));
        // Compile-time: FollowedSiteRow has no Vec<u8>/secret field — reviewed.
    }
}
