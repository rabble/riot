//! Interim generator for the committed starter-catalog artifacts.
//!
//! Reads the frozen checklist app source under `fixtures/apps/checklist/`,
//! packs it into the two canonical CBOR artifacts embedded later via
//! `include_bytes!`, and self-checks the pair through the exact same
//! `verify_starter_catalog` path the runtime uses. Determinism matters: the
//! drift guard re-derives these bytes and compares, so resources are sorted
//! by path (byte order) before encoding.
//!
//! This lives as an example (not a library API) to unblock the catalog while
//! the `riot-app pack` CLI is built; `scripts/apps/repack-starter.sh`
//! switches to that CLI once it lands.

use std::path::{Path, PathBuf};

use riot_core::apps::bundle::{encode_app_bundle, AppBundle, AppResource};
use riot_core::apps::index::app_bundle_digest;
use riot_core::apps::manifest::{app_id_for, encode_manifest, AppManifest};
use riot_core::apps::starter::verify_starter_catalog;
use riot_core::willow::identity::{AuthorIdentity, NamespaceKind};

/// Fixed committed PUBLIC author identity for built-in apps. Placeholder in
/// the conference-fixture precedent — no private key exists for it; starter
/// integrity is content-addressed, not signature-verified.
const NAMESPACE_ID_HEX: &str = "27cd7747ceecf672b65a998f1606162fc1e39793dd61a442a0af65ba4f92951e";
const SUBSPACE_ID_HEX: &str = "99069a7b075d21e0dc7e4b7c7daf311f8e1d308001763d9d78ef60e9b9857157";

fn main() {
    if let Err(err) = run() {
        eprintln!("pack_checklist: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let source_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/apps/checklist")
        .canonicalize()
        .map_err(|e| format!("cannot locate fixtures/apps/checklist: {e}"))?;

    let manifest_meta = read_manifest_meta(&source_dir.join("riot-app.json"))?;

    let mut resources = build_resources(&source_dir)?;
    resources.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));

    if !resources
        .iter()
        .any(|r| r.path == manifest_meta.entry_point)
    {
        return Err(format!(
            "entry_point '{}' not present among packed resources",
            manifest_meta.entry_point
        ));
    }

    let bundle = AppBundle {
        entry_point: manifest_meta.entry_point.clone(),
        resources,
    };
    let bundle_bytes = encode_app_bundle(&bundle).map_err(|e| format!("encode bundle: {e}"))?;

    let manifest = AppManifest {
        name: manifest_meta.name.clone(),
        description: manifest_meta.description,
        version: manifest_meta.version,
        author: AuthorIdentity {
            namespace_id: decode_hex32(NAMESPACE_ID_HEX)?,
            subspace_id: decode_hex32(SUBSPACE_ID_HEX)?,
            namespace_kind: NamespaceKind::Communal,
            signing_key_id: decode_hex32(SUBSPACE_ID_HEX)?,
        },
        permissions: manifest_meta.permissions,
        entry_point: manifest_meta.entry_point,
    };
    let manifest_bytes = encode_manifest(&manifest).map_err(|e| format!("encode manifest: {e}"))?;

    // Self-check through the exact runtime verify path.
    let indexed = verify_starter_catalog(&[(&manifest_bytes, &bundle_bytes)]);
    if indexed.len() != 1 {
        return Err(format!(
            "verify_starter_catalog returned {} apps, expected exactly 1",
            indexed.len()
        ));
    }
    if indexed[0].manifest.name != manifest_meta.name {
        return Err(format!(
            "verified manifest name '{}' does not match source '{}'",
            indexed[0].manifest.name, manifest_meta.name
        ));
    }

    let out_dir = source_dir
        .parent()
        .ok_or("fixtures/apps has no parent")?
        .to_path_buf();
    std::fs::write(out_dir.join("checklist.manifest.cbor"), &manifest_bytes)
        .map_err(|e| format!("write manifest artifact: {e}"))?;
    std::fs::write(out_dir.join("checklist.bundle.cbor"), &bundle_bytes)
        .map_err(|e| format!("write bundle artifact: {e}"))?;

    let app_id = app_id_for(&manifest, &app_bundle_digest(&bundle_bytes))
        .map_err(|e| format!("derive app_id: {e}"))?;
    println!("app_id: {}", to_hex(&app_id));
    println!(
        "manifest: {} bytes, bundle: {} bytes",
        manifest_bytes.len(),
        bundle_bytes.len()
    );

    Ok(())
}

struct ManifestMeta {
    name: String,
    description: String,
    version: String,
    entry_point: String,
    permissions: Vec<String>,
}

fn read_manifest_meta(path: &Path) -> Result<ManifestMeta, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("read riot-app.json: {e}"))?;
    let value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("parse riot-app.json: {e}"))?;

    let name = required_str(&value, "name")?;
    let description = required_str(&value, "description")?;
    let version = required_str(&value, "version")?;
    let entry_point = required_str(&value, "entry_point")?;

    let permissions = value
        .get("permissions")
        .and_then(|p| p.as_array())
        .ok_or("riot-app.json: 'permissions' must be an array")?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_string)
                .ok_or_else(|| "riot-app.json: each permission must be a string".to_string())
        })
        .collect::<Result<Vec<String>, String>>()?;

    Ok(ManifestMeta {
        name,
        description,
        version,
        entry_point,
        permissions,
    })
}

fn required_str(value: &serde_json::Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| format!("riot-app.json: '{key}' must be a string"))
}

fn build_resources(source_dir: &Path) -> Result<Vec<AppResource>, String> {
    let mut resources = Vec::new();
    let entries =
        std::fs::read_dir(source_dir).map_err(|e| format!("read fixtures/apps/checklist: {e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read dir entry: {e}"))?;
        if !entry
            .file_type()
            .map_err(|e| format!("stat dir entry: {e}"))?
            .is_file()
        {
            continue;
        }
        let file_name = entry.file_name();
        let name = file_name
            .to_str()
            .ok_or("non-UTF-8 file name in fixtures/apps/checklist")?;
        if name == "riot-app.json" {
            continue;
        }
        let content_type = content_type_for(name)?;
        let bytes = std::fs::read(entry.path()).map_err(|e| format!("read {name}: {e}"))?;
        resources.push(AppResource {
            path: name.to_string(),
            content_type: content_type.to_string(),
            bytes,
        });
    }
    Ok(resources)
}

fn content_type_for(file_name: &str) -> Result<&'static str, String> {
    let ext = file_name.rsplit('.').next().unwrap_or("");
    match ext {
        "html" => Ok("text/html"),
        "js" => Ok("text/javascript"),
        "css" => Ok("text/css"),
        "svg" => Ok("image/svg+xml"),
        "png" => Ok("image/png"),
        _ => Err(format!(
            "unsupported resource file '{file_name}': no content-type mapping for extension '.{ext}'"
        )),
    }
}

fn decode_hex32(hex: &str) -> Result<[u8; 32], String> {
    if hex.len() != 64 {
        return Err(format!("hex constant must be 64 chars, got {}", hex.len()));
    }
    let mut out = [0u8; 32];
    let bytes = hex.as_bytes();
    for (i, slot) in out.iter_mut().enumerate() {
        let hi = hex_nibble(bytes[i * 2])?;
        let lo = hex_nibble(bytes[i * 2 + 1])?;
        *slot = (hi << 4) | lo;
    }
    Ok(out)
}

fn hex_nibble(b: u8) -> Result<u8, String> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(format!("invalid hex digit: {}", b as char)),
    }
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}
