//! The transport-independent `sync/2` finite state machine.
//!
//! [`Sync2Session`] models one endpoint (the responder anchor, or the connection
//! initiator). It consumes [`Sync2Frame`]s via [`Sync2Session::on_frame`] and
//! yields [`Sync2Action`]s — frames to transmit, chunk admissions, direction
//! promotions, and completion — while performing no IO. A duplex harness drives
//! two sessions against each other in memory.
//!
//! The responder routes an inbound [`OpenNamespace`] through
//! [`Sync2Repository::open_namespace`], which returns the ordered per-phase
//! sender/receiver [`PhaseParty`] plan for this endpoint (or a routing
//! [`Sync2Refusal`]). `ReadCommitted` has a single sender direction on the anchor
//! and therefore no stage to mutate; the staged modes carry their direction stages
//! privately until the parent operation promotes.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    encode_bundle, ids_page_digest, AdmissionSubject, DirectionComplete, EntriesChunk, IdsPage,
    NamespaceComplete, NeedEntries, OpenNamespace, PageComplete, PageNeedsComplete, SnapshotStart,
    Sync2Frame, Sync2FrameName, Sync2ModeTag, Sync2Phase, Sync2Refusal, MAX_ENTRIES_PER_CHUNK,
    MAX_IDS_PER_NEED, MAX_IDS_PER_PAGE, MAX_NEEDS_PER_PAGE,
};
use crate::digest::sync_snapshot_digest;

/// The ordered per-phase sender/receiver plan for one endpoint.
pub type PhasePlan<R> = Vec<(
    Sync2Phase,
    PhaseParty<<R as Sync2Repository>::Snapshot, <R as Sync2Repository>::DirectionStage>,
)>;

/// An immutable inventory snapshot the sender streams from.
pub trait Sync2Snapshot {
    /// The immutable snapshot digest.
    fn snapshot_digest(&self) -> [u8; 32];
    /// The snapshot's entry count.
    fn entry_count(&self) -> u64;
    /// The exact logical byte sum of the snapshot's canonical items.
    fn logical_bytes(&self) -> u64;
    /// The full entry IDs, ascending (lexicographic).
    fn sorted_entry_ids(&self) -> Vec<Vec<u8>>;
    /// The canonical item bytes for one entry ID, if present.
    fn item_bytes(&self, entry_id: &[u8]) -> Option<Vec<u8>>;
}

/// A direction-private staging area a receiver admits into and later promotes.
pub trait Sync2DirectionStage {
    /// Which of these page IDs the receiver is missing and must request.
    fn missing(&self, page_ids: &[Vec<u8>]) -> Vec<Vec<u8>>;
    /// Admit one chunk's items (parallel to `entry_ids`) into direction-private
    /// staging in a short transaction. Err carries the admission subject.
    fn admit(&mut self, entry_ids: &[Vec<u8>], items: &[Vec<u8>]) -> Result<(), AdmissionSubject>;
    /// The resulting committed+staged snapshot digest for this namespace.
    fn resulting_digest(&self, namespace_id: &[u8; 32]) -> [u8; 32];
    /// Atomically promote this direction's stage into its parent operation.
    fn promote(&mut self);
}

/// One phase's role for an endpoint: it either sends an immutable snapshot or
/// receives into a direction-private stage.
pub enum PhaseParty<S, D> {
    /// This endpoint is the inventory sender for the phase.
    Sender(S),
    /// This endpoint is the receiver for the phase.
    Receiver(D),
}

/// The routed per-endpoint plan produced by [`Sync2Repository::open_namespace`].
pub struct OpenedNamespace<R: Sync2Repository> {
    /// The routed namespace ID.
    pub namespace_id: [u8; 32],
    /// The routed mode.
    pub mode: Sync2ModeTag,
    /// The ordered phases for this endpoint, each with its role.
    pub parties: PhasePlan<R>,
    /// A pre-`SnapshotStart` refusal (a replica source that opened a stale
    /// immutable read transaction emits `stale_source` here).
    pub stale_source: Option<Sync2Refusal>,
}

/// The repository the responder (and, symmetrically, the initiator's own side)
/// routes an `OpenNamespace` through.
pub trait Sync2Repository: Sized {
    /// The immutable snapshot type.
    type Snapshot: Sync2Snapshot;
    /// The direction-private stage type.
    type DirectionStage: Sync2DirectionStage;
    /// Verify and route an `OpenNamespace`, returning this endpoint's phase plan or
    /// a routing refusal.
    fn open_namespace(
        &self,
        request: &OpenNamespace,
    ) -> Result<OpenedNamespace<Self>, Sync2Refusal>;
}

/// An action the driver must perform in response to a consumed frame.
pub enum Sync2Action {
    /// Transmit a frame to the peer.
    Send(Sync2Frame),
    /// A chunk was admitted into direction-private staging.
    Admit(EntriesChunk),
    /// The current direction's stage was promoted into its parent operation.
    PromoteDirection,
    /// The namespace session completed successfully.
    Complete,
}

