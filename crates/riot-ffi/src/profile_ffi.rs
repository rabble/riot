//! UniFFI surface for minimal profiles: claim a display name, read back the
//! names this device knows, and resolve any subspace id to something showable.
//!
//! Same shape as `apps_ffi.rs` — typed records, an opaque session handle over
//! the shared `ProfileState`, thin delegators into `mobile_state.rs`.
//!
//! **The rendering rule is enforced HERE, at the boundary.** `resolve_display_names`
//! returns names RAW, exactly as their owner claimed them. A raw name must never
//! cross this boundary as something a native surface could print on its own: it
//! is either passed through `render_display_name` first (`DisplayNameRecord.rendered`,
//! `my_display_name`), or handed over as the STRUCTURED `{display_name, tag}` pair
//! of [`WhoAmI`], whose contract is that the renderer reassembles both halves.
//! There is no third option, and no method here returns a bare claimed name.

use std::sync::Arc;

use crate::mobile_api::{MobileError, MobileProfile};

/// Who the current person is, in the one form an app should store: a stable
/// **id** plus the parts needed to draw it.
///
/// The id is the point. `display_name` is a *claim that can change* — if an app
/// stores the name, then a later rename can never repair the rows it already
/// wrote, and Ana stays under her old name forever in every item she ever
/// touched. Apps store `id` and re-resolve the name at render time (see
/// `profile_for`). The id is the only field here that is stable.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WhoAmI {
    /// The raw 32-byte subspace id — the stable handle, and the same raw-bytes
    /// id convention the directory surface uses.
    pub id: Vec<u8>,
    /// The self-claimed name, or `"member"` if this person has never set one (or
    /// claimed one that sanitizes away to nothing).
    ///
    /// **Self-claimed and unverified.** Never display this alone — it is half of
    /// a pair, and showing it without `tag` is exactly the impersonation the tag
    /// exists to blunt.
    ///
    /// Already run through `sanitize_display_name`: no separator, no bidi or
    /// control characters. It is safe for a renderer to flatten this into
    /// `"{display_name} · {tag}"` — the name cannot forge a second boundary — and
    /// flattening it is the ONLY sanctioned way to show it.
    pub display_name: String,
    /// The key-derived tag: 8 lowercase hex chars of the subspace id. Display as
    /// `"{display_name} · {tag}"` — the same string `my_display_name` returns.
    pub tag: String,
}

/// One `subspace id → name` row, already rendered. The id is raw bytes, matching
/// the convention settled in the apps FFI.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct DisplayNameRecord {
    pub subspace_id: Vec<u8>,
    /// Always via `render_display_name` — e.g. `"Ana · a3f91122"`. Never a bare
    /// claimed name.
    pub rendered: String,
}

#[derive(uniffi::Object)]
pub struct ProfileSession {
    pub(crate) inner: std::sync::Arc<std::sync::Mutex<crate::mobile_state::ProfileState>>,
}

#[uniffi::export]
impl MobileProfile {
    /// The display-name surface for this profile. Stateless handle over the same
    /// profile state, exactly like `app_runtime()` — no lifecycle to cancel.
    pub fn profile(&self) -> Arc<ProfileSession> {
        Arc::new(ProfileSession {
            inner: std::sync::Arc::clone(&self.inner),
        })
    }
}

#[uniffi::export]
impl ProfileSession {
    /// Claims a display name for this person, as an ordinary signed entry in
    /// their own subspace. One slot, last-write-wins: calling this again
    /// replaces the previous name rather than adding a second one.
    ///
    /// The name is bounded by the core codec (`MAX_DISPLAY_NAME_BYTES`), which
    /// is the single enforcement point — an empty or oversized name comes back
    /// as `InvalidInput` from there, never from a duplicate check here.
    ///
    /// Fails with `InvalidInput` while a sync session is open: the commit runs
    /// through `store.inspect`, which would clobber the session-wide preview
    /// slot an in-flight sync review is holding.
    pub fn set_display_name(&self, name: String) -> Result<(), MobileError> {
        crate::mobile_state::set_display_name(&self.inner, name)
    }

    /// This person's own name, rendered: `"Ana · a3f91122"`, or
    /// `"member · a3f91122"` before they have claimed one.
    pub fn my_display_name(&self) -> Result<String, MobileError> {
        crate::mobile_state::my_display_name(&self.inner)
    }

    /// This person's stable id plus the parts to draw it. Apps store the `id`.
    pub fn whoami(&self) -> Result<WhoAmI, MobileError> {
        crate::mobile_state::whoami(&self.inner)
    }

    /// Resolves any subspace id to something showable.
    ///
    /// An id this device has never seen a profile for is NOT an error — it
    /// resolves to the `member` fallback. That is load-bearing: an app must be
    /// able to draw a row authored by someone whose profile has not synced yet,
    /// and a peer who has simply never claimed a name is a normal peer, not a
    /// failure. Only a wrong-length id is an error.
    pub fn profile_for(&self, id: Vec<u8>) -> Result<WhoAmI, MobileError> {
        crate::mobile_state::profile_for(&self.inner, id)
    }

    /// Every display name this device knows, rendered. The id→name map a UI
    /// needs for board rows, endorsement lists, and checklist attribution.
    pub fn display_names(&self) -> Result<Vec<DisplayNameRecord>, MobileError> {
        crate::mobile_state::display_names(&self.inner)
    }
}
