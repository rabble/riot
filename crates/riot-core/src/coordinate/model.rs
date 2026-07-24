//! Frozen Coordinate v1 data model and strict canonical CBOR codec.
//!
//! WU-1 lands the single object-kind record [`CoordinateItemV1`] — the ask
//! (`Need`), `Offer`, and `Task`, carried under one schema so the codec/path/
//! inspect machinery is written once. The item is **immutable** once signed;
//! lifecycle changes (claim/complete/cancel) are separate status records added
//! in later work units.
//!
//! The codec follows the newswire convention exactly (`crate::newswire::model`):
//! a definite CBOR map keyed by strictly-ascending small integers, schema string
//! at key 0, `validate` before encode and after decode, and a canonical-form
//! proof (`prove_canonical`) that re-encodes and compares byte-for-byte.

use minicbor::data::Type;
use minicbor::{Decoder, Encoder};

pub const COORDINATE_ITEM_SCHEMA: &str = "org.riot.coordinate.item/1";
pub const MAX_COORDINATE_PAYLOAD_BYTES: usize = 131_072;

const MAX_TITLE_BYTES: usize = 512;
const MAX_BODY_BYTES: usize = 65_536;
const MIN_LANGUAGE_BYTES: usize = 2;
const MAX_LANGUAGE_BYTES: usize = 35;
const MAX_CATEGORY_TAGS: usize = 32;
const MAX_TAG_BYTES: usize = 128;
const MAX_COARSE_LOCATION_BYTES: usize = 2_048;
const MAX_CONTACT_INSTRUCTIONS_BYTES: usize = 2_048;
const MAX_SOURCE_CLAIMS: usize = 16;
const MAX_SOURCE_CLAIM_BYTES: usize = 1_024;

/// The object kind of a Coordinate item. Symmetric on the wire — the product
/// asymmetry ("Ask for help" is the prominent action; asks carry no capacity)
/// lives in validation and the Swift surface, not in this closed enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinateKind {
    Need = 0,
    Offer = 1,
    Task = 2,
}

/// The ask / offer / task record. One CBOR object kind for all three.
///
/// `*_unix_seconds` fields are **display metadata only**; the authoritative
/// record time is the TAI/J2000-microsecond timestamp stamped onto the Willow
/// entry path at signing time (a later work unit), never these fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoordinateItemV1 {
    /// Binds the item to the community room (a newswire space descriptor).
    pub space_descriptor_entry_id: [u8; 32],
    pub kind: CoordinateKind,
    pub title: String,
    pub body: String,
    pub language: String,
    /// Free-form categorization (Events / Help / Info …).
    pub category_tags: Vec<String>,
    /// Location-minimized, like a post. When present, `expires_at` is required.
    pub coarse_location: Option<String>,
    /// Task/Offer: how many claimants can fill it. `None` = single-claimant.
    /// An ask (`Need`) must leave this `None` — asks are not "filled N times".
    pub capacity: Option<u32>,
    /// Soft deadline (display + sort).
    pub needed_by_unix_seconds: Option<u64>,
    /// Hard expiry — the item drops off the open ledger after this.
    pub expires_at_unix_seconds: Option<u64>,
    /// How to reach the author. May be empty; bounded in length.
    pub contact_instructions: String,
    pub source_claims: Vec<String>,
    /// MANDATORY flag: a decode with this key absent is a `MissingKey` error.
    pub ai_assisted: bool,
}

