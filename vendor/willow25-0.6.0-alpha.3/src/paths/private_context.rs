//! Module providing abstractions for [`PrivatePathContext`](https://willowprotocol.org/specs/encodings/index.html#PrivatePathContext)s, which facilitate [private encodings](https://willowprotocol.org/specs/encodings/index.html#enc_private). The [`PrivatePathContext`] is the common context between two parties which makes it possible to decode a private encoding. You probably don't need to interact with this unless you are building your own private encoding scheme.

use core::fmt;

use crate::prelude::*;
use ufotofu::{
    codec::Blame,
    codec_prelude::{RelativeDecodable, RelativeEncodable},
};
use willow_data_model::paths::private::{ComponentsNotRelatedError, PrivatePathContext as PPC};

wrapper! {
  /// The [`PrivatePathContext`](https://willowprotocol.org/specs/encodings/index.html#PrivatePathContext) necessary to privately encode a [`Path`] relative to one of its prefixes, while keeping secret all Components that coincide with a third Path.
  #[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
  PrivatePathContext; willow_data_model::paths::private::PrivatePathContext<MCL, MCC, MPL>
}

impl fmt::Debug for PrivatePathContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl PrivatePathContext {
    /// Returns a new [`PrivatePathContext`] with the given private and relative paths.
    ///
    /// Will return a [`ComponentsNotRelatedError`] if the given private and relative paths are not [related](https://willowprotocol.org/specs/data-model/index.html#path_related).
    pub fn new(private: Path, rel: Path) -> Result<Self, ComponentsNotRelatedError> {
        let inner = PPC::new(private.into(), rel.into())?;

        Ok(Self(inner))
    }

    /// Returns a new [`PrivatePathContext`] with the given private and relative paths *without* checking if the given private and relative paths are [related](https://willowprotocol.org/specs/data-model/index.html#path_related).
    ///
    /// #### Safety
    ///
    /// Undefined behaviour if private and rel are not [related](https://willowprotocol.org/specs/data-model/index.html#path_related).
    pub unsafe fn new_unchecked(private: Path, rel: Path) -> Self {
        let inner = unsafe { PPC::new_unchecked(private.into(), rel.into()) };

        Self(inner)
    }

    /// Returns the private path of `&self`.
    pub fn private(&self) -> &Path {
        <&willow_data_model::paths::Path<MCL, MCC, MPL> as Into<&Path>>::into(self.0.private())
    }

    /// Returns the relative path of `&self`.
    pub fn rel(&self) -> &Path {
        <&willow_data_model::paths::Path<MCL, MCC, MPL> as Into<&Path>>::into(self.0.rel())
    }
}

impl RelativeEncodable<PrivatePathContext> for Path {
    async fn relative_encode<C>(
        &self,
        rel: &PrivatePathContext,
        consumer: &mut C,
    ) -> Result<(), C::Error>
    where
        C: ufotofu::BulkConsumer<Item = u8> + ?Sized,
    {
        self.0
            .relative_encode(<&PPC<MCL, MCC, MPL>>::from(rel), consumer)
            .await
    }

    /// Returns false if the path is not a [prefix](https://willowprotocol.org/specs/data-model/index.html#path_prefix) of `rel.rel`, OR if `self` is not [related to]((https://willowprotocol.org/specs/data-model/index.html#path_related)) to `rel.private`.
    fn can_be_encoded_relative_to(&self, rel: &PrivatePathContext) -> bool {
        self.0
            .can_be_encoded_relative_to(<&PPC<MCL, MCC, MPL>>::from(rel))
    }
}

impl RelativeDecodable<PrivatePathContext> for Path {
    type ErrorReason = Blame;

    async fn relative_decode<P>(
        rel: &PrivatePathContext,
        producer: &mut P,
    ) -> Result<Self, ufotofu::codec::DecodeError<P::Final, P::Error, Self::ErrorReason>>
    where
        P: ufotofu::BulkProducer<Item = u8> + ?Sized,
        Self: Sized,
    {
        let inner = willow_data_model::paths::Path::<MCL, MCC, MPL>::relative_decode(
            <&PPC<MCL, MCC, MPL>>::from(rel),
            producer,
        )
        .await?;

        Ok(Self(inner))
    }
}
