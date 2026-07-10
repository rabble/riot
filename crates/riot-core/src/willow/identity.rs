//! Communal-author generation and the public identity view.
//!
//! The signer owns zeroizing secret material, is neither `Clone` nor
//! `Debug`, and exposes no accessor for the signing key. The privilege-less
//! communal namespace secret is discarded (zeroized) at generation.
//!
//! The **production** factory `generate_communal_author` takes no injectable
//! sources: it always draws from OS randomness. Injectable entropy and the
//! raw-secret constructor live behind the `conformance` feature, which the
//! release `riot-ffi` graph never enables.

use willow25::authorisation::WriteCapability;
use willow25::prelude::*;
use zeroize::Zeroize;

use super::WillowError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamespaceKind {
    Communal,
}

/// The complete public identity exposed across FFI: full 32-byte IDs, never
/// truncated. `signing_key_id` is the same public identity as `subspace_id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorIdentity {
    pub namespace_id: [u8; 32],
    pub subspace_id: [u8; 32],
    pub namespace_kind: NamespaceKind,
    pub signing_key_id: [u8; 32],
}

/// An ephemeral communal evidence author. Deliberately not `Clone` and not
/// `Debug`: the subspace secret must never be duplicated or printed.
pub struct EvidenceAuthor {
    namespace_id: NamespaceId,
    subspace_secret: SubspaceSecret,
}

impl EvidenceAuthor {
    pub fn namespace_id(&self) -> &NamespaceId {
        &self.namespace_id
    }

    pub fn subspace_id(&self) -> SubspaceId {
        self.subspace_secret.corresponding_subspace_id()
    }

    /// Zero-delegation communal write capability for the author's own subspace.
    pub fn write_capability(&self) -> WriteCapability {
        WriteCapability::new_communal(self.namespace_id.clone(), self.subspace_id())
    }

    pub fn identity(&self) -> AuthorIdentity {
        let subspace = *self.subspace_id().as_bytes();
        AuthorIdentity {
            namespace_id: *self.namespace_id.as_bytes(),
            subspace_id: subspace,
            namespace_kind: NamespaceKind::Communal,
            signing_key_id: subspace,
        }
    }

    pub(crate) fn subspace_secret(&self) -> &SubspaceSecret {
        &self.subspace_secret
    }

    /// Shared constructor core. `fill` writes fresh random bytes; each temp
    /// secret array is zeroized before returning, whatever the outcome.
    fn generate<F>(mut fill: F) -> Result<Self, WillowError>
    where
        F: FnMut(&mut [u8]) -> Result<(), WillowError>,
    {
        // Draw namespace candidates until the public key is communal (even
        // last byte). Each draw consumes fresh entropy; the namespace secret
        // is discarded — it confers no privilege in a communal namespace.
        let mut namespace_id = None;
        for _ in 0..128 {
            let mut secret_bytes = [0u8; 32];
            let result = fill(&mut secret_bytes);
            let candidate = result
                .map(|()| NamespaceSecret::from_bytes(&secret_bytes).corresponding_namespace_id());
            secret_bytes.zeroize();
            let candidate = candidate?;
            if candidate.is_communal() {
                namespace_id = Some(candidate);
                break;
            }
        }
        let namespace_id = namespace_id.ok_or(WillowError::EntropyUnavailable)?;

        let mut subspace_secret_bytes = [0u8; 32];
        let result = fill(&mut subspace_secret_bytes);
        let subspace_secret = result.map(|()| SubspaceSecret::from_bytes(&subspace_secret_bytes));
        subspace_secret_bytes.zeroize();

        Ok(Self {
            namespace_id,
            subspace_secret: subspace_secret?,
        })
    }

    #[cfg(feature = "conformance")]
    pub fn from_parts_for_tests(
        namespace_id: NamespaceId,
        subspace_secret_bytes: &[u8; 32],
    ) -> Self {
        Self {
            namespace_id,
            subspace_secret: SubspaceSecret::from_bytes(subspace_secret_bytes),
        }
    }
}

/// Fills `buf` from OS randomness; `ENTROPY_UNAVAILABLE` on failure.
fn os_fill(buf: &mut [u8]) -> Result<(), WillowError> {
    use rand_core::RngCore;
    rand_core::OsRng
        .try_fill_bytes(buf)
        .map_err(|_| WillowError::EntropyUnavailable)
}

/// Production factory: a fresh communal author from OS randomness only. No
/// injection point exists in the release build.
pub fn generate_communal_author() -> Result<EvidenceAuthor, WillowError> {
    EvidenceAuthor::generate(os_fill)
}

// ---------------------------------------------------------------------------
// Conformance-only injection surface (feature-gated; absent from release).
// ---------------------------------------------------------------------------

/// A fallible randomness source. Test/conformance only.
#[cfg(feature = "conformance")]
pub trait EntropySource {
    fn fill(&mut self, buf: &mut [u8]) -> Result<(), WillowError>;
}

/// OS-backed entropy for tests that want the real source explicitly.
#[cfg(feature = "conformance")]
pub struct OsEntropy;

#[cfg(feature = "conformance")]
impl EntropySource for OsEntropy {
    fn fill(&mut self, buf: &mut [u8]) -> Result<(), WillowError> {
        os_fill(buf)
    }
}

/// Injectable generation for deterministic/failing entropy in tests.
#[cfg(feature = "conformance")]
pub fn generate_communal_author_with(
    entropy: &mut dyn EntropySource,
) -> Result<EvidenceAuthor, WillowError> {
    EvidenceAuthor::generate(|buf| entropy.fill(buf))
}
