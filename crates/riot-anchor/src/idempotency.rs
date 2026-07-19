//! The idempotency-index admission state machine.
//!
//! The durable index ([`crate::repository`]) stores a `control_request_digest`
//! and a [`crate::repository::IdempotencyClaimState`] per 128-bit key. This
//! module is the *decision* layer over one constant-time lookup: it maps an
//! existing row (or its absence) and the incoming request digest onto the closed
//! admission disposition the design's ordering table defines.
//!
//! The three participating states — `Claimed`, `Prepared`, `Terminal` — are the
//! only ones that replay. A *pre-claim* refusal (busy / quota / work) never
//! writes a row, so the same key and body may retry and still succeed; a claimed
//! key replayed with a **changed** body is `idempotency_conflict` and never
//! reveals or mutates the stored state.

use crate::repository::{IdempotencyClaimState, IdempotencyRow};

/// A winning `Claimed` row's lease (design "A `Claimed` row has a 30-second
/// lease").
pub const CLAIM_LEASE_SECS: u64 = 30;

/// Terminal / ordinary single-call results are retained for 24 hours.
pub const TERMINAL_RETENTION_SECS: u64 = 24 * 60 * 60;

/// A prepared mapping is retained through `operation_expiry + 24 hours`.
pub const PREPARED_RETENTION_EXTRA_SECS: u64 = 24 * 60 * 60;

/// `result_class` for an ordinary claim (never consumes the reserved partition).
pub const RESULT_CLASS_ORDINARY: u8 = 0;

/// `result_class` for the reserved removal partition.
pub const RESULT_CLASS_RESERVED: u8 = 1;

/// Constant-time equality over two 32-byte digests. The loop always inspects all
/// 32 bytes so a same-key attacker cannot learn *where* a mismatch occurred (or,
/// combined with the caller, that a stored result even exists) from timing.
#[must_use]
pub fn digests_equal_constant_time(left: &[u8; 32], right: &[u8; 32]) -> bool {
    let mut difference = 0u8;
    for (a, b) in left.iter().zip(right.iter()) {
        difference |= a ^ b;
    }
    difference == 0
}

/// The disposition of a request against the idempotency index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionLookup {
    /// No row for this key exists; proceed to the expensive checks and, only if
    /// they all pass, the durable claim.
    Novel,
    /// The key exists with a byte-equal digest; replay its retained state.
    ReplayEqual {
        /// The retained claim state to replay.
        state: IdempotencyClaimState,
        /// The operation the claim created, for `Prepared`/`Terminal` replay.
        operation_id: Option<[u8; 32]>,
    },
    /// The key exists with a different digest; `idempotency_conflict`. The stored
    /// state is neither revealed nor changed.
    Conflict,
}

/// Classify a request against its (optional) retained idempotency row.
///
/// This performs the design's precedence: an equal digest replays the exact
/// stored state; an unequal digest under the same key is a conflict; an absent
/// key proceeds. It runs *before* any authority / quota / work check.
#[must_use]
pub fn classify(existing: Option<&IdempotencyRow>, incoming_digest: &[u8; 32]) -> AdmissionLookup {
    match existing {
        None => AdmissionLookup::Novel,
        Some(row) => {
            if digests_equal_constant_time(&row.control_request_digest, incoming_digest) {
                AdmissionLookup::ReplayEqual {
                    state: row.claim_state,
                    operation_id: row.operation_id,
                }
            } else {
                AdmissionLookup::Conflict
            }
        }
    }
}
