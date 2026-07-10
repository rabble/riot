//! One clock snapshot yields both time views: UTC Unix seconds for the
//! signed alert payload and TAI/J2000 microseconds for the Willow entry.
//! They are separately labelled and never interchangeable.
//!
//! The production reading is `system_snapshot`, which takes no injectable
//! source. Injectable clocks and the deterministic-instant helper live
//! behind the `conformance` feature.

use super::WillowError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockSnapshot {
    /// UTC Unix seconds — the product-interchange view inside signed alerts.
    pub unix_seconds: u64,
    /// Microseconds of TAI since J2000 — the Willow join-recency view.
    pub tai_j2000_micros: u64,
    /// Conservative local clock uncertainty for preview provenance.
    pub uncertainty_seconds: u32,
}

/// Converts one UTC Unix reading into a full snapshot via pinned hifitime.
/// System failure, pre-J2000 readings, and out-of-range conversions all map
/// to `CLOCK_UNAVAILABLE` — no partial snapshot escapes. Internal to the
/// production path; also the deterministic helper behind `conformance`.
pub(crate) fn snapshot_from_unix_seconds_internal(
    unix_seconds: i64,
    uncertainty_seconds: u32,
) -> Result<ClockSnapshot, WillowError> {
    if unix_seconds < 0 {
        return Err(WillowError::ClockUnavailable);
    }
    let epoch = hifitime::Epoch::from_unix_seconds(unix_seconds as f64);
    let timestamp =
        willow25::entry::Timestamp::try_from(epoch).map_err(|_| WillowError::ClockUnavailable)?;
    Ok(ClockSnapshot {
        unix_seconds: unix_seconds as u64,
        tai_j2000_micros: u64::from(timestamp),
        uncertainty_seconds,
    })
}

/// Production reading: one `SystemTime` read, conservative uncertainty.
pub fn system_snapshot() -> Result<ClockSnapshot, WillowError> {
    let unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| WillowError::ClockUnavailable)?;
    let seconds = i64::try_from(unix.as_secs()).map_err(|_| WillowError::ClockUnavailable)?;
    snapshot_from_unix_seconds_internal(seconds, 60)
}

// ---------------------------------------------------------------------------
// Conformance-only injection surface (feature-gated; absent from release).
// ---------------------------------------------------------------------------

/// A fallible clock. Test/conformance only.
#[cfg(feature = "conformance")]
pub trait ClockSource {
    fn snapshot(&self) -> Result<ClockSnapshot, WillowError>;
}

/// Deterministic-instant helper for tests.
#[cfg(feature = "conformance")]
pub fn snapshot_from_unix_seconds(
    unix_seconds: i64,
    uncertainty_seconds: u32,
) -> Result<ClockSnapshot, WillowError> {
    snapshot_from_unix_seconds_internal(unix_seconds, uncertainty_seconds)
}

/// The production system clock, exposed as an injectable source for tests.
#[cfg(feature = "conformance")]
pub struct SystemClock;

#[cfg(feature = "conformance")]
impl ClockSource for SystemClock {
    fn snapshot(&self) -> Result<ClockSnapshot, WillowError> {
        system_snapshot()
    }
}
