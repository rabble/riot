//! Cross-city demo — LOCAL-loopback anchor daemon.
//!
//! The real `riot-anchor` binary binds a PUBLIC endpoint (relay + discovery)
//! and is what an operator deploys (`deploy/riot-anchor/`). This example runs
//! the SAME daemon path — persisted secrets, deployment lease, descriptor
//! persistence, control + sync/2 serving — over a LOCAL endpoint (direct only,
//! no relay) and prints its dialable address, so `scripts/anchor/
//! demo-cross-city.sh --local` can run the whole host→push→commit→pull demo on
//! one machine without discovery infrastructure.
//!
//! Configuration surface is identical to the production daemon (the authority
//! is `riot_anchor::config`): `--db <path>` plus the `RIOT_ANCHOR_*`
//! environment variables.
//!
//! ```sh
//! RIOT_ANCHOR_OPERATOR_KEY_HEX=$(openssl rand -hex 32) \
//! cargo run -p riot-anchor --features daemon --example demo_anchor -- --db /tmp/demo-anchor.sqlite3
//! ```

mod demo_common;

use std::process::ExitCode;

use riot_anchor::config::{finalize_service, resolve_config, secret_proposals};
use riot_anchor::daemon::{
    bind_local_anchor_endpoint, load_or_initialize_secrets, os_entropy, serve,
};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("demo_anchor: {message}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let env: Vec<(String, String)> = std::env::vars().collect();

    let config = resolve_config(&args, &env).map_err(|message| {
        format!("{message}\nusage: demo_anchor --db <path>  (operator key via RIOT_ANCHOR_OPERATOR_KEY_HEX or _FILE)")
    })?;
    if let Some(warning) = config.endpoint_identity_warning() {
        eprintln!("demo_anchor: {warning}");
    }

    // The same startup order as `daemon::run`: load the database-durable
    // secrets FIRST, assemble the service from the persisted values, then bind
    // — only the endpoint preset differs (local direct-only vs public).
    let proposals = secret_proposals(&config);
    let persisted = load_or_initialize_secrets(config.db_path(), &proposals)
        .map_err(|error| error.to_string())?;
    let (daemon_config, service) = finalize_service(config, persisted);

    let endpoint = bind_local_anchor_endpoint(daemon_config.endpoint_secret_key)
        .await
        .map_err(|error| error.to_string())?;

    // Wait until at least one direct (IP) address is discovered, then print
    // the machine-parseable lines the demo script consumes.
    let _ = riot_transport::iroh::dialable_addr(&endpoint).await;
    let node_id = riot_transport::iroh::node_id(&endpoint);
    println!("ANCHOR_NODE_ID={}", demo_common::to_hex(&node_id));
    println!(
        "ANCHOR_ADDR={}",
        riot_transport::iroh::endpoint_addr_hint(&endpoint)
    );
    println!("demo_anchor: serving riot/anchor/1 + riot/sync/2 (Ctrl-C / SIGTERM to stop)");

    // Stop on SIGINT (Ctrl-C) OR SIGTERM. The demo teardown (`scripts/anchor/
    // demo-cross-city.sh` and the `kill <pid>` an operator would type) sends
    // SIGTERM; catching it means the loopback anchor reaches the same graceful
    // lease-relinquish path as production and never leaves a standing lease.
    serve(
        endpoint,
        daemon_config,
        service,
        os_entropy(),
        shutdown_signal(),
    )
    .await
    .map_err(|error| error.to_string())
}

/// SIGINT-or-SIGTERM shutdown future (mirrors the production `riot-anchor`
/// binary). SIGTERM is what the demo script's `kill <pid>` teardown sends;
/// catching it lets the loopback daemon relinquish its lease cleanly. Must be
/// awaited inside a tokio runtime.
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
