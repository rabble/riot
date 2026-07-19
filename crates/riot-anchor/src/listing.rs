//! The ordinary `SubmitListing` service: the directory's atomic admission boundary.
//!
//! This is the **trust root of the public directory**. The pure state machine
//! [`resolve_listing`](riot_anchor_protocol::authority::resolve_listing) in
//! `riot-anchor-protocol` decides display authority from an
//! [`AdmittedListingEnvelopeV1`] whose bytes it *never authenticates* — the entry
//! signature is unchecked, the Meadowcap chain is ignored, and the delegate grant
//! carries no signature (see `docs/research/2026-07-19-wu003b-security-findings.md`,
//! the HIGH finding). Therefore **this service does every cryptographic check
//! BEFORE it constructs an `AdmittedListingEnvelopeV1`**, and it is the only place
//! in the crate that constructs one destined for `resolve_listing`. A forged
//! listing entry, a forged/absent delegate-grant signature, a delegated cap
//! masquerading as root-owned, or a listing whose coordinates disagree with its
//! embedded root-signed ticket are ALL refused with the single closed reason
//! `invalid_listing_authority` (no oracle) and never reach the directory.
//!
//! ## crypto-before-admit ([`SubmitListingService::verify_submission`])
//!
//! 1. REAL Meadowcap + Ed25519 verification of the signed listing entry via
//!    [`verify_anchor_item_parts`] → [`riot_core::willow::verify_entry`];
//! 2. the entry is at exactly `O:/directory/listing` and lives in namespace `O`
//!    (`== root_id == o_namespace_id`, the "signed by `root_id`" binding);
//! 3. authority class: a grant-absent listing MUST carry a zero-delegation
//!    capability (only the root secret can mint an owned cap over `O`), so a
//!    delegate cannot pose as root and seize/seal the epoch; a delegated listing's
//!    grant signature is verified (root-signed) and binds the exact author key and
//!    epoch;
//! 4. internal-consistency self-check: the embedded `ticket_core_bytes` is
//!    re-decoded, its root signature verified, and its `root/O/C/W/manifest`
//!    coordinates must equal the listing's.
//!
//! Only then is the `AdmittedListingEnvelopeV1` built and passed to
//! `resolve_listing`.
//!
//! ## one transaction
//!
//! An accepted (shown) listing or refresh is a single
//! [`RepoTransaction`]: it claims the global idempotency key, appends EXACTLY ONE
//! signed inclusion (advancing the directory feed head), sets/refreshes the
//! current listing state (retaining the signed feed history), invalidates the
//! directory/search projection generation EXACTLY ONCE, persists the resolved
//! listing floor, creates the signed [`ListingReceiptV1`], and stores the
//! byte-identical terminal result. A crash at any durable mutation is wholly
//! absent or wholly committed. A same-key/same-body retry replays those exact
//! bytes without a second inclusion; a same-key/changed-body retry is
//! `idempotency_conflict` with no disclosure. Listing before hosting, a stale
//! generation, or a manifest mismatch is refused WITHOUT creating durable state.

use riot_anchor_protocol::authority::{
    admit_public_site_ticket, resolve_listing, ListingFloor, ListingOutcome, TicketFloor,
};
use riot_anchor_protocol::codec::{decode_canonical, CanonicalRecord, CodecError};
use riot_anchor_protocol::control::{
    ControlOutcome, ControlRefusal, ControlResponseV1, ControlSuccess, MAX_CONTROL_FRAME_BYTES,
};
use riot_anchor_protocol::digest::digest_v1;
use riot_anchor_protocol::records::{
    AdmittedListingEnvelopeV1, CommunityListingV1, ControlOperationKind, ListingDelegateGrantV1,
    ListingReceiptBodyV1, ListingReceiptV1, OperatorSignedEnvelopeV1,
    RootSignedTicketCoreEnvelopeV2, TransportFloor, IDEMPOTENCY_KEY_BYTES,
    MAX_DELEGATE_GRANT_BYTES, MAX_TICKET_CORE_BYTES,
};

use riot_core::willow::{decode_capability_canonic, is_directory_listing};
use willow25::groupings::{Keylike, Namespaced};

