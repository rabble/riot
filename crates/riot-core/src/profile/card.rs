//! The profile payload: exactly one display-name field, canonically encoded.
//! Same rules as `apps/endorse.rs` — definite lengths, ascending integer
//! keys, no trailing bytes, and a decode-side re-encode equality proof so a
//! non-canonical encoding of the same value can never be admitted.

use minicbor::data::Type;
use minicbor::{Decoder, Encoder};

use super::ProfileError;

pub const MAX_DISPLAY_NAME_BYTES: usize = 64;
pub const MAX_PROFILE_CARD_BYTES: usize = 256;

/// The number of top-level CBOR map entries a canonical card always has.
const FIELD_COUNT: u64 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileCard {
    /// Self-claimed, unverified. Never render without the key suffix — see
    /// `resolver::render_display_name`.
    pub display_name: String,
}

fn validate(card: &ProfileCard) -> Result<(), ProfileError> {
    if card.display_name.is_empty() || card.display_name.len() > MAX_DISPLAY_NAME_BYTES {
        return Err(ProfileError::FieldInvalid);
    }
    Ok(())
}

pub fn encode_profile_card(card: &ProfileCard) -> Result<Vec<u8>, ProfileError> {
    validate(card)?;

    let mut buffer: Vec<u8> = Vec::new();
    let mut e = Encoder::new(&mut buffer);
    let r: Result<_, minicbor::encode::Error<core::convert::Infallible>> = (|| {
        e.map(FIELD_COUNT)?;
        e.u8(0)?.str(&card.display_name)?;
        Ok(())
    })();
    r.map_err(|_| ProfileError::FieldInvalid)?;

    if buffer.len() > MAX_PROFILE_CARD_BYTES {
        return Err(ProfileError::FieldInvalid);
    }
    Ok(buffer)
}

pub fn decode_profile_card(input: &[u8]) -> Result<ProfileCard, ProfileError> {
    if input.len() > MAX_PROFILE_CARD_BYTES {
        return Err(ProfileError::FieldInvalid);
    }

    let mut d = Decoder::new(input);
    let err = |_| ProfileError::FieldInvalid;

    if d.map().map_err(err)? != Some(FIELD_COUNT) {
        return Err(ProfileError::FieldInvalid);
    }
    if d.u8().map_err(err)? != 0 {
        return Err(ProfileError::FieldInvalid);
    }
    if d.datatype().map_err(err)? != Type::String {
        return Err(ProfileError::FieldInvalid);
    }
    let display_name = d.str().map_err(err)?.to_string();

    if d.position() != input.len() {
        return Err(ProfileError::FieldInvalid);
    }

    let card = ProfileCard { display_name };
    validate(&card)?;

    // Canonicality proof: only the exact encoder output is acceptable.
    if encode_profile_card(&card)? != input {
        return Err(ProfileError::FieldInvalid);
    }
    Ok(card)
}
