use core::fmt;
use core::ops::RangeBounds;

#[cfg(feature = "dev")]
use arbitrary::Arbitrary;

use order_theory::GreatestElement;

use willow_data_model::prelude as wdm;

use crate::prelude::*;

wrapper! {
    /// An arbitrary non-empty box in three-dimensional willow space, consisting of a [`SubspaceRange`](super::SubspaceRange), a [`PathRange`](super::PathRange), and a [`TimeRange`](super::TimeRange).
    ///
    /// As an application developer, you probably do not need to interact with 3d ranges, you should prefer [`Areas`](super::Area) — the latter work well with encrypted data, whereas 3d ranges do not define human-meaningful subsets of data when working with encryption.
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let r = Range3d::new(
    ///     SubspaceRange::full(),
    ///     PathRange::full(),
    ///     TimeRange::new_closed(0.into(), 17.into()),
    /// );
    ///
    /// assert!(r.includes(&([5; 32].into(), Path::new(), Timestamp::from(9))));
    /// assert_eq!(r.subspaces(), &SubspaceRange::full());
    ///
    /// let r2 = Range3d::new(
    ///     SubspaceRange::full(),
    ///     PathRange::full(),
    ///     TimeRange::new_open(15.into()),
    /// );
    /// assert_eq!(
    ///     r.intersection(&r2),
    ///     Ok(Range3d::new(
    ///         SubspaceRange::full(),
    ///         PathRange::full(),
    ///         TimeRange::new_closed(15.into(), 17.into()),
    ///     )),
    /// );
    /// ```
    ///
    /// [Specification](https://willowprotocol.org/specs/grouping-entries/index.html#D3Range)
    #[derive(Clone, Hash, PartialEq, Eq, PartialOrd)]
    #[cfg_attr(feature = "dev", derive(Arbitrary))]
    Range3d; wdm::Range3d<MCL, MCC, MPL, SubspaceId>
}

impl fmt::Debug for Range3d {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Grouping for Range3d {
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

impl RangeBounds<SubspaceId> for Range3d {
    fn start_bound(&self) -> core::ops::Bound<&SubspaceId> {
        self.0.subspaces().start_bound()
    }

    fn end_bound(&self) -> core::ops::Bound<&SubspaceId> {
        self.0.subspaces().end_bound()
    }
}

impl RangeBounds<Path> for Range3d {
    fn start_bound(&self) -> core::ops::Bound<&Path> {
        self.0.paths().start_bound().map(Into::into)
    }

    fn end_bound(&self) -> core::ops::Bound<&Path> {
        self.0.paths().end_bound().map(Into::into)
    }
}

impl RangeBounds<Timestamp> for Range3d {
    fn start_bound(&self) -> core::ops::Bound<&Timestamp> {
        self.0.times().start_bound()
    }

    fn end_bound(&self) -> core::ops::Bound<&Timestamp> {
        self.0.times().end_bound()
    }
}

impl Range3d {
    /// Creates a new `Range3d` from its constituent [`SubspaceRange`](super::SubspaceRange), [`PathRange`](super::PathRange), and [`TimeRange`](super::TimeRange).
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let r = Range3d::new(
    ///     SubspaceRange::new_open([5; 32].into()),
    ///     PathRange::full(),
    ///     TimeRange::new_closed(0.into(), 17.into()),
    /// );
    ///
    /// assert!(r.includes(&([6; 32].into(), Path::new(), Timestamp::from(9))));
    /// assert_eq!(r.subspaces(), &SubspaceRange::new_open([5; 32].into()));
    /// ```
    pub fn new<SR, PR, TR>(subspaces: SR, paths: PR, times: TR) -> Self
    where
        SR: Into<SubspaceRange>,
        PR: Into<PathRange>,
        TR: Into<TimeRange>,
    {
        wdm::Range3d::<MCL, MCC, MPL, SubspaceId>::new(
            subspaces,
            paths.into().map(Into::into),
            times,
        )
        .into()
    }

    /// Returns a reference to the inner [`SubspaceRange`](super::SubspaceRange).
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let r = Range3d::new(
    ///     SubspaceRange::new_open([5; 32].into()),
    ///     PathRange::full(),
    ///     TimeRange::new_closed(0.into(), 17.into()),
    /// );
    /// assert_eq!(r.subspaces(), &SubspaceRange::new_open([5; 32].into()));
    /// ```
    ///
    /// [Definition](https://willowprotocol.org/specs/grouping-entries/index.html#D3RangeSubspace).
    pub fn subspaces(&self) -> &SubspaceRange {
        self.0.subspaces()
    }

    /// Returns a reference to the inner [`PathRange`](super::PathRange).
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let r = Range3d::new(
    ///     SubspaceRange::new_open([5; 32].into()),
    ///     PathRange::full(),
    ///     TimeRange::new_closed(0.into(), 17.into()),
    /// );
    /// assert_eq!(r.paths(), &WillowRange::full());
    /// ```
    ///
    /// [Definition](https://willowprotocol.org/specs/grouping-entries/index.html#D3RangePath).
    pub fn paths(&self) -> &PathRange {
        unsafe { self.0.paths().cast() }
    }

    /// Returns a reference to the inner [`TimeRange`](super::TimeRange).
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let r = Range3d::new(
    ///     SubspaceRange::new_open([5; 32].into()),
    ///     PathRange::full(),
    ///     TimeRange::new_closed(0.into(), 17.into()),
    /// );
    /// assert_eq!(r.times(), &TimeRange::new_closed(0.into(), 17.into()));
    /// ```
    ///
    /// [Definition](https://willowprotocol.org/specs/grouping-entries/index.html#D3RangeTime).
    pub fn times(&self) -> &TimeRange {
        self.0.times()
    }

    /// Sets the inner [`SubspaceRange`](super::SubspaceRange).
    pub fn set_subspaces<SR>(&mut self, new_range: SR)
    where
        SR: Into<SubspaceRange>,
    {
        self.0.set_subspaces(new_range);
    }

    /// Sets the inner [`PathRange`](super::PathRange).
    pub fn set_paths<PR>(&mut self, new_range: PR)
    where
        PR: Into<PathRange>,
    {
        self.0.set_paths(new_range.into().map(Into::into));
    }

    /// Sets the inner [`TimeRange`](super::TimeRange).
    pub fn set_times<TR>(&mut self, new_range: TR)
    where
        TR: Into<TimeRange>,
    {
        self.0.set_times(new_range)
    }

    /// Returns the [`Range3d`] which [includes](Range3d::includes) `coord` but no other value.
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let r = Range3d::singleton(&([5; 32].into(), Path::new(), Timestamp::from(9)));
    ///
    /// assert!(r.includes(&([5; 32].into(), Path::new(), Timestamp::from(9))));
    /// assert!(!r.includes(&([5; 32].into(), Path::new(), Timestamp::from(10))));
    /// ```
    pub fn singleton<Coord>(coord: &Coord) -> Self
    where
        Coord: Coordinatelike,
    {
        wdm::Range3d::<MCL, MCC, MPL, SubspaceId>::singleton(coord).into()
    }

    /// Returns the `Range3d` which [includes](Range3d::includes) every [coordinate](Coordinatelike).
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let r = Range3d::full();
    ///
    /// assert!(r.includes(&([5; 32].into(), Path::new(), Timestamp::from(9))));
    /// assert!(r.includes(&([5; 32].into(), Path::new(), Timestamp::from(10))));
    /// ```
    pub fn full() -> Self {
        wdm::Range3d::<MCL, MCC, MPL, SubspaceId>::full().into()
    }

    /// Returns whether `self` is the full 3d range, i.e., the 3d range which [includes](Range3d::includes) every [coordinate](Coordinatelike).
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// assert!(Range3d::full().is_full());
    /// assert!(!Range3d::new(
    ///     SubspaceRange::full(),
    ///     PathRange::full(),
    ///     TimeRange::new_closed(0.into(), 17.into()),
    /// ).is_full());
    /// ```
    pub fn is_full(&self) -> bool {
        self.0.is_full()
    }
}

impl GreatestElement for Range3d {
    fn greatest() -> Self {
        wdm::Range3d::<MCL, MCC, MPL, SubspaceId>::greatest().into()
    }
}

impl From<Area> for Range3d {
    fn from(value: Area) -> Self {
        Self(value.0.into())
    }
}
