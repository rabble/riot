//! Signed alert payload model and deterministic CBOR codec.
//!
//! Wire rules (schema `org.riot.alert/1`, see `schemas/alert.cddl`):
//! definite lengths only, integer map keys in ascending order, shortest
//! integer encodings, no floating point, no duplicate or unknown keys.
//! Decoding accepts only the exact canonical encoding: parsed payloads are
//! re-encoded and must reproduce the input bytes.

use minicbor::data::Type;
use minicbor::{Decoder, Encoder};

pub const ALERT_SCHEMA: &str = "org.riot.alert/1";

/// `payload_bytes` ceiling from fixtures/manifest.json.
pub const MAX_PAYLOAD_BYTES: usize = 1_048_576;

const MAX_LANGUAGE_BYTES: usize = 35;
const MIN_LANGUAGE_BYTES: usize = 2;
const MAX_HEADLINE_BYTES: usize = 512;
const MAX_DESCRIPTION_BYTES: usize = 65_536;
const MAX_AREA_BYTES: usize = 2_048;
const MAX_SOURCE_CLAIMS: usize = 16;
const MAX_SOURCE_CLAIM_BYTES: usize = 1_024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Urgency {
    Immediate = 0,
    Expected = 1,
    Future = 2,
    Past = 3,
    Unknown = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Extreme = 0,
    Severe = 1,
    Moderate = 2,
    Minor = 3,
    Unknown = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Certainty {
    Observed = 0,
    Likely = 1,
    Possible = 2,
    Unlikely = 3,
    Unknown = 4,
}

macro_rules! enum_from_u8 {
    ($name:ident, $($variant:ident => $value:literal),+) => {
        impl $name {
            fn from_u8(value: u8) -> Option<Self> {
                match value {
                    $($value => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }
    };
}

enum_from_u8!(Urgency, Immediate => 0, Expected => 1, Future => 2, Past => 3, Unknown => 4);
enum_from_u8!(Severity, Extreme => 0, Severe => 1, Moderate => 2, Minor => 3, Unknown => 4);
enum_from_u8!(Certainty, Observed => 0, Likely => 1, Possible => 2, Unlikely => 3, Unknown => 4);

/// The author-signed alert payload. Signer, namespace, capability, Willow
/// timestamp, and payload digest belong to the Willow entry, never in here.
/// Local receipt facts (route, first-seen, trust) never enter this payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertPayload {
    pub object_id: [u8; 16],
    pub revision_id: [u8; 16],
    pub created_at: u64,
    pub valid_from: Option<u64>,
    pub expires_at: u64,
    pub language: String,
    pub urgency: Urgency,
    pub severity: Severity,
    pub certainty: Certainty,
    pub headline: String,
    pub description: String,
    pub affected_area_claim: Option<String>,
    pub source_claims: Vec<String>,
    pub ai_assisted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlertError {
    ExpiryNotAfterCreated,
    MissingSourceClaim,
    TooManySourceClaims,
    FieldEmpty(&'static str),
    FieldTooLarge(&'static str),
    InputTooLarge,
    UnknownKey(u64),
    DuplicateOrMisorderedKey(u64),
    MissingKey(u64),
    WrongSchema,
    InvalidEnum(&'static str),
    NonCanonical,
    TrailingBytes,
    Malformed,
}

impl std::fmt::Display for AlertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for AlertError {}

fn validate(alert: &AlertPayload) -> Result<(), AlertError> {
    if alert.expires_at <= alert.created_at {
        return Err(AlertError::ExpiryNotAfterCreated);
    }
    check_text(
        "language",
        &alert.language,
        MIN_LANGUAGE_BYTES,
        MAX_LANGUAGE_BYTES,
    )?;
    check_text("headline", &alert.headline, 1, MAX_HEADLINE_BYTES)?;
    check_text("description", &alert.description, 1, MAX_DESCRIPTION_BYTES)?;
    if let Some(area) = &alert.affected_area_claim {
        check_text("affected_area_claim", area, 1, MAX_AREA_BYTES)?;
    }
    if alert.source_claims.is_empty() {
        return Err(AlertError::MissingSourceClaim);
    }
    if alert.source_claims.len() > MAX_SOURCE_CLAIMS {
        return Err(AlertError::TooManySourceClaims);
    }
    for claim in &alert.source_claims {
        if claim.trim().is_empty() {
            return Err(AlertError::MissingSourceClaim);
        }
        if claim.len() > MAX_SOURCE_CLAIM_BYTES {
            return Err(AlertError::FieldTooLarge("source_claim"));
        }
    }
    Ok(())
}

fn check_text(name: &'static str, value: &str, min: usize, max: usize) -> Result<(), AlertError> {
    if value.trim().is_empty() || value.len() < min {
        return Err(AlertError::FieldEmpty(name));
    }
    if value.len() > max {
        return Err(AlertError::FieldTooLarge(name));
    }
    Ok(())
}

/// Validates and encodes the canonical byte representation.
pub fn encode_alert(alert: &AlertPayload) -> Result<Vec<u8>, AlertError> {
    validate(alert)?;

    let mut pairs: u64 = 13;
    if alert.valid_from.is_some() {
        pairs += 1;
    }
    if alert.affected_area_claim.is_some() {
        pairs += 1;
    }

    let mut buffer: Vec<u8> = Vec::new();
    let mut e = Encoder::new(&mut buffer);
    let r: Result<_, minicbor::encode::Error<core::convert::Infallible>> = (|| {
        e.map(pairs)?;
        e.u8(0)?.str(ALERT_SCHEMA)?;
        e.u8(1)?.bytes(&alert.object_id)?;
        e.u8(2)?.bytes(&alert.revision_id)?;
        e.u8(3)?.u64(alert.created_at)?;
        if let Some(valid_from) = alert.valid_from {
            e.u8(4)?.u64(valid_from)?;
        }
        e.u8(5)?.u64(alert.expires_at)?;
        e.u8(6)?.str(&alert.language)?;
        e.u8(7)?.u8(alert.urgency as u8)?;
        e.u8(8)?.u8(alert.severity as u8)?;
        e.u8(9)?.u8(alert.certainty as u8)?;
        e.u8(10)?.str(&alert.headline)?;
        e.u8(11)?.str(&alert.description)?;
        if let Some(area) = &alert.affected_area_claim {
            e.u8(12)?.str(area)?;
        }
        e.u8(13)?.array(alert.source_claims.len() as u64)?;
        for claim in &alert.source_claims {
            e.str(claim)?;
        }
        e.u8(14)?.bool(alert.ai_assisted)?;
        Ok(())
    })();
    r.map_err(|_| AlertError::Malformed)?;

    debug_assert!(buffer.len() <= MAX_PAYLOAD_BYTES);
    Ok(buffer)
}

/// Strict canonical decoder: rejects unknown/duplicate/misordered keys,
/// indefinite lengths, trailing bytes, and any non-canonical encoding.
pub fn decode_alert(input: &[u8]) -> Result<AlertPayload, AlertError> {
    if input.len() > MAX_PAYLOAD_BYTES {
        return Err(AlertError::InputTooLarge);
    }

    let mut d = Decoder::new(input);
    let pairs = d
        .map()
        .map_err(|_| AlertError::Malformed)?
        .ok_or(AlertError::NonCanonical)?;
    // `map_entries` ceiling; precise unknown-key rejection happens per key
    // below so hostile extra keys get their distinct error code.
    if pairs < 13 || pairs > 128 {
        return Err(AlertError::Malformed);
    }

    let mut schema: Option<String> = None;
    let mut object_id: Option<[u8; 16]> = None;
    let mut revision_id: Option<[u8; 16]> = None;
    let mut created_at: Option<u64> = None;
    let mut valid_from: Option<u64> = None;
    let mut expires_at: Option<u64> = None;
    let mut language: Option<String> = None;
    let mut urgency: Option<Urgency> = None;
    let mut severity: Option<Severity> = None;
    let mut certainty: Option<Certainty> = None;
    let mut headline: Option<String> = None;
    let mut description: Option<String> = None;
    let mut affected_area_claim: Option<String> = None;
    let mut source_claims: Option<Vec<String>> = None;
    let mut ai_assisted: Option<bool> = None;

    let mut last_key: Option<u64> = None;
    for _ in 0..pairs {
        let key = d.u64().map_err(|_| AlertError::Malformed)?;
        if let Some(previous) = last_key {
            if key <= previous {
                return Err(AlertError::DuplicateOrMisorderedKey(key));
            }
        }
        last_key = Some(key);

        match key {
            0 => schema = Some(decode_text(&mut d, "schema", 64)?),
            1 => object_id = Some(decode_id(&mut d)?),
            2 => revision_id = Some(decode_id(&mut d)?),
            3 => created_at = Some(d.u64().map_err(|_| AlertError::Malformed)?),
            4 => valid_from = Some(d.u64().map_err(|_| AlertError::Malformed)?),
            5 => expires_at = Some(d.u64().map_err(|_| AlertError::Malformed)?),
            6 => language = Some(decode_text(&mut d, "language", MAX_LANGUAGE_BYTES)?),
            7 => {
                let raw = d.u8().map_err(|_| AlertError::Malformed)?;
                urgency = Some(Urgency::from_u8(raw).ok_or(AlertError::InvalidEnum("urgency"))?);
            }
            8 => {
                let raw = d.u8().map_err(|_| AlertError::Malformed)?;
                severity = Some(Severity::from_u8(raw).ok_or(AlertError::InvalidEnum("severity"))?);
            }
            9 => {
                let raw = d.u8().map_err(|_| AlertError::Malformed)?;
                certainty =
                    Some(Certainty::from_u8(raw).ok_or(AlertError::InvalidEnum("certainty"))?);
            }
            10 => headline = Some(decode_text(&mut d, "headline", MAX_HEADLINE_BYTES)?),
            11 => description = Some(decode_text(&mut d, "description", MAX_DESCRIPTION_BYTES)?),
            12 => {
                affected_area_claim =
                    Some(decode_text(&mut d, "affected_area_claim", MAX_AREA_BYTES)?)
            }
            13 => {
                let len = d
                    .array()
                    .map_err(|_| AlertError::Malformed)?
                    .ok_or(AlertError::NonCanonical)?;
                if len == 0 {
                    return Err(AlertError::MissingSourceClaim);
                }
                if len as usize > MAX_SOURCE_CLAIMS {
                    return Err(AlertError::TooManySourceClaims);
                }
                let mut claims = Vec::with_capacity(len as usize);
                for _ in 0..len {
                    claims.push(decode_text(&mut d, "source_claim", MAX_SOURCE_CLAIM_BYTES)?);
                }
                source_claims = Some(claims);
            }
            14 => ai_assisted = Some(d.bool().map_err(|_| AlertError::Malformed)?),
            other => return Err(AlertError::UnknownKey(other)),
        }
    }

    if d.position() != input.len() {
        return Err(AlertError::TrailingBytes);
    }
    if schema.as_deref() != Some(ALERT_SCHEMA) {
        return Err(AlertError::WrongSchema);
    }

    let alert = AlertPayload {
        object_id: object_id.ok_or(AlertError::MissingKey(1))?,
        revision_id: revision_id.ok_or(AlertError::MissingKey(2))?,
        created_at: created_at.ok_or(AlertError::MissingKey(3))?,
        valid_from,
        expires_at: expires_at.ok_or(AlertError::MissingKey(5))?,
        language: language.ok_or(AlertError::MissingKey(6))?,
        urgency: urgency.ok_or(AlertError::MissingKey(7))?,
        severity: severity.ok_or(AlertError::MissingKey(8))?,
        certainty: certainty.ok_or(AlertError::MissingKey(9))?,
        headline: headline.ok_or(AlertError::MissingKey(10))?,
        description: description.ok_or(AlertError::MissingKey(11))?,
        affected_area_claim,
        source_claims: source_claims.ok_or(AlertError::MissingKey(13))?,
        ai_assisted: ai_assisted.ok_or(AlertError::MissingKey(14))?,
    };

    validate(&alert)?;

    // Canonicality proof: only the exact encoder output is acceptable.
    let reencoded = encode_alert(&alert)?;
    if reencoded != input {
        return Err(AlertError::NonCanonical);
    }

    Ok(alert)
}

fn decode_id(d: &mut Decoder<'_>) -> Result<[u8; 16], AlertError> {
    let bytes = d.bytes().map_err(|_| AlertError::Malformed)?;
    <[u8; 16]>::try_from(bytes).map_err(|_| AlertError::Malformed)
}

fn decode_text(d: &mut Decoder<'_>, name: &'static str, max: usize) -> Result<String, AlertError> {
    if d.datatype().map_err(|_| AlertError::Malformed)? != Type::String {
        return Err(AlertError::Malformed);
    }
    let text = d.str().map_err(|_| AlertError::Malformed)?;
    if text.len() > max {
        return Err(AlertError::FieldTooLarge(name));
    }
    Ok(text.to_string())
}
