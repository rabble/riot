use core::fmt;

#[cfg(feature = "dev")]
use arbitrary::Arbitrary;

use order_theory::GreatestElement;

use willow_data_model::prelude::{self as wdm, EmptyGrouping};

use crate::prelude::*;

pub use core::num::NonZeroU64;

wrapper! {
    /// An [`AreaOfInterest`](https://willowprotocol.org/specs/grouping-entries/#Area) is an [`Area`] together with a maximum number of entries it may include, and a maximum sum of total [payload_lengths](https://willowprotocol.org/specs/data-model/index.html#entry_payload_length) for the included entries.
    ///
    /// A [`max_count`](https://willowprotocol.org/specs/grouping-entries/#aoi_count) of [`u64::MAX`] indicates that there is no limit on the number of entries. A [`max_size`](https://willowprotocol.org/specs/grouping-entries/#aoi_size) of [`u64::MAX`] indicates that there is no limit on the total [payload_lengths](https://willowprotocol.org/specs/data-model/index.html#entry_payload_length) of the entries.
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let aoi = AreaOfInterest::new(
    ///     Area::new(None, Path::new(), TimeRange::new_closed(0.into(), 17.into())),
    ///     NonZeroU64::new(5).unwrap(),
    ///     400,
    /// );
    ///
    /// assert_eq!(
    ///     aoi.area(),
    ///     &Area::new(None, Path::new(), TimeRange::new_closed(0.into(), 17.into())),
    /// );
    /// assert_eq!(aoi.max_count(), NonZeroU64::new(5).unwrap());
    /// assert_eq!(aoi.max_size(), 400);
    /// ```
    ///
    /// [Specification](https://willowprotocol.org/specs/grouping-entries/#aois)
    #[derive(Clone, Hash, PartialEq, Eq, PartialOrd)]
    #[cfg_attr(feature = "dev", derive(Arbitrary))]
    AreaOfInterest; wdm::AreaOfInterest<MCL, MCC, MPL, SubspaceId>
}

impl fmt::Debug for AreaOfInterest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl AreaOfInterest {
    /// Creates a new `AreaOfInterest` from its constituent [`Area`], [max_count](https://willowprotocol.org/specs/grouping-entries/#aoi_count), and [max_size](https://willowprotocol.org/specs/grouping-entries/#aoi_size).
    ///
    /// A [`max_count`](https://willowprotocol.org/specs/grouping-entries/#aoi_count) of [`u64::MAX`] indicates that there is no limit on the number of entries. A [`max_size`](https://willowprotocol.org/specs/grouping-entries/#aoi_size) of [`u64::MAX`] indicates that there is no limit on the total [payload_lengths](https://willowprotocol.org/specs/data-model/index.html#entry_payload_length) of the entries.
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let aoi = AreaOfInterest::new(
    ///     Area::new(None, Path::new(), TimeRange::new_closed(0.into(), 17.into())),
    ///     NonZeroU64::new(5).unwrap(),
    ///     400,
    /// );;
    ///
    /// assert_eq!(
    ///     aoi.area(),
    ///     &Area::new(None, Path::new(), TimeRange::new_closed(0.into(), 17.into())),
    /// );
    /// assert_eq!(aoi.max_count(), NonZeroU64::new(5).unwrap());
    /// assert_eq!(aoi.max_size(), 400);
    /// ```
    pub fn new(area: Area, max_count: NonZeroU64, max_size: u64) -> Self {
        wdm::AreaOfInterest::<MCL, MCC, MPL, SubspaceId>::new(area.into(), max_count, max_size)
            .into()
    }

    /// Returns a reference to the inner [`Area`].
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let aoi = AreaOfInterest::new(
    ///     Area::new(None, Path::new(), TimeRange::new_closed(0.into(), 17.into())),
    ///     NonZeroU64::new(5).unwrap(),
    ///     400,
    /// );;
    /// assert_eq!(
    ///     aoi.area(),
    ///     &Area::new(None, Path::new(), TimeRange::new_closed(0.into(), 17.into())),
    /// );
    /// ```
    ///
    /// [Definition](https://willowprotocol.org/specs/grouping-entries/#aoi_area).
    pub fn area(&self) -> &Area {
        self.0.area().into()
    }

    /// Returns the [max_count](https://willowprotocol.org/specs/grouping-entries/#aoi_count) of entries.
    ///
    /// A [`max_count`](https://willowprotocol.org/specs/grouping-entries/#aoi_count) of [`u64::MAX`] indicates that there is no limit on the number of entries in this area of interest.
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let aoi = AreaOfInterest::new(
    ///     Area::new(None, Path::new(), TimeRange::new_closed(0.into(), 17.into())),
    ///     NonZeroU64::new(5).unwrap(),
    ///     400,
    /// );;
    /// assert_eq!(aoi.max_count(), NonZeroU64::new(5).unwrap());
    /// ```
    ///
    /// [Definition](https://willowprotocol.org/specs/grouping-entries/#AreaPath).
    pub fn max_count(&self) -> NonZeroU64 {
        self.0.max_count()
    }

    /// Returns the [max_count](https://willowprotocol.org/specs/grouping-entries/#aoi_count) of entries.
    ///
    /// A [`max_size`](https://willowprotocol.org/specs/grouping-entries/#aoi_size) of [`u64::MAX`] indicates that there is no limit on the total [payload_lengths](https://willowprotocol.org/specs/data-model/index.html#entry_payload_length) of the entries in this area of interest.
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let aoi = AreaOfInterest::new(
    ///     Area::new(None, Path::new(), TimeRange::new_closed(0.into(), 17.into())),
    ///     NonZeroU64::new(5).unwrap(),
    ///     400,
    /// );;
    /// assert_eq!(aoi.max_size(), 400);
    /// ```
    ///
    /// [Definition](https://willowprotocol.org/specs/grouping-entries/#AreaPath).
    pub fn max_size(&self) -> u64 {
        self.0.max_size()
    }

    /// Sets the inner area.
    pub fn set_area(&mut self, new_area: Area) {
        self.0.set_area(new_area.into())
    }

    /// Sets the [`max_count`](https://willowprotocol.org/specs/grouping-entries/#aoi_count).
    ///
    /// A [`max_count`](https://willowprotocol.org/specs/grouping-entries/#aoi_count) of [`u64::MAX`] indicates that there is no limit on the number of entries in this area of interest.
    pub fn set_max_count(&mut self, new_max_count: NonZeroU64) {
        self.0.set_max_count(new_max_count);
    }

    /// Sets the [`max_size`](https://willowprotocol.org/specs/grouping-entries/#aoi_size).
    ///
    /// A [`max_size`](https://willowprotocol.org/specs/grouping-entries/#aoi_size) of [`u64::MAX`] indicates that there is no limit on the total [payload_lengths](https://willowprotocol.org/specs/data-model/index.html#entry_payload_length) of the entries in this area of interest.
    pub fn set_max_size(&mut self, new_max_size: u64) {
        self.0.set_max_size(new_max_size);
    }
}

impl AreaOfInterest {
    /// Returns the [intersection](https://willowprotocol.org/specs/grouping-entries/#aoi_intersection) of `self` and `other`.
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let aoi1 = AreaOfInterest::new(
    ///     Area::new(None, Path::new(), TimeRange::new_closed(0.into(), 17.into())),
    ///     NonZeroU64::new(5).unwrap(),
    ///     400,
    /// );
    /// let aoi2 = AreaOfInterest::new(
    ///     Area::new(None, Path::new(), TimeRange::new_open(14.into())),
    ///     NonZeroU64::new(12).unwrap(),
    ///     32,
    /// );
    ///
    /// assert_eq!(
    ///     aoi1.intersection(&aoi2),
    ///     Ok(AreaOfInterest::new(
    ///         Area::new(
    ///             None,
    ///             Path::new(),
    ///             TimeRange::new_closed(14.into(), 17.into())
    ///         ),
    ///         NonZeroU64::new(5).unwrap(),
    ///         32,
    ///     )),
    /// );
    /// ```
    pub fn intersection(&self, other: &Self) -> Result<Self, EmptyGrouping> {
        self.0.intersection(other.into()).map(Into::into)
    }
}

impl GreatestElement for AreaOfInterest {
    fn greatest() -> Self {
        wdm::AreaOfInterest::<MCL, MCC, MPL, SubspaceId>::greatest().into()
    }

    fn is_greatest(&self) -> bool {
        self.0.is_greatest()
    }
}
