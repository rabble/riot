use crate::{
    authorisation::PriorAuthedEntryEntryPair, defaults::default_authorised_entry, prelude::*,
};

use super::DropSliceMetadata;
use compact_u64::{cu64_encode, cu64_encode_standalone, write_tag};
use core::marker::PhantomData;
use ufotofu::codec_prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
/// The errors which can be encountered when exporting a drop.
pub enum EncodeDropError<ProducerError, ConsumerError> {
    /// A producer of payload bytes returned an error.
    ProducerError(ProducerError),
    /// The consumer consuming the encoded bytes of the drop returned an error.
    ConsumerError(ConsumerError),
    /// The given consumer consumed a different number of bytes than was expected.
    ConsumedBytesMismatch,
    /// A payload of more than [`usize::MAX`] bytes was encountered. This can occur on 32-byte architectures. Our implementation cannot handle this, so it emits an error in this case.
    ArchitectureTooSmall,
}

/// A wrapper for a [`BulkConsumer<Item=u8>`]. The wrapper consumes the pairs of ([`DropSliceMetadata`], [`BulkProducer<Item=u8>`]) that describe the entries and payload slices of a drop. It writes the associated drop encoding into the wrapped consumer.
///
/// The bulk producers that are part of the consumed pairs must be producers that emit the payload slices. The associated [`DropSliceMetadata`] provides not only metadata about those slices, but also describes the entries that make up the drop in the first place.
pub struct DropEncoder<C, P>
where
    C: BulkConsumer<Item = u8>,
    P: BulkProducer<Item = u8>,
{
    drop_consumer: C,
    entry_state: AuthorisedEntry,
    chunk_offset: u64,
    phantom: PhantomData<P>,
}

impl<C, P> DropEncoder<C, P>
where
    C: BulkConsumer<Item = u8>,
    P: BulkProducer<Item = u8>,
{
    /// Creates a new encoder from a [`BulkConsumer<Item=u8>`].
    pub fn new(drop_consumer: C) -> Self {
        let entry_state = default_authorised_entry();

        DropEncoder {
            drop_consumer,
            entry_state,
            chunk_offset: 0,
            phantom: PhantomData,
        }
    }
}

impl<C, P> From<C> for DropEncoder<C, P>
where
    C: BulkConsumer<Item = u8>,
    P: BulkProducer<Item = u8>,
{
    fn from(value: C) -> Self {
        DropEncoder::new(value)
    }
}