impl core::fmt::Debug for Sync2Action {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Sync2Action::Send(frame) => write!(f, "Send({:?})", frame.name()),
            Sync2Action::Admit(chunk) => {
                write!(
                    f,
                    "Admit(req={}, idx={})",
                    chunk.request_id, chunk.chunk_index
                )
            }
            Sync2Action::PromoteDirection => f.write_str("PromoteDirection"),
            Sync2Action::Complete => f.write_str("Complete"),
        }
    }
}

// ---------------------------------------------------------------------------
// Transient per-direction state.
// ---------------------------------------------------------------------------

enum SenderStage {
    AwaitingNeeds,
    AwaitingDirectionComplete,
}

struct SenderState {
    phase: Sync2Phase,
    snapshot_digest: [u8; 32],
    sorted_ids: Vec<Vec<u8>>,
    cursor: usize,
    page_ids: Vec<Vec<u8>>,
    page_digest: [u8; 32],
    page_done: bool,
    pending: Vec<NeedEntries>,
    seen_request_ids: BTreeSet<u64>,
    stage: SenderStage,
}

struct RequestProgress {
    requested_ids: Vec<Vec<u8>>,
    next_chunk_index: u64,
    consumed: usize,
    done: bool,
}

enum ReceiverStage {
    AwaitingSnapshotStart,
    AwaitingPage,
    Receiving,
}

struct ReceiverState {
    phase: Sync2Phase,
    namespace_id: [u8; 32],
    snapshot_digest: [u8; 32],
    entry_count: u64,
    logical_bytes: u64,
    all_ids: Vec<Vec<u8>>,
    cursor: Option<Vec<u8>>,
    page_ids: Vec<Vec<u8>>,
    page_digest: [u8; 32],
    page_done: bool,
    outstanding: BTreeMap<u64, RequestProgress>,
    stage: ReceiverStage,
}

enum DirState {
    Idle,
    Sending(SenderState),
    Receiving(ReceiverState),
}

// ---------------------------------------------------------------------------
// The session.
// ---------------------------------------------------------------------------

/// A transport-independent `sync/2` endpoint state machine.
pub struct Sync2Session<R: Sync2Repository> {
    responder: bool,
    repo: R,
    open: Option<OpenNamespace>,
    mode: Sync2ModeTag,
    namespace_id: [u8; 32],
    steps: PhasePlan<R>,
    current: usize,
    dir: DirState,
    stale_source: Option<Sync2Refusal>,
    last_sender_digest: Option<[u8; 32]>,
    last_receiver_verified: Option<[u8; 32]>,
    awaiting_namespace_complete: bool,
    terminated: bool,
    complete: bool,
    refusal: Option<Sync2Refusal>,
}

impl<R: Sync2Repository> Sync2Session<R> {
    /// Create a responder (anchor) session that waits for an `OpenNamespace`.
    pub fn responder(repo: R) -> Self {
        Self::new(true, repo, None)
    }

    /// Create an initiator session that will open `open` on `start`.
    pub fn initiator(repo: R, open: OpenNamespace) -> Self {
        Self::new(false, repo, Some(open))
    }

    fn new(responder: bool, repo: R, open: Option<OpenNamespace>) -> Self {
        Sync2Session {
            responder,
            repo,
            open,
            mode: Sync2ModeTag::ReadCommitted,
            namespace_id: [0u8; 32],
            steps: Vec::new(),
            current: 0,
            dir: DirState::Idle,
            stale_source: None,
            last_sender_digest: None,
            last_receiver_verified: None,
            awaiting_namespace_complete: false,
            terminated: false,
            complete: false,
            refusal: None,
        }
    }

    /// Whether the session completed successfully.
    pub fn is_complete(&self) -> bool {
        self.complete
    }

    /// Whether the session terminated (completed or refused).
    pub fn is_terminated(&self) -> bool {
        self.terminated || self.complete
    }

    /// The refusal this endpoint sent or received, if any.
    pub fn refusal(&self) -> Option<&Sync2Refusal> {
        self.refusal.as_ref()
    }

    /// The routed mode (valid after routing).
    pub fn mode(&self) -> Sync2ModeTag {
        self.mode
    }

    /// Kick off an initiator session: emit the `OpenNamespace`, then begin the
    /// first direction if this endpoint sends first (or emit a `stale_source`
    /// refusal before `SnapshotStart`).
    pub fn start(&mut self) -> Vec<Sync2Action> {
        if self.responder {
            return Vec::new();
        }
        let open = match self.open.clone() {
            Some(o) => o,
            None => return Vec::new(),
        };
        let opened = match self.repo.open_namespace(&open) {
            Ok(o) => o,
            Err(r) => return self.refuse(r),
        };
        self.install(opened);
        let mut actions = vec![Sync2Action::Send(Sync2Frame::OpenNamespace(open))];
        // A replica source that opened a stale immutable snapshot refuses before
        // any `SnapshotStart`.
        if let Some(stale) = self.stale_source.take() {
            actions.extend(self.refuse(stale));
            return actions;
        }
        actions.extend(self.enter_current_step());
        actions
    }

