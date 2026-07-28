//! Slice 2 — the phone-side anchor pull client (issue #107, Phase 3).
//!
//! `sync_with_anchor` is the "leave the room, still sync" win: a phone dials a
//! known anchor over real iroh `riot/sync/2` `ReadCommitted`, receives a
//! community's committed O/C/W snapshot, verifies EVERY received item through
//! the canonical riot-core gate (`verify_entry` — never accept-all), and imports
//! the store-admissible entries into the phone's real willow store through the
//! canonical preview→plan→commit boundary (`session.rs`). Nobody has to be online
//! at the same moment: the anchor is durable store-and-forward.
//!
//! Security posture (see
//! `docs/decisions/2026-07-23-mobile-iroh-transport-design.md`):
//!   1. EVERY dial goes through the transport-floor gate BEFORE any packet.
//!      The dial is authenticated by the root-signed site ticket, whose floor is
//!      admitted here through the canonical [`admit_public_site_ticket`] gate
//!      (the `RootSignedTicketCoreEnvelopeV2` equivalent of `admit_dial`): the
//!      root signature is verified and a `require_arti` floor fails CLOSED before
//!      a connection is opened, so a `require:arti` site can never be dialed over
//!      cleartext iroh from the phone. Raw `sync_connect` is never exposed.
//!   2. Inbound admission does full canonical verification, never accept-all:
//!      every pulled item runs through riot-core's checked `verify_entry`, then
//!      through `store.inspect` (which re-verifies on the way into the store).
//!
//! The drive loop is synchronous-from-the-caller (`block_on` on the FFI-owned
//! runtime) so it is host-unit-testable over the deterministic `Sync2Session`
//! FSM — no async test harness, no device.
//!
//! The socket-owning drive loop (`NetRuntime::sync_with_anchor` and its helpers)
//! is `pub(crate)` and driven from two places: the in-crate e2e
//! (`net::anchor_e2e`) and the `net`-gated `uniffi` wrapper (`super::ffi`), which
//! exposes it to Swift/Kotlin as `MobileNetRuntime::sync_with_anchor` (Slice 3a).
//! Some internal helpers remain exercised only by tests, so keep the lenient
//! dead-code posture for the non-test lib build.
#![allow(dead_code)]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use iroh::{Endpoint, EndpointAddr};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::timeout;

use riot_anchor_protocol::authority::{admit_public_site_ticket, AuthorityError, TicketFloor};
use riot_anchor_protocol::codec::{decode_canonical, CanonicalRecord};
use riot_anchor_protocol::records::{
    RootSignedTicketCoreEnvelopeV2, TransportFloor, MAX_TICKET_CORE_BYTES,
};
use riot_anchor_protocol::sync2::{
    compute_snapshot_digest, AdmissionSubject, OpenNamespace, OpenedNamespace, PhaseParty,
    Sync2Action, Sync2DirectionStage, Sync2Frame, Sync2Mode, Sync2Phase, Sync2Refusal,
    Sync2Repository, Sync2Session, Sync2Snapshot, MAX_SYNC2_FRAME_BYTES,
};
use riot_transport::ALPN_SYNC_V2;

use riot_core::willow::{
    decode_capability_canonic, decode_entry_canonic, william3_digest, AuthorisationToken, Entry,
    SignedWillowEntry,
};
use willow25::entry::{Entrylike, SubspaceSignature};

use crate::mobile_state::{import_anchor_pulled_namespace, ProfileState};
use crate::MobileError;

use super::NetRuntime;

/// Per-network-step deadline: bounded so a dead anchor fails the pull instead of
/// hanging the runtime.
const STEP: Duration = Duration::from_secs(30);

