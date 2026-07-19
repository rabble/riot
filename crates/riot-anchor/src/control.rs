//! The `riot/anchor/1` control admission service.
//!
//! This is the ordered front door for the four WU-014 control operations —
//! `Describe`, `GetWorkChallenge`, `PrepareHost`, and `GetOperation` — over the
//! durable [`AnchorRepository`]. It implements the design's exhaustive admission
//! ordering:
//!
//! ```text
//! frame bound  →  canonical decode / re-encode  →  control_request_digest  →
//! constant-time idempotency lookup  →  (authority → capacity → work)  →
//! atomic durable claim + operation
//! ```
//!
//! The first four steps are cheap and precede every expensive check and any
//! durable row. A busy / quota / work refusal is *pre-claim*: it writes nothing,
//! so the same key and body may retry and still succeed. A claimed key replayed
//! with a **changed** body is `idempotency_conflict`, and the stored state is
//! never revealed or mutated. `PrepareHost` creates the operation, derives the
//! namespace tokens, and stores the byte-identical prepared response in one
//! transaction; `GetOperation` exposes the prepared / terminal / expired /
//! unknown lifecycle by operation id alone.

use riot_anchor_protocol::codec::{decode_canonical, CanonicalRecord, CodecError};
use riot_anchor_protocol::control::{
    ControlOperation, ControlOutcome, ControlRefusal, ControlRequestV1, ControlResponseV1,
    ControlSuccess, DescribeSuccessV1, EffectiveOperationLimits, GetOperationState,
    GetOperationSuccessV1, GetOperationV1, GetWorkChallengeV1, PrepareHostV1, PrepareKind,
    PrepareSuccessV1, TerminalOperationOutcome, MAX_CONTROL_FRAME_BYTES,
};
use riot_anchor_protocol::records::{
    AnchorLimitProfileV1, ControlOperationKind, DescriptorEnvelopeV1, HostingReceiptV1,
    IDEMPOTENCY_KEY_BYTES,
};

use crate::idempotency::{
    classify, AdmissionLookup, PREPARED_RETENTION_EXTRA_SECS, RESULT_CLASS_ORDINARY,
};
use crate::repository::{
    AnchorRepository, AnchorRepositoryError, IdempotencyClaimState, NewPreparedOperation,
    OperationKind, OperationStatus,
};
use crate::work::{
    issue_work_challenge, verify_admission_work, ChallengeSigningContext, OperatorSigner,
    PressurePolicy, RequiredWork, TokenSecretRing, MAX_WORK_CHALLENGE_TTL_SECS,
};

/// An error that prevents the service from producing any control result.
#[derive(Debug)]
#[non_exhaustive]
pub enum ControlError {
    /// A durable-store error.
    Repository(AnchorRepositoryError),
    /// A canonical-encoding error while building or reconstructing a payload.
    Codec(CodecError),
}

impl core::fmt::Display for ControlError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Repository(error) => write!(formatter, "control repository error: {error}"),
            Self::Codec(error) => write!(formatter, "control codec error: {error:?}"),
        }
    }
}

impl std::error::Error for ControlError {}

impl From<AnchorRepositoryError> for ControlError {
    fn from(error: AnchorRepositoryError) -> Self {
        Self::Repository(error)
    }
}

impl From<CodecError> for ControlError {
    fn from(error: CodecError) -> Self {
        Self::Codec(error)
    }
}

/// A bounded protocol failure that ends the control stream without a result
/// (design "bounded protocol failure/close"). No idempotency lookup or claim
/// occurs for any of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolFailure {
    /// The frame exceeded [`MAX_CONTROL_FRAME_BYTES`].
    FrameTooLarge,
    /// The frame did not decode as a canonical `ControlRequestV1`.
    Malformed,
    /// The frame decoded but did not re-encode to the exact input bytes.
    NonCanonical,
    /// The operation is not served by this build.
    Unsupported,
}

/// The outcome of handling a control frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlHandling {
    /// A control response (success or a closed refusal) to send back.
    Responded(ControlResponseV1),
    /// A bounded protocol failure; the stream closes with no result.
    ProtocolFailure(ProtocolFailure),
}

