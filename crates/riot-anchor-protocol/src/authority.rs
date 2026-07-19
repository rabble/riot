//! WU-003B authority layer: public-site-ticket admission and listing resolution.
//!
//! Two pure functions sit on top of the canonical records in [`crate::records`]:
//!
//! * [`admit_public_site_ticket`] — **SECURITY-CRITICAL**. Applies the exact
//!   fail-closed order from the design (`bounded canonical decode → root
//!   signature verify BEFORE any dial → min_sync_version == 2 → transport floor
//!   → expiry (inclusive) → epoch rollback → manifest match`). The root signature
//!   is checked before any transport or coordinate consideration; nothing here
//!   dials the network.
//! * [`resolve_listing`] — the deterministic listing state machine
//!   (epoch/seal/revision/equivocation/expiry rules). Pure; no manifest needed.
//!
//! ## Interpretation notes (design gaps decided here — flagged for review)
//!
//! * **`admit` carries a 5th `ticket_floor` argument.** The plan's stated 4-arg
//!   signature (`floor: &TransportFloor`) cannot express the epoch-rollback step,
//!   which needs the highest transport epoch already seen for the root. Since
//!   `TransportFloor` is the closed wire enum, epoch state is threaded through a
//!   separate durable [`TicketFloor`]. This is a deliberate deviation.
//! * **Manifest match** re-derives coordinates from the [`ValidatedManifest`]:
//!   `O` = `manifest.root`, `C`/`W` = the `Comments`/`OpenWire` member
//!   namespaces, `manifest_version` = `manifest.version`, and `manifest_digest`
//!   = `digest_v1` over the canonical manifest encoding. The design does not fix
//!   a site-manifest digest preimage, so [`SITE_MANIFEST_DIGEST_LABEL`] is
//!   INVENTED here and flagged.
//! * **`resolve_listing` reads the listing payload from
//!   `signed_listing_entry_bytes`.** This dependency-neutral crate cannot parse a
//!   `willow25` entry, and the entry's signature is validated at admission time
//!   (hence "Admitted"), so this function consumes the already-admitted canonical
//!   [`CommunityListingV1`] carried in those bytes. In the full system the caller
//!   (which owns `willow25`) extracts and verifies the entry, then passes its
//!   payload here.

use ed25519_dalek::{Signature, VerifyingKey};

use riot_core::site::{RequireTransport, SiteRole, ValidatedManifest};

use crate::codec::{CanonicalRecord, CodecError};
use crate::digest::digest_v1;
use crate::records::{
    decode_delegate_grant, decode_listing_payload, AdmittedListingEnvelopeV1, CommunityListingV1,
    ListingDelegateGrantV1, PublicSiteTicketV2Core, RootSignedTicketCoreEnvelopeV2, TransportFloor,
    MAX_TICKET_CORE_BYTES,
};

/// The required `min_sync_version` for a v2 ticket. Anything else is a downgrade.
const REQUIRED_MIN_SYNC_VERSION: u64 = 2;
/// Ticket expiry is capped at 90 days from issuance.
const MAX_TICKET_LIFETIME_SECONDS: u64 = 90 * 24 * 60 * 60;

/// `digest_v1` label for a site-manifest digest. **INVENTED** — the design binds
/// `manifest_digest` into the ticket/listing but does not specify the manifest's
/// own digest preimage. This is the proposed constant and is flagged for review.
pub const SITE_MANIFEST_DIGEST_LABEL: &[u8] = b"riot/site-manifest-digest/v1";

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// The closed refusal reason for `invalid_ticket` (design: `signature | root |
/// structure`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TicketReason {
    /// The root signature did not verify.
    Signature,
    /// `root_id` is not a valid Ed25519 verifying key.
    Root,
    /// Structural/canonicality fault (oversize core, malformed fields).
    Structure,
}

