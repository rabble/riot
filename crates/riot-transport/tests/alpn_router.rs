//! WU-007: the multi-ALPN router and bounded stream lifecycle.
//!
//! Every case drives the router over a fake [`RouterConnection`] backed by
//! in-memory duplex streams, so the lifecycle FSM (deadlines, permits, single
//! stream, exporter ordering) is exercised deterministically — no real network,
//! no timing flakiness. Time-based cases run under `start_paused` so tokio
//! auto-advances the clock to the next deadline instead of sleeping.

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncWriteExt, DuplexStream};
use tokio::sync::oneshot;

use riot_transport::router::{
    AlpnRouter, BoundedStream, BoxRead, BoxWrite, Deadline, Deadlines, Exporter, Handler,
    RouterConnection,
};
use riot_transport::{TransportError, ALPN, ALPN_ANCHOR_V1, ALPN_SYNC_V2};

// ---------------------------------------------------------------------------
// Fake connection
// ---------------------------------------------------------------------------

/// Deterministic 32-byte channel binding the fake exporter returns. Pins that
/// the exporter surfaces stable bytes to the authenticated handshake.
const PINNED_EXPORTER: [u8; 64] = [0xAB; 64];

struct FakeInner {
    alpn: Option<Vec<u8>>,
    /// Server-side stream halves handed out by `accept_bi` (taken once).
    halves: Mutex<Option<(BoxWrite, BoxRead)>>,
    stall_handshake: bool,
    handshake_done: AtomicBool,
    exporter_calls: AtomicUsize,
    close_calls: AtomicUsize,
    /// Fires `accept_extra` when the test simulates a forbidden extra stream.
    extra: Mutex<Option<oneshot::Receiver<()>>>,
}

#[derive(Clone)]
struct FakeConn {
    inner: Arc<FakeInner>,
}

impl FakeConn {
    fn new(alpn: Option<&[u8]>, halves: Option<(BoxWrite, BoxRead)>) -> Self {
        Self {
            inner: Arc::new(FakeInner {
                alpn: alpn.map(|a| a.to_vec()),
                halves: Mutex::new(halves),
                stall_handshake: false,
                handshake_done: AtomicBool::new(false),
                exporter_calls: AtomicUsize::new(0),
                close_calls: AtomicUsize::new(0),
                extra: Mutex::new(None),
            }),
        }
    }

    fn with_stalled_handshake(alpn: &[u8]) -> Self {
        let c = Self::new(Some(alpn), None);
        // Rebuild inner with stall flag set (fields are otherwise private).
        Self {
            inner: Arc::new(FakeInner {
                alpn: c.inner.alpn.clone(),
                halves: Mutex::new(None),
                stall_handshake: true,
                handshake_done: AtomicBool::new(false),
                exporter_calls: AtomicUsize::new(0),
                close_calls: AtomicUsize::new(0),
                extra: Mutex::new(None),
            }),
        }
    }

    fn arm_extra_stream(&self) -> oneshot::Sender<()> {
        let (tx, rx) = oneshot::channel();
        *self.inner.extra.lock().unwrap() = Some(rx);
        tx
    }
}

impl RouterConnection for FakeConn {
    fn negotiated_alpn(&self) -> Option<Vec<u8>> {
        self.inner.alpn.clone()
    }

    fn export_keying_material(
        &self,
        _label: &[u8],
        _context: &[u8],
        out_len: usize,
    ) -> Result<Vec<u8>, TransportError> {
        // The router MUST build the exporter only after the handshake: the fake
        // fails closed if asked for keying material before `accept_bi` ran.
        assert!(
            self.inner.handshake_done.load(Ordering::SeqCst),
            "exporter derived before handshake completed",
        );
        self.inner.exporter_calls.fetch_add(1, Ordering::SeqCst);
        Ok(PINNED_EXPORTER[..out_len].to_vec())
    }