    /// Consume one inbound frame, producing driver actions.
    pub fn on_frame(&mut self, frame: Sync2Frame) -> Vec<Sync2Action> {
        if self.terminated || self.complete {
            return Vec::new();
        }
        // Any inbound refusal terminates this side without promotion.
        if let Sync2Frame::Refuse(r) = frame {
            self.terminated = true;
            self.refusal = Some(r);
            return Vec::new();
        }
        // The responder routes the opening frame.
        if self.responder && self.steps.is_empty() {
            return match frame {
                Sync2Frame::OpenNamespace(open) => self.route(open),
                other => self.refuse(self.unexpected(
                    Sync2Phase::AnchorToClient,
                    &[Sync2FrameName::OpenNamespace],
                    other.name(),
                )),
            };
        }
        // A terminal namespace_complete on the initiator.
        if self.awaiting_namespace_complete {
            return match frame {
                Sync2Frame::NamespaceComplete(nc) => self.on_namespace_complete(nc),
                other => {
                    let phase = self.current_phase();
                    self.refuse(self.unexpected(
                        phase,
                        &[Sync2FrameName::NamespaceComplete],
                        other.name(),
                    ))
                }
            };
        }
        // Route to the active direction sub-FSM.
        match &self.dir {
            DirState::Sending(_) => self.sender_on_frame(frame),
            DirState::Receiving(_) => self.receiver_on_frame(frame),
            DirState::Idle => {
                let phase = self.current_phase();
                self.refuse(self.unexpected(phase, &[], frame.name()))
            }
        }
    }

    // --- routing -----------------------------------------------------------

    fn route(&mut self, open: OpenNamespace) -> Vec<Sync2Action> {
        let opened = match self.repo.open_namespace(&open) {
            Ok(o) => o,
            Err(r) => return self.refuse(r),
        };
        self.install(opened);
        if let Some(stale) = self.stale_source.take() {
            return self.refuse(stale);
        }
        self.enter_current_step()
    }

    fn install(&mut self, opened: OpenedNamespace<R>) {
        self.namespace_id = opened.namespace_id;
        self.mode = opened.mode;
        self.steps = opened.parties;
        self.stale_source = opened.stale_source;
        self.current = 0;
    }

    fn current_phase(&self) -> Sync2Phase {
        self.steps
            .get(self.current)
            .map(|(p, _)| *p)
            .unwrap_or(Sync2Phase::AnchorToClient)
    }

    fn refuse(&mut self, r: Sync2Refusal) -> Vec<Sync2Action> {
        self.terminated = true;
        self.refusal = Some(r.clone());
        vec![Sync2Action::Send(Sync2Frame::Refuse(r))]
    }

    fn unexpected(
        &self,
        phase: Sync2Phase,
        expected: &[Sync2FrameName],
        observed: Sync2FrameName,
    ) -> Sync2Refusal {
        Sync2Refusal::UnexpectedFrame {
            phase,
            expected_frame_names: expected.to_vec(),
            observed_frame_name: observed,
        }
    }

    // --- step lifecycle ----------------------------------------------------

    /// Activate the current step: begin sending if this endpoint sends, else wait.
    fn enter_current_step(&mut self) -> Vec<Sync2Action> {
        let phase = self.current_phase();
        match self.steps.get(self.current) {
            Some((_, PhaseParty::Sender(snap))) => {
                let sender = SenderState {
                    phase,
                    snapshot_digest: snap.snapshot_digest(),
                    sorted_ids: snap.sorted_entry_ids(),
                    cursor: 0,
                    page_ids: Vec::new(),
                    page_digest: [0u8; 32],
                    page_done: false,
                    pending: Vec::new(),
                    seen_request_ids: BTreeSet::new(),
                    stage: SenderStage::AwaitingNeeds,
                };
                let entry_count = snap.entry_count();
                let logical_bytes = snap.logical_bytes();
                let snapshot_digest = sender.snapshot_digest;
                self.dir = DirState::Sending(sender);
                let mut actions = vec![Sync2Action::Send(Sync2Frame::SnapshotStart(
                    SnapshotStart {
                        phase,
                        namespace_id: self.namespace_id,
                        snapshot_digest,
                        entry_count,
                        logical_bytes,
                    },
                ))];
                actions.extend(self.send_next_page());
                actions
            }
            Some((_, PhaseParty::Receiver(_))) => {
                self.dir = DirState::Receiving(ReceiverState {
                    phase,
                    namespace_id: self.namespace_id,
                    snapshot_digest: [0u8; 32],
                    entry_count: 0,
                    logical_bytes: 0,
                    all_ids: Vec::new(),
                    cursor: None,
                    page_ids: Vec::new(),
                    page_digest: [0u8; 32],
                    page_done: false,
                    outstanding: BTreeMap::new(),
                    stage: ReceiverStage::AwaitingSnapshotStart,
                });
                Vec::new()
            }
            None => Vec::new(),
        }
    }

