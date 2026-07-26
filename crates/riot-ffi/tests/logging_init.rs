//! Logging init contract: `init_logging` installs the process-wide tracing
//! subscriber exactly once and must be safe to call repeatedly from Swift (hot
//! reloads, re-entrant launches) without surfacing an error or panicking. This
//! pins the contract before the shells rely on it at every launch.
//!
//! `set_global_default` succeeds at most once per process; the `Once` guard in
//! `init_app_logging` turns every subsequent call into a no-op. This test
//! drives that guard directly (the UniFFI export is a one-line delegation).

use riot_ffi::{init_app_logging, LogLevel};

#[test]
fn init_logging_is_idempotent_across_repeated_calls() {
    // A shell calls this at every launch; the second and third calls must be
    // no-ops, never errors, never panics. We assert on behavior, not on the
    // internal Once state.
    init_app_logging(LogLevel::Info);
    init_app_logging(LogLevel::Debug);
    init_app_logging(LogLevel::Trace);

    // If we reached here, repeated installs did not panic across distinct
    // levels. Emit one event so the now-installed subscriber is exercised on
    // at least one code path (stderr fallback on non-Apple, os_log on Apple).
    tracing::info!(target: "riot::sync", "logging init contract exercised");
}

#[test]
fn every_log_level_variant_compiles_and_installs() {
    // Pins that the LogLevel enum stays exhaustive over the five tracing
    // levels — a regression here would break the Swift call site that picks a
    // level at launch.
    for level in [
        LogLevel::Error,
        LogLevel::Warn,
        LogLevel::Info,
        LogLevel::Debug,
        LogLevel::Trace,
    ] {
        init_app_logging(level);
    }
}
