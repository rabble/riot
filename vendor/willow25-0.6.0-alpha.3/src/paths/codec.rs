use ufotofu::codec_prelude::*;

use super::*;

/// Implements [encode_path](https://willowprotocol.org/specs/encodings/index.html#encode_path).
impl Encodable for Path {
    async fn encode<C>(&self, consumer: &mut C) -> Result<(), C::Error>
    where
        C: BulkConsumer<Item = u8> + ?Sized,
    {
        self.0.encode(consumer).await
    }
}

/// Implements [EncodePath](https://willowprotocol.org/specs/encodings/index.html#EncodePath).
impl Decodable for Path {
    type ErrorReason = Blame;

    async fn decode<P>(
        producer: &mut P,
    ) -> Result<Self, DecodeError<P::Final, P::Error, Self::ErrorReason>>
    where
        P: BulkProducer<Item = u8> + ?Sized,
    {
        wdm::Path::<MCL, MCC, MPL>::decode(producer)
            .await
            .map(Into::into)
    }
}

/// Implements [encode_path](https://willowprotocol.org/specs/encodings/index.html#encode_path).
impl DecodableCanonic for Path {
    type ErrorCanonic = Blame;

    async fn decode_canonic<P>(
        producer: &mut P,
    ) -> Result<Self, DecodeError<P::Final, P::Error, Self::ErrorCanonic>>
    where
        P: BulkProducer<Item = u8> + ?Sized,
    {
        wdm::Path::<MCL, MCC, MPL>::decode_canonic(producer)
            .await
            .map(Into::into)
    }
}

/// Implements [encode_path](https://willowprotocol.org/specs/encodings/index.html#encode_path).
impl EncodableKnownLength for Path {
    fn len_of_encoding(&self) -> usize {
        self.0.len_of_encoding()
    }
}

////////////////////////////////////
// Relative encoding path <> path //
////////////////////////////////////

/// Implements [path_rel_path](https://willowprotocol.org/specs/encodings/index.html#path_rel_path).
impl RelativeEncodable<Path> for Path {
    async fn relative_encode<Consumer>(
        &self,
        rel: &Path,
        consumer: &mut Consumer,
    ) -> Result<(), Consumer::Error>
    where
        Consumer: BulkConsumer<Item = u8> + ?Sized,
    {
        self.0
            .relative_encode(
                <&willow_data_model::paths::Path<MCL, MCC, MPL>>::from(rel),
                consumer,
            )
            .await
    }

    /// Any path can be encoded relative to every path.
    fn can_be_encoded_relative_to(&self, rel: &Path) -> bool {
        self.0
            .can_be_encoded_relative_to(<&willow_data_model::paths::Path<MCL, MCC, MPL>>::from(rel))
    }
}

/// Implements [EncodePathRelativePath](https://willowprotocol.org/specs/encodings/index.html#EncodePathRelativePath).
impl RelativeDecodable<Path> for Path {
    type ErrorReason = Blame;

    async fn relative_decode<P>(
        rel: &Path,
        producer: &mut P,
    ) -> Result<Self, DecodeError<P::Final, P::Error, Blame>>
    where
        P: BulkProducer<Item = u8> + ?Sized,
    {
        wdm::Path::<MCL, MCC, MPL>::relative_decode(
            <&willow_data_model::paths::Path<MCL, MCC, MPL>>::from(rel),
            producer,
        )
        .await
        .map(Into::into)
    }
}

/// Implements [path_relative_path](https://willowprotocol.org/specs/encodings/index.html#path_rel_path).
impl RelativeDecodableCanonic<Path> for Path {
    type ErrorCanonic = Blame;

    async fn relative_decode_canonic<P>(
        rel: &Path,
        producer: &mut P,
    ) -> Result<Self, DecodeError<P::Final, P::Error, Blame>>
    where
        P: BulkProducer<Item = u8> + ?Sized,
    {
        wdm::Path::<MCL, MCC, MPL>::relative_decode_canonic(rel.into(), producer)
            .await
            .map(Into::into)
    }
}

/// /// Implements [path_rel_path](https://willowprotocol.org/specs/encodings/index.html#path_rel_path).
impl RelativeEncodableKnownLength<Path> for Path {
    fn len_of_relative_encoding(&self, rel: &Path) -> usize {
        self.0.len_of_relative_encoding(rel.into())
    }
}

////////////////////////////////
// path extends path encoding //
////////////////////////////////

/// Implementations of the [EncodePathExtendsPath](https://willowprotocol.org/specs/encodings/index.html#EncodePathExtendsPath) encoding relation.
pub mod path_extends_path {
    use super::*;

    #[cfg(feature = "dev")]
    use arbitrary::Arbitrary;