    /// Advance to the next step or finish the session.
    fn advance_step(&mut self) -> Vec<Sync2Action> {
        self.current += 1;
        if self.current < self.steps.len() {
            self.dir = DirState::Idle;
            self.enter_current_step()
        } else {
            self.finish()
        }
    }

    /// All steps done: the responder sends `NamespaceComplete`; the initiator
    /// awaits it.
    fn finish(&mut self) -> Vec<Sync2Action> {
        self.dir = DirState::Idle;
        if self.responder {
            let digest = match self.steps.last() {
                Some((_, PhaseParty::Sender(_))) => self.last_sender_digest.unwrap_or([0u8; 32]),
                Some((_, PhaseParty::Receiver(stage))) => {
                    stage.resulting_digest(&self.namespace_id)
                }
                None => [0u8; 32],
            };
            self.complete = true;
            self.terminated = true;
            vec![
                Sync2Action::Send(Sync2Frame::NamespaceComplete(NamespaceComplete {
                    mode: self.mode,
                    final_snapshot_digest: digest,
                })),
                Sync2Action::Complete,
            ]
        } else {
            self.awaiting_namespace_complete = true;
            Vec::new()
        }
    }

    fn on_namespace_complete(&mut self, nc: NamespaceComplete) -> Vec<Sync2Action> {
        if nc.mode != self.mode {
            return self.refuse(Sync2Refusal::InvalidMode {
                observed_mode: nc.mode,
            });
        }
        let expected = match self.steps.last() {
            Some((_, PhaseParty::Sender(_))) => self.last_sender_digest.unwrap_or([0u8; 32]),
            Some((_, PhaseParty::Receiver(_))) => self.last_receiver_verified.unwrap_or([0u8; 32]),
            None => [0u8; 32],
        };
        if nc.final_snapshot_digest != expected {
            return self.refuse(Sync2Refusal::SnapshotMismatch {
                expected_snapshot_digest: expected,
                observed_snapshot_digest: nc.final_snapshot_digest,
            });
        }
        self.awaiting_namespace_complete = false;
        self.complete = true;
        self.terminated = true;
        vec![Sync2Action::Complete]
    }

    // --- sender ------------------------------------------------------------

    fn send_next_page(&mut self) -> Vec<Sync2Action> {
        let (page_ids, after_exclusive, done, sender_phase, snapshot_digest) = {
            let sender = match &mut self.dir {
                DirState::Sending(s) => s,
                _ => return Vec::new(),
            };
            let start = sender.cursor;
            let end = (start + MAX_IDS_PER_PAGE).min(sender.sorted_ids.len());
            let page_ids: Vec<Vec<u8>> = sender.sorted_ids[start..end].to_vec();
            let after_exclusive = if start == 0 {
                None
            } else {
                Some(sender.sorted_ids[start - 1].clone())
            };
            let done = end == sender.sorted_ids.len();
            sender.cursor = end;
            sender.page_ids = page_ids.clone();
            sender.page_done = done;
            sender.pending.clear();
            sender.seen_request_ids.clear();
            sender.stage = SenderStage::AwaitingNeeds;
            (
                page_ids,
                after_exclusive,
                done,
                sender.phase,
                sender.snapshot_digest,
            )
        };
        let page = IdsPage {
            phase: sender_phase,
            snapshot_digest,
            after_exclusive,
            entry_ids: page_ids,
            done,
        };
        let page_digest = ids_page_digest(&page);
        if let DirState::Sending(sender) = &mut self.dir {
            sender.page_digest = page_digest;
        }
        vec![Sync2Action::Send(Sync2Frame::IdsPage(page))]
    }

    fn sender_on_frame(&mut self, frame: Sync2Frame) -> Vec<Sync2Action> {
        let phase = self.current_phase();
        match frame {
            Sync2Frame::NeedEntries(need) => self.sender_on_need(need),
            Sync2Frame::PageNeedsComplete(pnc) => self.sender_on_needs_complete(pnc),
            Sync2Frame::DirectionComplete(dc) => self.sender_on_direction_complete(dc),
            other => self.refuse(self.unexpected(
                phase,
                &[
                    Sync2FrameName::NeedEntries,
                    Sync2FrameName::PageNeedsComplete,
                    Sync2FrameName::DirectionComplete,
                ],
                other.name(),
            )),
        }
    }

