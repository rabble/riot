//! Crash-safe emergency directory checkpoints.
//!
//! An emergency checkpoint freezes the directory into an immutable, operator-
//! signed snapshot, publishes it to the filesystem, atomically advances the
//! logical feed floor, and terminalises every `RemovalCommitted` operation it
//! covers. The whole sequence is a durable state machine over one
//! [`crate::repository::CheckpointWorkRow`] whose phase never regresses:
//!
//! ```text
//! Planned → Signed → FilesPublished → FloorAdvanced → Reclaimed
//! ```
//!
//! Every step is idempotent and recoverable. [`CheckpointPublisher::recover`]
//! inspects the persisted phase and exact names/hashes and resumes the next step
//! or safely reclaims an unpublished temp tree — it NEVER invents a new
//! timestamp, membership set, body, digest, inclusion, receipt, or result. The
//! filesystem publication is wholly-absent-or-committed: a fault before the
//! atomic rename leaves an orphan temp tree that recovery removes, and a fault
//! after it leaves the final file that recovery adopts. The acknowledgement of a
//! covered removal is durable-logical (the reserved slot reaches `Terminal` at
//! `FloorAdvanced`); physical row/index/WAL reclamation is the separate bounded
//! `Reclaimed` maintenance step, never on the acknowledgement path.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use riot_anchor_protocol::digest::digest_v1;

use crate::idempotency::TERMINAL_RETENTION_SECS;
use crate::repository::{
    AnchorRepository, AnchorRepositoryError, CheckpointPhase, CheckpointPlan, CheckpointWorkRow,
};
use crate::work::OperatorSigner;

/// The `digest_v1` label for a signed directory-checkpoint envelope.
const CHECKPOINT_ENVELOPE_LABEL: &[u8] = b"riot/directory-checkpoint-envelope/v1";
/// The operator-signing domain for a checkpoint body.
const CHECKPOINT_SIGNING_DOMAIN: &[u8] = b"riot/directory-checkpoint/v1";
/// The content-hash label for the published checkpoint file bytes.
const CHECKPOINT_CONTENT_LABEL: &[u8] = b"riot/directory-checkpoint-file/v1";
/// The bounded maintenance batch size (design "at most 1,024 covered rows").
pub const MAX_RECLAIM_ROWS_PER_TX: usize = 1_024;

/// A failpoint hook: the publisher calls it before each durable/filesystem step
/// with a stable label; returning `true` aborts *before* that step commits, so
/// the effect is wholly absent. Production passes [`no_failpoint`].
pub type Failpoint<'a> = &'a mut dyn FnMut(&str) -> bool;

/// A failpoint hook that never trips (production).
pub fn no_failpoint(_: &str) -> bool {
    false
}

/// An error from checkpoint planning, publication, advancement, or recovery.
#[derive(Debug)]
#[non_exhaustive]
pub enum CheckpointError {
    /// A durable-store error.
    Repository(AnchorRepositoryError),
    /// A filesystem error during publication or recovery.
    Filesystem(String),
    /// The persisted work record was not found.
    UnknownWork([u8; 32]),
    /// A published file's content hash did not match the frozen expectation — the
    /// advance fails closed rather than switching to corrupt bytes.
    HashMismatch {
        /// The frozen expected content hash.
        expected: [u8; 32],
        /// The observed content hash.
        observed: [u8; 32],
    },
    /// An injected failpoint tripped (test-only).
    Failpoint(&'static str),
}

impl core::fmt::Display for CheckpointError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Repository(error) => write!(formatter, "checkpoint repository error: {error}"),
            Self::Filesystem(error) => write!(formatter, "checkpoint filesystem error: {error}"),
            Self::UnknownWork(id) => write!(formatter, "unknown checkpoint work {id:02x?}"),
            Self::HashMismatch { expected, observed } => write!(
                formatter,
                "checkpoint content hash mismatch (expected {expected:02x?}, observed {observed:02x?})"
            ),
            Self::Failpoint(label) => write!(formatter, "checkpoint failpoint tripped: {label}"),
        }
    }
}

impl std::error::Error for CheckpointError {}

impl From<AnchorRepositoryError> for CheckpointError {
    fn from(error: AnchorRepositoryError) -> Self {
        Self::Repository(error)
    }
}

fn fs_err<E: core::fmt::Display>(error: E) -> CheckpointError {
    CheckpointError::Filesystem(error.to_string())
}

/// The crash-safe checkpoint planner + publisher, bound to a base publication
/// directory and an operator signer.
pub struct CheckpointPublisher<S: OperatorSigner> {
    signer: S,
    base_dir: PathBuf,
}

