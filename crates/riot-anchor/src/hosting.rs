//! The composite `CommitHost` service: the atomic promotion boundary.
//!
//! A hosting operation stages `O`, then `C`, then `W` under one stable operation
//! id via the [`crate::sync_service`] adapter. Nothing becomes visible until a
//! single [`crate::repository::RepoTransaction`] promotes the complete staged
//! site. [`CommitHostService::commit`] is that boundary. It:
//!
//! 1. resolves the operation and its captured host plan (base generation + ordered
//!    `O`/`C`/`W` namespaces) from the stored prepared response;
//! 2. admits `O` FIRST — the digest-matched manifest authorises the EXACT `C`/`W`
//!    routing;
//! 3. re-verifies EVERY staged entry with riot-core's real Meadowcap +
//!    signature check ([`crate::sync_service::verify_anchor_item`]) and enforces
//!    anchor-profile byte bounds — a forged or unauthorised entry is refused,
//!    never promoted;
//! 4. checks the declared snapshot digests match staged state;
//! 5. performs a generation compare-and-swap so two same-base commits have exactly
//!    one winner; and
//! 6. in ONE transaction promotes `O`/`C`/`W`, writes payload references, advances
//!    the generation, stores the signed [`HostingReceiptV1`], invalidates the
//!    namespace tokens (by terminalising the operation), records the terminal
//!    operation outcome, and terminalises the Commit idempotency key.
//!
//! Every refusal follows the design's closed Commit matrix disposition: reusable
//! `commit_busy`/`commit_over_quota` terminalise only the Commit key (the
//! operation stays `prepared`, staging/tokens valid); every terminal-cleanup
//! refusal atomically stores the refused operation outcome, deletes all staging,
//! invalidates every namespace token, and terminalises the Commit key.
//!
//! Because promotion, generation CAS, receipt, token invalidation, terminal
//! operation, and Commit result are one transaction, a crash at any durable
//! mutation is wholly absent or wholly committed, and a lost delivery reconstructs
//! the byte-identical receipt through `GetOperation` (the operation's terminal
//! bytes) or the Commit key's stored result.

use riot_anchor_protocol::authority::{
    admit_public_site_ticket, manifest_coordinates, AuthorityError, TicketFloor,
};
use riot_anchor_protocol::codec::{decode_canonical, CanonicalRecord, CodecError};
use riot_anchor_protocol::control::{
    CommitHostV1, ControlOutcome, ControlRefusal, ControlResponseV1, ControlSuccess,
    MAX_CONTROL_FRAME_BYTES,
};
use riot_anchor_protocol::records::{
    ControlOperationKind, HostingReceiptBodyV1, HostingReceiptV1, HostingStatus, NamespaceResult,
    OperatorSignedEnvelopeV1, PublicSiteTicketV2Core, RootSignedTicketCoreEnvelopeV2,
    TransportFloor, IDEMPOTENCY_KEY_BYTES, MAX_TICKET_CORE_BYTES,
};
use riot_anchor_protocol::sync2::compute_snapshot_digest;

use riot_core::site::validate_site_manifest;
use riot_core::willow::{SignedWillowEntry, MANIFEST_COMPONENT};

use crate::idempotency::{
    classify, AdmissionLookup, RESULT_CLASS_ORDINARY, TERMINAL_RETENTION_SECS,
};
use crate::repository::{
    AnchorRepository, AnchorRepositoryError, GenerationCas, IdempotencyClaimState, OperationStatus,
    RepoTransaction, StoredOperation,
};
use crate::sync_service::{ordered_host_plan, verify_anchor_item, verify_anchor_item_parts};
use crate::work::OperatorSigner;

/// An error that prevents the Commit service from producing any control result.
#[derive(Debug)]
#[non_exhaustive]
pub enum CommitError {
    /// A durable-store error.
    Repository(AnchorRepositoryError),
    /// A canonical-encoding error.
    Codec(CodecError),
    /// The stored operation's prepared response could not be decoded into a plan.
    MalformedPlan,
    /// An injected failpoint tripped before the transaction committed (test-only).
    Failpoint(&'static str),
}

impl core::fmt::Display for CommitError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Repository(error) => write!(formatter, "commit repository error: {error}"),
            Self::Codec(error) => write!(formatter, "commit codec error: {error:?}"),
            Self::MalformedPlan => write!(formatter, "stored prepared response is not a host plan"),
            Self::Failpoint(label) => write!(formatter, "commit failpoint tripped: {label}"),
        }
    }
}

