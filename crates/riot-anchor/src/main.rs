//! The `riot-anchor` control-plane daemon binary (WU-019 increment 1).
//!
//! A thin shell: read argv + environment, resolve the configuration
//! ([`riot_anchor::config`]), then run the daemon ([`riot_anchor::daemon::run`])
//! — which loads the database-durable secrets, assembles the control service
//! from the persisted values, and serves until SIGINT. All testable logic lives
//! in the library; see `riot_anchor::config` and `riot_anchor::daemon`.

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

    let shutdown = async {
        // Serve until SIGINT (Ctrl-C); on any signal error, run indefinitely.
        let _ = tokio::signal::ctrl_c().await;
    };

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
