//! Functionality around Willow [Paths](https://willowprotocol.org/specs/data-model/index.html#Path).
//!
//! The central type of this module is the [`Path`] struct, which represents a single willow Path. [`Paths`](Path) are *immutable*, which means any operation that would traditionally mutate paths (for example, appending a new component) returns a new, independent value instead.
//!
//! All (safely created) [`Paths`](Path) have a total length of at most 4096 ([`MPL`]) bytes and consist of no more than 4096 ([`MCC`]) components. The functions in this module return [`PathErrors`](PathError) or [`PathFromComponentsErrors`](PathFromComponentsError) when those invariants would be violated.
//!
//! You can conveniently create well-known paths with the [`path!`] macro. The type-level docs of [`Path`] list all the ways in which you can build up paths dynamically.
//!
//! The [`Path`] struct provides methods for checking various properties of paths: from simple properties ("[how many components does this have](Path::component_count)") to various comparisons ("[is this path a prefix of some other](Path::is_prefix_of)"), the methods should have you covered.
//!
//! Because [path prefixes](https://willowprotocol.org/specs/data-model/index.html#path_prefix) play such a [central role for deletion](https://willowprotocol.org/specs/data-model/index.html#prefix_pruning) in Willow, the functionality around prefixes is optimised. [Creating a prefix](Path::create_prefix) or even iterating over [all prefixes](Path::all_prefixes) performs no memory allocations.
//!
//! The [`Component`] type represents individual path [Components](https://willowprotocol.org/specs/data-model/index.html#Component). This type is a thin wrapper around `[u8]` (enforcing a maximal length of 4096 bytes); like `[u8]` it must always be used as part of a pointer type — for example, `&Component`. Alternatively, the [`OwnedComponent`] type can be used by itself — keeping an [`OwnedComponent`] alive will also keep around the heap allocation for the full [`Path`] from which it was constructed, though. Generally speaking, the more lightweight [`Component`] should be preferred over [`OwnedComponent`] where possible.

use core::cmp::Ordering;
use core::fmt;
use core::ops::RangeBounds;

#[cfg(feature = "dev")]
use arbitrary::Arbitrary;

use order_theory::GreatestElement;
use order_theory::LeastElement;
use order_theory::LowerSemilattice;
use order_theory::PredecessorExceptForLeast;
use order_theory::SuccessorExceptForGreatest;
use order_theory::TryPredecessor;
use order_theory::TrySuccessor;
use order_theory::UpperSemilattice;
use willow_data_model::prelude as wdm;

mod builder;
pub use builder::PathBuilder;

mod component;
pub use component::*;

mod codec;
pub use codec::*;

pub mod private_context;

use crate::prelude::*;

pub use crate::component;
pub use crate::path;

wrapper! {
    /// An immutable Willow [Path](https://willowprotocol.org/specs/data-model/index.html#Path). Thread-safe, cheap to clone, cheap to take prefixes of, expensive to append to (linear time complexity).
    ///
    /// A Willow25 Path is any sequence of bytestrings — called [Components](https://willowprotocol.org/specs/data-model/index.html#Component) — fulfilling certain constraints:
    ///
    /// - each [`Component`] has a length of at most 4096 ([`MCL`]),
    /// - each [`Path`] has at most 4096 ([`MCC`]) components, and
    /// - the total size in bytes of all [`Component`]s is at most 4096 ([`MPL`]).
    ///
    /// This type statically enforces these invariants for all (safely) created instances.
    ///
    /// Because appending [`Component`]s takes time linear in the length of the full [`Path`], you should not build up [`Path`]s this way. Instead, use [`Path::from_components`], [`Path::from_slices`], [`Path::from_components_iter`], [`Path::from_slices_iter`], or a [`PathBuilder`]. If you statically know the path in advance, use the [`path!`](path) macro.
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let p: Path = path!("/hi/ho");
    ///
    /// assert_eq!(p.component_count(), 2);
    /// assert_eq!(p.component(1), Some(Component::new(b"ho")?));
    /// assert_eq!(p.total_length(), 4);
    /// assert!(p.is_prefixed_by(&Path::from_slice(b"hi")?));
    /// assert_eq!(
    ///     p.longest_common_prefix(&path!("/hi/he")),
    ///     Path::from_slice(b"hi")?
    /// );
    /// # Ok::<(), PathError>(())
    /// ```
    #[derive(Clone, PartialEq, Eq, core::hash::Hash, Default)]
    #[cfg_attr(feature = "dev", derive(Arbitrary))]
    Path; wdm::Path<MCL, MCC, MPL>
}