/// The closed authority-layer refusal vocabulary. Exhaustive by design.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorityError {
    /// Ticket admission refusal with its precise reason.
    InvalidTicket(TicketReason),
    /// `min_sync_version != 2` (v1/v2 downgrade).
    UnsupportedVersion,
    /// A non-`require_none` transport was requested (e.g. `require_arti`), which
    /// the MVP cannot satisfy.
    UnsupportedTransport,
    /// The ticket's transport pair disagrees with the manifest's requirement.
    ManifestTransportMismatch,
    /// The ticket is expired (`now >= expiry`).
    ExpiredTicket,
    /// The ticket's transport epoch is older than the highest already seen.
    EpochRollback,
    /// Ticket/manifest coordinate disagreement (root/O/C/W/digest/version).
    ManifestMismatch,
    /// The listing is expired (`now >= expiry`).
    ExpiredListing,
    /// A delegated listing's grant is missing/expired or names a different
    /// root/epoch than the listing.
    InvalidDelegateGrant,
    /// A listing tried to advance the epoch illegally (delegate establishing, or
    /// a jump greater than one).
    InvalidEpochAdvance,
    /// The candidate listing's root does not match the durable floor's root.
    RootMismatch,
    /// A carried canonical record failed to decode.
    MalformedRecord(CodecError),
}

impl core::fmt::Display for AuthorityError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for AuthorityError {}

// ---------------------------------------------------------------------------
// admit_public_site_ticket
// ---------------------------------------------------------------------------

/// The durable per-root transport-epoch floor, supplied by the caller so
/// admission can reject rolled-back tickets. Separate from the wire
/// [`TransportFloor`] because the plan's closed enum cannot carry epoch state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TicketFloor {
    /// The root this floor is for.
    pub root_id: [u8; 32],
    /// The highest transport epoch already admitted for this root, if any.
    pub highest_transport_epoch: Option<u32>,
}

/// A ticket that passed every admission gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedTicket {
    /// The verified ticket core.
    pub core: PublicSiteTicketV2Core,
    /// `root_signed_ticket_core_digest` over the full signed-core envelope.
    pub root_signed_ticket_core_digest: [u8; 32],
}

/// Verify a raw Ed25519 signature over `message` under `pubkey`. Returns
/// `Err(Root)` if the key is not a valid point, `Err(Signature)` if the
/// signature does not verify.
fn verify_ed25519(
    pubkey: &[u8; 32],
    message: &[u8],
    signature: &[u8; 64],
) -> Result<(), TicketReason> {
    let verifying_key = VerifyingKey::from_bytes(pubkey).map_err(|_| TicketReason::Root)?;
    let signature = Signature::from_bytes(signature);
    verifying_key
        .verify_strict(message, &signature)
        .map_err(|_| TicketReason::Signature)
}

/// Coordinates re-derived from a [`ValidatedManifest`] for ticket matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManifestCoordinates {
    /// `O` namespace id (== `manifest.root`).
    pub o_namespace_id: [u8; 32],
    /// `C` namespace id (the `Comments` member).
    pub c_namespace_id: [u8; 32],
    /// `W` namespace id (the `OpenWire` member).
    pub w_namespace_id: [u8; 32],
    /// `digest_v1` over the canonical manifest encoding.
    pub manifest_digest: [u8; 32],
    /// Manifest version.
    pub manifest_version: u64,
    /// Manifest transport requirement.
    pub required_transport: TransportFloor,
}

