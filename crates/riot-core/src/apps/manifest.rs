//! App manifest: the plain-language description shown to a space organizer
//! before trusting an app, plus the fields needed to identify and locate
//! its bundle. Canonical encoding mirrors `model/mod.rs::encode_alert`'s
//! manual, strictly-ordered minicbor style: definite lengths only,
//! ascending integer map keys, no duplicate or unknown keys, no trailing
//! bytes, and decoding re-validates the same rules encoding enforces.

use minicbor::data::Type;
use minicbor::{Decoder, Encoder};
use sha2::{Digest, Sha256};

use crate::willow::identity::{AuthorIdentity, NamespaceKind};

use super::AppsError;

pub const MAX_APP_NAME_BYTES: usize = 80;
pub const MAX_APP_DESCRIPTION_BYTES: usize = 500;
pub const MAX_APP_VERSION_BYTES: usize = 32;
pub const MAX_APP_ENTRY_POINT_BYTES: usize = 256;
pub const MAX_APP_PERMISSIONS: usize = 8;
pub const MAX_APP_PERMISSION_BYTES: usize = 64;
pub const MAX_MANIFEST_BYTES: usize = 4_096;

const APP_ID_DOMAIN: &[u8] = b"riot/app-id/v1";
pub type AppId = [u8; 32];

/// The number of top-level CBOR map entries a canonical manifest always has.
const FIELD_COUNT: u64 = 9;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppManifest {
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: AuthorIdentity,
    pub permissions: Vec<String>,
    pub entry_point: String,
}

fn validate(manifest: &AppManifest) -> Result<(), AppsError> {
    check_text(&manifest.name, MAX_APP_NAME_BYTES)?;
    check_text(&manifest.description, MAX_APP_DESCRIPTION_BYTES)?;
    check_text(&manifest.version, MAX_APP_VERSION_BYTES)?;
    check_text(&manifest.entry_point, MAX_APP_ENTRY_POINT_BYTES)?;
    if manifest.permissions.len() > MAX_APP_PERMISSIONS {
        return Err(AppsError::ManifestFieldInvalid);
    }
    for permission in &manifest.permissions {
        check_text(permission, MAX_APP_PERMISSION_BYTES)?;
    }
    Ok(())
}

fn check_text(value: &str, max: usize) -> Result<(), AppsError> {
    if value.is_empty() || value.len() > max {
        return Err(AppsError::ManifestFieldInvalid);
    }
    Ok(())
}

/// Validates and encodes the canonical byte representation.
pub fn encode_manifest(manifest: &AppManifest) -> Result<Vec<u8>, AppsError> {
    validate(manifest)?;
    Ok(encode_validated_manifest(manifest))
}

/// Encodes a manifest that has already passed [`validate`]. The validated
/// field maxima cap the canonical document far below `MAX_MANIFEST_BYTES`, and
/// `Vec<u8>` is an infallible minicbor writer.
fn encode_validated_manifest(manifest: &AppManifest) -> Vec<u8> {
    let mut buffer: Vec<u8> = Vec::new();
    let mut e = Encoder::new(&mut buffer);
    let _ = e.map(FIELD_COUNT);
    let _ = e.u8(0);
    let _ = e.str(&manifest.name);
    let _ = e.u8(1);
    let _ = e.str(&manifest.description);
    let _ = e.u8(2);
    let _ = e.str(&manifest.version);
    let _ = e.u8(3);
    let _ = e.bytes(&manifest.author.namespace_id);
    let _ = e.u8(4);
    let _ = e.bytes(&manifest.author.subspace_id);
    let _ = e.u8(5);
    let _ = e.u8(namespace_kind_to_u8(manifest.author.namespace_kind));
    let _ = e.u8(6);
    let _ = e.bytes(&manifest.author.signing_key_id);
    let _ = e.u8(7);
    let _ = e.array(manifest.permissions.len() as u64);
    for permission in &manifest.permissions {
        let _ = e.str(permission);
    }
    let _ = e.u8(8);
    let _ = e.str(&manifest.entry_point);
    buffer
}