/// The anchor-profile item wire version + framing bounds — a byte-identical port
/// of `riot_anchor::sync_service`'s private item codec, reproduced here so the
/// SHIPPING net client can decode pulled items WITHOUT depending on the
/// `riot-anchor` daemon crate (iroh/tokio/rusqlite/fjall). The real trust
/// decision is riot-core's `verify_entry`, called on the decoded parts below.
const ITEM_VERSION: u8 = 1;
const SIGNATURE_BYTES: usize = 64;
/// Defensive ceiling on a single pulled item (mirrors the anchor's
/// `MAX_ANCHOR_ITEM_BYTES`, one willow entry + payload + framing).
const MAX_PULLED_ITEM_BYTES: usize = 1 << 20;

/// Why one pulled anchor item was refused at the phone's canonical gate. Every
/// variant is a fail-closed refusal: the item is NOT imported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PulledItemReject {
    /// The item envelope did not decode (framing/bounds/trailing bytes).
    Malformed,
    /// The canonical `Entry` did not decode.
    NonCanonicalEntry,
    /// The canonical `WriteCapability` did not decode.
    NonCanonicalCapability,
    /// The payload length or WILLIAM3 digest did not match the entry.
    PayloadMismatch,
    /// The capability + signature did not authorise the entry (Meadowcap fail).
    DoesNotAuthorise,
}

/// The decoded, verified parts of one pulled anchor item.
struct DecodedItem {
    entry_bytes: Vec<u8>,
    capability_bytes: Vec<u8>,
    signature: [u8; 64],
    payload_bytes: Vec<u8>,
}

/// Decode the anchor-profile item framing: version byte then four length-prefixed
/// fields `entry_bytes`, `capability_bytes`, 64-byte `signature`, `payload_bytes`.
fn decode_item_frame(bytes: &[u8]) -> Result<DecodedItem, PulledItemReject> {
    if bytes.len() > MAX_PULLED_ITEM_BYTES {
        return Err(PulledItemReject::Malformed);
    }
    let mut cursor = 0usize;
    let take = |cursor: &mut usize, n: usize| -> Result<&[u8], PulledItemReject> {
        let end = cursor.checked_add(n).ok_or(PulledItemReject::Malformed)?;
        if end > bytes.len() {
            return Err(PulledItemReject::Malformed);
        }
        let slice = &bytes[*cursor..end];
        *cursor = end;
        Ok(slice)
    };
    let read_len = |cursor: &mut usize| -> Result<usize, PulledItemReject> {
        let raw = take(cursor, 4)?;
        Ok(u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]) as usize)
    };

    if take(&mut cursor, 1)?[0] != ITEM_VERSION {
        return Err(PulledItemReject::Malformed);
    }
    let entry_len = read_len(&mut cursor)?;
    let entry_bytes = take(&mut cursor, entry_len)?.to_vec();
    let cap_len = read_len(&mut cursor)?;
    let capability_bytes = take(&mut cursor, cap_len)?.to_vec();
    let mut signature = [0u8; 64];
    signature.copy_from_slice(take(&mut cursor, SIGNATURE_BYTES)?);
    let payload_len = read_len(&mut cursor)?;
    let payload_bytes = take(&mut cursor, payload_len)?.to_vec();
    if cursor != bytes.len() {
        return Err(PulledItemReject::Malformed);
    }
    Ok(DecodedItem {
        entry_bytes,
        capability_bytes,
        signature,
        payload_bytes,
    })
}