/// Re-derive the ticket-matchable coordinates from a validated manifest.
///
/// Returns [`AuthorityError::ManifestMismatch`] when the manifest does not carry
/// exactly one `Comments` and one `OpenWire` member (the `C`/`W` namespaces
/// cannot otherwise be established), or its transport requirement is unknown.
pub fn manifest_coordinates(
    manifest: &ValidatedManifest,
) -> Result<ManifestCoordinates, AuthorityError> {
    let member_ns = |role: SiteRole| -> Option<[u8; 32]> {
        let mut found: Option<[u8; 32]> = None;
        for member in &manifest.manifest.members {
            if core::mem::discriminant(&member.role) == core::mem::discriminant(&role) {
                if found.is_some() {
                    // Ambiguous: more than one member of this role.
                    return None;
                }
                found = Some(member.ns);
            }
        }
        found
    };

    let c_namespace_id = member_ns(SiteRole::Comments).ok_or(AuthorityError::ManifestMismatch)?;
    let w_namespace_id = member_ns(SiteRole::OpenWire).ok_or(AuthorityError::ManifestMismatch)?;
    let required_transport = match manifest.manifest.transport_policy.require {
        RequireTransport::None => TransportFloor::RequireNone,
        RequireTransport::Arti => TransportFloor::RequireArti,
    };
    let manifest_digest = site_manifest_digest(manifest)?;

    Ok(ManifestCoordinates {
        o_namespace_id: manifest.manifest.root,
        c_namespace_id,
        w_namespace_id,
        manifest_digest,
        manifest_version: manifest.manifest.version,
        required_transport,
    })
}

/// `digest_v1(SITE_MANIFEST_DIGEST_LABEL, canonical_site_manifest)`. INVENTED
/// preimage (see [`SITE_MANIFEST_DIGEST_LABEL`]).
fn site_manifest_digest(manifest: &ValidatedManifest) -> Result<[u8; 32], AuthorityError> {
    let canonical = riot_core::site::encode_site_manifest(&manifest.manifest)
        .map_err(|_| AuthorityError::ManifestMismatch)?;
    Ok(digest_v1(SITE_MANIFEST_DIGEST_LABEL, &canonical))
}

/// Admit a root-signed public-site ticket. **SECURITY-CRITICAL.**
///
/// Fail-closed order (each step gates the next; the signature is verified before
/// any transport, expiry, or coordinate consideration, and nothing dials):
///
/// 1. re-encode the core and reject `> 768` bytes (`invalid_ticket:structure`);
/// 2. verify the 64-byte root signature over `SIGNING_DOMAIN || core` against
///    `root_id` (`invalid_ticket:root|signature`);
/// 3. `min_sync_version == 2` else `unsupported_version`;
/// 4. transport: ticket floor, manifest requirement, and client `floor` must all
///    be `require_none` else `unsupported_transport`; ticket lifetime `<= 90d`;
/// 5. expiry inclusive: `now >= expiry` ⇒ `expired_ticket`;
/// 6. epoch rollback: `transport_epoch < highest_seen` ⇒ `epoch_rollback`;
/// 7. if a manifest is present, root/O/C/W/digest/version/transport must match.
pub fn admit_public_site_ticket(
    envelope: &RootSignedTicketCoreEnvelopeV2,
    manifest: Option<&ValidatedManifest>,
    floor: &TransportFloor,
    ticket_floor: &TicketFloor,
    now: u64,
) -> Result<AdmittedTicket, AuthorityError> {
    let core = &envelope.core;

    // 1. Structural: bounded canonical core.
    let core_canonical = core
        .encode_canonical()
        .map_err(|_| AuthorityError::InvalidTicket(TicketReason::Structure))?;
    // Defense-in-depth. NOTE: `PublicSiteTicketV2Core` has only fixed-size fields
    // (five 32-byte ids, fixed ints, two closed transport tokens), so its canonical
    // encoding is always ~200-240 bytes and this bound is UNREACHABLE for the fixed
    // core — a phantom guard kept in case a variable-length field is ever added. The
    // 768-byte bound does real work at the `CommunityListingV1.ticket_core_bytes`
    // layer, not here. (See docs/research/2026-07-19-wu003b-security-findings.md.)
    if core_canonical.len() > MAX_TICKET_CORE_BYTES {
        return Err(AuthorityError::InvalidTicket(TicketReason::Structure));
    }

    // 2. Root signature BEFORE any dial / transport / coordinate check.
    let mut preimage = RootSignedTicketCoreEnvelopeV2::SIGNING_DOMAIN.to_vec();
    preimage.extend_from_slice(&core_canonical);
    verify_ed25519(&core.root_id, &preimage, &envelope.root_signature)
        .map_err(AuthorityError::InvalidTicket)?;

    // 3. Version downgrade.
    if core.min_sync_version != REQUIRED_MIN_SYNC_VERSION {
        return Err(AuthorityError::UnsupportedVersion);
    }

    // 4. Transport floor. MVP admits only require_none everywhere; ticket floor
    //    must be >= manifest requirement (both require_none). Reject require_arti.
    if core.transport_floor != TransportFloor::RequireNone
        || core.manifest_required_transport != TransportFloor::RequireNone
        || *floor != TransportFloor::RequireNone
        || core.transport_floor < core.manifest_required_transport
    {
        return Err(AuthorityError::UnsupportedTransport);
    }
    // Ticket lifetime cap (90 days from issuance).
    if core.expiry_unix_seconds
        > core
            .issued_unix_seconds
            .saturating_add(MAX_TICKET_LIFETIME_SECONDS)
    {
        return Err(AuthorityError::InvalidTicket(TicketReason::Structure));
    }

    // 5. Expiry (inclusive).
    if now >= core.expiry_unix_seconds {
        return Err(AuthorityError::ExpiredTicket);
    }

    // 6. Epoch rollback.
    if let Some(highest) = ticket_floor.highest_transport_epoch {
        if core.transport_epoch < highest {
            return Err(AuthorityError::EpochRollback);
        }
    }

    // 7. Manifest coordinate + transport match.
    if let Some(manifest) = manifest {
        let coords = manifest_coordinates(manifest)?;
        // Transport pair must agree with the manifest AND stay require_none.
        if core.manifest_required_transport != coords.required_transport
            || coords.required_transport != TransportFloor::RequireNone
        {
            return Err(AuthorityError::ManifestTransportMismatch);
        }
        if core.root_id != manifest.manifest.root
            || core.o_namespace_id != coords.o_namespace_id
            || core.c_namespace_id != coords.c_namespace_id
            || core.w_namespace_id != coords.w_namespace_id
            || core.manifest_digest != coords.manifest_digest
            || core.manifest_version != coords.manifest_version
        {
            return Err(AuthorityError::ManifestMismatch);
        }
    }

    let digest = envelope
        .root_signed_ticket_core_digest()
        .map_err(|_| AuthorityError::InvalidTicket(TicketReason::Structure))?;
    Ok(AdmittedTicket {
        core: core.clone(),
        root_signed_ticket_core_digest: digest,
    })
}

