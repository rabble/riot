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
use zeroize::{Zeroize, Zeroizing};

use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    Key, XChaCha20Poly1305, XNonce,
};

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

const SEALED_IDENTITY_MAGIC: &[u8; 8] = b"RIOTID\x01\0";
const SEALED_IDENTITY_AAD: &[u8] = b"riot/evidence-author/sealed/v1";
const SEALED_IDENTITY_NONCE_BYTES: usize = 24;
const SEALED_IDENTITY_PLAINTEXT_BYTES: usize = 64;
const SEALED_IDENTITY_TAG_BYTES: usize = 16;
pub const SEALED_IDENTITY_BYTES: usize = SEALED_IDENTITY_MAGIC.len()
    + SEALED_IDENTITY_NONCE_BYTES
    + SEALED_IDENTITY_PLAINTEXT_BYTES
    + SEALED_IDENTITY_TAG_BYTES;

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

    /// Authenticates and encrypts the complete signer identity under an
    /// application-provided 32-byte wrapping key. Only the opaque fixed-size
    /// blob may cross FFI; plaintext signer material is zeroized immediately.
    pub fn seal_identity(&self, wrapping_key: &[u8; 32]) -> Result<Vec<u8>, WillowError> {
        let mut nonce = [0u8; SEALED_IDENTITY_NONCE_BYTES];
        os_fill(&mut nonce)?;

        let mut plaintext = Zeroizing::new([0u8; SEALED_IDENTITY_PLAINTEXT_BYTES]);
        plaintext[..32].copy_from_slice(self.namespace_id.as_bytes());
        plaintext[32..].copy_from_slice(self.subspace_secret.as_bytes());
        let cipher = XChaCha20Poly1305::new(Key::from_slice(wrapping_key));
        let encrypted = cipher.encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: &plaintext[..],
                aad: SEALED_IDENTITY_AAD,
            },
        );
        let ciphertext = encrypted.map_err(|_| WillowError::IdentitySealFailed)?;

        let mut sealed = Vec::with_capacity(SEALED_IDENTITY_BYTES);
        sealed.extend_from_slice(SEALED_IDENTITY_MAGIC);
        sealed.extend_from_slice(&nonce);
        sealed.extend_from_slice(&ciphertext);
        nonce.zeroize();
        debug_assert_eq!(sealed.len(), SEALED_IDENTITY_BYTES);
        Ok(sealed)
    }

    /// Restores an author only after the fixed envelope and AEAD tag validate.
    /// No partially constructed author is returned on any failure.
    pub fn open_sealed_identity(
        wrapping_key: &[u8; 32],
        sealed: &[u8],
    ) -> Result<Self, WillowError> {
        if sealed.len() != SEALED_IDENTITY_BYTES
            || &sealed[..SEALED_IDENTITY_MAGIC.len()] != SEALED_IDENTITY_MAGIC
        {
            return Err(WillowError::SealedIdentityInvalid);
        }
        let nonce_start = SEALED_IDENTITY_MAGIC.len();
        let ciphertext_start = nonce_start + SEALED_IDENTITY_NONCE_BYTES;
        let cipher = XChaCha20Poly1305::new(Key::from_slice(wrapping_key));
        let plaintext = cipher
            .decrypt(
                XNonce::from_slice(&sealed[nonce_start..ciphertext_start]),
                Payload {
                    msg: &sealed[ciphertext_start..],
                    aad: SEALED_IDENTITY_AAD,
                },
            )
            .map(Zeroizing::new)
            .map_err(|_| WillowError::SealedIdentityInvalid)?;
        if plaintext.len() != SEALED_IDENTITY_PLAINTEXT_BYTES {
            return Err(WillowError::SealedIdentityInvalid);
        }

        let mut namespace_bytes = Zeroizing::new([0u8; 32]);
        namespace_bytes.copy_from_slice(&plaintext[..32]);
        let mut subspace_secret_bytes = Zeroizing::new([0u8; 32]);
        subspace_secret_bytes.copy_from_slice(&plaintext[32..]);

        let namespace_id = NamespaceId::from_bytes(&namespace_bytes);
        if !namespace_id.is_communal() {
            return Err(WillowError::SealedIdentityInvalid);
        }
        let subspace_secret = SubspaceSecret::from_bytes(&subspace_secret_bytes);
        Ok(Self {
            namespace_id,
            subspace_secret,
        })
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

/// Generates a fresh author subspace inside an existing communal namespace.
/// The caller supplies only the complete public namespace ID; fresh signing
/// material comes exclusively from OS entropy and never crosses this API.
pub fn generate_communal_author_for_namespace(
    namespace_id_bytes: [u8; 32],
) -> Result<EvidenceAuthor, WillowError> {
    let namespace_id = NamespaceId::from_bytes(&namespace_id_bytes);
    if !namespace_id.is_communal() {
        return Err(WillowError::NamespaceNotCommunal);
    }

    let mut subspace_secret_bytes = [0u8; 32];
    let result = os_fill(&mut subspace_secret_bytes);
    let subspace_secret = result.map(|()| SubspaceSecret::from_bytes(&subspace_secret_bytes));
    subspace_secret_bytes.zeroize();

    Ok(EvidenceAuthor {
        namespace_id,
        subspace_secret: subspace_secret?,
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealed_author_roundtrips_full_public_identity() {
        let author = generate_communal_author().unwrap();
        let identity = author.identity();
        let key = [0x91; 32];
        let sealed = author.seal_identity(&key).unwrap();

        assert_eq!(sealed.len(), SEALED_IDENTITY_BYTES);
        assert_ne!(&sealed[32..64], author.subspace_secret.as_bytes());
        assert_eq!(
            EvidenceAuthor::open_sealed_identity(&key, &sealed)
                .unwrap()
                .identity(),
            identity
        );
    }

    #[test]
    fn authenticated_noncommunal_plaintext_is_still_rejected() {
        let key = [0x37; 32];
        let mut namespace_bytes = [0u8; 32];
        namespace_bytes[31] = 1;
        assert!(!NamespaceId::from_bytes(&namespace_bytes).is_communal());

        let mut plaintext = [0u8; SEALED_IDENTITY_PLAINTEXT_BYTES];
        plaintext[..32].copy_from_slice(&namespace_bytes);
        plaintext[32..].copy_from_slice(&[0x55; 32]);
        let nonce = [0x22; SEALED_IDENTITY_NONCE_BYTES];
        let cipher = XChaCha20Poly1305::new(Key::from_slice(&key));
        let ciphertext = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &plaintext,
                    aad: SEALED_IDENTITY_AAD,
                },
            )
            .unwrap();
        plaintext.zeroize();
        let mut sealed = Vec::with_capacity(SEALED_IDENTITY_BYTES);
        sealed.extend_from_slice(SEALED_IDENTITY_MAGIC);
        sealed.extend_from_slice(&nonce);
        sealed.extend_from_slice(&ciphertext);

        assert!(matches!(
            EvidenceAuthor::open_sealed_identity(&key, &sealed),
            Err(WillowError::SealedIdentityInvalid)
        ));
    }
}
