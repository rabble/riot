use willow_data_model::prelude as wdm;

use crate::prelude::NamespaceId;

/// A namespaced value is one with an associated [namespace id](https://willowprotocol.org/specs/data-model/index.html#NamespaceId) (of type `N`).
pub trait Namespaced: wdm::Namespaced<NamespaceId> {
    /// Returns the [namespace id](https://willowprotocol.org/specs/data-model/index.html#NamespaceId) of `self`.
    fn namespace_id(&self) -> &NamespaceId;
}

impl<T> Namespaced for T
where
    T: wdm::Namespaced<NamespaceId> + ?Sized,
{
    fn namespace_id(&self) -> &NamespaceId {
        self.wdm_namespace_id()
    }
}