impl std::error::Error for CommitError {}

impl From<AnchorRepositoryError> for CommitError {
    fn from(error: AnchorRepositoryError) -> Self {
        Self::Repository(error)
    }
}

impl From<CodecError> for CommitError {
    fn from(error: CodecError) -> Self {
        Self::Codec(error)
    }
}

/// A failpoint hook. The service calls it before each durable mutation with a
/// stable label; returning `true` aborts the Commit before commit, so the entire
/// transaction rolls back (the crash-safety proof). Production passes a hook that
/// always returns `false`; [`no_failpoint`] is that hook.
pub type Failpoint<'a> = &'a mut dyn FnMut(&str) -> bool;

/// A failpoint hook that never trips (production).
pub fn no_failpoint(_: &str) -> bool {
    false
}

/// The immutable coordinates the Commit service stamps into every receipt.
#[derive(Debug, Clone)]
pub struct CommitHostContext {
    /// Stable anchor id.
    pub anchor_id: [u8; 32],
    /// Current signing operator key id.
    pub operator_key_id: [u8; 32],
    /// Current descriptor epoch.
    pub descriptor_epoch: u64,
    /// Current descriptor digest.
    pub descriptor_digest: [u8; 32],
    /// Advertised limit-profile digest bound into the receipt.
    pub limit_profile_digest: [u8; 32],
    /// Seconds added to `now` for the receipt's reported retention horizon.
    pub reported_retention_secs: u64,
}

/// The manifest routing an authorised Commit resolves for the operation's `O`
/// namespace: the digest-matched manifest that authorises the EXACT `C`/`W`
/// namespaces. The service asserts `ordered_namespaces` equals the operation's
/// captured plan before promoting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestAuthorization {
    /// The community id (the `O` root / full site root).
    pub community_id: [u8; 32],
    /// The full site root bound into the receipt.
    pub full_site_root: [u8; 32],
    /// The committed manifest digest bound into the receipt.
    pub manifest_digest: [u8; 32],
    /// The committed manifest version bound into the receipt.
    pub manifest_version: u64,
    /// The ordered `O`, `C`, `W` namespaces the manifest authorises. Must equal the
    /// operation's captured host plan.
    pub ordered_namespaces: [[u8; 32]; 3],
    /// The validated canonical manifest payload bytes, persisted into `manifests`
    /// inside the commit transaction (the `ReadCommitted` / `SubmitListing`
    /// equality source).
    pub manifest_bytes: Vec<u8>,
}

/// The pluggable Commit-time authority. It resolves the digest-matched manifest
/// (or returns a terminal-cleanup manifest/ticket refusal), and gates capacity
/// (the reusable `commit_busy` / `commit_over_quota`).
pub trait HostingAuthority {
    /// The reusable capacity gate. A refusal here (`commit_busy` /
    /// `commit_over_quota`) terminalises only the Commit key.
    fn commit_capacity(
        &self,
        community_root: &[u8; 32],
        observed_at: u64,
    ) -> Result<(), ControlRefusal>;

    /// Resolve the digest-matched manifest authorising the plan's `C`/`W` routing,
    /// or a terminal-cleanup refusal (`commit_manifest_mismatch`,
    /// `manifest_equivocation`, `manifest_transport_mismatch`, `ticket_expired`,
    /// or `invalid_operation_authority`). Receives the open Commit transaction
    /// READ-ONLY: both the persisted ticket and the staged/committed `/manifest`
    /// candidates live in the store, and reading them outside the transaction
    /// would be a TOCTOU hole.
    fn resolve_manifest(
        &self,
        tx: &RepoTransaction<'_>,
        plan: &HostPlanView,
        observed_at: u64,
    ) -> Result<ManifestAuthorization, ControlRefusal>;
}

/// The production Commit-time authority. It reconstructs the manifest authority
/// entirely from durable, root-signed artifacts — never operator say-so:
///
/// 1. the root-signed ticket persisted on the operation row at PrepareHost (the
///    ONLY ticket source; `CommitHostV1` carries none);
/// 2. the owner-signed `/manifest` entry located in the operation's staged `O`
///    union the committed `O` namespace, each candidate re-verified with
///    [`verify_anchor_item_parts`] (payload↔entry binding) THEN
///    [`validate_site_manifest`] (the zero-delegation keystone) — both, in that
///    order;
/// 3. the canonical [`admit_public_site_ticket`] gate WITH the validated manifest
///    attached (gate step 7 is the only legal ticket↔manifest coordinate and
///    transport equality check), re-enforcing expiry and the epoch floor at
///    commit time; and
/// 4. the durable `manifest_floors` rollback floor.
pub struct TicketManifestAuthority;

