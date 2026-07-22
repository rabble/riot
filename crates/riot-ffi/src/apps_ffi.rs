//! UniFFI surface for the signed-JS-apps runtime: manifest install,
//! per-profile trust decisions, and the namespace-scoped app-data bridge.
//! Same shape as the rest of `mobile_api.rs` — typed records, opaque
//! handles wrapping the shared `ProfileState`, thin delegators into
//! `mobile_state.rs`. Trust gating of *running* an app is the native host's
//! job; these data calls are the raw bridge underneath it.

use std::sync::Arc;

use crate::mobile_api::{MobileError, MobileProfile, PublicSpace};

/// The canonical bytes of an app the profile holds, as one verified read.
/// A host needs both halves — the bundle to serve the app's pages, the
/// manifest to re-admit it after a relaunch.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AppPairBytes {
    pub manifest_bytes: Vec<u8>,
    pub bundle_bytes: Vec<u8>,
}

/// One `(relative key, value)` pair of an app's own data.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AppDataItem {
    pub key: String,
    pub value: Vec<u8>,
}

/// The plain-language surface shown to a person deciding whether to trust
/// an installed app, plus its content-derived id (hex, 32 bytes).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct InstalledAppRecord {
    pub app_id: String,
    /// The same id as raw bytes — the convention the directory surface
    /// (`DirectoryListing.app_id`, `share_app`, `endorse_app`) uses, so
    /// natives never bridge the hex/raw seam themselves. `app_id` stays hex
    /// for released callers.
    pub app_id_bytes: Vec<u8>,
    pub name: String,
    pub description: String,
    pub version: String,
    pub entry_point: String,
    pub permissions: Vec<String>,
}

/// The result of `prepare_app_trust` (WU-002a): the app id + decision the host
/// records in its durable trusted-ID set between prepare and finalize. No trust
/// marker bytes cross the FFI for trust — restart re-issues per persisted id.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PreparedTrustRecord {
    pub app_id: String,
    pub trusted: bool,
}

/// Receipt bytes produced before an app-data mutation reaches the live store.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PreparedAppDataRecord {
    pub receipt: Vec<u8>,
}

/// One row of the computed app directory (`riot_core::apps::directory::
/// AppListing` flattened to FFI types). Unlike `InstalledAppRecord`, whose
/// `app_id` predates the directory surface and is hex text, all 32-byte ids
/// here are raw bytes — the shape the native storefront was promised.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct DirectoryListing {
    pub app_id: Vec<u8>,
    pub name: String,
    pub description: String,
    pub version: String,
    pub author_signing_key_id: Vec<u8>,
    pub permissions: Vec<String>,
    pub bundle_present: bool,
    pub built_in: bool,
    /// Whether this app was installed on this profile via `install_app`
    /// (starter built-ins start uninstalled).
    pub installed: bool,
    /// Set for carried apps: the subspace of the carrier whose complete
    /// verified pair won the scan. `None` for built-ins.
    pub carrier_subspace_id: Option<Vec<u8>>,
    /// Namespace ids of spaces whose recognized organizers currently trust
    /// this app.
    pub trusted_in_spaces: Vec<Vec<u8>>,
    /// Endorsing subspaces this profile has actually seen entries from.
    pub endorsing_met_subspaces: Vec<Vec<u8>>,
    /// Endorsers this profile has never met only bump an anonymous count.
    pub endorsing_unmet_count: u32,
    pub superseded_by: Option<Vec<u8>>,
}

#[derive(uniffi::Object)]
pub struct AppRuntimeSession {
    pub(crate) inner: std::sync::Arc<std::sync::Mutex<crate::mobile_state::ProfileState>>,
}

/// A Rust-owned, generation-bound handle for RUNNING one trusted app (Unit 0C).
///
/// Unlike `AppRuntimeSession` — a stateless bridge whose `app_data_*` calls
/// trust-gate nothing and take an arbitrary `app_id` — an `AppExecutionSession`
/// is scoped to exactly one app and revalidates its authority on every read and
/// commit. It is opened only for a currently-trusted app and captures the
/// approval generation and namespace at that instant. Four ways an app can be
/// invalidated each make the very next op fail *before* it touches data:
///
///   - **revoke** — trust withdrawn;
///   - **namespace replacement** — the profile's namespace is swapped;
///   - **explicit destruction** — the host calls `destroy`;
///   - **stale approval-generation** — the app is re-approved, so a session
///     opened before the re-approval fails even though trust is now TRUE.
///
/// This is the mechanism that lets the WebView host stop being the sole
/// enforcement point: containment holds even if the host keeps calling.
/// Every failure surfaces as `AppRejected` — the host's cue to tear the app
/// down and return to a named destination (recovery-state contract §4.7).
#[derive(uniffi::Object)]
pub struct AppExecutionSession {
    inner: std::sync::Arc<std::sync::Mutex<crate::mobile_state::ProfileState>>,
    snapshot: crate::mobile_state::AppExecutionSnapshot,
    destroyed: std::sync::atomic::AtomicBool,
}

