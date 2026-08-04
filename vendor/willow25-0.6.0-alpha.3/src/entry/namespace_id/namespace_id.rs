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

use crate::entry::ed25519::VerifyingKeyOrDummy;
use crate::prelude::*;

/// The width of a [`NamespaceId`] in bytes: 32.
///
/// [Specification](https://willowprotocol.org/specs/willow25/index.html#willow25_data_model)
pub const NAMESPACE_ID_WIDTH: usize = 32;

/// The type of [NamespaceIds](https://willowprotocol.org/specs/data-model/index.html#NamespaceId) used by [Willow’25](https://willowprotocol.org/specs/willow25/index.html#willow25_data_model).
///
/// Namespaces serve to group Willow data into independent universes — entries with nonequal namespace ids cannot overwrite each other. Namespaces further anchor the system of read and write access in Willow: namespace ids are public keys of a [digital signature system](https://ed25519.cr.yp.to/), and knowledge of the corresponding secret key allow you to issue [read and write capabilities](https://willowprotocol.org/specs/meadowcap/) for the namespace.
///
/// More precisely, Willow25 provides two different kinds of namespaces: [owned namespaces](https://willowprotocol.org/specs/meadowcap/#owned_namespace) and [communal namespaces](https://willowprotocol.org/specs/meadowcap/#communal_namespace). In an owned namespace, you need the corresponding secret key to issue initial read or write capabilities, and these then give access to the full namespace. In a communal namespace, the secret key grants no special privileges: everyone can create read and write capabilities, but these are only scoped to individual [subspaces](https://willowprotocol.org/specs/data-model/index.html#subspace) for which the creator knows the [`SubspaceSecret`](crate::prelude::SubspaceSecret). See the [`meadowcap`] module for more details on how access control works and how to refine it to more granular scopes.
///
/// That all said, you can also simply think of namespace ids as glorified `[u8; 32]` arrays. That perspective should be sufficient as long as you do not need to care about cryptographic details.
///
/// You will typically obtain a namespace id either by [randomly generating a key pair](super::randomly_generate_namespace), by cloning an existing namespace id, or by creating it from raw bytes via [`NamespaceId::from_bytes`]. See the [Willow25 specification](https://willowprotocol.org/specs/willow25/index.html#willow25_crypto) if you want to work with raw bytes — while every `[u8; 32]` can be converted into a [`NamespaceId`], some of these do not correspond to canonic public key encodings and thus always fail signature verification.
///
/// ```
/// use rand::rngs::OsRng;
/// use willow25::prelude::*;
///
/// let mut csprng = OsRng; // cryptographically secure pseudo-random number generator
/// let (namespace_id, _secret) = randomly_generate_communal_namespace(&mut csprng);
/// assert!(namespace_id.is_communal());
///
/// let namespace_id2 = NamespaceId::from_bytes(&[17; NAMESPACE_ID_WIDTH]);
/// assert!(namespace_id2.is_owned()); // Least-significant bit is one.
/// assert_eq!(namespace_id2.as_bytes(), &[17; 32]);
/// ```
#[derive(Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "dev", derive(Arbitrary))]
pub struct NamespaceId(VerifyingKeyOrDummy);

/// Implemented lexicographically on the compressed Edwards y coordinate encoding.
impl PartialOrd for NamespaceId {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Implemented lexicographically on the compressed Edwards y coordinate encoding.
impl Ord for NamespaceId {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.0.as_bytes().cmp(other.0.as_bytes())
    }
}

impl fmt::Debug for NamespaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("NamespaceId")
            .field(&NamespaceIdHexFormatter(self.0.to_bytes()))
            .finish()
    }
}

impl NamespaceId {
    /// Returns whether this namespace id defines a [communal namespace](https://willowprotocol.org/specs/meadowcap/index.html#is_communal) or not.
    ///
    /// A namespace id is communal iff its least significant bit is zero.
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let namespace_id1 = NamespaceId::from_bytes(&[16; NAMESPACE_ID_WIDTH]);
    /// assert!(namespace_id1.is_communal());
    ///
    /// let namespace_id2 = NamespaceId::from_bytes(&[17; NAMESPACE_ID_WIDTH]);
    /// assert!(!namespace_id2.is_communal());
    /// ```
    pub fn is_communal(&self) -> bool {
        self.as_bytes()[NAMESPACE_ID_WIDTH - 1].is_multiple_of(2)
    }

    /// Returns whether this namespace id defines an [owned namespace](https://willowprotocol.org/specs/meadowcap/index.html#is_communal) or not.
    ///
    /// A namespace id is owned iff its least significant bit is one.
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let namespace_id1 = NamespaceId::from_bytes(&[17; NAMESPACE_ID_WIDTH]);
    /// assert!(namespace_id1.is_owned());
    ///
    /// let namespace_id2 = NamespaceId::from_bytes(&[16; NAMESPACE_ID_WIDTH]);
    /// assert!(!namespace_id2.is_owned());
    /// ```
    pub fn is_owned(&self) -> bool {
        !self.is_communal()
    }