/// Verify one untrusted pulled item end-to-end through riot-core's CHECKED gate
/// and project it to a [`SignedWillowEntry`]. This is the phone's client-side
/// trust boundary (security requirement 2): it decodes the entry + capability,
/// rebuilds the authorisation token from the 64-byte signature, checks payload
/// length + WILLIAM3 digest, and runs riot-core's `verify_entry` (the same
/// checked Meadowcap path the anchor admits with). It NEVER trusts the anchor's
/// assertion of authorisation. Mirrors `riot_anchor::sync_service`'s
/// `verify_anchor_item_parts` using only riot-core primitives.
fn verify_pulled_item(item_bytes: &[u8]) -> Result<SignedWillowEntry, PulledItemReject> {
    let decoded = decode_item_frame(item_bytes)?;
    let entry: Entry = decode_entry_canonic(&decoded.entry_bytes)
        .map_err(|_| PulledItemReject::NonCanonicalEntry)?;
    let capability = decode_capability_canonic(&decoded.capability_bytes)
        .map_err(|_| PulledItemReject::NonCanonicalCapability)?;

    // Payload integrity: declared length + WILLIAM3 digest must match exactly.
    if entry.payload_length() != decoded.payload_bytes.len() as u64 {
        return Err(PulledItemReject::PayloadMismatch);
    }
    if *entry.payload_digest().as_bytes() != william3_digest(&decoded.payload_bytes) {
        return Err(PulledItemReject::PayloadMismatch);
    }

    // REAL Meadowcap + signature verification through riot-core's checked path.
    let token = AuthorisationToken::new(capability, SubspaceSignature::from(decoded.signature));
    if !riot_core::willow::verify_entry(&entry, &token) {
        return Err(PulledItemReject::DoesNotAuthorise);
    }

    Ok(SignedWillowEntry {
        entry_bytes: decoded.entry_bytes,
        capability_bytes: decoded.capability_bytes,
        signature: decoded.signature,
        payload_bytes: decoded.payload_bytes,
    })
}

// ---------------------------------------------------------------------------
// The client-side ReadCommitted `Sync2Repository`.
//
// A minimal shipping port of the anchor test harness's `ClientSyncRepo`
// (crates/riot-anchor/tests/hosting_common) specialised to a ONE-WAY
// `ReadCommitted` pull: a single `AnchorToClient` receiver phase over an empty
// base, admitting every delivered item into a shared sink. The pure FSM
// (`Sync2Session::initiator`) drives it; this repo carries no network.
// ---------------------------------------------------------------------------

type PulledItem = (Vec<u8>, Vec<u8>);

/// One namespace's pull result on the wire side: `(namespace_id, Ok(items) or a
/// transport error, optional ReadCommitted refusal string)`.
type NamespaceWireResult = ([u8; 32], Result<Vec<PulledItem>, String>, Option<String>);

fn logical_bytes(items: &[PulledItem]) -> u64 {
    items.iter().map(|(_, bytes)| bytes.len() as u64).sum()
}

/// A client-side snapshot digest — the SAME formula the anchor's committed view
/// uses, computed independently so the client never asks the code under test for
/// its own expected value.
fn client_snapshot_digest(namespace_id: &[u8; 32], items: &[PulledItem]) -> [u8; 32] {
    let ids: Vec<Vec<u8>> = items.iter().map(|(id, _)| id.clone()).collect();
    compute_snapshot_digest(namespace_id, logical_bytes(items), &ids)
}

/// The initiator's (unused) sender snapshot — a ReadCommitted pull sends nothing,
/// but the trait requires a `Snapshot` associated type.
struct ClientSnapshot {
    namespace_id: [u8; 32],
    items: Vec<PulledItem>,
}

impl Sync2Snapshot for ClientSnapshot {
    fn snapshot_digest(&self) -> [u8; 32] {
        client_snapshot_digest(&self.namespace_id, &self.items)
    }
    fn entry_count(&self) -> u64 {
        self.items.len() as u64
    }
    fn logical_bytes(&self) -> u64 {
        logical_bytes(&self.items)
    }
    fn sorted_entry_ids(&self) -> Vec<Vec<u8>> {
        let mut ids: Vec<Vec<u8>> = self.items.iter().map(|(id, _)| id.clone()).collect();
        ids.sort_unstable();
        ids
    }
    fn item_bytes(&self, entry_id: &[u8]) -> Option<Vec<u8>> {
        self.items
            .iter()
            .find(|(id, _)| id.as_slice() == entry_id)
            .map(|(_, bytes)| bytes.clone())
    }
}

/// The initiator's receiver stage: admitted items land in a shared sink so the
/// caller can inspect exactly what the pull delivered after the session ends.
struct ClientStage {
    admitted: Rc<RefCell<Vec<PulledItem>>>,
}

