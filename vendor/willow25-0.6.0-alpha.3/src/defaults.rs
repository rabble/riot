//! [Default values](https://willowprotocol.org/specs/willow25/index.html#willow25_defaults) for Willow’25.
//!
//! ```
//! use willow25::defaults::*;
//! use willow25::prelude::*;
//!
//! let namespace = default_namespace_id();
//!
//! let digest = default_payload_digest();
//! assert_eq!(DEFAULT_PAYLOAD_LENGTH, 0);
//!
//! let token = default_authorisation_token();
//! ```

use crate::authorisation::{AuthorisationToken, PossiblyAuthorisedEntry, WriteCapability};
use crate::prelude::*;
use ed25519_dalek::{SECRET_KEY_LENGTH, SIGNATURE_LENGTH, Signature, SigningKey};
#[cfg(feature = "std")]
use std::sync::LazyLock;

/// The default namespace (and subspace) ID as bytes, as specified in the [Willow protocol](https://willowprotocol.org/specs/willow25/index.html#willow25_default_namespace_id).
const DEFAULT_NAMESPACE_ID_BYTES: [u8; NAMESPACE_ID_WIDTH] = [
    147, 78, 96, 33, 51, 158, 31, 1, 59, 169, 73, 0, 237, 194, 93, 141, 116, 192, 180, 229, 115,
    118, 137, 16, 174, 15, 80, 125, 140, 129, 115, 24,
];

/// The default namespace (and subspace) secret as bytes
const DEFAULT_NAMESPACE_SECRET_BYTES: [u8; SECRET_KEY_LENGTH] = [
    94, 20, 172, 228, 210, 200, 2, 143, 200, 154, 143, 4, 118, 91, 25, 210, 205, 117, 45, 145, 187,
    55, 60, 12, 158, 212, 118, 39, 107, 92, 69, 65,
];

/// The [default payload digest](https://willowprotocol.org/specs/confidential-sync/index.html#sync_default_payload_digest) as bytes (WILLIAM3 digest of the empty string).
const DEFAULT_PAYLOAD_DIGEST_BYTES: [u8; PAYLOAD_DIGEST_WIDTH] = [
    150, 211, 76, 84, 120, 69, 130, 49, 227, 100, 118, 121, 82, 170, 234, 2, 163, 29, 34, 3, 198,
    111, 67, 101, 105, 46, 249, 31, 53, 16, 104, 210,
];

/// Since the default payload digest is the WILLIAM3 digest of the empty string, this constant is 0.
pub const DEFAULT_PAYLOAD_LENGTH: u64 = 0;

/// The [signature](https://willowprotocol.org/specs/meadowcap/index.html#mcat_sig) for the [`default_authorisation_token`] as bytes.
const DEFAULT_AUTHORISATION_TOKEN_SIGNATURE_BYTES: [u8; SIGNATURE_LENGTH] = [
    42, 201, 58, 210, 193, 63, 237, 182, 150, 52, 93, 186, 198, 231, 18, 84, 233, 156, 180, 68,
    229, 232, 27, 145, 111, 202, 8, 120, 240, 168, 169, 147, 43, 209, 164, 232, 70, 225, 202, 131,
    55, 123, 116, 10, 41, 184, 87, 10, 133, 3, 141, 80, 102, 43, 168, 175, 102, 229, 104, 60, 147,
    82, 20, 1,
];

fn build_default_authorisation_token() -> AuthorisationToken {
    let capability = WriteCapability::new_communal(default_namespace_id(), default_subspace_id());

    AuthorisationToken::new_for_entry(&default_entry(), &capability, &default_subspace_secret())
        .expect("default authorisation token should be valid")
}

#[cfg(feature = "std")]
static DEFAULT_NAMESPACE_ID: LazyLock<NamespaceId> =
    LazyLock::new(|| NamespaceId::from_bytes(&DEFAULT_NAMESPACE_ID_BYTES));
#[cfg(feature = "std")]
static DEFAULT_SUBSPACE_ID: LazyLock<SubspaceId> =
    LazyLock::new(|| SubspaceId::from_bytes(&DEFAULT_NAMESPACE_ID_BYTES));

#[cfg(feature = "std")]
static DEFAULT_NAMESPACE_SECRET: LazyLock<NamespaceSecret> =
    LazyLock::new(|| NamespaceSecret(SigningKey::from_bytes(&DEFAULT_NAMESPACE_SECRET_BYTES)));
#[cfg(feature = "std")]
static DEFAULT_SUBSPACE_SECRET: LazyLock<SubspaceSecret> =
    LazyLock::new(|| SubspaceSecret(SigningKey::from_bytes(&DEFAULT_NAMESPACE_SECRET_BYTES)));