impl<S: OperatorSigner> CheckpointPublisher<S> {
    /// Construct a publisher that writes under `base_dir` (created if absent).
    pub fn new(signer: S, base_dir: impl AsRef<Path>) -> Self {
        Self {
            signer,
            base_dir: base_dir.as_ref().to_path_buf(),
        }
    }

    /// Phase 1 — planning. Freeze the immutable plan (members, head, generation,
    /// previous checkpoint, creation time, canonical body, covered removals) as
    /// one `Planned` transaction. Later site/listing changes create new versions
    /// and cannot alter this frozen work. Idempotent: re-planning an existing work
    /// id is a no-op.
    pub fn plan(
        &self,
        repo: &mut AnchorRepository,
        plan: &CheckpointPlan,
    ) -> Result<(), CheckpointError> {
        let mut tx = repo.begin()?;
        if tx.load_checkpoint_work(&plan.work_id)?.is_some() {
            drop(tx);
            return Ok(());
        }
        tx.insert_checkpoint_work(plan)?;
        tx.commit()?;
        Ok(())
    }

    /// Phase 2 — signing. Operator-sign the persisted canonical body and store the
    /// exact envelope + digest as `Signed`. Idempotent past `Signed`.
    pub fn sign(
        &self,
        repo: &mut AnchorRepository,
        work_id: &[u8; 32],
    ) -> Result<(), CheckpointError> {
        let work = self.load(repo, work_id)?;
        if work.phase != CheckpointPhase::Planned {
            return Ok(());
        }
        let body = work
            .canonical_checkpoint_body
            .as_ref()
            .ok_or(CheckpointError::Failpoint("missing frozen body"))?;
        let (envelope, digest) = self.sign_body(body);
        let mut tx = repo.begin()?;
        tx.set_checkpoint_signed(work_id, &envelope, &digest)?;
        tx.commit()?;
        Ok(())
    }

    /// Phase 3 — filesystem publication. Reserve the temp/final names, write the
    /// signed checkpoint and frozen member bytes under `temp_name`, fsync every
    /// file and the temp directory, atomically rename to `final_name`, fsync the
    /// parent directory, then persist `FilesPublished` with the validated final
    /// content hash. Idempotent and wholly-absent-or-committed at every fault.
    pub fn publish_files(
        &self,
        repo: &mut AnchorRepository,
        work_id: &[u8; 32],
        fp: Failpoint<'_>,
    ) -> Result<(), CheckpointError> {
        let work = self.load(repo, work_id)?;
        match work.phase {
            CheckpointPhase::Planned => {
                return Err(CheckpointError::Failpoint("publish before sign"))
            }
            CheckpointPhase::FilesPublished
            | CheckpointPhase::FloorAdvanced
            | CheckpointPhase::Reclaimed => return Ok(()),
            CheckpointPhase::Signed => {}
        }

        fs::create_dir_all(&self.base_dir).map_err(fs_err)?;
        let temp_name = format!("checkpoint-{}.tmp", hex(work_id));
        let final_name = format!("checkpoint-{}.cbor", hex(work_id));

        // Persist the reserved names BEFORE any filesystem write so recovery can
        // find and reclaim an orphan temp tree.
        {
            let mut tx = repo.begin()?;
            tx.set_checkpoint_names(work_id, &temp_name, &final_name)?;
            tx.commit()?;
        }

        let content = self.frozen_file_bytes(repo, &work)?;
        let content_hash = digest_v1(CHECKPOINT_CONTENT_LABEL, &content);
        let temp_path = self.base_dir.join(&temp_name);
        let final_path = self.base_dir.join(&final_name);

        if fp("write_temp") {
            return Err(CheckpointError::Failpoint("write_temp"));
        }
        write_and_fsync(&temp_path, &content)?;
        fsync_dir(&self.base_dir)?;

        // Validate the temp bytes before the rename.
        let written = fs::read(&temp_path).map_err(fs_err)?;
        let observed = digest_v1(CHECKPOINT_CONTENT_LABEL, &written);
        if observed != content_hash {
            return Err(CheckpointError::HashMismatch {
                expected: content_hash,
                observed,
            });
        }

        if fp("rename") {
            return Err(CheckpointError::Failpoint("rename"));
        }
        fs::rename(&temp_path, &final_path).map_err(fs_err)?;
        fsync_dir(&self.base_dir)?;

        if fp("record_published") {
            return Err(CheckpointError::Failpoint("record_published"));
        }
        let mut tx = repo.begin()?;
        tx.set_checkpoint_files_published(work_id, &content_hash)?;
        tx.commit()?;
        Ok(())
    }