impl Sync2DirectionStage for ClientStage {
    fn missing(&self, page_ids: &[Vec<u8>]) -> Vec<Vec<u8>> {
        let admitted = self.admitted.borrow();
        page_ids
            .iter()
            .filter(|id| !admitted.iter().any(|(have, _)| have == *id))
            .cloned()
            .collect()
    }
    fn admit(&mut self, entry_ids: &[Vec<u8>], items: &[Vec<u8>]) -> Result<(), AdmissionSubject> {
        let mut admitted = self.admitted.borrow_mut();
        for (id, bytes) in entry_ids.iter().zip(items.iter()) {
            admitted.push((id.clone(), bytes.clone()));
        }
        Ok(())
    }
    fn resulting_digest(&self, namespace_id: &[u8; 32]) -> [u8; 32] {
        client_snapshot_digest(namespace_id, &self.admitted.borrow())
    }
    fn promote(&mut self) {}
}

/// The one-way ReadCommitted initiator repository.
struct ClientReadCommittedRepo {
    namespace_id: [u8; 32],
    admitted: Rc<RefCell<Vec<PulledItem>>>,
}

impl Sync2Repository for ClientReadCommittedRepo {
    type Snapshot = ClientSnapshot;
    type DirectionStage = ClientStage;
    fn open_namespace(
        &self,
        request: &OpenNamespace,
    ) -> Result<OpenedNamespace<Self>, Sync2Refusal> {
        let parties = vec![(
            Sync2Phase::AnchorToClient,
            PhaseParty::Receiver(ClientStage {
                admitted: Rc::clone(&self.admitted),
            }),
        )];
        Ok(OpenedNamespace {
            namespace_id: self.namespace_id,
            mode: request.mode.tag(),
            parties,
            stale_source: None,
        })
    }
}

/// Build a real ReadCommitted initiator session for `namespace_id` under
/// `ticket_core_bytes`, plus the shared sink the pulled items land in.
fn read_committed_initiator(
    namespace_id: [u8; 32],
    ticket_core_bytes: Vec<u8>,
) -> (
    Sync2Session<ClientReadCommittedRepo>,
    Rc<RefCell<Vec<PulledItem>>>,
) {
    let admitted = Rc::new(RefCell::new(Vec::new()));
    let repo = ClientReadCommittedRepo {
        namespace_id,
        admitted: Rc::clone(&admitted),
    };
    let open = OpenNamespace {
        protocol_version: 2,
        session_id: vec![1, 2, 3, 4],
        ticket_core_bytes,
        namespace_id,
        mode: Sync2Mode::ReadCommitted,
    };
    (Sync2Session::initiator(repo, open), admitted)
}

// ---------------------------------------------------------------------------
// Outcome + error types.
// ---------------------------------------------------------------------------

/// The per-namespace result of a pull.
///
/// This is a `uniffi::Record` (the `net` FFI surface): the raw `[u8; 32]`
/// namespace id is projected to lowercase hex and the counts to `u32`, matching
/// the crate's existing id-as-hex FFI convention (`CurrentEntry::namespace_id`,
/// `CommunityRow::namespace_id`). UniFFI cannot carry `[u8; 32]` or `usize`.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct NamespacePullOutcome {
    /// The pulled namespace (one of the ticket's O/C/W ids), lowercase hex.
    pub namespace_id: String,
    /// Items received over the wire AND verified through the canonical gate.
    pub verified: u32,
    /// Items committed into the phone's willow store through the canonical
    /// preview→plan→commit boundary. May be < `verified`: an entry the store
    /// deliberately does not admit as content (e.g. the reserved `/manifest`,
    /// which is validated on its own path, never stored) is verified but not
    /// imported — that is correct, not a fault.
    pub imported: u32,
    /// A structured refusal if the ReadCommitted session did not complete (e.g.
    /// the anchor has nothing committed for this community). `None` on success.
    pub refusal: Option<String>,
    /// Items received but REFUSED at the phone's canonical gate (never imported).
    /// Non-empty means the anchor served something that did not verify.
    pub rejected: u32,
}

