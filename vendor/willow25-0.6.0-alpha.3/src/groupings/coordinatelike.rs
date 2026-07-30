use willow_data_model::prelude as wdm;

use crate::prelude::*;

/// A coordinatelike value represents a point in the [three-dimensional willow space](https://willowprotocol.org/specs/grouping-entries/index.html#D3Range) (within a given [namespace](https://willowprotocol.org/specs/data-model/index.html#namespace)).
pub trait Coordinatelike: Keylike + wdm::Coordinatelike<MCL, MCC, MPL, SubspaceId> {
    /// Returns the [timestamp](https://willowprotocol.org/specs/data-model/index.html#entry_timestamp) of `self`.
    fn timestamp(&self) -> Timestamp;
}

impl<T> Coordinatelike for T
where
    T: Keylike + wdm::Coordinatelike<MCL, MCC, MPL, SubspaceId> + ?Sized,
{
    fn timestamp(&self) -> Timestamp {
        self.wdm_timestamp()
    }
}

// SPT

impl wdm::Keylike<MCL, MCC, MPL, SubspaceId> for (SubspaceId, Path, Timestamp) {
    fn wdm_subspace_id(&self) -> &SubspaceId {
        &self.0
    }

    fn wdm_path(&self) -> &wdm::Path<MCL, MCC, MPL> {
        (&self.1).into()
    }
}

impl wdm::Coordinatelike<MCL, MCC, MPL, SubspaceId> for (SubspaceId, Path, Timestamp) {
    fn wdm_timestamp(&self) -> Timestamp {
        self.2
    }
}

// STP

impl wdm::Keylike<MCL, MCC, MPL, SubspaceId> for (SubspaceId, Timestamp, Path) {
    fn wdm_subspace_id(&self) -> &SubspaceId {
        &self.0
    }

    fn wdm_path(&self) -> &wdm::Path<MCL, MCC, MPL> {
        (&self.2).into()
    }
}

impl wdm::Coordinatelike<MCL, MCC, MPL, SubspaceId> for (SubspaceId, Timestamp, Path) {
    fn wdm_timestamp(&self) -> Timestamp {
        self.1
    }
}

// PST

impl wdm::Keylike<MCL, MCC, MPL, SubspaceId> for (Path, SubspaceId, Timestamp) {
    fn wdm_subspace_id(&self) -> &SubspaceId {
        &self.1
    }

    fn wdm_path(&self) -> &wdm::Path<MCL, MCC, MPL> {
        (&self.0).into()
    }
}

impl wdm::Coordinatelike<MCL, MCC, MPL, SubspaceId> for (Path, SubspaceId, Timestamp) {
    fn wdm_timestamp(&self) -> Timestamp {
        self.2
    }
}

// PTS

impl wdm::Keylike<MCL, MCC, MPL, SubspaceId> for (Path, Timestamp, SubspaceId) {
    fn wdm_subspace_id(&self) -> &SubspaceId {
        &self.2
    }

    fn wdm_path(&self) -> &wdm::Path<MCL, MCC, MPL> {
        (&self.0).into()
    }
}

impl wdm::Coordinatelike<MCL, MCC, MPL, SubspaceId> for (Path, Timestamp, SubspaceId) {
    fn wdm_timestamp(&self) -> Timestamp {
        self.1
    }
}

// TSP

impl wdm::Keylike<MCL, MCC, MPL, SubspaceId> for (Timestamp, SubspaceId, Path) {
    fn wdm_subspace_id(&self) -> &SubspaceId {
        &self.1
    }

    fn wdm_path(&self) -> &wdm::Path<MCL, MCC, MPL> {
        (&self.2).into()
    }
}

impl wdm::Coordinatelike<MCL, MCC, MPL, SubspaceId> for (Timestamp, SubspaceId, Path) {
    fn wdm_timestamp(&self) -> Timestamp {
        self.0
    }
}

// TPS

impl wdm::Keylike<MCL, MCC, MPL, SubspaceId> for (Timestamp, Path, SubspaceId) {
    fn wdm_subspace_id(&self) -> &SubspaceId {
        &self.2
    }

    fn wdm_path(&self) -> &wdm::Path<MCL, MCC, MPL> {
        (&self.1).into()
    }
}

impl wdm::Coordinatelike<MCL, MCC, MPL, SubspaceId> for (Timestamp, Path, SubspaceId) {
    fn wdm_timestamp(&self) -> Timestamp {
        self.0
    }
}

