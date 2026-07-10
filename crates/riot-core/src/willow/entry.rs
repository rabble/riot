//! The signed-alert entry factory: one clock snapshot sets both the alert's
//! `created_at` (UTC) and the Willow entry timestamp (TAI/J2000 µs); fresh
//! entropy mints the object/revision IDs; the author's subspace secret signs
//! the canonical entry encoding. Any failure constructs no partial entry.

use crate::model::{encode_alert, AlertPayload, Certainty, Severity, Urgency};

use super::clock::{ClockSnapshot, ClockSource};
use super::identity::{EntropySource, EvidenceAuthor};
use super::{alert_path, WillowError};

/// Everything the author chooses; times and IDs are factory-assigned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertDraft {
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

/// Canonical component bytes of one signed, authorised Willow entry — the
/// exact shape a bundle item carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedWillowEntry {
    pub entry_bytes: Vec<u8>,
    pub capability_bytes: Vec<u8>,
    pub signature: [u8; 64],
    pub payload_bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedAlert {
    pub signed: SignedWillowEntry,
    pub snapshot: ClockSnapshot,
    pub payload: AlertPayload,
    pub object_id: [u8; 16],
    pub revision_id: [u8; 16],
}

/// Builds, signs, and authorises one alert entry. Order matters: entropy
/// first (IDs), clock second, validation before signing — a failure at any
/// stage returns before any signature or allocation-heavy work.
pub fn create_signed_alert(
    author: &EvidenceAuthor,
    entropy: &mut dyn EntropySource,
    clock: &dyn ClockSource,
    draft: AlertDraft,
) -> Result<SignedAlert, WillowError> {
    let mut object_id = [0u8; 16];
    entropy.fill(&mut object_id)?;
    let mut revision_id = [0u8; 16];
    entropy.fill(&mut revision_id)?;

    let snapshot = clock.snapshot()?;

    let payload = AlertPayload {
        object_id,
        revision_id,
        created_at: snapshot.unix_seconds,
        valid_from: draft.valid_from,
        expires_at: draft.expires_at,
        language: draft.language,
        urgency: draft.urgency,
        severity: draft.severity,
        certainty: draft.certainty,
        headline: draft.headline,
        description: draft.description,
        affected_area_claim: draft.affected_area_claim,
        source_claims: draft.source_claims,
        ai_assisted: draft.ai_assisted,
    };
    // Validity fields are checked against the snapshot-derived created_at.
    let payload_bytes = encode_alert(&payload).map_err(WillowError::InvalidAlert)?;

    let entry = super::build_alert_entry(
        author,
        &object_id,
        &revision_id,
        snapshot.tai_j2000_micros,
        &payload_bytes,
    )?;
    let authorised = super::authorise_entry(author, entry)?;

    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();

    Ok(SignedAlert {
        signed: SignedWillowEntry {
            entry_bytes: super::encode_entry(authorised.entry()),
            capability_bytes: super::encode_capability(token.capability()),
            signature: signature.to_bytes(),
            payload_bytes,
        },
        snapshot,
        payload,
        object_id,
        revision_id,
    })
}

/// Path binding sanity used by tests: the factory always writes to the
/// fixed four-component alert path.
pub fn expected_alert_path(
    object_id: &[u8; 16],
    revision_id: &[u8; 16],
) -> Result<willow25::paths::Path, WillowError> {
    alert_path(object_id, revision_id)
}
