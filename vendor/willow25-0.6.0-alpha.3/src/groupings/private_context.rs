//! Provides abstractions for [privately encoding areas relative to other areas](https://willowprotocol.org/specs/encodings/index.html#enc_private_areas) and [private interests](https://willowprotocol.org/specs/pio/index.html#pio_private_interests).
//!
//! You probably don't need to interact with this unless you are building your own private encoding scheme.

use core::fmt;

use crate::prelude::*;
use ufotofu::{
    codec::Blame,
    codec_prelude::{RelativeDecodable, RelativeEncodable},
};
use willow_data_model::groupings::private_interest::{
    AreaNotAlmostIncludedError, PrivateAreaContext as PrivateAreaContextGeneric,
    PrivateInterest as PrivateInterestGeneric,
};

wrapper! {
  /// Confidential data that relates to determining the [`AreaOfInterest`] that peers might be interested in synchronising.
  #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
  PrivateInterest;
  PrivateInterestGeneric<MCL, MCC, MPL, NamespaceId, SubspaceId>
}

impl fmt::Debug for PrivateInterest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl PrivateInterest {
    /// Returns a new [`PrivateInterest`] with the gives attributes.
    pub fn new(namespace_id: NamespaceId, subspace_id: Option<SubspaceId>, path: Path) -> Self {
        Self(PrivateInterestGeneric::new(
            namespace_id,
            subspace_id,
            path.into(),
        ))
    }

    /// Returns the Namespace ID of this [`PrivateInterest`].
    pub fn namespace(&self) -> &NamespaceId {
        self.0.namespace()
    }

    /// Returns the specific SubspaceId of this [`PrivateInterest`], if present. `None` denotes interest in all subspaces of the namespace.
    pub fn subspace(&self) -> Option<&SubspaceId> {
        self.0.subspace()
    }

    /// Returns the path of this [`PrivateInterest`].
    pub fn path(&self) -> &Path {
        <&willow_data_model::paths::Path<MCL, MCC, MPL> as Into<&Path>>::into(self.0.path())
    }

    /// Returns `true` is this [`PrivateInterest`] is [more specific](https://willowprotocol.org/specs/pio/index.html#pi_more_specific) than `other`.
    pub fn is_more_specific(&self, other: &Self) -> bool {
        self.0.is_more_specific(other.into())
    }

    /// Returns `true` is this [`PrivateInterest`] is [disjoint](https://willowprotocol.org/specs/pio/index.html#pi_disjoint) from `other`, namely that there is no [`Entry`] which can be included in both.
    pub fn is_disjoint(&self, other: &Self) -> bool {
        self.0.is_disjoint(other.into())
    }

    /// Returns `true` is this [`PrivateInterest`] is [less specific](https://willowprotocol.org/specs/pio/index.html#pi_less_specific) than `other`.
    pub fn is_less_specific(&self, other: &Self) -> bool {
        self.0.is_less_specific(other.into())
    }

    /// Returns `true` is this [`PrivateInterest`] is [comparable](https://willowprotocol.org/specs/pio/index.html#pi_comparable) to `other`.
    pub fn is_comparable(&self, other: &Self) -> bool {
        self.0.is_comparable(other.into())
    }

    /// Returns true if `self` and `other` are [awkward](https://willowprotocol.org/specs/pio/index.html#pi_awkward), meaning they are neither [comparable](https://willowprotocol.org/specs/pio/index.html#pi_comparable) nor [disjoint](https://willowprotocol.org/specs/pio/index.html#pi_disjoint).
    pub fn are_awkward(&self, other: &Self) -> bool {
        self.0.are_awkward(other.into())
    }

    /// Returns `true` if the given [`Entry`] is [included](https://willowprotocol.org/specs/pio/index.html#pi_include_entry) by `self`.
    pub fn includes_entry<PD>(&self, entry: &Entry) -> bool {
        self.0.includes_entry(entry.into())
    }

    /// Returns `true` if the given [`Area`] in [included](https://willowprotocol.org/specs/pio/index.html#pi_include_area) by `self`.
    pub fn includes_area(&self, area: &Area) -> bool {
        self.0.includes_area(area.into())
    }