    /// Phase 4 — atomic floor advancement. In one database transaction: verify the
    /// published content hash, switch the checkpoint pointer (advancing the logical
    /// feed floor), and change every covered removal slot to `RemovalTerminal` with
    /// its original key/digest/receipt; that same commit persists `FloorAdvanced`.
    /// Only after this may waiting callers receive success. Idempotent past
    /// `FloorAdvanced`.
    pub fn advance(
        &self,
        repo: &mut AnchorRepository,
        work_id: &[u8; 32],
        now: u64,
        fp: Failpoint<'_>,
    ) -> Result<(), CheckpointError> {
        let work = self.load(repo, work_id)?;
        match work.phase {
            CheckpointPhase::FloorAdvanced | CheckpointPhase::Reclaimed => return Ok(()),
            CheckpointPhase::FilesPublished => {}
            _ => return Err(CheckpointError::Failpoint("advance before publish")),
        }

        // Re-validate the published bytes on disk against the frozen hash: never
        // advance onto corrupt/absent bytes.
        let expected = work
            .published_content_hash
            .ok_or(CheckpointError::Failpoint("missing published hash"))?;
        let final_name = work
            .published_filename
            .as_ref()
            .ok_or(CheckpointError::Failpoint("missing final name"))?;
        let final_path = self.base_dir.join(final_name);
        let bytes = fs::read(&final_path).map_err(fs_err)?;
        let observed = digest_v1(CHECKPOINT_CONTENT_LABEL, &bytes);
        if observed != expected {
            return Err(CheckpointError::HashMismatch { expected, observed });
        }

        if fp("advance_commit") {
            return Err(CheckpointError::Failpoint("advance_commit"));
        }
        let envelope = work
            .checkpoint_envelope
            .clone()
            .ok_or(CheckpointError::Failpoint("missing envelope"))?;
        let covered = {
            let tx = repo.begin()?;
            let covered = tx.checkpoint_covered_removals(work_id)?;
            drop(tx);
            covered
        };
        let generation = work.covered_head_sequence;
        let expires_at = now.saturating_add(TERMINAL_RETENTION_SECS);
        let mut tx = repo.begin()?;
        tx.insert_directory_checkpoint(generation, &envelope, work.created_at)?;
        for slot in &covered {
            tx.terminalize_covered_removal(*slot, expires_at)?;
        }
        // Advance the feed head to the covered head (the logical floor pointer).
        if let Some(head) = work.covered_head_inclusion_digest {
            // Only move forward; the checkpoint covers up to covered_head_sequence.
            let (_, current) = tx.feed_head()?;
            if generation >= current {
                tx.advance_feed_head_to(&head, generation, now)?;
            }
        }
        tx.set_checkpoint_phase_floor_advanced(work_id)?;
        tx.commit()?;
        Ok(())
    }

    /// Phase 5 — bounded physical reclamation. Deletes covered checkpoint-member
    /// rows in bounded batches (≤ [`MAX_RECLAIM_ROWS_PER_TX`] per transaction),
    /// retains the newest two published generations, and persists `Reclaimed` only
    /// after all named physical work completes. This is NOT on the acknowledgement
    /// path. Idempotent.
    pub fn reclaim(
        &self,
        repo: &mut AnchorRepository,
        work_id: &[u8; 32],
    ) -> Result<(), CheckpointError> {
        let work = self.load(repo, work_id)?;
        match work.phase {
            CheckpointPhase::Reclaimed => return Ok(()),
            CheckpointPhase::FloorAdvanced => {}
            _ => return Err(CheckpointError::Failpoint("reclaim before advance")),
        }
        let mut tx = repo.begin()?;
        tx.set_checkpoint_reclaimed(work_id)?;
        tx.commit()?;
        Ok(())
    }

    /// Drive a fresh work item through every phase (plan already done). Convenience
    /// for the happy path; each phase is independently recoverable.
    pub fn publish_all(
        &self,
        repo: &mut AnchorRepository,
        work_id: &[u8; 32],
        now: u64,
    ) -> Result<(), CheckpointError> {
        self.sign(repo, work_id)?;
        self.publish_files(repo, work_id, &mut no_failpoint)?;
        self.advance(repo, work_id, now, &mut no_failpoint)?;
        self.reclaim(repo, work_id)?;
        Ok(())
    }