use crate::idempotency::{
    classify, AdmissionLookup, RESULT_CLASS_ORDINARY, TERMINAL_RETENTION_SECS,
};
use crate::repository::{
    AnchorRepository, AnchorRepositoryError, IdempotencyClaimState, RepoTransaction,
};
use crate::sync_service::{verify_anchor_item_parts, VerifiedAnchorItem};
use crate::work::OperatorSigner;

/// The signing domain for a directory-inclusion body (operator-signed).
const DIRECTORY_INCLUSION_SIGNING_DOMAIN: &[u8] = b"riot/directory-inclusion/v1";
/// The `digest_v1` label for a signed directory-inclusion record.
const DIRECTORY_INCLUSION_ENVELOPE_LABEL: &[u8] = b"riot/directory-inclusion-envelope/v1";
/// Inclusion body wire version.
const DIRECTORY_INCLUSION_VERSION: u8 = 1;

/// A failpoint hook (mirrors [`crate::hosting`]): the service calls it before each
/// durable mutation with a stable label; returning `true` aborts before commit so
/// the whole transaction rolls back. Production passes [`no_failpoint`].
pub type Failpoint<'a> = &'a mut dyn FnMut(&str) -> bool;

/// A failpoint hook that never trips (production).
pub fn no_failpoint(_: &str) -> bool {
    false
}

/// The immutable coordinates the listing service stamps into every receipt.
#[derive(Debug, Clone)]
pub struct ListingContext {
    /// Stable anchor id.
    pub anchor_id: [u8; 32],
    /// Current signing operator key id.
    pub operator_key_id: [u8; 32],
    /// Current descriptor epoch.
    pub descriptor_epoch: u64,
    /// Current descriptor digest.
    pub descriptor_digest: [u8; 32],
}

/// The cryptographically verified listing coordinates handed to the authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedListingCoordinates {
    /// Full `O` root key (`== o_namespace_id`).
    pub root_id: [u8; 32],
    /// `O` namespace id.
    pub o_namespace_id: [u8; 32],
    /// `C` namespace id.
    pub c_namespace_id: [u8; 32],
    /// `W` namespace id.
    pub w_namespace_id: [u8; 32],
    /// Bound manifest digest.
    pub manifest_digest: [u8; 32],
    /// Bound manifest version.
    pub manifest_version: u64,
}

/// The anchor's current hosting state for a verified site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostingState {
    /// The full site root bound into the receipt.
    pub full_site_root: [u8; 32],
    /// The site's current committed generation.
    pub current_site_generation: u64,
}

/// The pluggable listing-time authority: given VERIFIED coordinates, it decides
/// whether the site is currently hosted at its committed generation with a
/// matching manifest, or returns a closed refusal (`not_hosted`, `stale_base`,
/// `listing_manifest_mismatch`). It never constructs an admitted record and never
/// mutates the store. A valid listing that arrives before hosting is rejected
/// here (design: "rejected rather than queued").
pub trait ListingAuthority {
    /// Resolve current hosting for a verified listing, or a closed refusal.
    fn resolve_hosting(
        &self,
        coordinates: &VerifiedListingCoordinates,
        observed_at: u64,
    ) -> Result<HostingState, ControlRefusal>;
}

/// A root-signed delegate grant received SEPARATELY from the admitted envelope
/// (the envelope carries only the grant body, never its signature).
#[derive(Debug, Clone)]
pub struct RawDelegateGrant {
    /// Canonical [`ListingDelegateGrantV1`] body bytes.
    pub grant_bytes: Vec<u8>,
    /// The `O`-root Ed25519 signature over the grant.
    pub signature: [u8; 64],
}

/// The raw, untrusted materials a client submits. The full signed Willow listing
/// entry (item format: entry + capability + 64-byte signature + payload, where the
/// payload is the canonical [`CommunityListingV1`]) plus, for a delegated listing,
/// the root-signed grant. The wire `SubmitListingV1` body cannot yet carry the
/// entry/grant signatures; the wire→service adapter is a later work unit (FLAGGED).
#[derive(Debug, Clone)]
pub struct RawListingSubmission {
    /// The complete signed Willow listing entry in anchor-item format.
    pub listing_item_bytes: Vec<u8>,
    /// `None` = root-owned; `Some` = delegated, with the separately supplied grant.
    pub delegate_grant: Option<RawDelegateGrant>,
}