impl<C, P> Consumer for DropEncoder<C, P>
where
    C: BulkConsumer<Item = u8>,
    P: BulkProducer<Item = u8>,
{
    type Item = (DropSliceMetadata, P);

    type Final = ();

    type Error = EncodeDropError<P::Error, C::Error>;

    async fn consume(&mut self, val: Either<Self::Item, Self::Final>) -> Result<(), Self::Error> {
        let (metadata, mut payload) = match val {
            Right(()) => {
                return self
                    .drop_consumer
                    .consume_item(0u8)
                    .await
                    .map_err(EncodeDropError::ConsumerError);
            }
            Left((metadata, payload)) => (metadata, payload),
        };

        let expected_bytes = metadata.expected_bytes();

        if metadata.entry() != &self.entry_state {
            // New entry!

            let entry = metadata.entry();

            // Reset the tracked chunk offset
            self.chunk_offset = 0;

            let mut needs_namespace = false;
            let mut needs_subspace = false;

            let mut header = 0b0100_0000;
            if entry.namespace_id() != self.entry_state.namespace_id() {
                needs_namespace = true;
                header |= 0b0010_0000;
            }
            if entry.subspace_id() != self.entry_state.subspace_id() {
                needs_subspace = true;
                header |= 0b0001_0000;
            }

            write_tag(&mut header, 2, 4, entry.timestamp().into());

            // Ambiguous between complete and multislice cases...
            if metadata.is_complete() {
                header |= 0b0000_0001;
            }

            // Ambiguous between prefix and multislice cases...
            if !metadata.is_complete() && metadata.chunk_count() > 0 && metadata.first_chunk() == 0
            {
                header |= 0b0000_0010;
            }

            // ...so disambiguate here.
            if metadata.is_multislice() {
                header |= 0b0000_0011;
            }

            self.drop_consumer
                .consume_item(header)
                .await
                .map_err(EncodeDropError::ConsumerError)?;

            if needs_namespace {
                entry
                    .namespace_id()
                    .encode(&mut self.drop_consumer)
                    .await
                    .map_err(EncodeDropError::ConsumerError)?;
            }

            if needs_subspace {
                entry
                    .subspace_id()
                    .encode(&mut self.drop_consumer)
                    .await
                    .map_err(EncodeDropError::ConsumerError)?;
            }

            entry
                .path()
                .relative_encode(self.entry_state.path(), &mut self.drop_consumer)
                .await
                .map_err(EncodeDropError::ConsumerError)?;

            cu64_encode(entry.timestamp().into(), 2, &mut self.drop_consumer)
                .await
                .map_err(EncodeDropError::ConsumerError)?;

            cu64_encode_standalone(entry.payload_length(), &mut self.drop_consumer)
                .await
                .map_err(EncodeDropError::ConsumerError)?;

            entry
                .payload_digest()
                .encode(&mut self.drop_consumer)
                .await
                .map_err(EncodeDropError::ConsumerError)?;

            let pair =
                PriorAuthedEntryEntryPair::new(self.entry_state.clone(), entry.entry().clone());

            entry
                .authorisation_token()
                .relative_encode(&pair, &mut self.drop_consumer)
                .await
                .map_err(EncodeDropError::ConsumerError)?;

            self.entry_state = entry.clone();

            match header & 0b0000_0011 {
                0b0000_0000 => {
                    // No payload
                    return Ok(());
                }
                0b0000_0011 => (), // Payload slices are handled after this conditional block
                _ => {
                    // Complete payload OR payload prefix:
                    // Consume the whole payload, and check that it contained the right number of bytes

                    let mut consumed_bytes: usize = 0;

                    loop {
                        match payload
                            .expose_items(async |items| {
                                match self.drop_consumer.bulk_consume(items).await {
                                    Ok(consumed) => (consumed, Ok(consumed)),
                                    Err(err) => (0, Err(EncodeDropError::ConsumerError(err))),
                                }
                            })
                            .await
                            .map_err(EncodeDropError::ProducerError)?
                        {
                            Left(progress) => {
                                consumed_bytes = consumed_bytes
                                    .checked_add(progress?)
                                    .ok_or(EncodeDropError::ArchitectureTooSmall)?;
                                if consumed_bytes as u64 > expected_bytes {
                                    return Err(EncodeDropError::ConsumedBytesMismatch);
                                }
                            }
                            Right(_) => {
                                if consumed_bytes as u64 != expected_bytes {
                                    return Err(EncodeDropError::ConsumedBytesMismatch);
                                }
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }

        // Now we start handling the (potentially) multi-slice case

        let mut slice_header = 0b1000_0000;
        let chunk_interval = metadata.first_chunk() - self.chunk_offset.saturating_sub(1);

        write_tag(&mut slice_header, 3, 1, chunk_interval);
        write_tag(&mut slice_header, 4, 3, metadata.chunk_count());

        cu64_encode(chunk_interval, 3, &mut self.drop_consumer)
            .await
            .map_err(EncodeDropError::ConsumerError)?;
        cu64_encode(metadata.chunk_count(), 4, &mut self.drop_consumer)
            .await
            .map_err(EncodeDropError::ConsumerError)?;

        let mut consumed_bytes: usize = 0;
        loop {
            match payload
                .expose_items(
                    async |items| match self.drop_consumer.bulk_consume(items).await {
                        Ok(consumed) => (consumed, Ok(consumed)),
                        Err(err) => (0, Err(EncodeDropError::ConsumerError(err))),
                    },
                )
                .await
                .map_err(EncodeDropError::ProducerError)?
            {
                Left(progress) => {
                    consumed_bytes = consumed_bytes
                        .checked_add(progress?)
                        .ok_or(EncodeDropError::ArchitectureTooSmall)?;
                    if (consumed_bytes as u64) > expected_bytes {
                        return Err(EncodeDropError::ConsumedBytesMismatch);
                    }
                }
                Right(_) => {
                    if consumed_bytes as u64 != expected_bytes {
                        return Err(EncodeDropError::ConsumedBytesMismatch);
                    }
                    self.chunk_offset += metadata.chunk_count();
                    return Ok(());
                }
            }
        }
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        self.drop_consumer
            .flush()
            .await
            .map_err(EncodeDropError::ConsumerError)
    }
}
