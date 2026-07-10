//! Fallible communal-author generation and the public identity view.
//!
//! The signer owns zeroizing secret material (ed25519-dalek's `zeroize`
//! feature), is neither `Clone` nor `Debug`, and exposes no accessor for the
//! signing key. The privilege-less communal namespace secret is discarded at
//! generation. Entropy failure returns `ENTROPY_UNAVAILABLE` and constructs
//! no author.

use willow25::authorisation::WriteCapability;
use willow25::prelude::*;

use super::WillowError;

/// A fallible randomness source. Production code uses [`OsEntropy`];
/// deterministic or failing sources live only in tests and conformance.
pub trait EntropySource {
    fn fill(&mut self, buf: &mut [u8]) -> Result<(), WillowError>;
}

/// OS-backed entropy via `rand_core::OsRng::try_fill_bytes`.
pub struct OsEntropy;

impl EntropySource for OsEntropy {
    fn fill(&mut self, buf: &mut [u8]) -> Result<(), WillowError> {
        use rand_core::RngCore;
        rand_core::OsRng
            .try_fill_bytes(buf)
            .map_err(|_| WillowError::EntropyUnavailable)
    }
}

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

    /// Test/conformance constructor from raw secret bytes. Never part of the
    /// release FFI surface.
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

/// Generates a fresh communal namespace and author subspace. Every random
/// byte comes from the provided fallible source; failure constructs nothing.
pub fn generate_communal_author(
    entropy: &mut dyn EntropySource,
) -> Result<EvidenceAuthor, WillowError> {
    // Draw namespace candidates until the public key is communal (even last
    // byte). Each draw consumes fresh entropy; the secret is discarded — it
    // confers no privilege in a communal namespace.
    let mut namespace_id = None;
    for _ in 0..128 {
        let mut secret_bytes = [0u8; 32];
        entropy.fill(&mut secret_bytes)?;
        let candidate = NamespaceSecret::from_bytes(&secret_bytes).corresponding_namespace_id();
        if candidate.is_communal() {
            namespace_id = Some(candidate);
            break;
        }
    }
    let namespace_id = namespace_id.ok_or(WillowError::EntropyUnavailable)?;

    let mut subspace_secret_bytes = [0u8; 32];
    entropy.fill(&mut subspace_secret_bytes)?;
    let subspace_secret = SubspaceSecret::from_bytes(&subspace_secret_bytes);

    Ok(EvidenceAuthor {
        namespace_id,
        subspace_secret,
    })
}