#[cfg(feature = "std")]
static DEFAULT_PAYLOAD_DIGEST: LazyLock<PayloadDigest> =
    LazyLock::new(|| PayloadDigest::from(DEFAULT_PAYLOAD_DIGEST_BYTES));

#[cfg(feature = "std")]
static DEFAULT_AUTHORISATION_TOKEN: LazyLock<AuthorisationToken> =
    LazyLock::new(build_default_authorisation_token);

/// Returns the [default_namespace_id](https://willowprotocol.org/specs/willow25/index.html#willow25_default_namespace_id).
pub fn default_namespace_id() -> NamespaceId {
    #[cfg(feature = "std")]
    return DEFAULT_NAMESPACE_ID.clone();

    #[cfg(not(feature = "std"))]
    return NamespaceId::from_bytes(&DEFAULT_NAMESPACE_ID_BYTES);
}

/// Returns the [default_subspace_id](https://willowprotocol.org/specs/willow25/index.html#willow25_default_subspace_id).
pub fn default_subspace_id() -> SubspaceId {
    #[cfg(feature = "std")]
    return DEFAULT_SUBSPACE_ID.clone();

    #[cfg(not(feature = "std"))]
    return SubspaceId::from_bytes(&DEFAULT_NAMESPACE_ID_BYTES);
}

/// Returns the default namespace secret, i.e. the secret key that corresponds to the [`default_namespace_id`].
pub fn default_namespace_secret() -> NamespaceSecret {
    #[cfg(feature = "std")]
    return DEFAULT_NAMESPACE_SECRET.clone();

    #[cfg(not(feature = "std"))]
    return NamespaceSecret(SigningKey::from_bytes(&DEFAULT_NAMESPACE_SECRET_BYTES));
}

/// Returns the default subspace secret, i.e. the secret key that corresponds to the [`default_subspace_id`].
pub fn default_subspace_secret() -> SubspaceSecret {
    #[cfg(feature = "std")]
    return DEFAULT_SUBSPACE_SECRET.clone();

    #[cfg(not(feature = "std"))]
    return SubspaceSecret(SigningKey::from_bytes(&DEFAULT_NAMESPACE_SECRET_BYTES));
}

/// Returns the [default_payload_digest](https://willowprotocol.org/specs/confidential-sync/index.html#sync_default_payload_digest).
pub fn default_payload_digest() -> PayloadDigest {
    #[cfg(feature = "std")]
    return DEFAULT_PAYLOAD_DIGEST.clone();

    #[cfg(not(feature = "std"))]
    return PayloadDigest::from(DEFAULT_PAYLOAD_DIGEST_BYTES);
}

/// Returns the [default_authorisation_token](https://willowprotocol.org/specs/confidential-sync/index.html#sync_default_authorisation_token).
pub fn default_authorisation_token() -> AuthorisationToken {
    #[cfg(feature = "std")]
    return DEFAULT_AUTHORISATION_TOKEN.clone();

    #[cfg(not(feature = "std"))]
    return build_default_authorisation_token();
}

/// Returns the [signature](https://willowprotocol.org/specs/meadowcap/index.html#mcat_sig) for the [`default_authorisation_token`].
///
/// ```
/// use willow25::defaults::{default_authorisation_token, default_authorisation_token_signature};
///
/// let token = default_authorisation_token();
/// let token_signature: ed25519_dalek::Signature = token.signature().clone().into();
///
/// assert_eq!(default_authorisation_token_signature(), token_signature.into());
/// ```
pub fn default_authorisation_token_signature() -> SubspaceSignature {
    SubspaceSignature::from(Signature::from_bytes(
        &DEFAULT_AUTHORISATION_TOKEN_SIGNATURE_BYTES,
    ))
}

/// Returns the [default_entry](https://willowprotocol.org/misc-definitions/index.html#default_entry).
pub fn default_entry() -> Entry {
    Entry::builder()
        .default_namespace_id()
        .default_subspace_id()
        .path(Path::new())
        .timestamp(0)
        .default_payload()
        .build()
}

/// Returns the [default entry](https://willowprotocol.org/misc-definitions/index.html#default_entry) authorised by the
/// [default authorisation token](https://willowprotocol.org/specs/confidential-sync/index.html#sync_default_authorisation_token).
pub fn default_authorised_entry() -> AuthorisedEntry {
    let entry = default_entry();
    let authorisation_token = default_authorisation_token();
    unsafe {
        PossiblyAuthorisedEntry::new(entry, authorisation_token).into_authorised_entry_unchecked()
    }
}
