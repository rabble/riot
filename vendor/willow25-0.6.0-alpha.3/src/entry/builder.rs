use bab_rs::William3Hasher;
use ufotofu::BulkProducer;
use willow_data_model::entry::entry_builder;
use willow_data_model::prelude as wdm;

use crate::prelude::*;

wrapper! {
    /// Builder for [`Entry`].
    EntryBuilder<State: entry_builder::State = entry_builder::Empty>; wdm::EntryBuilder<MCL, MCC, MPL, NamespaceId, SubspaceId, PayloadDigest, State>
}

impl<State: entry_builder::State> EntryBuilder<State> {
    /// Sets the [namespace_id](https://willowprotocol.org/specs/data-model/index.html#entry_namespace_id) of the entry being built.
    pub fn namespace_id(
        self,
        value: NamespaceId,
    ) -> EntryBuilder<entry_builder::SetNamespaceId<State>> {
        self.0.namespace_id(value).into()
    }

    /// Sets the [subspace_id](https://willowprotocol.org/specs/data-model/index.html#entry_subspace_id) of the entry being built.
    pub fn subspace_id(
        self,
        value: SubspaceId,
    ) -> EntryBuilder<entry_builder::SetSubspaceId<State>> {
        self.0.subspace_id(value).into()
    }

    /// Sets the [path](https://willowprotocol.org/specs/data-model/index.html#entry_path) of the entry being built.
    pub fn path(self, value: Path) -> EntryBuilder<entry_builder::SetPath<State>> {
        self.0.path(value.into()).into()
    }

    /// Sets the [timestamp](https://willowprotocol.org/specs/data-model/index.html#entry_timestamp) of the entry being built.
    pub fn timestamp<T: Into<Timestamp>>(
        self,
        value: T,
    ) -> EntryBuilder<entry_builder::SetTimestamp<State>> {
        self.0.timestamp(value.into()).into()
    }

    /// Sets the [payload_length](https://willowprotocol.org/specs/data-model/index.html#entry_payload_length) of the entry being built.
    pub fn payload_length(
        self,
        value: u64,
    ) -> EntryBuilder<entry_builder::SetPayloadLength<State>> {
        self.0.payload_length(value).into()
    }

    /// Sets the [payload_digest](https://willowprotocol.org/specs/data-model/index.html#entry_payload_digest) of the entry being built.
    pub fn payload_digest(
        self,
        value: PayloadDigest,
    ) -> EntryBuilder<entry_builder::SetPayloadDigest<State>> {
        self.0.payload_digest(value).into()
    }

    /// Sets the [timestamp](https://willowprotocol.org/specs/data-model/index.html#entry_timestamp) of the entry being built to the current time.
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let entry = Entry::builder()
    ///     .now().unwrap()
    ///     .namespace_id([0; 32].into())
    ///     .subspace_id([1; 32].into())
    ///     .path(path!("/vacation/plan"))
    ///     .payload_digest([77; 32].into())
    ///     .payload_length(17)
    ///     .build();
    ///
    /// assert!(entry.timestamp() > 814673376414009.into());
    /// // 814673376414009 is the timestamp for 26.10.2025 afternoon-ish in Berlin.
    /// ```
    #[cfg(feature = "std")]
    pub fn now(self) -> Result<EntryBuilder<entry_builder::SetTimestamp<State>>, HifitimeError> {
        self.0.now().map(|b| b.into())
    }

    /// Sets the [payload_length](https://willowprotocol.org/specs/data-model/index.html#entry_payload_length) and [payload_digest](https://willowprotocol.org/specs/data-model/index.html#entry_payload_digest) of the entry being built to those of the given [Payload](https://willowprotocol.org/specs/data-model/index.html#Payload).
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let entry = Entry::builder()
    ///     .namespace_id([0; 32].into())
    ///     .subspace_id([1; 32].into())
    ///     .path(path!("/vacation/plan"))
    ///     .timestamp(12345)
    ///     .payload(b"See all the sights!")
    ///     .build();
    ///
    /// assert_eq!(*entry.payload_digest(), [
    ///     91, 29, 243, 211, 140, 181, 127, 138,
    ///     116, 79, 56, 47, 85, 50, 157, 82,
    ///     55, 192, 253, 122, 72, 250, 70, 43,
    ///     56, 116, 99, 53, 188, 85, 83, 234,
    /// ].into());
    /// assert_eq!(entry.payload_length(), 19);
    /// ```
    pub fn payload<Payload: AsRef<[u8]>>(
        self,
        payload: Payload,
    ) -> EntryBuilder<entry_builder::SetPayloadLength<entry_builder::SetPayloadDigest<State>>> {
        self.0
            .payload::<Payload, William3Hasher, bab_rs::William3Digest>(payload)
            .into()
    }

