use super::DropSliceMetadata;
use crate::authorisation::{PossiblyAuthorisedEntry, PriorAuthedEntryEntryPair};
use crate::defaults::default_authorised_entry;
use crate::is_bitflagged;
use crate::prelude::*;
use alloc::rc::Rc;
use bab_rs::{CHUNK_SIZE, generic::storage::units::optimal_left_skip};
use compact_u64::{cu64_decode, cu64_decode_standalone};
use frugal_async::TakeCell;
use ufotofu::{ProduceLimitError, codec_prelude::*, producer::Limit};

/// A wrapper for a [`BulkProducer<Item=u8>`] which decodes the wrapped producer to produce pairs of ([`DropSliceMetadata`], [`PayloadProducer`]).
///
/// # Warning
///
/// Any [`SliceStreamProducer`] produced by a `DropDecoder` ***must be completely consumed, emitting its [`Final`](Producer::Final) value*** in order
/// for further calls to [`DropDecoder::produce`] to make progress.
pub struct DropDecoder<P>
where
    P: BulkProducer<Item = u8>,
{
    drop_bytes: Rc<TakeCell<Option<P>>>,
    entry_state: Option<AuthorisedEntry>,
    chunk_offset: u64,
}

/// A [`BulkProducer<Item=u8>`] produced by a [`DropDecoder`], which produces the [verifiable slice stream](https://bab-hash.org/spec#slice_verification) for one contiguous slice of the payload of one [`AuthorisedEntry`] decoded from the [Willow Drop Format](https://willowprotocol.org/specs/drop-format/index.html#willow_drop_format).
///
/// # Warning
///
/// Any `SliceStreamProducer` produced by a [`DropDecoder`] ***must be completely consumed, emitting its [`Final`](Producer::Final) value*** in order for further calls to [`DropDecoder::produce`] to make progress.
#[must_use = "A DropDecoder which produces a SliceStreamProducer cannot make progress until the SliceStreamProducer is completely consumed."]
pub struct SliceStreamProducer<P>
where
    P: BulkProducer<Item = u8>,
{
    slice_bytes: Option<Limit<P>>,
    return_slot: Rc<TakeCell<Option<P>>>,
}

impl<P: BulkProducer<Item = u8>> SliceStreamProducer<P> {
    /// Manually constructs a `PayloadProducer` from a [limited](ProducerExt::to_limit) [`BulkProducer<Item=u8>`]. For development and debugging purposes only.
    #[cfg(feature = "dev")]
    #[doc(hidden)]
    pub fn new(inner: Limit<P>) -> Self {
        SliceStreamProducer {
            slice_bytes: Some(inner),
            return_slot: Rc::new(TakeCell::new()),
        }
    }
}

impl<P> DropDecoder<P>
where
    P: BulkProducer<Item = u8>,
{
    /// Creates a new decoder from the given `drop_producer`. The `drop_producer` must emit the bytes of an encoded [Willow drop](https://willowprotocol.org/specs/drop-format/index.html#drop). The `DropDecoder` then acts as a producer of decoded entries.
    pub fn new(drop_producer: P) -> Self {
        Self {
            drop_bytes: Rc::new(TakeCell::new_with(Some(drop_producer))),
            entry_state: None,
            chunk_offset: 0,
        }
    }
}

impl<P> From<P> for DropDecoder<P>
where
    P: BulkProducer<Item = u8>,
{
    fn from(value: P) -> Self {
        DropDecoder::new(value)
    }
}

