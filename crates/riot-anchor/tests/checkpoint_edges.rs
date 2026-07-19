//! Edge coverage for `checkpoint.rs`: the phase-guard refusal arms (publish
//! before sign, advance before publish, reclaim before advance), the idempotent
//! early returns (sign/publish/reclaim past their phase, recover on a fully
//! reclaimed work), the "feed head never regresses" branch, and the `Display` /
//! `From` surface of every `CheckpointError` variant. Each error value is
//! obtained from the public API (the enum is `#[non_exhaustive]` and cannot be
//! constructed from an integration test).

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use ed25519_dalek::{Signer as _, SigningKey};

use riot_anchor::checkpoint::{no_failpoint, CheckpointError, CheckpointPublisher};
use riot_anchor::repository::{
    AnchorRepository, CheckpointMember, CheckpointPhase, CheckpointPlan, RemovalSlotState,
};
use riot_anchor::work::OperatorSigner;

// ---------------------------------------------------------------------------
// fixtures (mirrored from checkpoint_recovery.rs)
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
        let path = std::env::temp_dir().join(format!(
            "riot-anchor-ckpt-edge-{}-{unique}",
            std::process::id()
        ));
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

/// Put a removal slot into the `Committed` state bound to `work_id`.
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

fn feed_length(repo: &mut AnchorRepository) -> u64 {
    let tx = repo.begin().expect("begin");
    tx.feed_head().expect("feed head").1
}

// ---------------------------------------------------------------------------
// phase-guard refusal arms
// ---------------------------------------------------------------------------

#[test]
fn publish_before_sign_is_refused() {
    let dir = TempDir::new();
    let mut repo = repo();
    let publisher = publisher(&dir);
    let work_id = d32(1);
    publisher
        .plan(&mut repo, &plan(work_id, vec![], 1_000))
        .expect("plan");

    // Publishing a work that is still `Planned` (never signed) is refused, and
    // nothing is written to disk.
    let err = publisher
        .publish_files(&mut repo, &work_id, &mut no_failpoint)
        .expect_err("publish before sign must refuse");
    assert!(matches!(
        err,
        CheckpointError::Failpoint("publish before sign")
    ));
    assert_eq!(phase(&mut repo, &work_id), CheckpointPhase::Planned);
    let leftovers: Vec<_> = std::fs::read_dir(dir.path())
        .expect("read dir")
        .filter_map(Result::ok)
        .collect();
    assert!(leftovers.is_empty(), "no files written before signing");
}

#[test]
fn advance_before_publish_is_refused() {
    let dir = TempDir::new();
    let mut repo = repo();
    let publisher = publisher(&dir);
    let work_id = d32(2);
    let slot = committed_removal(&mut repo, 0x60, &work_id);
    publisher
        .plan(&mut repo, &plan(work_id, vec![slot], 1_000))
        .expect("plan");
    publisher.sign(&mut repo, &work_id).expect("sign");

    // The work is only `Signed`; advancing the floor before the file is published
    // is refused and the covered removal stays committed.
    let err = publisher
        .advance(&mut repo, &work_id, 1_000, &mut no_failpoint)
        .expect_err("advance before publish must refuse");
    assert!(matches!(
        err,
        CheckpointError::Failpoint("advance before publish")
    ));
    assert_eq!(phase(&mut repo, &work_id), CheckpointPhase::Signed);
    assert_eq!(slot_state(&mut repo, slot), RemovalSlotState::Committed);
}

#[test]
fn reclaim_before_advance_is_refused() {
    let dir = TempDir::new();
    let mut repo = repo();
    let publisher = publisher(&dir);
    let work_id = d32(3);
    publisher
        .plan(&mut repo, &plan(work_id, vec![], 1_000))
        .expect("plan");

    // Reclaiming physical rows before the floor has advanced is refused.
    let err = publisher
        .reclaim(&mut repo, &work_id)
        .expect_err("reclaim before advance must refuse");
    assert!(matches!(
        err,
        CheckpointError::Failpoint("reclaim before advance")
    ));
    assert_eq!(phase(&mut repo, &work_id), CheckpointPhase::Planned);
}

// ---------------------------------------------------------------------------
// idempotent early returns past a phase
// ---------------------------------------------------------------------------

