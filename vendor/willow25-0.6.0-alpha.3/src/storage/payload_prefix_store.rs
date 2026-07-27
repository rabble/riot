#[cfg(feature = "dev")]
use arbitrary::Arbitrary;

use bab_rs::generic::storage::single_slice_store::SliceStreamResumptionInfo;
use bab_rs::generic::storage::units::{ByteCount, ChunkCount, ChunkIndex};
use ufotofu::prelude::*;

use bab_rs::generic::storage::verifiable_streaming::SliceStreamingOptions;

use crate::prelude::*;
use crate::storage::Store;

/// An extension of the basic [`Store`] trait, adding the ability to store partial payloads.
///
/// More specifically, a [`PayloadPrefixStore`] stores one *prefix* of the payload of each entry. It cannot store multiple disjoint subslices of the payload, nor single subslices that aren't prefixes (i.e., sublices that don't start from the beginning of the payload). On the upside, these constraints make for a more simple API than if things were more flexible.
///
/// A [`PayloadPrefixStore`] ensures that the prefixes it stores actually belong to the claimed payload, via the [WILLIAM3](https://worm-blossom.github.io/bab/#instantiations_william). Hence, to append data to a payload prefix, it does not suffice to supply merely the raw payload bytes to append: you have to supply a [WILLIAM3 verifiable slice stream](https://worm-blossom.github.io/bab/#slice_verification). See [`PayloadPrefixStore::append_to_payload_prefix`] for more information.
pub trait PayloadPrefixStore: Store {
    /// Appends to the payload prefix stored for some entry, and verifies the integrity of the appended data.
    ///
    /// The producer `p` must emit a [verifiable slice encoding](https://worm-blossom.github.io/bab/#baseline_slice) that starts at the first chunk not currently available in the store, and whose optimisations match those in the given `stream_options`.
    ///
    /// Use the [`PayloadPrefixStore::prefix_stream_resumption_info`] method to obtain information about how to request an appropriate verifiable slice encoding.
    async fn append_to_payload_prefix<K, P>(
        &mut self,
        namespace_id: &NamespaceId,
        key: &K,
        p: &mut P,
        stream_options: SliceStreamingOptions,
    ) -> Result<(), AppendToPayloadPrefixError<P::Error, Self::InternalError>>
    where
        K: Keylike,
        P: BulkProducer<Item = u8>;

    /// Returns how many contiguous payload bytes from the beginning of the payload of some entry are available. Addressing an entry that doesn't exist is signalled via an error variant.
    ///
    /// Note that by the time you call a different method on this store, the number of available bytes may have changed again.
    ///
    /// If the `expected_digest` is `Some(pd)` but the entry for the given namespace_id and key does not have `pd` as its payload digest, the method acts as if no matching entry was found (i.e., it returns an error).
    async fn length_of_payload_prefix<K>(
        &mut self,
        namespace_id: &NamespaceId,
        key: &K,
        expected_digest: Option<PayloadDigest>,
    ) -> Result<ByteCount, InternalOrNoSuchEntryError<Self::InternalError>>
    where
        K: Keylike;

    /// Returns the [`SliceStreamResumptionInfo`] describing what data to request from a peer to continue appending to the payload of an entry, or `Ok(None)` if the full payload of the entry is available. Addressing an entry that doesn't exist is signalled via an error variant.
    ///
    /// If the `expected_digest` is `Some(pd)` but the entry for the given namespace_id and key does not have `pd` as its payload digest, the method acts as if no matching entry was found (i.e., it returns an error).
    async fn prefix_stream_resumption_info<K>(
        &mut self,
        namespace_id: &NamespaceId,
        key: &K,
        expected_digest: Option<PayloadDigest>,
    ) -> Result<Option<SliceStreamResumptionInfo>, InternalOrNoSuchEntryError<Self::InternalError>>
    where
        K: Keylike;

    /// Writes a verifiable WILLIAM3 slice stream of (part of) the available payload prefix into `c`, and returns how many bytes were written into `c`.
    ///
    /// The `start` index is given in chunks.
    ///
    /// The `stream_options` determine which optimisations are performed to obtain the stream. The `length` is given in chunks, not bytes.
    ///
    /// If the `expected_digest` is `Some(pd)` but the entry for the given namespace_id and key does not have `pd` as its payload digest, the method acts as if no matching entry was found (i.e., it returns an error).
    async fn get_verifiable_stream<K, C>(
        &mut self,
        namespace_id: &NamespaceId,
        key: &K,
        expected_digest: Option<PayloadDigest>,
        start: ChunkIndex,
        length: ChunkCount,
        stream_options: SliceStreamingOptions,
        c: &mut C,
    ) -> Result<ByteCount, GetVerifiableStreamError<C::Error, Self::InternalError>>
    where
        K: Keylike,
        C: BulkConsumer<Item = u8>;
}

/// Everything that can go wrong when calling methods where nothing can go wrong except for internal errors or an entry being unavailable.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Hash, Debug)]
#[cfg_attr(feature = "dev", derive(Arbitrary))]
pub enum InternalOrNoSuchEntryError<StoreError> {
    /// There was no entry in the store for the given namespace and key.
    NoSuchEntry,
    /// The store encountered some other error.
    StoreError(StoreError),
}

impl<StoreError> From<StoreError> for InternalOrNoSuchEntryError<StoreError> {
    fn from(value: StoreError) -> Self {
        Self::StoreError(value)
    }
}

/// Everything that can go wrong when calling [`PayloadPrefixStore::append_to_payload_prefix`].
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Hash, Debug)]
#[cfg_attr(feature = "dev", derive(Arbitrary))]
pub enum AppendToPayloadPrefixError<ProducerError, StoreError> {
    /// The [`BulkProducer`] supplying the bytes of the verifiable slice stream emitted an error.
    ProducerError(ProducerError),
    /// The [`BulkProducer`] supplying the bytes of the verifiable slice stream ended unexpectedly (for example by ending in the middle of a transmitting chunk).
    UnexpectedEndOfStream,
    /// The bytes we received failed verification, they did not constitute a valid slice stream. Shame on the entity that supplied these bytes!
    VerificationError,
    /// The store does not store a matching entry to whose payload to append in the first place.
    NoSuchEntry,
    /// The store encountered some other error.
    StoreError(StoreError),
}

/// Everything that can go wrong when calling [`PayloadPrefixStore::get_verifiable_stream`].
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Hash, Debug)]
#[cfg_attr(feature = "dev", derive(Arbitrary))]
pub enum GetVerifiableStreamError<ConsumerError, StoreError> {
    /// The [`BulkConsumer`] receiving the bytes of the verifiable slice stream emitted an error.
    ConsumerError(ConsumerError),
    /// The store encountered some other error.
    StoreError(StoreError),
    /// The store does not store a matching entry for whose payload to emit a stream in the first place..
    NoSuchEntry,
}
