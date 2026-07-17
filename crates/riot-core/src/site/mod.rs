//! Composite-site Unit 2 — the owner-signed site manifest.
//!
//! `manifest` owns the pure schema + canonical CBOR codec. Signer validation
//! (independent of admission), member classification, and the durable version
//! floor arrive in sibling modules.

pub mod manifest;

pub use manifest::{
    decode_site_manifest, encode_site_manifest, RequireTransport, SiteDisplay, SiteLayout,
    SiteManifestError, SiteManifestV1, SiteMemberV1, SiteRole, SiteRule, SiteTransport,
    TransportPolicyV1, MAX_MODERATION_PATH_COMPONENTS, MAX_SITE_MANIFEST_BYTES, MAX_SITE_MEMBERS,
    MAX_TRANSPORT_ALLOW, SITE_MANIFEST_SCHEMA,
};
