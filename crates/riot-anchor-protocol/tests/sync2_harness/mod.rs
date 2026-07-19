//! Shared in-memory harness for the `sync/2` FSM and duplex tests.
//!
//! Provides concrete [`Sync2Snapshot`], [`Sync2DirectionStage`], and
//! [`Sync2Repository`] implementations plus a duplex driver that pumps two
//! [`Sync2Session`]s against each other entirely in memory (no IO), exercising the
//! transport-independent FSM.

#![allow(dead_code)]

use std::collections::BTreeMap;

use riot_anchor_protocol::sync2::{
    compute_snapshot_digest, AdmissionSubject, OpenNamespace, OpenedNamespace, PhaseParty,
    Sync2Action, Sync2DirectionStage, Sync2Frame, Sync2ModeTag, Sync2Phase, Sync2Refusal,
    Sync2Repository, Sync2Session, Sync2Snapshot,
};

/// One `(entry_id, canonical_item_bytes)` pair.
pub type Item = (Vec<u8>, Vec<u8>);

/// Deterministic entry ID for a numeric index (4-byte big-endian: fixed width so
/// numeric order equals lexicographic order).
pub fn id(index: u32) -> Vec<u8> {
    index.to_be_bytes().to_vec()
}

/// Deterministic canonical item bytes for an index (non-empty).
pub fn item_bytes(index: u32) -> Vec<u8> {
    let mut v = vec![0xA0u8];
    v.extend_from_slice(&index.to_be_bytes());
    v.extend_from_slice(&[index as u8; 3]);
    v
}

/// Build `count` items numbered `[start, start+count)`.
pub fn items(start: u32, count: u32) -> Vec<Item> {
    (start..start + count)
        .map(|i| (id(i), item_bytes(i)))
        .collect()
}

fn logical_bytes(items: &[Item]) -> u64 {
    items.iter().map(|(_, b)| b.len() as u64).sum()
}

/// An immutable in-memory snapshot.
pub struct MemSnapshot {
    namespace_id: [u8; 32],
    entries: BTreeMap<Vec<u8>, Vec<u8>>,
    logical_bytes: u64,
}

impl MemSnapshot {
    pub fn new(namespace_id: [u8; 32], items: Vec<Item>) -> Self {
        let logical = logical_bytes(&items);
        MemSnapshot {
            namespace_id,
            entries: items.into_iter().collect(),
            logical_bytes: logical,
        }
    }
}

impl Sync2Snapshot for MemSnapshot {
    fn snapshot_digest(&self) -> [u8; 32] {
        let ids: Vec<Vec<u8>> = self.entries.keys().cloned().collect();
        compute_snapshot_digest(&self.namespace_id, self.logical_bytes, &ids)
    }
    fn entry_count(&self) -> u64 {
        self.entries.len() as u64
    }
    fn logical_bytes(&self) -> u64 {
        self.logical_bytes
    }
    fn sorted_entry_ids(&self) -> Vec<Vec<u8>> {
        self.entries.keys().cloned().collect()
    }
    fn item_bytes(&self, entry_id: &[u8]) -> Option<Vec<u8>> {
        self.entries.get(entry_id).cloned()
    }
}

/// A direction-private in-memory stage.
pub struct MemStage {
    namespace_id: [u8; 32],
    base: BTreeMap<Vec<u8>, Vec<u8>>,
    admitted: BTreeMap<Vec<u8>, Vec<u8>>,
    fail_admit: Option<AdmissionSubject>,
    promoted: bool,
}

impl MemStage {
    pub fn new(
        namespace_id: [u8; 32],
        base: Vec<Item>,
        fail_admit: Option<AdmissionSubject>,
    ) -> Self {
        MemStage {
            namespace_id,
            base: base.into_iter().collect(),
            admitted: BTreeMap::new(),
            fail_admit,
            promoted: false,
        }
    }
}

impl Sync2DirectionStage for MemStage {
    fn missing(&self, page_ids: &[Vec<u8>]) -> Vec<Vec<u8>> {
        page_ids
            .iter()
            .filter(|id| !self.base.contains_key(*id) && !self.admitted.contains_key(*id))
            .cloned()
            .collect()
    }
    fn admit(&mut self, entry_ids: &[Vec<u8>], items: &[Vec<u8>]) -> Result<(), AdmissionSubject> {
        if let Some(subject) = self.fail_admit {
            return Err(subject);
        }
        for (id, bytes) in entry_ids.iter().zip(items.iter()) {
            self.admitted.insert(id.clone(), bytes.clone());
        }
        Ok(())
    }
    fn resulting_digest(&self, namespace_id: &[u8; 32]) -> [u8; 32] {
        let mut union: BTreeMap<Vec<u8>, Vec<u8>> = self.base.clone();
        for (k, v) in &self.admitted {
            union.insert(k.clone(), v.clone());
        }
        let logical: u64 = union.values().map(|b| b.len() as u64).sum();
        let ids: Vec<Vec<u8>> = union.keys().cloned().collect();
        compute_snapshot_digest(namespace_id, logical, &ids)
    }
    fn promote(&mut self) {
        self.promoted = true;
    }
}

