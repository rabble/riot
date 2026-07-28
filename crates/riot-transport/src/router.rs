//! The multi-ALPN iroh router and bounded stream lifecycle (design "Transport
//! Roles" + "Public iroh control/sync admission").
//!
//! One iroh [`Endpoint`](iroh::Endpoint) routes several application protocols
//! (`riot/sync/1`, `riot/sync/2`, `riot/anchor/1`). The accept loop reads the
//! negotiated ALPN and dispatches to the matching handler. Everything a stalled
//! or abusive peer could hold is bounded here, NOT in the handlers:
//!
//! - exactly one session permit per connection, released the instant the session
//!   ends for ANY reason (completion, timeout, stream violation, cancellation);
//! - exactly one bidirectional application stream — a second bi-stream or any
//!   unidirectional stream resets and closes the connection with no extra work;
//! - a full ladder of deadlines: handshake, first-frame, per-frame progress
//!   (trickle guard), whole-frame read/write, idle-between-frames, and an
//!   absolute session lifetime that fires regardless of activity.
//!
//! The router is transport-agnostic through [`RouterConnection`]: the real iroh
//! [`Connection`](iroh::endpoint::Connection) implements it (see
//! [`crate::iroh`]); tests implement it over in-memory duplex streams so the
//! whole lifecycle FSM is exercised deterministically without a real network.
//!
//! The live TLS exporter (channel binding) is surfaced to a handler ONLY through
//! the [`Exporter`] capability, which the router constructs AFTER the QUIC
//! handshake completes and the single bi-stream opens — so exporter bytes never
//! exist before the peer is authenticated.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::Semaphore;
use tokio::time::{timeout, Instant};

use riot_core::sync::MAX_SYNC_FRAME_BYTES;

use crate::TransportError;

/// The owned read half handed to a handler. Type-erased so the router and its
/// handler map are independent of the concrete transport.
pub type BoxRead = Pin<Box<dyn AsyncRead + Send>>;

/// The owned write half handed to a handler.
pub type BoxWrite = Pin<Box<dyn AsyncWrite + Send>>;

/// Which bounded-lifecycle deadline elapsed. Carried by
/// [`TransportError::Timeout`] so a caller (or test) can assert exactly which
/// bound fired rather than conflating them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deadline {
    /// The QUIC handshake / first bi-stream did not open in time.
    Handshake,
    /// The first application frame did not begin arriving in time.
    FirstFrame,
    /// A frame started but stalled between bytes (trickle guard).
    Progress,
    /// A frame started but did not finish within its whole-frame budget.
    Frame,
    /// No frame was in progress and the next one did not begin in time.
    Idle,
    /// The absolute session lifetime elapsed regardless of activity.
    Absolute,
}

/// The six bounded-lifecycle deadlines for one routed protocol session. Fields
/// are public so callers can pin the exact profile a signed transport floor
/// requires; the presets below are the design's default ("Resource ceilings")
/// column.
#[derive(Debug, Clone, Copy)]
pub struct Deadlines {
    /// Wall time to complete the QUIC handshake and open the one bi-stream.
    pub handshake: Duration,
    /// Wall time for the first application frame to begin arriving.
    pub first_frame: Duration,
    /// Maximum gap between consecutive bytes of a frame in progress.
    pub progress_interval: Duration,
    /// Maximum wall time for one whole frame (read or write); one-byte trickles
    /// do not extend it.
    pub frame: Duration,
    /// Maximum idle wall time while no frame is in progress.
    pub idle: Duration,
    /// Absolute session lifetime; closes the session regardless of activity.
    pub absolute: Duration,
}

impl Deadlines {
    /// Control-plane (`riot/anchor/1`) default profile: 10 s frame read/write.
    pub const fn control() -> Self {
        Self {
            handshake: Duration::from_secs(5),
            first_frame: Duration::from_secs(5),
            progress_interval: Duration::from_secs(5),
            frame: Duration::from_secs(10),
            idle: Duration::from_secs(30),
            absolute: Duration::from_secs(15 * 60),
        }
    }

    /// Sync (`riot/sync/1`, `riot/sync/2`) default profile: 30 s frame
    /// read/write (a snapshot page may be large).
    pub const fn sync() -> Self {
        Self {
            frame: Duration::from_secs(30),
            ..Self::control()
        }
    }
}

/// The closure a live connection provides to derive exporter keying material
/// (`label, context, out_len -> bytes`).
type ExporterFn = dyn Fn(&[u8], &[u8], usize) -> Result<Vec<u8>, TransportError> + Send + Sync;

