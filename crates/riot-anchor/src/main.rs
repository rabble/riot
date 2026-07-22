//! The `riot-anchor` control-plane daemon binary (WU-019 increment 1).
//!
//! A thin shell: read argv + environment, resolve the configuration
//! ([`riot_anchor::config`]), then run the daemon ([`riot_anchor::daemon::run`])
//! — which loads the database-durable secrets, assembles the control service
//! from the persisted values, and serves until a termination signal. All
//! testable logic lives in the library; see `riot_anchor::config` and
//! `riot_anchor::daemon`.
//!
//! Shutdown is driven by SIGINT **and** SIGTERM (unix). SIGTERM is what
//! `docker compose stop`/`restart` and most init systems send; catching it is
//! what lets the graceful path run (relinquish the deployment lease in place so
//! an immediate restart is not refused `LeaseHeld`, then close the SQLite
//! writer). Without the SIGTERM arm, exec-form PID 1 discards SIGTERM and Docker
//! escalates to SIGKILL (exit 137) — the graceful path never runs.

use std::process::ExitCode;

use riot_anchor::config::resolve_config;
use riot_anchor::daemon;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let env: Vec<(String, String)> = std::env::vars().collect();

    let config = match resolve_config(&args, &env) {
        Ok(config) => config,
        Err(message) => {
            eprintln!("riot-anchor: {message}");
            eprintln!(
                "usage: riot-anchor --db <path>  \
                 (operator key via RIOT_ANCHOR_OPERATOR_KEY_HEX or _FILE)"
            );
            return ExitCode::from(2);
        }
    };
    if let Some(warning) = config.endpoint_identity_warning() {
        eprintln!("riot-anchor: {warning}");
    }

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("riot-anchor: failed to start tokio runtime: {error}");
            return ExitCode::FAILURE;
        }
    };

    // Serve until SIGINT (Ctrl-C) or SIGTERM (`docker compose stop`, init
    // systems); whichever arrives first resolves the shutdown future. On any
    // signal-install error, fall back to SIGINT only (run indefinitely if even
    // that fails).
    let shutdown = shutdown_signal();

    // `run` loads the database-durable secrets and assembles the service from
    // the persisted values before binding the public endpoint.
    match runtime.block_on(daemon::run(config, daemon::os_entropy(), shutdown)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("riot-anchor: {error}");
            ExitCode::FAILURE
        }
    }
}

/// A shutdown future that resolves on the first termination signal.
///
/// On unix this is SIGINT **or** SIGTERM — SIGTERM is what `docker compose
/// stop`, `docker compose restart`, and init systems send, and catching it is
/// what lets the daemon reach its graceful lease-relinquish + clean-close path
/// instead of being SIGKILLed (exit 137). If the SIGTERM handler cannot be
/// installed we fall back to SIGINT alone; if even that fails the future never
/// resolves and the daemon serves until the process is killed. Must be awaited
/// inside a tokio runtime (the caller drives it via `block_on`).
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::terminate()) {
            Ok(mut sigterm) => {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = sigterm.recv() => {}
                }
            }
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
