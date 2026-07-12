//! UniFFI surface for demo mode: load the seeded space from a signed bundle,
//! and stop listing it again.
//!
//! Same shape as `apps_ffi.rs` and `profile_ffi.rs` — thin delegators on the
//! shared profile object, no state of their own.
//!
//! **There is no privileged import here.** `load_demo_space` hands the bundle to
//! the same `inspect → plan → commit` pipeline a peer's bundle goes through, and
//! a bundle that fails any part of it is refused exactly as a peer's would be.
//! That is why the demo ships as a real signed RIOTE1 bundle: if the demo needed
//! a back door to load, the demo would not be showing the real product.
//!
//! **Failure is deliberately mute.** Every way this can fail — corrupt bytes, a
//! bad signature, a space already listed — comes back as a plain error with no
//! diagnostic code attached, and the native surface renders exactly one sentence
//! for all of them: "Couldn't load the demo space". The import pipeline is
//! transactional, so a refused load leaves the profile in its previous state
//! with nothing half-imported.

use crate::mobile_api::{MobileError, MobileProfile, PublicSpace};

#[uniffi::export]
impl MobileProfile {
    /// Imports the seeded demo space from a signed evidence bundle and lists its
    /// namespace, returning the listed space so the caller can persist it.
    ///
    /// Additive: it refuses if another space is already listed, and leaves that
    /// space's entries bit-for-bit untouched. Idempotent: entries are
    /// content-addressed, so loading the same bundle twice commits nothing the
    /// second time and creates no duplicates.
    ///
    /// Fails while a sync session is open — the commit runs through
    /// `store.inspect`, which would clobber the preview slot an in-flight sync
    /// review is holding.
    pub fn load_demo_space(&self, bytes: Vec<u8>) -> Result<PublicSpace, MobileError> {
        crate::mobile_state::load_demo_space(&self.inner, bytes)
    }

    /// Stops listing the demo space and restores the pre-demo identity.
    ///
    /// Hiding is NOT deleting. Willow is append-only and there is no delete
    /// primitive: the demo's entries stay in the local store, inert, with no
    /// space listing their namespace and nothing in the UI able to reach them.
    /// Reclaiming the bytes means resetting the profile.
    ///
    /// A no-op if demo mode was never on. It never un-lists a space it did not
    /// itself list.
    pub fn hide_demo_space(&self) -> Result<(), MobileError> {
        crate::mobile_state::hide_demo_space(&self.inner)
    }
}