    /// Sets the [namespace_id](https://willowprotocol.org/specs/data-model/index.html#entry_namespace_id) of the entry being built to the [default_namespace_id](https://willowprotocol.org/specs/willow25/index.html#willow25_default_namespace_id).
    pub fn default_namespace_id(self) -> EntryBuilder<entry_builder::SetNamespaceId<State>> {
        self.namespace_id(crate::defaults::default_namespace_id())
    }

    /// Sets the [subspace_id](https://willowprotocol.org/specs/data-model/index.html#entry_subspace_id) of the entry being built to the [default_subspace_id](https://willowprotocol.org/specs/willow25/index.html#willow25_default_subspace_id).
    pub fn default_subspace_id(self) -> EntryBuilder<entry_builder::SetSubspaceId<State>> {
        self.subspace_id(crate::defaults::default_subspace_id())
    }

    /// Sets the [payload_digest](https://willowprotocol.org/specs/data-model/index.html#entry_payload_digest) and [payload_length](https://willowprotocol.org/specs/data-model/index.html#entry_payload_length) of the entry being built to the [default_payload_digest](https://willowprotocol.org/specs/confidential-sync/index.html#sync_default_payload_digest).
    pub fn default_payload(
        self,
    ) -> EntryBuilder<entry_builder::SetPayloadLength<entry_builder::SetPayloadDigest<State>>> {
        self.payload_digest(crate::defaults::default_payload_digest())
            .payload_length(crate::defaults::DEFAULT_PAYLOAD_LENGTH)
    }

    /// Sets the [payload_length](https://willowprotocol.org/specs/data-model/index.html#entry_payload_length) and [payload_digest](https://willowprotocol.org/specs/data-model/index.html#entry_payload_digest) of the entry being built to those of the payload given by the producer.
    ///
    /// ```
    /// use willow25::prelude::*;
    /// use ufotofu::producer::clone_from_slice;
    ///
    /// # pollster::block_on(async {
    /// let mut producer = clone_from_slice(b"See all the sights!");
    ///
    /// let entry = Entry::builder()
    ///     .namespace_id([0; 32].into())
    ///     .subspace_id([1; 32].into())
    ///     .path(path!("/vacation/plan"))
    ///     .timestamp(12345)
    ///     .payload_async(&mut producer).await.unwrap()
    ///     .build();
    ///
    /// assert_eq!(*entry.payload_digest(), [
    ///     91, 29, 243, 211, 140, 181, 127, 138,
    ///     116, 79, 56, 47, 85, 50, 157, 82,
    ///     55, 192, 253, 122, 72, 250, 70, 43,
    ///     56, 116, 99, 53, 188, 85, 83, 234,
    /// ].into());
    /// assert_eq!(entry.payload_length(), 19);
    /// # })
    /// ```
    pub async fn payload_async<P>(
        self,
        payload_producer: &mut P,
    ) -> Result<
        EntryBuilder<entry_builder::SetPayloadLength<entry_builder::SetPayloadDigest<State>>>,
        P::Error,
    >
    where
        P: BulkProducer<Item = u8, Final = ()>,
    {
        self.0
            .payload_async::<P, William3Hasher, bab_rs::William3Digest>(payload_producer)
            .await
            .map(|b| b.into())
    }

    /// Builds the [`Entry`].
    pub fn build(self) -> Entry
    where
        State: entry_builder::IsComplete,
    {
        self.0.build().into()
    }
}