impl TicketManifestAuthority {
    /// Map a canonical [`AuthorityError`] onto the closed CommitHost refusal
    /// matrix. The three distinguished cases carry actionable detail; every other
    /// authority fault fails closed as `invalid_operation_authority`.
    fn map_authority_error(
        core: &PublicSiteTicketV2Core,
        observed_at: u64,
        observed_manifest_digest: [u8; 32],
        error: AuthorityError,
    ) -> ControlRefusal {
        match error {
            AuthorityError::ExpiredTicket => ControlRefusal::TicketExpired {
                expires_at: core.expiry_unix_seconds,
                observed_at,
            },
            AuthorityError::ManifestTransportMismatch => {
                ControlRefusal::ManifestTransportMismatch {
                    expected_digest: core.manifest_digest,
                    observed_digest: observed_manifest_digest,
                }
            }
            AuthorityError::ManifestMismatch => ControlRefusal::CommitManifestMismatch {
                expected_digest: core.manifest_digest,
                observed_digest: observed_manifest_digest,
            },
            _ => ControlRefusal::InvalidOperationAuthority,
        }
    }
}

/// The deterministic canonical path-bytes encoding of the reserved `/manifest`
/// path (one component), matching `sync_service`'s staged-entry path encoding:
/// `u32be(component_count)` then `u32be(len) || component` per component.
fn manifest_path_bytes() -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + MANIFEST_COMPONENT.len());
    out.extend_from_slice(&1u32.to_be_bytes());
    out.extend_from_slice(&(MANIFEST_COMPONENT.len() as u32).to_be_bytes());
    out.extend_from_slice(MANIFEST_COMPONENT);
    out
}

impl HostingAuthority for TicketManifestAuthority {
    fn commit_capacity(
        &self,
        _community_root: &[u8; 32],
        _observed_at: u64,
    ) -> Result<(), ControlRefusal> {
        // Parity with PrepareHost: capacity accounting is deferred; no
        // `commit_busy` / `commit_over_quota` back-pressure yet.
        Ok(())
    }