    /// Returns `true` if the given [`Area`] is related to `self`, that is:
    ///
    /// - the path of `self` and the path of the `area` are [related](Path::is_related_to), and
    /// - either `self.subspace()` is `None` or `self.subspace() == area.subspace()`.
    pub fn is_related_to_area(&self, area: &Area) -> bool {
        self.0.includes_area(area.into())
    }

    /// Returns `true` if `self` [almost includes](https://willowprotocol.org/specs/encodings/index.html#almost_include) the given [`Area`].
    pub fn almost_includes_area(&self, area: &Area) -> bool {
        self.0.almost_includes_area(area.into())
    }

    /// Clones self, but replaces the subspace id with `None`.
    pub fn relax(&self) -> Self {
        self.0.relax().into()
    }
}

wrapper! {
  /// The immutable [`PrivateAreaContext`](https://willowprotocol.org/specs/encodings/index.html#PrivateAreaContext) necessary to privately encode an [`Area`] relative to another [`Area`] which [almost includes](https://willowprotocol.org/specs/encodings/index.html#pi_amost_include) it.
  #[derive(Clone, PartialEq, Eq, PartialOrd, Hash)]
  PrivateAreaContext; PrivateAreaContextGeneric<MCL, MCC, MPL, NamespaceId, SubspaceId>
}

impl fmt::Debug for PrivateAreaContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl PrivateAreaContext {
    /// Returns a new [`PrivateAreaContext`] with the given `private` and `rel` attributes. Will fail if the given [`PrivateInterest`]'s path is not a prefix of `rel`'s path.
    pub fn new(private: PrivateInterest, rel: Area) -> Result<Self, AreaNotAlmostIncludedError> {
        let inner = PrivateAreaContextGeneric::new(private.into(), rel.into())?;

        Ok(Self(inner))
    }

    /// Returns the [`PrivateInterest`] of this [`PrivateAreaContext`].
    pub fn private(&self) -> &PrivateInterest {
        self.0.private().into()
    }

    /// Returns the relative [`Area`] of this [`PrivateAreaContext`].
    pub fn rel(&self) -> &Area {
        self.0.rel().into()
    }

    /// Returns whether an [`Area`] _almost includes_ another area, that is if the other [`Area`] would be included by this [`Area] if it had the same [`SubspaceId`].
    pub fn almost_includes_area(&self, other: &Area) -> bool {
        self.0.almost_includes_area(other.into())
    }
}

impl RelativeEncodable<PrivateAreaContext> for Area {
    async fn relative_encode<C>(
        &self,
        rel: &PrivateAreaContext,
        consumer: &mut C,
    ) -> Result<(), C::Error>
    where
        C: ufotofu::BulkConsumer<Item = u8> + ?Sized,
    {
        self.0
            .relative_encode(
                <&PrivateAreaContextGeneric<MCL, MCC, MPL, NamespaceId, SubspaceId>>::from(rel),
                consumer,
            )
            .await
    }

    /// Returns `false` when `self` is not [almost included by](https://willowprotocol.org/specs/encodings/index.html#almost_include) the relative [`PrivateAreaContext`].
    fn can_be_encoded_relative_to(&self, rel: &PrivateAreaContext) -> bool {
        self.0
            .can_be_encoded_relative_to(<&PrivateAreaContextGeneric<
                MCL,
                MCC,
                MPL,
                NamespaceId,
                SubspaceId,
            >>::from(rel))
    }
}

impl RelativeDecodable<PrivateAreaContext> for Area {
    type ErrorReason = Blame;

    async fn relative_decode<P>(
        rel: &PrivateAreaContext,
        producer: &mut P,
    ) -> Result<Self, ufotofu::codec::DecodeError<P::Final, P::Error, Self::ErrorReason>>
    where
        P: ufotofu::BulkProducer<Item = u8> + ?Sized,
        Self: Sized,
    {
        let inner =
            willow_data_model::groupings::Area::<MCL, MCC, MPL, SubspaceId>::relative_decode(
                <&PrivateAreaContextGeneric<MCL, MCC, MPL, NamespaceId, SubspaceId>>::from(rel),
                producer,
            )
            .await?;

        Ok(Self(inner))
    }
}
