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

use riot_core::willow::{system_snapshot, tai_j2000_micros_from_unix_seconds, WillowError};

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