impl fmt::Debug for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Path {
    /// Returns an empty [`Path`], i.e., a [`Path`] of zero [`Component`]s.
    ///
    /// #### Complexity
    ///
    /// Runs in `O(1)`.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// assert_eq!(Path::new().component_count(), 0);
    /// assert_eq!(Path::new(), Path::default());
    /// assert_eq!(Path::new(), path!(""));
    /// ```
    pub fn new() -> Self {
        Self(wdm::Path::new())
    }

    /// Creates a singleton [`Path`], consisting of exactly one [`Component`].
    ///
    /// Copies the bytes of the [`Component`] into an owned allocation on the heap.
    ///
    /// #### Complexity
    ///
    /// Runs in `O(n)`, where `n` is the length of the [`Component`]. Performs a single allocation of `O(n)` bytes.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let p = Path::from_component(component!("hi"))?;
    /// assert_eq!(p.component_count(), 1);
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn from_component(comp: &Component) -> Result<Self, PathFromComponentsError> {
        Ok(Self(wdm::Path::from_component(comp.into())?))
    }

    /// Creates a singleton [`Path`], consisting of exactly one [`Component`], from a raw slice of bytes.
    ///
    /// Copies the bytes into an owned allocation on the heap.
    ///
    /// #### Complexity
    ///
    /// Runs in `O(n)`, where `n` is the length of the [`Component`]. Performs a single allocation of `O(n)` bytes.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let p = Path::from_slice(b"hi")?;
    /// assert_eq!(p.component_count(), 1);
    ///
    /// assert_eq!(
    ///     Path::from_slice(b"too_loooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooong"),
    ///     Err(PathError::ComponentTooLong),
    /// );
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn from_slice(comp: &[u8]) -> Result<Self, PathError> {
        Ok(Self(wdm::Path::from_slice(comp)?))
    }

    /// Creates a [`Path`] from a slice of [`Component`]s.
    ///
    /// Copies the bytes of the [`Component`]s into an owned allocation on the heap.
    ///
    /// #### Complexity
    ///
    /// Runs in `O(n + m)`, where `n` is the total length of the created [`Path`] in bytes, and `m` is the number of its [`Component`]s. Performs a single allocation of `O(n + m)` bytes.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let components: Vec<&Component> = vec![
    ///     component!("hi"),
    ///     component!("ho"),
    /// ];
    ///
    /// assert!(Path::from_components(&components[..]).is_ok());
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn from_components(components: &[&Component]) -> Result<Self, PathFromComponentsError> {
        let mut total_length = 0;
        for comp in components {
            total_length += comp.len();
        }

        Self::from_components_iter(total_length, &mut components.iter().cloned())
    }

    /// Create a new [`Path`] from a slice of byte slices.
    ///
    /// #### Complexity
    ///
    /// Runs in `O(n + m)`, where `n` is the total length of the created [`Path`] in bytes, and `m` is the number of its [`Component`]s. Performs a single allocation of `O(n + m)` bytes.
    ///
    /// # Example
    ///
    /// ```
    /// use willow25::prelude::*;
    /// // Ok
    /// let path = Path::from_slices(&[b"alfie", b"notes"]).unwrap();
    ///
    /// // Err
    /// let result1 = Path::from_slices(&[
    ///   b"tooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooo",
    ///   b"looooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooong",
    /// ]);
    /// assert_eq!(result1, Err(PathError::PathTooLong));
    ///
    /// // Err
    /// let result2 = Path::from_slices(vec![b"".as_slice(); MCC + 1].as_slice());
    /// assert_eq!(result2, Err(PathError::TooManyComponents));
    /// ```
    pub fn from_slices(slices: &[&[u8]]) -> Result<Self, PathError> {
        Ok(Self(wdm::Path::from_slices(slices)?))
    }

