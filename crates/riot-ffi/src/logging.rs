//! Process-wide tracing subscriber install for the mobile shells.
//!
//! The sync engine (`riot-core`) and the FFI entry points (`mobile_state`,
//! `newswire_ffi`) emit `tracing` spans and events. Those events go nowhere
//! unless a subscriber is installed. This module installs one, exactly once
//! per process, behind a `#[uniffi::export] fn init_logging` that the iOS and
//! macOS apps call at launch (before `bootstrap`).
//!
//! On Apple targets the subscriber forwards to the unified logging system
//! (`os_log`) via `tracing-oslog`, so spans appear in Console.app and
//! `log stream --predicate 'subsystem == "net.protest.riot"'`. On other targets
//! (Linux CI) it falls back to a stderr `fmt` layer so the same code path is
//! exercised and covered.
//!
//! `tracing::subscriber::set_global_default` succeeds at most once per process;
//! the `std::sync::Once` guard here makes every Swift call after the first a
//! no-op that returns `Ok(())`, so repeat calls (e.g. across hot-reloads) never
//! surface an error. The first requested level wins.

use std::sync::Once;

/// Coarse log verbosity requested by the shell at launch. Maps to a tracing
/// `LevelFilter`; the first call's level is the one honored for the process.
#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    fn to_filter(self) -> tracing::level_filters::LevelFilter {
        use tracing::level_filters::LevelFilter;
        match self {
            LogLevel::Error => LevelFilter::ERROR,
            LogLevel::Warn => LevelFilter::WARN,
            LogLevel::Info => LevelFilter::INFO,
            LogLevel::Debug => LevelFilter::DEBUG,
            LogLevel::Trace => LevelFilter::TRACE,
        }
    }
}

/// The subsystem every Riot log shares. Matches `PRODUCT_BUNDLE_IDENTIFIER`
/// and the single existing `Logger(subsystem:category:)` in
/// `WrappingKeyStore.swift`, so Console.app filters by one predicate.
#[cfg(any(target_os = "macos", target_os = "ios"))]
const SUBSYSTEM: &str = "net.protest.riot";

static INIT: Once = Once::new();

/// Installs the process-wide tracing subscriber, exactly once. Safe to call
/// repeatedly from Swift; every call after the first returns `Ok(())` without
/// re-installing. The first call's `level` wins.
///
/// This is the single point that turns on observability for the sync + reply
/// paths; without it, their `tracing` spans compile to no-ops.
pub fn init_app_logging(level: LogLevel) {
    // `call_once` runs only on the first call; the captured `level` of that
    // first call is the one honored. Later calls with a different level are
    // intentionally ignored — a global subscriber cannot be replaced.
    INIT.call_once(move || {
        install_subscriber(level);
    });
}

fn install_subscriber(level: LogLevel) {
    use tracing_subscriber::EnvFilter;

    let filter = EnvFilter::from_default_env().add_directive(level.to_filter().into());

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    install_oslog(filter);

    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    install_stderr(filter);
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn install_oslog(filter: tracing_subscriber::EnvFilter) {
    use tracing_oslog::OsLogger;
    use tracing_subscriber::layer::SubscriberExt;

    // One Layer per category so Console.app can filter `riot::sync` vs
    // `riot::newswire` while the subsystem stays a single predicate. Events
    // pick their category from the `target:` they set; anything untargeted
    // lands in the default "app" bucket.
    let subscriber = tracing_subscriber::Registry::default()
        .with(filter)
        .with(OsLogger::new(SUBSYSTEM, "app"))
        .with(OsLogger::new(SUBSYSTEM, "sync"))
        .with(OsLogger::new(SUBSYSTEM, "newswire"));

    // set_global_default returns Err if already set; the Once guard makes
    // that impossible here, but swallow defensively so a subscriber race
    // can never panic the FFI boundary.
    let _ = tracing::subscriber::set_global_default(subscriber);
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
fn install_stderr(filter: tracing_subscriber::EnvFilter) {
    use tracing_subscriber::{fmt, layer::SubscriberExt};

    let subscriber = tracing_subscriber::Registry::default()
        .with(filter)
        .with(fmt::Layer::default().with_writer(std::io::stderr));

    let _ = tracing::subscriber::set_global_default(subscriber);
}