/// Strict canonical decoder: rejects unknown/duplicate/misordered keys,
/// indefinite lengths, trailing bytes, and any non-canonical encoding.
pub fn decode_manifest(input: &[u8]) -> Result<AppManifest, AppsError> {
    if input.len() > MAX_MANIFEST_BYTES {
        return Err(AppsError::ManifestFieldInvalid);
    }

    let mut d = Decoder::new(input);
    let pairs = d
        .map()
        .map_err(|_| AppsError::ManifestFieldInvalid)?
        .ok_or(AppsError::ManifestFieldInvalid)?;
    if pairs != FIELD_COUNT {
        return Err(AppsError::ManifestFieldInvalid);
    }

    require_key(&mut d, 0)?;
    let name = decode_text(&mut d, MAX_APP_NAME_BYTES)?;
    require_key(&mut d, 1)?;
    let description = decode_text(&mut d, MAX_APP_DESCRIPTION_BYTES)?;
    require_key(&mut d, 2)?;
    let version = decode_text(&mut d, MAX_APP_VERSION_BYTES)?;
    require_key(&mut d, 3)?;
    let namespace_id = decode_id32(&mut d)?;
    require_key(&mut d, 4)?;
    let subspace_id = decode_id32(&mut d)?;
    require_key(&mut d, 5)?;
    let raw_kind = d.u8().map_err(|_| AppsError::ManifestFieldInvalid)?;
    let namespace_kind = namespace_kind_from_u8(raw_kind).ok_or(AppsError::ManifestFieldInvalid)?;
    require_key(&mut d, 6)?;
    let signing_key_id = decode_id32(&mut d)?;
    require_key(&mut d, 7)?;
    let len = d
        .array()
        .map_err(|_| AppsError::ManifestFieldInvalid)?
        .ok_or(AppsError::ManifestFieldInvalid)?;
    if len as usize > MAX_APP_PERMISSIONS {
        return Err(AppsError::ManifestFieldInvalid);
    }
    let mut permissions = Vec::with_capacity(len as usize);
    for _ in 0..len {
        permissions.push(decode_text(&mut d, MAX_APP_PERMISSION_BYTES)?);
    }
    require_key(&mut d, 8)?;
    let entry_point = decode_text(&mut d, MAX_APP_ENTRY_POINT_BYTES)?;

    if d.position() != input.len() {
        return Err(AppsError::ManifestFieldInvalid);
    }

    let manifest = AppManifest {
        name,
        description,
        version,
        author: AuthorIdentity {
            namespace_id,
            subspace_id,
            namespace_kind,
            signing_key_id,
        },
        permissions,
        entry_point,
    };

    // Canonicality proof: only the exact encoder output is acceptable.
    // The field decoders above enforce every invariant from `validate` before
    // allocating the reconstructed manifest, so re-encoding is infallible.
    let reencoded = encode_validated_manifest(&manifest);
    if reencoded != input {
        return Err(AppsError::ManifestFieldInvalid);
    }

    Ok(manifest)
}

fn require_key(d: &mut Decoder<'_>, expected: u64) -> Result<(), AppsError> {
    if d.u64().map_err(|_| AppsError::ManifestFieldInvalid)? == expected {
        Ok(())
    } else {
        Err(AppsError::ManifestFieldInvalid)
    }
}

fn namespace_kind_to_u8(kind: NamespaceKind) -> u8 {
    match kind {
        NamespaceKind::Communal => 0,
        NamespaceKind::Owned => 1,
    }
}

fn namespace_kind_from_u8(value: u8) -> Option<NamespaceKind> {
    match value {
        0 => Some(NamespaceKind::Communal),
        1 => Some(NamespaceKind::Owned),
        _ => None,
    }
}

fn decode_id32(d: &mut Decoder<'_>) -> Result<[u8; 32], AppsError> {
    let bytes = d.bytes().map_err(|_| AppsError::ManifestFieldInvalid)?;
    <[u8; 32]>::try_from(bytes).map_err(|_| AppsError::ManifestFieldInvalid)
}

fn decode_text(d: &mut Decoder<'_>, max: usize) -> Result<String, AppsError> {
    if d.datatype().map_err(|_| AppsError::ManifestFieldInvalid)? != Type::String {
        return Err(AppsError::ManifestFieldInvalid);
    }
    let text = d.str().map_err(|_| AppsError::ManifestFieldInvalid)?;
    if text.is_empty() || text.len() > max {
        return Err(AppsError::ManifestFieldInvalid);
    }
    Ok(text.to_string())
}

/// Content-derived app identity: `SHA256("riot/app-id/v1" ||
/// u32be(manifest_bytes.len()) || manifest_bytes || bundle_digest)`.
///
/// Deliberately bundle-sensitive: publishing a new version of the same app
/// (a new bundle, even under an unchanged manifest) yields a different
/// `AppId` by design, so a space's per-app trust decision never silently
/// carries forward onto code it never reviewed.
pub fn app_id_for(manifest: &AppManifest, bundle_digest: &[u8; 32]) -> Result<AppId, AppsError> {
    let manifest_bytes = encode_manifest(manifest)?;
    let mut hasher = Sha256::new();
    hasher.update(APP_ID_DOMAIN);
    hasher.update((manifest_bytes.len() as u32).to_be_bytes());
    hasher.update(&manifest_bytes);
    hasher.update(bundle_digest);
    Ok(hasher.finalize().into())
}
