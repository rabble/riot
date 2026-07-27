#![allow(clippy::module_inception)]

use core::fmt;

#[cfg(feature = "dev")]
use arbitrary::Arbitrary;

use ed25519_dalek::VerifyingKey;
use signature::Verifier;
use ufotofu::codec_prelude::*;

use order_theory::{
    GreatestElement, LeastElement, LowerSemilattice, PredecessorExceptForLeast,
    SuccessorExceptForGreatest, TryPredecessor, TrySuccessor, UpperSemilattice,
};

use crate::entry::HexFormatter;
use crate::entry::ed25519::VerifyingKeyOrDummy;
use crate::prelude::*;

/// The width of a [`SubspaceId`] in bytes: 32.
///
/// [Specification](https://willowprotocol.org/specs/willow25/index.html#willow25_data_model)
pub const SUBSPACE_ID_WIDTH: usize = 32;

/// The type of [SubspaceIds](https://willowprotocol.org/specs/data-model/index.html#SubspaceId) used by [Willow’25](https://willowprotocol.org/specs/willow25/index.html#willow25_data_model).
///
/// Subspaces group Willow data within the same [namespace](`NamespaceId`) into independent collections — entries with nonequal subspace ids cannot overwrite each other. Subspaces further play a role in access control for Willow: subspace ids are public keys of a [digital signature system](https://ed25519.cr.yp.to/), and knowledge of the corresponding secret key allow you to issue write entries to a subspace, and to delegate access to other users — see the [`meadowcap`] module for more details.
///
/// That all said, you can also simply think of subspace ids as glorified `[u8; 32]` arrays. That perspective should be sufficient as long as you do not need to care about cryptographic details.
///
/// You will typically obtain a subspace id either by [randomly generating a key pair](super::randomly_generate_subspace), by cloning an existing subspace id, or by creating it from raw bytes via [`SubspaceId::from_bytes`]. See the [Willow25 specification](https://willowprotocol.org/specs/willow25/index.html#willow25_crypto) if you want to work with raw bytes — while every `[u8; 32]` can be converted into a [`SubspaceId`], some of these do not correspond to canonic public key encodings and thus always fail signature verification.
///
/// ```
/// use rand::rngs::OsRng;
/// use willow25::prelude::*;
///
/// let mut csprng = OsRng; // cryptographically secure pseudo-random number generator
/// let (_subspace_id, _secret) = randomly_generate_subspace(&mut csprng);
///
/// let subspace_id2 = SubspaceId::from_bytes(&[17; SUBSPACE_ID_WIDTH]);
/// assert_eq!(subspace_id2.as_bytes(), &[17; 32]);
/// ```
#[derive(Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "dev", derive(Arbitrary))]
pub struct SubspaceId(VerifyingKeyOrDummy);

/// Implemented lexicographically on the compressed Edwards y coordinate encoding.
impl PartialOrd for SubspaceId {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Implemented lexicographically on the compressed Edwards y coordinate encoding.
impl Ord for SubspaceId {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.0.as_bytes().cmp(other.0.as_bytes())
    }
}

impl fmt::Debug for SubspaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SubspaceId")
            .field(&HexFormatter(self.to_bytes()))
            .finish()
    }
}

impl SubspaceId {
    /// Returns a new [`SubspaceId`] corresponding to the supplied bytes.
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let subspace_id = SubspaceId::from_bytes(&[17; SUBSPACE_ID_WIDTH]);
    /// assert_eq!(subspace_id.as_bytes(), &[17; SUBSPACE_ID_WIDTH]);
    /// ```
    pub fn from_bytes(bytes: &[u8; SUBSPACE_ID_WIDTH]) -> Self {
        Self(VerifyingKeyOrDummy::from_bytes(bytes))
    }

    /// Returns a reference to the raw bytes that make up this subspace id.
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let subspace_id = SubspaceId::from_bytes(&[17; SUBSPACE_ID_WIDTH]);
    /// assert_eq!(subspace_id.as_bytes(), &[17; SUBSPACE_ID_WIDTH]);
    /// ```
    pub fn as_bytes(&self) -> &[u8; SUBSPACE_ID_WIDTH] {
        self.0.as_bytes()
    }

