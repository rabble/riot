use core::fmt;

#[cfg(feature = "dev")]
use arbitrary::Arbitrary;

use ed25519_dalek::{SECRET_KEY_LENGTH, SigningKey};
use rand_core::CryptoRngCore;
use signature::{Keypair, Signer};

use crate::prelude::*;

wrapper! {
    /// A secret key corresponding to a [`NamespaceId`] — see the [`NamespaceId`] docs for an overview, and see the [`authentication`](crate::authorisation) docs for how to use the secret for authentication.
    ///
    /// You can randomly generate namespace secrets and their corresponding namespace ids with the [`randomly_generate_namespace`], [`randomly_generate_communal_namespace`], and [`randomly_generate_owned_namespace`] function.
    ///
    /// This type is a fairly thin wrapper around [`SigningKey`], use the [`From`] implementations to convert between the two.
    ///
    /// ```
    /// use rand::rngs::OsRng;
    /// use willow25::prelude::*;
    ///
    /// let mut csprng = OsRng; // cryptographically secure pseudo-random number generator
    ///
    /// let (namespace_id1, _secret1) = randomly_generate_communal_namespace(&mut csprng);
    /// assert!(namespace_id1.is_communal());
    ///
    /// let (namespace_id2, _secret2) = randomly_generate_owned_namespace(&mut csprng);
    /// assert!(namespace_id2.is_owned());
    /// ```
    #[derive(PartialEq, Eq, Clone)]
    NamespaceSecret; SigningKey
}

impl fmt::Debug for NamespaceSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl NamespaceSecret {
    /// Computes the [`NamespaceId`] corresponding to `self`, i.e., the [`NamespaceId`] which can be used to verify [`NamespaceSignatures`](super::NamespaceSignature) issued by `self`.
    ///
    /// ```
    /// use rand::rngs::OsRng;
    /// use willow25::prelude::*;
    ///
    /// let mut csprng = OsRng; // cryptographically secure pseudo-random number generator
    ///
    /// let (namespace_id, secret) = randomly_generate_namespace(&mut csprng);
    ///
    /// assert_eq!(secret.corresponding_namespace_id(), namespace_id);
    /// ```
    pub fn corresponding_namespace_id(&self) -> NamespaceId {
        NamespaceId::from_ed25519_verifying_key(self.0.verifying_key())
    }

    /// Converts `self` into the underlying byte array.
    ///
    /// This type deliberately does not provide this functionality through a trait, in order to make it less likely to leak any secrets.
    pub fn into_bytes(self) -> [u8; SECRET_KEY_LENGTH] {
        self.0.to_bytes()
    }

    /// Returns a reference to the underlying byte array.
    ///
    /// This type deliberately does not provide this functionality through a trait, in order to make it less likely to leak any secrets.
    pub fn as_bytes(&self) -> &[u8; SECRET_KEY_LENGTH] {
        self.0.as_bytes()
    }

    /// Returns a new [`SubspaceSecret`] constructed from a  [`SECRET_KEY_LENGTH`]-length array of bytes.
    pub fn from_bytes(bytes: &[u8; SECRET_KEY_LENGTH]) -> Self {
        Self(SigningKey::from_bytes(bytes))
    }
}

#[cfg(feature = "dev")]
impl<'a> Arbitrary<'a> for NamespaceSecret {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self(SigningKey::from_bytes(&Arbitrary::arbitrary(u)?)))
    }
}

/// Randomly generate a [`NamespaceSecret`] and the corresponding [`NamespaceId`].
///
/// See also [`randomly_generate_communal_namespace`] and [`randomly_generate_owned_namespace`] if you need control over whether to generate an [owned namespace](https://willowprotocol.org/specs/meadowcap/#owned_namespace) or a [communal namespace](https://willowprotocol.org/specs/meadowcap/#communal_namespace).
///
/// ```
/// use rand::rngs::OsRng;
/// use willow25::prelude::*;
///
/// let mut csprng = OsRng; // cryptographically secure pseudo-random number generator
///
/// let (_namespace_id, _secret) = randomly_generate_namespace(&mut csprng);
/// ```
pub fn randomly_generate_namespace<R: CryptoRngCore + ?Sized>(
    csprng: &mut R,
) -> (NamespaceId, NamespaceSecret) {
    let signing_key = SigningKey::generate(csprng);
    let verifying_key = signing_key.verifying_key();

    (
        NamespaceId::from_ed25519_verifying_key(verifying_key),
        NamespaceSecret(signing_key),
    )
}

/// Randomly generate a [`NamespaceSecret`] for a [communal namespace](https://willowprotocol.org/specs/meadowcap/#communal_namespace) and the corresponding [`NamespaceId`].
///
/// ```
/// use rand::rngs::OsRng;
/// use willow25::prelude::*;
///
/// let mut csprng = OsRng; // cryptographically secure pseudo-random number generator
///
/// let (namespace_id, _secret) = randomly_generate_communal_namespace(&mut csprng);
/// assert!(namespace_id.is_communal());
/// ```
pub fn randomly_generate_communal_namespace<R: CryptoRngCore + ?Sized>(
    csprng: &mut R,
) -> (NamespaceId, NamespaceSecret) {
    loop {
        let (pk, sk) = randomly_generate_namespace(csprng);

        if pk.is_communal() {
            return (pk, sk);
        }
    }
}

/// Randomly generate a [`NamespaceSecret`] for an [owned namespace](https://willowprotocol.org/specs/meadowcap/#owned_namespace) and the corresponding [`NamespaceId`].
///
/// ```
/// use rand::rngs::OsRng;
/// use willow25::prelude::*;
///
/// let mut csprng = OsRng; // cryptographically secure pseudo-random number generator
///
/// let (namespace_id, _secret) = randomly_generate_owned_namespace(&mut csprng);
/// assert!(namespace_id.is_owned());
/// ```
pub fn randomly_generate_owned_namespace<R: CryptoRngCore + ?Sized>(
    csprng: &mut R,
) -> (NamespaceId, NamespaceSecret) {
    loop {
        let (pk, sk) = randomly_generate_namespace(csprng);

        if !pk.is_communal() {
            return (pk, sk);
        }
    }
}

impl Keypair for NamespaceSecret {
    type VerifyingKey = NamespaceId;

    fn verifying_key(&self) -> Self::VerifyingKey {
        self.corresponding_namespace_id()
    }
}

impl Signer<NamespaceSignature> for NamespaceSecret {
    fn try_sign(&self, msg: &[u8]) -> Result<NamespaceSignature, signature::Error> {
        self.0.try_sign(msg).map(Into::into)
    }
}