/// A post-handshake capability to compute the live QUIC TLS exporter (channel
/// binding, RFC 5705). The router constructs it ONLY after the handshake
/// completes and the single bi-stream opens, so a handler cannot obtain
/// exporter bytes before the peer is authenticated. `riot/anchor/1` uses it for
/// the peer-proof transcript; sync handlers ignore it.
#[derive(Clone)]
pub struct Exporter(Arc<ExporterFn>);

impl Exporter {
    /// Derive `out_len` bytes of exporter keying material under `label` and
    /// `context` from the live connection. The context is passed through exactly
    /// (a present-empty `&[]` is NOT the same as an omitted-context API on the
    /// wire, per the design's exporter fixtures).
    pub fn channel_binding(
        &self,
        label: &[u8],
        context: &[u8],
        out_len: usize,
    ) -> Result<Vec<u8>, TransportError> {
        (self.0)(label, context, out_len)
    }
}

impl std::fmt::Debug for Exporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Exporter(..)")
    }
}

/// A length-delimited application stream whose every read and write is bounded
/// by the session [`Deadlines`]. Handlers speak frames through this type and can
/// hold NO buffer, permit, or snapshot beyond the published bounds: a stalled or
/// trickling peer is cut off deterministically.
pub struct BoundedStream {
    send: BoxWrite,
    recv: BoxRead,
    deadlines: Deadlines,
    max_frame_bytes: usize,
    /// True once any complete frame has been read (distinguishes the very first
    /// frame's `first_frame` budget from later frames' `idle` budget).
    seen_first_frame: bool,
    /// True while bytes of the current frame are still being read (distinguishes
    /// the idle gap before a frame from progress within it).
    frame_in_progress: bool,
    /// Deadline for the current frame, anchored at its first byte.
    frame_deadline: Option<Instant>,
}

impl BoundedStream {
    fn new(send: BoxWrite, recv: BoxRead, deadlines: Deadlines, max_frame_bytes: usize) -> Self {
        Self {
            send,
            recv,
            deadlines,
            max_frame_bytes,
            seen_first_frame: false,
            frame_in_progress: false,
            frame_deadline: None,
        }
    }

    /// Consume the bounded stream, yielding its raw type-erased halves. The
    /// `riot/sync/1` compatibility handler uses this to drive the legacy
    /// [`pump`](crate::pump) — whose own length-prefix framing must not be
    /// double-wrapped — while still running inside the router's permit,
    /// single-stream, handshake, and absolute-lifetime envelope.
    pub fn into_halves(self) -> (BoxWrite, BoxRead) {
        (self.send, self.recv)
    }

    /// Read one length-prefixed frame (`u32be(len) || body`) under the full
    /// lifecycle ladder. Errors carry the precise [`Deadline`] that fired.
    pub async fn read_frame(&mut self) -> Result<Vec<u8>, TransportError> {
        let mut len_bytes = [0u8; 4];
        self.fill(&mut len_bytes).await?;
        let len = u32::from_be_bytes(len_bytes) as usize;
        if len > self.max_frame_bytes {
            return Err(TransportError::FrameTooLarge);
        }
        let mut body = vec![0u8; len];
        self.fill(&mut body).await?;
        // Frame complete: the next frame starts fresh under the idle budget.
        self.frame_in_progress = false;
        self.frame_deadline = None;
        self.seen_first_frame = true;
        Ok(body)
    }

    /// Fill `buf` completely, applying the idle/first-frame gate to the first
    /// byte and the progress/whole-frame bounds to the rest.
    async fn fill(&mut self, buf: &mut [u8]) -> Result<(), TransportError> {
        let mut filled = 0;
        while filled < buf.len() {
            let (budget, kind) = self.next_read_budget()?;
            let n = match timeout(budget, self.recv.read(&mut buf[filled..])).await {
                Err(_elapsed) => return Err(TransportError::Timeout(kind)),
                Ok(Ok(0)) => return Err(TransportError::StreamClosed),
                Ok(Ok(n)) => n,
                Ok(Err(e)) => return Err(e.into()),
            };
            if !self.frame_in_progress {
                // First byte of this frame arrived: start its whole-frame clock.
                self.frame_in_progress = true;
                self.frame_deadline = Some(Instant::now() + self.deadlines.frame);
            }
            filled += n;
        }
        Ok(())
    }

