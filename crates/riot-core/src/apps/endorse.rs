//! Endorsement marker: a signed "we use this" from an organizer, stored one
//! per (app, endorser subspace) at `app_index_endorsement_path`. Overwrite
//! to update; set `retracted` to withdraw. Canonical minicbor, same rules
//! as `manifest.rs`: definite lengths only, ascending integer map keys, no
//! duplicate or unknown keys, no trailing bytes, and decoding re-validates
//! the same rules encoding enforces.

use minicbor::data::Type;
use minicbor::{Decoder, Encoder};

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

fn validate(marker: &EndorsementMarker) -> Result<(), AppsError> {
    if marker.note.len() > MAX_ENDORSEMENT_NOTE_BYTES {
        return Err(AppsError::EndorsementFieldInvalid);
    }
    Ok(())
}

/// Validates and encodes the canonical byte representation.
pub fn encode_endorsement(marker: &EndorsementMarker) -> Result<Vec<u8>, AppsError> {
    validate(marker)?;

    let mut buffer: Vec<u8> = Vec::new();
    let mut e = Encoder::new(&mut buffer);
    let r: Result<_, minicbor::encode::Error<core::convert::Infallible>> = (|| {
        e.map(FIELD_COUNT)?;
        e.u8(0)?.bytes(&marker.app_id)?;
        e.u8(1)?.str(&marker.note)?;
        e.u8(2)?.u8(u8::from(marker.retracted))?;
        Ok(())
    })();
    r.map_err(|_| AppsError::EndorsementFieldInvalid)?;

    if buffer.len() > MAX_ENDORSEMENT_BYTES {
        return Err(AppsError::EndorsementFieldInvalid);
    }
    Ok(buffer)
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

    let mut app_id: Option<AppId> = None;
    let mut note: Option<String> = None;
    let mut retracted: Option<bool> = None;

    let mut last_key: Option<u64> = None;
    for _ in 0..pairs {
        let key = d.u64().map_err(|_| AppsError::EndorsementFieldInvalid)?;
        if let Some(previous) = last_key {
            if key <= previous {
                return Err(AppsError::EndorsementFieldInvalid);
            }
        }
        last_key = Some(key);

        match key {
            0 => app_id = Some(decode_id32(&mut d)?),
            1 => note = Some(decode_text(&mut d, MAX_ENDORSEMENT_NOTE_BYTES)?),
            2 => {
                let raw = d.u8().map_err(|_| AppsError::EndorsementFieldInvalid)?;
                retracted = Some(match raw {
                    0 => false,
                    1 => true,
                    _ => return Err(AppsError::EndorsementFieldInvalid),
                });
            }
            _ => return Err(AppsError::EndorsementFieldInvalid),
        }
    }

    if d.position() != input.len() {
        return Err(AppsError::EndorsementFieldInvalid);
    }

    let marker = EndorsementMarker {
        app_id: app_id.ok_or(AppsError::EndorsementFieldInvalid)?,
        note: note.ok_or(AppsError::EndorsementFieldInvalid)?,
        retracted: retracted.ok_or(AppsError::EndorsementFieldInvalid)?,
    };

    validate(&marker)?;

    // Canonicality proof: only the exact encoder output is acceptable.
    let reencoded = encode_endorsement(&marker)?;
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
    if d.datatype().map_err(|_| AppsError::EndorsementFieldInvalid)? != Type::String {
        return Err(AppsError::EndorsementFieldInvalid);
    }
    let text = d.str().map_err(|_| AppsError::EndorsementFieldInvalid)?;
    if text.len() > max {
        return Err(AppsError::EndorsementFieldInvalid);
    }
    Ok(text.to_string())
}