    fn resolve_manifest(
        &self,
        tx: &RepoTransaction<'_>,
        plan: &HostPlanView,
        observed_at: u64,
    ) -> Result<ManifestAuthorization, ControlRefusal> {
        // Any store fault inside resolution fails CLOSED as
        // invalid_operation_authority — the trait's refusal vocabulary carries no
        // infrastructure error, and nothing may be promoted on an unread ticket.
        let store_fault = |_| ControlRefusal::InvalidOperationAuthority;

        // 1. The persisted ticket is the EXCLUSIVE ticket source.
        let operation = tx
            .load_operation(&plan.operation_id)
            .map_err(store_fault)?
            .ok_or(ControlRefusal::InvalidOperationAuthority)?;
        let ticket_bytes = operation
            .ticket_envelope_bytes
            .ok_or(ControlRefusal::InvalidOperationAuthority)?;
        let envelope = decode_canonical::<RootSignedTicketCoreEnvelopeV2>(
            &ticket_bytes,
            MAX_TICKET_CORE_BYTES + 128,
        )
        .map_err(|_| ControlRefusal::InvalidOperationAuthority)?;
        let core = envelope.core.clone();

        // 2. Locate `/manifest` candidates in the staged ∪ committed O namespace.
        let o_namespace = plan.ordered_namespaces[0];
        let path = manifest_path_bytes();
        let mut candidates: Vec<Vec<u8>> = tx
            .staged_entries(&plan.operation_id, &o_namespace)
            .map_err(store_fault)?
            .into_iter()
            .filter(|entry| entry.path_bytes == path)
            .map(|entry| entry.item_bytes)
            .collect();
        candidates.extend(
            tx.committed_entries_by_path(&o_namespace, &path)
                .map_err(store_fault)?
                .into_iter()
                .map(|(_, item_bytes)| item_bytes),
        );

        // 3. Verify each candidate — payload↔entry binding FIRST
        //    (verify_anchor_item_parts), the zero-delegation keystone SECOND
        //    (validate_site_manifest) — and select the digest match. A candidate
        //    at the reserved path that fails either check fails the operation
        //    closed: the O stage is corrupt or hostile.
        let mut selected = None;
        let mut observed_digest = [0u8; 32];
        for item_bytes in &candidates {
            let parts = verify_anchor_item_parts(item_bytes)
                .map_err(|_| ControlRefusal::InvalidOperationAuthority)?;
            let signed = SignedWillowEntry {
                entry_bytes: parts.entry_bytes,
                capability_bytes: parts.capability_bytes,
                signature: parts.signature,
                payload_bytes: parts.payload_bytes,
            };
            let validated = validate_site_manifest(&signed, &plan.community_root)
                .map_err(|_| ControlRefusal::InvalidOperationAuthority)?;
            let coordinates = manifest_coordinates(&validated)
                .map_err(|_| ControlRefusal::InvalidOperationAuthority)?;
            observed_digest = coordinates.manifest_digest;
            if coordinates.manifest_digest == core.manifest_digest {
                selected = Some((validated, signed.payload_bytes));
                break;
            }
        }
        let (validated, manifest_bytes) = match selected {
            Some(found) => found,
            None => {
                return Err(ControlRefusal::CommitManifestMismatch {
                    expected_digest: core.manifest_digest,
                    observed_digest,
                })
            }
        };

        // 4. The canonical gate WITH the manifest attached (mirror of the
        //    PrepareHost sibling call, plus `Some(manifest)`): gate step 7 is the
        //    ONLY legal ticket↔manifest coordinate/transport equality check, and
        //    the call re-enforces expiry and the epoch floor at commit time.
        let highest_transport_epoch = tx
            .highest_ticket_transport_epoch(&core.root_id)
            .map_err(store_fault)?;
        admit_public_site_ticket(
            &envelope,
            Some(&validated),
            &TransportFloor::RequireNone,
            &TicketFloor {
                root_id: core.root_id,
                highest_transport_epoch,
            },
            observed_at,
        )
        .map_err(|error| {
            Self::map_authority_error(&core, observed_at, core.manifest_digest, error)
        })?;

        // 5. The durable manifest rollback floor: an older (or same-version,
        //    different-digest) root-signed manifest+ticket pair never rolls the
        //    site backward.
        if let Some((floor_generation, floor_digest)) = tx
            .manifest_floor(&core.o_namespace_id)
            .map_err(store_fault)?
        {
            if floor_generation > core.manifest_version
                || (floor_generation == core.manifest_version
                    && floor_digest != core.manifest_digest)
            {
                return Err(ControlRefusal::ManifestEquivocation {
                    first_digest: floor_digest,
                    second_digest: core.manifest_digest,
                });
            }
        }

        Ok(ManifestAuthorization {
            community_id: core.o_namespace_id,
            full_site_root: core.root_id,
            manifest_digest: core.manifest_digest,
            manifest_version: core.manifest_version,
            ordered_namespaces: [
                core.o_namespace_id,
                core.c_namespace_id,
                core.w_namespace_id,
            ],
            manifest_bytes,
        })
    }
}

/// The operation's captured host plan, projected from its stored prepared response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostPlanView {
    /// The stable operation id.
    pub operation_id: [u8; 32],
    /// The captured base site generation.
    pub base_generation: u64,
    /// The ordered `O`, `C`, `W` namespace ids.
    pub ordered_namespaces: [[u8; 32]; 3],
    /// The ordered `O`, `C`, `W` retained (base) snapshot digests.
    pub base_snapshot_digests: [[u8; 32]; 3],
    /// The community root (the `O` namespace id).
    pub community_root: [u8; 32],
}

/// The composite `CommitHost` service.
pub struct CommitHostService<A: HostingAuthority, S: OperatorSigner> {
    context: CommitHostContext,
    authority: A,
    signer: S,
}

impl<A: HostingAuthority, S: OperatorSigner> CommitHostService<A, S> {
    /// Construct a Commit service.
    pub fn new(context: CommitHostContext, authority: A, signer: S) -> Self {
        Self {
            context,
            authority,
            signer,
        }
    }

    /// Adopt the currently-served descriptor coordinates. The control service
    /// calls this from `install_persisted_descriptor` — the persisted descriptor
    /// wins over construction-time metadata, and a receipt stamped with stale
    /// coordinates would misbind the operator's accountability chain.
    pub fn set_descriptor(&mut self, descriptor_epoch: u64, descriptor_digest: [u8; 32]) {
        self.context.descriptor_epoch = descriptor_epoch;
        self.context.descriptor_digest = descriptor_digest;
    }

