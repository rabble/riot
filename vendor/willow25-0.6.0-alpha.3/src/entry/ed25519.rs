//! Internal functionality around ed25519. Wish this module didn't have to exist, but, oh well.

use core::hash::Hash;

#[cfg(feature = "dev")]
use arbitrary::Arbitrary;

use ed25519_dalek::VerifyingKey;

use ufotofu::codec_prelude::*;

use order_theory::{
    GreatestElement, LeastElement, LowerSemilattice, PredecessorExceptForLeast,
    SuccessorExceptForGreatest, TryPredecessor, TrySuccessor, UpperSemilattice,
};

/// Either a proper Verifying Key, or just an array of bytes if those bytes do not properly decode to a curve point.
///
/// Willow25 allows arbitrary byte arrays to be used as namespce ids and subspace ids, yet we want to verify signatures with them. This type is the compromise: upon creation, it checks whether the bytes decode to a proper verifying key. If so, it is stored in the `Key` variant of this enum, and can be used for signature verification. Otherwise, the `Dummy` variant stores the raw bytes, and always fails signature verification.
#[derive(Clone, Debug)]
pub(crate) enum VerifyingKeyOrDummy {
    Key(VerifyingKey),
    Dummy([u8; 32]),
}

#[cfg(feature = "dev")]
impl<'a> Arbitrary<'a> for VerifyingKeyOrDummy {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let bytes: [u8; 32] = Arbitrary::arbitrary(u)?;

        Ok(Self::from_bytes(&bytes))
    }
}

impl PartialEq for VerifyingKeyOrDummy {
    fn eq(&self, other: &Self) -> bool {
        let self_bytes = self.as_bytes();
        let other_bytes = other.as_bytes();

        constant_time_eq::constant_time_eq_n(self_bytes, other_bytes)
    }
}

impl Eq for VerifyingKeyOrDummy {}

impl Hash for VerifyingKeyOrDummy {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.as_bytes().hash(state)
    }
}

impl PartialOrd for VerifyingKeyOrDummy {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for VerifyingKeyOrDummy {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.as_bytes().cmp(other.as_bytes())
    }
}

impl VerifyingKeyOrDummy {
    /// Returns a new [`VerifyingKeyOrDummy`] corresponding to the supplied bytes, or `None` if these bytes are [not valid](https://willowprotocol.org/specs/willow25/index.html#willow25_crypto).
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        match VerifyingKey::from_bytes(bytes) {
            Ok(key) => Self::Key(key),
            Err(_) => Self::Dummy(*bytes),
        }
    }

    /// Returns a reference to the raw bytes that make up this namespace id.
    ///
    /// This type deliberately does not provide implementations of [`AsRef`], [`Deref`](core::ops::Deref) or [`Borrow`](core::borrow::Borrow), to make it more difficult to leak information accidentally. Apologies if this is inconvenient.
    pub fn as_bytes(&self) -> &[u8; 32] {
        match self {
            VerifyingKeyOrDummy::Dummy(bytes) => bytes,
            VerifyingKeyOrDummy::Key(key) => key.as_bytes(),
        }
    }

    /// Returns the raw bytes that make up this namespace id.
    ///
    /// This type deliberately does not provide implementations of [`Into`], to make it more difficult to leak information accidentally. Apologies if this is inconvenient.
    pub fn to_bytes(&self) -> [u8; 32] {
        match self {
            VerifyingKeyOrDummy::Dummy(bytes) => *bytes,
            VerifyingKeyOrDummy::Key(key) => key.to_bytes(),
        }
    }
}

impl LeastElement for VerifyingKeyOrDummy {
    fn least() -> Self {
        Self::from_bytes(&<[u8; 32]>::least())
    }
}

impl GreatestElement for VerifyingKeyOrDummy {
    fn greatest() -> Self {
        Self::from_bytes(&<[u8; 32]>::greatest())
    }
}

impl LowerSemilattice for VerifyingKeyOrDummy {
    fn greatest_lower_bound(&self, other: &Self) -> Self {
        Self::from_bytes(&self.as_bytes().greatest_lower_bound(other.as_bytes()))
    }
}

impl UpperSemilattice for VerifyingKeyOrDummy {
    fn least_upper_bound(&self, other: &Self) -> Self {
        Self::from_bytes(&self.as_bytes().least_upper_bound(other.as_bytes()))
    }
}

impl TryPredecessor for VerifyingKeyOrDummy {
    fn try_predecessor(&self) -> Option<Self> {
        self.as_bytes()
            .try_predecessor()
            .map(|bytes| Self::from_bytes(&bytes))
    }
}

impl TrySuccessor for VerifyingKeyOrDummy {
    fn try_successor(&self) -> Option<Self> {
        self.as_bytes()
            .try_successor()
            .map(|bytes| Self::from_bytes(&bytes))
    }
}

impl PredecessorExceptForLeast for VerifyingKeyOrDummy {}

impl SuccessorExceptForGreatest for VerifyingKeyOrDummy {}

impl Encodable for VerifyingKeyOrDummy {
    async fn encode<C>(&self, consumer: &mut C) -> Result<(), C::Error>
    where
        C: BulkConsumer<Item = u8> + ?Sized,
    {
        consumer
            .consume_full_slice(self.as_bytes())
            .await
            .map_err(|err| err.into_reason())
    }
}

impl EncodableKnownLength for VerifyingKeyOrDummy {
    fn len_of_encoding(&self) -> usize {
        32
    }
}

impl Decodable for VerifyingKeyOrDummy {
    type ErrorReason = Infallible;

    async fn decode<P>(
        producer: &mut P,
    ) -> Result<Self, DecodeError<P::Final, P::Error, Self::ErrorReason>>
    where
        P: BulkProducer<Item = u8> + ?Sized,
        Self: Sized,
    {
        let mut buf = [0; 32];
        producer.overwrite_full_slice(&mut buf).await?;
        Ok(Self::from_bytes(&buf))
    }
}

impl DecodableCanonic for VerifyingKeyOrDummy {
    type ErrorCanonic = Infallible;

    async fn decode_canonic<P>(
        producer: &mut P,
    ) -> Result<Self, DecodeError<P::Final, P::Error, Self::ErrorCanonic>>
    where
        P: BulkProducer<Item = u8> + ?Sized,
        Self: Sized,
    {
        Self::decode(producer).await
    }
}