    fn sender_on_need(&mut self, need: NeedEntries) -> Vec<Sync2Action> {
        let phase = self.current_phase();
        let sender = match &mut self.dir {
            DirState::Sending(s) if matches!(s.stage, SenderStage::AwaitingNeeds) => s,
            _ => {
                return self.refuse(self.unexpected(
                    phase,
                    &[Sync2FrameName::DirectionComplete],
                    Sync2FrameName::NeedEntries,
                ))
            }
        };
        if need.phase != sender.phase {
            return self.refuse(self.unexpected(
                phase,
                &[Sync2FrameName::NeedEntries],
                Sync2FrameName::NeedEntries,
            ));
        }
        if need.page_digest != sender.page_digest {
            let expected = sender.page_digest;
            return self.refuse(Sync2Refusal::PageMismatch {
                expected_page_digest: expected,
                observed_page_digest: need.page_digest,
            });
        }
        if sender.pending.len() >= MAX_NEEDS_PER_PAGE {
            return self.refuse(Sync2Refusal::RequestMismatch {
                request_id: need.request_id,
            });
        }
        if !sender.seen_request_ids.insert(need.request_id) {
            return self.refuse(Sync2Refusal::RequestMismatch {
                request_id: need.request_id,
            });
        }
        if need.entry_ids.len() > MAX_IDS_PER_NEED {
            return self.refuse(Sync2Refusal::RequestMismatch {
                request_id: need.request_id,
            });
        }
        // Every requested ID must occur in the page and at most once.
        let page: BTreeSet<&[u8]> = sender.page_ids.iter().map(|v| v.as_slice()).collect();
        let mut seen: BTreeSet<&[u8]> = BTreeSet::new();
        for id in &need.entry_ids {
            if !page.contains(id.as_slice()) || !seen.insert(id.as_slice()) {
                return self.refuse(Sync2Refusal::RequestMismatch {
                    request_id: need.request_id,
                });
            }
        }
        sender.pending.push(need);
        Vec::new()
    }

    fn sender_on_needs_complete(&mut self, pnc: PageNeedsComplete) -> Vec<Sync2Action> {
        let phase = self.current_phase();
        let (page_digest, pending, sender_phase, page_done) = {
            let sender = match &mut self.dir {
                DirState::Sending(s) if matches!(s.stage, SenderStage::AwaitingNeeds) => s,
                _ => {
                    return self.refuse(self.unexpected(
                        phase,
                        &[Sync2FrameName::DirectionComplete],
                        Sync2FrameName::PageNeedsComplete,
                    ))
                }
            };
            if pnc.page_digest != sender.page_digest {
                let expected = sender.page_digest;
                return self.refuse(Sync2Refusal::PageMismatch {
                    expected_page_digest: expected,
                    observed_page_digest: pnc.page_digest,
                });
            }
            (
                sender.page_digest,
                std::mem::take(&mut sender.pending),
                sender.phase,
                sender.page_done,
            )
        };
        let mut actions = Vec::new();
        // Serve each request in order.
        for need in &pending {
            let mut items: Vec<(Vec<u8>, Vec<u8>)> = Vec::with_capacity(need.entry_ids.len());
            for id in &need.entry_ids {
                let bytes = match self.snapshot_item(id) {
                    Some(b) => b,
                    None => {
                        return self.refuse(Sync2Refusal::RequestMismatch {
                            request_id: need.request_id,
                        })
                    }
                };
                items.push((id.clone(), bytes));
            }
            let chunks = partition(items);
            let last = chunks.len().saturating_sub(1);
            for (index, chunk) in chunks.into_iter().enumerate() {
                let item_bytes: Vec<Vec<u8>> = chunk.into_iter().map(|(_, b)| b).collect();
                let bundle_bytes = match encode_bundle(&item_bytes) {
                    Ok(b) => b,
                    Err(_) => {
                        return self.refuse(Sync2Refusal::RequestMismatch {
                            request_id: need.request_id,
                        })
                    }
                };
                actions.push(Sync2Action::Send(Sync2Frame::EntriesChunk(EntriesChunk {
                    phase: sender_phase,
                    page_digest,
                    request_id: need.request_id,
                    chunk_index: index as u64,
                    done: index == last,
                    bundle_bytes,
                })));
            }
        }
        actions.push(Sync2Action::Send(Sync2Frame::PageComplete(PageComplete {
            phase: sender_phase,
            page_digest,
        })));
        if page_done {
            if let DirState::Sending(sender) = &mut self.dir {
                sender.stage = SenderStage::AwaitingDirectionComplete;
            }
        } else {
            actions.extend(self.send_next_page());
        }
        actions
    }

    fn snapshot_item(&self, entry_id: &[u8]) -> Option<Vec<u8>> {
        match self.steps.get(self.current) {
            Some((_, PhaseParty::Sender(snap))) => snap.item_bytes(entry_id),
            _ => None,
        }
    }