/// Methods for working with [`Coordinatelikes`](Coordinatelike).
///
/// This trait is automatically implemented by all types implementing [`Coordinatelike`].
pub trait CoordinatelikeExt: Coordinatelike + KeylikeExt {
    /// Returns whether `self` and `other` describe equal coordinates, i.e., whether their [subspace ids](Keylike::subspace_id), [paths](Keylike::path), and [timestamps](Coordinatelike::timestamp) are all equal.
    ///
    /// # Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// assert!(
    ///     (SubspaceId::from([0; SUBSPACE_ID_WIDTH]), path!(""), Timestamp::from(25))
    ///     .key_eq(&(SubspaceId::from([0; SUBSPACE_ID_WIDTH]), path!(""), Timestamp::from(25)))
    /// );
    /// assert!(
    ///     !(SubspaceId::from([0; SUBSPACE_ID_WIDTH]), path!(""), Timestamp::from(25))
    ///     .key_eq(&(SubspaceId::from([1; SUBSPACE_ID_WIDTH]), path!(""), Timestamp::from(25)))
    /// );
    /// assert!(
    ///     !(SubspaceId::from([0; SUBSPACE_ID_WIDTH]), path!(""), Timestamp::from(25))
    ///     .key_eq(&(SubspaceId::from([0; SUBSPACE_ID_WIDTH]), path!("/nope"), Timestamp::from(25)))
    /// );
    /// ```
    fn coordinate_eq<OtherCoordinate>(&self, other: &OtherCoordinate) -> bool
    where
        OtherCoordinate: Coordinatelike,
    {
        self.key_eq(other) && self.timestamp() == other.timestamp()
    }

    /// Returns whether `self` and `other` describe non-equal coordinates, i.e., whether their [subspace ids](Keylike::subspace_id), [paths](Keylike::path), and [timestamps](Coordinatelike::timestamp) are not all equal.
    ///
    /// # Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// assert!(
    ///     !(SubspaceId::from([0; SUBSPACE_ID_WIDTH]), path!(""), Timestamp::from(25))
    ///         .coordinate_ne(
    ///             &(SubspaceId::from([0; SUBSPACE_ID_WIDTH]), path!(""), Timestamp::from(25)),
    ///         )
    /// );
    /// assert!(
    ///     (SubspaceId::from([0; SUBSPACE_ID_WIDTH]), path!(""), Timestamp::from(25))
    ///         .coordinate_ne(
    ///             &(SubspaceId::from([1; SUBSPACE_ID_WIDTH]), path!(""), Timestamp::from(25))
    ///         )
    /// );
    /// assert!(
    ///     (SubspaceId::from([0; SUBSPACE_ID_WIDTH]), path!(""), Timestamp::from(25))
    ///         .coordinate_ne(
    ///             &(SubspaceId::from([0; SUBSPACE_ID_WIDTH]), path!("/nope"), Timestamp::from(25))
    ///         )
    /// );
    /// ```
    fn coordinate_ne<OtherCoordinate>(&self, other: &OtherCoordinate) -> bool
    where
        OtherCoordinate: Coordinatelike,
    {
        self.key_ne(other) || self.timestamp() != other.timestamp()
    }

    /// Returns whether `self` is included in the given [`Grouping`].
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// assert!(
    ///     (SubspaceId::from([5; 32]), Path::new(), Timestamp::from(9))
    ///         .is_in(&Range3d::full())
    /// );
    /// ```
    fn is_in<G>(&self, grouping: &G) -> bool
    where
        G: Grouping,
    {
        grouping.includes(self)
    }

    /// Returns whether `self` is included in the intersection of the two given [`Groupings`](Grouping).
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let r1 = Range3d::new(
    ///     SubspaceRange::full(),
    ///     PathRange::full(),
    ///     TimeRange::new_closed(0.into(), 17.into()),
    /// );
    /// let r2 = Range3d::new(
    ///     SubspaceRange::full(),
    ///     PathRange::full(),
    ///     TimeRange::new_open(15.into()),
    /// );
    ///
    /// assert!(
    ///     (SubspaceId::from([5; 32]), Path::new(), Timestamp::from(16))
    ///         .is_in_intersection(&r1, &r2)
    /// );
    /// assert!(
    ///     !(SubspaceId::from([5; 32]), Path::new(), Timestamp::from(29))
    ///         .is_in_intersection(&r1, &r2)
    /// );
    /// ```
    fn is_in_intersection<G>(&self, grouping1: &G, grouping2: &G) -> bool
    where
        G: Grouping,
    {
        grouping1.includes_in_intersection(grouping2, self)
    }
}

impl<T> CoordinatelikeExt for T where T: Coordinatelike + ?Sized {}
