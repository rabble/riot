use minicbor::{Decoder, Encoder};

use crate::import::MAX_BUNDLE_BYTES;
use crate::willow::EntryId;

const SYNC_CODEC: &str = "org.riot.conference-sync/1";
pub const MAX_SYNC_IDS: usize = 64;
pub const MAX_SYNC_FRAME_BYTES: usize = MAX_BUNDLE_BYTES + 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncError {
    FrameTooLarge,
    MalformedFrame,
    NonCanonicalFrame,
    UnsupportedCodec,
    TooManyEntryIds,
    DuplicateEntryId,
    EntryIdsNotSorted,
    BundleTooLarge,
    NamespaceMismatch,
    UnexpectedFrame,
    UnknownEntryId,
    InvalidBundle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncFrame {
    Hello {
        namespace_id: [u8; 32],
    },
    Summary {
        namespace_id: [u8; 32],
        entry_ids: Vec<EntryId>,
    },
    Request {
        namespace_id: [u8; 32],
        entry_ids: Vec<EntryId>,
    },
    Entries {
        namespace_id: [u8; 32],
        bundle_bytes: Vec<u8>,
    },
    Complete {
        namespace_id: [u8; 32],
    },
    Reject {
        namespace_id: [u8; 32],
        code: u8,
    },
}

pub fn encode_frame(frame: &SyncFrame) -> Result<Vec<u8>, SyncError> {
    validate(frame)?;
    Ok(encode_validated_frame(frame))
}

/// Encodes a frame that has passed [`validate`]. All frame variants are
/// bounded below [`MAX_SYNC_FRAME_BYTES`], and `Vec<u8>` is an infallible
/// minicbor writer.
fn encode_validated_frame(frame: &SyncFrame) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = Encoder::new(&mut bytes);
    let _ = encoder.map(3);
    let _ = encoder.u8(0);
    let _ = encoder.str(SYNC_CODEC);
    let _ = encoder.u8(1);
    let _ = encoder.u8(kind(frame));
    let _ = encoder.u8(2);
    match frame {
        SyncFrame::Hello { namespace_id } | SyncFrame::Complete { namespace_id } => {
            let _ = encoder.bytes(namespace_id);
        }
        SyncFrame::Summary {
            namespace_id,
            entry_ids,
        }
        | SyncFrame::Request {
            namespace_id,
            entry_ids,
        } => {
            let _ = encoder.array(2);
            let _ = encoder.bytes(namespace_id);
            let _ = encoder.array(entry_ids.len() as u64);
            for entry_id in entry_ids {
                let _ = encoder.bytes(entry_id);
            }
        }
        SyncFrame::Entries {
            namespace_id,
            bundle_bytes,
        } => {
            let _ = encoder.array(2);
            let _ = encoder.bytes(namespace_id);
            let _ = encoder.bytes(bundle_bytes);
        }
        SyncFrame::Reject { namespace_id, code } => {
            let _ = encoder.array(2);
            let _ = encoder.bytes(namespace_id);
            let _ = encoder.u8(*code);
        }
    }
    debug_assert!(bytes.len() <= MAX_SYNC_FRAME_BYTES);
    bytes
}

pub fn decode_frame(bytes: &[u8]) -> Result<SyncFrame, SyncError> {
    if bytes.len() > MAX_SYNC_FRAME_BYTES {
        return Err(SyncError::FrameTooLarge);
    }
    let frame = parse_frame(bytes)?;
    let canonical = encode_validated_frame(&frame);
    if canonical != bytes {
        return Err(SyncError::NonCanonicalFrame);
    }
    Ok(frame)
}

