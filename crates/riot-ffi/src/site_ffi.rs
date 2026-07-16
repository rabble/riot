//! Composite-site owner FFI: create and restore an `OwnedMasthead`.
//!
//! Owner secret material NEVER crosses the FFI boundary unsealed. The caller
//! supplies a 32-byte wrapping key (used only in-process); what crosses back is
//! solely hex id `String`s plus an opaque sealed `Vec<u8>` (the AEAD envelope
//! produced by `OwnedMasthead::seal`). No `willow25` types, no plaintext root
//! or subspace secrets are exposed. The local key copy is zeroed before return.
//!
//! Unit 0 scope: create + restore only. Editor delegation is deferred to Unit 6.
//!
//! NOTE: `CreatedSite` is a new `uniffi::Record`. Native bindings must be
//! regenerated (and the staticlib rebuilt) before any native app can consume
//! these functions — that regen happens in Unit 6, not here.

use riot_core::willow::OwnedMasthead;

use crate::mobile_api::MobileError;

/// Owner-side result of creating or restoring a composite site.
///
/// All fields are transport-safe: ids are lowercase hex, and `sealed_root` is
/// the opaque encrypted envelope — never plaintext secret material.
#[derive(uniffi::Record)]
pub struct CreatedSite {
    /// The owned namespace id (site root of trust), hex-encoded (64 chars).
    pub namespace_id: String,
    /// The owner's subspace id (receiver of the owner write capability), hex.
    pub owner_subspace_id: String,
    /// The sealed masthead envelope. Opaque to callers; persist as-is and pass
    /// back to `restore_owned_site`.
    pub sealed_root: Vec<u8>,
}

/// Lowercase hex encoding of a byte slice.
fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        value.push(HEX[(byte >> 4) as usize] as char);
        value.push(HEX[(byte & 0x0f) as usize] as char);
    }
    value
}

/// Coerce a caller-supplied key slice into an exact 32-byte array.
fn exact_key(k: &[u8]) -> Result<[u8; 32], MobileError> {
    <[u8; 32]>::try_from(k).map_err(|_| MobileError::InvalidInput)
}

/// Create a fresh composite site owned by the caller.
///
/// Generates a new `OwnedMasthead`, seals it under `wrapping_key`, and returns
/// the site's hex ids plus the opaque sealed root. The in-process key copy is
/// zeroed before returning.
#[uniffi::export]
pub fn create_owned_site(mut wrapping_key: Vec<u8>) -> Result<CreatedSite, MobileError> {
    let key = exact_key(&wrapping_key)?;
    let result = (|| {
        let masthead = OwnedMasthead::generate().map_err(|_| MobileError::InvalidInput)?;
        let sealed_root = masthead.seal(&key).map_err(|_| MobileError::InvalidInput)?;
        Ok(CreatedSite {
            namespace_id: hex(masthead.namespace_id().as_bytes()),
            owner_subspace_id: hex(masthead.owner_subspace_id().as_bytes()),
            sealed_root,
        })
    })();
    wrapping_key.iter_mut().for_each(|b| *b = 0);
    result
}

/// Restore a previously sealed composite site.
///
/// Opens `sealed_root` under `wrapping_key` and returns the site's hex ids,
/// echoing the same sealed root back. Fails if the key is wrong or the envelope
/// is malformed. The in-process key copy is zeroed before returning.
#[uniffi::export]
pub fn restore_owned_site(
    mut wrapping_key: Vec<u8>,
    sealed_root: Vec<u8>,
) -> Result<CreatedSite, MobileError> {
    let key = exact_key(&wrapping_key)?;
    let result = (|| {
        let masthead = OwnedMasthead::open_sealed(&key, &sealed_root)
            .map_err(|_| MobileError::InvalidInput)?;
        Ok(CreatedSite {
            namespace_id: hex(masthead.namespace_id().as_bytes()),
            owner_subspace_id: hex(masthead.owner_subspace_id().as_bytes()),
            sealed_root,
        })
    })();
    wrapping_key.iter_mut().for_each(|b| *b = 0);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_site_returns_owned_namespace_and_sealed_root() {
        let key = vec![0x22; 32];
        let created = create_owned_site(key.clone()).expect("create should succeed");
        assert_eq!(created.namespace_id.len(), 64);
        assert!(!created.sealed_root.is_empty());

        let restored = restore_owned_site(key, created.sealed_root.clone())
            .expect("restore with the same key should succeed");
        assert_eq!(restored.namespace_id, created.namespace_id);
        assert_eq!(restored.owner_subspace_id, created.owner_subspace_id);
    }

    #[test]
    fn restore_with_wrong_key_fails() {
        let created = create_owned_site(vec![0x01; 32]).expect("create should succeed");
        assert!(restore_owned_site(vec![0x02; 32], created.sealed_root).is_err());
    }
}