    /// Creates a [`Path`] of known total length from an [`ExactSizeIterator`] of [`Component`]s.
    ///
    /// Copies the bytes of the [`Component`]s into an owned allocation on the heap.
    ///
    /// Panics if the claimed `total_length` does not match the sum of the lengths of all the [`Component`]s.
    ///
    /// #### Complexity
    ///
    /// Runs in `O(n + m)`, where `n` is the total length of the created [`Path`] in bytes, and `m` is the number of its [`Component`]s. Performs a single allocation of `O(n + m)` bytes.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let components: Vec<&Component> = vec![
    ///     component!("hi"),
    ///     component!("ho"),
    /// ];
    ///
    /// assert!(Path::from_components_iter(4, &mut components.into_iter()).is_ok());
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn from_components_iter<'c, I>(
        total_length: usize,
        iter: &mut I,
    ) -> Result<Self, PathFromComponentsError>
    where
        I: ExactSizeIterator<Item = &'c Component>,
    {
        let mut builder = PathBuilder::new(total_length, iter.len())?;

        for component in iter {
            builder.append_component(component);
        }

        Ok(builder.build())
    }

    /// Creates a [`Path`] of known total length from an [`ExactSizeIterator`] of byte slices.
    ///
    /// Copies the bytes of the [`Component`]s into an owned allocation on the heap.
    ///
    /// Panics if the claimed `total_length` does not match the sum of the lengths of all the [`Component`]s.
    ///
    /// #### Complexity
    ///
    /// Runs in `O(n + m)`, where `n` is the total length of the created [`Path`] in bytes, and `m` is the number of its [`Component`]s. Performs a single allocation of `O(n + m)` bytes.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let components: Vec<&[u8]> = vec![b"hi", b"!"];
    /// assert!(Path::from_slices_iter(3, &mut components.into_iter()).is_ok());
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn from_slices_iter<'c, I>(total_length: usize, iter: &mut I) -> Result<Self, PathError>
    where
        I: ExactSizeIterator<Item = &'c [u8]>,
    {
        Ok(Self(wdm::Path::from_slices_iter(total_length, iter)?))
    }

    /// Creates a new [`Path`] by appending a [`Component`] to `&self`.
    ///
    /// Creates a fully separate copy of the new data on the heap; which includes cloning all data in `&self`. To efficiently construct [`Path`]s, use [`Path::from_components`], [`Path::from_slices`], [`Path::from_components_iter`], [`Path::from_slices_iter`], or a [`PathBuilder`].
    ///
    /// #### Complexity
    ///
    /// Runs in `O(n + m)`, where `n` is the total length of the resulting [`Path`] in bytes, and `m` is the number of its [`Component`]s. Performs a single allocation of `O(n + m)` bytes.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let p0: Path = Path::new();
    /// let p1 = p0.append_component(component!("hi"))?;
    /// let p2 = p1.append_component(component!("ho"))?;
    /// assert_eq!(
    ///     p2.append_component(component!("too_looooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooong")),
    ///     Err(PathFromComponentsError::PathTooLong),
    /// );
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn append_component(&self, comp: &Component) -> Result<Self, PathFromComponentsError> {
        Ok(Self(self.0.append_component(comp.into())?))
    }

    /// Creates a new [`Path`] by appending a [`Component`] to `&self`.
    ///
    /// Creates a fully separate copy of the new data on the heap; which includes cloning all data in `&self`. To efficiently construct [`Path`]s, use [`Path::from_components`], [`Path::from_slices`], [`Path::from_components_iter`], [`Path::from_slices_iter`], or a [`PathBuilder`].
    ///
    /// #### Complexity
    ///
    /// Runs in `O(n + m)`, where `n` is the total length of the resulting [`Path`] in bytes, and `m` is the number of its [`Component`]s. Performs a single allocation of `O(n + m)` bytes.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let p0: Path = Path::new();
    /// let p1 = p0.append_slice(b"hi")?;
    /// let p2 = p1.append_slice(b"ho")?;
    /// assert_eq!(
    ///     p2.append_slice(b"too_looooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooong"),
    ///     Err(PathError::PathTooLong),
    /// );
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn append_slice(&self, comp: &[u8]) -> Result<Self, PathError> {
        Ok(Self(self.0.append_slice(comp)?))
    }

    /// Creates a new [`Path`] by appending a slice of [`Component`]s to `&self`.
    ///
    /// Creates a fully separate copy of the new data on the heap; which includes cloning all data in `&self`. To efficiently construct [`Path`]s, use [`Path::from_components`], [`Path::from_slices`], [`Path::from_components_iter`], [`Path::from_slices_iter`], or a [`PathBuilder`].
    ///
    /// #### Complexity
    ///
    /// Runs in `O(n + m)`, where `n` is the total length of the resulting [`Path`] in bytes, and `m` is the number of its [`Component`]s. Performs a single allocation of `O(n + m)` bytes.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let p0: Path = Path::new();
    /// let p1 = p0.append_components(&[component!("hi"), component!("ho")])?;
    /// assert_eq!(
    ///     p1.append_components(&[Component::new(b"too_looooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooong")?]),
    ///     Err(PathFromComponentsError::PathTooLong),
    /// );
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn append_components(
        &self,
        components: &[&Component],
    ) -> Result<Self, PathFromComponentsError> {
        let mut total_length = self.total_length();
        for comp in components {
            total_length += comp.len();
        }

        let mut builder = PathBuilder::new_from_prefix(
            total_length,
            self.component_count() + components.len(),
            self,
            self.component_count(),
        )?;

        for additional_component in components {
            builder.append_component(additional_component);
        }

        Ok(builder.build())
    }

    /// Creates a new [`Path`] by appending a slice of [`Component`]s to `&self`.
    ///
    /// Creates a fully separate copy of the new data on the heap; which includes cloning all data in `&self`. To efficiently construct [`Path`]s, use [`Path::from_components`], [`Path::from_slices`], [`Path::from_components_iter`], [`Path::from_slices_iter`], or a [`PathBuilder`].
    ///
    /// #### Complexity
    ///
    /// Runs in `O(n + m)`, where `n` is the total length of the resulting [`Path`] in bytes, and `m` is the number of its [`Component`]s. Performs a single allocation of `O(n + m)` bytes.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let p0: Path = Path::new();
    /// let p1 = p0.append_slices(&[b"hi", b"ho"])?;
    /// assert_eq!(
    ///     p1.append_slices(&[b"too_looooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooong"]),
    ///     Err(PathError::PathTooLong),
    /// );
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn append_slices(&self, components: &[&[u8]]) -> Result<Self, PathError> {
        Ok(Self(self.0.append_slices(components)?))
    }

    /// Creates a new [`Path`] by appending another [`Path`] to `&self`.
    ///
    /// Creates a fully separate copy of the new data on the heap; which includes cloning all data in `&self` and in `&other`.
    ///
    /// #### Complexity
    ///
    /// Runs in `O(n + m)`, where `n` is the total length of the resulting [`Path`] in bytes, and `m` is the number of its [`Component`]s. Performs a single allocation of `O(n + m)` bytes.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let p0: Path = Path::new();
    /// let p1 = p0.append_path(&path!("/hi/ho"))?;
    /// assert_eq!(
    ///     p1.append_path(&Path::from_slice(b"too_looooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooong")?),
    ///     Err(PathFromComponentsError::PathTooLong),
    /// );
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn append_path(&self, other: &Path) -> Result<Self, PathFromComponentsError> {
        Ok(Self(self.0.append_path(other.into())?))
    }

    /// Returns the least [`Path`] which is strictly greater (lexicographically) than `self` and which is not [prefixed by](https://willowprotocol.org/specs/data-model/index.html#path_prefix) `self`; or [`None`] if no such [`Path`] exists.
    ///
    /// #### Complexity
    ///
    /// Runs in `O(n + m)`, where `n` is the total length of the [`Path`] in bytes, and `m` is the number of [`Component`]s. Performs a single allocation of `O(n + m)` bytes.
    pub fn greater_but_not_prefixed(&self) -> Option<Self> {
        self.0.greater_but_not_prefixed().map(Self)
    }

    /// Returns the number of [`Component`]s in `&self`.
    ///
    /// Guaranteed to be at most `MCC`.
    ///
    /// #### Complexity
    ///
    /// Runs in `O(1)`, performs no allocations.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let p: Path = path!("/hi/ho");
    /// assert_eq!(p.component_count(), 2);
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn component_count(&self) -> usize {
        self.0.component_count()
    }

    /// Returns whether `&self` has zero [`Component`]s.
    ///
    /// #### Complexity
    ///
    /// Runs in `O(1)`, performs no allocations.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// assert_eq!(Path::new().is_empty(), true);
    ///
    /// let p: Path = path!("/hi/ho");
    /// assert_eq!(p.is_empty(), false);
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns the sum of the lengths of all [`Component`]s in `&self`.
    ///
    /// Guaranteed to be at most `MCC`.
    ///
    /// #### Complexity
    ///
    /// Runs in `O(1)`, performs no allocations.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let p: Path = path!("/hi/ho");
    /// assert_eq!(p.total_length(), 4);
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn total_length(&self) -> usize {
        self.0.total_length()
    }

    /// Returns the sum of the lengths of the first `i` [`Component`]s in `&self`; panics if `i >= self.component_count()`. More efficient than `path.create_prefix(i).path_length()`.
    ///
    /// Guaranteed to be at most `MCC`.
    ///
    /// #### Complexity
    ///
    /// Runs in `O(1)`, performs no allocations.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let p: Path = path!("/hi/ho");
    /// assert_eq!(p.total_length_of_prefix(0), 0);
    /// assert_eq!(p.total_length_of_prefix(1), 2);
    /// assert_eq!(p.total_length_of_prefix(2), 4);
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn total_length_of_prefix(&self, i: usize) -> usize {
        self.0.total_length_of_prefix(i)
    }

    /// Tests whether `&self` is a [prefix](https://willowprotocol.org/specs/data-model/index.html#path_prefix) of the given [`Path`].
    /// [`Path`]s are always a prefix of themselves, and the empty [`Path`] is a prefix of every [`Path`].
    ///
    /// #### Complexity
    ///
    /// Runs in `O(n + m)`, where `n` is the total length of the longer [`Path`] in bytes, and `m` is the greatest number of [`Component`]s.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let p: Path = path!("/hi/ho");
    /// assert!(Path::new().is_prefix_of(&p));
    /// assert!(path!("/hi").is_prefix_of(&p));
    /// assert!(path!("/hi/ho").is_prefix_of(&p));
    /// assert!(!path!("/hi/gh").is_prefix_of(&p));
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn is_prefix_of(&self, other: &Self) -> bool {
        self.0.is_prefix_of(other.into())
    }

    /// Tests whether `&self` is [prefixed by](https://willowprotocol.org/specs/data-model/index.html#path_prefix) the given [`Path`].
    /// [`Path`]s are always prefixed by themselves, and by the empty [`Path`].
    ///
    /// #### Complexity
    ///
    /// Runs in `O(n + m)`, where `n` is the total length of the longer [`Path`] in bytes, and `m` is the greatest number of [`Component`]s.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let p: Path = path!("/hi/ho");
    /// assert!(p.is_prefixed_by(&Path::new()));
    /// assert!(p.is_prefixed_by(&path!("/hi")));
    /// assert!(p.is_prefixed_by(&path!("/hi/ho")));
    /// assert!(!p.is_prefixed_by(&path!("/hi/gh")));
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn is_prefixed_by(&self, other: &Self) -> bool {
        self.0.is_prefixed_by(other.into())
    }

    /// Tests whether `&self` is [related](https://willowprotocol.org/specs/data-model/index.html#path_related) to the given [`Path`], that is, whether either one is a [prefix](https://willowprotocol.org/specs/data-model/index.html#path_prefix) of the other.
    ///
    /// #### Complexity
    ///
    /// Runs in `O(n + m)`, where `n` is the total length of the longer [`Path`] in bytes, and `m` is the greatest number of [`Component`]s.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let p: Path = Path::from_slice(b"hi")?;
    /// assert!(p.is_related_to(&Path::new()));
    /// assert!(p.is_related_to(&path!("/hi")));
    /// assert!(p.is_related_to(&path!("/hi/ho")));
    /// assert!(!p.is_related_to(&path!("/no")));
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn is_related_to(&self, other: &Self) -> bool {
        self.0.is_related_to(other.into())
    }

    /// Returns the [`Ordering`] describing the prefix relation between `self` and `other`.
    ///
    /// #### Complexity
    ///
    /// Runs in `O(n + m)`, where `n` is the total length of the longer [`Path`] in bytes, and `m` is the greatest number of [`Component`]s.
    ///
    /// #### Examples
    ///
    /// ```
    /// use core::cmp::Ordering;
    /// use willow25::prelude::*;
    /// let p: Path = Path::from_slice(b"hi")?;
    /// assert_eq!(p.prefix_cmp(&Path::new()), Some(Ordering::Greater));
    /// assert_eq!(p.prefix_cmp(&path!("/hi")), Some(Ordering::Equal));
    /// assert_eq!(p.prefix_cmp(&path!("/hi/ho")), Some(Ordering::Less));
    /// assert_eq!(p.prefix_cmp(&path!("/no")), None);
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn prefix_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.prefix_cmp(other.into())
    }

    /// Returns the `i`-th [`Component`] of `&self`.
    ///
    /// #### Complexity
    ///
    /// Runs in `O(1)`, performs no allocations.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let p: Path = path!("/hi/ho");
    /// assert_eq!(p.component(0), Some(component!("hi")));
    /// assert_eq!(p.component(1), Some(Component::new(b"ho")?));
    /// assert_eq!(p.component(2), None);
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn component(&self, i: usize) -> Option<&Component> {
        self.0
            .component(i)
            .map(|wdm_comp| unsafe { Component::new_unchecked(wdm_comp) })
    }

    /// Returns the `i`-th [`Component`] of `&self`, without checking whether `i < self.component_count()`.
    ///
    /// #### Safety
    ///
    /// Undefined behaviour if `i >= self.component_count()`.
    ///
    /// #### Complexity
    ///
    /// Runs in `O(1)`, performs no allocations.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let p: Path = path!("/hi/ho");
    /// assert_eq!(unsafe { p.component_unchecked(0) }, component!("hi"));
    /// assert_eq!(unsafe { p.component_unchecked(1) }, Component::new(b"ho")?);
    /// # Ok::<(), PathError>(())
    /// ```
    pub unsafe fn component_unchecked(&self, i: usize) -> &Component {
        unsafe { Component::new_unchecked(self.0.component_unchecked(i)) }
    }

    /// Returns an [owned handle](OwnedComponent) to the `i`-th component of `&self`.
    ///
    /// #### Complexity
    ///
    /// Runs in `O(1)`, performs no allocations.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let p: Path = path!("/hi/ho");
    /// assert_eq!(p.owned_component(0), Some(OwnedComponent::new(b"hi")?));
    /// assert_eq!(p.owned_component(1), Some(OwnedComponent::new(b"ho")?));
    /// assert_eq!(p.owned_component(2), None);
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn owned_component(&self, i: usize) -> Option<OwnedComponent> {
        self.0.owned_component(i).map(From::from)
    }

    /// Returns an [owned handle](OwnedComponent) to the `i`-th component of `&self`, without checking whether `i < self.component_count()`.
    ///
    /// #### Safety
    ///
    /// Undefined behaviour if `i >= self.component_count()`.
    ///
    /// #### Complexity
    ///
    /// Runs in `O(1)`, performs no allocations.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let p: Path = path!("/hi/ho");
    /// assert_eq!(p.owned_component(0), Some(OwnedComponent::new(b"hi")?));
    /// assert_eq!(p.owned_component(1), Some(OwnedComponent::new(b"ho")?));
    /// assert_eq!(p.owned_component(2), None);
    /// # Ok::<(), PathError>(())
    /// ```
    pub unsafe fn owned_component_unchecked(&self, i: usize) -> OwnedComponent {
        unsafe { self.0.owned_component_unchecked(i).into() }
    }

    /// Creates an iterator over the [`Component`]s of `&self`.
    ///
    /// Stepping the iterator takes `O(1)` time and performs no memory allocations.
    ///
    /// #### Complexity
    ///
    /// Runs in `O(1)`, performs no allocations.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let p: Path = path!("/hi/ho");
    /// let mut comps = p.components();
    /// assert_eq!(comps.next(), Some(component!("hi")));
    /// assert_eq!(comps.next(), Some(Component::new(b"ho")?));
    /// assert_eq!(comps.next(), None);
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn components(
        &self,
    ) -> impl DoubleEndedIterator<Item = &Component> + ExactSizeIterator<Item = &Component> {
        self.suffix_components(0)
    }

    /// Creates an iterator over the [`Component`]s of `&self`, starting at the `i`-th [`Component`]. If `i` is greater than or equal to the number of [`Component`]s, the iterator yields zero items.
    ///
    /// Stepping the iterator takes `O(1)` time and performs no memory allocations.
    ///
    /// #### Complexity
    ///
    /// Runs in `O(1)`, performs no allocations.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let p: Path = path!("/hi/ho");
    /// let mut comps = p.suffix_components(1);
    /// assert_eq!(comps.next(), Some(Component::new(b"ho")?));
    /// assert_eq!(comps.next(), None);
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn suffix_components(
        &'_ self,
        i: usize,
    ) -> impl DoubleEndedIterator<Item = &Component> + ExactSizeIterator<Item = &Component> {
        (i..self.component_count()).map(|i| {
            self.component(i).unwrap() // Only `None` if `i >= self.component_count()`
        })
    }

    /// Creates an iterator over [owned handles](OwnedComponent) to the components of `&self`.
    ///
    /// Stepping the iterator takes `O(1)` time and performs no memory allocations.
    ///
    /// #### Complexity
    ///
    /// Runs in `O(1)`, performs no allocations.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let p: Path = path!("/hi/ho");
    /// let mut comps = p.owned_components();
    /// assert_eq!(comps.next(), Some(OwnedComponent::new(b"hi")?));
    /// assert_eq!(comps.next(), Some(OwnedComponent::new(b"ho")?));
    /// assert_eq!(comps.next(), None);
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn owned_components(
        &self,
    ) -> impl DoubleEndedIterator<Item = OwnedComponent> + ExactSizeIterator<Item = OwnedComponent> + '_
    {
        self.suffix_owned_components(0)
    }

    /// Creates an iterator over [owned handles](OwnedComponent) to the components of `&self`, starting at the `i`-th [`OwnedComponent`]. If `i` is greater than or equal to the number of [`OwnedComponent`], the iterator yields zero items.
    ///
    /// Stepping the iterator takes `O(1)` time and performs no memory allocations.
    ///
    /// #### Complexity
    ///
    /// Runs in `O(1)`, performs no allocations.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let p: Path = path!("/hi/ho");
    /// let mut comps = p.suffix_owned_components(1);
    /// assert_eq!(comps.next(), Some(OwnedComponent::new(b"ho")?));
    /// assert_eq!(comps.next(), None);
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn suffix_owned_components(
        &self,
        i: usize,
    ) -> impl DoubleEndedIterator<Item = OwnedComponent> + ExactSizeIterator<Item = OwnedComponent> + '_
    {
        (i..self.component_count()).map(|i| {
            self.owned_component(i).unwrap() // Only `None` if `i >= self.component_count()`
        })
    }

    /// Creates a new [`Path`] that consists of the first `component_count` [`Component`]s of `&self`. More efficient than creating a new [`Path`] from scratch.
    ///
    /// Returns [`None`] if `component_count` is greater than `self.get_component_count()`.
    ///
    /// #### Complexity
    ///
    /// Runs in `O(1)`, performs no allocations.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let p: Path = path!("/hi/ho");
    /// assert_eq!(p.create_prefix(0), Some(Path::new()));
    /// assert_eq!(p.create_prefix(1), Some(Path::from_slice(b"hi")?));
    /// assert_eq!(p.create_prefix(2), Some(path!("/hi/ho")));
    /// assert_eq!(p.create_prefix(3), None);
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn create_prefix(&self, component_count: usize) -> Option<Self> {
        self.0.create_prefix(component_count).map(Self)
    }

    /// Creates a new [`Path`] that consists of the first `component_count` [`Component`]s of `&self`. More efficient than creating a new [`Path`] from scratch.
    ///
    /// #### Safety
    ///
    /// Undefined behaviour if `component_count` is greater than `self.component_count()`. May manifest directly, or at any later
    /// function invocation that operates on the resulting [`Path`].
    ///
    /// #### Complexity
    ///
    /// Runs in `O(1)`, performs no allocations.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let p: Path = path!("/hi/ho");
    /// assert_eq!(unsafe { p.create_prefix_unchecked(0) }, Path::new());
    /// assert_eq!(unsafe { p.create_prefix_unchecked(1) }, Path::from_slice(b"hi")?);
    /// assert_eq!(unsafe { p.create_prefix_unchecked(2) }, path!("/hi/ho"));
    /// # Ok::<(), PathError>(())
    /// ```
    pub unsafe fn create_prefix_unchecked(&self, component_count: usize) -> Self {
        unsafe { self.0.create_prefix_unchecked(component_count).into() }
    }

    /// Creates an iterator over all [prefixes](https://willowprotocol.org/specs/data-model/index.html#path_prefix) of `&self` (including the empty [`Path`] and `&self` itself).
    ///
    /// Stepping the iterator takes `O(1)` time and performs no memory allocations.
    ///
    /// #### Complexity
    ///
    /// Runs in `O(1)`, performs no allocations.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let p: Path = path!("/hi/ho");
    /// let mut prefixes = p.all_prefixes();
    /// assert_eq!(prefixes.next(), Some(Path::new()));
    /// assert_eq!(prefixes.next(), Some(Path::from_slice(b"hi")?));
    /// assert_eq!(prefixes.next(), Some(path!("/hi/ho")));
    /// assert_eq!(prefixes.next(), None);
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn all_prefixes(&self) -> impl DoubleEndedIterator<Item = Self> + '_ {
        (0..=self.component_count()).map(|i| {
            unsafe {
                self.create_prefix_unchecked(i) // safe to call for i <= self.component_count()
            }
        })
    }

    /// Returns the longest common [prefix](https://willowprotocol.org/specs/data-model/index.html#path_prefix) of `&self` and the given [`Path`].
    ///
    /// #### Complexity
    ///
    /// Runs in `O(n + m)`, where `n` is the total length of the shorter of the two [`Path`]s, and `m` is the lesser number of [`Component`]s. Performs a single allocation of `O(n + m)` bytes to create the return value.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let p1: Path = path!("/hi/ho");
    /// let p2: Path = path!("/hi/he");
    /// assert_eq!(p1.longest_common_prefix(&p2), Path::from_slice(b"hi")?);
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn longest_common_prefix(&self, other: &Self) -> Self {
        Self(self.0.longest_common_prefix(other.into()))
    }

    /// Creates a [`Path`] whose [`Component`]s are those of `&self` indexed by the given `range`, without checking that those components exist.
    ///
    /// #### Safety
    ///
    /// Undefined behaviour if either the start or the end of `range` are explicitly greater than `self.component_count()`, or if `range` is decreasing.
    ///
    /// #### Complexity
    ///
    /// Runs in `O(1)` and performs no allocations.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let p = path!("/hi/ho");
    /// assert_eq!(unsafe{p.create_slice_unchecked(..)}, p);
    /// assert_eq!(unsafe{p.create_slice_unchecked(..1)}, path!("/hi"));
    /// assert_eq!(unsafe{p.create_slice_unchecked(1..)}, path!("/ho"));
    /// assert_eq!(unsafe{p.create_slice_unchecked(1..1)}, Path::new());
    /// # Ok::<(), PathError>(())
    /// ```
    pub unsafe fn create_slice_unchecked<R>(&self, range: R) -> Self
    where
        R: RangeBounds<usize>,
    {
        unsafe { Self(self.0.create_slice_unchecked(range)) }
    }

    /// Creates a [`Path`] whose [`Component`]s are those of `&self` indexed by the given `range`.
    /// Returns [`None`] if either the start or end of `range` are explicitly greater than `&self.component_count()` or if `range` is decreasing.
    ///
    /// #### Complexity
    ///
    /// Runs in `O(1)` and performs no allocations.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let p = path!("/hi/ho");
    /// assert_eq!(p.create_slice(..), Some(p.clone()));
    /// assert_eq!(p.create_slice(1..2), Some(path!("/ho")));
    /// assert_eq!(p.create_slice(1..3), None);
    /// assert_eq!(p.create_slice(2..1), None);
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn create_slice<R>(&self, range: R) -> Option<Self>
    where
        R: RangeBounds<usize>,
    {
        self.0.create_slice(range).map(Self)
    }

    /// Creates a [`Path`] whose [`Component`]s are the last `component_count` components of `&self`, without checking that `&self` has at least `component_count` components.
    ///
    /// #### Safety
    ///
    /// Undefined behaviour if `component_count > self.component_count`.
    ///
    /// #### Complexity
    ///
    /// Runs in `O(1)` and performs no allocations.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let p = path!("/a/b/c/d");
    /// assert_eq!(unsafe{p.create_suffix_unchecked(2)}, path!("/c/d"));
    /// assert_eq!(unsafe{p.create_suffix_unchecked(0)}, Path::new());
    /// # Ok::<(), PathError>(())
    /// ```
    pub unsafe fn create_suffix_unchecked(&self, component_count: usize) -> Self {
        unsafe { Self(self.0.create_suffix_unchecked(component_count)) }
    }

    /// Creates a [`Path`] whose [`Component`]s are the last `component_count` components of `&self`.
    /// Returns [`None`] if `&self` does not have at least `component_count` components.
    ///
    /// #### Complexity
    ///
    /// Runs in `O(1)` and performs no allocations.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let p = path!("/hi/ho");
    /// assert_eq!(p.create_suffix(0), Some(Path::new()));
    /// assert_eq!(p.create_suffix(1), Some(path!("/ho")));
    /// assert_eq!(p.create_suffix(2), Some(path!("/hi/ho")));
    /// assert_eq!(p.create_suffix(3), None);
    ///
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn create_suffix(&self, component_count: usize) -> Option<Self> {
        self.0.create_suffix(component_count).map(Self)
    }
}

