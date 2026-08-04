use willow_data_model::prelude as wdm;

use crate::prelude::{MCC, MCL, MPL, Path, SubspaceId};

/// A keylike value is one that can be used to uniquely identify an [Entry](https://willowprotocol.org/specs/data-model/index.html#Entry) within a [namespace](https://willowprotocol.org/specs/data-model/index.html#namespace)-specific [store](https://willowprotocol.org/specs/data-model/index.html#store).
///
/// A [subspace_id](https://willowprotocol.org/specs/data-model/index.html#entry_subspace_id) and a [path](https://willowprotocol.org/specs/data-model/index.html#entry_path) together uniquely identify an entry in a namespace, because between any two non-equal entries with of equal [subspace_id](https://willowprotocol.org/specs/data-model/index.html#entry_subspace_id), [path](https://willowprotocol.org/specs/data-model/index.html#entry_path), and [namespace_id](https://willowprotocol.org/specs/data-model/index.html#entry_namespace_id), one would overwrite the other.
pub trait Keylike: wdm::Keylike<MCL, MCC, MPL, SubspaceId> {
    /// Returns the [subspace_id](https://willowprotocol.org/specs/data-model/index.html#entry_subspace_id) of `self`.
    fn subspace_id(&self) -> &SubspaceId;

    /// Returns the [path](https://willowprotocol.org/specs/data-model/index.html#entry_path) of `self`.
    fn path(&self) -> &Path;
}

impl<T> Keylike for T
where
    T: wdm::Keylike<MCL, MCC, MPL, SubspaceId> + ?Sized,
{
    fn subspace_id(&self) -> &SubspaceId {
        self.wdm_subspace_id()
    }

    fn path(&self) -> &Path {
        self.wdm_path().into()
    }
}

impl wdm::Keylike<MCL, MCC, MPL, SubspaceId> for (SubspaceId, Path) {
    fn wdm_subspace_id(&self) -> &SubspaceId {
        &self.0
    }

    fn wdm_path(&self) -> &wdm::Path<MCL, MCC, MPL> {
        (&self.1).into()
    }
}

impl wdm::Keylike<MCL, MCC, MPL, SubspaceId> for (Path, SubspaceId) {
    fn wdm_subspace_id(&self) -> &SubspaceId {
        &self.1
    }

    fn wdm_path(&self) -> &wdm::Path<MCL, MCC, MPL> {
        (&self.0).into()
    }
}

/// Methods for working with [`Keylikes`](Keylike).
///
/// This trait is automatically implemented by all types implementing [`Keylike`].
pub trait KeylikeExt: wdm::KeylikeExt<MCL, MCC, MPL, SubspaceId> {
    /// Returns whether `self` and `other` describe equal keys, i.e., whether their [subspace ids](Keylike::subspace_id) and [paths](Keylike::path) are both equal.
    ///
    /// # Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// assert!(
    ///     (SubspaceId::from([0; SUBSPACE_ID_WIDTH]), path!(""))
    ///     .key_eq(&(SubspaceId::from([0; SUBSPACE_ID_WIDTH]), path!("")))
    /// );
    /// assert!(
    ///     !(SubspaceId::from([0; SUBSPACE_ID_WIDTH]), path!(""))
    ///     .key_eq(&(SubspaceId::from([1; SUBSPACE_ID_WIDTH]), path!("")))
    /// );
    /// assert!(
    ///     !(SubspaceId::from([0; SUBSPACE_ID_WIDTH]), path!(""))
    ///     .key_eq(&(SubspaceId::from([0; SUBSPACE_ID_WIDTH]), path!("/nope")))
    /// );
    /// ```
    fn key_eq<OtherKey>(&self, other: &OtherKey) -> bool
    where
        OtherKey: Keylike,
    {
        self.wdm_key_eq(other)
    }

    /// Returns whether `self` and `other` describe non-equal keys, i.e., whether their [subspace ids](Keylike::subspace_id) and [paths](Keylike::path) are not both equal.
    ///
    /// # Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// assert!(
    ///     !(SubspaceId::from([0; SUBSPACE_ID_WIDTH]), path!(""))
    ///     .key_ne(&(SubspaceId::from([0; SUBSPACE_ID_WIDTH]), path!("")))
    /// );
    /// assert!(
    ///     (SubspaceId::from([0; SUBSPACE_ID_WIDTH]), path!(""))
    ///     .key_ne(&(SubspaceId::from([1; SUBSPACE_ID_WIDTH]), path!("")))
    /// );
    /// assert!(
    ///     (SubspaceId::from([0; SUBSPACE_ID_WIDTH]), path!(""))
    ///     .key_ne(&(SubspaceId::from([0; SUBSPACE_ID_WIDTH]), path!("/nope")))
    /// );
    /// ```
    fn key_ne<OtherKey>(&self, other: &OtherKey) -> bool
    where
        OtherKey: Keylike,
    {
        self.wdm_key_ne(other)
    }
}

impl<T> KeylikeExt for T where T: Keylike + ?Sized {}