#[test]
fn sign_is_idempotent_past_signed() {
    let dir = TempDir::new();
    let mut repo = repo();
    let publisher = publisher(&dir);
    let work_id = d32(4);
    publisher
        .plan(&mut repo, &plan(work_id, vec![], 1_000))
        .expect("plan");
    publisher.sign(&mut repo, &work_id).expect("sign");
    assert_eq!(phase(&mut repo, &work_id), CheckpointPhase::Signed);

    // Re-signing an already-signed work is a no-op that does not error or
    // re-sign (phase stays `Signed`).
    publisher.sign(&mut repo, &work_id).expect("re-sign");
    assert_eq!(phase(&mut repo, &work_id), CheckpointPhase::Signed);
}

#[test]
fn publish_is_idempotent_past_files_published() {
    let dir = TempDir::new();
    let mut repo = repo();
    let publisher = publisher(&dir);
    let work_id = d32(5);
    publisher
        .plan(&mut repo, &plan(work_id, vec![], 1_000))
        .expect("plan");
    publisher.sign(&mut repo, &work_id).expect("sign");
    publisher
        .publish_files(&mut repo, &work_id, &mut no_failpoint)
        .expect("publish");
    assert_eq!(phase(&mut repo, &work_id), CheckpointPhase::FilesPublished);

    // Re-publishing an already-published work returns Ok without changing phase.
    publisher
        .publish_files(&mut repo, &work_id, &mut no_failpoint)
        .expect("re-publish");
    assert_eq!(phase(&mut repo, &work_id), CheckpointPhase::FilesPublished);
}

#[test]
fn reclaim_is_idempotent_past_reclaimed() {
    let dir = TempDir::new();
    let mut repo = repo();
    let publisher = publisher(&dir);
    let work_id = d32(6);
    publisher
        .plan(&mut repo, &plan(work_id, vec![], 1_000))
        .expect("plan");
    publisher
        .publish_all(&mut repo, &work_id, 1_000)
        .expect("publish all");
    assert_eq!(phase(&mut repo, &work_id), CheckpointPhase::Reclaimed);

    // Re-reclaiming an already-reclaimed work returns Ok without changing phase.
    publisher.reclaim(&mut repo, &work_id).expect("re-reclaim");
    assert_eq!(phase(&mut repo, &work_id), CheckpointPhase::Reclaimed);
}

#[test]
fn recover_on_fully_reclaimed_work_is_a_noop() {
    let dir = TempDir::new();
    let mut repo = repo();
    let publisher = publisher(&dir);
    let work_id = d32(7);
    publisher
        .plan(&mut repo, &plan(work_id, vec![], 1_000))
        .expect("plan");
    publisher
        .publish_all(&mut repo, &work_id, 1_000)
        .expect("publish all");
    assert_eq!(phase(&mut repo, &work_id), CheckpointPhase::Reclaimed);

    // Recovery on an already-terminal work inspects the phase, drops any orphan
    // temp tree (there is none), and reports the unchanged terminal phase.
    let recovered = publisher
        .recover(&mut repo, &work_id, 2_000)
        .expect("recover");
    assert_eq!(recovered, CheckpointPhase::Reclaimed);
}

// ---------------------------------------------------------------------------
// feed head never regresses
// ---------------------------------------------------------------------------

#[test]
fn advance_never_regresses_a_higher_feed_head() {
    let dir = TempDir::new();
    let mut repo = repo();
    let publisher = publisher(&dir);
    let work_id = d32(8);
    let slot = committed_removal(&mut repo, 0x90, &work_id);

    // Move the feed head ahead of this checkpoint's covered head (generation 10):
    // a later checkpoint must not pull the logical floor backward.
    {
        let mut tx = repo.begin().expect("begin");
        tx.advance_feed_head_to(&d32(0xEE), 100, 500)
            .expect("pre-advance feed head");
        tx.commit().expect("commit");
    }
    assert_eq!(feed_length(&mut repo), 100);

    publisher
        .plan(&mut repo, &plan(work_id, vec![slot], 1_000))
        .expect("plan");
    publisher
        .publish_all(&mut repo, &work_id, 1_000)
        .expect("publish all");

    // The checkpoint still completed (phase advanced, covered removal terminalized)
    // but the higher feed head was left untouched — advancement only moves forward.
    assert_eq!(phase(&mut repo, &work_id), CheckpointPhase::Reclaimed);
    assert_eq!(slot_state(&mut repo, slot), RemovalSlotState::Terminal);
    assert_eq!(
        feed_length(&mut repo),
        100,
        "checkpoint must not regress a feed head already ahead of it"
    );
}

// ---------------------------------------------------------------------------
// CheckpointError Display + From surface (values obtained via the public API)
// ---------------------------------------------------------------------------