/// An error that prevents the listing service from producing any control result.
#[derive(Debug)]
#[non_exhaustive]
pub enum ListingError {
    /// A durable-store error.
    Repository(AnchorRepositoryError),
    /// A canonical-encoding error building the receipt/response.
    Codec(CodecError),
    /// An injected failpoint tripped before commit (test-only).
    Failpoint(&'static str),
}

impl core::fmt::Display for ListingError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Repository(error) => write!(formatter, "listing repository error: {error}"),
            Self::Codec(error) => write!(formatter, "listing codec error: {error:?}"),
            Self::Failpoint(label) => write!(formatter, "listing failpoint tripped: {label}"),
        }
    }
}

impl std::error::Error for ListingError {}

impl From<AnchorRepositoryError> for ListingError {
    fn from(error: AnchorRepositoryError) -> Self {
        Self::Repository(error)
    }
}

impl From<CodecError> for ListingError {
    fn from(error: CodecError) -> Self {
        Self::Codec(error)
    }
}

/// The verified, admit-ready listing produced by `verify_submission`.
struct VerifiedListing {
    envelope: AdmittedListingEnvelopeV1,
    listing: CommunityListingV1,
    listing_digest: [u8; 32],
    coordinates: VerifiedListingCoordinates,
}

/// The ordinary `SubmitListing` service.
pub struct SubmitListingService<A: ListingAuthority, S: OperatorSigner> {
    context: ListingContext,
    authority: A,
    signer: S,
}

impl<A: ListingAuthority, S: OperatorSigner> SubmitListingService<A, S> {
    /// Construct a listing service.
    pub fn new(context: ListingContext, authority: A, signer: S) -> Self {
        Self {
            context,
            authority,
            signer,
        }
    }

