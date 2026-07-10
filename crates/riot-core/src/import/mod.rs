//! Evidence bundle codec (`RiotEvidenceBundleV1`) and, in WU2, the
//! preview-first atomic import store.

pub mod bundle;

pub use bundle::{
    decode_bundle, encode_bundle, BundleDecodeOutcome, BundleDiagnostic, BundleEncodeError,
    BundleItemFrame, BundleRejection, DecodedBundle, DecodedItem, DiagnosticCode, ItemComponent,
    ItemStatus, RejectionCode, ValidItem, BUNDLE_CODEC_ID, BUNDLE_MAGIC, MAX_AUTH_BYTES_PER_BUNDLE,
    MAX_BUNDLE_BYTES, MAX_BUNDLE_ENTRIES, MAX_CAPABILITY_BYTES, MAX_ENTRY_BYTES,
    MAX_ITEM_PAYLOAD_BYTES,
};

// Digest vocabulary lives in `crate::willow::digest`; re-exported here for
// import-layer callers.
pub use crate::willow::{bundle_digest, entry_id, evidence_digest, object_digest};