/// Compares paths lexicographically, since that is the path ordering that the Willow spec always uses.
impl PartialOrd for Path {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Compares paths lexicographically, since that is the path ordering that the Willow spec always uses.
impl Ord for Path {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.0.cmp(other.into())
    }
}

/// The least path is the empty path.
impl LeastElement for Path {
    /// Returns the least path.
    fn least() -> Self {
        Self(wdm::Path::<MCL, MCC, MPL>::least())
    }
}

impl GreatestElement for Path {
    /// Creates the greatest possible [`Path`] (with respect to lexicographical ordering, which is also the [`Ord`] implementation of [`Path`]).
    ///
    /// It consists of a single component of `255` bytes, followed by 4095 empty components.
    ///
    /// #### Complexity
    ///
    /// Runs in `O(MCC + MPL)`. Performs a single allocation of `O(MPL)` bytes.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// use order_theory::GreatestElement;
    ///
    /// let p = Path::greatest();
    /// assert_eq!(p.component_count(), 4096);
    /// assert_eq!(p.component(0).unwrap().len(), 4096);
    /// assert!(p.component(1).unwrap().is_empty());
    /// assert!(p.component(4095).unwrap().is_empty());
    /// ```
    fn greatest() -> Self {
        Self(wdm::Path::<MCL, MCC, MPL>::greatest())
    }
}

impl LowerSemilattice for Path {
    fn greatest_lower_bound(&self, other: &Self) -> Self {
        Self(self.0.greatest_lower_bound(other.into()))
    }
}

impl UpperSemilattice for Path {
    fn least_upper_bound(&self, other: &Self) -> Self {
        Self(self.0.least_upper_bound(other.into()))
    }
}

impl TryPredecessor for Path {
    fn try_predecessor(&self) -> Option<Self> {
        self.0.try_predecessor().map(Self)
    }
}

impl PredecessorExceptForLeast for Path {}

impl TrySuccessor for Path {
    /// Returns the least path which is strictly greater than `self`, or return `None` if `self` is the greatest possible path.
    ///
    /// #### Complexity
    ///
    /// Runs in `O(n + m)`, where `n` is the total length of the [`Path`] in bytes, and `m` is the number of [`Component`]s. Performs a single allocation of `O(n + m)` bytes.
    fn try_successor(&self) -> Option<Self> {
        self.0.try_successor().map(Self)
    }
}

impl SuccessorExceptForGreatest for Path {}

impl fmt::Display for Path {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}