    /// Adopt the deployment's reported retention horizon (seconds past `now`
    /// stamped into every receipt). `assemble_service` threads this.
    pub fn set_reported_retention(&mut self, reported_retention_secs: u64) {
        self.context.reported_retention_secs = reported_retention_secs;
    }

    /// Handle one `CommitHost` request against its own idempotency key. The entire
    /// disposition — promotion or refusal — is one transaction. `fp` injects
    /// crash-safety failpoints (pass [`no_failpoint`] in production).
    #[allow(clippy::too_many_arguments)]
    pub fn commit(
        &self,
        repo: &mut AnchorRepository,
        idempotency_key: &[u8; IDEMPOTENCY_KEY_BYTES],
        body: &CommitHostV1,
        control_request_digest: &[u8; 32],
        now: u64,
        entropy: &mut dyn FnMut() -> [u8; 32],
        fp: Failpoint<'_>,
    ) -> Result<ControlResponseV1, CommitError> {
        let mut tx = repo.begin()?;

        // 1. Commit-key idempotency lookup (its own row; a changed body is a
        //    conflict, an equal body replays the byte-identical stored result).
        match classify(
            tx.lookup_idempotency(idempotency_key)?.as_ref(),
            control_request_digest,
        ) {
            AdmissionLookup::ReplayEqual { .. } => {
                let bytes = tx
                    .ordinary_result(control_request_digest)?
                    .ok_or(CommitError::MalformedPlan)?;
                drop(tx);
                return Ok(decode_canonical::<ControlResponseV1>(
                    &bytes,
                    MAX_CONTROL_FRAME_BYTES,
                )?);
            }
            AdmissionLookup::Conflict => {
                drop(tx);
                return Ok(refuse(ControlRefusal::IdempotencyConflict));
            }
            AdmissionLookup::Novel => {}
        }

        // 2. Resolve the operation. A missing operation refuses the Commit request
        //    with no key claim and no mutation.
        let operation = match tx.load_operation(&body.operation_id)? {
            Some(operation) => operation,
            None => {
                drop(tx);
                return Ok(refuse(ControlRefusal::OperationNotFound {
                    operation_id: body.operation_id,
                }));
            }
        };
        // A novel Commit key against an already-terminalised operation replays the
        // operation's terminal outcome without a fresh mutation.
        match operation.status {
            OperationStatus::Committed => {
                let response = committed_response_from_operation(&operation)?;
                drop(tx);
                return Ok(response);
            }
            OperationStatus::Refused => {
                let response = refused_response_from_operation(&operation)?;
                drop(tx);
                return Ok(response);
            }
            OperationStatus::Prepared => {}
        }

        // 3. Operation expiry: terminal cleanup.
        if now >= operation.operation_expiry {
            return self.terminal_cleanup(
                tx,
                &operation,
                idempotency_key,
                control_request_digest,
                ControlRefusal::OperationExpired {
                    operation_id: operation.operation_id,
                    expires_at: operation.operation_expiry,
                },
                now,
                fp,
            );
        }

        // Project the captured host plan from the stored prepared response.
        let plan = match project_plan(&operation) {
            Some(plan) => plan,
            None => return Err(CommitError::MalformedPlan),
        };

        // 4. Capacity gate (reusable): terminalise only the Commit key.
        if let Err(refusal) = self.authority.commit_capacity(&plan.community_root, now) {
            return self.reusable_refuse(
                tx,
                idempotency_key,
                control_request_digest,
                &operation.operation_id,
                refusal,
                now,
                fp,
            );
        }

        // 5. O FIRST: resolve the digest-matched manifest that authorises C/W.
        let manifest = match self.authority.resolve_manifest(&tx, &plan, now) {
            Ok(manifest) => manifest,
            Err(refusal) => {
                return self.terminal_cleanup(
                    tx,
                    &operation,
                    idempotency_key,
                    control_request_digest,
                    refusal,
                    now,
                    fp,
                )
            }
        };
        // The manifest must authorise EXACTLY the captured O/C/W routing.
        if manifest.ordered_namespaces != plan.ordered_namespaces {
            let observed = compute_snapshot_digest(
                &plan.community_root,
                0,
                &manifest
                    .ordered_namespaces
                    .iter()
                    .map(|n| n.to_vec())
                    .collect::<Vec<_>>(),
            );
            return self.terminal_cleanup(
                tx,
                &operation,
                idempotency_key,
                control_request_digest,
                ControlRefusal::CommitManifestMismatch {
                    expected_digest: manifest.manifest_digest,
                    observed_digest: observed,
                },
                now,
                fp,
            );
        }

        // 6. Verify every staged entry (REAL Meadowcap) and check declared digests,
        //    O then C then W.
        let mut resulting: Vec<NamespaceUnion> = Vec::with_capacity(3);
        for (index, namespace_id) in plan.ordered_namespaces.iter().enumerate() {
            let union = match self.build_and_verify_union(
                &tx,
                &operation.operation_id,
                namespace_id,
                &body.ordered_namespace_snapshot_digests[index],
            )? {
                Ok(union) => union,
                Err(refusal) => {
                    return self.terminal_cleanup(
                        tx,
                        &operation,
                        idempotency_key,
                        control_request_digest,
                        refusal,
                        now,
                        fp,
                    )
                }
            };
            resulting.push(union);
        }

        // 7. Generation CAS: exactly one same-base commit wins.
        let committed_generation = plan.base_generation.saturating_add(1);
        if fp("cas") {
            return Err(CommitError::Failpoint("cas"));
        }
        match tx.commit_generation_cas(
            &manifest.community_id,
            now,
            plan.base_generation,
            committed_generation,
        )? {
            GenerationCas::Committed => {}
            GenerationCas::Stale { current_generation } => {
                let current_digests = self.current_committed_digests(&tx, &plan)?;
                return self.terminal_cleanup(
                    tx,
                    &operation,
                    idempotency_key,
                    control_request_digest,
                    ControlRefusal::StaleBase {
                        current_generation,
                        ordered_namespace_snapshot_digests: current_digests,
                    },
                    now,
                    fp,
                );
            }
        }

        // 7b. The committed-manifest state, in the SAME transaction: the CAS
        //     above ensured the community row, so the `manifests` /
        //     `manifest_floors` foreign keys resolve, and a crash leaves the
        //     manifest rows exactly as promoted or exactly absent.
        if fp("manifest") {
            return Err(CommitError::Failpoint("manifest"));
        }
        tx.upsert_manifest(
            &manifest.community_id,
            manifest.manifest_version,
            &manifest.manifest_digest,
            &manifest.manifest_bytes,
        )?;
        tx.advance_manifest_floor(
            &manifest.community_id,
            manifest.manifest_version,
            &manifest.manifest_digest,
        )?;

        // 8. Promote O, then C, then W (payload refs + committed entries).
        for (index, union) in resulting.iter().enumerate() {
            for entry in &union.staged {
                tx.insert_committed_entry(&manifest.community_id, index as u8, entry)?;
            }
            if fp("promote") {
                return Err(CommitError::Failpoint("promote"));
            }
        }

        // Build the signed receipt.
        let mut ordered_namespace_results = Vec::with_capacity(3);
        for (index, union) in resulting.iter().enumerate() {
            ordered_namespace_results.push(NamespaceResult {
                namespace_id: plan.ordered_namespaces[index],
                snapshot_digest: body.ordered_namespace_snapshot_digests[index],
                entry_count: union.total_count,
            });
        }
        let receipt = self.sign_receipt(HostingReceiptBodyV1 {
            anchor_id: self.context.anchor_id,
            operator_key_id: self.context.operator_key_id,
            descriptor_epoch: self.context.descriptor_epoch,
            descriptor_digest: self.context.descriptor_digest,
            hosting_operation_id: operation.operation_id,
            full_site_root: manifest.full_site_root,
            manifest_digest: manifest.manifest_digest,
            manifest_version: manifest.manifest_version,
            base_site_generation: plan.base_generation,
            committed_site_generation: committed_generation,
            ordered_namespace_results,
            status: HostingStatus::Committed,
            accepted_at: now,
            reported_retention_through: now.saturating_add(self.context.reported_retention_secs),
            limit_profile_digest: self.context.limit_profile_digest,
        })?;

        let receipt_bytes = receipt.encode_canonical()?;
        let response = ControlResponseV1 {
            kind: ControlOperationKind::CommitHost,
            outcome: ControlOutcome::Success(ControlSuccess::CommitHost(Box::new(receipt.clone()))),
        };
        let response_bytes = response.encode_canonical()?;

        // 9. Atomic terminalisation: receipt row, operation terminal (which
        //    invalidates the namespace tokens by leaving `prepared`), staging
        //    deleted, Commit key terminalised with its byte-identical result.
        if fp("receipt") {
            return Err(CommitError::Failpoint("receipt"));
        }
        let receipt_id = entropy();
        tx.insert_hosting_receipt(&receipt_id, &manifest.community_id, now, &receipt_bytes)?;
        tx.set_operation_terminal(
            &operation.operation_id,
            OperationStatus::Committed,
            &receipt_bytes,
        )?;
        if fp("terminal") {
            return Err(CommitError::Failpoint("terminal"));
        }
        tx.delete_staging_for_operation(&operation.operation_id)?;
        tx.claim_idempotency(
            control_request_digest,
            idempotency_key,
            RESULT_CLASS_ORDINARY,
            IdempotencyClaimState::Terminal,
            Some(&operation.operation_id),
            None,
            now,
            now.saturating_add(TERMINAL_RETENTION_SECS),
        )?;
        tx.store_ordinary_result(control_request_digest, &response_bytes)?;
        if fp("commit") {
            return Err(CommitError::Failpoint("commit"));
        }
        tx.commit()?;
        Ok(response)
    }

