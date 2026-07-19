//! Admission work: challenge issuance, stamp verification, and the deterministic
//! namespace-token derivation.
//!
//! The design binds a `WorkChallengeV1` to every coordinate of the intended
//! request (`operation_kind`, `idempotency_key`, `work_target_digest`,
//! `community_root`) plus the pressure-band policy (`policy_epoch`,
//! `difficulty`). A `PrepareHost` stamp is verified against those exact
//! coordinates *before* any durable claim, so an insufficient or mismatched
//! stamp is a pre-claim refusal that leaves no idempotency row.
//!
//! Namespace tokens are `HMAC-SHA256(anchor_operation_secret, preimage)` over the
//! design's `riot/namespace-token/v1` input. The anchor holds the keyed secret
//! ring here; the protocol crate only builds the preimage bytes. Rotating the
//! ring retains prior secrets until every operation minted under them expires.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use riot_anchor_protocol::codec::CodecError;
use riot_anchor_protocol::control::{ControlRefusal, GetWorkChallengeV1};
use riot_anchor_protocol::digest::namespace_token_hmac_input;
use riot_anchor_protocol::records::{
    ControlOperationKind, OperatorSignedEnvelopeV1, WorkChallengeBodyV1, WorkChallengeV1,
    WorkStampV1, IDEMPOTENCY_KEY_BYTES,
};

/// The maximum admission-work-challenge lifetime (design "at most 5 minutes").
pub const MAX_WORK_CHALLENGE_TTL_SECS: u64 = 300;

/// Abstracts the anchor's operator signing key so this crate stays independent
/// of a concrete key-management backend. Production wiring lands with the
/// operator-key work unit; tests provide an Ed25519 signer.
pub trait OperatorSigner {
    /// Return the 64-byte Ed25519 signature over `preimage`.
    fn sign(&self, preimage: &[u8]) -> [u8; 64];
}

/// The pressure-band policy the anchor applies to a community: the current
/// `policy_epoch` and the required leading-zero-bit `difficulty` (`0` = no work
/// required this band).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PressurePolicy {
    /// The pressure-policy epoch bound into every challenge.
    pub policy_epoch: u64,
    /// The required difficulty (`0..=24`); `0` means no admission work.
    pub difficulty: u64,
}

/// The immutable descriptor coordinates a signed work challenge carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChallengeSigningContext {
    /// Stable anchor id.
    pub anchor_id: [u8; 32],
    /// Signing operator key id.
    pub operator_key_id: [u8; 32],
    /// Signing descriptor epoch.
    pub descriptor_epoch: u64,
    /// Signing descriptor digest.
    pub descriptor_digest: [u8; 32],
}

/// Issue a signed `WorkChallengeV1` binding the intended request coordinates and
/// the pressure-band policy. `ttl` is clamped to [`MAX_WORK_CHALLENGE_TTL_SECS`].
pub fn issue_work_challenge<S: OperatorSigner>(
    signer: &S,
    context: &ChallengeSigningContext,
    request: &GetWorkChallengeV1,
    policy: PressurePolicy,
    random_challenge: [u8; 32],
    issued_at: u64,
    ttl: u64,
) -> Result<WorkChallengeV1, CodecError> {
    let ttl = ttl.min(MAX_WORK_CHALLENGE_TTL_SECS);
    let body = WorkChallengeBodyV1 {
        anchor_id: context.anchor_id,
        operator_key_id: context.operator_key_id,
        descriptor_epoch: context.descriptor_epoch,
        descriptor_digest: context.descriptor_digest,
        operation_kind: request.intended_operation_kind,
        idempotency_key: request.intended_idempotency_key,
        work_target_digest: request.work_target_digest,
        community_root: request.community_root,
        random_challenge,
        policy_epoch: policy.policy_epoch,
        difficulty: policy.difficulty,
        issued_at,
        expires_at: issued_at.saturating_add(ttl),
    };
    let mut envelope = OperatorSignedEnvelopeV1 {
        body,
        operator_signature: [0u8; 64],
    };
    let preimage = envelope.signing_preimage()?;
    envelope.operator_signature = signer.sign(&preimage);
    Ok(envelope)
}

/// The coordinates an admission work stamp must bind for a specific request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequiredWork {
    /// The intended operation kind.
    pub operation_kind: ControlOperationKind,
    /// The bound 128-bit idempotency key.
    pub idempotency_key: [u8; IDEMPOTENCY_KEY_BYTES],
    /// The bound `work_target_digest` (request with the work-stamp slot null).
    pub work_target_digest: [u8; 32],
    /// The bound community root.
    pub community_root: [u8; 32],
    /// The required pressure-band policy.
    pub policy: PressurePolicy,
}

fn work_required(required: &RequiredWork) -> ControlRefusal {
    ControlRefusal::WorkRequired {
        policy_epoch: required.policy.policy_epoch,
        difficulty: required.policy.difficulty,
    }
}

