//! UniFFI surface for the signed-JS-apps runtime: manifest install,
//! per-profile trust decisions, and the namespace-scoped app-data bridge.
//! Same shape as the rest of `mobile_api.rs` — typed records, opaque
//! handles wrapping the shared `ProfileState`, thin delegators into
//! `mobile_state.rs`. Trust gating of *running* an app is the native host's
//! job; these data calls are the raw bridge underneath it.

use std::sync::Arc;

use crate::mobile_api::{MobileError, MobileProfile, PublicSpace};

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

    pub fn trust_app(&self, app_id: String) -> Result<(), MobileError> {
        crate::mobile_state::set_app_trust(&self.inner, app_id, true)
    }

    pub fn untrust_app(&self, app_id: String) -> Result<(), MobileError> {
        crate::mobile_state::set_app_trust(&self.inner, app_id, false)
    }

    pub fn is_app_trusted(&self, app_id: String) -> Result<bool, MobileError> {
        crate::mobile_state::is_app_trusted(&self.inner, app_id)
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
