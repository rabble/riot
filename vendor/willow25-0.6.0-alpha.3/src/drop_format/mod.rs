//! A [drop format](https://willowprotocol.org/specs/drop-format/index.html#willow_drop_format) for Willow stores.

// POSSIBLE IMPROVEMENT: Re-order the areas in a way which would give the highest probability of an efficiently relatively encoded entries.
// POSSIBLE IMPROVEMENT: De-duplicate entries from overlapping areas.

use std::fmt::Debug;

use crate::{
    entry::Entrylike,
    prelude::*,
    storage::{
        AppendToPayloadPrefixError, GetVerifiableStreamError, InternalOrNoSuchEntryError,
        PayloadPrefixStore,
    },
};
use bab_rs::CHUNK_SIZE;
use futures_lite::future::zip;
use ufotofu::{
    ExpectedFinalError, PipeError, ProduceAtLeastError,
    channels::{new_sssr, sssr::Receiver},
    consumer::compat::fn_mut::*,
    pipe,
    prelude::*,
    queues::{UnboundedElastic, new_unbounded_elastic},
};

mod decode;
pub use decode::*;

mod encode;
pub use encode::*;

mod slice_metadata;
pub use slice_metadata::{DropSliceMetadata, SliceMetadataError};

/// Reads entries from the given `entry_producer`, retrieves their payloads from the `store`, computes corresponding [`DropSliceMetadata`], and writes that data into the given `consumer`. Using a [`DropEncoder`] as the `consumer` will result in an encoding of the produced entries in the [Willow drop format](https://willowprotocol.org/specs/drop-format/index.html#drop).
pub async fn export_drop<S, C, P>(
    store: &mut S,
    entry_producer: &mut P,
    consumer: &mut C,
) -> Result<(), ExportDropError<S::InternalError, C::Error, P::Error>>
where
    C: Consumer<Item = (DropSliceMetadata, Receiver<UnboundedElastic<u8>, ()>), Final = ()>,
    S: PayloadPrefixStore + Clone,
    P: Producer<Item = AuthorisedEntry>,
{
    let mut s = store.clone();

    let f =
        async |value: Either<AuthorisedEntry, _>| -> Result<(), ExportDropError<S::InternalError, C::Error, P::Error>> {
            match value {
                Left(authorised_entry) => {
                    let byte_count = match s
                        .length_of_payload_prefix(
                            &authorised_entry.namespace_id(),
                            &authorised_entry,
                            Some(authorised_entry.payload_digest().clone()),
                        )
                        .await
                    {
                        Ok(byte_count) => byte_count,
                        Err(err) => match err {
                            InternalOrNoSuchEntryError::NoSuchEntry => {
                                // The requested entry is not part of the store, so there is no work to do here
                                return Ok(());
                            }
                            InternalOrNoSuchEntryError::StoreError(err) => {
                                return Err(ExportDropError::StoreError(err));
                            }
                        },
                    };

                    // TODO: Remove cast?
                    let chunk_count = byte_count.div_ceil(CHUNK_SIZE as u64);
                    let digest = authorised_entry.payload_digest().clone();

                    let metadata =
                        DropSliceMetadata::new(
                            authorised_entry.clone(),
                            0, // Always zero for stores which only store prefixes of payloads
                            chunk_count,
                            0, // Always zero for stores which only store prefixes of payloads
                            false // Again, always false for stores which only store prefixes of payloads
                        ).expect("metadata generated from a known entry in the store should be valid");

                    // Hmm. Should this function take a configurable queue type via a Fn() -> Q: Queue parameter?
                    let (mut sender, receiver) = new_sssr(new_unbounded_elastic());

                    let stream_options = metadata.streaming_options();

                    let slice_stream_generator = async {
                        match s
                            .get_verifiable_stream(
                                &authorised_entry.namespace_id(),
                                &authorised_entry,
                                Some(digest),
                                0,
                                byte_count,
                                stream_options,
                                &mut sender,
                            )
                            .await
                        {
                            Ok(_) => {
                                sender.consume_final(()).await.expect("sender is infallible"); 
                                Ok(())
                            },
                            Err(err) => match err {
                                GetVerifiableStreamError::ConsumerError(_) => unreachable!("sender is infallible"),
                                GetVerifiableStreamError::StoreError(err) => {
                                    Err(ExportDropError::StoreError(err))
                                }
                                GetVerifiableStreamError::NoSuchEntry => {
                                    // The entry was removed since we started exporting it.
                                    // We have already committed a number of bytes in the metadata.
                                    // We have to error out.
                                    Err(ExportDropError::EntryDeleted)
                                }
                            },
                        }
                    };

                    // Zshwoop!
                    let (generator_result, consumer_result) = zip(
                        slice_stream_generator,
                        consumer.consume_item((metadata, receiver)),
                    )
                    .await;

                    generator_result?;
                    consumer_result.map_err(ExportDropError::ConsumerError)
                }
                Right(_) => consumer.consume_final(()).await.map_err(ExportDropError::ConsumerError)
            }
        };

    pipe(entry_producer, &mut ClosureConsumer::new(f))
        .await
        .map_err(|err| match err {
            PipeError::Producer(err) => ExportDropError::ProducerError(err),
            PipeError::Consumer(err) => err,
        })
}