    fn sender_on_direction_complete(&mut self, dc: DirectionComplete) -> Vec<Sync2Action> {
        let phase = self.current_phase();
        let sender = match &mut self.dir {
            DirState::Sending(s) if matches!(s.stage, SenderStage::AwaitingDirectionComplete) => s,
            _ => {
                return self.refuse(self.unexpected(
                    phase,
                    &[
                        Sync2FrameName::NeedEntries,
                        Sync2FrameName::PageNeedsComplete,
                    ],
                    Sync2FrameName::DirectionComplete,
                ))
            }
        };
        if dc.sender_snapshot_digest != sender.snapshot_digest {
            let expected = sender.snapshot_digest;
            return self.refuse(Sync2Refusal::SnapshotMismatch {
                expected_snapshot_digest: expected,
                observed_snapshot_digest: dc.sender_snapshot_digest,
            });
        }
        self.last_sender_digest = Some(sender.snapshot_digest);
        self.advance_step()
    }

    // --- receiver ----------------------------------------------------------

    fn receiver_on_frame(&mut self, frame: Sync2Frame) -> Vec<Sync2Action> {
        match frame {
            Sync2Frame::SnapshotStart(ss) => self.receiver_on_snapshot_start(ss),
            Sync2Frame::IdsPage(page) => self.receiver_on_page(page),
            Sync2Frame::EntriesChunk(chunk) => self.receiver_on_chunk(chunk),
            Sync2Frame::PageComplete(pc) => self.receiver_on_page_complete(pc),
            other => {
                let phase = self.current_phase();
                let expected = self.receiver_expected_frames();
                self.refuse(self.unexpected(phase, &expected, other.name()))
            }
        }
    }

    fn receiver_expected_frames(&self) -> Vec<Sync2FrameName> {
        match &self.dir {
            DirState::Receiving(r) => match r.stage {
                ReceiverStage::AwaitingSnapshotStart => vec![Sync2FrameName::SnapshotStart],
                ReceiverStage::AwaitingPage => vec![Sync2FrameName::IdsPage],
                ReceiverStage::Receiving => {
                    vec![Sync2FrameName::EntriesChunk, Sync2FrameName::PageComplete]
                }
            },
            _ => Vec::new(),
        }
    }

    fn receiver_on_snapshot_start(&mut self, ss: SnapshotStart) -> Vec<Sync2Action> {
        let phase = self.current_phase();
        let r = match &mut self.dir {
            DirState::Receiving(r) if matches!(r.stage, ReceiverStage::AwaitingSnapshotStart) => r,
            _ => {
                let expected = self.receiver_expected_frames();
                return self.refuse(self.unexpected(
                    phase,
                    &expected,
                    Sync2FrameName::SnapshotStart,
                ));
            }
        };
        if ss.phase != r.phase || ss.namespace_id != r.namespace_id {
            return self.refuse(self.unexpected(
                phase,
                &[Sync2FrameName::SnapshotStart],
                Sync2FrameName::SnapshotStart,
            ));
        }
        r.snapshot_digest = ss.snapshot_digest;
        r.entry_count = ss.entry_count;
        r.logical_bytes = ss.logical_bytes;
        r.stage = ReceiverStage::AwaitingPage;
        Vec::new()
    }