/// The structured outcome of a `sync_with_anchor` pull. `uniffi::Record` — the
/// value the native app receives from `MobileNetRuntime::sync_with_anchor`.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AnchorSyncOutcome {
    /// The community root (== the ticket's O namespace), lowercase hex.
    pub root: String,
    /// One entry per ordered O/C/W namespace attempted.
    pub namespaces: Vec<NamespacePullOutcome>,
}

impl AnchorSyncOutcome {
    /// Total items imported into the phone store across all namespaces.
    pub fn total_imported(&self) -> u32 {
        self.namespaces.iter().map(|ns| ns.imported).sum()
    }
    /// Total items verified across all namespaces.
    pub fn total_verified(&self) -> u32 {
        self.namespaces.iter().map(|ns| ns.verified).sum()
    }
}

/// Why a `sync_with_anchor` pull failed before producing an outcome.
///
/// Rust-internal typed error (carries `AuthorityError` / `MobileError`). The
/// `net` FFI surface projects it to the flat `uniffi::Error` [`super::ffi::AnchorSyncError`]
/// via `From`, because UniFFI cannot carry those foreign payload types.
#[derive(Debug)]
pub enum AnchorPullError {
    /// The ticket bytes did not decode as a `RootSignedTicketCoreEnvelopeV2`.
    TicketMalformed,
    /// The transport-floor gate REFUSED the dial (bad root signature, expired,
    /// epoch rollback, or a `require:arti` floor the phone cannot provide). NO
    /// connection was opened. Fail-closed — the security-critical refusal.
    DialRefused(AuthorityError),
    /// A network/transport fault while pulling.
    Transport(String),
    /// The phone-store import failed.
    Import(MobileError),
}

// ---------------------------------------------------------------------------
// The transport-floor gate (security requirement 1) + the pull entry point.
// ---------------------------------------------------------------------------

/// The pre-dial transport-floor gate for the V2 anchor ticket — the
/// `RootSignedTicketCoreEnvelopeV2` equivalent of `admit_dial`. Runs the
/// canonical [`admit_public_site_ticket`] gate (root-signature verification +
/// transport-floor fail-closed) BEFORE any packet. The client floor is
/// `require_none`: the phone provides iroh but NOT arti, so a `require:arti`
/// ticket returns `UnsupportedTransport` and NO dial happens. `manifest = None`:
/// the phone has no committed manifest to match yet (first pull); the gate still
/// verifies the signature, version, transport floor, lifetime, and expiry.
fn admit_anchor_ticket(
    ticket_bytes: &[u8],
    now_unix: u64,
) -> Result<RootSignedTicketCoreEnvelopeV2, AnchorPullError> {
    let envelope = decode_canonical::<RootSignedTicketCoreEnvelopeV2>(
        ticket_bytes,
        MAX_TICKET_CORE_BYTES + 128,
    )
    .map_err(|_| AnchorPullError::TicketMalformed)?;
    admit_public_site_ticket(
        &envelope,
        None,
        &TransportFloor::RequireNone,
        &TicketFloor {
            root_id: envelope.core.root_id,
            highest_transport_epoch: None,
        },
        now_unix,
    )
    .map_err(AnchorPullError::DialRefused)?;
    Ok(envelope)
}

