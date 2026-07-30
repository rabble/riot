//! Non-empty [groupings](https://willowprotocol.org/specs/grouping-entries/) of entries in 3d space.
//!
//! This module implements the [groupings](https://willowprotocol.org/specs/grouping-entries/) used by the Willow specifications:
//!
//! - [ranges](https://willowprotocol.org/specs/grouping-entries/#ranges) ([`WillowRange`]),
//! - [3d ranges](https://willowprotocol.org/specs/grouping-entries/#D3Range) ([`Range3d`]),
//! - [areas](https://willowprotocol.org/specs/grouping-entries/#areas) ([`Area`]), and
//! - [areas of interest](https://willowprotocol.org/specs/grouping-entries/#aois) ([`AreaOfInterest`]).
//!
//! Our implementations center around two traits: [`Coordinatelike`] describes values which fall into some position in three-dimensional Willow space, and [`Grouping`] describes some non-empty region of that space. The [`CoordinatelikeExt`] trait then provides convenient methods for checking which values are included in which groupings.
//!
//! ```
//! use willow25::prelude::*;
//!
//! let entry = Entry::builder()
//!     .namespace_id([0; 32].into())
//!     .subspace_id([1; 32].into())
//!     .path(path!("/vacation/plan"))
//!     .timestamp(12345)
//!     .payload_digest([77; 32].into())
//!     .payload_length(17)
//!     .build();
//!
//! assert!(entry.is_in(&Range3d::full()));
//! ```

use core::ops::RangeBounds;

use willow_data_model as wdm;

use crate::prelude::*;

pub use wdm::groupings::WillowRange;

mod keylike;
pub use keylike::*;

mod coordinatelike;
pub use coordinatelike::*;

mod namespaced;
pub use namespaced::*;

mod range_3d;
pub use range_3d::*;

mod area;
pub use area::*;

mod area_of_interest;
pub use area_of_interest::*;

pub mod private_context;

/// A [`WillowRange`] of [SubspaceIds](https://willowprotocol.org/specs/data-model/index.html#SubspaceId).
///
/// [Specification](https://willowprotocol.org/specs/grouping-entries/index.html#SubspaceRange)
pub type SubspaceRange = WillowRange<SubspaceId>;

/// A [`WillowRange`] of [Paths](https://willowprotocol.org/specs/data-model/index.html#Path).
///
/// [Specification](https://willowprotocol.org/specs/grouping-entries/index.html#PathRange)
pub type PathRange = WillowRange<Path>;

/// A [`WillowRange`] of [Timestamps](https://willowprotocol.org/specs/data-model/index.html#Timestamp).
///
/// [Specification](https://willowprotocol.org/specs/grouping-entries/index.html#TimeRange)
pub type TimeRange = wdm::groupings::TimeRange;

/// A grouping of entries.
///
/// A grouping describes a non-empty set of [`Coordinatelike`] values, with [`Grouping::includes`] providing the membership test. Groupings must implement [`PartialOrd`] as the subset relation.
pub trait Grouping: PartialOrd + Sized {
    /// Returns `true` iff the given [`Coordinatelike`] value is included in this grouping.
    fn includes<Coord>(&self, coord: &Coord) -> bool
    where
        Coord: Coordinatelike + ?Sized;

    /// Returns `true` iff every value [included](Grouping::includes) in `other` is also [included](Grouping::includes) in `self`.
    fn includes_grouping(&self, other: &Self) -> bool {
        self >= other
    }

    /// Returns `true` iff every value [included](Grouping::includes) in `other` is also [included](Grouping::includes) in `self` *and* `self` and `other` are not equal.
    fn strictly_includes_grouping(&self, other: &Self) -> bool {
        self > other
    }

    /// Returns a grouping which [includes](Grouping::includes) exactly those values [included](Grouping::includes) by both `self` and `other`.
    fn intersection(&self, other: &Self) -> Result<Self, wdm::prelude::EmptyGrouping>;

    /// Returns `true` iff the given [`Coordinatelike`] value is [included](Grouping::includes) in the [intersection](Grouping::intersection) of `self` and `other`.
    fn includes_in_intersection<Coord>(&self, other: &Self, coord: &Coord) -> bool
    where
        Coord: Coordinatelike + ?Sized,
    {
        self.includes(coord) && other.includes(coord)
    }
}

impl Grouping for WillowRange<SubspaceId> {
    fn includes<Coord>(&self, coord: &Coord) -> bool
    where
        Coord: Coordinatelike + ?Sized,
    {
        self.contains(coord.subspace_id())
    }

    fn intersection(&self, other: &Self) -> Result<Self, wdm::prelude::EmptyGrouping> {
        self.intersection_willow_range(other)
    }
}

impl Grouping for WillowRange<Path> {
    fn includes<Coord>(&self, coord: &Coord) -> bool
    where
        Coord: Coordinatelike + ?Sized,
    {
        self.contains(coord.path())
    }

    fn intersection(&self, other: &Self) -> Result<Self, wdm::prelude::EmptyGrouping> {
        self.intersection_willow_range(other)
    }
}

impl Grouping for WillowRange<Timestamp> {
    fn includes<Coord>(&self, coord: &Coord) -> bool
    where
        Coord: Coordinatelike + ?Sized,
    {
        self.contains(&coord.timestamp())
    }

    fn intersection(&self, other: &Self) -> Result<Self, wdm::prelude::EmptyGrouping> {
        self.intersection_willow_range(other)
    }
}
