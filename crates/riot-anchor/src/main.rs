//! riot-anchor — the public community anchor control-plane daemon (WU-019
//! increment 1).
//!
//! Opens the durable `AnchorRepository`, acquires the single-writer deployment
//! lease, binds a public iroh endpoint advertising the `riot/anchor/1` control
//! ALPN, and serves real control round-trips until SIGINT/SIGTERM.
//!
//! Configuration is read entirely from the environment (see
//! `riot_anchor::daemon::DaemonConfig::from_env`) — no CLI flag ever carries
//! key material:
//!
//!   RIOT_ANCHOR_DB                 file path, or `memory` for an in-memory repo
//!   RIOT_ANCHOR_SECRET_KEY_PATH    path to a 32-byte root secret file
//!   RIOT_ANCHOR_LEASE_TTL_SECS     optional, default 300
//!   RIOT_ANCHOR_MAX_SESSIONS       optional, default admission::DEFAULT_MAX_CONCURRENT_SESSIONS
//!
//! Scope: the control plane only (WU-019 increment 1). The `sync/2` data path
//! and the full ingress DoS-hardening tail are not yet served by this binary —
//! see `docs/coordination/2026-07-20-anchor-runnability-gap.md`.

use riot_anchor::daemon::Daemon;

#[tokio::main]
async fn main() {
    let config = match riot_anchor::daemon::DaemonConfig::from_env() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("riot-anchor: configuration error: {error}");
            std::process::exit(1);
        }
    };

    let (daemon, mut readiness) = match Daemon::start(config).await {
        Ok(started) => started,
        Err(error) => {
            eprintln!("riot-anchor: failed to start: {error}");
            std::process::exit(1);
        }
    };

    // Diagnostics: node id and anchor id are public identifiers, never secret.
    eprintln!("riot-anchor up");
    eprintln!("  node id   : {}", hex(&daemon.node_id()));
    eprintln!("  anchor id : {}", hex(&daemon.anchor_id()));

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    tokio::spawn(async move {
        wait_for_shutdown_signal().await;
        eprintln!("riot-anchor: shutdown signal received, closing");
        let _ = shutdown_tx.send(true);
    });

    let run_handle = tokio::spawn(daemon.run(shutdown_rx));

    if readiness.changed().await.is_ok() && *readiness.borrow() {
        eprintln!("riot-anchor: serving riot/anchor/1");
    }

    match run_handle.await {
        Ok(Ok(())) => eprintln!("riot-anchor: stopped"),
        Ok(Err(error)) => {
            eprintln!("riot-anchor: stopped with error: {error}");
            std::process::exit(1);
        }
        Err(_) => {
            eprintln!("riot-anchor: daemon task panicked");
            std::process::exit(1);
        }
    }
}

#[cfg(unix)]
async fn wait_for_shutdown_signal() {
    use tokio::signal::unix::{signal, SignalKind};
    let mut sigterm = signal(SignalKind::terminate()).expect("install SIGTERM handler");
    let mut sigint = signal(SignalKind::interrupt()).expect("install SIGINT handler");
    tokio::select! {
        _ = sigterm.recv() => {}
        _ = sigint.recv() => {}
    }
}

#[cfg(not(unix))]
async fn wait_for_shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