    /// Compute the timeout for the next underlying read and the [`Deadline`] it
    /// would report. Before a frame: idle (or first-frame). During a frame: the
    /// smaller of the progress interval and the remaining whole-frame budget,
    /// labelled accordingly so a trickle reports `Progress` and an over-long
    /// frame reports `Frame`.
    fn next_read_budget(&self) -> Result<(Duration, Deadline), TransportError> {
        if !self.frame_in_progress {
            return Ok(if self.seen_first_frame {
                (self.deadlines.idle, Deadline::Idle)
            } else {
                (self.deadlines.first_frame, Deadline::FirstFrame)
            });
        }
        let deadline = self
            .frame_deadline
            .expect("frame in progress has a deadline");
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(TransportError::Timeout(Deadline::Frame));
        }
        if remaining <= self.deadlines.progress_interval {
            Ok((remaining, Deadline::Frame))
        } else {
            Ok((self.deadlines.progress_interval, Deadline::Progress))
        }
    }

    /// Write one length-prefixed frame under the whole-frame write deadline.
    pub async fn write_frame(&mut self, frame: &[u8]) -> Result<(), TransportError> {
        let len = u32::try_from(frame.len()).map_err(|_| TransportError::FrameTooLarge)?;
        let deadline = self.deadlines.frame;
        let send = &mut self.send;
        let write = async move {
            send.write_all(&len.to_be_bytes()).await?;
            send.write_all(frame).await?;
            send.flush().await?;
            Ok::<(), TransportError>(())
        };
        match timeout(deadline, write).await {
            Err(_elapsed) => Err(TransportError::Timeout(Deadline::Frame)),
            Ok(res) => res,
        }
    }
}

/// A handler drives one accepted application session: it owns the bounded stream
/// and the post-handshake [`Exporter`], and returns when the protocol completes
/// or errors. Returning (or being dropped on cancellation) releases every
/// per-session resource.
pub type Handler = Arc<
    dyn Fn(
            BoundedStream,
            Exporter,
        ) -> Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send>>
        + Send
        + Sync,
>;

/// The connection abstraction the router drives. The real iroh
/// [`Connection`](iroh::endpoint::Connection) implements it; tests implement it
/// over in-memory duplex streams. `Clone` is cheap (an iroh `Connection` is an
/// `Arc` handle) and lets the exporter capability outlive the dispatch frame.
pub trait RouterConnection: Clone + Send + Sync + 'static {
    /// The negotiated ALPN, or `None` if the peer negotiated no known protocol.
    fn negotiated_alpn(&self) -> Option<Vec<u8>>;

    /// Derive `out_len` bytes of TLS exporter keying material. Meaningful only
    /// after the handshake — the router never calls it before [`Self::accept_bi`].
    fn export_keying_material(
        &self,
        label: &[u8],
        context: &[u8],
        out_len: usize,
    ) -> Result<Vec<u8>, TransportError>;

    /// Accept the single bidirectional application stream (post-handshake).
    fn accept_bi(&self)
        -> impl Future<Output = Result<(BoxWrite, BoxRead), TransportError>> + Send;

    /// Resolves when the peer opens a FORBIDDEN extra stream — a second
    /// bidirectional stream or any unidirectional stream. For a well-behaved
    /// peer that keeps to its one stream this future never resolves.
    fn accept_extra(&self) -> impl Future<Output = ()> + Send;

    /// Close the connection with a short reason. Idempotent.
    fn close(&self, reason: &[u8]);
}

struct Registration {
    deadlines: Deadlines,
    max_frame_bytes: usize,
    handler: Handler,
}

/// Routes negotiated ALPNs to bounded protocol handlers over one iroh endpoint.
///
/// Register a handler per ALPN with [`AlpnRouter::register`]; feed each accepted
/// connection to [`AlpnRouter::dispatch`]. A single [`Semaphore`] caps concurrent
/// sessions; the permit is acquired before any protocol work and released the
/// instant the session ends for any reason.
pub struct AlpnRouter {
    handlers: HashMap<Vec<u8>, Registration>,
    sessions: Arc<Semaphore>,
}

impl AlpnRouter {
    /// A router that admits at most `max_sessions` concurrent sessions.
    pub fn new(max_sessions: usize) -> Self {
        Self {
            handlers: HashMap::new(),
            sessions: Arc::new(Semaphore::new(max_sessions)),
        }
    }

