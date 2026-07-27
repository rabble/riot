use core::fmt;

#[cfg(feature = "dev")]
use arbitrary::Arbitrary;

use ed25519_dalek::Signature;

use ufotofu::codec_prelude::*;

wrapper! {
    /// A secure digital signature, issued by a [`NamespaceSecret`](crate::prelude::NamespaceSecret) and verifiable with the corresponding [`NamespaceId`](crate::prelude::NamespaceId).
    ///
    /// This type is a fairly thin wrapper around [`ed25519_dalek::Signature`], use the [`From`] implementations to convert between the two.
    ///
    /// Chances are you never need to interact with this type directly, the [`meadowcap`] module should handle all authentication needs for you.
    #[derive(PartialEq, Eq, Clone)]
    NamespaceSignature; Signature
}

impl fmt::Debug for NamespaceSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<[u8; Signature::BYTE_SIZE]> for NamespaceSignature {
    fn from(value: [u8; Signature::BYTE_SIZE]) -> Self {
        Self(Signature::from(value))
    }
}

#[cfg(feature = "dev")]
impl<'a> Arbitrary<'a> for NamespaceSignature {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self(Signature::from_bytes(&Arbitrary::arbitrary(u)?)))
    }
}

/// Implements the identity function (an ed25519 signature is already a byte string).
impl Encodable for NamespaceSignature {
    async fn encode<C>(&self, consumer: &mut C) -> Result<(), C::Error>
    where
        C: BulkConsumer<Item = u8> + ?Sized,
    {
        consumer
            .consume_full_slice(&self.0.to_bytes())
            .await
            .map_err(|err| err.reason)
    }
}

/// Implements the identity function (an ed25519 signature is already a byte string).
impl EncodableKnownLength for NamespaceSignature {
    fn len_of_encoding(&self) -> usize {
        Signature::BYTE_SIZE
    }
}

/// Implements the identity function (an ed25519 signature is already a byte string).
impl Decodable for NamespaceSignature {
    type ErrorReason = Infallible;

    async fn decode<P>(
        producer: &mut P,
    ) -> Result<Self, DecodeError<P::Final, P::Error, Self::ErrorReason>>
    where
        P: BulkProducer<Item = u8> + ?Sized,
        Self: Sized,
    {
        let mut buf = [0; Signature::BYTE_SIZE];
        producer.overwrite_full_slice(&mut buf).await?;
        Ok(Self(Signature::from_bytes(&buf)))
    }
}

/// Implements the identity function (an ed25519 signature is already a byte string).
impl DecodableCanonic for NamespaceSignature {
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