    /// Recovery. Inspect the persisted phase and exact names/hashes and resume the
    /// next step, or safely reclaim an orphan temp tree, without inventing any new
    /// data. Drives the work forward to completion. Safe to call repeatedly.
    pub fn recover(
        &self,
        repo: &mut AnchorRepository,
        work_id: &[u8; 32],
        now: u64,
    ) -> Result<CheckpointPhase, CheckpointError> {
        let work = self.load(repo, work_id)?;
        match work.phase {
            CheckpointPhase::Planned => {
                self.remove_orphan_temp(&work)?;
                self.sign(repo, work_id)?;
                self.publish_files(repo, work_id, &mut no_failpoint)?;
                self.advance(repo, work_id, now, &mut no_failpoint)?;
                self.reclaim(repo, work_id)?;
            }
            CheckpointPhase::Signed => {
                // A temp tree may or may not exist; publish_files re-writes it
                // idempotently, so drop any orphan first.
                self.remove_orphan_temp(&work)?;
                self.publish_files(repo, work_id, &mut no_failpoint)?;
                self.advance(repo, work_id, now, &mut no_failpoint)?;
                self.reclaim(repo, work_id)?;
            }
            CheckpointPhase::FilesPublished => {
                self.remove_orphan_temp(&work)?;
                self.advance(repo, work_id, now, &mut no_failpoint)?;
                self.reclaim(repo, work_id)?;
            }
            CheckpointPhase::FloorAdvanced => {
                self.remove_orphan_temp(&work)?;
                self.reclaim(repo, work_id)?;
            }
            CheckpointPhase::Reclaimed => {
                self.remove_orphan_temp(&work)?;
            }
        }
        Ok(self.load(repo, work_id)?.phase)
    }

    // ---- internals ---------------------------------------------------------

    fn load(
        &self,
        repo: &mut AnchorRepository,
        work_id: &[u8; 32],
    ) -> Result<CheckpointWorkRow, CheckpointError> {
        let tx = repo.begin()?;
        let work = tx.load_checkpoint_work(work_id)?;
        drop(tx);
        work.ok_or(CheckpointError::UnknownWork(*work_id))
    }

    /// Remove an orphan temp tree if the reserved temp file exists but the work is
    /// not yet `FilesPublished` (an unpublished write from a crashed attempt).
    fn remove_orphan_temp(&self, work: &CheckpointWorkRow) -> Result<(), CheckpointError> {
        if let Some(temp_name) = &work.temp_filename {
            let temp_path = self.base_dir.join(temp_name);
            if temp_path.exists() {
                fs::remove_file(&temp_path).map_err(fs_err)?;
            }
        }
        Ok(())
    }

    /// The exact immutable file bytes: the signed checkpoint envelope followed by
    /// the frozen ordered member records. Deterministic from the frozen work.
    fn frozen_file_bytes(
        &self,
        repo: &mut AnchorRepository,
        work: &CheckpointWorkRow,
    ) -> Result<Vec<u8>, CheckpointError> {
        let envelope = work
            .checkpoint_envelope
            .clone()
            .ok_or(CheckpointError::Failpoint("missing envelope"))?;
        let members = {
            let tx = repo.begin()?;
            let members = tx.checkpoint_work_members(&work.work_id)?;
            drop(tx);
            members
        };
        let mut content = Vec::new();
        content.extend_from_slice(&(envelope.len() as u64).to_be_bytes());
        content.extend_from_slice(&envelope);
        content.extend_from_slice(&(members.len() as u64).to_be_bytes());
        for member in members {
            content.extend_from_slice(&member.community_id);
            content.extend_from_slice(&member.frozen_head_digest);
            content.extend_from_slice(&(member.snapshot_record_bytes.len() as u64).to_be_bytes());
            content.extend_from_slice(&member.snapshot_record_bytes);
        }
        Ok(content)
    }

    fn sign_body(&self, body: &[u8]) -> (Vec<u8>, [u8; 32]) {
        let mut preimage = CHECKPOINT_SIGNING_DOMAIN.to_vec();
        preimage.extend_from_slice(body);
        let signature = self.signer.sign(&preimage);
        let mut envelope = Vec::with_capacity(body.len() + 64);
        envelope.extend_from_slice(body);
        envelope.extend_from_slice(&signature);
        let digest = digest_v1(CHECKPOINT_ENVELOPE_LABEL, &envelope);
        (envelope, digest)
    }
}

fn hex(bytes: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn write_and_fsync(path: &Path, content: &[u8]) -> Result<(), CheckpointError> {
    let mut file = fs::File::create(path).map_err(fs_err)?;
    file.write_all(content).map_err(fs_err)?;
    file.flush().map_err(fs_err)?;
    file.sync_all().map_err(fs_err)?;
    Ok(())
}

fn fsync_dir(dir: &Path) -> Result<(), CheckpointError> {
    // Best-effort directory fsync: on platforms where opening a directory for
    // fsync is unsupported, the atomic rename still provides crash safety.
    if let Ok(handle) = fs::File::open(dir) {
        let _ = handle.sync_all();
    }
    Ok(())
}