/// A phase's role specification, cloneable so a repository can build fresh parties.
#[derive(Clone)]
pub enum RoleSpec {
    /// This endpoint sends an immutable snapshot of `items`.
    Sender(Vec<Item>),
    /// This endpoint receives into a stage over `base` (optionally failing admit).
    Receiver {
        /// The stage's committed base.
        base: Vec<Item>,
        /// If set, every admit fails with this subject.
        fail_admit: Option<AdmissionSubject>,
    },
}

/// A configurable in-memory repository for one endpoint.
#[derive(Clone)]
pub struct HarnessRepo {
    pub namespace_id: [u8; 32],
    pub mode: Sync2ModeTag,
    pub plan: Vec<(Sync2Phase, RoleSpec)>,
    pub open_error: Option<Sync2Refusal>,
    pub stale_source: Option<Sync2Refusal>,
}

impl HarnessRepo {
    pub fn new(namespace_id: [u8; 32], mode: Sync2ModeTag) -> Self {
        HarnessRepo {
            namespace_id,
            mode,
            plan: Vec::new(),
            open_error: None,
            stale_source: None,
        }
    }
    pub fn with_plan(mut self, plan: Vec<(Sync2Phase, RoleSpec)>) -> Self {
        self.plan = plan;
        self
    }
    pub fn with_open_error(mut self, refusal: Sync2Refusal) -> Self {
        self.open_error = Some(refusal);
        self
    }
    pub fn with_stale_source(mut self, refusal: Sync2Refusal) -> Self {
        self.stale_source = Some(refusal);
        self
    }
}

impl Sync2Repository for HarnessRepo {
    type Snapshot = MemSnapshot;
    type DirectionStage = MemStage;
    fn open_namespace(
        &self,
        _request: &OpenNamespace,
    ) -> Result<OpenedNamespace<Self>, Sync2Refusal> {
        if let Some(err) = &self.open_error {
            return Err(err.clone());
        }
        let parties =
            self.plan
                .iter()
                .map(|(phase, spec)| {
                    let party = match spec {
                        RoleSpec::Sender(items) => {
                            PhaseParty::Sender(MemSnapshot::new(self.namespace_id, items.clone()))
                        }
                        RoleSpec::Receiver { base, fail_admit } => PhaseParty::Receiver(
                            MemStage::new(self.namespace_id, base.clone(), *fail_admit),
                        ),
                    };
                    (*phase, party)
                })
                .collect();
        Ok(OpenedNamespace {
            namespace_id: self.namespace_id,
            mode: self.mode,
            parties,
            stale_source: self.stale_source.clone(),
        })
    }
}

/// The outcome of a duplex run.
pub struct DuplexReport {
    pub anchor_complete: bool,
    pub client_complete: bool,
    pub anchor_refusal: Option<Sync2Refusal>,
    pub client_refusal: Option<Sync2Refusal>,
    pub admits: usize,
    pub promotions: usize,
    pub frames: usize,
}

/// A test namespace ID.
pub fn ns(seed: u8) -> [u8; 32] {
    [seed; 32]
}

/// Build a standard `OpenNamespace` for the given mode.
pub fn open_namespace(
    namespace_id: [u8; 32],
    mode: riot_anchor_protocol::sync2::Sync2Mode,
) -> OpenNamespace {
    OpenNamespace {
        protocol_version: 2,
        session_id: vec![1, 2, 3, 4],
        ticket_core_bytes: vec![0x9u8; 40],
        namespace_id,
        mode,
    }
}

/// Drive two sessions against each other until quiescent. `anchor` is the
/// responder; `client` the initiator. Frames emitted by one are delivered to the
/// other in FIFO order.
pub fn run_duplex(
    mut anchor: Sync2Session<HarnessRepo>,
    mut client: Sync2Session<HarnessRepo>,
) -> DuplexReport {
    // A queued frame and its recipient (`true` => deliver to anchor).
    let mut queue: Vec<(bool, Sync2Frame)> = Vec::new();
    let mut report = DuplexReport {
        anchor_complete: false,
        client_complete: false,
        anchor_refusal: None,
        client_refusal: None,
        admits: 0,
        promotions: 0,
        frames: 0,
    };

    let absorb = |actions: Vec<Sync2Action>,
                  to_anchor: bool,
                  queue: &mut Vec<(bool, Sync2Frame)>,
                  report: &mut DuplexReport| {
        for action in actions {
            match action {
                Sync2Action::Send(frame) => {
                    report.frames += 1;
                    queue.push((to_anchor, frame));
                }
                Sync2Action::Admit(_) => report.admits += 1,
                Sync2Action::PromoteDirection => report.promotions += 1,
                Sync2Action::Complete => {}
            }
        }
    };

    let start = client.start();
    absorb(start, true, &mut queue, &mut report);

    let mut guard = 0;
    while let Some((to_anchor, frame)) = pop_front(&mut queue) {
        guard += 1;
        assert!(guard < 100_000, "duplex did not converge");
        let actions = if to_anchor {
            anchor.on_frame(frame)
        } else {
            client.on_frame(frame)
        };
        absorb(actions, !to_anchor, &mut queue, &mut report);
    }

    report.anchor_complete = anchor.is_complete();
    report.client_complete = client.is_complete();
    report.anchor_refusal = anchor.refusal().cloned();
    report.client_refusal = client.refusal().cloned();
    report
}

fn pop_front(queue: &mut Vec<(bool, Sync2Frame)>) -> Option<(bool, Sync2Frame)> {
    if queue.is_empty() {
        None
    } else {
        Some(queue.remove(0))
    }
}
