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

    let mut buffer: Vec<u8> = Vec::new();
    let mut e = Encoder::new(&mut buffer);
    let r: Result<_, minicbor::encode::Error<core::convert::Infallible>> = (|| {
        e.map(FIELD_COUNT)?;
        e.u8(0)?.str(&manifest.name)?;
        e.u8(1)?.str(&manifest.description)?;
        e.u8(2)?.str(&manifest.version)?;
        e.u8(3)?.bytes(&manifest.author.namespace_id)?;
        e.u8(4)?.bytes(&manifest.author.subspace_id)?;
        e.u8(5)?.u8(namespace_kind_to_u8(manifest.author.namespace_kind))?;
        e.u8(6)?.bytes(&manifest.author.signing_key_id)?;
        e.u8(7)?.array(manifest.permissions.len() as u64)?;
        for permission in &manifest.permissions {
            e.str(permission)?;
        }
        e.u8(8)?.str(&manifest.entry_point)?;
        Ok(())
    })();
    r.map_err(|_| AppsError::ManifestFieldInvalid)?;

    if buffer.len() > MAX_MANIFEST_BYTES {
        return Err(AppsError::ManifestFieldInvalid);
    }
    Ok(buffer)
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

    let mut name: Option<String> = None;
    let mut description: Option<String> = None;
    let mut version: Option<String> = None;
    let mut namespace_id: Option<[u8; 32]> = None;
    let mut subspace_id: Option<[u8; 32]> = None;
    let mut namespace_kind: Option<NamespaceKind> = None;
    let mut signing_key_id: Option<[u8; 32]> = None;
    let mut permissions: Option<Vec<String>> = None;
    let mut entry_point: Option<String> = None;

    let mut last_key: Option<u64> = None;
    for _ in 0..pairs {
        let key = d.u64().map_err(|_| AppsError::ManifestFieldInvalid)?;
        if let Some(previous) = last_key {
            if key <= previous {
                return Err(AppsError::ManifestFieldInvalid);
            }
        }
        last_key = Some(key);

        match key {
            0 => name = Some(decode_text(&mut d, MAX_APP_NAME_BYTES)?),
            1 => description = Some(decode_text(&mut d, MAX_APP_DESCRIPTION_BYTES)?),
            2 => version = Some(decode_text(&mut d, MAX_APP_VERSION_BYTES)?),
            3 => namespace_id = Some(decode_id32(&mut d)?),
            4 => subspace_id = Some(decode_id32(&mut d)?),
            5 => {
                let raw = d.u8().map_err(|_| AppsError::ManifestFieldInvalid)?;
                namespace_kind =
                    Some(namespace_kind_from_u8(raw).ok_or(AppsError::ManifestFieldInvalid)?);
            }
            6 => signing_key_id = Some(decode_id32(&mut d)?),
            7 => {
                let len = d
                    .array()
                    .map_err(|_| AppsError::ManifestFieldInvalid)?
                    .ok_or(AppsError::ManifestFieldInvalid)?;
                if len as usize > MAX_APP_PERMISSIONS {
                    return Err(AppsError::ManifestFieldInvalid);
                }
                let mut items = Vec::with_capacity(len as usize);
                for _ in 0..len {
                    items.push(decode_text(&mut d, MAX_APP_PERMISSION_BYTES)?);
                }
                permissions = Some(items);
            }
            8 => entry_point = Some(decode_text(&mut d, MAX_APP_ENTRY_POINT_BYTES)?),
            _ => return Err(AppsError::ManifestFieldInvalid),
        }
    }

    if d.position() != input.len() {
        return Err(AppsError::ManifestFieldInvalid);
    }

    let manifest = AppManifest {
        name: name.ok_or(AppsError::ManifestFieldInvalid)?,
        description: description.ok_or(AppsError::ManifestFieldInvalid)?,
        version: version.ok_or(AppsError::ManifestFieldInvalid)?,
        author: AuthorIdentity {
            namespace_id: namespace_id.ok_or(AppsError::ManifestFieldInvalid)?,
            subspace_id: subspace_id.ok_or(AppsError::ManifestFieldInvalid)?,
            namespace_kind: namespace_kind.ok_or(AppsError::ManifestFieldInvalid)?,
            signing_key_id: signing_key_id.ok_or(AppsError::ManifestFieldInvalid)?,
        },
        permissions: permissions.ok_or(AppsError::ManifestFieldInvalid)?,
        entry_point: entry_point.ok_or(AppsError::ManifestFieldInvalid)?,
    };

    validate(&manifest)?;

    // Canonicality proof: only the exact encoder output is acceptable.
    let reencoded = encode_manifest(&manifest)?;
    if reencoded != input {
        return Err(AppsError::ManifestFieldInvalid);
    }

    Ok(manifest)
}

fn namespace_kind_to_u8(kind: NamespaceKind) -> u8 {
    match kind {
        NamespaceKind::Communal => 0,
    }
}

fn namespace_kind_from_u8(value: u8) -> Option<NamespaceKind> {
    match value {
        0 => Some(NamespaceKind::Communal),
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