/// Stable, closed failure vocabulary for both semantic validation and parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoordinateModelError {
    InputTooLarge,
    FieldEmpty(&'static str),
    FieldTooSmall(&'static str),
    FieldTooLarge(&'static str),
    TooManyEntries(&'static str),
    /// A `Task`/`Offer` declared `capacity: Some(0)`.
    CapacityZero,
    /// A `Need` (ask) carried a capacity — asks are single, uncounted.
    AskHasCapacity,
    /// An item carried a `coarse_location` without an `expires_at`.
    LocationRequiresExpiry,
    UnknownKey(u64),
    DuplicateOrMisorderedKey(u64),
    MissingKey(u64),
    WrongSchema,
    InvalidEnum(&'static str),
    NonCanonical,
    TrailingBytes,
    Malformed,
}

impl std::fmt::Display for CoordinateModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for CoordinateModelError {}

pub fn encode_coordinate_item(item: &CoordinateItemV1) -> Result<Vec<u8>, CoordinateModelError> {
    validate_item(item)?;
    let mut pairs = 10u64;
    pairs += u64::from(item.coarse_location.is_some());
    pairs += u64::from(item.capacity.is_some());
    pairs += u64::from(item.needed_by_unix_seconds.is_some());
    pairs += u64::from(item.expires_at_unix_seconds.is_some());

    encode_bounded(|e| {
        e.map(pairs)?;
        e.u8(0)?.str(COORDINATE_ITEM_SCHEMA)?;
        e.u8(1)?.bytes(&item.space_descriptor_entry_id)?;
        e.u8(2)?.u8(item.kind as u8)?;
        e.u8(3)?.str(&item.title)?;
        e.u8(4)?.str(&item.body)?;
        e.u8(5)?.str(&item.language)?;
        encode_text_array(e, 6, &item.category_tags)?;
        if let Some(location) = &item.coarse_location {
            e.u8(7)?.str(location)?;
        }
        if let Some(capacity) = item.capacity {
            e.u8(8)?.u32(capacity)?;
        }
        if let Some(needed_by) = item.needed_by_unix_seconds {
            e.u8(9)?.u64(needed_by)?;
        }
        if let Some(expires_at) = item.expires_at_unix_seconds {
            e.u8(10)?.u64(expires_at)?;
        }
        e.u8(11)?.str(&item.contact_instructions)?;
        encode_text_array(e, 12, &item.source_claims)?;
        e.u8(13)?.bool(item.ai_assisted)?;
        Ok(())
    })
}

pub fn decode_coordinate_item(input: &[u8]) -> Result<CoordinateItemV1, CoordinateModelError> {
    check_input_size(input)?;
    let mut d = Decoder::new(input);
    let pairs = definite_map(&mut d)?;
    if pairs > 14 {
        return Err(CoordinateModelError::Malformed);
    }

    let mut schema = None;
    let mut space_descriptor_entry_id = None;
    let mut kind = None;
    let mut title = None;
    let mut body = None;
    let mut language = None;
    let mut category_tags = None;
    let mut coarse_location = None;
    let mut capacity = None;
    let mut needed_by_unix_seconds = None;
    let mut expires_at_unix_seconds = None;
    let mut contact_instructions = None;
    let mut source_claims = None;
    let mut ai_assisted = None;
    let mut last_key = None;

    for _ in 0..pairs {
        let key = decode_ordered_key(&mut d, &mut last_key)?;
        match key {
            0 => schema = Some(decode_text(&mut d, "schema", 1, 64)?),
            1 => space_descriptor_entry_id = Some(decode_id32(&mut d)?),
            2 => {
                let raw = d.u8().map_err(|_| CoordinateModelError::Malformed)?;
                kind = Some(coordinate_kind_from_u8(raw)?);
            }
            3 => title = Some(decode_text(&mut d, "title", 1, MAX_TITLE_BYTES)?),
            4 => body = Some(decode_text(&mut d, "body", 1, MAX_BODY_BYTES)?),
            5 => {
                language = Some(decode_text(
                    &mut d,
                    "language",
                    MIN_LANGUAGE_BYTES,
                    MAX_LANGUAGE_BYTES,
                )?)
            }
            6 => {
                category_tags = Some(decode_text_array(
                    &mut d,
                    "category_tags",
                    MAX_CATEGORY_TAGS,
                    "category_tag",
                    1,
                    MAX_TAG_BYTES,
                )?)
            }
            7 => {
                coarse_location = Some(decode_text(
                    &mut d,
                    "coarse_location",
                    1,
                    MAX_COARSE_LOCATION_BYTES,
                )?)
            }
            8 => capacity = Some(decode_u32(&mut d)?),
            9 => needed_by_unix_seconds = Some(decode_u64(&mut d)?),
            10 => expires_at_unix_seconds = Some(decode_u64(&mut d)?),
            11 => {
                contact_instructions = Some(decode_bounded_text(
                    &mut d,
                    "contact_instructions",
                    MAX_CONTACT_INSTRUCTIONS_BYTES,
                )?)
            }
            12 => {
                source_claims = Some(decode_text_array(
                    &mut d,
                    "source_claims",
                    MAX_SOURCE_CLAIMS,
                    "source_claim",
                    1,
                    MAX_SOURCE_CLAIM_BYTES,
                )?)
            }
            13 => ai_assisted = Some(d.bool().map_err(|_| CoordinateModelError::Malformed)?),
            other => return Err(CoordinateModelError::UnknownKey(other)),
        }
    }
    finish_input(&d, input)?;
    if schema.as_deref() != Some(COORDINATE_ITEM_SCHEMA) {
        return Err(CoordinateModelError::WrongSchema);
    }

    let item = CoordinateItemV1 {
        space_descriptor_entry_id: space_descriptor_entry_id
            .ok_or(CoordinateModelError::MissingKey(1))?,
        kind: kind.ok_or(CoordinateModelError::MissingKey(2))?,
        title: title.ok_or(CoordinateModelError::MissingKey(3))?,
        body: body.ok_or(CoordinateModelError::MissingKey(4))?,
        language: language.ok_or(CoordinateModelError::MissingKey(5))?,
        category_tags: category_tags.ok_or(CoordinateModelError::MissingKey(6))?,
        coarse_location,
        capacity,
        needed_by_unix_seconds,
        expires_at_unix_seconds,
        contact_instructions: contact_instructions.ok_or(CoordinateModelError::MissingKey(11))?,
        source_claims: source_claims.ok_or(CoordinateModelError::MissingKey(12))?,
        ai_assisted: ai_assisted.ok_or(CoordinateModelError::MissingKey(13))?,
    };
    validate_item(&item)?;
    prove_canonical(input, encode_coordinate_item(&item)?)?;
    Ok(item)
}

fn validate_item(item: &CoordinateItemV1) -> Result<(), CoordinateModelError> {
    check_text("title", &item.title, 1, MAX_TITLE_BYTES)?;
    check_text("body", &item.body, 1, MAX_BODY_BYTES)?;
    check_text(
        "language",
        &item.language,
        MIN_LANGUAGE_BYTES,
        MAX_LANGUAGE_BYTES,
    )?;
    check_text_list(
        "category_tags",
        &item.category_tags,
        MAX_CATEGORY_TAGS,
        "category_tag",
        1,
        MAX_TAG_BYTES,
    )?;
    if let Some(location) = &item.coarse_location {
        check_text("coarse_location", location, 1, MAX_COARSE_LOCATION_BYTES)?;
    }
    check_bounded(
        "contact_instructions",
        &item.contact_instructions,
        MAX_CONTACT_INSTRUCTIONS_BYTES,
    )?;
    check_text_list(
        "source_claims",
        &item.source_claims,
        MAX_SOURCE_CLAIMS,
        "source_claim",
        1,
        MAX_SOURCE_CLAIM_BYTES,
    )?;

    match item.kind {
        CoordinateKind::Need => {
            if item.capacity.is_some() {
                return Err(CoordinateModelError::AskHasCapacity);
            }
        }
        CoordinateKind::Offer | CoordinateKind::Task => {
            if item.capacity == Some(0) {
                return Err(CoordinateModelError::CapacityZero);
            }
        }
    }

    if item.coarse_location.is_some() && item.expires_at_unix_seconds.is_none() {
        return Err(CoordinateModelError::LocationRequiresExpiry);
    }
    Ok(())
}

fn check_text(
    field: &'static str,
    value: &str,
    min: usize,
    max: usize,
) -> Result<(), CoordinateModelError> {
    if value.trim().is_empty() {
        return Err(CoordinateModelError::FieldEmpty(field));
    }
    if value.len() < min {
        return Err(CoordinateModelError::FieldTooSmall(field));
    }
    if value.len() > max {
        return Err(CoordinateModelError::FieldTooLarge(field));
    }
    Ok(())
}

/// Length-only bound: allows an empty string (used for `contact_instructions`,
/// which is optional in practice but always present on the wire as a `String`).
fn check_bounded(field: &'static str, value: &str, max: usize) -> Result<(), CoordinateModelError> {
    if value.len() > max {
        return Err(CoordinateModelError::FieldTooLarge(field));
    }
    Ok(())
}

fn check_text_list(
    list_field: &'static str,
    values: &[String],
    max_entries: usize,
    item_field: &'static str,
    min_bytes: usize,
    max_bytes: usize,
) -> Result<(), CoordinateModelError> {
    if values.len() > max_entries {
        return Err(CoordinateModelError::TooManyEntries(list_field));
    }
    for value in values {
        check_text(item_field, value, min_bytes, max_bytes)?;
    }
    Ok(())
}

fn encode_bounded<F>(encode: F) -> Result<Vec<u8>, CoordinateModelError>
where
    F: FnOnce(
        &mut Encoder<&mut Vec<u8>>,
    ) -> Result<(), minicbor::encode::Error<core::convert::Infallible>>,
{
    let mut buffer = Vec::new();
    let mut encoder = Encoder::new(&mut buffer);
    encode(&mut encoder).map_err(|_| CoordinateModelError::Malformed)?;
    if buffer.len() > MAX_COORDINATE_PAYLOAD_BYTES {
        return Err(CoordinateModelError::InputTooLarge);
    }
    Ok(buffer)
}

fn encode_text_array(
    e: &mut Encoder<&mut Vec<u8>>,
    key: u8,
    values: &[String],
) -> Result<(), minicbor::encode::Error<core::convert::Infallible>> {
    e.u8(key)?.array(values.len() as u64)?;
    for value in values {
        e.str(value)?;
    }
    Ok(())
}

fn check_input_size(input: &[u8]) -> Result<(), CoordinateModelError> {
    if input.len() > MAX_COORDINATE_PAYLOAD_BYTES {
        Err(CoordinateModelError::InputTooLarge)
    } else {
        Ok(())
    }
}

fn definite_map(d: &mut Decoder<'_>) -> Result<u64, CoordinateModelError> {
    d.map()
        .map_err(|_| CoordinateModelError::Malformed)?
        .ok_or(CoordinateModelError::NonCanonical)
}

fn definite_array(d: &mut Decoder<'_>) -> Result<u64, CoordinateModelError> {
    d.array()
        .map_err(|_| CoordinateModelError::Malformed)?
        .ok_or(CoordinateModelError::NonCanonical)
}

fn decode_ordered_key(
    d: &mut Decoder<'_>,
    last_key: &mut Option<u64>,
) -> Result<u64, CoordinateModelError> {
    let key = d.u64().map_err(|_| CoordinateModelError::Malformed)?;
    if last_key.is_some_and(|previous| key <= previous) {
        return Err(CoordinateModelError::DuplicateOrMisorderedKey(key));
    }
    *last_key = Some(key);
    Ok(key)
}

fn decode_id32(d: &mut Decoder<'_>) -> Result<[u8; 32], CoordinateModelError> {
    let bytes = d.bytes().map_err(|_| CoordinateModelError::Malformed)?;
    <[u8; 32]>::try_from(bytes).map_err(|_| CoordinateModelError::Malformed)
}

fn decode_u32(d: &mut Decoder<'_>) -> Result<u32, CoordinateModelError> {
    d.u32().map_err(|_| CoordinateModelError::Malformed)
}

fn decode_u64(d: &mut Decoder<'_>) -> Result<u64, CoordinateModelError> {
    d.u64().map_err(|_| CoordinateModelError::Malformed)
}

fn decode_text(
    d: &mut Decoder<'_>,
    field: &'static str,
    min: usize,
    max: usize,
) -> Result<String, CoordinateModelError> {
    if d.datatype().map_err(|_| CoordinateModelError::Malformed)? != Type::String {
        return Err(CoordinateModelError::Malformed);
    }
    let value = d.str().map_err(|_| CoordinateModelError::Malformed)?;
    check_text(field, value, min, max)?;
    Ok(value.to_string())
}

/// Decodes a length-bounded string that may be empty (mirror of `check_bounded`).
fn decode_bounded_text(
    d: &mut Decoder<'_>,
    field: &'static str,
    max: usize,
) -> Result<String, CoordinateModelError> {
    if d.datatype().map_err(|_| CoordinateModelError::Malformed)? != Type::String {
        return Err(CoordinateModelError::Malformed);
    }
    let value = d.str().map_err(|_| CoordinateModelError::Malformed)?;
    check_bounded(field, value, max)?;
    Ok(value.to_string())
}

fn decode_text_array(
    d: &mut Decoder<'_>,
    list_field: &'static str,
    max_entries: usize,
    item_field: &'static str,
    min_bytes: usize,
    max_bytes: usize,
) -> Result<Vec<String>, CoordinateModelError> {
    let len = definite_array(d)?;
    if len as usize > max_entries {
        return Err(CoordinateModelError::TooManyEntries(list_field));
    }
    let mut values = Vec::with_capacity(len as usize);
    for _ in 0..len {
        values.push(decode_text(d, item_field, min_bytes, max_bytes)?);
    }
    Ok(values)
}

fn finish_input(d: &Decoder<'_>, input: &[u8]) -> Result<(), CoordinateModelError> {
    if d.position() == input.len() {
        Ok(())
    } else {
        Err(CoordinateModelError::TrailingBytes)
    }
}

fn prove_canonical(input: &[u8], encoded: Vec<u8>) -> Result<(), CoordinateModelError> {
    if encoded == input {
        Ok(())
    } else {
        Err(CoordinateModelError::NonCanonical)
    }
}

fn coordinate_kind_from_u8(value: u8) -> Result<CoordinateKind, CoordinateModelError> {
    match value {
        0 => Ok(CoordinateKind::Need),
        1 => Ok(CoordinateKind::Offer),
        2 => Ok(CoordinateKind::Task),
        _ => Err(CoordinateModelError::InvalidEnum("coordinate_kind")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task() -> CoordinateItemV1 {
        CoordinateItemV1 {
            space_descriptor_entry_id: [0x11; 32],
            kind: CoordinateKind::Task,
            title: "Sort donations at the church hall".into(),
            body: "Two hours, Saturday morning. Sturdy shoes help.".into(),
            language: "en".into(),
            category_tags: vec!["help".into(), "logistics".into()],
            coarse_location: Some("Riverside, near the bridge".into()),
            capacity: Some(4),
            needed_by_unix_seconds: Some(1_800_000_200),
            expires_at_unix_seconds: Some(1_800_100_000),
            contact_instructions: "Ask for Maria at the door".into(),
            source_claims: vec![],
            ai_assisted: false,
        }
    }

    fn ask() -> CoordinateItemV1 {
        CoordinateItemV1 {
            space_descriptor_entry_id: [0x22; 32],
            kind: CoordinateKind::Need,
            title: "Need a ride to the clinic Tuesday".into(),
            body: "Morning appointment, can share fuel cost.".into(),
            language: "en".into(),
            category_tags: vec![],
            coarse_location: None,
            capacity: None,
            needed_by_unix_seconds: None,
            expires_at_unix_seconds: None,
            contact_instructions: String::new(),
            source_claims: vec![],
            ai_assisted: true,
        }
    }

    #[test]
    fn full_task_roundtrips() {
        let value = task();
        let bytes = encode_coordinate_item(&value).unwrap();
        assert_eq!(decode_coordinate_item(&bytes).unwrap(), value);
    }

    #[test]
    fn minimal_ask_roundtrips_with_empty_contact() {
        let value = ask();
        let bytes = encode_coordinate_item(&value).unwrap();
        assert_eq!(decode_coordinate_item(&bytes).unwrap(), value);
    }

    #[test]
    fn offer_with_capacity_roundtrips() {
        let mut value = task();
        value.kind = CoordinateKind::Offer;
        value.capacity = Some(1);
        let bytes = encode_coordinate_item(&value).unwrap();
        assert_eq!(decode_coordinate_item(&bytes).unwrap(), value);
    }

    #[test]
    fn all_three_kinds_encode_distinct_tag_bytes() {
        for (kind, tag) in [
            (CoordinateKind::Need, 0u8),
            (CoordinateKind::Offer, 1u8),
            (CoordinateKind::Task, 2u8),
        ] {
            assert_eq!(kind as u8, tag);
        }
    }

    #[test]
    fn ask_with_capacity_is_rejected() {
        let mut value = ask();
        value.capacity = Some(1);
        assert_eq!(
            encode_coordinate_item(&value),
            Err(CoordinateModelError::AskHasCapacity)
        );
    }

    #[test]
    fn task_with_zero_capacity_is_rejected() {
        let mut value = task();
        value.capacity = Some(0);
        assert_eq!(
            encode_coordinate_item(&value),
            Err(CoordinateModelError::CapacityZero)
        );
    }

    #[test]
    fn offer_with_zero_capacity_is_rejected() {
        let mut value = task();
        value.kind = CoordinateKind::Offer;
        value.capacity = Some(0);
        assert_eq!(
            encode_coordinate_item(&value),
            Err(CoordinateModelError::CapacityZero)
        );
    }

    #[test]
    fn location_without_expiry_is_rejected() {
        let mut value = task();
        value.expires_at_unix_seconds = None;
        assert_eq!(
            encode_coordinate_item(&value),
            Err(CoordinateModelError::LocationRequiresExpiry)
        );
    }

    #[test]
    fn missing_ai_assisted_key_is_a_missing_key_error() {
        // Hand-build a canonical map with keys 0..=6, 11, 12 — every mandatory
        // key EXCEPT ai_assisted (13). Decode must reject it as MissingKey(13),
        // proving the flag is mandatory on the wire.
        let mut buffer = Vec::new();
        let mut e = Encoder::new(&mut buffer);
        e.map(9).unwrap();
        e.u8(0).unwrap().str(COORDINATE_ITEM_SCHEMA).unwrap();
        e.u8(1).unwrap().bytes(&[0x11; 32]).unwrap();
        e.u8(2).unwrap().u8(CoordinateKind::Task as u8).unwrap();
        e.u8(3).unwrap().str("Title").unwrap();
        e.u8(4).unwrap().str("Body").unwrap();
        e.u8(5).unwrap().str("en").unwrap();
        e.u8(6).unwrap().array(0).unwrap();
        e.u8(11).unwrap().str("").unwrap();
        e.u8(12).unwrap().array(0).unwrap();
        assert_eq!(
            decode_coordinate_item(&buffer),
            Err(CoordinateModelError::MissingKey(13))
        );
    }

    #[test]
    fn wrong_schema_is_rejected() {
        let mut buffer = Vec::new();
        let mut e = Encoder::new(&mut buffer);
        e.map(1).unwrap();
        e.u8(0).unwrap().str("org.riot.newswire.post/1").unwrap();
        assert_eq!(
            decode_coordinate_item(&buffer),
            Err(CoordinateModelError::WrongSchema)
        );
    }

    #[test]
    fn unknown_kind_tag_is_rejected() {
        let mut buffer = Vec::new();
        let mut e = Encoder::new(&mut buffer);
        e.map(3).unwrap();
        e.u8(0).unwrap().str(COORDINATE_ITEM_SCHEMA).unwrap();
        e.u8(1).unwrap().bytes(&[0x11; 32]).unwrap();
        e.u8(2).unwrap().u8(7).unwrap();
        assert_eq!(
            decode_coordinate_item(&buffer),
            Err(CoordinateModelError::InvalidEnum("coordinate_kind"))
        );
    }

    #[test]
    fn misordered_keys_are_rejected() {
        let mut buffer = Vec::new();
        let mut e = Encoder::new(&mut buffer);
        e.map(2).unwrap();
        e.u8(1).unwrap().bytes(&[0x11; 32]).unwrap();
        e.u8(0).unwrap().str(COORDINATE_ITEM_SCHEMA).unwrap();
        assert_eq!(
            decode_coordinate_item(&buffer),
            Err(CoordinateModelError::DuplicateOrMisorderedKey(0))
        );
    }

    #[test]
    fn trailing_bytes_are_rejected() {
        let value = ask();
        let mut bytes = encode_coordinate_item(&value).unwrap();
        bytes.push(0x00);
        assert_eq!(
            decode_coordinate_item(&bytes),
            Err(CoordinateModelError::TrailingBytes)
        );
    }

    #[test]
    fn empty_title_is_rejected() {
        let mut value = task();
        value.title = String::new();
        assert_eq!(
            encode_coordinate_item(&value),
            Err(CoordinateModelError::FieldEmpty("title"))
        );
    }

    #[test]
    fn oversized_title_is_rejected() {
        let mut value = task();
        value.title = "x".repeat(MAX_TITLE_BYTES + 1);
        assert_eq!(
            encode_coordinate_item(&value),
            Err(CoordinateModelError::FieldTooLarge("title"))
        );
    }

    #[test]
    fn short_language_is_rejected() {
        let mut value = task();
        value.language = "e".into();
        assert_eq!(
            encode_coordinate_item(&value),
            Err(CoordinateModelError::FieldTooSmall("language"))
        );
    }

    #[test]
    fn too_many_category_tags_are_rejected() {
        let mut value = task();
        value.category_tags = (0..MAX_CATEGORY_TAGS + 1)
            .map(|i| format!("t{i}"))
            .collect();
        assert_eq!(
            encode_coordinate_item(&value),
            Err(CoordinateModelError::TooManyEntries("category_tags"))
        );
    }
}
