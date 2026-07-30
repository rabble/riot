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
///
/// Apple-only: the sole reference is `OsLogger::new` in the Apple branch of
/// `build_dispatch`. Without the gate this is dead code on Linux, and CI runs
/// clippy with `-D warnings`.
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
        // set_global_default returns Err if already set; the Once guard makes
        // that impossible here, but swallow defensively so a subscriber race
        // can never panic the FFI boundary.
        let _ = tracing::dispatcher::set_global_default(build_dispatch(level));
    });
}

/// Builds the process subscriber. Extracted from `init_app_logging` so tests
/// can drive the EXACT shipped layer shape (install path uses a
/// `Once`-guarded global, untestable twice in one process).
fn build_dispatch(level: LogLevel) -> tracing::Dispatch {
    use tracing_subscriber::EnvFilter;

    let filter = EnvFilter::from_default_env().add_directive(level.to_filter().into());

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        apple_dispatch(filter)
    }

    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    {
        stderr_dispatch(filter)
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn apple_dispatch(filter: tracing_subscriber::EnvFilter) -> tracing::Dispatch {
    use tracing_oslog::OsLogger;
    use tracing_subscriber::layer::SubscriberExt;

    // EXACTLY ONE OsLogger layer. tracing-oslog stores a single `Activity` in
    // each span's extensions on `on_new_span` (shared across layers), but
    // EVERY layer's `on_close` removes it with `.expect("span didn't contain
    // activity wtf")` — with two or more OsLogger layers the second layer's
    // remove finds nothing and panics, and the span-close-in-destructor
    // during unwinding panics again → `panic in a destructor during cleanup`
    // → SIGABRT. That was the TestFlight 0.1.1 (1020) launch crash. One layer
    // means one insert, one remove. All events land in the "app" category;
    // Console.app still filters by the single subsystem predicate.
    let subscriber = tracing_subscriber::Registry::default()
        .with(filter)
        .with(OsLogger::new(SUBSYSTEM, "app"));

    tracing::Dispatch::new(subscriber)
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
fn stderr_dispatch(filter: tracing_subscriber::EnvFilter) -> tracing::Dispatch {
    use tracing_subscriber::{fmt, layer::SubscriberExt};

    let subscriber = tracing_subscriber::Registry::default()
        .with(filter)
        .with(fmt::Layer::default().with_writer(std::io::stderr));

    tracing::Dispatch::new(subscriber)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: TestFlight 0.1.1 (1020) crashed on launch with
    /// `span didn't contain activity wtf` → `panic in a destructor during
    /// cleanup` → SIGABRT the moment the launch relay sync closed its first
    /// span. The shipped subscriber MUST survive the full span lifecycle —
    /// new span, enter, event, close — without panicking.
    #[test]
    fn subscriber_survives_span_lifecycle_without_panicking() {
        let dispatch = build_dispatch(LogLevel::Info);
        tracing::dispatcher::with_default(&dispatch, || {
            let span = tracing::info_span!("riot_test_span");
            let entered = span.enter();
            tracing::info!("riot test event inside span");
            drop(entered);
            drop(span); // close: the crash site
        });
    }

    /// The nested-span path the crash actually took: a CHILD span closes
    /// while its parent is still open (iroh's instrumented actor tasks nest
    /// spans under the sync drive span).
    #[test]
    fn subscriber_survives_nested_span_close_without_panicking() {
        let dispatch = build_dispatch(LogLevel::Info);
        tracing::dispatcher::with_default(&dispatch, || {
            let parent = tracing::info_span!("riot_test_parent");
            parent.in_scope(|| {
                let child = tracing::info_span!("riot_test_child");
                child.in_scope(|| {
                    tracing::info!("event inside child");
                });
                drop(child); // child closes first, parent still open
                tracing::info!("event in parent after child closed");
            });
            drop(parent);
        });
    }
}
