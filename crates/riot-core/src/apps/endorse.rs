//! Endorsement marker: a signed "we use this" from an organizer, stored one
//! per (app, endorser subspace) at `app_index_endorsement_path`. Overwrite
//! to update; set `retracted` to withdraw. Canonical minicbor, same rules
//! as `manifest.rs`: definite lengths only, ascending integer map keys, no
//! duplicate or unknown keys, no trailing bytes, and decoding re-validates
//! the same rules encoding enforces.

use minicbor::data::Type;
use minicbor::{Decoder, Encoder};

use crate::session::{commit_at, EvidenceStore};
use crate::willow::identity::EvidenceAuthor;

use super::manifest::AppId;
use super::AppsError;

pub const MAX_ENDORSEMENT_NOTE_BYTES: usize = 200;
pub const MAX_ENDORSEMENT_BYTES: usize = 512;

/// The number of top-level CBOR map entries a canonical endorsement always has.
const FIELD_COUNT: u64 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndorsementMarker {
    /// Repeats the app id so the marker bytes stay bound to the app even
    /// if seen out of path context.
    pub app_id: AppId,
    /// Optional short plain-language note ("we ran jail support with
    /// this"); empty string means no note.
    pub note: String,
    pub retracted: bool,
}

/// Writes a marker at the endorser's own Willow coordinate. Rewriting the
/// same coordinate relies on Willow recency in `commit_at`.
pub fn write_endorsement(
    store: &EvidenceStore,
    endorser: &EvidenceAuthor,
    marker: &EndorsementMarker,
    willow_timestamp_micros: u64,
) -> Result<(), AppsError> {
    let payload = encode_endorsement(marker)?;
    let path =
        super::index::app_index_endorsement_path(&marker.app_id, endorser.subspace_id().as_bytes())
            .expect("fixed-size app-index endorsement path is always valid");
    commit_at(store, endorser, &path, &payload, willow_timestamp_micros)
}

fn validate(marker: &EndorsementMarker) -> Result<(), AppsError> {
    if marker.note.len() > MAX_ENDORSEMENT_NOTE_BYTES {
        return Err(AppsError::EndorsementFieldInvalid);
    }
    Ok(())
}

/// Validates and encodes the canonical byte representation.
pub fn encode_endorsement(marker: &EndorsementMarker) -> Result<Vec<u8>, AppsError> {
    validate(marker)?;
    Ok(encode_validated_endorsement(marker))
}

/// Encodes a marker that has already passed [`validate`]. Its maximum encoded
/// size is well below `MAX_ENDORSEMENT_BYTES`, and `Vec<u8>` is an infallible
/// minicbor writer.
fn encode_validated_endorsement(marker: &EndorsementMarker) -> Vec<u8> {
    let mut buffer: Vec<u8> = Vec::new();
    let mut e = Encoder::new(&mut buffer);
    let _ = e.map(FIELD_COUNT);
    let _ = e.u8(0);
    let _ = e.bytes(&marker.app_id);
    let _ = e.u8(1);
    let _ = e.str(&marker.note);
    let _ = e.u8(2);
    let _ = e.u8(u8::from(marker.retracted));
    buffer
}

/// Strict canonical decoder: rejects unknown/duplicate/misordered keys,
/// indefinite lengths, trailing bytes, and any non-canonical encoding.
pub fn decode_endorsement(input: &[u8]) -> Result<EndorsementMarker, AppsError> {
    if input.len() > MAX_ENDORSEMENT_BYTES {
        return Err(AppsError::EndorsementFieldInvalid);
    }

    let mut d = Decoder::new(input);
    let pairs = d
        .map()
        .map_err(|_| AppsError::EndorsementFieldInvalid)?
        .ok_or(AppsError::EndorsementFieldInvalid)?;
    if pairs != FIELD_COUNT {
        return Err(AppsError::EndorsementFieldInvalid);
    }

    if d.u64().map_err(|_| AppsError::EndorsementFieldInvalid)? != 0 {
        return Err(AppsError::EndorsementFieldInvalid);
    }
    let app_id = decode_id32(&mut d)?;
    if d.u64().map_err(|_| AppsError::EndorsementFieldInvalid)? != 1 {
        return Err(AppsError::EndorsementFieldInvalid);
    }
    let note = decode_text(&mut d, MAX_ENDORSEMENT_NOTE_BYTES)?;
    if d.u64().map_err(|_| AppsError::EndorsementFieldInvalid)? != 2 {
        return Err(AppsError::EndorsementFieldInvalid);
    }
    let retracted = match d.u8().map_err(|_| AppsError::EndorsementFieldInvalid)? {
        0 => false,
        1 => true,
        _ => return Err(AppsError::EndorsementFieldInvalid),
    };

    if d.position() != input.len() {
        return Err(AppsError::EndorsementFieldInvalid);
    }

    let marker = EndorsementMarker {
        app_id,
        note,
        retracted,
    };

    // Canonicality proof: only the exact encoder output is acceptable.
    // `decode_text` already enforces the sole marker invariant (note length),
    // so no second validation pass is needed here.
    let reencoded = encode_validated_endorsement(&marker);
    if reencoded != input {
        return Err(AppsError::EndorsementFieldInvalid);
    }

    Ok(marker)
}

fn decode_id32(d: &mut Decoder<'_>) -> Result<[u8; 32], AppsError> {
    let bytes = d.bytes().map_err(|_| AppsError::EndorsementFieldInvalid)?;
    <[u8; 32]>::try_from(bytes).map_err(|_| AppsError::EndorsementFieldInvalid)
}

fn decode_text(d: &mut Decoder<'_>, max: usize) -> Result<String, AppsError> {
    if d.datatype()
        .map_err(|_| AppsError::EndorsementFieldInvalid)?
        != Type::String
    {
        return Err(AppsError::EndorsementFieldInvalid);
    }
    let text = d.str().map_err(|_| AppsError::EndorsementFieldInvalid)?;
    if text.len() > max {
        return Err(AppsError::EndorsementFieldInvalid);
    }
    Ok(text.to_string())
}
