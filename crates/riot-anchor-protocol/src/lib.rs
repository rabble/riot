//! Canonical wire layer for the Riot public community anchor network.
//!
//! `riot-anchor-protocol` is the dependency-neutral crate that owns the shared
//! canonical frames, records, state enums, digests, and codecs of the anchor
//! network. It is consumed by the client internet runtime (`riot-client-net`),
//! the anchor daemon (`riot-anchor`), and — through conformance vectors — by the
//! native shells. To keep the verify/project core reusable everywhere (including
//! `wasm32`), this crate never depends on SQLite, HTTP, iroh, Tokio, a renderer,
//! FFI, or any platform adapter; it depends on [`riot_core`] with default
//! features disabled. `tests/dependency_boundary.rs` fails the build if that
//! contract is ever broken.
//!
//! Later work units add the codec, digest, authority, control, sync, directory,
//! and peer modules; each declares its own `pub mod` here in the same commit
//! that creates the module, so the crate stays buildable at every commit.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod authority;
pub mod codec;
pub mod control;
pub mod digest;
pub mod records;
pub mod sync2;

pub use codec::{decode_canonical, CanonicalRecord, CodecError};
pub use digest::digest_v1;

pub use authority::{
    admit_public_site_ticket, manifest_coordinates, resolve_listing, verify_pulled_listing,
    AdmittedTicket, AuthorityClass, AuthorityError, ListingFloor, ListingOutcome,
    ListingTransition, ManifestCoordinates, TicketFloor, TicketReason, VerifiedListingRow,
};
pub use records::{
    terminal_capability_digest, AdmittedListingEnvelopeV1, CommunityListingV1,
    ListingDelegateGrantV1, PublicSiteTicketV2Core, RootSignedTicketCoreEnvelopeV2, TransportFloor,
    COMMUNITY_LISTING_SCHEMA, MAX_DELEGATE_GRANT_BYTES, MAX_LISTING_ENVELOPE_BYTES,
    MAX_TICKET_CORE_BYTES,
};

pub use records::{
    AnchorBootstrapV1, AnchorDescriptorBodyV1, AnchorLimitEntry, AnchorLimitId,
    AnchorLimitProfileV1, AnchorSignedBody, BootstrapDescriptorV1, ControlOperationKind,
    DescriptorEnvelopeV1, DescriptorFloor, EnabledRole, HostingReceiptBodyV1, HostingReceiptV1,
    HostingStatus, LimitValue, ListingReceiptBodyV1, ListingReceiptV1, NamespaceResult,
    OperatorSignedEnvelopeV1, OperatorVerificationKeyV1, ReplicaPrepareChallengeV1,
    ReplicaSourceAttestationBodyV1, ReplicaSourceAttestationV1, WorkChallengeBodyV1,
    WorkChallengeV1, WorkStampError, WorkStampV1, ALL_LIMIT_IDS,
};

pub use control::{
    verify_descriptor_chain, CheckpointReason, ControlOperation, ControlOutcome, ControlRefusal,
    ControlRequestV1, ControlResponseV1, ControlSuccess, CursorKind, CursorReason, DescriptorError,
    EffectiveOperationLimits, FeedPullSuccessV1, GetOperationState, GetOperationSuccessV1,
    PeerAuthStage, PeerContextReason, PeerSide, PrepareKind, PrepareSuccessV1, RefusalSubject,
    RetryScope, SnapshotCursorBodyV1, SnapshotCursorV1, SnapshotPullSuccessV1, StorageClass,
    TerminalOperationOutcome, TransportMode,
};