#[uniffi::export]
impl MobileProfile {
    /// Open an execution session for `app_id`, which must be trusted right now.
    /// Returns `AppRejected` if it is not — the launch gate, enforced in Rust.
    pub fn open_app_execution(
        &self,
        app_id: String,
    ) -> Result<Arc<AppExecutionSession>, MobileError> {
        let snapshot = crate::mobile_state::app_execution_open(&self.inner, app_id)?;
        Ok(Arc::new(AppExecutionSession {
            inner: std::sync::Arc::clone(&self.inner),
            snapshot,
            destroyed: std::sync::atomic::AtomicBool::new(false),
        }))
    }
}

impl AppExecutionSession {
    /// True once the host has torn this session down. Read before every op so a
    /// destroyed session denies closed without reaching the store.
    fn is_destroyed(&self) -> bool {
        self.destroyed.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[uniffi::export]
impl AppExecutionSession {
    /// Read one key of this app's data, after revalidating the session.
    pub fn app_data_get(&self, key: String) -> Result<Option<Vec<u8>>, MobileError> {
        if self.is_destroyed() {
            return Err(MobileError::AppRejected);
        }
        crate::mobile_state::app_execution_get(&self.inner, &self.snapshot, key)
    }

    /// List this app's data under `prefix`, after revalidating the session.
    pub fn app_data_list(&self, prefix: String) -> Result<Vec<AppDataItem>, MobileError> {
        if self.is_destroyed() {
            return Err(MobileError::AppRejected);
        }
        crate::mobile_state::app_execution_list(&self.inner, &self.snapshot, prefix)
    }

    /// Commit one key of this app's data, after revalidating the session.
    pub fn app_data_put(&self, key: String, value: Vec<u8>) -> Result<(), MobileError> {
        self.app_data_put_with_receipt(key, value).map(|_| ())
    }

    /// `app_data_put` that also returns the signed bundle bytes it committed, for
    /// a host that persists app data across relaunch.
    pub fn app_data_put_with_receipt(
        &self,
        key: String,
        value: Vec<u8>,
    ) -> Result<Vec<u8>, MobileError> {
        if self.is_destroyed() {
            return Err(MobileError::AppRejected);
        }
        crate::mobile_state::app_execution_put_with_receipt(&self.inner, &self.snapshot, key, value)
    }

    /// Prepare persistence bytes without committing app data.
    pub fn prepare_app_execution_put(
        &self,
        key: String,
        value: Vec<u8>,
    ) -> Result<PreparedAppDataRecord, MobileError> {
        if self.is_destroyed() {
            return Err(MobileError::AppRejected);
        }
        crate::mobile_state::prepare_app_execution_put(&self.inner, &self.snapshot, key, value)
    }

    /// Commit the prepared app-data write after host persistence succeeds.
    pub fn finalize_app_execution_put(&self) -> Result<(), MobileError> {
        if self.is_destroyed() {
            return Err(MobileError::AppRejected);
        }
        crate::mobile_state::finalize_app_execution_put(&self.inner, &self.snapshot)
    }

    /// Whether this session is still valid right now: not destroyed, and passing
    /// the same revocation / namespace / generation revalidation the data path
    /// runs. The native bridge calls this after an `AppRejected` from a read or
    /// commit to tell an INVALIDATION (revoked / namespace-swapped / stale
    /// generation → close the app to "Return to Tools", §4.7) apart from an
    /// ordinary per-op rejection (a malformed key → inline error, stay open).
    /// Because both surface as the same `AppRejected`, this is the disambiguator.
    pub fn is_valid(&self) -> bool {
        if self.is_destroyed() {
            return false;
        }
        crate::mobile_state::app_execution_is_valid(&self.inner, &self.snapshot)
    }

    /// Tear this session down. Idempotent. Every subsequent read/commit fails
    /// with `AppRejected`. The host calls this when it closes the app view or
    /// navigates away, so no in-flight bridge call can outlive the UI.
    ///
    /// NOT named `destroy`: UniFFI reserves `destroy()` on every object for its
    /// Kotlin `Disposable` handle-release, and an exported `destroy` collides
    /// with it in the generated binding. `invalidate` is the containment verb.
    pub fn invalidate(&self) {
        self.destroyed
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

#[uniffi::export]
impl MobileProfile {
    /// The app-runtime surface for this profile. Stateless handle over the
    /// same profile state — no separate lifecycle to cancel, unlike
    /// `MobileSyncSession`.
    pub fn app_runtime(&self) -> Arc<AppRuntimeSession> {
        Arc::new(AppRuntimeSession {
            inner: std::sync::Arc::clone(&self.inner),
        })
    }
}

#[uniffi::export]
impl AppRuntimeSession {
    pub fn install_app(
        &self,
        manifest_bytes: Vec<u8>,
        bundle_bytes: Vec<u8>,
    ) -> Result<InstalledAppRecord, MobileError> {
        crate::mobile_state::install_app(&self.inner, manifest_bytes, bundle_bytes)
    }

    /// Install an app this profile already holds — one that arrived over
    /// nearby sync, or was published here — from the store's own bytes. This
    /// is how a `DirectoryListing` with `bundle_present` is opened; the caller
    /// never has to hold the manifest or bundle itself. Takes the raw 32-byte
    /// `DirectoryListing.app_id`.
    ///
    /// `AppRejected` when the app cannot be opened from here: never arrived,
    /// bundle still in flight, or no stored copy re-derives this id.
    pub fn install_from_directory(
        &self,
        app_id: Vec<u8>,
    ) -> Result<InstalledAppRecord, MobileError> {
        crate::mobile_state::install_from_directory(&self.inner, app_id)
    }

    /// The stored manifest+bundle bytes for an app this profile holds.
    /// `install_from_directory` admits a carried app into the runtime, but the
    /// native host needs the bytes themselves for two things the store cannot do
    /// for it: *serve* the app's pages to the WebView, and *persist* the app so
    /// it survives a relaunch (the store is in-memory; a host re-admits its apps
    /// on open exactly the way it admits the starter catalog). A carried app has
    /// no local file to read either half from — the store holds the only copy.
    ///
    /// Both halves come from one verified read, so they can never disagree: they
    /// are the same canonical bytes the pair invariant checked, and a host that
    /// decodes them re-derives this exact `app_id`.
    ///
    /// `AppRejected` on the same conditions as `install_from_directory`.
    pub fn app_pair_bytes(&self, app_id: Vec<u8>) -> Result<AppPairBytes, MobileError> {
        crate::mobile_state::app_pair_bytes(&self.inner, app_id)
    }

    pub fn trust_app(&self, app_id: String) -> Result<(), MobileError> {
        crate::mobile_state::set_app_trust(&self.inner, app_id, true)
    }

    pub fn untrust_app(&self, app_id: String) -> Result<(), MobileError> {
        crate::mobile_state::set_app_trust(&self.inner, app_id, false)
    }

    /// Two-phase trust, phase 1 (WU-002a): validate + sign without mutating the
    /// live store. The host durably records the returned `{app_id, trusted}` in
    /// its trusted-ID set, then calls `finalize_app_trust`.
    pub fn prepare_app_trust(
        &self,
        app_id: String,
        trusted: bool,
    ) -> Result<PreparedTrustRecord, MobileError> {
        crate::mobile_state::prepare_app_trust(&self.inner, app_id, trusted)
    }

    /// Two-phase trust, phase 2 (WU-002a): commit the held prepared mutation
    /// after the durable persist. Errors (trust unchanged) if nothing is
    /// prepared or the generation moved.
    pub fn finalize_app_trust(&self) -> Result<(), MobileError> {
        crate::mobile_state::finalize_app_trust(&self.inner)
    }

    /// Drop a prepared trust mutation without committing (host persist failed or
    /// the flow was cancelled).
    pub fn discard_prepared_trust(&self) -> Result<(), MobileError> {
        crate::mobile_state::discard_prepared_trust(&self.inner)
    }

    pub fn is_app_trusted(&self, app_id: String) -> Result<bool, MobileError> {
        crate::mobile_state::is_app_trusted(&self.inner, app_id)
    }

    /// True when this profile may approve apps for its space — i.e. when
    /// `trust_app` would get past the organizer gate.
    ///
    /// Ask BEFORE drawing "Let everyone here use this". Offering a button that
    /// cannot succeed is how the original bug felt from the outside: the tap did
    /// nothing and said nothing.
    pub fn is_organizer(&self) -> Result<bool, MobileError> {
        crate::mobile_state::is_organizer(&self.inner)
    }

    /// False only for a profile made before spaces had organizers, which can
    /// never approve an app for any space. Separates "ask the organizer" (a
    /// member, fine) from "start a new profile" (legacy, and nothing else works).
    pub fn can_organize(&self) -> Result<bool, MobileError> {
        crate::mobile_state::can_organize(&self.inner)
    }

    pub fn app_data_put(
        &self,
        app_id: String,
        key: String,
        value: Vec<u8>,
    ) -> Result<(), MobileError> {
        crate::mobile_state::app_data_put(&self.inner, app_id, key, value)
    }

    /// `app_data_put` that also returns the canonical signed bundle bytes it
    /// committed, for a host that persists app data across relaunch. The
    /// void-returning `app_data_put` above stays for callers that don't need
    /// the receipt (Android/iOS bridges); both commit identically.
    pub fn app_data_put_with_receipt(
        &self,
        app_id: String,
        key: String,
        value: Vec<u8>,
    ) -> Result<Vec<u8>, MobileError> {
        crate::mobile_state::app_data_put_with_receipt(&self.inner, app_id, key, value)
    }

    /// Prepare persistence bytes without committing app data.
    pub fn prepare_app_data_put(
        &self,
        app_id: String,
        key: String,
        value: Vec<u8>,
    ) -> Result<PreparedAppDataRecord, MobileError> {
        crate::mobile_state::prepare_app_data_put(&self.inner, app_id, key, value)
    }

    /// Commit the prepared app-data write after host persistence succeeds.
    pub fn finalize_app_data_put(&self) -> Result<(), MobileError> {
        crate::mobile_state::finalize_app_data_put(&self.inner)
    }

    /// Clear the shared prepared-mutation slot without committing.
    pub fn discard_prepared_app_data(&self) -> Result<(), MobileError> {
        crate::mobile_state::discard_prepared_app_data(&self.inner)
    }

    /// Re-admits app-data bundle bytes previously returned by
    /// `app_data_put_with_receipt`, rebuilding persisted app state on a fresh
    /// profile. Rejects any bundle that is not app-data-only.
    pub fn replay_app_data_bundle(&self, bytes: Vec<u8>) -> Result<(), MobileError> {
        crate::mobile_state::replay_app_data_bundle(&self.inner, bytes)
    }

    /// A short, stable, non-identifying label for the current person that an
    /// app may display (`"member-"` + 8 hex chars of the subspace id).
    pub fn app_display_name(&self) -> Result<String, MobileError> {
        crate::mobile_state::app_display_name(&self.inner)
    }

    pub fn app_data_get(
        &self,
        app_id: String,
        key: String,
    ) -> Result<Option<Vec<u8>>, MobileError> {
        crate::mobile_state::app_data_get(&self.inner, app_id, key)
    }

    pub fn app_data_list(
        &self,
        app_id: String,
        prefix: String,
    ) -> Result<Vec<AppDataItem>, MobileError> {
        crate::mobile_state::app_data_list(&self.inner, app_id, prefix)
    }

    /// The computed app directory: starter catalog + every verified app in
    /// the live app-index, with trust and endorsement summaries. Assembled
    /// on demand, never stored.
    pub fn directory_listings(&self) -> Result<Vec<DirectoryListing>, MobileError> {
        crate::mobile_state::directory_listings(&self.inner)
    }

    /// Re-publishes an app's manifest+bundle into `space` with this profile
    /// as carrier. Sharing never auto-trusts.
    pub fn share_app(&self, app_id: Vec<u8>, space: PublicSpace) -> Result<(), MobileError> {
        crate::mobile_state::share_app(&self.inner, app_id, space)
    }

    /// Writes (or, with `retract`, withdraws) this profile's endorsement
    /// marker for an app at its own Willow coordinate. Endorsing an app id
    /// whose pair has not arrived yet is permitted by design — the marker
    /// composes with the app's later arrival.
    pub fn endorse_app(
        &self,
        app_id: Vec<u8>,
        note: String,
        retract: bool,
    ) -> Result<(), MobileError> {
        crate::mobile_state::endorse_app(&self.inner, app_id, note, retract)
    }

    /// Named retraction for storefront call sites: `endorse_app` with an
    /// empty note and `retract` set.
    pub fn retract_endorsement(&self, app_id: Vec<u8>) -> Result<(), MobileError> {
        crate::mobile_state::endorse_app(&self.inner, app_id, String::new(), true)
    }
}
