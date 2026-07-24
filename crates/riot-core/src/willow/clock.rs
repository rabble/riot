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
    let unix_seconds = u64::try_from(unix_seconds).map_err(|_| WillowError::ClockUnavailable)?;
    let epoch = hifitime::Epoch::from_unix_seconds(unix_seconds as f64);
    let timestamp =
        willow25::entry::Timestamp::try_from(epoch).map_err(|_| WillowError::ClockUnavailable)?;
    Ok(ClockSnapshot {
        unix_seconds,
        tai_j2000_micros: u64::from(timestamp),
        uncertainty_seconds,
    })
}

/// Production reading: one `SystemTime` read, conservative uncertainty.
pub fn system_snapshot() -> Result<ClockSnapshot, WillowError> {
    snapshot_from_unix_duration(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH))
}

fn snapshot_from_unix_duration(
    unix: Result<std::time::Duration, std::time::SystemTimeError>,
) -> Result<ClockSnapshot, WillowError> {
    unix.map_err(|_| WillowError::ClockUnavailable)
        .and_then(|duration| {
            i64::try_from(duration.as_secs()).map_err(|_| WillowError::ClockUnavailable)
        })
        .and_then(|seconds| snapshot_from_unix_seconds_internal(seconds, 60))
}

/// Convert a UTC Unix-seconds wall-clock reading into TAI/J2000 microseconds —
/// the unit every Willow entry `Timestamp` uses (see `tai_j2000_micros`). This
/// is the production converter for turning a caller-supplied wall-clock instant
/// (e.g. a capability expiry) into the entry-time domain; it takes the SAME
/// pinned-hifitime path as the live snapshot, so a converted expiry and a real
/// entry timestamp are directly comparable. Out-of-range / pre-J2000 readings
/// map to `CLOCK_UNAVAILABLE`. The conversion is strictly increasing in
/// `unix_seconds`, so ordering is preserved across the unit change.
pub fn tai_j2000_micros_from_unix_seconds(unix_seconds: u64) -> Result<u64, WillowError> {
    let seconds = i64::try_from(unix_seconds).map_err(|_| WillowError::ClockUnavailable)?;
    Ok(snapshot_from_unix_seconds_internal(seconds, 0)?.tai_j2000_micros)
}

/// The inverse of [`tai_j2000_micros_from_unix_seconds`]: recover the UTC
/// Unix-seconds wall-clock reading from a Willow entry `Timestamp` (TAI/J2000
/// microseconds). Used to give a renderer a real "created at" wall-clock instant
/// from the only time an open-wire post carries — its entry timestamp — so no
/// epoch math is hand-rolled at the display boundary (this repo has a documented
/// time-unit trap: entry timestamps are TAI/J2000 µs, NOT Unix seconds). Takes
/// the SAME pinned-hifitime path as the forward converter and the live snapshot,
/// so the round trip is exact at second resolution. Pre-epoch instants (a
/// timestamp earlier than 1970) and out-of-range readings map to
/// `CLOCK_UNAVAILABLE` rather than silently wrapping.
pub fn unix_seconds_from_tai_j2000_micros(tai_j2000_micros: u64) -> Result<u64, WillowError> {
    // Invert the forward path EXACTLY. Forward is
    //   micros = (Epoch::from_unix_seconds(s) - J2000_REF_EPOCH).total_nanos / 1000,
    // whose subtraction is a UTC-clock duration. hifitime's own
    // `From<Timestamp> for Epoch` reconstructs through a proper-time (TAI) add and
    // so loses the leap seconds accrued since J2000 (a ~5s error at present) — do
    // NOT use it. Rebuilding the epoch from the SAME UTC-duration base recovers
    // the original Unix second exactly, across leap-second eras.
    let micros = i128::from(tai_j2000_micros);
    let duration_from_j2000 = hifitime::Duration::from_total_nanoseconds(micros * 1000);
    let epoch = hifitime::Epoch::from_utc_duration(
        hifitime::J2000_REF_EPOCH.to_utc_duration() + duration_from_j2000,
    );
    let unix_seconds = epoch.to_unix_seconds();
    if !unix_seconds.is_finite() || unix_seconds < 0.0 {
        return Err(WillowError::ClockUnavailable);
    }
    Ok(unix_seconds.round() as u64)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn system_time_adapter_rejects_pre_epoch_and_i64_overflow() {
        assert_eq!(
            snapshot_from_unix_duration(
                (UNIX_EPOCH - Duration::from_secs(1)).duration_since(UNIX_EPOCH)
            ),
            Err(WillowError::ClockUnavailable)
        );
        assert_eq!(
            snapshot_from_unix_duration(Ok(Duration::from_secs(u64::MAX))),
            Err(WillowError::ClockUnavailable)
        );
    }
}