/// Dial the anchor's `riot/sync/2` ALPN and drive a ReadCommitted `session` to
/// completion over the connection. The gate in [`admit_anchor_ticket`] has
/// already run (verify-before-dial); this opens exactly one authenticated
/// connection through the anchor's own bounded router. Raw `sync_connect`
/// (the ungated `riot/sync/1` primitive) is never used.
async fn drive_read_committed(
    endpoint: &Endpoint,
    anchor_addr: EndpointAddr,
    mut session: Sync2Session<ClientReadCommittedRepo>,
) -> Result<Sync2Session<ClientReadCommittedRepo>, String> {
    let conn = timeout(STEP, endpoint.connect(anchor_addr, ALPN_SYNC_V2))
        .await
        .map_err(|_| "sync dial timed out".to_string())?
        .map_err(|error| format!("sync dial failed: {error}"))?;
    let (send, recv) = conn
        .open_bi()
        .await
        .map_err(|error| format!("open sync stream: {error}"))?;
    let mut send = Box::pin(send);
    let mut recv = Box::pin(recv);

    fn encode_sends(actions: Vec<Sync2Action>) -> Result<Vec<Vec<u8>>, String> {
        actions
            .into_iter()
            .filter_map(|action| match action {
                Sync2Action::Send(frame) => Some(
                    frame
                        .encode_canonical()
                        .map_err(|error| format!("encode sync2 frame: {error:?}")),
                ),
                _ => None,
            })
            .collect()
    }

    for bytes in encode_sends(session.start())? {
        write_frame(&mut send, &bytes).await?;
    }
    while !session.is_terminated() {
        let bytes = timeout(STEP, read_frame(&mut recv))
            .await
            .map_err(|_| "sync frame timed out".to_string())??;
        let frame = decode_canonical::<Sync2Frame>(&bytes, MAX_SYNC2_FRAME_BYTES)
            .map_err(|error| format!("inbound sync2 frame did not decode: {error:?}"))?;
        for out in encode_sends(session.on_frame(frame))? {
            write_frame(&mut send, &out).await?;
        }
    }
    let _ = send.shutdown().await;
    Ok(session)
}

/// Write one `u32be`-length-prefixed frame.
async fn write_frame<W: tokio::io::AsyncWrite + Unpin>(
    writer: &mut W,
    body: &[u8],
) -> Result<(), String> {
    let len = u32::try_from(body.len()).map_err(|_| "frame too large".to_string())?;
    writer
        .write_all(&len.to_be_bytes())
        .await
        .map_err(|error| format!("write frame length: {error}"))?;
    writer
        .write_all(body)
        .await
        .map_err(|error| format!("write frame body: {error}"))?;
    writer
        .flush()
        .await
        .map_err(|error| format!("flush frame: {error}"))
}

/// Read one `u32be`-length-prefixed frame.
async fn read_frame<R: tokio::io::AsyncRead + Unpin>(reader: &mut R) -> Result<Vec<u8>, String> {
    let mut len = [0u8; 4];
    reader
        .read_exact(&mut len)
        .await
        .map_err(|error| format!("read frame length: {error}"))?;
    let n = u32::from_be_bytes(len) as usize;
    if n > MAX_SYNC2_FRAME_BYTES {
        return Err("inbound frame exceeds sync2 ceiling".to_string());
    }
    let mut body = vec![0u8; n];
    reader
        .read_exact(&mut body)
        .await
        .map_err(|error| format!("read frame body: {error}"))?;
    Ok(body)
}

