use core::ops::RangeBounds;
use core::{fmt, ops::Bound};

#[cfg(feature = "dev")]
use arbitrary::Arbitrary;

use ufotofu::codec_prelude::*;

use order_theory::GreatestElement;

use willow_data_model::prelude as wdm;

use crate::prelude::*;

wrapper! {
    /// An [`Area`](https://willowprotocol.org/specs/grouping-entries/#Area) is a box in three-dimensional willow space, consisting of all entries matching either a single [`SubspaceId`] or entries of arbitrary [`SubspaceIds`](SubspaceId), prefixed by some [`Path`], and a [`TimeRange`](super::TimeRange).
    ///
    /// Areas are the default way by which application developers should aggregate entries. See [the specification](https://willowprotocol.org/specs/grouping-entries/#areas) for more details.
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let a1 = Area::new(None, Path::new(), TimeRange::new_closed(0.into(), 17.into()));
    ///
    /// assert!(a1.includes(&([5; 32].into(), Path::new(), Timestamp::from(9))));
    /// assert_eq!(a1.subspace(), None);
    ///
    /// let a2 = Area::new(Some([42; 32].into()), Path::new(), TimeRange::new_open(15.into()));
    /// assert_eq!(
    ///     a1.intersection(&a2),
    ///     Ok(Area::new(
    ///         Some([42; 32].into()),
    ///         Path::new(),
    ///         TimeRange::new_closed(15.into(), 17.into()),
    ///     )),
    /// );
    /// ```
    ///
    /// [Specification](https://willowprotocol.org/specs/grouping-entries/#Area)
    #[derive(Clone, Hash, PartialEq, Eq, PartialOrd)]
    #[cfg_attr(feature = "dev", derive(Arbitrary))]
    Area; wdm::Area<MCL, MCC, MPL, SubspaceId>
}

impl fmt::Debug for Area {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Grouping for Area {
    fn includes<Coord>(&self, coord: &Coord) -> bool
    where
        Coord: Coordinatelike + ?Sized,
    {
        wdm::Grouping::wdm_includes(&self.0, coord)
    }

    fn intersection(
        &self,
        other: &Self,
    ) -> Result<Self, willow_data_model::prelude::EmptyGrouping> {
        wdm::Grouping::wdm_intersection(&self.0, &other.0).map(Into::into)
    }
}

impl RangeBounds<SubspaceId> for Area {
    fn start_bound(&self) -> Bound<&SubspaceId> {
        match self.subspace() {
            None => Bound::Unbounded,
            Some(s) => Bound::Included(s),
        }
    }

    fn end_bound(&self) -> Bound<&SubspaceId> {
        match self.subspace() {
            None => Bound::Unbounded,
            Some(s) => Bound::Included(s),
        }
    }
}

impl RangeBounds<Timestamp> for Area {
    fn start_bound(&self) -> Bound<&Timestamp> {
        self.times().start_bound()
    }

    fn end_bound(&self) -> Bound<&Timestamp> {
        self.times().end_bound()
    }
}

impl Area {
    /// Creates a new `Area` from its constituent optional [`SubspaceId`], [`Path`], and [`TimeRange`](super::TimeRange).
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let a = Area::new(None, Path::new(), TimeRange::new_closed(0.into(), 17.into()));
    ///
    /// assert!(a.includes(&([5; 32].into(), Path::new(), Timestamp::from(9))));
    /// assert_eq!(a.subspace(), None);
    /// ```
    pub fn new(subspace: Option<SubspaceId>, path: Path, times: TimeRange) -> Self {
        wdm::Area::new(subspace, path.into(), times).into()
    }