impl<P> Producer for DropDecoder<P>
where
    P: BulkProducer<Item = u8, Final = ()>,
{
    type Item = (DropSliceMetadata, SliceStreamProducer<P>);

    type Final = ();

    type Error = DecodeError<(), P::Error, Blame>;

    async fn produce(&mut self) -> Result<Either<Self::Item, Self::Final>, Self::Error> {
        let Some(mut bytes) = self.drop_bytes.take().await else {
            return Err(DecodeError::UnexpectedEndOfInput(()));
        };

        let header = match bytes.produce().await.map_err(DecodeError::ProducerError)? {
            Left(0u8) => {
                // The zero byte indicates that the drop is finished.
                return Ok(Right(()));
            }
            Left(header) => header,
            Right(_) => {
                // We reached the end of the stream without the zero byte indicating the end of the drop!
                return Err(DecodeError::UnexpectedEndOfInput(()));
            }
        };

        if header & 0b1100_0000 == 0b0100_0000 {
            // We are processing a new entry.

            let entry_state = self
                .entry_state
                .take()
                .unwrap_or_else(default_authorised_entry);
            self.chunk_offset = 0;

            let namespace_is_encoded = is_bitflagged(header, 2);
            let subspace_is_encoded = is_bitflagged(header, 3);

            let namespace_id = if namespace_is_encoded {
                NamespaceId::decode(&mut bytes)
                    .await
                    .map_err(|err| err.map_other(|_infallbile| unreachable!()))?
            } else {
                entry_state.namespace_id().clone()
            };

            let subspace_id = if subspace_is_encoded {
                SubspaceId::decode(&mut bytes)
                    .await
                    .map_err(|err| err.map_other(|_infallbile| unreachable!()))?
            } else {
                entry_state.subspace_id().clone()
            };

            let path = Path::relative_decode(entry_state.path(), &mut bytes).await?;

            let timestamp = cu64_decode(header, 2, 4, &mut bytes)
                .await
                .map_err(|err| err.map_other(|_infallbile| unreachable!()))?;

            let payload_length = cu64_decode_standalone(&mut bytes)
                .await
                .map_err(|err| err.map_other(|_infallbile| unreachable!()))?;

            let payload_digest = PayloadDigest::decode(&mut bytes)
                .await
                .map_err(|err| err.map_other(|_infallbile| unreachable!()))?;

            let entry = Entry::builder()
                .namespace_id(namespace_id)
                .subspace_id(subspace_id)
                .path(path)
                .timestamp(timestamp)
                .payload_length(payload_length)
                .payload_digest(payload_digest)
                .build();

            let pair = PriorAuthedEntryEntryPair::new(entry_state.clone(), entry);

            let auth_token = AuthorisationToken::relative_decode(&pair, &mut bytes).await?;

            let (_, entry) = pair.into_parts();

            let decoded_authed_entry = PossiblyAuthorisedEntry::new(entry, auth_token)
                .into_authorised_entry()
                .map_err(|_| DecodeError::Other(Blame::TheirFault))?;

            self.entry_state = Some(decoded_authed_entry.clone());
        }

        let entry_state = self
            .entry_state
            .take()
            .ok_or(DecodeError::Other(Blame::TheirFault))?;

        let payload_length = entry_state.payload_length();
        let max_chunks = payload_length.div_ceil(CHUNK_SIZE as u64);

        let (chunk_count, left_skip, is_multislice) = match header & 0b0000_0011 {
            // No payload included
            0b0000_0000 => (0, 0, false),

            // Complete payload included
            0b0000_0001 => (max_chunks, 0, false),

            // Single incomplete payload slice starting from 0
            0b0000_0010 => {
                let prefix_chunks = cu64_decode_standalone(&mut bytes)
                    .await
                    .map_err(|err| err.map_other(|_infallbile| unreachable!()))?;
                (prefix_chunks, 0, false)
            }

            // Partial slice or slices offset from 0
            0b0000_0011 => {
                match bytes
                    .peek(async |&next| is_bitflagged(next, 0))
                    .await
                    .map_err(DecodeError::ProducerError)?
                {
                    Left(true) => {
                        // The next byte is the start of a new slice, carry on

                        let Left(slice_header) =
                            bytes.produce().await.map_err(DecodeError::ProducerError)?
                        else {
                            return Err(DecodeError::UnexpectedEndOfInput(()));
                        };

                        let prior_end = self.chunk_offset;

                        self.chunk_offset += cu64_decode(slice_header, 3, 1, &mut bytes)
                            .await
                            .map_err(|err| err.map_other(|_infallbile| unreachable!()))?;

                        let chunk_count = cu64_decode(slice_header, 4, 4, &mut bytes)
                            .await
                            .map_err(|err| err.map_other(|_infallbile| unreachable!()))?;

                        let left_skip =
                            optimal_left_skip(chunk_count, prior_end, self.chunk_offset);

                        (chunk_count, left_skip, true)
                    }
                    Left(false) => (0, 0, true), // The next byte either starts a new entry or ends the drop, so there was no slice data
                    _ => return Err(DecodeError::UnexpectedEndOfInput(())),
                }
            }
            _ => unreachable!(),
        };

        let metadata = DropSliceMetadata::new(
            entry_state.clone(),
            self.chunk_offset,
            chunk_count,
            left_skip,
            is_multislice,
        )
        .map_err(|_| DecodeError::Other(Blame::TheirFault))?;

        let payload = SliceStreamProducer {
            slice_bytes: Some(
                bytes.to_limit(
                    metadata
                        .expected_bytes()
                        .try_into()
                        .map_err(|_| DecodeError::Other(Blame::OurFault))?,
                ),
            ),
            return_slot: Rc::clone(&self.drop_bytes),
        };

        self.chunk_offset += chunk_count;
        self.entry_state = Some(entry_state);

        Ok(Left((metadata, payload)))
    }

    async fn slurp(&mut self) -> Result<(), Self::Error> {
        let Some(mut bytes) = self.drop_bytes.take().await else {
            return Err(DecodeError::UnexpectedEndOfInput(()));
        };

        match bytes.slurp().await {
            Ok(()) => self.drop_bytes.set(Some(bytes)),
            Err(err) => {
                self.drop_bytes.set(None);
                return Err(DecodeError::ProducerError(err));
            }
        }
        Ok(())
    }
}

