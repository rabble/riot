use std::process::Command;

use riot_transport::iroh::{bind, serve_followed_site};
use riot_transport::ticket::mint;

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_riot-follow"))
        .args(args)
        .output()
        .expect("run riot-follow")
}

#[test]
fn missing_ticket_prints_usage_and_exits_two() {
    let output = run(&[]);

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("usage: riot-follow"));
}

#[test]
fn malformed_ticket_is_rejected_without_dialing() {
    let output = run(&["not-a-ticket"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("bad ticket"));
}

#[test]
fn expired_signed_ticket_is_refused_before_dialing() {
    let key = ed25519_dalek::SigningKey::from_bytes(&[7u8; 32]);
    let ticket = mint(
        &key,
        [0x11; 32],
        "none",
        1,
        100,
        [0x22; 32],
        Some("attacker-controlled-node-hint".into()),
        None,
        None,
    );

    let output = run(&[&ticket.encode()]);

    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("dial refused (fail-closed): ticket expired"));
}

#[tokio::test(flavor = "multi_thread")]
async fn valid_iroh_ticket_syncs_with_a_real_seed() {
    let namespace = [0x11; 32];
    let seed = bind().await.expect("bind seed");
    let node_id = seed
        .id()
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let bound = seed
        .bound_sockets()
        .into_iter()
        .next()
        .expect("seed has a bound UDP socket");
    let loopback = if bound.is_ipv4() {
        format!("127.0.0.1:{}", bound.port())
    } else {
        format!("[::1]:{}", bound.port())
    };
    let node_hint = format!("{node_id}@{loopback}");
    let seed_task =
        tokio::spawn(async move { serve_followed_site(&seed, namespace, vec![]).await });

    let key = ed25519_dalek::SigningKey::from_bytes(&[7u8; 32]);
    let ticket = mint(
        &key,
        namespace,
        "none",
        1,
        u64::MAX,
        [0x22; 32],
        Some(node_hint),
        None,
        None,
    )
    .encode();

    let cli_task = async move {
        tokio::process::Command::new(env!("CARGO_BIN_EXE_riot-follow"))
            .arg(ticket)
            .kill_on_drop(true)
            .output()
            .await
            .expect("run riot-follow")
    };
    let timeout = std::time::Duration::from_secs(30);
    let (served, output) = tokio::join!(
        tokio::time::timeout(timeout, seed_task),
        tokio::time::timeout(timeout, cli_task),
    );

    let output = output.expect("riot-follow CLI timed out");
    assert!(
        output.status.success(),
        "riot-follow failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    served
        .expect("seed timed out")
        .expect("seed task panicked")
        .expect("seed failed to serve");
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "synced — 0 entries now live in the local store\n"
    );
}