// ---------------------------------------------------------------------------
// resolve_listing
// ---------------------------------------------------------------------------

/// The authority class of a candidate listing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorityClass {
    /// Root-owned zero-delegation listing (no grant present).
    RootOwned,
    /// Delegated listing (carries a [`ListingDelegateGrantV1`]).
    Delegated,
}

/// The durable, per-root listing floor. Persisted so restart/eviction cannot roll
/// the listing backward.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListingFloor {
    /// The root this floor tracks.
    pub root_id: [u8; 32],
    /// Highest root-established epoch.
    pub epoch: u32,
    /// Whether the current epoch is sealed by a root-owned listing.
    pub sealed: bool,
    /// Highest admitted revision in the current epoch (for the shown class).
    pub highest_revision: u32,
    /// Digest of the currently-shown listing, or `None` when nothing is shown /
    /// the coordinates are equivocated.
    pub shown_digest: Option<[u8; 32]>,
    /// Authority class of the currently-shown listing.
    pub shown_class: Option<AuthorityClass>,
    /// Whether the current coordinates are equivocated (shown: neither).
    pub equivocated: bool,
}

impl ListingFloor {
    /// A fresh floor for a root with nothing admitted yet (epoch 0, unsealed).
    pub fn new(root_id: [u8; 32]) -> Self {
        ListingFloor {
            root_id,
            epoch: 0,
            sealed: false,
            highest_revision: 0,
            shown_digest: None,
            shown_class: None,
            equivocated: false,
        }
    }
}