    /// Register `handler` for `alpn` with its lifecycle `deadlines`. Replacing an
    /// existing registration is allowed (last write wins). The default frame
    /// ceiling is the sync protocol's maximum; protocols with a smaller
    /// canonical ceiling must use [`Self::register_with_max_frame`].
    pub fn register(&mut self, alpn: &[u8], deadlines: Deadlines, handler: Handler) {
        self.register_with_max_frame(alpn, deadlines, MAX_SYNC_FRAME_BYTES, handler);
    }

    /// Register a handler with a protocol-specific pre-allocation frame ceiling.
    ///
    /// The bound is checked immediately after the four-byte length prefix and
    /// before allocating the body, so a smaller control protocol cannot inherit
    /// the much larger sync allocation ceiling.
    pub fn register_with_max_frame(
        &mut self,
        alpn: &[u8],
        deadlines: Deadlines,
        max_frame_bytes: usize,
        handler: Handler,
    ) {
        self.handlers.insert(
            alpn.to_vec(),
            Registration {
                deadlines,
                max_frame_bytes,
                handler,
            },
        );
    }

    /// Every ALPN this router routes, for `Endpoint::builder(..).alpns(..)`.
    pub fn alpns(&self) -> Vec<Vec<u8>> {
        self.handlers.keys().cloned().collect()
    }

    /// Currently available session permits (concurrency headroom). Primarily for
    /// tests asserting that a cancelled session releases its permit.
    pub fn available_permits(&self) -> usize {
        self.sessions.available_permits()
    }

    /// Route one accepted connection: match the ALPN, acquire the single session
    /// permit, accept the one bi-stream under the handshake deadline, then run
    /// the handler under the absolute lifetime while watching for a forbidden
    /// extra stream. On ANY failure the connection is closed and the permit
    /// released. Unknown ALPN and permit exhaustion allocate NO session.
    pub async fn dispatch<C: RouterConnection>(&self, conn: C) -> Result<(), TransportError> {
        let alpn = match conn.negotiated_alpn() {
            Some(a) => a,
            None => {
                conn.close(b"no-alpn");
                return Err(TransportError::UnknownAlpn);
            }
        };
        let reg = match self.handlers.get(&alpn) {
            Some(r) => r,
            None => {
                conn.close(b"unknown-alpn");
                return Err(TransportError::UnknownAlpn);
            }
        };
        // Acquire the one session permit BEFORE any protocol allocation. The
        // owned permit is held for the session's lifetime and dropped — releasing
        // it — when this scope ends, whatever the exit path.
        let permit = match Arc::clone(&self.sessions).try_acquire_owned() {
            Ok(p) => p,
            Err(_) => {
                conn.close(b"busy");
                return Err(TransportError::Busy);
            }
        };
        let result = run_session(&conn, reg).await;
        if result.is_err() {
            conn.close(b"session-closed");
        }
        drop(permit);
        result
    }
}

/// Run one session over an already-matched, permit-holding connection. Kept free
/// of the permit/ALPN bookkeeping so the lifecycle is easy to read: handshake →
/// build exporter → run handler under absolute lifetime while watching for a
/// forbidden extra stream.
async fn run_session<C: RouterConnection>(
    conn: &C,
    reg: &Registration,
) -> Result<(), TransportError> {
    // Handshake + single bi-stream, bounded by the handshake deadline.
    let (send, recv) = match timeout(reg.deadlines.handshake, conn.accept_bi()).await {
        Err(_elapsed) => return Err(TransportError::Timeout(Deadline::Handshake)),
        Ok(res) => res?,
    };

    // Post-handshake ONLY: hand the handler a capability to the live exporter.
    let exporter_conn = conn.clone();
    let exporter = Exporter(Arc::new(
        move |label: &[u8], context: &[u8], out_len: usize| {
            exporter_conn.export_keying_material(label, context, out_len)
        },
    ));

    let stream = BoundedStream::new(send, recv, reg.deadlines, reg.max_frame_bytes);
    let handler_fut = (reg.handler)(stream, exporter);

    // The absolute lifetime bounds the whole session; the extra-stream watcher
    // cancels it (dropping the handler future, releasing its resources) the
    // moment the peer opens a second/unidirectional stream.
    tokio::select! {
        biased;
        _ = conn.accept_extra() => Err(TransportError::StreamViolation),
        outcome = timeout(reg.deadlines.absolute, handler_fut) => match outcome {
            Err(_elapsed) => Err(TransportError::Timeout(Deadline::Absolute)),
            Ok(res) => res,
        },
    }
}
