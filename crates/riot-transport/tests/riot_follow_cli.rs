use std::process::Command;

use riot_core::willow::site_paths::MOD_COMPONENT;
use riot_core::willow::{
    encode_capability, encode_entry, Entry, OwnedMasthead, Path, SignedWillowEntry,
};
use riot_transport::iroh::{bind, serve_followed_site};
use riot_transport::ticket::mint;

fn owner_sign(
    masthead: &OwnedMasthead,
    path: &[&[u8]],
    timestamp: u64,
    payload: &[u8],
) -> SignedWillowEntry {
    let entry = Entry::builder()
        .namespace_id(masthead.namespace_id().clone())
        .subspace_id(masthead.owner_subspace_id())
        .path(Path::from_slices(path).expect("path"))
        .timestamp(timestamp)
        .payload(payload)
        .build();
    let authorised = masthead
        .authorise_owner_entry(entry)
        .expect("owner authorises");
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    SignedWillowEntry {
        entry_bytes: encode_entry(authorised.entry()),
        capability_bytes: encode_capability(token.capability()),
        signature: signature.to_bytes(),
        payload_bytes: payload.to_vec(),
    }
}

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
    let masthead = OwnedMasthead::generate().expect("masthead");
    let namespace = *masthead.namespace_id().as_bytes();
    let owner_record = owner_sign(
        &masthead,
        &[MOD_COMPONENT, b"cli-test"],
        100,
        b"owner moderation record",
    );
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
        tokio::spawn(
            async move { serve_followed_site(&seed, namespace, vec![owner_record]).await },
        );

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
    let output = tokio::time::timeout(timeout, cli_task)
        .await
        .expect("riot-follow CLI timed out");
    assert!(
        output.status.success(),
        "riot-follow failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "synced — 1 entries now live in the local store\n"
    );

    // The successful CLI result proves the seed served the signed entry. Iroh
    // endpoint shutdown can outlive the child process under instrumentation, so
    // bound cleanup separately instead of turning delayed teardown into a
    // protocol failure.
    let mut seed_task = seed_task;
    match tokio::time::timeout(std::time::Duration::from_secs(2), &mut seed_task).await {
        Ok(served) => {
            served
                .expect("seed task panicked")
                .expect("seed failed to serve");
        }
        Err(_) => {
            seed_task.abort();
            match seed_task.await {
                Ok(Ok(_)) => {}
                Ok(Err(error)) => panic!("seed failed while being cancelled: {error}"),
                Err(error) if error.is_cancelled() => {}
                Err(error) => panic!("seed task failed while being cancelled: {error}"),
            }
        }
    }
}
