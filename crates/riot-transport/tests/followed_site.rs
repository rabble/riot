//! Followed-site delivery over the transport, end to end and deterministic (no
//! network): an owner serves its owned /mod records over a `ByteSyncSession`
//! keyed on the site root; a follower pulls them over an in-memory duplex and
//! admits each bundle through the SINGLE canonical core gate
//! (`riot_core::site::admit_followed_site_frame`). Proves the real admission
//! path — the same gate the manual (Option B) and sync-session (WU2) callers
//! use — commits owner content delivered by the transport.

use riot_core::session::RiotSession;
use riot_core::site::admit_followed_site_frame;
use riot_core::sync::ByteSyncSession;
use riot_core::willow::site_paths::MOD_COMPONENT;
use riot_core::willow::{
    encode_capability, encode_entry, entry_id, Entry, OwnedMasthead, Path, SignedWillowEntry,
};
use riot_transport::iroh::{bind, connect_followed_site, dialable_addr, serve_followed_site};
use riot_transport::pump;
use riot_transport::ticket::{mint, Capabilities, TransportBlocked};
use riot_transport::TransportError;
use tokio::io::split;

/// One owner-signed record at `path` under the masthead's owned cap.
fn owner_sign(
    masthead: &OwnedMasthead,
    path: &[&[u8]],
    ts: u64,
    payload: &[u8],
) -> SignedWillowEntry {
    let entry = Entry::builder()
        .namespace_id(masthead.namespace_id().clone())
        .subspace_id(masthead.owner_subspace_id())
        .path(Path::from_slices(path).expect("path"))
        .timestamp(ts)
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

#[tokio::test]
async fn a_follower_pulls_and_admits_owner_mod_records_over_the_transport() {
    let masthead = OwnedMasthead::generate().expect("masthead");
    let root = *masthead.namespace_id().as_bytes();

    // The owner's followed-site offer: two owned /mod records (what
    // build_followed_site_offer(O) would return on a real device).
    let m1 = owner_sign(&masthead, &[MOD_COMPONENT, b"m1"], 100, b"mod-record-1");
    let m2 = owner_sign(&masthead, &[MOD_COMPONENT, b"m2"], 101, b"mod-record-2");
    let m1_id = entry_id(&m1.entry_bytes);
    let m2_id = entry_id(&m2.entry_bytes);
    let owner = ByteSyncSession::new(root, vec![m1, m2]).expect("owner session");

    // The follower holds nothing for the site and must pull it.
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let follower = ByteSyncSession::new(root, vec![]).expect("follower session");

    let (a, b) = tokio::io::duplex(1 << 16);
    let (mut a_read, mut a_write) = split(a);
    let (mut b_read, mut b_write) = split(b);

    // Owner serves read-mostly (accepts nothing to ingest in v1); the follower
    // admits every delivered bundle through the canonical core gate.
    let serve = pump(owner, &mut a_write, &mut a_read, true, |_bundle| true);
    let admit = pump(follower, &mut b_write, &mut b_read, false, |bundle| {
        admit_followed_site_frame(&store, root, bundle, "site-follow-transport").is_ok()
    });

    let (served, admitted) = tokio::join!(serve, admit);
    let owner = served.expect("owner pump");
    let follower = admitted.expect("follower pump");
    assert!(
        owner.is_terminal() && follower.is_terminal(),
        "both sessions reach Complete"
    );

    // The transport delivered and the core gate committed both owner /mod records.
    let live = store.live_entry_ids().expect("live ids");
    assert!(
        live.contains(&m1_id) && live.contains(&m2_id),
        "both owner /mod records were admitted into the follower store over the transport"
    );
    assert_eq!(live.len(), 2, "exactly the two delivered records are live");
}

#[tokio::test(flavor = "multi_thread")]
async fn connect_followed_site_delivers_and_admits_over_real_iroh() {
    let masthead = OwnedMasthead::generate().expect("masthead");
    let root = *masthead.namespace_id().as_bytes();
    let m1 = owner_sign(&masthead, &[MOD_COMPONENT, b"m1"], 100, b"mod-record-1");
    let m1_id = entry_id(&m1.entry_bytes);

    let seed = bind().await.expect("bind seed");
    let follower = bind().await.expect("bind follower");
    let seed_addr = dialable_addr(&seed).await;

    // The owner serves its /mod offer over a real QUIC endpoint.
    let offer = vec![m1];
    let seed_task = tokio::spawn(async move { serve_followed_site(&seed, root, offer).await });

    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");

    // A valid, signed, floor:none ticket for the site — the fail-closed gate admits it.
    let ticket_key = ed25519_dalek::SigningKey::from_bytes(&[7u8; 32]);
    let ticket = mint(&ticket_key, root, "none", 1, u64::MAX, [0u8; 32], None);
    let caps = Capabilities {
        iroh: true,
        arti: false,
    };

    let follower_session = connect_followed_site(
        &follower,
        seed_addr,
        &ticket,
        &caps,
        1_000,
        0,
        &store,
        root,
        vec![],
    )
    .await
    .expect("follower dial + admit");

    seed_task.await.expect("seed task").expect("seed serve");
    assert!(
        follower_session.is_terminal(),
        "the follower session completes"
    );
    assert!(
        store.live_entry_ids().expect("live").contains(&m1_id),
        "the owner /mod record was delivered and admitted over real iroh"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn connect_followed_site_fails_closed_and_admits_nothing() {
    let masthead = OwnedMasthead::generate().expect("masthead");
    let root = *masthead.namespace_id().as_bytes();

    // A seed exists (a reachable address), but the ticket floor requires arti and
    // the follower has only iroh — the gate must refuse BEFORE dialing.
    let seed = bind().await.expect("bind seed");
    let follower = bind().await.expect("bind follower");
    let seed_addr = dialable_addr(&seed).await;

    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");

    let ticket_key = ed25519_dalek::SigningKey::from_bytes(&[7u8; 32]);
    let ticket = mint(&ticket_key, root, "arti", 1, u64::MAX, [0u8; 32], None);
    let caps = Capabilities {
        iroh: true,
        arti: false,
    };

    let result = connect_followed_site(
        &follower,
        seed_addr,
        &ticket,
        &caps,
        1_000,
        0,
        &store,
        root,
        vec![],
    )
    .await;

    assert!(
        matches!(
            result,
            Err(TransportError::Blocked(
                TransportBlocked::RequiresUnavailableTransport(_)
            ))
        ),
        "a require:arti site refuses an iroh-only follower before dialing"
    );
    assert!(
        store.live_entry_ids().expect("live").is_empty(),
        "a refused dial opens no connection and admits nothing"
    );
}
