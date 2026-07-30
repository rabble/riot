use core::fmt;

#[cfg(feature = "dev")]
use arbitrary::Arbitrary;

use ed25519_dalek::{SECRET_KEY_LENGTH, SigningKey};
use rand_core::CryptoRngCore;
use signature::{Keypair, Signer};

use crate::prelude::*;

wrapper! {
    /// A secret key corresponding to a [`SubspaceId`] — see the [`SubspaceId`] docs for an overview, and see the [`authentication`](crate::authorisation) docs for how to use the secret for authentication.
    ///
    /// You can randomly generate subspace secrets and their corresponding subspace ids with the [`randomly_generate_subspace`] function.
    ///
    /// This type is a fairly thin wrapper around [`SigningKey`], use the [`From`] implementations to convert between the two.
    ///
    /// ```
    /// use rand::rngs::OsRng;
    /// use willow25::prelude::*;
    ///
    /// let mut csprng = OsRng; // cryptographically secure pseudo-random number generator
    ///
    /// let (_subspace_id, _secret) = randomly_generate_subspace(&mut csprng);
    /// ```
    #[derive(PartialEq, Eq, Clone)]
    SubspaceSecret; SigningKey
}

impl fmt::Debug for SubspaceSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl SubspaceSecret {
    /// Computes the [`SubspaceId`] corresponding to `self`, i.e., the [`SubspaceId`] which can be used to verify [`SubspaceSignatures`](super::SubspaceSignature) issued by `self`.
    ///
    /// ```
    /// use rand::rngs::OsRng;
    /// use willow25::prelude::*;
    ///
    /// let mut csprng = OsRng; // cryptographically secure pseudo-random number generator
    ///
    /// let (subspace_id, secret) = randomly_generate_subspace(&mut csprng);
    ///
    /// assert_eq!(secret.corresponding_subspace_id(), subspace_id);
    /// ```
    pub fn corresponding_subspace_id(&self) -> SubspaceId {
        SubspaceId::from_ed25519_verifying_key(self.0.verifying_key())
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
impl<'a> Arbitrary<'a> for SubspaceSecret {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self(SigningKey::from_bytes(&Arbitrary::arbitrary(u)?)))
    }
}

/// Randomly generate a [`SubspaceSecret`] and the corresponding [`SubspaceId`].
///
/// ```
/// use rand::rngs::OsRng;
/// use willow25::prelude::*;
///
/// let mut csprng = OsRng; // cryptographically secure pseudo-random number generator
///
/// let (_subspace_id, _secret) = randomly_generate_subspace(&mut csprng);
/// ```
pub fn randomly_generate_subspace<R: CryptoRngCore + ?Sized>(
    csprng: &mut R,
) -> (SubspaceId, SubspaceSecret) {
    let signing_key = SigningKey::generate(csprng);
    let verifying_key = signing_key.verifying_key();

    (
        SubspaceId::from_ed25519_verifying_key(verifying_key),
        SubspaceSecret(signing_key),
    )
}

impl Keypair for SubspaceSecret {
    type VerifyingKey = SubspaceId;

    fn verifying_key(&self) -> Self::VerifyingKey {
        self.corresponding_subspace_id()
    }
}

impl Signer<SubspaceSignature> for SubspaceSecret {
    fn try_sign(&self, msg: &[u8]) -> Result<SubspaceSignature, signature::Error> {
        self.0.try_sign(msg).map(Into::into)
    }
}
