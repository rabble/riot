use core::fmt;

use anyhash::{Hasher, HasherWrite};

#[cfg(feature = "dev")]
use arbitrary::Arbitrary;

use ufotofu::codec_prelude::*;

use bab_rs::{William3Digest, William3Hasher};
use order_theory::{
    GreatestElement, LeastElement, LowerSemilattice, PredecessorExceptForLeast,
    SuccessorExceptForGreatest, TryPredecessor, TrySuccessor, UpperSemilattice,
};

use super::HexFormatter;

/// The width of a [`PayloadDigest`] in bytes: 32.
///
/// [Specification](https://willowprotocol.org/specs/willow25/index.html#willow25_data_model)
pub const PAYLOAD_DIGEST_WIDTH: usize = bab_rs::WIDTH;

wrapper! {
    /// The type of [PayloadDigests](https://willowprotocol.org/specs/data-model/index.html#PayloadDigest) used by [Willow’25](https://willowprotocol.org/specs/willow25/index.html#willow25_data_model).
    ///
    /// This is a thin wrapper around [`William3Digest`].
    #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    #[cfg_attr(feature = "dev", derive(Arbitrary))]
    PayloadDigest; William3Digest
}

impl fmt::Debug for PayloadDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("PayloadDigest")
            .field(&HexFormatter(self.0.clone().into_bytes()))
            .finish()
    }
}

impl From<[u8; PAYLOAD_DIGEST_WIDTH]> for PayloadDigest {
    fn from(value: [u8; PAYLOAD_DIGEST_WIDTH]) -> Self {
        Self(value.into())
    }
}

impl PayloadDigest {
    /// Hashes a bytestring into the corrsponding [`PayloadDigest`].
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// assert_eq!(
    ///     PayloadDigest::from_payload(b"See all the sights!"),
    ///     [
    ///         91, 29, 243, 211, 140, 181, 127, 138,
    ///         116, 79, 56, 47, 85, 50, 157, 82,
    ///         55, 192, 253, 122, 72, 250, 70, 43,
    ///         56, 116, 99, 53, 188, 85, 83, 234,
    ///     ].into(),
    /// );
    /// ```
    pub fn from_payload<Payload: AsRef<[u8]>>(payload: Payload) -> Self {
        let mut hasher = William3Hasher::new();
        hasher.write(payload.as_ref());

        hasher.finish().into()
    }

    /// Converts `self` into the underlying byte array.
    ///
    /// This type deliberately does not provide this functionality through a trait, in order to make it less likely to leak any secrets.
    pub fn into_bytes(self) -> [u8; PAYLOAD_DIGEST_WIDTH] {
        self.0.into_bytes()
    }

    /// Returns a reference to the underlying byte array.
    ///
    /// This type deliberately does not provide this functionality through a trait, in order to make it less likely to leak any secrets.
    pub fn as_bytes(&self) -> &[u8; PAYLOAD_DIGEST_WIDTH] {
        self.0.as_bytes()
    }

    /// Returns a mutable reference to the underlying byte array.
    ///
    /// This type deliberately does not provide this functionality through a trait, in order to make it less likely to leak any secrets.
    pub fn as_mut_bytes(&mut self) -> &mut [u8; PAYLOAD_DIGEST_WIDTH] {
        self.0.as_mut_bytes()
    }
}

impl LeastElement for PayloadDigest {
    fn least() -> Self {
        William3Digest::least().into()
    }
}

impl GreatestElement for PayloadDigest {
    fn greatest() -> Self {
        William3Digest::greatest().into()
    }
}

impl LowerSemilattice for PayloadDigest {
    fn greatest_lower_bound(&self, other: &Self) -> Self {
        self.0.greatest_lower_bound(other.into()).into()
    }
}

impl UpperSemilattice for PayloadDigest {
    fn least_upper_bound(&self, other: &Self) -> Self {
        self.0.least_upper_bound(other.into()).into()
    }
}

impl TryPredecessor for PayloadDigest {
    fn try_predecessor(&self) -> Option<Self> {
        self.0.try_predecessor().map(Self)
    }
}

impl TrySuccessor for PayloadDigest {
    fn try_successor(&self) -> Option<Self> {
        self.0.try_successor().map(Self)
    }
}

impl PredecessorExceptForLeast for PayloadDigest {}

impl SuccessorExceptForGreatest for PayloadDigest {}

impl Encodable for PayloadDigest {
    async fn encode<C>(&self, consumer: &mut C) -> Result<(), C::Error>
    where
        C: BulkConsumer<Item = u8> + ?Sized,
    {
        consumer
            .consume_full_slice(self.0.as_bytes())
            .await
            .map_err(|err| err.into_reason())
    }
}

impl EncodableKnownLength for PayloadDigest {
    fn len_of_encoding(&self) -> usize {
        PAYLOAD_DIGEST_WIDTH
    }
}

impl Decodable for PayloadDigest {
    type ErrorReason = Infallible;

    async fn decode<P>(
        producer: &mut P,
    ) -> Result<Self, DecodeError<P::Final, P::Error, Self::ErrorReason>>
    where
        P: BulkProducer<Item = u8> + ?Sized,
        Self: Sized,
    {
        let mut buf = [0; PAYLOAD_DIGEST_WIDTH];
        producer.overwrite_full_slice(&mut buf).await?;
        Ok(Self(buf.into()))
    }
}

impl DecodableCanonic for PayloadDigest {
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