/// The outcome of resolving one candidate against the durable floor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListingOutcome {
    /// The candidate is now the shown listing.
    Shown,
    /// The candidate is a byte-identical repeat of the shown listing.
    Deduplicated,
    /// The candidate is valid but does not beat the shown listing.
    Superseded,
    /// The candidate collides with the shown listing at identical coordinates but
    /// a different digest — neither is shown.
    Equivocation,
    /// The candidate wins but is an explicit unlisting tombstone (`listed=false`);
    /// display stops.
    Unlisted,
}

/// The result of [`resolve_listing`]: the outcome plus the updated durable floor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListingTransition {
    /// What happened to display state.
    pub outcome: ListingOutcome,
    /// The floor to persist after this candidate.
    pub floor: ListingFloor,
}

/// Resolve a candidate admitted listing against the durable per-root floor.
///
/// Implements the design's state machine: higher valid epoch wins; only a
/// root-owned zero-delegation listing may establish the next epoch (advancing by
/// exactly one) and it seals that epoch; within an epoch a root-owned listing
/// beats every delegated listing regardless of revision; higher revision wins
/// within a class; identical coordinates + digest dedupe; identical coordinates +
/// different digest equivocate (show neither); a higher-revision root-owned
/// listing clears equivocation and cannot be pinned by a delegate at `u32::MAX`;
/// expiry is inclusive; and the floor never rolls backward.
pub fn resolve_listing(
    durable: &ListingFloor,
    candidate: &AdmittedListingEnvelopeV1,
    now: u64,
) -> Result<ListingTransition, AuthorityError> {
    let listing = decode_listing_payload(&candidate.signed_listing_entry_bytes)
        .map_err(AuthorityError::MalformedRecord)?;
    let digest = candidate
        .listing_digest()
        .map_err(AuthorityError::MalformedRecord)?;

    if listing.root_id != durable.root_id {
        return Err(AuthorityError::RootMismatch);
    }

    // Expiry (inclusive).
    if now >= listing.expiry_unix_seconds {
        return Err(AuthorityError::ExpiredListing);
    }

    // Authority class + grant validation.
    let class = match &candidate.delegate_grant_bytes {
        None => AuthorityClass::RootOwned,
        Some(grant_bytes) => {
            let grant =
                decode_delegate_grant(grant_bytes).map_err(AuthorityError::MalformedRecord)?;
            validate_grant(&grant, &listing, now)?;
            AuthorityClass::Delegated
        }
    };

    let e = listing.listing_epoch;

    // Epoch relationship.
    if e < durable.epoch {
        // Stale epoch: never rolls the floor backward.
        return Ok(superseded(durable));
    }
    if e > durable.epoch {
        // Only a root-owned listing may establish the next epoch, by exactly one.
        if class == AuthorityClass::RootOwned && e == durable.epoch + 1 {
            return Ok(establish_epoch(durable, e, &listing, digest));
        }
        return Err(AuthorityError::InvalidEpochAdvance);
    }

    // e == durable.epoch: within-epoch resolution.
    within_epoch(durable, class, &listing, digest)
}

/// Validate a delegate grant against the listing it accompanies.
fn validate_grant(
    grant: &ListingDelegateGrantV1,
    listing: &CommunityListingV1,
    now: u64,
) -> Result<(), AuthorityError> {
    if grant.root_id != listing.root_id
        || grant.listing_epoch != listing.listing_epoch
        || now >= grant.expiry_unix_seconds
    {
        return Err(AuthorityError::InvalidDelegateGrant);
    }
    Ok(())
}

fn superseded(durable: &ListingFloor) -> ListingTransition {
    ListingTransition {
        outcome: ListingOutcome::Superseded,
        floor: durable.clone(),
    }
}

/// Build the "this candidate is now shown" floor + outcome.
fn show(
    root_id: [u8; 32],
    epoch: u32,
    class: AuthorityClass,
    listing: &CommunityListingV1,
    digest: [u8; 32],
) -> ListingTransition {
    let sealed = class == AuthorityClass::RootOwned;
    let outcome = if listing.listed {
        ListingOutcome::Shown
    } else {
        ListingOutcome::Unlisted
    };
    ListingTransition {
        outcome,
        floor: ListingFloor {
            root_id,
            epoch,
            sealed,
            highest_revision: listing.listing_revision,
            shown_digest: Some(digest),
            shown_class: Some(class),
            equivocated: false,
        },
    }
}