/// For every pair of [`DropSliceMetadata`] and [`BulkProducer<Item=u8>`] emitted by the given `producer`,
/// appends data to the given `store` from the 64-grouped verifiable slice stream produced by the bulk producer
/// when it is compatible with the current state of the store.
///
/// Using a [`DropDecoder`] as the `producer` will result in importing all compatible entries from a corresponding
/// encoding in the [Willow drop format](https://willowprotocol.org/specs/drop-format/index.html#drop).
///
/// If `skip_incompatible_slices` is `true`, partial payload slices which cannot be appended to an existing
/// prefix are ignored, otherwise an error will be reported and the import stopped when such slices are encountered.
///
/// In practice, if the store already has a non-empty prefix of a payload, then it will discard any payload data
/// for that entry (or error if `skip_incompatible_slices` is `false`), unless the payload data in the drop
/// happens to start exactly where the stored non-empty prefix ends. This is not great, and we will update this
/// method to successfully import payload data under more conditions in the future.
pub async fn import_drop<S, P, PP>(
    store: &mut S,
    producer: &mut P,
    skip_incompatible_slices: bool,
) -> Result<(), ImportDropError<P::Error, PP::Error, S::InternalError>>
where
    S: PayloadPrefixStore + Clone,
    P: Producer<Item = (DropSliceMetadata, PP)>,
    PP: BulkProducer<Item = u8>,
{
    while let Left((metadata, mut slice)) = producer
        .produce()
        .await
        .map_err(ImportDropError::ImportProducerError)?
    {
        let entry = metadata.entry();
        let expected_digest = Some(entry.payload_digest().clone());

        if !store
            .insert_entry(entry.clone())
            .await
            .map_err(ImportDropError::StoreError)?
        {
            // This entry is outdated, we can skip it and move on to the next.
            skip_slice(&metadata, slice).await?;
            continue;
        }

        let Some(resumption_info) = store
            .prefix_stream_resumption_info(entry.namespace_id(), entry, expected_digest)
            .await
            .map_err(|err| match err {
                InternalOrNoSuchEntryError::NoSuchEntry => ImportDropError::EntryDeleted,
                InternalOrNoSuchEntryError::StoreError(err) => ImportDropError::StoreError(err),
            })?
        else {
            // Our copy of this entry is already complete, we can skip it.
            skip_slice(&metadata, slice).await?;
            continue;
        };

        if metadata.resumption_info() == resumption_info && metadata.chunk_count() > 0 {
            // We can add the data from this slice stream to this entry.
            store
                .append_to_payload_prefix(
                    entry.namespace_id(),
                    entry,
                    &mut slice,
                    metadata.streaming_options(),
                )
                .await?;

            slice.produce_final().await?;
            continue;
        }

        // What if we can't append data from the new slice stream to what we already have stored, but
        // the data in the new slice stream is more complete than our current record?

        // There are options here, each with problems. If we outright reject the slice which doesn't match
        // the resumption info, then we can miss out on complete payloads and delay completion of the entry (or
        // a malicious peer could circulate prefixes in a community where payload subslice streams are rare,
        // thereby blocking those entries in that community). If we optimistically forget a prefix in order to
        // replace it with one which claims to be more complete, a malicious peer can circulate bogus metadata
        // to force stores to forget payloads. If we want to verify the payload, we must read it into some
        // other store first to do so, where we have otherwise been able to avoid buffering. This last option
        // feels most correct, but is sadly the most expensive.

        // We couldn't find a use for this slice stream, so we skip it.
        skip_slice(&metadata, slice).await?;

        // We encountered a slice which we couldn't append to a prefix, and were asked to report an error
        // in that case.
        if metadata.first_chunk() > 0
            && resumption_info.start_chunk == 0
            && skip_incompatible_slices
        {
            return Err(ImportDropError::PartialPayloadSliceEncountered);
        }
    }

    Ok(())
}

