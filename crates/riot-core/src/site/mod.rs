//! Composite-site Unit 2 — the owner-signed site manifest.
//!
//! `manifest` owns the pure schema + canonical CBOR codec. Signer validation
//! (independent of admission), member classification, and the durable version
//! floor arrive in sibling modules.

pub mod follow;
pub mod manifest;
pub mod moderation;
pub mod moderation_entry;
pub mod resolve;
pub mod ticket;
pub mod validate;
pub mod version_floor;

pub use follow::{admit_followed_site_frame, is_followed_site_family, FollowedSiteAdmitError};

pub use moderation_entry::{
    create_signed_moderation_record, ModerationSignError, SignedModerationRecord,
};

pub use resolve::{
    item_treatment, resolve_degradation, resolve_soft_link, resolve_trust_tier,
    CompositeDegradation, DegradationInputs, SoftLink, TrustTier, WriteStatus, WriterCapState,
};

pub use manifest::{
    decode_site_manifest, encode_site_manifest, RequireTransport, SiteDisplay, SiteLayout,
    SiteManifestError, SiteManifestV1, SiteMemberV1, SiteRole, SiteRule, SiteTransport,
    TransportPolicyV1, MAX_MODERATION_PATH_COMPONENTS, MAX_SITE_MANIFEST_BYTES, MAX_SITE_MEMBERS,
    MAX_TRANSPORT_ALLOW, SITE_MANIFEST_SCHEMA,
};
pub use moderation::{
    compute_mod_set_digest, decode_moderation_record, encode_moderation_record, evaluate_freshness,
    read_moderation_record, Endorse, HeldModerationRecord, ModEpoch, ModerationFreshness,
    ModerationLoading, ModerationRecord, ModerationRecordError, Revoke, Tombstone,
    MAX_MODERATION_RECORD_BYTES, MODERATION_FRESHNESS_WINDOW_SECS, MODERATION_RECORD_SCHEMA,
};
pub use validate::{
    validate_site_manifest, ClassifiedMember, MemberClassification, SiteManifestValidationError,
    ValidatedManifest,
};
pub use version_floor::{
    admit_manifest_version, VersionFloorError, VersionFloorOutcome, VersionFloorStore,
};