    /// Returns a reference to the inner [`SubspaceId`], if any.
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let a1 = Area::new(
    ///     Some([17; 32].into()),
    ///     Path::new(),
    ///     TimeRange::new_closed(0.into(), 17.into()),
    /// );
    /// assert_eq!(a1.subspace(), Some(&[17; 32].into()));
    ///
    /// let a2 = Area::new(None, Path::new(), TimeRange::new_closed(0.into(), 17.into()));
    /// assert_eq!(a2.subspace(), None);
    /// ```
    ///
    /// [Definition](https://willowprotocol.org/specs/grouping-entries/#AreaSubspace).
    pub fn subspace(&self) -> Option<&SubspaceId> {
        self.0.subspace()
    }

    /// Returns a reference to the inner [`Path`].
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let a = Area::new(
    ///     Some([17; 32].into()),
    ///     Path::new(),
    ///     TimeRange::new_closed(0.into(), 17.into()),
    /// );
    /// assert_eq!(a.path(), &Path::new());
    /// ```
    ///
    /// [Definition](https://willowprotocol.org/specs/grouping-entries/#AreaPath).
    pub fn path(&self) -> &Path {
        self.0.path().into()
    }

    /// Returns a reference to the inner [`TimeRange`](super::TimeRange).
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let a = Area::new(
    ///     Some([17; 32].into()),
    ///     Path::new(),
    ///     TimeRange::new_closed(0.into(), 17.into()),
    /// );
    /// assert_eq!(a.times(), &WillowRange::from(TimeRange::new_closed(0.into(), 17.into())));
    /// ```
    ///
    /// [Definition](https://willowprotocol.org/specs/grouping-entries/#AreaTime).
    pub fn times(&self) -> &TimeRange {
        self.0.times()
    }

    /// Sets the inner [`SubspaceId`].
    pub fn set_subspace(&mut self, new_subspace: Option<SubspaceId>) {
        self.0.set_subspace(new_subspace);
    }

    /// Sets the inner [`Path`].
    pub fn set_path(&mut self, new_path: Path) {
        self.0.set_path(new_path.into());
    }

    /// Sets the inner [`TimeRange`].
    pub fn set_times<TR>(&mut self, new_range: TR)
    where
        TR: Into<TimeRange>,
    {
        self.0.set_times(new_range)
    }

    /// Returns the [subspace area](https://willowprotocol.org/specs/grouping-entries/#subspace_area) for the given subspace id, i.e., the area which includes exactly the entries of the given subspace id.
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let a = Area::new_subspace_area([17; 32].into());
    ///
    /// assert!(a.includes(&([17; 32].into(), Path::new(), Timestamp::from(9))));
    /// assert!(!a.includes(&([18; 32].into(), Path::new(), Timestamp::from(9))));
    /// assert_eq!(a.subspace(), Some(&[17; 32].into()));
    /// ```
    pub fn new_subspace_area(subspace_id: SubspaceId) -> Self {
        wdm::Area::new_subspace_area(subspace_id).into()
    }

    /// Returns whether an [`Entry`] of the given [coordinate](Coordinatelike) could possibly cause [prefix pruning](https://willowprotocol.org/specs/data-model/index.html#prefix_pruning) in this area.
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let a = Area::new(
    ///     Some([17; 32].into()),
    ///     Path::new(),
    ///     TimeRange::new_closed(0.into(), 17.into()),
    /// );
    ///
    /// assert!(a.admits_pruning_by(&([17; 32].into(), Path::new(), Timestamp::from(9))));
    /// assert!(!a.admits_pruning_by(&([18; 32].into(), Path::new(), Timestamp::from(9))));
    /// ```
    pub fn admits_pruning_by<Coord>(&self, coord: &Coord) -> bool
    where
        Coord: Coordinatelike,
    {
        self.0.admits_pruning_by(coord)
    }

    /// Returns the [`Area`] which [includes](Area::includes) every [coordinate](Coordinatelike).
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let a = Area::full();
    ///
    /// assert!(a.includes(&([5; 32].into(), Path::new(), Timestamp::from(9))));
    /// assert!(a.includes(&(SubspaceId::from([16; 32]), Path::new(), Timestamp::from(9))));
    /// ```
    pub fn full() -> Self {
        wdm::Area::full().into()
    }

    /// Returns whether `self` is the full area, i.e., the area which [includes](Area::includes) every [coordinate](Coordinatelike).
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// assert!(Area::full().is_full());
    /// assert!(!Area::new(
    ///     Some([17; 32].into()),
    ///     Path::new(),
    ///     TimeRange::new_closed(0.into(), 17.into()),
    /// ).is_full());
    /// ```
    pub fn is_full(&self) -> bool {
        self.0.is_full()
    }
}

impl GreatestElement for Area {
    fn greatest() -> Self {
        wdm::Area::greatest().into()
    }
}

///////////////////////
// Codec Stuff Below //
///////////////////////

/// Implements [encode_area_in_area](https://willowprotocol.org/specs/encodings/index.html#encode_area_in_area).
impl RelativeEncodable<Area> for Area {
    async fn relative_encode<C>(&self, rel: &Area, consumer: &mut C) -> Result<(), C::Error>
    where
        C: BulkConsumer<Item = u8> + ?Sized,
    {
        self.0
            .relative_encode(
                <&willow_data_model::groupings::Area<MCL, MCC, MPL, SubspaceId>>::from(rel),
                consumer,
            )
            .await
    }

    /// Returns `true` iff `rel` includes `self`.
    fn can_be_encoded_relative_to(&self, rel: &Area) -> bool {
        self.0
            .can_be_encoded_relative_to(<&willow_data_model::groupings::Area<
                MCL,
                MCC,
                MPL,
                SubspaceId,
            >>::from(rel))
    }
}

/// Implements [EncodeAreaInArea](https://willowprotocol.org/specs/encodings/index.html#EncodeAreaInArea).
impl RelativeDecodable<Area> for Area {
    type ErrorReason = Blame;

    async fn relative_decode<P>(
        rel: &Area,
        producer: &mut P,
    ) -> Result<Self, DecodeError<P::Final, P::Error, Blame>>
    where
        P: BulkProducer<Item = u8> + ?Sized,
    {
        wdm::Area::<MCL, MCC, MPL, SubspaceId>::relative_decode(
            <&willow_data_model::groupings::Area<MCL, MCC, MPL, SubspaceId>>::from(rel),
            producer,
        )
        .await
        .map(Into::into)
    }
}

/// Implements [encode_area_in_area](https://willowprotocol.org/specs/encodings/index.html#encode_area_in_area).
impl RelativeDecodableCanonic<Area> for Area {
    type ErrorCanonic = Blame;

    async fn relative_decode_canonic<P>(
        rel: &Area,
        producer: &mut P,
    ) -> Result<Self, DecodeError<P::Final, P::Error, Blame>>
    where
        P: BulkProducer<Item = u8> + ?Sized,
        Self: Sized,
    {
        wdm::Area::<MCL, MCC, MPL, SubspaceId>::relative_decode_canonic(rel.into(), producer)
            .await
            .map(Into::into)
    }
}

/// Implements [encode_area_in_area](https://willowprotocol.org/specs/encodings/index.html#encode_area_in_area).
impl RelativeEncodableKnownLength<Area> for Area {
    fn len_of_relative_encoding(&self, rel: &Area) -> usize {
        self.0.len_of_relative_encoding(rel.into())
    }
}

/// Returns an [`Arbitrary`] area which is guaranteed to be included in the reference area.
#[cfg(feature = "dev")]
pub fn arbitrary_area_in_area<'a>(
    reference: &Area,
    u: &mut arbitrary::Unstructured<'a>,
) -> arbitrary::Result<Area> {
    willow_data_model::groupings::arbitrary_area_in_area::<MCL, MCC, MPL, SubspaceId>(
        reference.into(),
        u,
    )
    .map(Into::into)
}
