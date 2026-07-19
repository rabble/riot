//! Riot transport adapters. iroh + tokio live here, NOT in riot-core — the
//! reconcile core stays pure, sync, and wasm-clean. This crate is the thin
//! adapter the composite spec §5 describes: a `FrameChannel` pumps the same
//! `SyncFrame` bytes through `ByteSyncSession`, and the transport-agnostic FSM
//! never learns which channel it is on.
//!
//! [`pump`] is generic over any async byte stream (`AsyncRead`/`AsyncWrite`), so
//! it drives a reconcile session over an in-memory duplex, a TCP socket, or a
//! real iroh QUIC bi-stream identically. The concrete iroh channel is in
//! [`iroh`].

use riot_core::sync::{ByteSyncOutcome, ByteSyncSession, SyncError, MAX_SYNC_FRAME_BYTES};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub mod iroh;
pub mod router;
pub mod seed;
pub mod ticket;

/// The v1 sync ALPN — the reconcile protocol negotiated on an iroh connection.
pub const ALPN: &[u8] = b"riot/sync/1";

/// The v2 routed-paginated sync ALPN (design "`riot/sync/2`"). The handler is
/// supplied by the anchor crate; this crate only routes it.
pub const ALPN_SYNC_V2: &[u8] = b"riot/sync/2";

/// The anchor control-plane ALPN (design "`riot/anchor/1`: Control Plane"). The
/// exact 13-byte ASCII value bound into the peer-proof transcript.
pub const ALPN_ANCHOR_V1: &[u8] = b"riot/anchor/1";

#[derive(Debug)]
pub enum TransportError {
    /// A framed sync frame exceeded the protocol's own bound.
    FrameTooLarge,
    /// The peer closed the stream mid-exchange.
    StreamClosed,
    /// The pre-connection gate refused to dial (fail-closed, §5.1-5.3). No
    /// connection was opened — this is a REFUSAL, not a network failure.
    Blocked(ticket::TransportBlocked),
    /// The peer negotiated an ALPN this endpoint does not route (or none at all).
    /// The connection is closed WITHOUT allocating a protocol session.
    UnknownAlpn,
    /// No session permit was available: the endpoint is at its concurrent-session
    /// ceiling. The connection is closed without allocating a session.
    Busy,
    /// The peer opened a forbidden second application stream (bidirectional) or
    /// any unidirectional stream. Each connection carries exactly one session and
    /// one bi-stream; a violation resets and closes the connection.
    StreamViolation,
    /// A bounded-lifecycle deadline elapsed (which one is carried). Cancellation
    /// releases the session permit and all per-session resources.
    Timeout(router::Deadline),
    Io(std::io::Error),
    Sync(SyncError),
}

impl From<ticket::TransportBlocked> for TransportError {
    fn from(e: ticket::TransportBlocked) -> Self {
        Self::Blocked(e)
    }
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FrameTooLarge => write!(f, "sync frame exceeds MAX_SYNC_FRAME_BYTES"),
            Self::StreamClosed => write!(f, "peer closed the stream mid-exchange"),
            Self::Blocked(b) => write!(f, "dial refused (fail-closed): {b}"),
            Self::UnknownAlpn => write!(f, "peer negotiated an unrouted ALPN"),
            Self::Busy => write!(f, "session ceiling reached; connection refused"),
            Self::StreamViolation => {
                write!(f, "peer opened a forbidden second/unidirectional stream")
            }
            Self::Timeout(d) => write!(f, "bounded lifecycle deadline elapsed: {d:?}"),
            Self::Io(e) => write!(f, "transport io: {e}"),
            Self::Sync(e) => write!(f, "reconcile error: {e:?}"),
        }
    }
}

impl std::error::Error for TransportError {}

impl From<std::io::Error> for TransportError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<SyncError> for TransportError {
    fn from(e: SyncError) -> Self {
        Self::Sync(e)
    }
}

/// Length-prefixed frame write: `u32be(len) || frame`, flushed.
async fn write_frame<S: AsyncWrite + Unpin>(
    stream: &mut S,
    frame: &[u8],
) -> Result<(), TransportError> {
    let len = u32::try_from(frame.len()).map_err(|_| TransportError::FrameTooLarge)?;
    stream.write_all(&len.to_be_bytes()).await?;
    stream.write_all(frame).await?;
    stream.flush().await?;
    Ok(())
}

/// Read one length-prefixed frame, bounded by the protocol's own frame ceiling
/// (an attacker cannot make us allocate more than the reconcile FSM would accept).
async fn read_frame<R: AsyncRead + Unpin>(stream: &mut R) -> Result<Vec<u8>, TransportError> {
    let mut len_bytes = [0u8; 4];
    match stream.read_exact(&mut len_bytes).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
            return Err(TransportError::StreamClosed)
        }
        Err(e) => return Err(e.into()),
    }
    let len = u32::from_be_bytes(len_bytes) as usize;
    if len > MAX_SYNC_FRAME_BYTES {
        return Err(TransportError::FrameTooLarge);
    }
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).await?;
    Ok(buf)
}

/// Drive a reconcile session to completion over one bidirectional byte stream.
///
/// `initiator` opens with a Hello; the other side waits for it. `on_bundle` is
/// the admission hook: a received `Entries` bundle is handed to it, and its
/// bool decides accept (import) vs reject — this is where the caller runs the
/// real preview→commit boundary. Returns the terminal session.
pub async fn pump<S, R, F>(
    mut session: ByteSyncSession,
    send: &mut S,
    recv: &mut R,
    initiator: bool,
    mut on_bundle: F,
) -> Result<ByteSyncSession, TransportError>
where
    S: AsyncWrite + Unpin,
    R: AsyncRead + Unpin,
    F: FnMut(&[u8]) -> bool,
{
    let mut outcome = if initiator {
        session.begin()?
    } else {
        let first = read_frame(recv).await?;
        session.receive_bytes(&first)?
    };

    loop {
        // A received bundle needs an admission decision, then re-drives the FSM
        // WITHOUT reading from the peer (import produces the next frame to send).
        if let ByteSyncOutcome::ImportBundle(bytes) = outcome {
            outcome = if on_bundle(&bytes) {
                session.import_accepted()?
            } else {
                session.import_rejected(1)?
            };
            continue;
        }

        // FrameReady / Rejected / Complete: flush whatever frame is queued.
        if let Some(frame) = session.take_outbound_frame() {
            write_frame(send, &frame).await?;
        }
        if session.is_terminal() {
            break;
        }
        let bytes = read_frame(recv).await?;
        outcome = session.receive_bytes(&bytes)?;
    }
    Ok(session)
}