    fn receiver_on_page(&mut self, page: IdsPage) -> Vec<Sync2Action> {
        let phase = self.current_phase();
        // Validate against receiver state, then compute needs.
        let page_digest = {
            let r = match &mut self.dir {
                DirState::Receiving(r) if matches!(r.stage, ReceiverStage::AwaitingPage) => r,
                _ => {
                    let expected = self.receiver_expected_frames();
                    return self.refuse(self.unexpected(phase, &expected, Sync2FrameName::IdsPage));
                }
            };
            if page.phase != r.phase {
                return self.refuse(self.unexpected(
                    phase,
                    &[Sync2FrameName::IdsPage],
                    Sync2FrameName::IdsPage,
                ));
            }
            if page.snapshot_digest != r.snapshot_digest {
                let expected = r.snapshot_digest;
                return self.refuse(Sync2Refusal::SnapshotMismatch {
                    expected_snapshot_digest: expected,
                    observed_snapshot_digest: page.snapshot_digest,
                });
            }
            // Cursor: after_exclusive must equal our tracked cursor and the first
            // ID of the page must be strictly greater than it (and than the last
            // accumulated ID). Pre-clone the cursor so refusal construction does
            // not extend the `r` borrow across `self.refuse`.
            let tracked_cursor = r.cursor.clone();
            let observed_first = page.entry_ids.first().cloned().unwrap_or_default();
            if page.after_exclusive != tracked_cursor {
                return self.refuse(Sync2Refusal::CursorRegression {
                    after_exclusive: tracked_cursor,
                    observed_first_id: observed_first,
                });
            }
            let cursor_regressed = match (&tracked_cursor, page.entry_ids.first()) {
                (Some(cursor), Some(first)) => first.as_slice() <= cursor.as_slice(),
                _ => false,
            };
            let continuity_regressed = match (r.all_ids.last(), page.entry_ids.first()) {
                (Some(last), Some(first)) => first.as_slice() <= last.as_slice(),
                _ => false,
            };
            if cursor_regressed || continuity_regressed {
                return self.refuse(Sync2Refusal::CursorRegression {
                    after_exclusive: tracked_cursor,
                    observed_first_id: observed_first,
                });
            }
            let page_digest = ids_page_digest(&page);
            r.page_ids = page.entry_ids.clone();
            r.page_digest = page_digest;
            r.page_done = page.done;
            r.cursor = page.entry_ids.last().cloned().or_else(|| r.cursor.clone());
            r.all_ids.extend(page.entry_ids.iter().cloned());
            page_digest
        };
        // Compute missing via the stage and build up to four NeedEntries.
        let page_ids = match &self.dir {
            DirState::Receiving(r) => r.page_ids.clone(),
            _ => Vec::new(),
        };
        let missing = self.stage_missing(&page_ids);
        let requests = chunk_needs(&missing);
        let mut actions = Vec::new();
        let phase_tok = match &self.dir {
            DirState::Receiving(r) => r.phase,
            _ => phase,
        };
        for (request_id, ids) in requests.iter().enumerate() {
            let request_id = request_id as u64;
            if let DirState::Receiving(r) = &mut self.dir {
                r.outstanding.insert(
                    request_id,
                    RequestProgress {
                        requested_ids: ids.clone(),
                        next_chunk_index: 0,
                        consumed: 0,
                        done: false,
                    },
                );
            }
            actions.push(Sync2Action::Send(Sync2Frame::NeedEntries(NeedEntries {
                phase: phase_tok,
                page_digest,
                request_id,
                entry_ids: ids.clone(),
            })));
        }
        actions.push(Sync2Action::Send(Sync2Frame::PageNeedsComplete(
            PageNeedsComplete {
                phase: phase_tok,
                page_digest,
            },
        )));
        if let DirState::Receiving(r) = &mut self.dir {
            r.stage = ReceiverStage::Receiving;
        }
        actions
    }

    fn stage_missing(&self, page_ids: &[Vec<u8>]) -> Vec<Vec<u8>> {
        match self.steps.get(self.current) {
            Some((_, PhaseParty::Receiver(stage))) => stage.missing(page_ids),
            _ => Vec::new(),
        }
    }

    fn receiver_on_chunk(&mut self, chunk: EntriesChunk) -> Vec<Sync2Action> {
        let phase = self.current_phase();
        // Validate chunk framing and pull the slice to admit.
        let (request_id, ids_slice, items, done) = {
            let r = match &mut self.dir {
                DirState::Receiving(r) if matches!(r.stage, ReceiverStage::Receiving) => r,
                _ => {
                    let expected = self.receiver_expected_frames();
                    return self.refuse(self.unexpected(
                        phase,
                        &expected,
                        Sync2FrameName::EntriesChunk,
                    ));
                }
            };
            if chunk.phase != r.phase {
                return self.refuse(self.unexpected(
                    phase,
                    &[Sync2FrameName::EntriesChunk],
                    Sync2FrameName::EntriesChunk,
                ));
            }
            if chunk.page_digest != r.page_digest {
                let expected = r.page_digest;
                return self.refuse(Sync2Refusal::PageMismatch {
                    expected_page_digest: expected,
                    observed_page_digest: chunk.page_digest,
                });
            }
            let progress = match r.outstanding.get_mut(&chunk.request_id) {
                Some(p) if !p.done => p,
                _ => {
                    return self.refuse(Sync2Refusal::RequestMismatch {
                        request_id: chunk.request_id,
                    })
                }
            };
            if chunk.chunk_index != progress.next_chunk_index {
                let expected_index = progress.next_chunk_index;
                return self.refuse(Sync2Refusal::ChunkMismatch {
                    request_id: chunk.request_id,
                    expected_index,
                    observed_index: chunk.chunk_index,
                });
            }
            let items = match super::decode_bundle(&chunk.bundle_bytes) {
                Ok(items) => items,
                Err(_) => {
                    return self.refuse(Sync2Refusal::AdmissionFailed {
                        subject: AdmissionSubject::Bundle,
                    })
                }
            };
            let remaining = progress.requested_ids.len() - progress.consumed;
            if items.len() > remaining || (chunk.done && items.len() != remaining) {
                return self.refuse(Sync2Refusal::RequestMismatch {
                    request_id: chunk.request_id,
                });
            }
            let ids_slice: Vec<Vec<u8>> =
                progress.requested_ids[progress.consumed..progress.consumed + items.len()].to_vec();
            progress.consumed += items.len();
            progress.next_chunk_index += 1;
            progress.done = chunk.done;
            (chunk.request_id, ids_slice, items, chunk.done)
        };
        let _ = (request_id, done);
        // Admit into the direction-private stage (short transaction).
        if let Some((_, PhaseParty::Receiver(stage))) = self.steps.get_mut(self.current) {
            if let Err(subject) = stage.admit(&ids_slice, &items) {
                return self.refuse(Sync2Refusal::AdmissionFailed { subject });
            }
        }
        vec![Sync2Action::Admit(chunk)]
    }