impl NetRuntime {
    /// Pull a community's committed O/C/W data from `anchor_addr` over
    /// `riot/sync/2` `ReadCommitted`, verify every entry through the canonical
    /// gate, and import the store-admissible entries into `profile_inner`'s
    /// willow store through the canonical preview→plan→commit boundary.
    ///
    /// Synchronous from the caller's view (the design's `block_on` seam): the
    /// gated dial + FSM drive runs on this runtime's endpoint, so the whole thing
    /// is host-unit-testable over a loopback anchor. `now_unix` is the wall-clock
    /// second used for ticket freshness (injectable for deterministic tests).
    ///
    /// Order (fail-closed): (1) the transport-floor gate runs BEFORE any packet;
    /// (2) each of the ticket's O/C/W namespaces is dialed + pulled; (3) every
    /// received item is verified with riot-core's `verify_entry`; (4) verified
    /// entries are imported (the store re-verifies). A namespace the anchor never
    /// committed simply refuses at ReadCommitted open — recorded as a refusal,
    /// imports nothing, no crash.
    pub(crate) fn sync_with_anchor(
        &self,
        profile_inner: &Arc<Mutex<ProfileState>>,
        anchor_addr: EndpointAddr,
        ticket_bytes: &[u8],
        now_unix: u64,
    ) -> Result<AnchorSyncOutcome, AnchorPullError> {
        // (1) SECURITY: the transport-floor gate, BEFORE any dial. A refusal here
        //     returns without opening a connection.
        let envelope = admit_anchor_ticket(ticket_bytes, now_unix)?;
        let core = &envelope.core;
        let root = core.root_id;
        let namespaces = [
            core.o_namespace_id,
            core.c_namespace_id,
            core.w_namespace_id,
        ];
        let ticket_core_bytes = ticket_bytes.to_vec();

        // (2) Pull each namespace on the owned runtime. Network + verification
        //     only — no profile lock is held across an await. `runtime` and
        //     `endpoint` are the parent `NetRuntime`'s private fields, reachable
        //     from this child module.
        let endpoint = &self.endpoint;
        let pulls: Vec<NamespaceWireResult> = self.runtime.block_on(async {
            let mut results = Vec::with_capacity(namespaces.len());
            for namespace_id in namespaces {
                let (session, admitted) =
                    read_committed_initiator(namespace_id, ticket_core_bytes.clone());
                match drive_read_committed(endpoint, anchor_addr.clone(), session).await {
                    Ok(session) => {
                        if session.is_complete() {
                            let items = admitted.borrow().clone();
                            results.push((namespace_id, Ok(items), None));
                        } else {
                            let refusal = format!("{:?}", session.refusal());
                            results.push((namespace_id, Ok(Vec::new()), Some(refusal)));
                        }
                    }
                    Err(error) => results.push((namespace_id, Err(error), None)),
                }
            }
            results
        });

        // (3) + (4) Verify every pulled item, then import the verified set through
        //     the canonical boundary. Done OUTSIDE the async block (sync store I/O).
        let mut outcomes = Vec::with_capacity(namespaces.len());
        for (namespace_id, pulled, refusal) in pulls {
            let items = match pulled {
                Ok(items) => items,
                Err(error) => return Err(AnchorPullError::Transport(error)),
            };
            let mut verified: Vec<SignedWillowEntry> = Vec::with_capacity(items.len());
            let mut rejected = 0usize;
            for (_entry_id, item_bytes) in &items {
                match verify_pulled_item(item_bytes) {
                    Ok(signed) => verified.push(signed),
                    Err(_) => rejected += 1,
                }
            }
            let imported = if verified.is_empty() {
                0
            } else {
                import_anchor_pulled_namespace(profile_inner, root, &verified)
                    .map_err(AnchorPullError::Import)?
            };
            outcomes.push(NamespacePullOutcome {
                namespace_id: crate::mobile_state::hex(&namespace_id),
                verified: verified.len() as u32,
                imported: imported as u32,
                refusal,
                rejected: rejected as u32,
            });
        }

        Ok(AnchorSyncOutcome {
            root: crate::mobile_state::hex(&root),
            namespaces: outcomes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_item_frame_round_trips_and_rejects_trailing_bytes() {
        // version | u32 entry_len | entry | u32 cap_len | cap | 64 sig | u32 plen | payload
        let mut item = vec![ITEM_VERSION];
        item.extend_from_slice(&3u32.to_be_bytes());
        item.extend_from_slice(b"abc");
        item.extend_from_slice(&2u32.to_be_bytes());
        item.extend_from_slice(b"xy");
        item.extend_from_slice(&[9u8; SIGNATURE_BYTES]);
        item.extend_from_slice(&4u32.to_be_bytes());
        item.extend_from_slice(b"data");
        let decoded = decode_item_frame(&item).expect("well-formed item decodes");
        assert_eq!(decoded.entry_bytes, b"abc");
        assert_eq!(decoded.capability_bytes, b"xy");
        assert_eq!(decoded.signature, [9u8; SIGNATURE_BYTES]);
        assert_eq!(decoded.payload_bytes, b"data");

        // A trailing byte is rejected (no slack).
        item.push(0x00);
        assert!(matches!(
            decode_item_frame(&item),
            Err(PulledItemReject::Malformed)
        ));
    }
}