    /// Read a namespace's staged entries, re-verify each (REAL Meadowcap + bounds),
    /// compute the committed-∪-staged snapshot digest, and compare it to the
    /// client's declared digest. `Ok(Ok(union))` promotes; `Ok(Err(refusal))` is a
    /// terminal-cleanup refusal.
    fn build_and_verify_union(
        &self,
        tx: &RepoTransaction<'_>,
        operation_id: &[u8; 32],
        namespace_id: &[u8; 32],
        declared_digest: &[u8; 32],
    ) -> Result<Result<NamespaceUnion, ControlRefusal>, CommitError> {
        let staged = tx.staged_entries(operation_id, namespace_id)?;
        // Defense in depth: the anchor NEVER promotes an entry it has not itself
        // verified. A forged/unauthorised staged entry is `invalid_operation_authority`.
        for entry in &staged {
            if verify_anchor_item(&entry.item_bytes).is_err() {
                return Ok(Err(ControlRefusal::InvalidOperationAuthority));
            }
        }
        let committed = tx.committed_entries(namespace_id)?;
        let mut ids: Vec<Vec<u8>> = committed.iter().map(|(id, _)| id.clone()).collect();
        let mut logical: u64 = committed.iter().map(|(_, item)| item.len() as u64).sum();
        for entry in &staged {
            ids.push(entry.entry_id.to_vec());
            logical += entry.item_bytes.len() as u64;
        }
        let observed = compute_snapshot_digest(namespace_id, logical, &ids);
        if &observed != declared_digest {
            return Ok(Err(ControlRefusal::SnapshotMismatch {
                expected_snapshot_digest: *declared_digest,
                observed_snapshot_digest: observed,
            }));
        }
        let total_count = committed.len() as u64 + staged.len() as u64;
        Ok(Ok(NamespaceUnion {
            staged,
            total_count,
        }))
    }

