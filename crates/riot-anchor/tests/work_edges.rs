//! WU-M2 coverage — admission-work verification edges and the token-secret-ring
//! getter. The happy path (difficulty 0, or a fresh valid stamp) is covered by the
//! control suite; here we drive the aged-out (`work_expired`) and clock-skew
//! (`work_required`) refusals and the `secret()` accessor.

use ed25519_dalek::{Signer as _, SigningKey};

use riot_anchor::work::{
    issue_work_challenge, verify_admission_work, ChallengeSigningContext, OperatorSigner,
    PressurePolicy, RequiredWork, TokenSecretRing,
};

use riot_anchor_protocol::codec::CanonicalRecord;
use riot_anchor_protocol::control::{ControlRefusal, GetWorkChallengeV1};
use riot_anchor_protocol::digest::{digest_v1, label, work_proof};
use riot_anchor_protocol::records::{ControlOperationKind, WorkChallengeV1, WorkStampV1};

struct TestSigner(SigningKey);
impl OperatorSigner for TestSigner {
    fn sign(&self, preimage: &[u8]) -> [u8; 64] {
        self.0.sign(preimage).to_bytes()
    }
}

fn leading_zero_bits(bytes: &[u8; 32]) -> u32 {
    let mut count = 0;
    for byte in bytes {
        if *byte == 0 {
            count += 8;
        } else {
            count += byte.leading_zeros();
            break;
        }
    }
    count
}

fn mine_stamp(challenge: &WorkChallengeV1, difficulty: u64) -> WorkStampV1 {
    let bytes = challenge.encode_canonical().expect("encode challenge");
    let challenge_digest = digest_v1(label::WORK_CHALLENGE_ENVELOPE, &bytes);
    let mut counter = 0u64;
    loop {
        let proof = work_proof(&challenge_digest, counter);
        if u64::from(leading_zero_bits(&proof)) >= difficulty {
            return WorkStampV1 {
                challenge_envelope_bytes: bytes,
                counter,
                proof_bytes: proof,
            };
        }
        counter += 1;
    }
}

const ISSUED_AT: u64 = 1_000;
const TTL: u64 = 300;
const IDEMPOTENCY_KEY: [u8; 16] = [1u8; 16];
const COMMUNITY_ROOT: [u8; 32] = [2u8; 32];
const WORK_TARGET: [u8; 32] = [3u8; 32];
const POLICY_EPOCH: u64 = 4;
const DIFFICULTY: u64 = 1;

fn signed_challenge_and_key() -> (WorkChallengeV1, [u8; 32]) {
    let key = SigningKey::from_bytes(&[7u8; 32]);
    let public = key.verifying_key().to_bytes();
    let signer = TestSigner(key);
    let context = ChallengeSigningContext {
        anchor_id: [10u8; 32],
        operator_key_id: [11u8; 32],
        descriptor_epoch: 0,
        descriptor_digest: [12u8; 32],
    };
    let request = GetWorkChallengeV1 {
        intended_operation_kind: ControlOperationKind::PrepareHost,
        intended_idempotency_key: IDEMPOTENCY_KEY,
        community_root: COMMUNITY_ROOT,
        work_target_digest: WORK_TARGET,
    };
    let policy = PressurePolicy {
        policy_epoch: POLICY_EPOCH,
        difficulty: DIFFICULTY,
    };
    let challenge = issue_work_challenge(
        &signer, &context, &request, policy, [5u8; 32], ISSUED_AT, TTL,
    )
    .expect("issue challenge");
    (challenge, public)
}

fn required() -> RequiredWork {
    RequiredWork {
        operation_kind: ControlOperationKind::PrepareHost,
        idempotency_key: IDEMPOTENCY_KEY,
        work_target_digest: WORK_TARGET,
        community_root: COMMUNITY_ROOT,
        policy: PressurePolicy {
            policy_epoch: POLICY_EPOCH,
            difficulty: DIFFICULTY,
        },
    }
}

#[test]
fn a_correctly_bound_stamp_is_accepted_inside_its_window() {
    let (challenge, public) = signed_challenge_and_key();
    let stamp = mine_stamp(&challenge, DIFFICULTY);
    // Observed strictly inside [issued_at, expires_at).
    assert!(verify_admission_work(&public, Some(&stamp), &required(), ISSUED_AT + 1).is_ok());
}

#[test]
fn a_correctly_bound_but_aged_out_stamp_is_work_expired() {
    let (challenge, public) = signed_challenge_and_key();
    let stamp = mine_stamp(&challenge, DIFFICULTY);
    let expires_at = ISSUED_AT + TTL;
    // Observed at exactly the expiry (inclusive) → work_expired.
    match verify_admission_work(&public, Some(&stamp), &required(), expires_at) {
        Err(ControlRefusal::WorkExpired {
            expires_at: reported,
            observed_at,
        }) => {
            assert_eq!(reported, expires_at);
            assert_eq!(observed_at, expires_at);
        }
        other => panic!("expected WorkExpired, got {other:?}"),
    }
}

#[test]
fn a_not_yet_valid_stamp_is_treated_as_work_required() {
    let (challenge, public) = signed_challenge_and_key();
    let stamp = mine_stamp(&challenge, DIFFICULTY);
    // Observed BEFORE issuance (clock skew): fetch a fresh one → work_required.
    match verify_admission_work(&public, Some(&stamp), &required(), ISSUED_AT - 1) {
        Err(ControlRefusal::WorkRequired {
            policy_epoch,
            difficulty,
        }) => {
            assert_eq!(policy_epoch, POLICY_EPOCH);
            assert_eq!(difficulty, DIFFICULTY);
        }
        other => panic!("expected WorkRequired, got {other:?}"),
    }
}

#[test]
fn token_secret_ring_exposes_only_retained_epochs() {
    let mut ring = TokenSecretRing::new(0, [1u8; 32]);
    assert_eq!(ring.secret(0), Some(&[1u8; 32]));
    assert!(ring.secret(9).is_none(), "an unminted epoch has no secret");

    ring.rotate(1, [2u8; 32]);
    assert_eq!(ring.current_epoch(), 1);
    assert_eq!(ring.secret(0), Some(&[1u8; 32]), "prior secret retained");
    assert_eq!(ring.secret(1), Some(&[2u8; 32]));

    // Retiring below the current epoch drops the stale secret but keeps current.
    ring.retire_below(1);
    assert!(ring.secret(0).is_none(), "retired epoch dropped");
    assert_eq!(ring.secret(1), Some(&[2u8; 32]));
}
