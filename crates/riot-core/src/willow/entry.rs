//! The signed-alert entry factory: one clock snapshot sets both the alert's
//! `created_at` (UTC) and the Willow entry timestamp (TAI/J2000 µs); fresh
//! entropy mints the object/revision IDs; the author's subspace secret signs
//! the canonical entry encoding. Any failure constructs no partial entry.
//!
//! The production factory `create_signed_alert` uses OS randomness and the
//! system clock. The injectable variant is behind `conformance`.

use crate::model::{encode_alert, AlertPayload, Certainty, Severity, Urgency};

use super::clock::ClockSnapshot;
use super::identity::EvidenceAuthor;
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

/// Canonical component bytes of one signed, authorised Willow entry.
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

/// Shared core. `fill` supplies fresh ID bytes; `snapshot` supplies one
/// clock reading. Order: entropy first (IDs), clock second, validation
/// before signing — a failure at any stage returns before signing.
fn build<F>(
    author: &EvidenceAuthor,
    mut fill: F,
    snapshot: ClockSnapshot,
    draft: AlertDraft,
) -> Result<SignedAlert, WillowError>
where
    F: FnMut(&mut [u8]) -> Result<(), WillowError>,
{
    let mut object_id = [0u8; 16];
    fill(&mut object_id)?;
    let mut revision_id = [0u8; 16];
    fill(&mut revision_id)?;

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

fn os_fill(buf: &mut [u8]) -> Result<(), WillowError> {
    use rand_core::RngCore;
    rand_core::OsRng
        .try_fill_bytes(buf)
        .map_err(|_| WillowError::EntropyUnavailable)
}

/// Production factory: OS randomness for IDs, system clock for time.
pub fn create_signed_alert(
    author: &EvidenceAuthor,
    draft: AlertDraft,
) -> Result<SignedAlert, WillowError> {
    let snapshot = super::clock::system_snapshot()?;
    build(author, os_fill, snapshot, draft)
}

/// Injectable variant for deterministic/failing entropy and clocks in tests.
#[cfg(feature = "conformance")]
pub fn create_signed_alert_with(
    author: &EvidenceAuthor,
    entropy: &mut dyn super::identity::EntropySource,
    clock: &dyn super::clock::ClockSource,
    draft: AlertDraft,
) -> Result<SignedAlert, WillowError> {
    let snapshot = clock.snapshot()?;
    build(author, |buf| entropy.fill(buf), snapshot, draft)
}

/// Path binding sanity used by tests.
pub fn expected_alert_path(
    object_id: &[u8; 16],
    revision_id: &[u8; 16],
) -> Result<willow25::paths::Path, WillowError> {
    alert_path(object_id, revision_id)
}
