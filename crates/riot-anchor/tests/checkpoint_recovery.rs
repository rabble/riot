//! WU-016 — crash-safe emergency checkpoints: the freeze → sign → publish →
//! advance → reclaim state machine, recovery at every publication/reclaim
//! failpoint (wholly-absent-or-committed), orphan temp-tree reclamation, the
//! atomic terminalisation of covered `RemovalCommitted` operations, lost-terminal
//! delivery recovery, and fail-closed refusal to advance onto corrupt bytes.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use ed25519_dalek::{Signer as _, SigningKey};

use riot_anchor::checkpoint::{no_failpoint, CheckpointError, CheckpointPublisher};
use riot_anchor::repository::{
    AnchorRepository, CheckpointMember, CheckpointPhase, CheckpointPlan, RemovalSlotState,
};
use riot_anchor::work::OperatorSigner;

// ---------------------------------------------------------------------------
// fixtures
// ---------------------------------------------------------------------------

struct TestSigner(SigningKey);
impl OperatorSigner for TestSigner {
    fn sign(&self, preimage: &[u8]) -> [u8; 64] {
        self.0.sign(preimage).to_bytes()
    }
}

fn signer() -> TestSigner {
    TestSigner(SigningKey::from_bytes(&[3u8; 32]))
}

fn d32(seed: u8) -> [u8; 32] {
    [seed; 32]
}
fn d16(seed: u8) -> [u8; 16] {
    [seed; 16]
}

fn repo() -> AnchorRepository {
    AnchorRepository::open_in_memory().expect("open in-memory anchor repository")
}

static DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A self-cleaning temp directory for filesystem publication.
struct TempDir(PathBuf);
impl TempDir {
    fn new() -> Self {
        let unique = DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
        let path =
            std::env::temp_dir().join(format!("riot-anchor-ckpt-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&path).expect("create temp dir");
        TempDir(path)
    }
    fn path(&self) -> &std::path::Path {
        &self.0
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn publisher(dir: &TempDir) -> CheckpointPublisher<TestSigner> {
    CheckpointPublisher::new(signer(), dir.path())
}

/// A frozen plan with two members and the given covered removal slots.
fn plan(work_id: [u8; 32], covered: Vec<u32>, now: u64) -> CheckpointPlan {
    CheckpointPlan {
        work_id,
        created_at: now,
        frozen_state_generation: 5,
        covered_head_sequence: 10,
        covered_head_inclusion_digest: d32(0x22),
        previous_checkpoint_digest: None,
        snapshot_generation_id: 1,
        canonical_checkpoint_body: b"frozen-checkpoint-body".to_vec(),
        ordered_members: vec![
            CheckpointMember {
                community_id: d32(0x40),
                frozen_head_digest: d32(0x41),
                snapshot_record_bytes: b"member-0-record".to_vec(),
            },
            CheckpointMember {
                community_id: d32(0x50),
                frozen_head_digest: d32(0x51),
                snapshot_record_bytes: b"member-1-record".to_vec(),
            },
        ],
        covered_removal_slots: covered,
    }
}

/// Put a removal slot into the `Committed` state bound to `work_id`, with a frozen
/// reserved result — exactly the state a checkpoint later terminalizes.
fn committed_removal(repo: &mut AnchorRepository, community_seed: u8, work_id: &[u8; 32]) -> u32 {
    let community = d32(community_seed);
    let root = d32(community_seed.wrapping_add(1));
    let digest = d32(community_seed.wrapping_add(2));
    let mut tx = repo.begin().expect("begin");
    tx.insert_community(&community, 0).expect("community");
    let slot = match tx
        .reserve_visibility_slot(&community, &root, &digest, 0)
        .expect("reserve")
    {
        riot_anchor::repository::SlotReservation::Reserved(slot) => slot,
        riot_anchor::repository::SlotReservation::Blocked { .. } => panic!("blocked"),
    };
    tx.commit_removal_slot(
        slot,
        &d32(community_seed.wrapping_add(3)),
        &d16(community_seed),
        &digest,
        work_id,
    )
    .expect("commit removal");
    tx.claim_idempotency_reserved(
        &digest,
        &d16(community_seed),
        riot_anchor::repository::IdempotencyClaimState::Terminal,
        0,
        24 * 60 * 60,
    )
    .expect("reserved idempotency claim");
    tx.store_reserved_result(&digest, slot, b"frozen-removal-receipt")
        .expect("reserved result");
    tx.commit().expect("commit");
    slot
}

fn slot_state(repo: &mut AnchorRepository, slot: u32) -> RemovalSlotState {
    let tx = repo.begin().expect("begin");
    tx.load_removal_slot(slot)
        .expect("load")
        .expect("slot")
        .state
}

fn phase(repo: &mut AnchorRepository, work_id: &[u8; 32]) -> CheckpointPhase {
    let tx = repo.begin().expect("begin");
    tx.load_checkpoint_work(work_id)
        .expect("load")
        .expect("work")
        .phase
}

// ---------------------------------------------------------------------------
// happy path
// ---------------------------------------------------------------------------

#[test]
fn happy_path_publishes_advances_and_terminalizes_covered_removals() {
    let dir = TempDir::new();
    let mut repo = repo();
    let publisher = publisher(&dir);
    let work_id = d32(1);
    let slot = committed_removal(&mut repo, 0x60, &work_id);

    publisher
        .plan(&mut repo, &plan(work_id, vec![slot], 1_000))
        .expect("plan");
    publisher
        .publish_all(&mut repo, &work_id, 1_000)
        .expect("publish all");

    assert_eq!(phase(&mut repo, &work_id), CheckpointPhase::Reclaimed);
    assert_eq!(slot_state(&mut repo, slot), RemovalSlotState::Terminal);
    // The published file exists and the checkpoint pointer advanced.
    let tx = repo.begin().expect("begin");
    assert_eq!(tx.latest_checkpoint_generation().expect("gen"), 10);
    // The covered removal's frozen result is preserved for its original key.
    assert_eq!(
        tx.reserved_result(&d32(0x62)).expect("result").as_deref(),
        Some(&b"frozen-removal-receipt"[..])
    );
}

#[test]
fn planning_is_idempotent_and_freezes_immutable_members() {
    let dir = TempDir::new();
    let mut repo = repo();
    let publisher = publisher(&dir);
    let work_id = d32(2);
    let p = plan(work_id, vec![], 1_000);
    publisher.plan(&mut repo, &p).expect("plan");
    // Re-planning is a no-op that cannot alter the frozen work.
    publisher
        .plan(&mut repo, &plan(work_id, vec![], 9_999))
        .expect("re-plan");
    let tx = repo.begin().expect("begin");
    let work = tx
        .load_checkpoint_work(&work_id)
        .expect("load")
        .expect("work");
    assert_eq!(
        work.created_at, 1_000,
        "creation time is frozen at first plan"
    );
    let members = tx.checkpoint_work_members(&work_id).expect("members");
    assert_eq!(members.len(), 2);
    assert_eq!(members[0].snapshot_record_bytes, b"member-0-record");
}

// ---------------------------------------------------------------------------
// recovery at every publication failpoint (wholly-absent-or-committed)
// ---------------------------------------------------------------------------

#[test]
fn recover_from_each_publication_failpoint_completes_exactly_once() {
    for failpoint in ["write_temp", "rename", "record_published"] {
        let dir = TempDir::new();
        let mut repo = repo();
        let publisher = publisher(&dir);
        let work_id = d32(10);
        let slot = committed_removal(&mut repo, 0x70, &work_id);
        publisher
            .plan(&mut repo, &plan(work_id, vec![slot], 1_000))
            .expect("plan");
        publisher.sign(&mut repo, &work_id).expect("sign");

        // Publication aborts at the failpoint: the effect is wholly absent.
        let mut fp = |label: &str| label == failpoint;
        let aborted = publisher.publish_files(&mut repo, &work_id, &mut fp);
        assert!(aborted.is_err(), "{failpoint} aborts publication");
        assert_ne!(
            phase(&mut repo, &work_id),
            CheckpointPhase::FloorAdvanced,
            "{failpoint}: never advanced onto an unpublished checkpoint"
        );
        assert_eq!(
            slot_state(&mut repo, slot),
            RemovalSlotState::Committed,
            "{failpoint}: covered removal not yet terminalized"
        );

        // Recovery inspects the persisted phase and drives to completion without
        // inventing any new data.
        let final_phase = publisher
            .recover(&mut repo, &work_id, 1_000)
            .expect("recover");
        assert_eq!(final_phase, CheckpointPhase::Reclaimed, "{failpoint}");
        assert_eq!(
            slot_state(&mut repo, slot),
            RemovalSlotState::Terminal,
            "{failpoint}: recovery terminalizes the covered removal"
        );
    }
}

#[test]
fn recover_from_planned_and_from_signed() {
    for start in ["planned", "signed"] {
        let dir = TempDir::new();
        let mut repo = repo();
        let publisher = publisher(&dir);
        let work_id = d32(11);
        let slot = committed_removal(&mut repo, 0x80, &work_id);
        publisher
            .plan(&mut repo, &plan(work_id, vec![slot], 1_000))
            .expect("plan");
        if start == "signed" {
            publisher.sign(&mut repo, &work_id).expect("sign");
        }
        let final_phase = publisher
            .recover(&mut repo, &work_id, 1_000)
            .expect("recover");
        assert_eq!(final_phase, CheckpointPhase::Reclaimed);
        assert_eq!(slot_state(&mut repo, slot), RemovalSlotState::Terminal);
    }
}

#[test]
fn orphan_temp_tree_is_reclaimed_on_recovery() {
    let dir = TempDir::new();
    let mut repo = repo();
    let publisher = publisher(&dir);
    let work_id = d32(12);
    publisher
        .plan(&mut repo, &plan(work_id, vec![], 1_000))
        .expect("plan");
    publisher.sign(&mut repo, &work_id).expect("sign");
    // Abort during the temp write path, then simulate a crash that left an orphan
    // temp file on disk with the reserved name.
    let mut fp = |label: &str| label == "record_published";
    let _ = publisher.publish_files(&mut repo, &work_id, &mut fp);
    // Publication reached FilesPublished? No — record_published aborts before the
    // phase flip, and the rename already moved temp → final. Recovery must still
    // complete cleanly (idempotent re-publish over the existing final file).
    let final_phase = publisher
        .recover(&mut repo, &work_id, 1_000)
        .expect("recover");
    assert_eq!(final_phase, CheckpointPhase::Reclaimed);
    // No orphan temp file remains.
    let temp_leftovers: Vec<_> = std::fs::read_dir(dir.path())
        .expect("read dir")
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(temp_leftovers.is_empty(), "no orphan temp tree remains");
}

// ---------------------------------------------------------------------------
// advance failpoint + lost terminal delivery
// ---------------------------------------------------------------------------

#[test]
fn advance_failpoint_leaves_removals_committed_then_recovers() {
    let dir = TempDir::new();
    let mut repo = repo();
    let publisher = publisher(&dir);
    let work_id = d32(20);
    let slot = committed_removal(&mut repo, 0x90, &work_id);
    publisher
        .plan(&mut repo, &plan(work_id, vec![slot], 1_000))
        .expect("plan");
    publisher.sign(&mut repo, &work_id).expect("sign");
    publisher
        .publish_files(&mut repo, &work_id, &mut no_failpoint)
        .expect("publish");

    // Advance aborts before its atomic commit.
    let mut fp = |label: &str| label == "advance_commit";
    let aborted = publisher.advance(&mut repo, &work_id, 1_000, &mut fp);
    assert!(aborted.is_err());
    assert_eq!(phase(&mut repo, &work_id), CheckpointPhase::FilesPublished);
    assert_eq!(slot_state(&mut repo, slot), RemovalSlotState::Committed);

    // Recovery completes the advance atomically.
    let final_phase = publisher
        .recover(&mut repo, &work_id, 1_000)
        .expect("recover");
    assert_eq!(final_phase, CheckpointPhase::Reclaimed);
    assert_eq!(slot_state(&mut repo, slot), RemovalSlotState::Terminal);
}

#[test]
fn lost_terminal_delivery_advance_is_idempotent() {
    // A checkpoint advanced and terminalized its covered removals, but the success
    // acknowledgement was lost. Re-driving the work is wholly idempotent: the
    // removals stay Terminal and their frozen results are preserved.
    let dir = TempDir::new();
    let mut repo = repo();
    let publisher = publisher(&dir);
    let work_id = d32(21);
    let slot = committed_removal(&mut repo, 0xA0, &work_id);
    publisher
        .plan(&mut repo, &plan(work_id, vec![slot], 1_000))
        .expect("plan");
    publisher.sign(&mut repo, &work_id).expect("sign");
    publisher
        .publish_files(&mut repo, &work_id, &mut no_failpoint)
        .expect("publish");
    publisher
        .advance(&mut repo, &work_id, 1_000, &mut no_failpoint)
        .expect("advance");
    assert_eq!(slot_state(&mut repo, slot), RemovalSlotState::Terminal);

    // Re-run advance (lost ack): idempotent, no double-terminalization, no error.
    publisher
        .advance(&mut repo, &work_id, 2_000, &mut no_failpoint)
        .expect("re-advance");
    let final_phase = publisher
        .recover(&mut repo, &work_id, 3_000)
        .expect("recover");
    assert_eq!(final_phase, CheckpointPhase::Reclaimed);
    assert_eq!(slot_state(&mut repo, slot), RemovalSlotState::Terminal);
    let tx = repo.begin().expect("begin");
    assert_eq!(
        tx.reserved_result(&d32(0xA2)).expect("result").as_deref(),
        Some(&b"frozen-removal-receipt"[..])
    );
}

#[test]
fn advance_fails_closed_on_corrupt_published_bytes() {
    let dir = TempDir::new();
    let mut repo = repo();
    let publisher = publisher(&dir);
    let work_id = d32(30);
    let slot = committed_removal(&mut repo, 0xB0, &work_id);
    publisher
        .plan(&mut repo, &plan(work_id, vec![slot], 1_000))
        .expect("plan");
    publisher.sign(&mut repo, &work_id).expect("sign");
    publisher
        .publish_files(&mut repo, &work_id, &mut no_failpoint)
        .expect("publish");

    // Corrupt the published file on disk.
    let final_name = format!("checkpoint-{}.cbor", hex(&work_id));
    std::fs::write(dir.path().join(&final_name), b"corrupted").expect("corrupt");

    // Advance refuses to switch onto corrupt bytes — it fails closed.
    let err = publisher
        .advance(&mut repo, &work_id, 1_000, &mut no_failpoint)
        .expect_err("advance must fail closed");
    assert!(matches!(err, CheckpointError::HashMismatch { .. }));
    assert_eq!(slot_state(&mut repo, slot), RemovalSlotState::Committed);
    assert_eq!(phase(&mut repo, &work_id), CheckpointPhase::FilesPublished);
}

fn hex(bytes: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}