    /// Returns the raw bytes that make up this subspace id.
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let subspace_id = SubspaceId::from_bytes(&[17; SUBSPACE_ID_WIDTH]);
    /// assert_eq!(subspace_id.to_bytes(), [17; SUBSPACE_ID_WIDTH]);
    /// ```
    pub fn to_bytes(&self) -> [u8; SUBSPACE_ID_WIDTH] {
        self.0.to_bytes()
    }

    /// Creates a subspace id from an ed25519 verifying key.
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let from_key = SubspaceId::from_ed25519_verifying_key(
    ///     ed25519_dalek::VerifyingKey::from_bytes(&[17; 32]).unwrap(),
    /// );
    /// let from_bytes = SubspaceId::from_bytes(&[17; SUBSPACE_ID_WIDTH]);
    ///
    /// assert_eq!(from_key, from_bytes);
    /// ```
    pub fn from_ed25519_verifying_key(verifying_key: VerifyingKey) -> Self {
        Self(VerifyingKeyOrDummy::Key(verifying_key))
    }
}

impl From<[u8; SUBSPACE_ID_WIDTH]> for SubspaceId {
    fn from(value: [u8; SUBSPACE_ID_WIDTH]) -> Self {
        Self::from_bytes(&value)
    }
}

impl LeastElement for SubspaceId {
    fn least() -> Self {
        Self(VerifyingKeyOrDummy::least())
    }
}

impl GreatestElement for SubspaceId {
    fn greatest() -> Self {
        Self(VerifyingKeyOrDummy::greatest())
    }
}

impl LowerSemilattice for SubspaceId {
    fn greatest_lower_bound(&self, other: &Self) -> Self {
        Self(self.0.greatest_lower_bound(&other.0))
    }
}

impl UpperSemilattice for SubspaceId {
    fn least_upper_bound(&self, other: &Self) -> Self {
        Self(self.0.least_upper_bound(&other.0))
    }
}

impl TryPredecessor for SubspaceId {
    fn try_predecessor(&self) -> Option<Self> {
        self.0.try_predecessor().map(Self)
    }
}

impl TrySuccessor for SubspaceId {
    fn try_successor(&self) -> Option<Self> {
        self.0.try_successor().map(Self)
    }
}

impl PredecessorExceptForLeast for SubspaceId {}

impl SuccessorExceptForGreatest for SubspaceId {}

impl Encodable for SubspaceId {
    async fn encode<C>(&self, consumer: &mut C) -> Result<(), C::Error>
    where
        C: BulkConsumer<Item = u8> + ?Sized,
    {
        self.0.encode(consumer).await
    }
}

impl EncodableKnownLength for SubspaceId {
    fn len_of_encoding(&self) -> usize {
        self.0.len_of_encoding()
    }
}

impl Decodable for SubspaceId {
    type ErrorReason = Infallible;

    async fn decode<P>(
        producer: &mut P,
    ) -> Result<Self, DecodeError<P::Final, P::Error, Self::ErrorReason>>
    where
        P: BulkProducer<Item = u8> + ?Sized,
        Self: Sized,
    {
        Ok(Self(VerifyingKeyOrDummy::decode(producer).await?))
    }
}

impl DecodableCanonic for SubspaceId {
    type ErrorCanonic = Infallible;

    async fn decode_canonic<P>(
        producer: &mut P,
    ) -> Result<Self, DecodeError<P::Final, P::Error, Self::ErrorCanonic>>
    where
        P: BulkProducer<Item = u8> + ?Sized,
        Self: Sized,
    {
        Ok(Self(VerifyingKeyOrDummy::decode_canonic(producer).await?))
    }
}

impl Verifier<SubspaceSignature> for SubspaceId {
    fn verify(&self, msg: &[u8], signature: &SubspaceSignature) -> Result<(), signature::Error> {
        match self.0 {
            VerifyingKeyOrDummy::Dummy(_) => Err(signature::Error::new()),
            VerifyingKeyOrDummy::Key(sk) => sk.verify_strict(msg, signature.into()),
        }
    }
}