/// The ways in which exporting a [drop](https://willowprotocol.org/specs/drop-format/index.html) from a [store](PayloadPrefixStore) can fail.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ExportDropError<StoreError, ConsumerError, ProducerError> {
    /// The store encountered an error while a drop was being exported.
    StoreError(StoreError),
    /// The consumer processing the export reported an error.
    ConsumerError(ConsumerError),
    /// The producer providing entries to include in the drop reported an error.
    ProducerError(ProducerError),
    /// An entry was deleted concurrently to it being exported, invalidating the encoding we have emitted already.
    EntryDeleted,
}

/// The ways in which importing a [drop](link_the_spec) into a [store](PayloadPrefixStore) can fail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportDropError<ImportProducerError, SliceProducerError, StoreError> {
    /// The number of bytes produced by a payload slice stream was different from the number expected from the corresponding metadata.
    SliceStreamBytesMismatch,
    /// The producer of ([`DropSliceMetadata`], `SliceStreamProducer`) pairs encountered an error.
    ImportProducerError(ImportProducerError),
    /// A producer of a verifiable slice stream for an entry in the drop encountered an error.
    SliceProducerError(SliceProducerError),
    /// The store into which data was being imported encountered an error.
    StoreError(StoreError),
    /// The verifiable slice stream for an entry in the drop failed verification.
    VerificationError,
    /// An entry was removed from the store before the corresponding verifiable slice stream from the drop could be appended.
    EntryDeleted,
    /// A payload of more than [`usize::MAX`] bytes was encountered. This can occur on 32-byte architectures. Our implementation cannot handle this, so it emits an error in this case.
    ArchitectureTooSmall,
    /// A partial payload slice appeared in the drop and was not compatible with the existing prefix, and the caller of [`import_drop`] indicated that this should be treated as a fatal error.
    PartialPayloadSliceEncountered,
}

async fn skip_slice<P: BulkProducer<Item = u8>, DropProducerError, StoreError>(
    metadata: &DropSliceMetadata,
    mut slice: P,
) -> Result<(), ImportDropError<DropProducerError, P::Error, StoreError>> {
    slice
        .skip(
            metadata
                .expected_bytes()
                .try_into()
                .map_err(|_| ImportDropError::ArchitectureTooSmall)?,
        )
        .await?;

    slice.produce_final().await?;

    Ok(())
}

impl<Fin, ImportProducerError, SliceProducerError, StoreError>
    From<ProduceAtLeastError<Fin, SliceProducerError>>
    for ImportDropError<ImportProducerError, SliceProducerError, StoreError>
{
    fn from(
        ProduceAtLeastError { count: _, reason }: ProduceAtLeastError<Fin, SliceProducerError>,
    ) -> Self {
        match reason {
            Ok(_) => ImportDropError::SliceStreamBytesMismatch,
            Err(err) => ImportDropError::SliceProducerError(err),
        }
    }
}

impl<ImportProducerError, SliceProducerError, StoreError>
    From<AppendToPayloadPrefixError<SliceProducerError, StoreError>>
    for ImportDropError<ImportProducerError, SliceProducerError, StoreError>
{
    fn from(value: AppendToPayloadPrefixError<SliceProducerError, StoreError>) -> Self {
        match value {
            AppendToPayloadPrefixError::ProducerError(err) => Self::SliceProducerError(err),
            AppendToPayloadPrefixError::UnexpectedEndOfStream => Self::SliceStreamBytesMismatch,
            AppendToPayloadPrefixError::VerificationError => Self::VerificationError,
            AppendToPayloadPrefixError::NoSuchEntry => Self::EntryDeleted,
            AppendToPayloadPrefixError::StoreError(err) => Self::StoreError(err),
        }
    }
}

impl<ImportProducerError, SliceProducerError, StoreError>
    From<ExpectedFinalError<u8, SliceProducerError>>
    for ImportDropError<ImportProducerError, SliceProducerError, StoreError>
{
    fn from(value: ExpectedFinalError<u8, SliceProducerError>) -> Self {
        match value {
            ExpectedFinalError::Item(_) => Self::SliceStreamBytesMismatch,
            ExpectedFinalError::Error(err) => Self::SliceProducerError(err),
        }
    }
}