#[test]
fn display_repository_error_via_from_conversion() {
    // Obtain a genuine repository error (foreign-key violation) and convert it —
    // exercising `From<AnchorRepositoryError>` and the `Repository` Display arm.
    let mut repo = repo();
    let mut tx = repo.begin().expect("begin");
    let anchor_err = tx
        .insert_directory_inclusion(&d32(1), &d32(2), 0, &[7u8; 16])
        .expect_err("inclusion for nonexistent community must be rejected");
    drop(tx);

    let err: CheckpointError = anchor_err.into();
    assert!(matches!(err, CheckpointError::Repository(_)));
    assert!(
        err.to_string().starts_with("checkpoint repository error:"),
        "unexpected Display: {err}"
    );
}

#[test]
fn display_filesystem_error_when_base_dir_is_a_file() {
    // A publisher whose base directory path is actually a regular file cannot
    // create the publication directory: `publish_files` surfaces the filesystem
    // error via `fs_err`.
    let unique = DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
    let file_path = std::env::temp_dir().join(format!(
        "riot-anchor-ckpt-notadir-{}-{unique}",
        std::process::id()
    ));
    std::fs::write(&file_path, b"i am a file, not a directory").expect("write blocker file");

    let mut repo = repo();
    let publisher = CheckpointPublisher::new(signer(), &file_path);
    let work_id = d32(9);
    publisher
        .plan(&mut repo, &plan(work_id, vec![], 1_000))
        .expect("plan");
    publisher.sign(&mut repo, &work_id).expect("sign");

    let err = publisher
        .publish_files(&mut repo, &work_id, &mut no_failpoint)
        .expect_err("cannot create a directory where a file exists");
    assert!(matches!(err, CheckpointError::Filesystem(_)));
    assert!(
        err.to_string().starts_with("checkpoint filesystem error:"),
        "unexpected Display: {err}"
    );

    let _ = std::fs::remove_file(&file_path);
}

#[test]
fn display_unknown_work_error() {
    let mut repo = repo();
    let publisher = CheckpointPublisher::new(signer(), std::env::temp_dir());
    let unknown = d32(0xAB);

    // Signing a work id that was never planned fails with `UnknownWork`.
    let err = publisher
        .sign(&mut repo, &unknown)
        .expect_err("unknown work must be rejected");
    assert!(matches!(err, CheckpointError::UnknownWork(id) if id == unknown));
    assert!(
        err.to_string().starts_with("unknown checkpoint work"),
        "unexpected Display: {err}"
    );
}

#[test]
fn display_hash_mismatch_error_on_corrupt_published_bytes() {
    let dir = TempDir::new();
    let mut repo = repo();
    let publisher = publisher(&dir);
    let work_id = d32(0x0C);
    publisher
        .plan(&mut repo, &plan(work_id, vec![], 1_000))
        .expect("plan");
    publisher.sign(&mut repo, &work_id).expect("sign");
    publisher
        .publish_files(&mut repo, &work_id, &mut no_failpoint)
        .expect("publish");

    // Corrupt the published file so advancement fails closed on the hash check.
    let final_name = format!("checkpoint-{}.cbor", hex(&work_id));
    std::fs::write(dir.path().join(&final_name), b"corrupted").expect("corrupt");

    let err = publisher
        .advance(&mut repo, &work_id, 1_000, &mut no_failpoint)
        .expect_err("advance must fail closed");
    assert!(matches!(err, CheckpointError::HashMismatch { .. }));
    assert!(
        err.to_string().contains("content hash mismatch"),
        "unexpected Display: {err}"
    );
}

#[test]
fn display_failpoint_error() {
    let dir = TempDir::new();
    let mut repo = repo();
    let publisher = publisher(&dir);
    let work_id = d32(0x0D);
    publisher
        .plan(&mut repo, &plan(work_id, vec![], 1_000))
        .expect("plan");

    // `publish before sign` is surfaced as a `Failpoint` variant; check its label
    // renders in the Display string.
    let err = publisher
        .publish_files(&mut repo, &work_id, &mut no_failpoint)
        .expect_err("publish before sign");
    assert!(matches!(err, CheckpointError::Failpoint(_)));
    let rendered = err.to_string();
    assert!(
        rendered.starts_with("checkpoint failpoint tripped:")
            && rendered.contains("publish before sign"),
        "unexpected Display: {rendered}"
    );
}

fn hex(bytes: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}
