//! Direct unit tests for `tai_j2000_micros_from_unix_seconds`.
//!
//! This converter (added with the #76 fix) turns a caller-supplied UTC Unix-seconds
//! wall-clock instant — e.g. a capability expiry — into the TAI/J2000 MICROSECONDS
//! domain that every real Willow entry `Timestamp` lives in. It was previously
//! exercised only indirectly through the `delegate_editor_section` FFI contract.
//! These tests pin the properties the FFI relies on directly:
//!
//!   * it agrees exactly with the live `system_snapshot` path (same hifitime route),
//!   * one Unix second is exactly 1_000_000 micros (the UNIT — the #76 bug was a
//!     seconds/micros mixup),
//!   * it is strictly increasing (so a `now < expires` seconds guard implies a
//!     non-inverted micros range), and
//!   * it fails closed (not panics) on pre-J2000 and out-of-range inputs.

use riot_core::willow::{
    system_snapshot, tai_j2000_micros_from_unix_seconds, unix_seconds_from_tai_j2000_micros,
    WillowError,
};

const MICROS_PER_SEC: u64 = 1_000_000;

#[test]
fn converter_agrees_with_the_live_snapshot_path() {
    // The whole point of the converter: a value it produces for some unix_seconds
    // must equal the tai_j2000_micros that system_snapshot derives for the SAME
    // unix_seconds — otherwise a converted expiry and a real entry timestamp would
    // not be comparable (the #76 failure mode).
    let snap = system_snapshot().expect("clock");
    let converted = tai_j2000_micros_from_unix_seconds(snap.unix_seconds).expect("convert");
    assert_eq!(
        converted, snap.tai_j2000_micros,
        "converter must match system_snapshot's tai_j2000_micros for the same unix seconds"
    );
}

#[test]
fn one_unix_second_is_exactly_one_million_micros() {
    // Pins the UNIT. If the converter ever returned seconds (or millis), this delta
    // would not be 1_000_000. Anchored at a real 'now' so we exercise a realistic
    // magnitude, not just small integers.
    let base = system_snapshot().expect("clock").unix_seconds;
    let a = tai_j2000_micros_from_unix_seconds(base).expect("convert base");
    let b = tai_j2000_micros_from_unix_seconds(base + 1).expect("convert base+1");
    assert_eq!(
        b - a,
        MICROS_PER_SEC,
        "one Unix second must convert to exactly 1_000_000 TAI/J2000 micros"
    );
}

#[test]
fn output_is_in_the_micros_domain_not_seconds() {
    // A contemporary instant in micros is ~8.3e14; the same instant in seconds is
    // ~1.7e9. Assert the result is far above any plausible seconds value — this is
    // the exact scale mismatch that made the #76 cap authorise zero real entries.
    let base = system_snapshot().expect("clock").unix_seconds;
    let micros = tai_j2000_micros_from_unix_seconds(base).expect("convert");
    assert!(
        micros > 1_000_000_000_000,
        "a present-day instant in micros must exceed 1e12 (got {micros}); a seconds value would not"
    );
}

#[test]
fn converter_is_strictly_increasing() {
    // Monotonicity is what lets the FFI keep its expiry guard in seconds: if
    // expires_secs > now_secs then convert(expires) > convert(now), so the micros
    // TimeRange is never inverted.
    let base = system_snapshot().expect("clock").unix_seconds;
    let now = tai_j2000_micros_from_unix_seconds(base).expect("now");
    let later = tai_j2000_micros_from_unix_seconds(base + 3_600).expect("later");
    assert!(
        later > now,
        "convert must be strictly increasing in unix_seconds ({now} !< {later})"
    );
}

#[test]
fn pre_j2000_input_fails_closed() {
    // J2000 (the epoch) is 2000-01-01; Unix second 0 is 1970 — before J2000, so the
    // TAI/J2000 offset is negative and cannot be represented as an unsigned
    // Timestamp. Must be a typed error, never a panic or a wrapped value.
    assert!(
        matches!(
            tai_j2000_micros_from_unix_seconds(0),
            Err(WillowError::ClockUnavailable)
        ),
        "a pre-J2000 (1970) input must fail closed"
    );
}

#[test]
fn out_of_range_input_fails_closed() {
    // u64::MAX seconds cannot fit the internal i64 seconds domain -> typed error,
    // no panic across what is ultimately an FFI-reachable path.
    assert!(
        matches!(
            tai_j2000_micros_from_unix_seconds(u64::MAX),
            Err(WillowError::ClockUnavailable)
        ),
        "an out-of-range input must fail closed"
    );
}

// --- inverse: unix_seconds_from_tai_j2000_micros --------------------------
//
// The display path needs a REAL wall-clock instant for each open-wire post, but
// a projected post carries only its entry `Timestamp` (TAI/J2000 micros). This
// inverse recovers Unix seconds so the app can render "2h ago" without any
// hand-rolled epoch math against the wrong unit.

#[test]
fn inverse_round_trips_forward_at_second_resolution() {
    // The load-bearing property: convert a Unix-seconds instant into the entry
    // micros domain and back, and land on the SAME second. If the two converters
    // disagreed, every rendered post time would be off by the offset error.
    for base in [
        1_000_000_000, // 2001-09, safely after the J2000 reference epoch
        1_700_000_000,
        1_752_000_000,
        2_000_000_000,
    ] {
        let micros = tai_j2000_micros_from_unix_seconds(base).expect("forward");
        let back = unix_seconds_from_tai_j2000_micros(micros).expect("inverse");
        assert_eq!(
            back, base,
            "round trip must preserve the Unix second (base {base}, micros {micros})"
        );
    }
}

#[test]
fn inverse_agrees_with_the_live_snapshot_path() {
    // A real entry timestamp, taken from system_snapshot, must map back to the
    // same Unix seconds the snapshot recorded — otherwise a rendered "created at"
    // would drift from the wall clock that signed the post.
    let snap = system_snapshot().expect("clock");
    let recovered = unix_seconds_from_tai_j2000_micros(snap.tai_j2000_micros).expect("inverse");
    assert_eq!(
        recovered, snap.unix_seconds,
        "inverse must match system_snapshot's unix_seconds for the same entry timestamp"
    );
}

#[test]
fn inverse_output_is_in_the_seconds_domain_not_micros() {
    // Guard the unit at the OTHER end: a present-day recovered value is ~1.7e9
    // seconds, nowhere near the ~8.3e14 micros it came from. If the inverse ever
    // leaked micros, this would blow past any plausible seconds magnitude.
    let snap = system_snapshot().expect("clock");
    let recovered = unix_seconds_from_tai_j2000_micros(snap.tai_j2000_micros).expect("inverse");
    assert!(
        (1_000_000_000..10_000_000_000).contains(&recovered),
        "a present-day instant in seconds must be ~1e9..1e10 (got {recovered}), not micros"
    );
}

#[test]
fn inverse_of_the_j2000_reference_epoch_is_a_valid_positive_second() {
    // Entry-timestamp 0 is the J2000 reference epoch (2000-01-01T12:00 TT), which
    // is AFTER the Unix epoch (1970), so it maps to a positive Unix second — never
    // an error, a panic, or a wrap. Its exact value depends on the TT/TAI/leap
    // offset, so assert it lands in the year-2000 neighbourhood rather than pin a
    // brittle magic number.
    let unix = unix_seconds_from_tai_j2000_micros(0).expect("J2000 is a valid instant");
    assert!(
        (946_000_000..947_500_000).contains(&unix),
        "TAI/J2000 micros 0 is 2000-01-01 noon -> a positive Unix second near 9.467e8 (got {unix})"
    );
}