    /// Returns a new [`NamespaceId`] corresponding to the supplied bytes.
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let namespace_id = NamespaceId::from_bytes(&[17; NAMESPACE_ID_WIDTH]);
    /// assert_eq!(namespace_id.as_bytes(), &[17; NAMESPACE_ID_WIDTH]);
    /// ```
    pub fn from_bytes(bytes: &[u8; NAMESPACE_ID_WIDTH]) -> Self {
        Self(VerifyingKeyOrDummy::from_bytes(bytes))
    }

    /// Returns a reference to the raw bytes that make up this namespace id.
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let namespace_id = NamespaceId::from_bytes(&[17; NAMESPACE_ID_WIDTH]);
    /// assert_eq!(namespace_id.as_bytes(), &[17; NAMESPACE_ID_WIDTH]);
    /// ```
    pub fn as_bytes(&self) -> &[u8; NAMESPACE_ID_WIDTH] {
        self.0.as_bytes()
    }

    /// Returns the raw bytes that make up this namespace id.
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let namespace_id = NamespaceId::from_bytes(&[17; NAMESPACE_ID_WIDTH]);
    /// assert_eq!(namespace_id.to_bytes(), [17; NAMESPACE_ID_WIDTH]);
    /// ```
    pub fn to_bytes(&self) -> [u8; NAMESPACE_ID_WIDTH] {
        self.0.to_bytes()
    }

    /// Creates a namespace id from an ed25519 verifying key.
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let from_key = NamespaceId::from_ed25519_verifying_key(
    ///     ed25519_dalek::VerifyingKey::from_bytes(&[17; 32]).unwrap(),
    /// );
    /// let from_bytes = NamespaceId::from_bytes(&[17; NAMESPACE_ID_WIDTH]);
    ///
    /// assert_eq!(from_key, from_bytes);
    /// ```
    pub fn from_ed25519_verifying_key(verifying_key: VerifyingKey) -> Self {
        Self(VerifyingKeyOrDummy::Key(verifying_key))
    }
}

impl From<[u8; NAMESPACE_ID_WIDTH]> for NamespaceId {
    fn from(value: [u8; NAMESPACE_ID_WIDTH]) -> Self {
        Self::from_bytes(&value)
    }
}

impl LeastElement for NamespaceId {
    fn least() -> Self {
        Self(VerifyingKeyOrDummy::least())
    }
}

impl GreatestElement for NamespaceId {
    fn greatest() -> Self {
        Self(VerifyingKeyOrDummy::greatest())
    }
}

impl LowerSemilattice for NamespaceId {
    fn greatest_lower_bound(&self, other: &Self) -> Self {
        Self(self.0.greatest_lower_bound(&other.0))
    }
}

impl UpperSemilattice for NamespaceId {
    fn least_upper_bound(&self, other: &Self) -> Self {
        Self(self.0.least_upper_bound(&other.0))
    }
}

impl TryPredecessor for NamespaceId {
    fn try_predecessor(&self) -> Option<Self> {
        self.0.try_predecessor().map(Self)
    }
}

impl TrySuccessor for NamespaceId {
    fn try_successor(&self) -> Option<Self> {
        self.0.try_successor().map(Self)
    }
}

impl PredecessorExceptForLeast for NamespaceId {}

impl SuccessorExceptForGreatest for NamespaceId {}

struct NamespaceIdHexFormatter<const N: usize>([u8; N]);

impl<const N: usize> fmt::Debug for NamespaceIdHexFormatter<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // First render a `+` if the namespace is communal, and a `-` if it is not.
        if self.0[31] & 0b0000_0001 == 0 {
            f.write_str("+")?;
        } else {
            f.write_str("-")?;
        }

        // Then render the bytes as lowercase hex.
        let mut buffer = const_hex::Buffer::<N>::new();
        let printed = buffer.format(&self.0);
        f.write_str(printed)
    }
}

impl Encodable for NamespaceId {
    async fn encode<C>(&self, consumer: &mut C) -> Result<(), C::Error>
    where
        C: BulkConsumer<Item = u8> + ?Sized,
    {
        self.0.encode(consumer).await
    }
}

impl EncodableKnownLength for NamespaceId {
    fn len_of_encoding(&self) -> usize {
        self.0.len_of_encoding()
    }
}

impl Decodable for NamespaceId {
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

impl DecodableCanonic for NamespaceId {
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

impl Verifier<NamespaceSignature> for NamespaceId {
    fn verify(&self, msg: &[u8], signature: &NamespaceSignature) -> Result<(), signature::Error> {
        match self.0 {
            VerifyingKeyOrDummy::Dummy(_) => Err(signature::Error::new()),
            VerifyingKeyOrDummy::Key(sk) => sk.verify_strict(msg, signature.into()),
        }
    }
}