impl<P> Producer for SliceStreamProducer<P>
where
    P: BulkProducer<Item = u8>,
{
    type Item = P::Item;

    type Final = ();

    type Error = DecodeError<(), P::Error, Blame>;

    async fn produce(&mut self) -> Result<Either<Self::Item, Self::Final>, Self::Error> {
        let mut bytes = self.slice_bytes.take().expect(
            "produce must not be called on a producer which has emitted an error or a final value",
        );
        match bytes.produce().await {
            Ok(Left(byte)) => {
                self.slice_bytes = Some(bytes);
                Ok(Left(byte))
            }
            Ok(Right(_)) => {
                self.return_slot.set(None);
                Err(DecodeError::UnexpectedEndOfInput(()))
            }
            Err(err) => match err {
                ProduceLimitError::Inner(err) => {
                    self.return_slot.set(None);
                    Err(DecodeError::ProducerError(err))
                }
                ProduceLimitError::LimitReached => {
                    self.return_slot.set(Some(bytes.into_inner()));
                    Ok(Right(()))
                }
            },
        }
    }

    async fn slurp(&mut self) -> Result<(), Self::Error> {
        let mut bytes = self.slice_bytes.take().expect(
            "slurp must not be called on a producer which has emitted an error or a final value",
        );
        match bytes.slurp().await {
            Ok(()) => {
                self.slice_bytes = Some(bytes);
                Ok(())
            }
            Err(err) => match err {
                ProduceLimitError::Inner(err) => {
                    self.return_slot.set(None);
                    Err(DecodeError::ProducerError(err))
                }
                ProduceLimitError::LimitReached => unreachable!(
                    "slurping on a limit producer should never cause the limit to be reached"
                ),
            },
        }
    }
}

impl<P> BulkProducer for SliceStreamProducer<P>
where
    P: BulkProducer<Item = u8>,
{
    async fn expose_items_gracefully<F, R>(
        &mut self,
        f: F,
    ) -> Result<Either<R, (F, Self::Final)>, (F, Self::Error)>
    where
        F: AsyncFnOnce(&[Self::Item]) -> (usize, R),
    {
        let mut bytes = self.slice_bytes.take().expect("producer methods must not be called on a producer which has emitted an error or a final value");
        match bytes.expose_items_gracefully(f).await {
            Ok(Left(returned)) => {
                self.slice_bytes = Some(bytes);
                Ok(Left(returned))
            }
            Ok(Right((f, _))) => {
                // The slice stream contained fewer bytes than promised by the decoded entry metadata
                self.return_slot.set(None);
                Err((f, DecodeError::UnexpectedEndOfInput(())))
            }
            Err((f, err)) => match err {
                ProduceLimitError::Inner(err) => {
                    self.return_slot.set(None);
                    Err((f, DecodeError::ProducerError(err)))
                }
                ProduceLimitError::LimitReached => {
                    self.return_slot.set(Some(bytes.into_inner()));
                    Ok(Right((f, ())))
                }
            },
        }
    }
}
