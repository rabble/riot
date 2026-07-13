//! Frozen Newswire v1 data model and strict canonical CBOR codecs.

use std::collections::BTreeSet;

use minicbor::data::Type;
use minicbor::{Decoder, Encoder};

use crate::model::{Certainty, Severity, Urgency};

pub const SPACE_SCHEMA: &str = "org.riot.newswire.space/1";
pub const POST_SCHEMA: &str = "org.riot.newswire.post/1";
pub const ACTION_SCHEMA: &str = "org.riot.newswire.editorial-action/1";
pub const MAX_NEWSWIRE_PAYLOAD_BYTES: usize = 131_072;

const MAX_SPACE_NAME_BYTES: usize = 256;
const MAX_SPACE_SUMMARY_BYTES: usize = 4_096;
const MAX_LANGUAGES: usize = 16;
const MIN_LANGUAGE_BYTES: usize = 2;
const MAX_LANGUAGE_BYTES: usize = 35;
const MAX_GEOGRAPHIC_TAGS: usize = 32;
const MAX_TOPIC_TAGS: usize = 32;
const MAX_TAG_BYTES: usize = 128;
const MAX_EDITORIAL_ROSTER: usize = 64;
const MAX_HEADLINE_BYTES: usize = 512;
const MAX_BODY_OR_CORRECTION_BYTES: usize = 65_536;
const MAX_COARSE_LOCATION_BYTES: usize = 2_048;
const MAX_SOURCE_CLAIMS: usize = 16;
const MAX_SOURCE_CLAIM_BYTES: usize = 1_024;
const MAX_CONTACT_INSTRUCTIONS_BYTES: usize = 2_048;
const MAX_EDITORIAL_REASON_BYTES: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceDescriptorV1 {
    pub namespace_id: [u8; 32],
    pub name: String,
    pub summary: String,
    pub languages: Vec<String>,
    pub geographic_tags: Vec<String>,
    pub topic_tags: Vec<String>,
    pub editorial_roster: Vec<[u8; 32]>,
    pub predecessor: Option<[u8; 32]>,
    pub successor: Option<[u8; 32]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertProfileV1 {
    pub urgency: Urgency,
    pub severity: Severity,
    pub certainty: Certainty,
    pub valid_from_unix_seconds: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestKind {
    Need = 0,
    Offer = 1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestProfileV1 {
    pub kind: RequestKind,
    pub needed_by_unix_seconds: Option<u64>,
    pub contact_instructions: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationalProfileV1 {
    Alert(AlertProfileV1),
    Request(RequestProfileV1),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewsPostV1 {
    pub space_descriptor_entry_id: [u8; 32],
    pub headline: String,
    pub body: String,
    pub language: String,
    pub event_time_unix_seconds: Option<u64>,
    pub expires_at_unix_seconds: Option<u64>,
    pub coarse_location: Option<String>,
    pub source_claims: Vec<String>,
    pub operational_profile: Option<OperationalProfileV1>,
    pub ai_assisted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorialActionKind {
    Feature = 0,
    Verify = 1,
    Correct = 2,
    Hide = 3,
    Tombstone = 4,
    Retract = 5,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorialActionV1 {
    pub space_descriptor_entry_id: [u8; 32],
    pub target_entry_id: [u8; 32],
    pub kind: EditorialActionKind,
    pub reason: Option<String>,
    pub correction_text: Option<String>,
}

/// Stable, closed failure vocabulary for both semantic validation and parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewswireModelError {
    InputTooLarge,
    FieldEmpty(&'static str),
    FieldTooSmall(&'static str),
    FieldTooLarge(&'static str),
    TooManyEntries(&'static str),
    DuplicateEditorialRosterKey,
    EditorialReasonRequired,
    EditorialReasonForbidden,
    CorrectionTextRequired,
    CorrectionTextForbidden,
    AlertExpiryRequired,
    AlertLocationRequired,
    AlertSourceClaimRequired,
    RequestExpiryRequired,
    RequestLocationRequired,
    UnknownKey(u64),
    DuplicateOrMisorderedKey(u64),
    MissingKey(u64),
    WrongSchema,
    InvalidEnum(&'static str),
    NonCanonical,
    TrailingBytes,
    Malformed,
}

impl std::fmt::Display for NewswireModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for NewswireModelError {}

pub fn encode_space_descriptor(
    descriptor: &SpaceDescriptorV1,
) -> Result<Vec<u8>, NewswireModelError> {
    validate_space(descriptor)?;
    let mut pairs = 8u64;
    pairs += u64::from(descriptor.predecessor.is_some());
    pairs += u64::from(descriptor.successor.is_some());

    encode_bounded(|e| {
        e.map(pairs)?;
        e.u8(0)?.str(SPACE_SCHEMA)?;
        e.u8(1)?.bytes(&descriptor.namespace_id)?;
        e.u8(2)?.str(&descriptor.name)?;
        e.u8(3)?.str(&descriptor.summary)?;
        encode_text_array(e, 4, &descriptor.languages)?;
        encode_text_array(e, 5, &descriptor.geographic_tags)?;
        encode_text_array(e, 6, &descriptor.topic_tags)?;
        e.u8(7)?.array(descriptor.editorial_roster.len() as u64)?;
        for editor in &descriptor.editorial_roster {
            e.bytes(editor)?;
        }
        if let Some(predecessor) = descriptor.predecessor {
            e.u8(8)?.bytes(&predecessor)?;
        }
        if let Some(successor) = descriptor.successor {
            e.u8(9)?.bytes(&successor)?;
        }
        Ok(())
    })
}

pub fn decode_space_descriptor(input: &[u8]) -> Result<SpaceDescriptorV1, NewswireModelError> {
    check_input_size(input)?;
    let mut d = Decoder::new(input);
    let pairs = definite_map(&mut d)?;
    if pairs > 10 {
        return Err(NewswireModelError::Malformed);
    }

    let mut schema = None;
    let mut namespace_id = None;
    let mut name = None;
    let mut summary = None;
    let mut languages = None;
    let mut geographic_tags = None;
    let mut topic_tags = None;
    let mut editorial_roster = None;
    let mut predecessor = None;
    let mut successor = None;
    let mut last_key = None;

    for _ in 0..pairs {
        let key = decode_ordered_key(&mut d, &mut last_key)?;
        match key {
            0 => schema = Some(decode_text(&mut d, "schema", 1, 64)?),
            1 => namespace_id = Some(decode_id32(&mut d)?),
            2 => name = Some(decode_text(&mut d, "name", 1, MAX_SPACE_NAME_BYTES)?),
            3 => summary = Some(decode_text(&mut d, "summary", 1, MAX_SPACE_SUMMARY_BYTES)?),
            4 => {
                languages = Some(decode_text_array(
                    &mut d,
                    "languages",
                    MAX_LANGUAGES,
                    "language",
                    MIN_LANGUAGE_BYTES,
                    MAX_LANGUAGE_BYTES,
                )?)
            }
            5 => {
                geographic_tags = Some(decode_text_array(
                    &mut d,
                    "geographic_tags",
                    MAX_GEOGRAPHIC_TAGS,
                    "geographic_tag",
                    1,
                    MAX_TAG_BYTES,
                )?)
            }
            6 => {
                topic_tags = Some(decode_text_array(
                    &mut d,
                    "topic_tags",
                    MAX_TOPIC_TAGS,
                    "topic_tag",
                    1,
                    MAX_TAG_BYTES,
                )?)
            }
            7 => {
                let len = definite_array(&mut d)?;
                if len as usize > MAX_EDITORIAL_ROSTER {
                    return Err(NewswireModelError::TooManyEntries("editorial_roster"));
                }
                let mut roster = Vec::with_capacity(len as usize);
                for _ in 0..len {
                    roster.push(decode_id32(&mut d)?);
                }
                editorial_roster = Some(roster);
            }
            8 => predecessor = Some(decode_id32(&mut d)?),
            9 => successor = Some(decode_id32(&mut d)?),
            other => return Err(NewswireModelError::UnknownKey(other)),
        }
    }
    finish_input(&d, input)?;
    if schema.as_deref() != Some(SPACE_SCHEMA) {
        return Err(NewswireModelError::WrongSchema);
    }

    let descriptor = SpaceDescriptorV1 {
        namespace_id: namespace_id.ok_or(NewswireModelError::MissingKey(1))?,
        name: name.ok_or(NewswireModelError::MissingKey(2))?,
        summary: summary.ok_or(NewswireModelError::MissingKey(3))?,
        languages: languages.ok_or(NewswireModelError::MissingKey(4))?,
        geographic_tags: geographic_tags.ok_or(NewswireModelError::MissingKey(5))?,
        topic_tags: topic_tags.ok_or(NewswireModelError::MissingKey(6))?,
        editorial_roster: editorial_roster.ok_or(NewswireModelError::MissingKey(7))?,
        predecessor,
        successor,
    };
    validate_space(&descriptor)?;
    prove_canonical(input, encode_space_descriptor(&descriptor)?)?;
    Ok(descriptor)
}

pub fn encode_news_post(post: &NewsPostV1) -> Result<Vec<u8>, NewswireModelError> {
    validate_post(post)?;
    let mut pairs = 7u64;
    pairs += u64::from(post.event_time_unix_seconds.is_some());
    pairs += u64::from(post.expires_at_unix_seconds.is_some());
    pairs += u64::from(post.coarse_location.is_some());
    pairs += u64::from(post.operational_profile.is_some());

    encode_bounded(|e| {
        e.map(pairs)?;
        e.u8(0)?.str(POST_SCHEMA)?;
        e.u8(1)?.bytes(&post.space_descriptor_entry_id)?;
        e.u8(2)?.str(&post.headline)?;
        e.u8(3)?.str(&post.body)?;
        e.u8(4)?.str(&post.language)?;
        if let Some(event_time) = post.event_time_unix_seconds {
            e.u8(5)?.u64(event_time)?;
        }
        if let Some(expires_at) = post.expires_at_unix_seconds {
            e.u8(6)?.u64(expires_at)?;
        }
        if let Some(location) = &post.coarse_location {
            e.u8(7)?.str(location)?;
        }
        encode_text_array(e, 8, &post.source_claims)?;
        if let Some(profile) = &post.operational_profile {
            e.u8(9)?;
            encode_operational_profile(e, profile)?;
        }
        e.u8(10)?.bool(post.ai_assisted)?;
        Ok(())
    })
}

pub fn decode_news_post(input: &[u8]) -> Result<NewsPostV1, NewswireModelError> {
    check_input_size(input)?;
    let mut d = Decoder::new(input);
    let pairs = definite_map(&mut d)?;
    if pairs > 11 {
        return Err(NewswireModelError::Malformed);
    }

    let mut schema = None;
    let mut space_descriptor_entry_id = None;
    let mut headline = None;
    let mut body = None;
    let mut language = None;
    let mut event_time_unix_seconds = None;
    let mut expires_at_unix_seconds = None;
    let mut coarse_location = None;
    let mut source_claims = None;
    let mut operational_profile = None;
    let mut ai_assisted = None;
    let mut last_key = None;

    for _ in 0..pairs {
        let key = decode_ordered_key(&mut d, &mut last_key)?;
        match key {
            0 => schema = Some(decode_text(&mut d, "schema", 1, 64)?),
            1 => space_descriptor_entry_id = Some(decode_id32(&mut d)?),
            2 => headline = Some(decode_text(&mut d, "headline", 1, MAX_HEADLINE_BYTES)?),
            3 => {
                body = Some(decode_text(
                    &mut d,
                    "body",
                    1,
                    MAX_BODY_OR_CORRECTION_BYTES,
                )?)
            }
            4 => {
                language = Some(decode_text(
                    &mut d,
                    "language",
                    MIN_LANGUAGE_BYTES,
                    MAX_LANGUAGE_BYTES,
                )?)
            }
            5 => event_time_unix_seconds = Some(decode_u64(&mut d)?),
            6 => expires_at_unix_seconds = Some(decode_u64(&mut d)?),
            7 => {
                coarse_location = Some(decode_text(
                    &mut d,
                    "coarse_location",
                    1,
                    MAX_COARSE_LOCATION_BYTES,
                )?)
            }
            8 => {
                source_claims = Some(decode_text_array(
                    &mut d,
                    "source_claims",
                    MAX_SOURCE_CLAIMS,
                    "source_claim",
                    1,
                    MAX_SOURCE_CLAIM_BYTES,
                )?)
            }
            9 => operational_profile = Some(decode_operational_profile(&mut d)?),
            10 => ai_assisted = Some(d.bool().map_err(|_| NewswireModelError::Malformed)?),
            other => return Err(NewswireModelError::UnknownKey(other)),
        }
    }
    finish_input(&d, input)?;
    if schema.as_deref() != Some(POST_SCHEMA) {
        return Err(NewswireModelError::WrongSchema);
    }

    let post = NewsPostV1 {
        space_descriptor_entry_id: space_descriptor_entry_id
            .ok_or(NewswireModelError::MissingKey(1))?,
        headline: headline.ok_or(NewswireModelError::MissingKey(2))?,
        body: body.ok_or(NewswireModelError::MissingKey(3))?,
        language: language.ok_or(NewswireModelError::MissingKey(4))?,
        event_time_unix_seconds,
        expires_at_unix_seconds,
        coarse_location,
        source_claims: source_claims.ok_or(NewswireModelError::MissingKey(8))?,
        operational_profile,
        ai_assisted: ai_assisted.ok_or(NewswireModelError::MissingKey(10))?,
    };
    validate_post(&post)?;
    prove_canonical(input, encode_news_post(&post)?)?;
    Ok(post)
}

pub fn encode_editorial_action(action: &EditorialActionV1) -> Result<Vec<u8>, NewswireModelError> {
    validate_action(action)?;
    let mut pairs = 4u64;
    pairs += u64::from(action.reason.is_some());
    pairs += u64::from(action.correction_text.is_some());

    encode_bounded(|e| {
        e.map(pairs)?;
        e.u8(0)?.str(ACTION_SCHEMA)?;
        e.u8(1)?.bytes(&action.space_descriptor_entry_id)?;
        e.u8(2)?.bytes(&action.target_entry_id)?;
        e.u8(3)?.u8(action.kind as u8)?;
        if let Some(reason) = &action.reason {
            e.u8(4)?.str(reason)?;
        }
        if let Some(correction) = &action.correction_text {
            e.u8(5)?.str(correction)?;
        }
        Ok(())
    })
}

pub fn decode_editorial_action(input: &[u8]) -> Result<EditorialActionV1, NewswireModelError> {
    check_input_size(input)?;
    let mut d = Decoder::new(input);
    let pairs = definite_map(&mut d)?;
    if pairs > 6 {
        return Err(NewswireModelError::Malformed);
    }

    let mut schema = None;
    let mut space_descriptor_entry_id = None;
    let mut target_entry_id = None;
    let mut kind = None;
    let mut reason = None;
    let mut correction_text = None;
    let mut last_key = None;

    for _ in 0..pairs {
        let key = decode_ordered_key(&mut d, &mut last_key)?;
        match key {
            0 => schema = Some(decode_text(&mut d, "schema", 1, 64)?),
            1 => space_descriptor_entry_id = Some(decode_id32(&mut d)?),
            2 => target_entry_id = Some(decode_id32(&mut d)?),
            3 => {
                let raw = d.u8().map_err(|_| NewswireModelError::Malformed)?;
                kind = Some(editorial_action_kind_from_u8(raw)?);
            }
            4 => {
                reason = Some(decode_text(
                    &mut d,
                    "reason",
                    1,
                    MAX_EDITORIAL_REASON_BYTES,
                )?)
            }
            5 => {
                correction_text = Some(decode_text(
                    &mut d,
                    "correction_text",
                    1,
                    MAX_BODY_OR_CORRECTION_BYTES,
                )?)
            }
            other => return Err(NewswireModelError::UnknownKey(other)),
        }
    }
    finish_input(&d, input)?;
    if schema.as_deref() != Some(ACTION_SCHEMA) {
        return Err(NewswireModelError::WrongSchema);
    }

    let action = EditorialActionV1 {
        space_descriptor_entry_id: space_descriptor_entry_id
            .ok_or(NewswireModelError::MissingKey(1))?,
        target_entry_id: target_entry_id.ok_or(NewswireModelError::MissingKey(2))?,
        kind: kind.ok_or(NewswireModelError::MissingKey(3))?,
        reason,
        correction_text,
    };
    validate_action(&action)?;
    prove_canonical(input, encode_editorial_action(&action)?)?;
    Ok(action)
}

fn validate_space(descriptor: &SpaceDescriptorV1) -> Result<(), NewswireModelError> {
    check_text("name", &descriptor.name, 1, MAX_SPACE_NAME_BYTES)?;
    check_text("summary", &descriptor.summary, 1, MAX_SPACE_SUMMARY_BYTES)?;
    check_text_list(
        "languages",
        &descriptor.languages,
        MAX_LANGUAGES,
        "language",
        MIN_LANGUAGE_BYTES,
        MAX_LANGUAGE_BYTES,
    )?;
    check_text_list(
        "geographic_tags",
        &descriptor.geographic_tags,
        MAX_GEOGRAPHIC_TAGS,
        "geographic_tag",
        1,
        MAX_TAG_BYTES,
    )?;
    check_text_list(
        "topic_tags",
        &descriptor.topic_tags,
        MAX_TOPIC_TAGS,
        "topic_tag",
        1,
        MAX_TAG_BYTES,
    )?;
    if descriptor.editorial_roster.len() > MAX_EDITORIAL_ROSTER {
        return Err(NewswireModelError::TooManyEntries("editorial_roster"));
    }
    let unique: BTreeSet<_> = descriptor.editorial_roster.iter().collect();
    if unique.len() != descriptor.editorial_roster.len() {
        return Err(NewswireModelError::DuplicateEditorialRosterKey);
    }
    Ok(())
}

fn validate_post(post: &NewsPostV1) -> Result<(), NewswireModelError> {
    check_text("headline", &post.headline, 1, MAX_HEADLINE_BYTES)?;
    check_text("body", &post.body, 1, MAX_BODY_OR_CORRECTION_BYTES)?;
    check_text(
        "language",
        &post.language,
        MIN_LANGUAGE_BYTES,
        MAX_LANGUAGE_BYTES,
    )?;
    if let Some(location) = &post.coarse_location {
        check_text("coarse_location", location, 1, MAX_COARSE_LOCATION_BYTES)?;
    }
    check_text_list(
        "source_claims",
        &post.source_claims,
        MAX_SOURCE_CLAIMS,
        "source_claim",
        1,
        MAX_SOURCE_CLAIM_BYTES,
    )?;

    match &post.operational_profile {
        Some(OperationalProfileV1::Alert(_)) => {
            if post.expires_at_unix_seconds.is_none() {
                return Err(NewswireModelError::AlertExpiryRequired);
            }
            if post.coarse_location.is_none() {
                return Err(NewswireModelError::AlertLocationRequired);
            }
            if post.source_claims.is_empty() {
                return Err(NewswireModelError::AlertSourceClaimRequired);
            }
        }
        Some(OperationalProfileV1::Request(profile)) => {
            check_text(
                "contact_instructions",
                &profile.contact_instructions,
                1,
                MAX_CONTACT_INSTRUCTIONS_BYTES,
            )?;
            if post.expires_at_unix_seconds.is_none() {
                return Err(NewswireModelError::RequestExpiryRequired);
            }
            if post.coarse_location.is_none() {
                return Err(NewswireModelError::RequestLocationRequired);
            }
        }
        None => {}
    }
    Ok(())
}

fn validate_action(action: &EditorialActionV1) -> Result<(), NewswireModelError> {
    if let Some(reason) = &action.reason {
        check_text("reason", reason, 1, MAX_EDITORIAL_REASON_BYTES)?;
    }
    if let Some(correction) = &action.correction_text {
        check_text(
            "correction_text",
            correction,
            1,
            MAX_BODY_OR_CORRECTION_BYTES,
        )?;
    }
    if matches!(
        action.kind,
        EditorialActionKind::Correct
            | EditorialActionKind::Hide
            | EditorialActionKind::Tombstone
            | EditorialActionKind::Retract
    ) && action.reason.is_none()
    {
        return Err(NewswireModelError::EditorialReasonRequired);
    }
    match (action.kind, action.correction_text.is_some()) {
        (EditorialActionKind::Correct, false) => Err(NewswireModelError::CorrectionTextRequired),
        (EditorialActionKind::Correct, true) => Ok(()),
        (_, true) => Err(NewswireModelError::CorrectionTextForbidden),
        (EditorialActionKind::Feature | EditorialActionKind::Verify, false)
            if action.reason.is_some() =>
        {
            Err(NewswireModelError::EditorialReasonForbidden)
        }
        (_, false) => Ok(()),
    }
}

fn check_text(
    field: &'static str,
    value: &str,
    min: usize,
    max: usize,
) -> Result<(), NewswireModelError> {
    if value.trim().is_empty() {
        return Err(NewswireModelError::FieldEmpty(field));
    }
    if value.len() < min {
        return Err(NewswireModelError::FieldTooSmall(field));
    }
    if value.len() > max {
        return Err(NewswireModelError::FieldTooLarge(field));
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
) -> Result<(), NewswireModelError> {
    if values.len() > max_entries {
        return Err(NewswireModelError::TooManyEntries(list_field));
    }
    for value in values {
        check_text(item_field, value, min_bytes, max_bytes)?;
    }
    Ok(())
}

fn encode_bounded<F>(encode: F) -> Result<Vec<u8>, NewswireModelError>
where
    F: FnOnce(
        &mut Encoder<&mut Vec<u8>>,
    ) -> Result<(), minicbor::encode::Error<core::convert::Infallible>>,
{
    let mut buffer = Vec::new();
    let mut encoder = Encoder::new(&mut buffer);
    encode(&mut encoder).map_err(|_| NewswireModelError::Malformed)?;
    if buffer.len() > MAX_NEWSWIRE_PAYLOAD_BYTES {
        return Err(NewswireModelError::InputTooLarge);
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

fn encode_operational_profile(
    e: &mut Encoder<&mut Vec<u8>>,
    profile: &OperationalProfileV1,
) -> Result<(), minicbor::encode::Error<core::convert::Infallible>> {
    e.map(2)?;
    match profile {
        OperationalProfileV1::Alert(alert) => {
            e.u8(0)?.u8(0)?;
            e.u8(1)?
                .map(3 + u64::from(alert.valid_from_unix_seconds.is_some()))?;
            e.u8(0)?.u8(alert.urgency as u8)?;
            e.u8(1)?.u8(alert.severity as u8)?;
            e.u8(2)?.u8(alert.certainty as u8)?;
            if let Some(valid_from) = alert.valid_from_unix_seconds {
                e.u8(3)?.u64(valid_from)?;
            }
        }
        OperationalProfileV1::Request(request) => {
            e.u8(0)?.u8(1)?;
            e.u8(1)?
                .map(2 + u64::from(request.needed_by_unix_seconds.is_some()))?;
            e.u8(0)?.u8(request.kind as u8)?;
            if let Some(needed_by) = request.needed_by_unix_seconds {
                e.u8(1)?.u64(needed_by)?;
            }
            e.u8(2)?.str(&request.contact_instructions)?;
        }
    }
    Ok(())
}

fn decode_operational_profile(
    d: &mut Decoder<'_>,
) -> Result<OperationalProfileV1, NewswireModelError> {
    if definite_map(d)? != 2 {
        return Err(NewswireModelError::Malformed);
    }
    let mut last_key = None;
    if decode_ordered_key(d, &mut last_key)? != 0 {
        return Err(NewswireModelError::MissingKey(0));
    }
    let tag = d.u8().map_err(|_| NewswireModelError::Malformed)?;
    if decode_ordered_key(d, &mut last_key)? != 1 {
        return Err(NewswireModelError::MissingKey(1));
    }
    match tag {
        0 => decode_alert_profile(d).map(OperationalProfileV1::Alert),
        1 => decode_request_profile(d).map(OperationalProfileV1::Request),
        _ => Err(NewswireModelError::InvalidEnum("operational_profile")),
    }
}

fn decode_alert_profile(d: &mut Decoder<'_>) -> Result<AlertProfileV1, NewswireModelError> {
    let pairs = definite_map(d)?;
    if !(3..=4).contains(&pairs) {
        return Err(NewswireModelError::Malformed);
    }
    let mut urgency = None;
    let mut severity = None;
    let mut certainty = None;
    let mut valid_from_unix_seconds = None;
    let mut last_key = None;
    for _ in 0..pairs {
        match decode_ordered_key(d, &mut last_key)? {
            0 => {
                urgency = Some(urgency_from_u8(
                    d.u8().map_err(|_| NewswireModelError::Malformed)?,
                )?)
            }
            1 => {
                severity = Some(severity_from_u8(
                    d.u8().map_err(|_| NewswireModelError::Malformed)?,
                )?)
            }
            2 => {
                certainty = Some(certainty_from_u8(
                    d.u8().map_err(|_| NewswireModelError::Malformed)?,
                )?)
            }
            3 => valid_from_unix_seconds = Some(decode_u64(d)?),
            other => return Err(NewswireModelError::UnknownKey(other)),
        }
    }
    Ok(AlertProfileV1 {
        urgency: urgency.ok_or(NewswireModelError::MissingKey(0))?,
        severity: severity.ok_or(NewswireModelError::MissingKey(1))?,
        certainty: certainty.ok_or(NewswireModelError::MissingKey(2))?,
        valid_from_unix_seconds,
    })
}

fn decode_request_profile(d: &mut Decoder<'_>) -> Result<RequestProfileV1, NewswireModelError> {
    let pairs = definite_map(d)?;
    if !(2..=3).contains(&pairs) {
        return Err(NewswireModelError::Malformed);
    }
    let mut kind = None;
    let mut needed_by_unix_seconds = None;
    let mut contact_instructions = None;
    let mut last_key = None;
    for _ in 0..pairs {
        match decode_ordered_key(d, &mut last_key)? {
            0 => {
                let raw = d.u8().map_err(|_| NewswireModelError::Malformed)?;
                kind = Some(match raw {
                    0 => RequestKind::Need,
                    1 => RequestKind::Offer,
                    _ => return Err(NewswireModelError::InvalidEnum("request_kind")),
                });
            }
            1 => needed_by_unix_seconds = Some(decode_u64(d)?),
            2 => {
                contact_instructions = Some(decode_text(
                    d,
                    "contact_instructions",
                    1,
                    MAX_CONTACT_INSTRUCTIONS_BYTES,
                )?)
            }
            other => return Err(NewswireModelError::UnknownKey(other)),
        }
    }
    Ok(RequestProfileV1 {
        kind: kind.ok_or(NewswireModelError::MissingKey(0))?,
        needed_by_unix_seconds,
        contact_instructions: contact_instructions.ok_or(NewswireModelError::MissingKey(2))?,
    })
}

fn check_input_size(input: &[u8]) -> Result<(), NewswireModelError> {
    if input.len() > MAX_NEWSWIRE_PAYLOAD_BYTES {
        Err(NewswireModelError::InputTooLarge)
    } else {
        Ok(())
    }
}

fn definite_map(d: &mut Decoder<'_>) -> Result<u64, NewswireModelError> {
    d.map()
        .map_err(|_| NewswireModelError::Malformed)?
        .ok_or(NewswireModelError::NonCanonical)
}

fn definite_array(d: &mut Decoder<'_>) -> Result<u64, NewswireModelError> {
    d.array()
        .map_err(|_| NewswireModelError::Malformed)?
        .ok_or(NewswireModelError::NonCanonical)
}

fn decode_ordered_key(
    d: &mut Decoder<'_>,
    last_key: &mut Option<u64>,
) -> Result<u64, NewswireModelError> {
    let key = d.u64().map_err(|_| NewswireModelError::Malformed)?;
    if last_key.is_some_and(|previous| key <= previous) {
        return Err(NewswireModelError::DuplicateOrMisorderedKey(key));
    }
    *last_key = Some(key);
    Ok(key)
}

fn decode_id32(d: &mut Decoder<'_>) -> Result<[u8; 32], NewswireModelError> {
    let bytes = d.bytes().map_err(|_| NewswireModelError::Malformed)?;
    <[u8; 32]>::try_from(bytes).map_err(|_| NewswireModelError::Malformed)
}

fn decode_u64(d: &mut Decoder<'_>) -> Result<u64, NewswireModelError> {
    d.u64().map_err(|_| NewswireModelError::Malformed)
}

fn decode_text(
    d: &mut Decoder<'_>,
    field: &'static str,
    min: usize,
    max: usize,
) -> Result<String, NewswireModelError> {
    if d.datatype().map_err(|_| NewswireModelError::Malformed)? != Type::String {
        return Err(NewswireModelError::Malformed);
    }
    let value = d.str().map_err(|_| NewswireModelError::Malformed)?;
    check_text(field, value, min, max)?;
    Ok(value.to_string())
}

fn decode_text_array(
    d: &mut Decoder<'_>,
    list_field: &'static str,
    max_entries: usize,
    item_field: &'static str,
    min_bytes: usize,
    max_bytes: usize,
) -> Result<Vec<String>, NewswireModelError> {
    let len = definite_array(d)?;
    if len as usize > max_entries {
        return Err(NewswireModelError::TooManyEntries(list_field));
    }
    let mut values = Vec::with_capacity(len as usize);
    for _ in 0..len {
        values.push(decode_text(d, item_field, min_bytes, max_bytes)?);
    }
    Ok(values)
}

fn finish_input(d: &Decoder<'_>, input: &[u8]) -> Result<(), NewswireModelError> {
    if d.position() == input.len() {
        Ok(())
    } else {
        Err(NewswireModelError::TrailingBytes)
    }
}

fn prove_canonical(input: &[u8], encoded: Vec<u8>) -> Result<(), NewswireModelError> {
    if encoded == input {
        Ok(())
    } else {
        Err(NewswireModelError::NonCanonical)
    }
}

fn urgency_from_u8(value: u8) -> Result<Urgency, NewswireModelError> {
    match value {
        0 => Ok(Urgency::Immediate),
        1 => Ok(Urgency::Expected),
        2 => Ok(Urgency::Future),
        3 => Ok(Urgency::Past),
        4 => Ok(Urgency::Unknown),
        _ => Err(NewswireModelError::InvalidEnum("urgency")),
    }
}

fn severity_from_u8(value: u8) -> Result<Severity, NewswireModelError> {
    match value {
        0 => Ok(Severity::Extreme),
        1 => Ok(Severity::Severe),
        2 => Ok(Severity::Moderate),
        3 => Ok(Severity::Minor),
        4 => Ok(Severity::Unknown),
        _ => Err(NewswireModelError::InvalidEnum("severity")),
    }
}

fn certainty_from_u8(value: u8) -> Result<Certainty, NewswireModelError> {
    match value {
        0 => Ok(Certainty::Observed),
        1 => Ok(Certainty::Likely),
        2 => Ok(Certainty::Possible),
        3 => Ok(Certainty::Unlikely),
        4 => Ok(Certainty::Unknown),
        _ => Err(NewswireModelError::InvalidEnum("certainty")),
    }
}

fn editorial_action_kind_from_u8(value: u8) -> Result<EditorialActionKind, NewswireModelError> {
    match value {
        0 => Ok(EditorialActionKind::Feature),
        1 => Ok(EditorialActionKind::Verify),
        2 => Ok(EditorialActionKind::Correct),
        3 => Ok(EditorialActionKind::Hide),
        4 => Ok(EditorialActionKind::Tombstone),
        5 => Ok(EditorialActionKind::Retract),
        _ => Err(NewswireModelError::InvalidEnum("editorial_action_kind")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn space() -> SpaceDescriptorV1 {
        SpaceDescriptorV1 {
            namespace_id: [0x10; 32],
            name: "Riverside Independent Media".into(),
            summary: "Open publishing by and for the community.".into(),
            languages: vec!["en".into()],
            geographic_tags: vec!["riverside".into()],
            topic_tags: vec!["community-media".into()],
            editorial_roster: vec![[0x20; 32]],
            predecessor: None,
            successor: None,
        }
    }

    fn post() -> NewsPostV1 {
        NewsPostV1 {
            space_descriptor_entry_id: [0x11; 32],
            headline: "Night march reaches the square".into(),
            body: "Witness report from the community assembly.".into(),
            language: "en".into(),
            event_time_unix_seconds: Some(1_800_000_100),
            expires_at_unix_seconds: None,
            coarse_location: Some("central district".into()),
            source_claims: vec!["participant account".into()],
            operational_profile: None,
            ai_assisted: false,
        }
    }

    fn action() -> EditorialActionV1 {
        EditorialActionV1 {
            space_descriptor_entry_id: [0x11; 32],
            target_entry_id: [0x22; 32],
            kind: EditorialActionKind::Correct,
            reason: Some("Clarifies the assembly's decision.".into()),
            correction_text: Some("The assembly reconvenes Friday.".into()),
        }
    }

    #[test]
    fn frozen_boundary_table_is_enforced_on_both_sides() {
        let mut valid_spaces = Vec::new();
        let mut value = space();
        value.name = "n".into();
        valid_spaces.push(value);
        let mut value = space();
        value.name = "n".repeat(MAX_SPACE_NAME_BYTES);
        valid_spaces.push(value);
        let mut value = space();
        value.summary = "s".into();
        valid_spaces.push(value);
        let mut value = space();
        value.summary = "s".repeat(MAX_SPACE_SUMMARY_BYTES);
        valid_spaces.push(value);
        let mut value = space();
        value.languages = vec!["aa".into(); MAX_LANGUAGES];
        value.languages[0] = "l".repeat(MAX_LANGUAGE_BYTES);
        valid_spaces.push(value);
        let mut value = space();
        value.geographic_tags = vec!["g".into(); MAX_GEOGRAPHIC_TAGS];
        value.geographic_tags[0] = "g".repeat(MAX_TAG_BYTES);
        valid_spaces.push(value);
        let mut value = space();
        value.topic_tags = vec!["t".into(); MAX_TOPIC_TAGS];
        value.topic_tags[0] = "t".repeat(MAX_TAG_BYTES);
        valid_spaces.push(value);
        let mut value = space();
        value.editorial_roster = (0..MAX_EDITORIAL_ROSTER)
            .map(|index| {
                let mut key = [0u8; 32];
                key[0] = index as u8;
                key
            })
            .collect();
        valid_spaces.push(value);
        for value in valid_spaces {
            assert!(encode_space_descriptor(&value).is_ok());
        }

        let mut invalid_spaces = Vec::new();
        let mut value = space();
        value.name = "n".repeat(MAX_SPACE_NAME_BYTES + 1);
        invalid_spaces.push((value, NewswireModelError::FieldTooLarge("name")));
        let mut value = space();
        value.summary = "s".repeat(MAX_SPACE_SUMMARY_BYTES + 1);
        invalid_spaces.push((value, NewswireModelError::FieldTooLarge("summary")));
        let mut value = space();
        value.languages = vec!["en".into(); MAX_LANGUAGES + 1];
        invalid_spaces.push((value, NewswireModelError::TooManyEntries("languages")));
        let mut value = space();
        value.languages = vec!["e".into()];
        invalid_spaces.push((value, NewswireModelError::FieldTooSmall("language")));
        let mut value = space();
        value.languages = vec!["l".repeat(MAX_LANGUAGE_BYTES + 1)];
        invalid_spaces.push((value, NewswireModelError::FieldTooLarge("language")));
        let mut value = space();
        value.geographic_tags = vec!["g".into(); MAX_GEOGRAPHIC_TAGS + 1];
        invalid_spaces.push((value, NewswireModelError::TooManyEntries("geographic_tags")));
        let mut value = space();
        value.geographic_tags = vec!["g".repeat(MAX_TAG_BYTES + 1)];
        invalid_spaces.push((value, NewswireModelError::FieldTooLarge("geographic_tag")));
        let mut value = space();
        value.topic_tags = vec!["t".into(); MAX_TOPIC_TAGS + 1];
        invalid_spaces.push((value, NewswireModelError::TooManyEntries("topic_tags")));
        let mut value = space();
        value.topic_tags = vec!["t".repeat(MAX_TAG_BYTES + 1)];
        invalid_spaces.push((value, NewswireModelError::FieldTooLarge("topic_tag")));
        let mut value = space();
        value.editorial_roster = vec![[0x20; 32]; MAX_EDITORIAL_ROSTER + 1];
        invalid_spaces.push((
            value,
            NewswireModelError::TooManyEntries("editorial_roster"),
        ));
        for (value, expected) in invalid_spaces {
            assert_eq!(encode_space_descriptor(&value), Err(expected));
        }

        let mut valid_posts = Vec::new();
        let mut value = post();
        value.headline = "h".into();
        valid_posts.push(value);
        let mut value = post();
        value.headline = "h".repeat(MAX_HEADLINE_BYTES);
        valid_posts.push(value);
        let mut value = post();
        value.body = "b".into();
        valid_posts.push(value);
        let mut value = post();
        value.body = "b".repeat(MAX_BODY_OR_CORRECTION_BYTES);
        valid_posts.push(value);
        let mut value = post();
        value.language = "en".into();
        valid_posts.push(value);
        let mut value = post();
        value.language = "l".repeat(MAX_LANGUAGE_BYTES);
        valid_posts.push(value);
        let mut value = post();
        value.coarse_location = Some("c".into());
        valid_posts.push(value);
        let mut value = post();
        value.coarse_location = Some("c".repeat(MAX_COARSE_LOCATION_BYTES));
        valid_posts.push(value);
        let mut value = post();
        value.source_claims = vec!["s".into(); MAX_SOURCE_CLAIMS];
        value.source_claims[0] = "s".repeat(MAX_SOURCE_CLAIM_BYTES);
        valid_posts.push(value);
        for value in valid_posts {
            assert!(encode_news_post(&value).is_ok());
        }

        let invalid_posts = [
            {
                let mut value = post();
                value.headline = "h".repeat(MAX_HEADLINE_BYTES + 1);
                (value, NewswireModelError::FieldTooLarge("headline"))
            },
            {
                let mut value = post();
                value.body = "b".repeat(MAX_BODY_OR_CORRECTION_BYTES + 1);
                (value, NewswireModelError::FieldTooLarge("body"))
            },
            {
                let mut value = post();
                value.language.clear();
                (value, NewswireModelError::FieldEmpty("language"))
            },
            {
                let mut value = post();
                value.language = "e".into();
                (value, NewswireModelError::FieldTooSmall("language"))
            },
            {
                let mut value = post();
                value.language = "l".repeat(MAX_LANGUAGE_BYTES + 1);
                (value, NewswireModelError::FieldTooLarge("language"))
            },
            {
                let mut value = post();
                value.coarse_location = Some("c".repeat(MAX_COARSE_LOCATION_BYTES + 1));
                (value, NewswireModelError::FieldTooLarge("coarse_location"))
            },
            {
                let mut value = post();
                value.source_claims = vec!["s".into(); MAX_SOURCE_CLAIMS + 1];
                (value, NewswireModelError::TooManyEntries("source_claims"))
            },
            {
                let mut value = post();
                value.source_claims = vec!["s".repeat(MAX_SOURCE_CLAIM_BYTES + 1)];
                (value, NewswireModelError::FieldTooLarge("source_claim"))
            },
        ];
        for (value, expected) in invalid_posts {
            assert_eq!(encode_news_post(&value), Err(expected));
        }

        let mut valid_action = action();
        valid_action.reason = Some("r".into());
        valid_action.correction_text = Some("c".into());
        assert!(encode_editorial_action(&valid_action).is_ok());
        let mut valid_action = action();
        valid_action.reason = Some("r".repeat(MAX_EDITORIAL_REASON_BYTES));
        valid_action.correction_text = Some("c".repeat(MAX_BODY_OR_CORRECTION_BYTES));
        assert!(encode_editorial_action(&valid_action).is_ok());
        let mut invalid_action = action();
        invalid_action.reason = Some("r".repeat(MAX_EDITORIAL_REASON_BYTES + 1));
        assert_eq!(
            encode_editorial_action(&invalid_action),
            Err(NewswireModelError::FieldTooLarge("reason"))
        );
        let mut invalid_action = action();
        invalid_action.correction_text = Some("c".repeat(MAX_BODY_OR_CORRECTION_BYTES + 1));
        assert_eq!(
            encode_editorial_action(&invalid_action),
            Err(NewswireModelError::FieldTooLarge("correction_text"))
        );

        let mut max_contact = post();
        max_contact.expires_at_unix_seconds = Some(1_800_000_900);
        max_contact.operational_profile = Some(OperationalProfileV1::Request(RequestProfileV1 {
            kind: RequestKind::Need,
            needed_by_unix_seconds: None,
            contact_instructions: "c".repeat(MAX_CONTACT_INSTRUCTIONS_BYTES),
        }));
        assert!(encode_news_post(&max_contact).is_ok());

        let mut min_contact = max_contact.clone();
        if let Some(OperationalProfileV1::Request(profile)) =
            min_contact.operational_profile.as_mut()
        {
            profile.contact_instructions = "c".into();
        }
        assert!(encode_news_post(&min_contact).is_ok());

        let mut empty_contact = max_contact.clone();
        if let Some(OperationalProfileV1::Request(profile)) =
            empty_contact.operational_profile.as_mut()
        {
            profile.contact_instructions.clear();
        }
        assert_eq!(
            encode_news_post(&empty_contact),
            Err(NewswireModelError::FieldEmpty("contact_instructions"))
        );

        let mut oversized_contact = max_contact;
        if let Some(OperationalProfileV1::Request(profile)) =
            oversized_contact.operational_profile.as_mut()
        {
            profile.contact_instructions = "c".repeat(MAX_CONTACT_INSTRUCTIONS_BYTES + 1);
        }
        assert_eq!(
            encode_news_post(&oversized_contact),
            Err(NewswireModelError::FieldTooLarge("contact_instructions"))
        );
    }

    #[test]
    fn semantic_rule_table_mirrors_the_public_contract() {
        let mut duplicate_roster = space();
        duplicate_roster.editorial_roster = vec![[0x20; 32], [0x20; 32]];
        assert_eq!(
            encode_space_descriptor(&duplicate_roster),
            Err(NewswireModelError::DuplicateEditorialRosterKey)
        );

        let blank_space_cases = [
            {
                let mut value = space();
                value.name = " \t".into();
                (value, NewswireModelError::FieldEmpty("name"))
            },
            {
                let mut value = space();
                value.summary = "\n".into();
                (value, NewswireModelError::FieldEmpty("summary"))
            },
            {
                let mut value = space();
                value.languages = vec!["  ".into()];
                (value, NewswireModelError::FieldEmpty("language"))
            },
            {
                let mut value = space();
                value.geographic_tags = vec!["  ".into()];
                (value, NewswireModelError::FieldEmpty("geographic_tag"))
            },
            {
                let mut value = space();
                value.topic_tags = vec!["  ".into()];
                (value, NewswireModelError::FieldEmpty("topic_tag"))
            },
        ];
        for (value, expected) in blank_space_cases {
            assert_eq!(encode_space_descriptor(&value), Err(expected));
        }

        let blank_post_cases = [
            {
                let mut value = post();
                value.headline = "  ".into();
                (value, NewswireModelError::FieldEmpty("headline"))
            },
            {
                let mut value = post();
                value.body = "\t".into();
                (value, NewswireModelError::FieldEmpty("body"))
            },
            {
                let mut value = post();
                value.language = "  ".into();
                (value, NewswireModelError::FieldEmpty("language"))
            },
            {
                let mut value = post();
                value.coarse_location = Some("  ".into());
                (value, NewswireModelError::FieldEmpty("coarse_location"))
            },
            {
                let mut value = post();
                value.source_claims = vec!["  ".into()];
                (value, NewswireModelError::FieldEmpty("source_claim"))
            },
        ];
        for (value, expected) in blank_post_cases {
            assert_eq!(encode_news_post(&value), Err(expected));
        }

        let mut blank_reason = action();
        blank_reason.reason = Some("  ".into());
        assert_eq!(
            encode_editorial_action(&blank_reason),
            Err(NewswireModelError::FieldEmpty("reason"))
        );
        let mut blank_correction = action();
        blank_correction.correction_text = Some("  ".into());
        assert_eq!(
            encode_editorial_action(&blank_correction),
            Err(NewswireModelError::FieldEmpty("correction_text"))
        );

        let alert = OperationalProfileV1::Alert(AlertProfileV1 {
            urgency: Urgency::Immediate,
            severity: Severity::Severe,
            certainty: Certainty::Observed,
            valid_from_unix_seconds: Some(1_800_000_000),
        });
        let request = OperationalProfileV1::Request(RequestProfileV1 {
            kind: RequestKind::Need,
            needed_by_unix_seconds: Some(1_800_000_500),
            contact_instructions: "Meet at the community kitchen.".into(),
        });
        let profile_requirement_cases = [
            {
                let mut value = post();
                value.operational_profile = Some(alert.clone());
                (value, NewswireModelError::AlertExpiryRequired)
            },
            {
                let mut value = post();
                value.operational_profile = Some(alert.clone());
                value.expires_at_unix_seconds = Some(1_800_000_900);
                value.coarse_location = None;
                (value, NewswireModelError::AlertLocationRequired)
            },
            {
                let mut value = post();
                value.operational_profile = Some(alert);
                value.expires_at_unix_seconds = Some(1_800_000_900);
                value.source_claims.clear();
                (value, NewswireModelError::AlertSourceClaimRequired)
            },
            {
                let mut value = post();
                value.operational_profile = Some(request.clone());
                (value, NewswireModelError::RequestExpiryRequired)
            },
            {
                let mut value = post();
                value.operational_profile = Some(request);
                value.expires_at_unix_seconds = Some(1_800_000_900);
                value.coarse_location = None;
                (value, NewswireModelError::RequestLocationRequired)
            },
        ];
        for (value, expected) in profile_requirement_cases {
            assert_eq!(encode_news_post(&value), Err(expected));
        }

        let mut missing_correction = action();
        missing_correction.correction_text = None;
        assert_eq!(
            encode_editorial_action(&missing_correction),
            Err(NewswireModelError::CorrectionTextRequired)
        );
        for kind in [
            EditorialActionKind::Feature,
            EditorialActionKind::Verify,
            EditorialActionKind::Hide,
            EditorialActionKind::Tombstone,
            EditorialActionKind::Retract,
        ] {
            let mut value = action();
            value.kind = kind;
            assert_eq!(
                encode_editorial_action(&value),
                Err(NewswireModelError::CorrectionTextForbidden)
            );
        }
        for kind in [EditorialActionKind::Feature, EditorialActionKind::Verify] {
            let mut value = action();
            value.kind = kind;
            value.correction_text = None;
            assert_eq!(
                encode_editorial_action(&value),
                Err(NewswireModelError::EditorialReasonForbidden)
            );
        }
        for kind in [
            EditorialActionKind::Correct,
            EditorialActionKind::Hide,
            EditorialActionKind::Tombstone,
            EditorialActionKind::Retract,
        ] {
            let mut value = action();
            value.kind = kind;
            value.reason = None;
            if kind != EditorialActionKind::Correct {
                value.correction_text = None;
            }
            assert_eq!(
                encode_editorial_action(&value),
                Err(NewswireModelError::EditorialReasonRequired)
            );
        }
    }

    #[test]
    fn complete_payload_size_boundary_is_exact() {
        let exact_limit = vec![0u8; MAX_NEWSWIRE_PAYLOAD_BYTES];
        assert_ne!(
            decode_space_descriptor(&exact_limit),
            Err(NewswireModelError::InputTooLarge)
        );
        let over_limit = vec![0u8; MAX_NEWSWIRE_PAYLOAD_BYTES + 1];
        assert_eq!(
            decode_space_descriptor(&over_limit),
            Err(NewswireModelError::InputTooLarge)
        );
    }

    #[test]
    fn every_payload_round_trips_with_optional_keys_present() {
        let mut descriptor = space();
        descriptor.predecessor = Some([0x30; 32]);
        descriptor.successor = Some([0x31; 32]);
        let bytes = encode_space_descriptor(&descriptor).unwrap();
        assert_eq!(decode_space_descriptor(&bytes).unwrap(), descriptor);

        let value = post();
        let bytes = encode_news_post(&value).unwrap();
        assert_eq!(decode_news_post(&bytes).unwrap(), value);

        let value = action();
        let bytes = encode_editorial_action(&value).unwrap();
        assert_eq!(decode_editorial_action(&bytes).unwrap(), value);
    }

    #[test]
    fn canonicality_table_rejects_hostile_encodings() {
        let canonical = encode_space_descriptor(&space()).unwrap();
        let mut unknown = canonical.clone();
        unknown[0] += 1;
        unknown.extend_from_slice(&[0x18, 0x63, 0x00]);
        let mut trailing = canonical.clone();
        trailing.push(0);
        let mut widened = Vec::with_capacity(canonical.len() + 1);
        widened.push(canonical[0]);
        widened.extend_from_slice(&[0x18, 0x00]);
        widened.extend_from_slice(&canonical[2..]);
        let mut indefinite_map = vec![0xbf];
        indefinite_map.extend_from_slice(&canonical[1..]);
        indefinite_map.push(0xff);
        let mut indefinite_array = canonical.clone();
        let marker = [0x04, 0x81, 0x62, b'e', b'n'];
        let position = indefinite_array
            .windows(marker.len())
            .position(|window| window == marker)
            .unwrap();
        indefinite_array[position + 1] = 0x9f;
        indefinite_array.insert(position + marker.len(), 0xff);
        let mut invalid_utf8 = canonical.clone();
        let name_position = invalid_utf8
            .windows(space().name.len())
            .position(|window| window == space().name.as_bytes())
            .unwrap();
        invalid_utf8[name_position] = 0xff;
        let mut wrong_schema = canonical.clone();
        let schema_position = wrong_schema
            .windows(SPACE_SCHEMA.len())
            .position(|window| window == SPACE_SCHEMA.as_bytes())
            .unwrap();
        wrong_schema[schema_position] = b'x';
        let misordered = {
            let mut bytes = Vec::new();
            let mut encoder = Encoder::new(&mut bytes);
            encoder.map(2).unwrap();
            encoder.u8(1).unwrap().bytes(&[0x10; 32]).unwrap();
            encoder.u8(0).unwrap().str(SPACE_SCHEMA).unwrap();
            bytes
        };

        let cases = [
            (unknown, NewswireModelError::UnknownKey(99)),
            (misordered, NewswireModelError::DuplicateOrMisorderedKey(0)),
            (trailing, NewswireModelError::TrailingBytes),
            (widened, NewswireModelError::NonCanonical),
            (indefinite_map, NewswireModelError::NonCanonical),
            (indefinite_array, NewswireModelError::NonCanonical),
            (invalid_utf8, NewswireModelError::Malformed),
            (wrong_schema, NewswireModelError::WrongSchema),
        ];
        for (bytes, expected) in cases {
            assert_eq!(decode_space_descriptor(&bytes), Err(expected));
        }
    }

    #[test]
    fn closed_operational_and_action_tags_reject_unknown_values() {
        let mut request_post = post();
        request_post.expires_at_unix_seconds = Some(1_800_000_900);
        request_post.operational_profile = Some(OperationalProfileV1::Request(RequestProfileV1 {
            kind: RequestKind::Need,
            needed_by_unix_seconds: None,
            contact_instructions: "Use the public desk.".into(),
        }));
        let request_bytes = encode_news_post(&request_post).unwrap();
        let profile_marker = [0x09, 0xa2, 0x00, 0x01, 0x01, 0xa2, 0x00, 0x00];
        let profile_position = request_bytes
            .windows(profile_marker.len())
            .position(|window| window == profile_marker)
            .unwrap();

        let mut unknown_profile = request_bytes.clone();
        unknown_profile[profile_position + 3] = 2;
        assert_eq!(
            decode_news_post(&unknown_profile),
            Err(NewswireModelError::InvalidEnum("operational_profile"))
        );

        let mut unknown_request_kind = request_bytes.clone();
        unknown_request_kind[profile_position + 7] = 2;
        assert_eq!(
            decode_news_post(&unknown_request_kind),
            Err(NewswireModelError::InvalidEnum("request_kind"))
        );

        let top_level_ai_key = request_bytes
            .windows(2)
            .rposition(|window| window == [0x0a, 0xf4])
            .unwrap();
        let mut unknown_request_key = request_bytes;
        unknown_request_key[profile_position + 5] = 0xa3;
        unknown_request_key.splice(top_level_ai_key..top_level_ai_key, [0x03, 0x00]);
        assert_eq!(
            decode_news_post(&unknown_request_key),
            Err(NewswireModelError::UnknownKey(3))
        );

        let mut alert_post = post();
        alert_post.expires_at_unix_seconds = Some(1_800_000_900);
        alert_post.operational_profile = Some(OperationalProfileV1::Alert(AlertProfileV1 {
            urgency: Urgency::Immediate,
            severity: Severity::Severe,
            certainty: Certainty::Observed,
            valid_from_unix_seconds: None,
        }));
        let alert_bytes = encode_news_post(&alert_post).unwrap();
        let alert_marker = [
            0x09, 0xa2, 0x00, 0x00, 0x01, 0xa3, 0x00, 0x00, 0x01, 0x01, 0x02, 0x00,
        ];
        let alert_position = alert_bytes
            .windows(alert_marker.len())
            .position(|window| window == alert_marker)
            .unwrap();
        for (offset, field) in [(7, "urgency"), (9, "severity"), (11, "certainty")] {
            let mut unknown_alert_enum = alert_bytes.clone();
            unknown_alert_enum[alert_position + offset] = 5;
            assert_eq!(
                decode_news_post(&unknown_alert_enum),
                Err(NewswireModelError::InvalidEnum(field))
            );
        }

        let canonical_action = encode_editorial_action(&action()).unwrap();
        let kind_position = canonical_action
            .windows(2)
            .position(|window| window == [0x03, EditorialActionKind::Correct as u8])
            .unwrap();
        let mut unknown_action_kind = canonical_action;
        unknown_action_kind[kind_position + 1] = 6;
        assert_eq!(
            decode_editorial_action(&unknown_action_kind),
            Err(NewswireModelError::InvalidEnum("editorial_action_kind"))
        );
    }

    #[test]
    fn closed_enum_variants_round_trip() {
        let urgency = [
            Urgency::Immediate,
            Urgency::Expected,
            Urgency::Future,
            Urgency::Past,
            Urgency::Unknown,
        ];
        let severity = [
            Severity::Extreme,
            Severity::Severe,
            Severity::Moderate,
            Severity::Minor,
            Severity::Unknown,
        ];
        let certainty = [
            Certainty::Observed,
            Certainty::Likely,
            Certainty::Possible,
            Certainty::Unlikely,
            Certainty::Unknown,
        ];
        for index in 0..5 {
            let mut value = post();
            value.expires_at_unix_seconds = Some(1_800_000_900);
            value.operational_profile = Some(OperationalProfileV1::Alert(AlertProfileV1 {
                urgency: urgency[index],
                severity: severity[index],
                certainty: certainty[index],
                valid_from_unix_seconds: None,
            }));
            let bytes = encode_news_post(&value).unwrap();
            assert_eq!(decode_news_post(&bytes).unwrap(), value);
        }

        for kind in [RequestKind::Need, RequestKind::Offer] {
            let mut value = post();
            value.expires_at_unix_seconds = Some(1_800_000_900);
            value.operational_profile = Some(OperationalProfileV1::Request(RequestProfileV1 {
                kind,
                needed_by_unix_seconds: None,
                contact_instructions: "Signal through the public desk.".into(),
            }));
            let bytes = encode_news_post(&value).unwrap();
            assert_eq!(decode_news_post(&bytes).unwrap(), value);
        }

        for kind in [
            EditorialActionKind::Feature,
            EditorialActionKind::Verify,
            EditorialActionKind::Correct,
            EditorialActionKind::Hide,
            EditorialActionKind::Tombstone,
            EditorialActionKind::Retract,
        ] {
            let mut value = action();
            value.kind = kind;
            match kind {
                EditorialActionKind::Feature | EditorialActionKind::Verify => {
                    value.reason = None;
                    value.correction_text = None;
                }
                EditorialActionKind::Correct => {}
                EditorialActionKind::Hide
                | EditorialActionKind::Tombstone
                | EditorialActionKind::Retract => value.correction_text = None,
            }
            let bytes = encode_editorial_action(&value).unwrap();
            assert_eq!(decode_editorial_action(&bytes).unwrap(), value);
        }
    }
}