fn parse_frame(bytes: &[u8]) -> Result<SyncFrame, SyncError> {
    let mut decoder = Decoder::new(bytes);
    if decoder.map().ok().flatten() != Some(3) || decoder.u8().ok() != Some(0) {
        return Err(SyncError::MalformedFrame);
    }
    let codec = decoder.str().map_err(|_| SyncError::MalformedFrame)?;
    if codec != SYNC_CODEC {
        return Err(SyncError::UnsupportedCodec);
    }
    if decoder.u8().ok() != Some(1) {
        return Err(SyncError::MalformedFrame);
    }
    let kind = decoder.u8().map_err(|_| SyncError::MalformedFrame)?;
    if decoder.u8().ok() != Some(2) {
        return Err(SyncError::MalformedFrame);
    }

    let frame = match kind {
        0 => SyncFrame::Hello {
            namespace_id: read_fixed_32(&mut decoder)?,
        },
        1 => {
            let (namespace_id, entry_ids) = read_id_body(&mut decoder)?;
            SyncFrame::Summary {
                namespace_id,
                entry_ids,
            }
        }
        2 => {
            let (namespace_id, entry_ids) = read_id_body(&mut decoder)?;
            SyncFrame::Request {
                namespace_id,
                entry_ids,
            }
        }
        3 => {
            require_array(&mut decoder, 2)?;
            let namespace_id = read_fixed_32(&mut decoder)?;
            let bundle_bytes = decoder
                .bytes()
                .map_err(|_| SyncError::MalformedFrame)?
                .to_vec();
            SyncFrame::Entries {
                namespace_id,
                bundle_bytes,
            }
        }
        4 => SyncFrame::Complete {
            namespace_id: read_fixed_32(&mut decoder)?,
        },
        5 => {
            require_array(&mut decoder, 2)?;
            SyncFrame::Reject {
                namespace_id: read_fixed_32(&mut decoder)?,
                code: decoder.u8().map_err(|_| SyncError::MalformedFrame)?,
            }
        }
        _ => return Err(SyncError::MalformedFrame),
    };
    if decoder.position() != bytes.len() {
        return Err(SyncError::NonCanonicalFrame);
    }
    validate(&frame)?;
    Ok(frame)
}

fn read_id_body(decoder: &mut Decoder<'_>) -> Result<([u8; 32], Vec<EntryId>), SyncError> {
    require_array(decoder, 2)?;
    let namespace_id = read_fixed_32(decoder)?;
    let count = decoder
        .array()
        .map_err(|_| SyncError::MalformedFrame)?
        .ok_or(SyncError::MalformedFrame)?;
    if count > MAX_SYNC_IDS as u64 {
        return Err(SyncError::TooManyEntryIds);
    }
    let count = count as usize;
    let mut ids = Vec::with_capacity(count);
    for _ in 0..count {
        ids.push(read_fixed_32(decoder)?);
    }
    Ok((namespace_id, ids))
}

fn read_fixed_32(decoder: &mut Decoder<'_>) -> Result<[u8; 32], SyncError> {
    let bytes = decoder.bytes().map_err(|_| SyncError::MalformedFrame)?;
    bytes.try_into().map_err(|_| SyncError::MalformedFrame)
}

fn require_array(decoder: &mut Decoder<'_>, length: u64) -> Result<(), SyncError> {
    if decoder.array().ok().flatten() == Some(length) {
        Ok(())
    } else {
        Err(SyncError::MalformedFrame)
    }
}

fn validate(frame: &SyncFrame) -> Result<(), SyncError> {
    match frame {
        SyncFrame::Summary { entry_ids, .. } | SyncFrame::Request { entry_ids, .. } => {
            if entry_ids.len() > MAX_SYNC_IDS {
                return Err(SyncError::TooManyEntryIds);
            }
            for pair in entry_ids.windows(2) {
                if pair[0] == pair[1] {
                    return Err(SyncError::DuplicateEntryId);
                }
                if pair[0] > pair[1] {
                    return Err(SyncError::EntryIdsNotSorted);
                }
            }
        }
        SyncFrame::Entries { bundle_bytes, .. } if bundle_bytes.len() > MAX_BUNDLE_BYTES => {
            return Err(SyncError::BundleTooLarge);
        }
        _ => {}
    }
    Ok(())
}

fn kind(frame: &SyncFrame) -> u8 {
    match frame {
        SyncFrame::Hello { .. } => 0,
        SyncFrame::Summary { .. } => 1,
        SyncFrame::Request { .. } => 2,
        SyncFrame::Entries { .. } => 3,
        SyncFrame::Complete { .. } => 4,
        SyncFrame::Reject { .. } => 5,
    }
}
