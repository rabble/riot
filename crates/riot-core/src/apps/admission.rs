//! Pure, host-agnostic install-capacity preflight. Apple and Android call the
//! same function with their own current counts/bytes so they cannot diverge on
//! whether a pair fits. Byte and count limits are enforced here BEFORE any
//! runtime, trust, serving-store, or disk mutation. This module performs no I/O
//! and holds no state; the caller owns the profile lock.

/// Hard cap on distinct installed app IDs. Matches Android's persisted-profile
/// count cap so the disk format and runtime agree.
pub const MAX_INSTALLED_APPS: usize = 32;

/// Aggregate ceiling on the sum of installed manifest + bundle byte lengths
/// across all held IDs. Exactly 3 MiB. Pre-upgrade over-quota profiles are
/// restore-only grandfathered by the caller (not this function).
pub const MAX_AGGREGATE_PAIR_BYTES: usize = 3 * 1024 * 1024;

/// The distinct outcomes of a capacity preflight. Count and bytes are never
/// collapsed: the two conditions have different user-facing copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionOutcome {
    Admit,
    RefuseCount,
    RefuseBytes,
}

/// Decide whether a prospective pair may be admitted.
///
/// * `held_count` — distinct app IDs already installed.
/// * `held_aggregate_bytes` — sum of held manifest+bundle byte lengths.
/// * `pair_already_held` — the prospective pair's ID is already installed
///   (idempotent restoration: no count/byte increase, always admits).
/// * `pair_bytes` — prospective pair's manifest.len() + bundle.len().
///
/// Count is checked before bytes so a caller mapping errors gets a
/// deterministic reason when both are exceeded.
pub fn preflight(
    held_count: usize,
    held_aggregate_bytes: usize,
    pair_already_held: bool,
    pair_bytes: usize,
) -> AdmissionOutcome {
    if pair_already_held {
        return AdmissionOutcome::Admit;
    }
    if held_count + 1 > MAX_INSTALLED_APPS {
        return AdmissionOutcome::RefuseCount;
    }
    if held_aggregate_bytes.saturating_add(pair_bytes) > MAX_AGGREGATE_PAIR_BYTES {
        return AdmissionOutcome::RefuseBytes;
    }
    AdmissionOutcome::Admit
}
