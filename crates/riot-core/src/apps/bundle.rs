//! App resource bundle: a fixed list of `(path, content_type, bytes)`
//! resources plus a primary entry point, deterministically CBOR-encoded in
//! the same manual style as `model/mod.rs::encode_alert`.
//!
//! This is a minimal, self-contained resource-pack format, not a
//! byte-for-byte WICG Web Bundle (`.wbn`) — the design doc names WICG Web
//! Bundle as the packaging inspiration, but nothing outside this crate's
//! own decoder ever parses these bytes: the native host unpacks a bundle
//! and serves its resources locally to an embedded webview. Full binary
//! spec compliance would buy nothing here, so we keep the encoding manual,
//! strictly ordered, and bounded like the rest of this crate's codecs
//! instead of pulling in a WICG-compliant framing implementation.
//!
//! Not to be confused with the unrelated `import::bundle` module, which
//! encodes evidence-import artifacts (`RiotEvidenceBundleV1`).

use minicbor::data::Type;
use minicbor::{Decoder, Encoder};
use sha2::{Digest, Sha256};

use super::AppsError;

const APP_BUNDLE_DIGEST_DOMAIN: &[u8] = b"riot/app-bundle/v1";

pub const MAX_BUNDLE_RESOURCES: usize = 32;
pub const MAX_RESOURCE_PATH_BYTES: usize = 256;
pub const MAX_RESOURCE_CONTENT_TYPE_BYTES: usize = 64;
pub const MAX_BUNDLE_TOTAL_BYTES: usize = 1_048_576;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppResource {
    pub path: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppBundle {
    pub entry_point: String,
    pub resources: Vec<AppResource>,
}

fn validate(bundle: &AppBundle) -> Result<(), AppsError> {
    if bundle.resources.is_empty() || bundle.resources.len() > MAX_BUNDLE_RESOURCES {
        return Err(AppsError::BundleFieldInvalid);
    }

    let mut total_bytes: usize = 0;
    let mut entry_point_found = false;
    for resource in &bundle.resources {
        if resource.path.is_empty() || resource.path.len() > MAX_RESOURCE_PATH_BYTES {
            return Err(AppsError::BundleFieldInvalid);
        }
        if resource.content_type.is_empty()
            || resource.content_type.len() > MAX_RESOURCE_CONTENT_TYPE_BYTES
        {
            return Err(AppsError::BundleFieldInvalid);
        }
        if resource.path == bundle.entry_point {
            entry_point_found = true;
        }
        total_bytes = total_bytes
            .checked_add(resource.bytes.len())
            .ok_or(AppsError::BundleTooLarge)?;
    }

    if !entry_point_found {
        return Err(AppsError::BundleFieldInvalid);
    }
    if total_bytes > MAX_BUNDLE_TOTAL_BYTES {
        return Err(AppsError::BundleTooLarge);
    }

    Ok(())
}

/// Domain-separated digest of a bundle's canonical encoded bytes — the
/// `bundle_digest` input to `manifest::app_id_for`, following the pattern
/// in `willow/digest.rs`.
pub fn app_bundle_digest(encoded_bundle: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(APP_BUNDLE_DIGEST_DOMAIN);
    hasher.update((encoded_bundle.len() as u32).to_be_bytes());
    hasher.update(encoded_bundle);
    hasher.finalize().into()
}

/// Validates and encodes the canonical byte representation.
pub fn encode_app_bundle(bundle: &AppBundle) -> Result<Vec<u8>, AppsError> {
    validate(bundle)?;

    let mut buffer: Vec<u8> = Vec::new();
    let mut e = Encoder::new(&mut buffer);
    let r: Result<_, minicbor::encode::Error<core::convert::Infallible>> = (|| {
        e.map(2)?;
        e.u8(0)?.str(&bundle.entry_point)?;
        e.u8(1)?.array(bundle.resources.len() as u64)?;
        for resource in &bundle.resources {
            e.map(3)?;
            e.u8(0)?.str(&resource.path)?;
            e.u8(1)?.str(&resource.content_type)?;
            e.u8(2)?.bytes(&resource.bytes)?;
        }
        Ok(())
    })();
    r.map_err(|_| AppsError::BundleFieldInvalid)?;

    if buffer.len() > MAX_BUNDLE_TOTAL_BYTES {
        return Err(AppsError::BundleTooLarge);
    }
    Ok(buffer)
}

/// Strict canonical decoder: rejects unknown/duplicate/misordered keys,
/// indefinite lengths, trailing bytes, and any non-canonical encoding.
/// Bounds (resource count, path/content-type lengths) are enforced before
/// any allocation sized from untrusted input.
pub fn decode_app_bundle(input: &[u8]) -> Result<AppBundle, AppsError> {
    if input.len() > MAX_BUNDLE_TOTAL_BYTES {
        return Err(AppsError::BundleTooLarge);
    }

    let mut d = Decoder::new(input);
    let pairs = d
        .map()
        .map_err(|_| AppsError::BundleFieldInvalid)?
        .ok_or(AppsError::BundleFieldInvalid)?;
    if pairs != 2 {
        return Err(AppsError::BundleFieldInvalid);
    }

    let key0 = d.u64().map_err(|_| AppsError::BundleFieldInvalid)?;
    if key0 != 0 {
        return Err(AppsError::BundleFieldInvalid);
    }
    let entry_point = decode_text(&mut d, MAX_RESOURCE_PATH_BYTES)?;

    let key1 = d.u64().map_err(|_| AppsError::BundleFieldInvalid)?;
    if key1 != 1 {
        return Err(AppsError::BundleFieldInvalid);
    }
    let resource_count = d
        .array()
        .map_err(|_| AppsError::BundleFieldInvalid)?
        .ok_or(AppsError::BundleFieldInvalid)?;
    if resource_count == 0 || resource_count as usize > MAX_BUNDLE_RESOURCES {
        return Err(AppsError::BundleFieldInvalid);
    }

    let mut resources = Vec::with_capacity(resource_count as usize);
    for _ in 0..resource_count {
        let resource_pairs = d
            .map()
            .map_err(|_| AppsError::BundleFieldInvalid)?
            .ok_or(AppsError::BundleFieldInvalid)?;
        if resource_pairs != 3 {
            return Err(AppsError::BundleFieldInvalid);
        }

        let rkey0 = d.u64().map_err(|_| AppsError::BundleFieldInvalid)?;
        if rkey0 != 0 {
            return Err(AppsError::BundleFieldInvalid);
        }
        let path = decode_text(&mut d, MAX_RESOURCE_PATH_BYTES)?;

        let rkey1 = d.u64().map_err(|_| AppsError::BundleFieldInvalid)?;
        if rkey1 != 1 {
            return Err(AppsError::BundleFieldInvalid);
        }
        let content_type = decode_text(&mut d, MAX_RESOURCE_CONTENT_TYPE_BYTES)?;

        let rkey2 = d.u64().map_err(|_| AppsError::BundleFieldInvalid)?;
        if rkey2 != 2 {
            return Err(AppsError::BundleFieldInvalid);
        }
        let bytes = decode_bytes(&mut d)?;

        resources.push(AppResource {
            path,
            content_type,
            bytes,
        });
    }

    if d.position() != input.len() {
        return Err(AppsError::BundleFieldInvalid);
    }

    let bundle = AppBundle {
        entry_point,
        resources,
    };

    validate(&bundle)?;

    // Canonicality proof: only the exact encoder output is acceptable.
    let reencoded = encode_app_bundle(&bundle)?;
    if reencoded != input {
        return Err(AppsError::BundleFieldInvalid);
    }

    Ok(bundle)
}

fn decode_text(d: &mut Decoder<'_>, max: usize) -> Result<String, AppsError> {
    if d.datatype().map_err(|_| AppsError::BundleFieldInvalid)? != Type::String {
        return Err(AppsError::BundleFieldInvalid);
    }
    let text = d.str().map_err(|_| AppsError::BundleFieldInvalid)?;
    if text.is_empty() || text.len() > max {
        return Err(AppsError::BundleFieldInvalid);
    }
    Ok(text.to_string())
}

fn decode_bytes(d: &mut Decoder<'_>) -> Result<Vec<u8>, AppsError> {
    if d.datatype().map_err(|_| AppsError::BundleFieldInvalid)? != Type::Bytes {
        return Err(AppsError::BundleFieldInvalid);
    }
    let bytes = d.bytes().map_err(|_| AppsError::BundleFieldInvalid)?;
    if bytes.len() > MAX_BUNDLE_TOTAL_BYTES {
        return Err(AppsError::BundleTooLarge);
    }
    Ok(bytes.to_vec())
}