    fn current_committed_digests(
        &self,
        tx: &RepoTransaction<'_>,
        plan: &HostPlanView,
    ) -> Result<[[u8; 32]; 3], CommitError> {
        let mut digests = [[0u8; 32]; 3];
        for (index, namespace_id) in plan.ordered_namespaces.iter().enumerate() {
            let committed = tx.committed_entries(namespace_id)?;
            let ids: Vec<Vec<u8>> = committed.iter().map(|(id, _)| id.clone()).collect();
            let logical: u64 = committed.iter().map(|(_, item)| item.len() as u64).sum();
            digests[index] = compute_snapshot_digest(namespace_id, logical, &ids);
        }
        Ok(digests)
    }

    /// Terminal-cleanup disposition (one transaction): store the refused operation
    /// outcome (which invalidates the namespace tokens by leaving `prepared`),
    /// delete all staging, and terminalise the Commit key with its byte-identical
    /// refusal result.
    #[allow(clippy::too_many_arguments)]
    fn terminal_cleanup(
        &self,
        mut tx: RepoTransaction<'_>,
        operation: &StoredOperation,
        idempotency_key: &[u8; IDEMPOTENCY_KEY_BYTES],
        control_request_digest: &[u8; 32],
        refusal: ControlRefusal,
        now: u64,
        fp: Failpoint<'_>,
    ) -> Result<ControlResponseV1, CommitError> {
        let response = refuse(refusal.clone());
        let response_bytes = response.encode_canonical()?;
        let refusal_bytes = refusal.encode_canonical()?;
        if fp("cleanup.operation") {
            return Err(CommitError::Failpoint("cleanup.operation"));
        }
        tx.set_operation_terminal(
            &operation.operation_id,
            OperationStatus::Refused,
            &refusal_bytes,
        )?;
        if fp("cleanup.staging") {
            return Err(CommitError::Failpoint("cleanup.staging"));
        }
        tx.delete_staging_for_operation(&operation.operation_id)?;
        tx.claim_idempotency(
            control_request_digest,
            idempotency_key,
            RESULT_CLASS_ORDINARY,
            IdempotencyClaimState::Terminal,
            Some(&operation.operation_id),
            None,
            now,
            now.saturating_add(TERMINAL_RETENTION_SECS),
        )?;
        tx.store_ordinary_result(control_request_digest, &response_bytes)?;
        if fp("cleanup.commit") {
            return Err(CommitError::Failpoint("cleanup.commit"));
        }
        tx.commit()?;
        Ok(response)
    }