    fn accept_bi(&self) -> impl Future<Output = Result<(BoxWrite, BoxRead), TransportError>> {
        let inner = Arc::clone(&self.inner);
        async move {
            if inner.stall_handshake {
                std::future::pending::<()>().await;
            }
            inner.handshake_done.store(true, Ordering::SeqCst);
            inner
                .halves
                .lock()
                .unwrap()
                .take()
                .ok_or(TransportError::StreamClosed)
        }
    }

    fn accept_extra(&self) -> impl Future<Output = ()> {
        let rx = self.inner.extra.lock().unwrap().take();
        async move {
            match rx {
                Some(rx) => {
                    let _ = rx.await;
                }
                None => std::future::pending::<()>().await,
            }
        }
    }

    fn close(&self, _reason: &[u8]) {
        self.inner.close_calls.fetch_add(1, Ordering::SeqCst);
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Wire the server-side (router) halves plus the peer-side halves the test
/// drives. Returns `(server_halves, peer_writer, peer_reader)`.
fn wire() -> ((BoxWrite, BoxRead), DuplexStream, DuplexStream) {
    let (peer_to_server, server_recv) = tokio::io::duplex(1 << 16);
    let (server_send, server_from_peer) = tokio::io::duplex(1 << 16);
    let halves: (BoxWrite, BoxRead) = (Box::pin(server_send), Box::pin(server_recv));
    (halves, peer_to_server, server_from_peer)
}

async fn write_frame_raw(w: &mut DuplexStream, body: &[u8]) {
    let len = (body.len() as u32).to_be_bytes();
    w.write_all(&len).await.unwrap();
    w.write_all(body).await.unwrap();
    w.flush().await.unwrap();
}

/// A handler that reads exactly `n` frames and returns Ok. Any lifecycle error
/// propagates out so the test can assert which deadline fired.
fn read_n_frames_handler(n: usize) -> Handler {
    Arc::new(move |mut stream: BoundedStream, _ex: Exporter| {
        Box::pin(async move {
            for _ in 0..n {
                stream.read_frame().await?;
            }
            Ok(())
        }) as Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send>>
    })
}

fn tiny_deadlines() -> Deadlines {
    Deadlines {
        handshake: Duration::from_secs(5),
        first_frame: Duration::from_secs(3),
        progress_interval: Duration::from_secs(2),
        frame: Duration::from_secs(10),
        idle: Duration::from_secs(4),
        absolute: Duration::from_secs(20),
    }
}

// ---------------------------------------------------------------------------
// Routing
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn routes_each_registered_alpn_to_its_handler() {
    for alpn in [ALPN, ALPN_SYNC_V2, ALPN_ANCHOR_V1] {
        let ran = Arc::new(AtomicBool::new(false));
        let ran2 = Arc::clone(&ran);
        let handler: Handler = Arc::new(move |mut stream: BoundedStream, _ex: Exporter| {
            let ran = Arc::clone(&ran2);
            Box::pin(async move {
                // Read the single frame the peer sends, then complete.
                stream.read_frame().await?;
                ran.store(true, Ordering::SeqCst);
                Ok(())
            }) as Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send>>
        });

        let mut router = AlpnRouter::new(4);
        router.register(alpn, tiny_deadlines(), handler);

        let (halves, mut peer_w, _peer_r) = wire();
        let conn = FakeConn::new(Some(alpn), Some(halves));
        write_frame_raw(&mut peer_w, b"hello").await;

        let out = router.dispatch(conn).await;
        assert!(out.is_ok(), "alpn {alpn:?} should route: {out:?}");
        assert!(ran.load(Ordering::SeqCst), "handler for {alpn:?} must run");
    }
}

#[tokio::test(start_paused = true)]
async fn unknown_alpn_closes_without_allocating_a_session() {
    let mut router = AlpnRouter::new(2);
    router.register(ALPN, tiny_deadlines(), read_n_frames_handler(1));

    let (halves, _pw, _pr) = wire();
    let conn = FakeConn::new(Some(b"riot/bogus/9"), Some(halves));

    let out = router.dispatch(conn.clone()).await;
    assert!(matches!(out, Err(TransportError::UnknownAlpn)), "{out:?}");
    // No session: permit untouched, handshake never happened, exporter never derived.
    assert_eq!(router.available_permits(), 2, "no permit may be consumed");
    assert!(!conn.inner.handshake_done.load(Ordering::SeqCst));
    assert_eq!(conn.inner.exporter_calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        conn.inner.close_calls.load(Ordering::SeqCst),
        1,
        "must close"
    );
}

#[tokio::test(start_paused = true)]
async fn no_negotiated_alpn_closes_without_a_session() {
    let mut router = AlpnRouter::new(1);
    router.register(ALPN, tiny_deadlines(), read_n_frames_handler(1));

    let conn = FakeConn::new(None, None);
    let out = router.dispatch(conn.clone()).await;
    assert!(matches!(out, Err(TransportError::UnknownAlpn)), "{out:?}");
    assert_eq!(router.available_permits(), 1);
    assert_eq!(conn.inner.close_calls.load(Ordering::SeqCst), 1);
}

// ---------------------------------------------------------------------------
// Single-stream enforcement
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn second_bidirectional_stream_resets_and_closes() {
    let mut router = AlpnRouter::new(1);
    // Handler blocks forever on a first frame that never arrives; the extra
    // stream must win and terminate the session as a violation.
    router.register(ALPN, tiny_deadlines(), read_n_frames_handler(1));

    let (halves, _pw, _pr) = wire();
    let conn = FakeConn::new(Some(ALPN), Some(halves));
    let trigger = conn.arm_extra_stream();

    // Simulate the peer opening a forbidden second stream after handshake.
    tokio::spawn(async move {
        let _ = trigger.send(());
    });

    let out = router.dispatch(conn.clone()).await;
    assert!(
        matches!(out, Err(TransportError::StreamViolation)),
        "{out:?}"
    );
    assert_eq!(
        router.available_permits(),
        1,
        "permit released on violation"
    );
    assert_eq!(conn.inner.close_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test(start_paused = true)]
async fn unidirectional_stream_is_forbidden() {
    // The fake models any forbidden extra stream (2nd bi or any uni) through the
    // same `accept_extra` trigger, matching the real iroh adapter's select over
    // accept_bi + accept_uni.
    let mut router = AlpnRouter::new(1);
    router.register(
        ALPN_ANCHOR_V1,
        Deadlines::control(),
        read_n_frames_handler(1),
    );

    let (halves, _pw, _pr) = wire();
    let conn = FakeConn::new(Some(ALPN_ANCHOR_V1), Some(halves));
    let trigger = conn.arm_extra_stream();
    tokio::spawn(async move {
        let _ = trigger.send(());
    });

    let out = router.dispatch(conn.clone()).await;
    assert!(
        matches!(out, Err(TransportError::StreamViolation)),
        "{out:?}"
    );
    assert_eq!(router.available_permits(), 1);
}

// ---------------------------------------------------------------------------
// Bounded-lifecycle deadlines
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn handshake_deadline_fires_when_bistream_never_opens() {
    let mut router = AlpnRouter::new(1);
    router.register(ALPN, tiny_deadlines(), read_n_frames_handler(1));

    let conn = FakeConn::with_stalled_handshake(ALPN);
    let out = router.dispatch(conn.clone()).await;
    assert!(
        matches!(out, Err(TransportError::Timeout(Deadline::Handshake))),
        "{out:?}"
    );
    assert_eq!(
        router.available_permits(),
        1,
        "permit released on handshake timeout"
    );
    assert_eq!(conn.inner.exporter_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test(start_paused = true)]
async fn first_frame_deadline_fires_when_no_frame_begins() {
    let mut router = AlpnRouter::new(1);
    router.register(ALPN, tiny_deadlines(), read_n_frames_handler(1));

    let (halves, _pw, _pr) = wire(); // peer never writes
    let conn = FakeConn::new(Some(ALPN), Some(halves));

    let out = router.dispatch(conn).await;
    assert!(
        matches!(out, Err(TransportError::Timeout(Deadline::FirstFrame))),
        "{out:?}"
    );
    assert_eq!(router.available_permits(), 1);
}

#[tokio::test(start_paused = true)]
async fn idle_deadline_fires_between_frames() {
    let mut router = AlpnRouter::new(1);
    router.register(ALPN, tiny_deadlines(), read_n_frames_handler(2));

    let (halves, mut peer_w, _pr) = wire();
    let conn = FakeConn::new(Some(ALPN), Some(halves));
    // One full frame, then silence: the SECOND read stalls on the idle budget.
    write_frame_raw(&mut peer_w, b"first-and-only").await;

    let out = router.dispatch(conn).await;
    assert!(
        matches!(out, Err(TransportError::Timeout(Deadline::Idle))),
        "{out:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn progress_deadline_fires_on_a_mid_frame_trickle() {
    let mut router = AlpnRouter::new(1);
    router.register(ALPN, tiny_deadlines(), read_n_frames_handler(1));

    let (halves, mut peer_w, _pr) = wire();
    let conn = FakeConn::new(Some(ALPN), Some(halves));
    // Two bytes of the length prefix arrive, then the peer stalls mid-frame:
    // the frame has begun, so the progress (trickle) guard must fire.
    peer_w.write_all(&[0x00, 0x00]).await.unwrap();
    peer_w.flush().await.unwrap();

    let out = router.dispatch(conn).await;
    assert!(
        matches!(out, Err(TransportError::Timeout(Deadline::Progress))),
        "{out:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn whole_frame_deadline_fires_when_frame_budget_is_shortest() {
    // frame < progress: after the first byte the remaining whole-frame budget is
    // the binding bound, so a stall reports `Frame`, not `Progress`.
    let deadlines = Deadlines {
        frame: Duration::from_secs(1),
        progress_interval: Duration::from_secs(5),
        ..tiny_deadlines()
    };
    let mut router = AlpnRouter::new(1);
    router.register(ALPN, deadlines, read_n_frames_handler(1));

    let (halves, mut peer_w, _pr) = wire();
    let conn = FakeConn::new(Some(ALPN), Some(halves));
    peer_w.write_all(&[0x00, 0x00]).await.unwrap();
    peer_w.flush().await.unwrap();

    let out = router.dispatch(conn).await;
    assert!(
        matches!(out, Err(TransportError::Timeout(Deadline::Frame))),
        "{out:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn absolute_lifetime_fires_regardless_of_activity() {
    // A handler that never returns; only the absolute lifetime can close it.
    let handler: Handler = Arc::new(move |_stream: BoundedStream, _ex: Exporter| {
        Box::pin(async move {
            std::future::pending::<()>().await;
            Ok(())
        }) as Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send>>
    });
    let mut router = AlpnRouter::new(1);
    router.register(ALPN, tiny_deadlines(), handler);

    let (halves, _pw, _pr) = wire();
    let conn = FakeConn::new(Some(ALPN), Some(halves));

    let out = router.dispatch(conn).await;
    assert!(
        matches!(out, Err(TransportError::Timeout(Deadline::Absolute))),
        "{out:?}"
    );
    assert_eq!(
        router.available_permits(),
        1,
        "permit released on absolute timeout"
    );
}

// ---------------------------------------------------------------------------
// Permits
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn a_cancelled_session_releases_its_permit_for_reuse() {
    let mut router = AlpnRouter::new(1);
    router.register(ALPN, tiny_deadlines(), read_n_frames_handler(1));

    // First session times out on the first frame.
    let (halves, _pw, _pr) = wire();
    let out = router
        .dispatch(FakeConn::new(Some(ALPN), Some(halves)))
        .await;
    assert!(matches!(
        out,
        Err(TransportError::Timeout(Deadline::FirstFrame))
    ));
    assert_eq!(
        router.available_permits(),
        1,
        "permit must return to the pool"
    );

    // The single permit is reusable: a well-behaved session now succeeds.
    let (halves, mut peer_w, _pr) = wire();
    write_frame_raw(&mut peer_w, b"ok").await;
    let out = router
        .dispatch(FakeConn::new(Some(ALPN), Some(halves)))
        .await;
    assert!(
        out.is_ok(),
        "reused permit should admit a new session: {out:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_full_router_refuses_a_new_session_as_busy() {
    // A handler that signals it has started, then blocks until released — so the
    // single permit is genuinely held while a second dispatch is attempted.
    let started_tx = Arc::new(Mutex::new(None::<oneshot::Sender<()>>));
    let release_rx = Arc::new(Mutex::new(None::<oneshot::Receiver<()>>));
    let (s_tx, s_rx) = oneshot::channel();
    let (r_tx, r_rx) = oneshot::channel();
    *started_tx.lock().unwrap() = Some(s_tx);
    *release_rx.lock().unwrap() = Some(r_rx);

    let started_tx2 = Arc::clone(&started_tx);
    let release_rx2 = Arc::clone(&release_rx);
    let handler: Handler = Arc::new(move |_stream: BoundedStream, _ex: Exporter| {
        let started = started_tx2.lock().unwrap().take();
        let release = release_rx2.lock().unwrap().take();
        Box::pin(async move {
            if let Some(tx) = started {
                let _ = tx.send(());
            }
            if let Some(rx) = release {
                let _ = rx.await;
            }
            Ok(())
        }) as Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send>>
    });

    let mut router = AlpnRouter::new(1);
    router.register(ALPN, Deadlines::sync(), handler);
    let router = Arc::new(router);

    let (halves, _pw, _pr) = wire();
    let conn1 = FakeConn::new(Some(ALPN), Some(halves));
    let router1 = Arc::clone(&router);
    let task1 = tokio::spawn(async move { router1.dispatch(conn1).await });

    // Wait until session 1 holds the permit and is running its handler.
    s_rx.await.unwrap();
    assert_eq!(
        router.available_permits(),
        0,
        "session 1 holds the only permit"
    );

    // Session 2 finds no permit: refused as Busy without opening a session.
    let conn2 = FakeConn::new(Some(ALPN), None);
    let out2 = router.dispatch(conn2.clone()).await;
    assert!(matches!(out2, Err(TransportError::Busy)), "{out2:?}");
    assert!(!conn2.inner.handshake_done.load(Ordering::SeqCst));
    assert_eq!(conn2.inner.close_calls.load(Ordering::SeqCst), 1);

    // Release session 1; the permit returns.
    let _ = r_tx.send(());
    let out1 = task1.await.unwrap();
    assert!(out1.is_ok(), "{out1:?}");
    assert_eq!(router.available_permits(), 1);
}

// ---------------------------------------------------------------------------
// Exporter
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn exporter_yields_deterministic_binding_only_after_handshake() {
    let got = Arc::new(Mutex::new(None::<Vec<u8>>));
    let got2 = Arc::clone(&got);
    let handler: Handler = Arc::new(move |mut stream: BoundedStream, ex: Exporter| {
        let got = Arc::clone(&got2);
        Box::pin(async move {
            // The exporter is usable inside the authenticated handshake and
            // returns the pinned channel binding.
            let cb = ex
                .channel_binding(b"EXPORTER-Riot-Anchor-Peer-v1", b"", 32)
                .expect("exporter available post-handshake");
            *got.lock().unwrap() = Some(cb);
            stream.read_frame().await?;
            Ok(())
        }) as Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send>>
    });

    let mut router = AlpnRouter::new(1);
    router.register(ALPN_ANCHOR_V1, Deadlines::control(), handler);

    let (halves, mut peer_w, _pr) = wire();
    let conn = FakeConn::new(Some(ALPN_ANCHOR_V1), Some(halves));
    write_frame_raw(&mut peer_w, b"go").await;

    let out = router.dispatch(conn.clone()).await;
    assert!(out.is_ok(), "{out:?}");
    assert_eq!(
        got.lock().unwrap().as_deref(),
        Some(&PINNED_EXPORTER[..32]),
        "handler saw the pinned 32-byte channel binding",
    );
    assert_eq!(conn.inner.exporter_calls.load(Ordering::SeqCst), 1);
    assert!(conn.inner.handshake_done.load(Ordering::SeqCst));
}