/// The immutable anchor coordinates and configuration the control service serves
/// from. Descriptor/limit-profile construction happens before readiness, so the
/// service only ever *serves* these signed/validated values.
#[derive(Debug, Clone)]
pub struct AnchorControlContext {
    /// Stable anchor id.
    pub anchor_id: [u8; 32],
    /// Current signing operator key id.
    pub operator_key_id: [u8; 32],
    /// Current operator public key (self-issued work challenges verify under it).
    pub operator_public_key: [u8; 32],
    /// Current descriptor epoch.
    pub descriptor_epoch: u64,
    /// Current descriptor digest.
    pub descriptor_digest: [u8; 32],
    /// The current signed descriptor envelope (served by `Describe`).
    pub descriptor: DescriptorEnvelopeV1,
    /// The advertised limit profile (served by `Describe`, projected into every
    /// prepared response's `effective_operation_limits`).
    pub limit_profile: AnchorLimitProfileV1,
    /// The negotiated sync version returned in prepared responses.
    pub sync_version: u64,
    /// The prepared-operation lifetime in seconds (design: at most one hour).
    pub operation_lifetime_secs: u64,
}

/// The pluggable admission checks the control service runs, in the design's
/// order, *after* the cheap idempotency lookup and *before* the durable claim.
///
/// Every method that returns a [`ControlRefusal`] is a pre-claim refusal: it must
/// leave the durable store untouched so the same key may retry.
pub trait AdmissionPolicy {
    /// The cheap-through-authority checks (version / transport / profile size /
    /// source rate / global headroom / ticket authority). On success it yields
    /// the host plan; on failure a pre-claim refusal.
    fn authorize_prepare_host(
        &self,
        request: &PrepareHostV1,
        observed_at: u64,
    ) -> Result<PreparePlan, ControlRefusal>;

    /// The capacity gate (`admission_busy` / `admission_over_quota`). Runs after
    /// authority and before work.
    fn capacity_for_prepare_host(
        &self,
        plan: &PreparePlan,
        observed_at: u64,
    ) -> Result<(), ControlRefusal>;

    /// The community's current pressure band (required work difficulty + policy
    /// epoch).
    fn pressure_band(&self, community_root: &[u8; 32], observed_at: u64) -> PressurePolicy;
}

/// The host plan an authorized `PrepareHost` yields: the ordered `O`/`C`/`W`
/// namespace ids, their currently-retained snapshot digests, the community root,
/// and the captured base site generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparePlan {
    /// The community root (ticket root id) bound to this operation.
    pub community_root: [u8; 32],
    /// The ordered `O`, `C`, `W` namespace ids.
    pub ordered_namespace_host_plan: [[u8; 32]; 3],
    /// The ordered `O`, `C`, `W` currently-retained snapshot digests.
    pub ordered_retained_snapshot_digests: [[u8; 32]; 3],
    /// The captured base site generation.
    pub base_generation: u64,
}

/// The `riot/anchor/1` control admission service.
pub struct AnchorControlService<P: AdmissionPolicy, S: OperatorSigner> {
    context: AnchorControlContext,
    policy: P,
    signer: S,
    token_ring: TokenSecretRing,
}

impl<P: AdmissionPolicy, S: OperatorSigner> AnchorControlService<P, S> {
    /// Construct a control service.
    pub fn new(
        context: AnchorControlContext,
        policy: P,
        signer: S,
        token_ring: TokenSecretRing,
    ) -> Self {
        Self {
            context,
            policy,
            signer,
            token_ring,
        }
    }

    /// The token secret ring (for rotation between operations).
    pub fn token_ring_mut(&mut self) -> &mut TokenSecretRing {
        &mut self.token_ring
    }

    /// Handle one canonical control frame. `entropy` yields fresh 256-bit
    /// randomness for anchor-created ids (operation id, challenge nonce).
    pub fn handle(
        &self,
        repo: &mut AnchorRepository,
        request_bytes: &[u8],
        now: u64,
        entropy: &mut dyn FnMut() -> [u8; 32],
    ) -> Result<ControlHandling, ControlError> {
        // 1. Frame bound (cheapest possible check; no lookup, no claim).
        if request_bytes.len() > MAX_CONTROL_FRAME_BYTES {
            return Ok(ControlHandling::ProtocolFailure(
                ProtocolFailure::FrameTooLarge,
            ));
        }
        // 2. Canonical decode (rejects non-canonical encodings and trailing bytes).
        let request =
            match decode_canonical::<ControlRequestV1>(request_bytes, MAX_CONTROL_FRAME_BYTES) {
                Ok(request) => request,
                Err(_) => {
                    return Ok(ControlHandling::ProtocolFailure(ProtocolFailure::Malformed));
                }
            };
        // 2b. Canonical re-encode must reproduce the exact input bytes.
        match request.encode_canonical() {
            Ok(reencoded) if reencoded == request_bytes => {}
            Ok(_) => {
                return Ok(ControlHandling::ProtocolFailure(
                    ProtocolFailure::NonCanonical,
                ));
            }
            Err(error) => return Err(ControlError::Codec(error)),
        }

        // 3. Dispatch.
        match &request.operation {
            ControlOperation::Describe(_) => Ok(ControlHandling::Responded(self.handle_describe())),
            ControlOperation::GetWorkChallenge(body) => Ok(ControlHandling::Responded(
                self.handle_get_work_challenge(body, now, entropy)?,
            )),
            ControlOperation::PrepareHost(body) => {
                // 3a. control_request_digest + work_target_digest (cheap, pre-claim).
                let control_request_digest = request.operation.control_request_digest()?;
                let work_target_digest = request.operation.work_target_digest()?;
                Ok(ControlHandling::Responded(self.handle_prepare_host(
                    repo,
                    &request.idempotency_key,
                    body,
                    &control_request_digest,
                    &work_target_digest,
                    now,
                    entropy,
                )?))
            }
            ControlOperation::GetOperation(body) => Ok(ControlHandling::Responded(
                self.handle_get_operation(repo, body, now)?,
            )),
            _ => Ok(ControlHandling::ProtocolFailure(
                ProtocolFailure::Unsupported,
            )),
        }
    }

