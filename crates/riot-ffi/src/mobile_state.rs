use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex, PoisonError};

use ed25519_dalek::Signature;
use willow25::entry::EntrylikeExt;
use willow25::groupings::Keylike;
use zeroize::{Zeroize, Zeroizing};

use riot_core::import::{
    decode_bundle, encode_bundle, BundleDecodeOutcome, ItemStatus, MAX_BUNDLE_BYTES,
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
    AlertDraft, EvidenceAuthor,
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
const MAX_INSTALLED_APPS: usize = 16;
const MAX_APP_TRUST_MARKERS: usize = 256;
/// The complete retained inventory must fit one protocol bundle. This caps
/// aggregate proof/payload retention at 8 MiB before any session clones it.
const MAX_SYNC_INVENTORY_BYTES: usize = MAX_BUNDLE_BYTES;

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

struct InspectableEntry {
    current: Option<CurrentEntry>,
    signed: SignedWillowEntry,
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
            installed_apps: Vec::new(),
            app_trust_markers: Vec::new(),
            app_data_timestamp_floor_micros: 0,
            demo_mode: None,
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
        // (`app-index/<app_id>/...`), and profile (`profile/<subspace>/card`)
        // entries share this store but are not alerts, so exclude them the same
        // way `ensure_complete_sync_inventory` does — otherwise a single local
        // `app_data_put` or `set_display_name`, or its replay on the next open,
        // leaves a live non-alert entry with no match in `profile.entries` and
        // bricks this listing with `Internal`.
        let app_index_prefix =
            riot_core::willow::Path::from_slices(&[riot_core::apps::index::APP_INDEX_COMPONENT])
                .map_err(|_| MobileError::Internal)?;
        let app_index_ids: std::collections::BTreeSet<_> = profile
            .store
            .entries_with_prefix(&app_index_prefix)
            .map_err(map_core_error)?
            .into_iter()
            .map(|(id, _, _)| id)
            .collect();
        let all_prefix =
            riot_core::willow::Path::from_slices(&[]).map_err(|_| MobileError::Internal)?;
        let alert_ids: Vec<_> = profile
            .store
            .entries_with_prefix(&all_prefix)
            .map_err(map_core_error)?
            .into_iter()
            .filter(|(id, entry, _)| {
                !riot_core::apps::entry::is_app_data_entry(entry)
                    && !app_index_ids.contains(id)
                    && !is_profile_prefixed(entry.path())
            })
            .map(|(id, _, _)| id)
            .collect();
        let mut entries = Vec::with_capacity(alert_ids.len());
        for live_id in alert_ids {
            let live_id = hex(&live_id);
            let entry = profile
                .entries
                .iter()
                .find(|entry| entry.entry_id == live_id)
                .cloned()
                .ok_or(MobileError::Internal)?;
            // One store can hold entries from more than one Willow namespace —
            // loading the demo space and then hiding it again leaves exactly
            // that (Willow is append-only; un-listing is not deleting). An entry
            // outside the listed space is simply not part of THIS board, so skip
            // it. Failing the whole listing here would brick the board for every
            // space the profile lists afterwards.
            if entry.namespace_id != *namespace_id {
                continue;
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
    let preview = inspect_core(&profile.store, bundle_bytes, "conference-sync")?;
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

fn inspectable_entries(
    bytes: &[u8],
    expected_namespace_id: &str,
) -> Result<Vec<InspectableEntry>, MobileError> {
    let decoded = match decode_bundle(bytes) {
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
        // App and profile entries sync and commit like any other, but they are
        // not alerts and carry no alert row. Anything else must decode AS an
        // alert — so a payload that is not one is rejected outright.
        //
        // Profile cards must be listed here explicitly. Without it a synced card
        // falls into the alert branch below, `decode_alert` fails on a
        // profile-card payload, and the ENTIRE import is rejected — which would
        // mean a display name could never reach another device at all.
        let is_non_alert = riot_core::apps::entry::is_app_data_entry(&decoded_entry)
            || riot_core::apps::index::classify_app_index_path(decoded_entry.path()).is_some()
            || is_profile_prefixed(decoded_entry.path());
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

fn install_sync_inventory(
    profile: &mut LocalProfile,
    mut inventory: Vec<SignedWillowEntry>,
) -> Result<(), MobileError> {
    let mut live_ids = profile.store.live_entry_ids().map_err(map_core_error)?;
    live_ids.sort_unstable();
    inventory.retain(|signed| live_ids.contains(&entry_id(&signed.entry_bytes)));
    inventory.sort_unstable_by_key(|signed| entry_id(&signed.entry_bytes));
    let inventory_ids: Vec<_> = inventory
        .iter()
        .map(|signed| entry_id(&signed.entry_bytes))
        .collect();
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
        if !is_space_organizer(profile) {
            return Err(MobileError::InvalidInput);
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
        Ok(())
    })
}

pub(crate) fn is_app_trusted(
    inner: &Arc<Mutex<ProfileState>>,
    app_id: String,
) -> Result<bool, MobileError> {
    with_active(inner, |profile| {
        let app_id = parse_entry_id(&app_id)?;
        let organizer = space_organizer_subspace_id(profile);
        let own_namespace_id = profile.author.identity().namespace_id;

        // Trust must be read from the STORE, not just the profile-local cache: an
        // organizer's approval reaches a member as a synced trust-marker entry.
        // Reading only the local cache (which `set_app_trust` fills for the author)
        // meant an organizer's decision could never reach anyone else — that is the
        // "one decision covers everyone, no install step" property.
        //
        // `is_trusted` fails closed if given two markers for the same coordinate, so
        // the store's Willow-resolved marker and the author's own cached copy must be
        // collapsed to one per (app, organizer) before asking. Newest wins, matching
        // Willow's own per-path resolution.
        let scanned = riot_core::apps::index::scan_app_index(&profile.store)
            .map_err(map_apps_error)?;
        let mut markers: Vec<riot_core::apps::trust::TrustMarker> = Vec::new();
        let mut push_resolved = |marker: riot_core::apps::trust::TrustMarker| {
            match markers.iter_mut().find(|existing| {
                existing.app_id == marker.app_id
                    && existing.author_subspace_id == marker.author_subspace_id
            }) {
                Some(existing) if marker.timestamp_micros > existing.timestamp_micros => {
                    *existing = marker;
                }
                Some(_) => {}
                None => markers.push(marker),
            }
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

        Ok(riot_core::apps::trust::is_trusted(&app_id, &markers, &[organizer]))
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
            .map(|(subspace_id, name)| crate::profile_ffi::DisplayNameRecord {
                subspace_id: subspace_id.to_vec(),
                // Every name crossing the boundary is rendered. `resolve_display_names`
                // hands back the raw claim; this is where it stops being raw.
                rendered: render_display_name(Some(&name), &subspace_id),
            })
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

        // Fresh profile joins the same space and replays the receipt.
        let fresh = open_local_profile().unwrap();
        join_public_space(&fresh.inner, space).unwrap();
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
}