/// Verify an optional admission work stamp against the required coordinates and
/// pressure band. Returns `Ok(())` when no work is required (band difficulty 0)
/// or the stamp binds every coordinate, meets the difficulty, and is unexpired.
///
/// Every failure is a *pre-claim* refusal (`work_required` or `work_expired`):
/// a missing, malformed, insufficient, or mis-bound stamp is `work_required`, so
/// the same idempotency key may retry with a fresh stamp because no digest was
/// ever stored. Only a well-formed, correctly-bound-but-expired challenge is
/// `work_expired`.
pub fn verify_admission_work(
    operator_public_key: &[u8; 32],
    stamp: Option<&WorkStampV1>,
    required: &RequiredWork,
    observed_at: u64,
) -> Result<(), ControlRefusal> {
    if required.policy.difficulty == 0 {
        return Ok(());
    }
    let stamp = stamp.ok_or_else(|| work_required(required))?;
    let challenge = stamp
        .verify(operator_public_key)
        .map_err(|_| work_required(required))?;

    let coordinates_bind = challenge.operation_kind == required.operation_kind
        && challenge.idempotency_key == required.idempotency_key
        && challenge.work_target_digest == required.work_target_digest
        && challenge.community_root == required.community_root
        && challenge.policy_epoch == required.policy.policy_epoch
        && challenge.difficulty >= required.policy.difficulty;
    if !coordinates_bind {
        return Err(work_required(required));
    }

    // A correctly-bound challenge that has aged out is `work_expired`; a
    // not-yet-valid clock skew is treated as `work_required` (fetch a fresh one).
    if observed_at >= challenge.expires_at {
        return Err(ControlRefusal::WorkExpired {
            expires_at: challenge.expires_at,
            observed_at,
        });
    }
    if observed_at < challenge.issued_at {
        return Err(work_required(required));
    }
    Ok(())
}

/// HMAC-SHA256 with a 32-byte key (shorter than the 64-byte SHA-256 block, so
/// the key is zero-padded, never hashed).
fn hmac_sha256(key: &[u8; 32], message: &[u8]) -> [u8; 32] {
    const BLOCK: usize = 64;
    let mut ipad = [0x36u8; BLOCK];
    let mut opad = [0x5cu8; BLOCK];
    for (index, byte) in key.iter().enumerate() {
        ipad[index] ^= byte;
        opad[index] ^= byte;
    }
    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(message);
    let inner_digest = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner_digest);
    let mut result = [0u8; 32];
    result.copy_from_slice(&outer.finalize());
    result
}

/// Derive one deterministic namespace token:
/// `HMAC-SHA256(secret, riot/namespace-token/v1 preimage)`.
pub fn derive_namespace_token(
    secret: &[u8; 32],
    operation_id: &[u8; 32],
    namespace_id: &[u8; 32],
    operation_expiry: u64,
    token_secret_epoch: u32,
) -> [u8; 32] {
    let preimage = namespace_token_hmac_input(
        operation_id,
        namespace_id,
        operation_expiry,
        token_secret_epoch,
    );
    hmac_sha256(secret, &preimage)
}

/// The anchor's namespace-token secret ring. Keyed by epoch so a rotation can
/// retain prior secrets until every operation minted under them has expired.
#[derive(Debug, Clone)]
pub struct TokenSecretRing {
    current_epoch: u32,
    secrets: BTreeMap<u32, [u8; 32]>,
}

impl TokenSecretRing {
    /// Create a ring with a single active epoch/secret.
    pub fn new(current_epoch: u32, secret: [u8; 32]) -> Self {
        let mut secrets = BTreeMap::new();
        secrets.insert(current_epoch, secret);
        Self {
            current_epoch,
            secrets,
        }
    }

    /// The current (minting) epoch.
    pub fn current_epoch(&self) -> u32 {
        self.current_epoch
    }

    /// The secret retained for `epoch`, if any.
    pub fn secret(&self, epoch: u32) -> Option<&[u8; 32]> {
        self.secrets.get(&epoch)
    }

    /// Rotate in a new active epoch/secret while retaining all prior secrets.
    pub fn rotate(&mut self, new_epoch: u32, secret: [u8; 32]) {
        self.secrets.insert(new_epoch, secret);
        self.current_epoch = new_epoch;
    }

    /// Drop every retained secret whose epoch is strictly below `min_epoch`
    /// (called once every operation under those epochs has expired). The current
    /// epoch is always retained.
    pub fn retire_below(&mut self, min_epoch: u32) {
        let current = self.current_epoch;
        self.secrets
            .retain(|epoch, _| *epoch >= min_epoch || *epoch == current);
    }

    /// Derive a namespace token under a retained epoch, or `None` if that epoch's
    /// secret was already retired.
    pub fn derive(
        &self,
        epoch: u32,
        operation_id: &[u8; 32],
        namespace_id: &[u8; 32],
        operation_expiry: u64,
    ) -> Option<[u8; 32]> {
        let secret = self.secrets.get(&epoch)?;
        Some(derive_namespace_token(
            secret,
            operation_id,
            namespace_id,
            operation_expiry,
            epoch,
        ))
    }
}