    fn receiver_on_page_complete(&mut self, pc: PageComplete) -> Vec<Sync2Action> {
        let phase = self.current_phase();
        let (page_done, sender_digest, sender_phase) = {
            let r = match &mut self.dir {
                DirState::Receiving(r) if matches!(r.stage, ReceiverStage::Receiving) => r,
                _ => {
                    let expected = self.receiver_expected_frames();
                    return self.refuse(self.unexpected(
                        phase,
                        &expected,
                        Sync2FrameName::PageComplete,
                    ));
                }
            };
            if pc.page_digest != r.page_digest {
                let expected = r.page_digest;
                return self.refuse(Sync2Refusal::PageMismatch {
                    expected_page_digest: expected,
                    observed_page_digest: pc.page_digest,
                });
            }
            // Every outstanding request must be fully delivered.
            for progress in r.outstanding.values() {
                if !progress.done || progress.consumed != progress.requested_ids.len() {
                    let request_id = *r
                        .outstanding
                        .iter()
                        .find(|(_, p)| !p.done || p.consumed != p.requested_ids.len())
                        .map(|(id, _)| id)
                        .unwrap_or(&0);
                    return self.refuse(Sync2Refusal::RequestMismatch { request_id });
                }
            }
            r.outstanding.clear();
            (r.page_done, r.snapshot_digest, r.phase)
        };
        if !page_done {
            if let DirState::Receiving(r) = &mut self.dir {
                r.stage = ReceiverStage::AwaitingPage;
            }
            return Vec::new();
        }
        // Final page: verify the advertised inventory digest from the ID stream.
        let (all_ids, entry_count, logical_bytes, namespace_id) = match &self.dir {
            DirState::Receiving(r) => (
                r.all_ids.clone(),
                r.entry_count,
                r.logical_bytes,
                r.namespace_id,
            ),
            _ => return Vec::new(),
        };
        if all_ids.len() as u64 != entry_count {
            return self.refuse(Sync2Refusal::SnapshotMismatch {
                expected_snapshot_digest: sender_digest,
                observed_snapshot_digest: [0u8; 32],
            });
        }
        let id_refs: Vec<&[u8]> = all_ids.iter().map(|v| v.as_slice()).collect();
        let recomputed = sync_snapshot_digest(&namespace_id, entry_count, logical_bytes, &id_refs);
        if recomputed != sender_digest {
            return self.refuse(Sync2Refusal::SnapshotMismatch {
                expected_snapshot_digest: sender_digest,
                observed_snapshot_digest: recomputed,
            });
        }
        self.last_receiver_verified = Some(sender_digest);
        // Promote the direction stage into its parent operation.
        if let Some((_, PhaseParty::Receiver(stage))) = self.steps.get_mut(self.current) {
            stage.promote();
        }
        let mut actions = vec![
            Sync2Action::PromoteDirection,
            Sync2Action::Send(Sync2Frame::DirectionComplete(DirectionComplete {
                phase: sender_phase,
                sender_snapshot_digest: sender_digest,
            })),
        ];
        actions.extend(self.advance_step());
        actions
    }
}

/// Partition requested items (in requested order) into legal chunks: at most 64
/// entries and 8 MiB each, at least one item per chunk.
fn partition(items: Vec<(Vec<u8>, Vec<u8>)>) -> Vec<Vec<(Vec<u8>, Vec<u8>)>> {
    let mut chunks: Vec<Vec<(Vec<u8>, Vec<u8>)>> = Vec::new();
    let mut cur: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
    let mut cur_bytes = 0usize;
    for (id, bytes) in items {
        let would = cur_bytes + bytes.len();
        if !cur.is_empty()
            && (cur.len() >= MAX_ENTRIES_PER_CHUNK || would > super::MAX_CHUNK_BUNDLE_BYTES)
        {
            chunks.push(std::mem::take(&mut cur));
            cur_bytes = 0;
        }
        cur_bytes += bytes.len();
        cur.push((id, bytes));
    }
    if !cur.is_empty() {
        chunks.push(cur);
    }
    chunks
}

/// Split missing IDs into at most four requests of at most 64 IDs, preserving
/// page order.
fn chunk_needs(missing: &[Vec<u8>]) -> Vec<Vec<Vec<u8>>> {
    let mut requests: Vec<Vec<Vec<u8>>> = Vec::new();
    for group in missing.chunks(MAX_IDS_PER_NEED) {
        if requests.len() >= MAX_NEEDS_PER_PAGE {
            break;
        }
        requests.push(group.to_vec());
    }
    requests
}