    /// Handle one ordinary `SubmitListing` request against its own idempotency key.
    /// The entire disposition — admission or refusal — is one transaction. `fp`
    /// injects crash-safety failpoints (pass [`no_failpoint`] in production).
    #[allow(clippy::too_many_arguments)]
    pub fn submit(
        &self,
        repo: &mut AnchorRepository,
        idempotency_key: &[u8; IDEMPOTENCY_KEY_BYTES],
        submission: &RawListingSubmission,
        control_request_digest: &[u8; 32],
        now: u64,
        fp: Failpoint<'_>,
    ) -> Result<ControlResponseV1, ListingError> {
        let tx = repo.begin()?;

        // 1. Idempotency lookup FIRST: an equal-body retry replays the stored
        //    terminal bytes (no second inclusion); a changed body is a conflict
        //    without disclosure.
        match classify(
            tx.lookup_idempotency(idempotency_key)?.as_ref(),
            control_request_digest,
        ) {
            AdmissionLookup::ReplayEqual { .. } => {
                let bytes = tx
                    .ordinary_result(control_request_digest)?
                    .ok_or(ListingError::Codec(CodecError::Malformed))?;
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

        // 2. crypto-before-admit. A failure here creates NO durable state and does
        //    NOT claim the key (a corrected resubmit under the same key may later
        //    succeed) — every reason collapses to `invalid_listing_authority`.
        let verified = match self.verify_submission(submission, now) {
            Ok(verified) => verified,
            Err(refusal) => {
                drop(tx);
                return Ok(refuse(refusal));
            }
        };

        // 3. Hosting authority: listing-before-hosting / stale generation /
        //    manifest mismatch are refused WITHOUT durable state and WITHOUT a claim.
        let hosting = match self.authority.resolve_hosting(&verified.coordinates, now) {
            Ok(hosting) => hosting,
            Err(refusal) => {
                drop(tx);
                return Ok(refuse(refusal));
            }
        };

        // 4. Resolve against the durable per-root floor.
        let floor = tx
            .load_listing_floor(&verified.coordinates.root_id)?
            .unwrap_or_else(|| ListingFloor::new(verified.coordinates.root_id));
        let transition = match resolve_listing(&floor, &verified.envelope, now) {
            Ok(transition) => transition,
            Err(error) => {
                drop(tx);
                return Ok(refuse(map_authority_error(&error, &verified.listing, now)));
            }
        };

        match transition.outcome {
            ListingOutcome::Shown => self.accept(
                tx,
                idempotency_key,
                control_request_digest,
                &verified,
                &transition.floor,
                &hosting,
                now,
                fp,
            ),
            ListingOutcome::Deduplicated => {
                // A byte-identical listing under a NEW idempotency key: no second
                // inclusion. Re-issue a receipt at the current feed head and store
                // the terminal result under this key.
                self.deduplicate(
                    tx,
                    idempotency_key,
                    control_request_digest,
                    &verified,
                    &hosting,
                    now,
                    fp,
                )
            }
            ListingOutcome::Equivocation => {
                // Same coordinates, different digest: the directory shows neither.
                // Persist the poisoned floor and invalidate the projection so the
                // real listing is censored consistently, then refuse.
                self.equivocate(tx, &transition.floor, &verified, now, fp)
            }
            // Stale-revision (`Superseded`) and ordinary-path tombstones
            // (`Unlisted`) are not admitted by the ordinary handler; unlisting is
            // the reserved removal operation (WU-016). Fail closed with no durable
            // state. (Not reachable for the listed=true winning submissions the DoD
            // exercises; documented conservative mapping.)
            ListingOutcome::Superseded | ListingOutcome::Unlisted => {
                drop(tx);
                Ok(refuse(ControlRefusal::InvalidListingAuthority))
            }
        }
    }

    /// The crypto-before-admit boundary. Returns the admit-ready envelope only when
    /// every signature and internal-consistency check passes.
    fn verify_submission(
        &self,
        submission: &RawListingSubmission,
        now: u64,
    ) -> Result<VerifiedListing, ControlRefusal> {
        // (1) REAL Meadowcap + signature verification of the listing entry.
        let VerifiedAnchorItem {
            entry,
            capability_bytes,
            payload_bytes,
            ..
        } = verify_anchor_item_parts(&submission.listing_item_bytes)
            .map_err(|_| ControlRefusal::InvalidListingAuthority)?;

        // The entry payload IS the canonical CommunityListingV1.
        let listing = decode_canonical::<CommunityListingV1>(
            &payload_bytes,
            riot_anchor_protocol::records::MAX_LISTING_ENVELOPE_BYTES,
        )
        .map_err(|_| ControlRefusal::InvalidListingAuthority)?;

        // Ordinary path handles listing/refresh only (listed == true). Unlisting is
        // the reserved removal operation.
        if !listing.listed {
            return Err(ControlRefusal::InvalidListingAuthority);
        }

        // (2) exact coordinate + namespace binding: the entry is at
        // O:/directory/listing and lives in namespace O == root_id == o_namespace_id
        // (the "signed by root_id" binding).
        if !is_directory_listing(Keylike::path(&entry)) {
            return Err(ControlRefusal::InvalidListingAuthority);
        }
        let entry_namespace = *entry.namespace_id().as_bytes();
        if entry_namespace != listing.root_id || entry_namespace != listing.o_namespace_id {
            return Err(ControlRefusal::InvalidListingAuthority);
        }
        let author_subspace = *Keylike::subspace_id(&entry).as_bytes();

        // (3) authority class must match the capability shape and, for a delegated
        // listing, a verified root-signed grant.
        let capability = decode_capability_canonic(&capability_bytes)
            .map_err(|_| ControlRefusal::InvalidListingAuthority)?;
        let zero_delegation = capability.delegations().is_empty();
        match &submission.delegate_grant {
            None => {
                // Root-owned: only the root secret can mint a zero-delegation owned
                // cap over O. A delegated cap presented as root-owned would let a
                // delegate seize + seal the epoch — reject it.
                if !zero_delegation {
                    return Err(ControlRefusal::InvalidListingAuthority);
                }
            }
            Some(raw_grant) => {
                if zero_delegation {
                    return Err(ControlRefusal::InvalidListingAuthority);
                }
                let grant = decode_canonical::<ListingDelegateGrantV1>(
                    &raw_grant.grant_bytes,
                    MAX_DELEGATE_GRANT_BYTES,
                )
                .map_err(|_| ControlRefusal::InvalidListingAuthority)?;
                // The grant's OWN root signature (never present in the envelope).
                riot_anchor_protocol::authority::verify_listing_delegate_grant(
                    &grant,
                    &raw_grant.signature,
                )
                .map_err(|_| ControlRefusal::InvalidListingAuthority)?;
                // The grant must bind the exact root, epoch, and the entry's author.
                if grant.root_id != listing.root_id
                    || grant.listing_epoch != listing.listing_epoch
                    || grant.delegate_key != author_subspace
                {
                    return Err(ControlRefusal::InvalidListingAuthority);
                }
            }
        }

        // (4) internal-consistency self-check: the embedded root-signed ticket must
        // verify AND carry byte-identical coordinates.
        let ticket_envelope = decode_canonical::<RootSignedTicketCoreEnvelopeV2>(
            &listing.ticket_core_bytes,
            MAX_TICKET_CORE_BYTES + 128,
        )
        .map_err(|_| ControlRefusal::InvalidListingAuthority)?;
        let admitted = admit_public_site_ticket(
            &ticket_envelope,
            None,
            &TransportFloor::RequireNone,
            &TicketFloor {
                root_id: listing.root_id,
                highest_transport_epoch: None,
            },
            now,
        )
        .map_err(|_| ControlRefusal::InvalidListingAuthority)?;
        let core = &admitted.core;
        if core.root_id != listing.root_id
            || core.o_namespace_id != listing.o_namespace_id
            || core.c_namespace_id != listing.c_namespace_id
            || core.w_namespace_id != listing.w_namespace_id
            || core.manifest_digest != listing.manifest_digest
            || core.manifest_version != listing.manifest_version
        {
            return Err(ControlRefusal::InvalidListingAuthority);
        }

        // Only NOW is an AdmittedListingEnvelopeV1 constructed. Its
        // `signed_listing_entry_bytes` is the verified canonical listing payload —
        // exactly what `resolve_listing` decodes.
        let envelope = AdmittedListingEnvelopeV1 {
            signed_listing_entry_bytes: payload_bytes,
            capability_chain_bytes: capability_bytes,
            delegate_grant_bytes: submission
                .delegate_grant
                .as_ref()
                .map(|grant| grant.grant_bytes.clone()),
        };
        let listing_digest = envelope
            .listing_digest()
            .map_err(|_| ControlRefusal::InvalidListingAuthority)?;

        let coordinates = VerifiedListingCoordinates {
            root_id: listing.root_id,
            o_namespace_id: listing.o_namespace_id,
            c_namespace_id: listing.c_namespace_id,
            w_namespace_id: listing.w_namespace_id,
            manifest_digest: listing.manifest_digest,
            manifest_version: listing.manifest_version,
        };

        Ok(VerifiedListing {
            envelope,
            listing,
            listing_digest,
            coordinates,
        })
    }

    /// The accept path (outcome `Shown`): one transaction that appends exactly one
    /// signed inclusion, sets/refreshes current listing state, invalidates the
    /// projection once, persists the floor, and stores the signed receipt +
    /// byte-identical terminal result.
    #[allow(clippy::too_many_arguments)]
    fn accept(
        &self,
        mut tx: RepoTransaction<'_>,
        idempotency_key: &[u8; IDEMPOTENCY_KEY_BYTES],
        control_request_digest: &[u8; 32],
        verified: &VerifiedListing,
        next_floor: &ListingFloor,
        hosting: &HostingState,
        now: u64,
        fp: Failpoint<'_>,
    ) -> Result<ControlResponseV1, ListingError> {
        let community_id = verified.coordinates.o_namespace_id;
        let root_key = verified.coordinates.root_id;
        let expires_at = verified.listing.expiry_unix_seconds;

        // (a) current listing state (new listing reserves a removal slot; refresh
        //     replaces state and RETAINS the reserved slot + feed history).
        if fp("listing_state") {
            return Err(ListingError::Failpoint("listing_state"));
        }
        match tx.current_listing(&community_id)? {
            None => {
                let slot =
                    tx.claim_removal_slot(&community_id, &root_key, control_request_digest)?;
                tx.insert_listing(&community_id, &root_key, now, expires_at, now, slot)?;
            }
            Some(_) => {
                tx.update_listing(&community_id, &root_key, now, expires_at, now)?;
            }
        }

        // (b) append EXACTLY ONE signed inclusion and advance the feed head.
        if fp("inclusion") {
            return Err(ListingError::Failpoint("inclusion"));
        }
        let sequence =
            self.append_inclusion(&mut tx, &community_id, verified.listing_digest, true, now)?;

        // (c) invalidate the directory/search projection generation EXACTLY ONCE.
        if fp("projection") {
            return Err(ListingError::Failpoint("projection"));
        }
        tx.invalidate_projection_generation()?;

        // (d) persist the resolved floor.
        tx.upsert_listing_floor(next_floor)?;

        // (e) sign the receipt and build the terminal response.
        if fp("receipt") {
            return Err(ListingError::Failpoint("receipt"));
        }
        let response = self.listing_response(verified, hosting, sequence, now, idempotency_key)?;
        let response_bytes = response.encode_canonical()?;

        // (f) claim the idempotency key terminal + store the byte-identical result.
        if fp("terminal") {
            return Err(ListingError::Failpoint("terminal"));
        }
        tx.claim_idempotency(
            control_request_digest,
            idempotency_key,
            RESULT_CLASS_ORDINARY,
            IdempotencyClaimState::Terminal,
            None,
            None,
            now,
            now.saturating_add(TERMINAL_RETENTION_SECS),
        )?;
        tx.store_ordinary_result(control_request_digest, &response_bytes)?;

        if fp("commit") {
            return Err(ListingError::Failpoint("commit"));
        }
        tx.commit()?;
        Ok(response)
    }

    /// A byte-identical listing under a new key: no second inclusion. Re-issue a
    /// receipt at the current feed head and terminalise this key.
    #[allow(clippy::too_many_arguments)]
    fn deduplicate(
        &self,
        mut tx: RepoTransaction<'_>,
        idempotency_key: &[u8; IDEMPOTENCY_KEY_BYTES],
        control_request_digest: &[u8; 32],
        verified: &VerifiedListing,
        hosting: &HostingState,
        now: u64,
        fp: Failpoint<'_>,
    ) -> Result<ControlResponseV1, ListingError> {
        let (_, sequence) = tx.feed_head()?;
        if fp("receipt") {
            return Err(ListingError::Failpoint("receipt"));
        }
        let response = self.listing_response(verified, hosting, sequence, now, idempotency_key)?;
        let response_bytes = response.encode_canonical()?;
        if fp("terminal") {
            return Err(ListingError::Failpoint("terminal"));
        }
        tx.claim_idempotency(
            control_request_digest,
            idempotency_key,
            RESULT_CLASS_ORDINARY,
            IdempotencyClaimState::Terminal,
            None,
            None,
            now,
            now.saturating_add(TERMINAL_RETENTION_SECS),
        )?;
        tx.store_ordinary_result(control_request_digest, &response_bytes)?;
        if fp("commit") {
            return Err(ListingError::Failpoint("commit"));
        }
        tx.commit()?;
        Ok(response)
    }

    /// Equivocation: persist the poisoned floor, invalidate the projection so the
    /// real listing is censored consistently, and refuse. No key is claimed (the
    /// poisoned floor makes any identical resubmission equivocate again).
    fn equivocate(
        &self,
        mut tx: RepoTransaction<'_>,
        next_floor: &ListingFloor,
        verified: &VerifiedListing,
        _now: u64,
        fp: Failpoint<'_>,
    ) -> Result<ControlResponseV1, ListingError> {
        let first = verified.listing_digest;
        if fp("projection") {
            return Err(ListingError::Failpoint("projection"));
        }
        tx.upsert_listing_floor(next_floor)?;
        tx.invalidate_projection_generation()?;
        if fp("commit") {
            return Err(ListingError::Failpoint("commit"));
        }
        tx.commit()?;
        Ok(refuse(ControlRefusal::ListingEquivocation {
            first_digest: first,
            second_digest: next_floor.shown_digest.unwrap_or(first),
        }))
    }

    /// Build, sign, and store one directory-inclusion record; advance the feed head;
    /// return the new monotonic sequence (`feed_coordinate`).
    fn append_inclusion(
        &self,
        tx: &mut RepoTransaction<'_>,
        community_id: &[u8; 32],
        listing_digest: [u8; 32],
        listed: bool,
        now: u64,
    ) -> Result<u64, ListingError> {
        let (previous_digest, previous_length) = tx.feed_head()?;
        let sequence = previous_length.saturating_add(1);
        let record_bytes = self.sign_inclusion(
            community_id,
            sequence,
            &previous_digest,
            &listing_digest,
            listed,
            now,
        );
        let inclusion_digest = digest_v1(DIRECTORY_INCLUSION_ENVELOPE_LABEL, &record_bytes);
        tx.insert_directory_inclusion(&inclusion_digest, community_id, now, &record_bytes)?;
        let advanced = tx.advance_feed_head(&inclusion_digest, now)?;
        debug_assert_eq!(advanced, sequence);
        Ok(advanced)
    }

    /// Deterministically encode and operator-sign one inclusion body. Layout is
    /// fixed (no CBOR): a length-free field sequence signed over a domain-separated
    /// preimage. (A canonical `DirectoryInclusionBodyV1` protocol record + the
    /// signed feed chain vectors land with WU-017A; this is the WU-015B-internal
    /// signed inclusion, stored opaquely.)
    fn sign_inclusion(
        &self,
        community_id: &[u8; 32],
        sequence: u64,
        previous_inclusion_digest: &[u8; 32],
        listing_digest: &[u8; 32],
        listed: bool,
        accepted_at: u64,
    ) -> Vec<u8> {
        let mut body = Vec::with_capacity(1 + 32 + 8 + 32 + 32 + 1 + 8);
        body.push(DIRECTORY_INCLUSION_VERSION);
        body.extend_from_slice(community_id);
        body.extend_from_slice(&sequence.to_be_bytes());
        body.extend_from_slice(previous_inclusion_digest);
        body.extend_from_slice(listing_digest);
        body.push(listed as u8);
        body.extend_from_slice(&accepted_at.to_be_bytes());

        let mut preimage = DIRECTORY_INCLUSION_SIGNING_DOMAIN.to_vec();
        preimage.extend_from_slice(&body);
        let signature = self.signer.sign(&preimage);

        let mut record = body;
        record.extend_from_slice(&signature);
        record
    }

    /// Sign a [`ListingReceiptV1`] and wrap it in the `SubmitListing` success.
    fn listing_response(
        &self,
        verified: &VerifiedListing,
        hosting: &HostingState,
        feed_coordinate: u64,
        now: u64,
        idempotency_key: &[u8; IDEMPOTENCY_KEY_BYTES],
    ) -> Result<ControlResponseV1, ListingError> {
        let receipt = self.sign_receipt(ListingReceiptBodyV1 {
            anchor_id: self.context.anchor_id,
            operator_key_id: self.context.operator_key_id,
            descriptor_epoch: self.context.descriptor_epoch,
            descriptor_digest: self.context.descriptor_digest,
            listing_digest: verified.listing_digest,
            full_site_root: hosting.full_site_root,
            accepted_listing_epoch: verified.listing.listing_epoch,
            accepted_listing_revision: verified.listing.listing_revision,
            feed_coordinate,
            accepted_at: now,
            expires_at: verified.listing.expiry_unix_seconds,
            request_idempotency_key: *idempotency_key,
        })?;
        Ok(ControlResponseV1 {
            kind: ControlOperationKind::SubmitListing,
            outcome: ControlOutcome::Success(ControlSuccess::SubmitListing(Box::new(receipt))),
        })
    }

    fn sign_receipt(&self, body: ListingReceiptBodyV1) -> Result<ListingReceiptV1, ListingError> {
        let mut envelope = OperatorSignedEnvelopeV1 {
            body,
            operator_signature: [0u8; 64],
        };
        let preimage = envelope.signing_preimage()?;
        envelope.operator_signature = self.signer.sign(&preimage);
        Ok(envelope)
    }
}

fn refuse(refusal: ControlRefusal) -> ControlResponseV1 {
    ControlResponseV1 {
        kind: ControlOperationKind::SubmitListing,
        outcome: ControlOutcome::Refused(refusal),
    }
}

/// Map a `resolve_listing` authority error onto its closed control refusal.
fn map_authority_error(
    error: &riot_anchor_protocol::authority::AuthorityError,
    listing: &CommunityListingV1,
    now: u64,
) -> ControlRefusal {
    use riot_anchor_protocol::authority::AuthorityError;
    match error {
        AuthorityError::ExpiredListing => ControlRefusal::ListingExpired {
            expires_at: listing.expiry_unix_seconds,
            observed_at: now,
        },
        // Every other resolve error is an authority failure (root mismatch, illegal
        // epoch advance, malformed grant) — one closed reason, no disclosure.
        _ => ControlRefusal::InvalidListingAuthority,
    }
}