    /// Implements encoding for the [path_extends_path](https://willowprotocol.org/specs/encodings/index.html#path_extends_path) encoding function.
    pub async fn encode_path_extends_path<C>(
        path: &Path,
        prefix: &Path,
        consumer: &mut C,
    ) -> Result<(), C::Error>
    where
        C: BulkConsumer<Item = u8> + ?Sized,
    {
        willow_data_model::paths::path_extends_path::encode_path_extends_path(
            path.into(),
            prefix.into(),
            consumer,
        )
        .await
    }

    /// Returns the length of the encoding of `path` relative to `prefix` according to the [path_extends_path](https://willowprotocol.org/specs/encodings/index.html#path_extends_path) encoding function.
    pub fn path_extends_path_encoding_len(path: &Path, prefix: &Path) -> usize {
        willow_data_model::paths::path_extends_path::path_extends_path_encoding_len(
            path.into(),
            prefix.into(),
        )
    }

    /// Implements decoding for the [EncodePathExtendsPath](https://willowprotocol.org/specs/encodings/index.html#EncodePathExtendsPath) encoding relation.
    pub async fn decode_path_extends_path<P>(
        prefix: &Path,
        producer: &mut P,
    ) -> Result<Path, DecodeError<P::Final, P::Error, Blame>>
    where
        P: BulkProducer<Item = u8> + ?Sized,
    {
        willow_data_model::paths::path_extends_path::decode_path_extends_path(
            prefix.into(),
            producer,
        )
        .await
        .map(Into::into)
    }

    /// Implements canonic decoding for the [path_extends_path](https://willowprotocol.org/specs/encodings/index.html#path_extends_path) encoding function.
    pub async fn decode_path_extends_path_canonic<P>(
        prefix: &Path,
        producer: &mut P,
    ) -> Result<Path, DecodeError<P::Final, P::Error, Blame>>
    where
        P: BulkProducer<Item = u8> + ?Sized,
    {
        willow_data_model::paths::path_extends_path::decode_path_extends_path_canonic(
            prefix.into(),
            producer,
        )
        .await
        .map(Into::into)
    }

    /// A wrapper type around [`Path`] whose implementations of the [codec_relative] traits implement the [EncodePathExtendsPath](https://willowprotocol.org/specs/encodings/index.html#encsec_EncodePathExtendsPath) encoding relation.
    #[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Default, Debug)]
    #[cfg_attr(feature = "dev", derive(Arbitrary))]
    pub struct CodecPathExtendsPath(pub Path);

    /// Implements [path_extends_path](https://willowprotocol.org/specs/encodings/index.html#path_extends_path).
    impl RelativeEncodable<Path> for CodecPathExtendsPath {
        async fn relative_encode<C>(&self, rel: &Path, consumer: &mut C) -> Result<(), C::Error>
        where
            C: BulkConsumer<Item = u8> + ?Sized,
        {
            encode_path_extends_path(&self.0, rel, consumer).await
        }

        /// Returns `true` iff `rel` is a prefix of `self`.
        fn can_be_encoded_relative_to(&self, rel: &Path) -> bool {
            rel.is_prefix_of(&self.0)
        }
    }

    /// Implements [path_extends_path](https://willowprotocol.org/specs/encodings/index.html#path_extends_path).
    impl RelativeEncodableKnownLength<Path> for CodecPathExtendsPath {
        fn len_of_relative_encoding(&self, rel: &Path) -> usize {
            path_extends_path_encoding_len(&self.0, rel)
        }
    }

    /// Implements [EncodePathExtendsPath](https://willowprotocol.org/specs/encodings/index.html#EncodePathExtendsPath).
    impl RelativeDecodable<Path> for CodecPathExtendsPath {
        type ErrorReason = Blame;

        async fn relative_decode<P>(
            rel: &Path,
            producer: &mut P,
        ) -> Result<Self, DecodeError<P::Final, P::Error, Self::ErrorReason>>
        where
            P: BulkProducer<Item = u8> + ?Sized,
            Self: Sized,
        {
            Ok(Self(decode_path_extends_path(rel, producer).await?))
        }
    }

    /// Implements [path_extends_path](https://willowprotocol.org/specs/encodings/index.html#path_extends_path).
    impl RelativeDecodableCanonic<Path> for CodecPathExtendsPath {
        type ErrorCanonic = Blame;

        async fn relative_decode_canonic<P>(
            rel: &Path,
            producer: &mut P,
        ) -> Result<Self, DecodeError<P::Final, P::Error, Self::ErrorReason>>
        where
            P: BulkProducer<Item = u8> + ?Sized,
            Self: Sized,
        {
            Ok(Self(decode_path_extends_path_canonic(rel, producer).await?))
        }
    }
}