    /// Reusable disposition (one transaction): terminalise only the Commit key; the
    /// operation stays `prepared` with valid staging and tokens.
    #[allow(clippy::too_many_arguments)]
    fn reusable_refuse(
        &self,
        mut tx: RepoTransaction<'_>,
        idempotency_key: &[u8; IDEMPOTENCY_KEY_BYTES],
        control_request_digest: &[u8; 32],
        operation_id: &[u8; 32],
        refusal: ControlRefusal,
        now: u64,
        fp: Failpoint<'_>,
    ) -> Result<ControlResponseV1, CommitError> {
        let response = refuse(refusal);
        let response_bytes = response.encode_canonical()?;
        if fp("reusable.write") {
            return Err(CommitError::Failpoint("reusable.write"));
        }
        tx.claim_idempotency(
            control_request_digest,
            idempotency_key,
            RESULT_CLASS_ORDINARY,
            IdempotencyClaimState::Terminal,
            Some(operation_id),
            None,
            now,
            now.saturating_add(TERMINAL_RETENTION_SECS),
        )?;
        tx.store_ordinary_result(control_request_digest, &response_bytes)?;
        if fp("reusable.commit") {
            return Err(CommitError::Failpoint("reusable.commit"));
        }
        tx.commit()?;
        Ok(response)
    }

    fn sign_receipt(&self, body: HostingReceiptBodyV1) -> Result<HostingReceiptV1, CommitError> {
        let mut envelope = OperatorSignedEnvelopeV1 {
            body,
            operator_signature: [0u8; 64],
        };
        let preimage = envelope.signing_preimage()?;
        envelope.operator_signature = self.signer.sign(&preimage);
        Ok(envelope)
    }
}

struct NamespaceUnion {
    staged: Vec<crate::repository::StagedEntry>,
    total_count: u64,
}

fn refuse(refusal: ControlRefusal) -> ControlResponseV1 {
    ControlResponseV1 {
        kind: ControlOperationKind::CommitHost,
        outcome: ControlOutcome::Refused(refusal),
    }
}

fn project_plan(operation: &StoredOperation) -> Option<HostPlanView> {
    let success = ordered_host_plan(&operation.prepare_response_bytes)?;
    Some(HostPlanView {
        operation_id: operation.operation_id,
        base_generation: success.base_site_generation,
        ordered_namespaces: success.ordered_namespace_host_plan,
        base_snapshot_digests: success.ordered_retained_snapshot_digests,
        community_root: success.ordered_namespace_host_plan[0],
    })
}

fn committed_response_from_operation(
    operation: &StoredOperation,
) -> Result<ControlResponseV1, CommitError> {
    let bytes = operation
        .terminal_result_bytes
        .as_ref()
        .ok_or(CommitError::MalformedPlan)?;
    let receipt = decode_canonical::<HostingReceiptV1>(bytes, MAX_CONTROL_FRAME_BYTES)?;
    Ok(ControlResponseV1 {
        kind: ControlOperationKind::CommitHost,
        outcome: ControlOutcome::Success(ControlSuccess::CommitHost(Box::new(receipt))),
    })
}

fn refused_response_from_operation(
    operation: &StoredOperation,
) -> Result<ControlResponseV1, CommitError> {
    let bytes = operation
        .terminal_result_bytes
        .as_ref()
        .ok_or(CommitError::MalformedPlan)?;
    let refusal = decode_canonical::<ControlRefusal>(bytes, MAX_CONTROL_FRAME_BYTES)?;
    Ok(refuse(refusal))
}