fn establish_epoch(
    durable: &ListingFloor,
    epoch: u32,
    listing: &CommunityListingV1,
    digest: [u8; 32],
) -> ListingTransition {
    show(
        durable.root_id,
        epoch,
        AuthorityClass::RootOwned,
        listing,
        digest,
    )
}

fn within_epoch(
    durable: &ListingFloor,
    class: AuthorityClass,
    listing: &CommunityListingV1,
    digest: [u8; 32],
) -> Result<ListingTransition, AuthorityError> {
    let revision = listing.listing_revision;

    // Root-owned unconditionally beats delegated and (re)seals the epoch.
    if class == AuthorityClass::RootOwned {
        match durable.shown_class {
            // Nothing shown yet, or a delegated listing shown, or an equivocation
            // among delegated listings: root-owned wins outright.
            None | Some(AuthorityClass::Delegated) => {
                return Ok(show(durable.root_id, durable.epoch, class, listing, digest));
            }
            Some(AuthorityClass::RootOwned) => {
                // Compete on revision among root-owned listings.
                if revision > durable.highest_revision {
                    // Higher-revision root-owned wins and clears any equivocation.
                    return Ok(show(durable.root_id, durable.epoch, class, listing, digest));
                }
                if revision == durable.highest_revision {
                    return Ok(resolve_equal_revision(durable, class, listing, digest));
                }
                return Ok(superseded(durable));
            }
        }
    }

    // Delegated candidate.
    // A sealed epoch rejects all later delegated changes.
    if durable.sealed {
        return Ok(superseded(durable));
    }
    match durable.shown_class {
        None => Ok(show(durable.root_id, durable.epoch, class, listing, digest)),
        Some(AuthorityClass::RootOwned) => {
            // Unreachable in practice (root-owned would have sealed), but stay
            // fail-safe: a delegated listing never beats a shown root-owned one.
            Ok(superseded(durable))
        }
        Some(AuthorityClass::Delegated) => {
            if revision > durable.highest_revision {
                Ok(show(durable.root_id, durable.epoch, class, listing, digest))
            } else if revision == durable.highest_revision {
                Ok(resolve_equal_revision(durable, class, listing, digest))
            } else {
                Ok(superseded(durable))
            }
        }
    }
}

/// Resolve a candidate whose `(epoch, class, revision)` equals the shown listing:
/// dedupe on identical digest, else equivocate (show neither).
fn resolve_equal_revision(
    durable: &ListingFloor,
    class: AuthorityClass,
    _listing: &CommunityListingV1,
    digest: [u8; 32],
) -> ListingTransition {
    if durable.shown_digest == Some(digest) {
        return ListingTransition {
            outcome: ListingOutcome::Deduplicated,
            floor: durable.clone(),
        };
    }
    ListingTransition {
        outcome: ListingOutcome::Equivocation,
        floor: ListingFloor {
            equivocated: true,
            shown_digest: None,
            shown_class: Some(class),
            ..durable.clone()
        },
    }
}

/// Verify a root-signed [`ListingDelegateGrantV1`] against its 64-byte signature,
/// using the grant's `root_id` as the verifying key. Provided for callers that
/// carry the grant signature separately (the admitted envelope does not); not
/// used by [`resolve_listing`], whose input is already admitted.
pub fn verify_listing_delegate_grant(
    grant: &ListingDelegateGrantV1,
    signature: &[u8; 64],
) -> Result<(), AuthorityError> {
    let preimage = grant
        .signing_preimage()
        .map_err(AuthorityError::MalformedRecord)?;
    verify_ed25519(&grant.root_id, &preimage, signature)
        .map_err(|_| AuthorityError::InvalidDelegateGrant)
}