    fn refuse(kind: ControlOperationKind, refusal: ControlRefusal) -> ControlResponseV1 {
        ControlResponseV1 {
            kind,
            outcome: ControlOutcome::Refused(refusal),
        }
    }

    fn handle_describe(&self) -> ControlResponseV1 {
        ControlResponseV1 {
            kind: ControlOperationKind::Describe,
            outcome: ControlOutcome::Success(ControlSuccess::Describe(Box::new(
                DescribeSuccessV1 {
                    descriptor: self.context.descriptor.clone(),
                    limit_profile: self.context.limit_profile.clone(),
                },
            ))),
        }
    }

    fn handle_get_work_challenge(
        &self,
        request: &GetWorkChallengeV1,
        now: u64,
        entropy: &mut dyn FnMut() -> [u8; 32],
    ) -> Result<ControlResponseV1, ControlError> {
        let policy = self.policy.pressure_band(&request.community_root, now);
        let signing_context = ChallengeSigningContext {
            anchor_id: self.context.anchor_id,
            operator_key_id: self.context.operator_key_id,
            descriptor_epoch: self.context.descriptor_epoch,
            descriptor_digest: self.context.descriptor_digest,
        };
        let nonce = entropy();
        let challenge = issue_work_challenge(
            &self.signer,
            &signing_context,
            request,
            policy,
            nonce,
            now,
            MAX_WORK_CHALLENGE_TTL_SECS,
        )?;
        Ok(ControlResponseV1 {
            kind: ControlOperationKind::GetWorkChallenge,
            outcome: ControlOutcome::Success(ControlSuccess::GetWorkChallenge(Box::new(challenge))),
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_prepare_host(
        &self,
        repo: &mut AnchorRepository,
        idempotency_key: &[u8; IDEMPOTENCY_KEY_BYTES],
        request: &PrepareHostV1,
        control_request_digest: &[u8; 32],
        work_target_digest: &[u8; 32],
        now: u64,
        entropy: &mut dyn FnMut() -> [u8; 32],
    ) -> Result<ControlResponseV1, ControlError> {
        let mut transaction = repo.begin()?;

        // ORDER STEP 4: constant-time idempotency lookup (precedes every
        // expensive check and any durable claim).
        let existing = transaction.lookup_idempotency(idempotency_key)?;
        match classify(existing.as_ref(), control_request_digest) {
            AdmissionLookup::ReplayEqual {
                state,
                operation_id,
            } => {
                let response = self.replay_prepare(&transaction, state, operation_id)?;
                drop(transaction); // replay writes nothing; roll back.
                return Ok(response);
            }
            AdmissionLookup::Conflict => {
                drop(transaction);
                return Ok(Self::refuse(
                    ControlOperationKind::PrepareHost,
                    ControlRefusal::IdempotencyConflict,
                ));
            }
            AdmissionLookup::Novel => {}
        }

        // ORDER STEP 5a: authority (version/transport/profile/source-rate/
        // headroom/authority). A refusal here is pre-claim → roll back, no row.
        let plan = match self.policy.authorize_prepare_host(request, now) {
            Ok(plan) => plan,
            Err(refusal) => {
                drop(transaction);
                return Ok(Self::refuse(ControlOperationKind::PrepareHost, refusal));
            }
        };

        // ORDER STEP 5b: capacity (busy / quota). Pre-claim.
        if let Err(refusal) = self.policy.capacity_for_prepare_host(&plan, now) {
            drop(transaction);
            return Ok(Self::refuse(ControlOperationKind::PrepareHost, refusal));
        }

        // ORDER STEP 5c: admission work (pressure band). Pre-claim: a missing,
        // mis-bound, or insufficient stamp is `work_required`, so the same key may
        // retry with a fresh stamp because no digest was ever stored.
        let policy = self.policy.pressure_band(&plan.community_root, now);
        let required = RequiredWork {
            operation_kind: ControlOperationKind::PrepareHost,
            idempotency_key: *idempotency_key,
            work_target_digest: *work_target_digest,
            community_root: plan.community_root,
            policy,
        };
        if let Err(refusal) = verify_admission_work(
            &self.context.operator_public_key,
            request.work_stamp.as_ref(),
            &required,
            now,
        ) {
            drop(transaction);
            return Ok(Self::refuse(ControlOperationKind::PrepareHost, refusal));
        }

        // ORDER STEP 6: all pre-claim checks passed → atomically create the
        // operation, derive tokens, store the exact prepared response, and claim
        // the idempotency key as `Prepared` in one transaction.
        let operation_id = entropy();
        let operation_expiry = now.saturating_add(self.context.operation_lifetime_secs);
        let retention_deadline = operation_expiry.saturating_add(PREPARED_RETENTION_EXTRA_SECS);
        let token_secret_epoch = self.token_ring.current_epoch();

        let mut ordered_namespace_tokens = [[0u8; 32]; 3];
        for (slot, namespace_id) in ordered_namespace_tokens
            .iter_mut()
            .zip(plan.ordered_namespace_host_plan.iter())
        {
            *slot = self
                .token_ring
                .derive(
                    token_secret_epoch,
                    &operation_id,
                    namespace_id,
                    operation_expiry,
                )
                .expect("current token-secret epoch is always retained");
        }

        let prepare_success = PrepareSuccessV1 {
            operation_id,
            base_site_generation: plan.base_generation,
            ordered_namespace_host_plan: plan.ordered_namespace_host_plan,
            ordered_namespace_tokens,
            ordered_retained_snapshot_digests: plan.ordered_retained_snapshot_digests,
            sync_version: self.context.sync_version,
            effective_operation_limits: EffectiveOperationLimits::from_profile(
                &self.context.limit_profile,
            ),
            operation_expiry,
        };
        let response = ControlResponseV1 {
            kind: ControlOperationKind::PrepareHost,
            outcome: ControlOutcome::Success(ControlSuccess::PrepareHost(Box::new(
                prepare_success,
            ))),
        };
        let prepare_response_bytes = response.encode_canonical()?;

        transaction.insert_operation(&NewPreparedOperation {
            operation_id,
            originating_kind: OperationKind::Host,
            token_secret_epoch,
            base_generation: plan.base_generation,
            created_at: now,
            operation_expiry,
            retention_deadline,
            prepare_response_bytes: prepare_response_bytes.clone(),
        })?;
        transaction.claim_idempotency(
            control_request_digest,
            idempotency_key,
            RESULT_CLASS_ORDINARY,
            IdempotencyClaimState::Prepared,
            Some(&operation_id),
            None,
            now,
            retention_deadline,
        )?;
        transaction.commit()?;

        Ok(response)
    }

    /// Reconstruct the byte-identical response for a same-key/same-body replay.
    fn replay_prepare(
        &self,
        transaction: &crate::repository::RepoTransaction<'_>,
        state: IdempotencyClaimState,
        operation_id: Option<[u8; 32]>,
    ) -> Result<ControlResponseV1, ControlError> {
        let operation_id = operation_id.ok_or(ControlError::Codec(CodecError::NonCanonical))?;
        let operation = transaction
            .load_operation(&operation_id)?
            .ok_or(ControlError::Codec(CodecError::NonCanonical))?;
        match state {
            IdempotencyClaimState::Prepared => Ok(decode_canonical::<ControlResponseV1>(
                &operation.prepare_response_bytes,
                MAX_CONTROL_FRAME_BYTES,
            )?),
            IdempotencyClaimState::Terminal => {
                // A terminalized prepare replays its terminal refusal.
                let bytes = operation
                    .terminal_result_bytes
                    .ok_or(ControlError::Codec(CodecError::NonCanonical))?;
                let refusal = decode_canonical::<ControlRefusal>(&bytes, MAX_CONTROL_FRAME_BYTES)?;
                Ok(Self::refuse(ControlOperationKind::PrepareHost, refusal))
            }
            IdempotencyClaimState::Claimed => {
                // WU-014 prepare claims atomically as `Prepared`; a committed
                // `Claimed` row is never externally observable. Fail closed.
                debug_assert!(false, "prepare never leaves a bare Claimed row");
                Ok(Self::refuse(
                    ControlOperationKind::PrepareHost,
                    ControlRefusal::IdempotencyConflict,
                ))
            }
        }
    }

    fn handle_get_operation(
        &self,
        repo: &AnchorRepository,
        request: &GetOperationV1,
        now: u64,
    ) -> Result<ControlResponseV1, ControlError> {
        let operation = match repo.load_operation(&request.operation_id)? {
            Some(operation) => operation,
            None => {
                return Ok(Self::refuse(
                    ControlOperationKind::GetOperation,
                    ControlRefusal::OperationNotFound {
                        operation_id: request.operation_id,
                    },
                ));
            }
        };
        let originating_prepare_kind = match operation.originating_kind {
            OperationKind::Host => PrepareKind::PrepareHost,
            OperationKind::Replica => PrepareKind::PrepareReplica,
        };

        let state = match operation.status {
            OperationStatus::Prepared => {
                if now < operation.operation_expiry {
                    let prepare_success =
                        stored_prepare_success(&operation.prepare_response_bytes)?;
                    GetOperationState::Prepared {
                        operation_expiry: operation.operation_expiry,
                        prepare_success: Box::new(prepare_success),
                    }
                } else if now < operation.retention_deadline {
                    // Expired but still retained: a top-level `operation_expired`.
                    return Ok(Self::refuse(
                        ControlOperationKind::GetOperation,
                        ControlRefusal::OperationExpired {
                            operation_id: request.operation_id,
                            expires_at: operation.operation_expiry,
                        },
                    ));
                } else {
                    // Past the retention horizon: indistinguishable from unknown.
                    return Ok(Self::refuse(
                        ControlOperationKind::GetOperation,
                        ControlRefusal::OperationNotFound {
                            operation_id: request.operation_id,
                        },
                    ));
                }
            }
            OperationStatus::Committed => {
                let bytes = operation
                    .terminal_result_bytes
                    .ok_or(ControlError::Codec(CodecError::NonCanonical))?;
                let receipt =
                    decode_canonical::<HostingReceiptV1>(&bytes, MAX_CONTROL_FRAME_BYTES)?;
                GetOperationState::Terminal {
                    outcome: TerminalOperationOutcome::Committed(Box::new(receipt)),
                }
            }
            OperationStatus::Refused => {
                let bytes = operation
                    .terminal_result_bytes
                    .ok_or(ControlError::Codec(CodecError::NonCanonical))?;
                let refusal = decode_canonical::<ControlRefusal>(&bytes, MAX_CONTROL_FRAME_BYTES)?;
                GetOperationState::Terminal {
                    outcome: TerminalOperationOutcome::Refused(refusal),
                }
            }
        };

        Ok(ControlResponseV1 {
            kind: ControlOperationKind::GetOperation,
            outcome: ControlOutcome::Success(ControlSuccess::GetOperation(Box::new(
                GetOperationSuccessV1 {
                    operation_id: request.operation_id,
                    originating_prepare_kind,
                    state,
                },
            ))),
        })
    }

    /// Terminalize an operation with an exact terminal outcome (used by
    /// session-close / security-exception handling). Flips both the operation
    /// status and its idempotency mapping to `Terminal` in one transaction.
    pub fn terminalize_operation(
        &self,
        repo: &mut AnchorRepository,
        operation_id: &[u8; 32],
        outcome: &TerminalOperationOutcome,
    ) -> Result<(), ControlError> {
        let (status, bytes) = match outcome {
            TerminalOperationOutcome::Committed(receipt) => {
                (OperationStatus::Committed, receipt.encode_canonical()?)
            }
            TerminalOperationOutcome::Refused(refusal) => {
                (OperationStatus::Refused, refusal.encode_canonical()?)
            }
        };
        let mut transaction = repo.begin()?;
        transaction.terminalize_operation(operation_id, status, &bytes)?;
        transaction.commit()?;
        Ok(())
    }
}

/// Extract the embedded `PrepareSuccessV1` from a stored prepared-response frame.
fn stored_prepare_success(bytes: &[u8]) -> Result<PrepareSuccessV1, ControlError> {
    let response = decode_canonical::<ControlResponseV1>(bytes, MAX_CONTROL_FRAME_BYTES)?;
    match response.outcome {
        ControlOutcome::Success(ControlSuccess::PrepareHost(payload))
        | ControlOutcome::Success(ControlSuccess::PrepareReplica(payload)) => Ok(*payload),
        _ => Err(ControlError::Codec(CodecError::NonCanonical)),
    }
}
