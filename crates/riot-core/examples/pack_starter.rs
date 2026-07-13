//! Deterministically pack every built-in community miniapp.
//!
//! The frozen Checklist artifacts are a compatibility boundary: this tool
//! computes every output first and aborts before writing anything if packing
//! Checklist would change either committed byte string.

use std::path::{Path, PathBuf};

use riot_core::apps::bundle::{encode_app_bundle, AppBundle, AppResource};
use riot_core::apps::index::app_bundle_digest;
use riot_core::apps::manifest::{app_id_for, encode_manifest, AppManifest};
use riot_core::apps::starter::verify_starter_catalog;
use riot_core::willow::identity::{AuthorIdentity, NamespaceKind};

const NAMESPACE_ID_HEX: &str = "27cd7747ceecf672b65a998f1606162fc1e39793dd61a442a0af65ba4f92951e";
const SUBSPACE_ID_HEX: &str = "99069a7b075d21e0dc7e4b7c7daf311f8e1d308001763d9d78ef60e9b9857157";
const STARTERS: &[&str] = &[
    "checklist",
    "supply-board",
    "roll-call",
    "quick-poll",
    "chat",
    "dispatches",
    "wiki",
    "photo-wall",
];

fn main() {
    if let Err(error) = run() {
        eprintln!("pack_starter: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/apps")
        .canonicalize()
        .map_err(|error| format!("cannot locate fixtures/apps: {error}"))?;
    let author = AuthorIdentity {
        namespace_id: decode_hex32(NAMESPACE_ID_HEX)?,
        subspace_id: decode_hex32(SUBSPACE_ID_HEX)?,
        namespace_kind: NamespaceKind::Communal,
        signing_key_id: decode_hex32(SUBSPACE_ID_HEX)?,
    };

    let mut packed = Vec::with_capacity(STARTERS.len());
    for slug in STARTERS {
        packed.push(pack_app(&root, slug, &author)?);
    }

    let checklist = packed
        .first()
        .ok_or("starter list must contain Checklist first")?;
    assert_frozen(
        &root.join("checklist.manifest.cbor"),
        &checklist.manifest_bytes,
    )?;
    assert_frozen(&root.join("checklist.bundle.cbor"), &checklist.bundle_bytes)?;

    for app in &packed {
        std::fs::write(
            root.join(format!("{}.manifest.cbor", app.slug)),
            &app.manifest_bytes,
        )
        .map_err(|error| format!("write {} manifest: {error}", app.slug))?;
        std::fs::write(
            root.join(format!("{}.bundle.cbor", app.slug)),
            &app.bundle_bytes,
        )
        .map_err(|error| format!("write {} bundle: {error}", app.slug))?;
        println!("{}: {}", app.slug, to_hex(&app.app_id));
    }
    Ok(())
}

struct PackedApp {
    slug: String,
    manifest_bytes: Vec<u8>,
    bundle_bytes: Vec<u8>,
    app_id: [u8; 32],
}

fn pack_app(root: &Path, slug: &str, author: &AuthorIdentity) -> Result<PackedApp, String> {
    let source_dir = root.join(slug);
    let metadata = read_manifest_meta(&source_dir.join("riot-app.json"))?;
    let mut resources = build_resources(&source_dir)?;
    resources.sort_by(|left, right| left.path.as_bytes().cmp(right.path.as_bytes()));
    if !resources
        .iter()
        .any(|resource| resource.path == metadata.entry_point)
    {
        return Err(format!(
            "{slug}: entry point '{}' is not a packed resource",
            metadata.entry_point
        ));
    }

    let bundle_bytes = encode_app_bundle(&AppBundle {
        entry_point: metadata.entry_point.clone(),
        resources,
    })
    .map_err(|error| format!("{slug}: encode bundle: {error}"))?;
    let manifest = AppManifest {
        name: metadata.name,
        description: metadata.description,
        version: metadata.version,
        author: author.clone(),
        permissions: metadata.permissions,
        entry_point: metadata.entry_point,
    };
    let manifest_bytes =
        encode_manifest(&manifest).map_err(|error| format!("{slug}: encode manifest: {error}"))?;
    let verified = verify_starter_catalog(&[(&manifest_bytes, &bundle_bytes)]);
    if verified.len() != 1 {
        return Err(format!(
            "{slug}: generated pair failed runtime verification"
        ));
    }
    let app_id = app_id_for(&manifest, &app_bundle_digest(&bundle_bytes))
        .map_err(|error| format!("{slug}: derive app id: {error}"))?;
    Ok(PackedApp {
        slug: slug.to_string(),
        manifest_bytes,
        bundle_bytes,
        app_id,
    })
}

fn assert_frozen(path: &Path, generated: &[u8]) -> Result<(), String> {
    let committed = std::fs::read(path)
        .map_err(|error| format!("read frozen artifact {}: {error}", path.display()))?;
    if committed != generated {
        return Err(format!(
            "refusing to change frozen Checklist artifact {}",
            path.display()
        ));
    }
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
    let raw = std::fs::read_to_string(path)
        .map_err(|error| format!("read {}: {error}", path.display()))?;
    let value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|error| format!("parse {}: {error}", path.display()))?;
    let permissions = value
        .get("permissions")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("{}: permissions must be an array", path.display()))?
        .iter()
        .map(|permission| {
            permission
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| format!("{}: permissions must be strings", path.display()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ManifestMeta {
        name: required_string(&value, "name", path)?,
        description: required_string(&value, "description", path)?,
        version: required_string(&value, "version", path)?,
        entry_point: required_string(&value, "entry_point", path)?,
        permissions,
    })
}

fn required_string(value: &serde_json::Value, key: &str, path: &Path) -> Result<String, String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("{}: {key} must be a string", path.display()))
}

fn build_resources(source_dir: &Path) -> Result<Vec<AppResource>, String> {
    let mut resources = Vec::new();
    let entries = std::fs::read_dir(source_dir)
        .map_err(|error| format!("read {}: {error}", source_dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("read directory entry: {error}"))?;
        if !entry
            .file_type()
            .map_err(|error| format!("stat {}: {error}", entry.path().display()))?
            .is_file()
        {
            continue;
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| format!("non-UTF-8 resource in {}", source_dir.display()))?;
        if name == "riot-app.json" {
            continue;
        }
        let bytes = std::fs::read(entry.path())
            .map_err(|error| format!("read {}: {error}", entry.path().display()))?;
        resources.push(AppResource {
            path: name.clone(),
            content_type: content_type_for(&name)?.to_string(),
            bytes,
        });
    }
    Ok(resources)
}

fn content_type_for(file_name: &str) -> Result<&'static str, String> {
    match file_name.rsplit('.').next().unwrap_or("") {
        "html" => Ok("text/html"),
        "js" => Ok("text/javascript"),
        "css" => Ok("text/css"),
        "svg" => Ok("image/svg+xml"),
        "png" => Ok("image/png"),
        extension => Err(format!(
            "unsupported resource {file_name}: no content type for .{extension}"
        )),
    }
}

fn decode_hex32(hex: &str) -> Result<[u8; 32], String> {
    if hex.len() != 64 {
        return Err(format!("expected 64 hex characters, got {}", hex.len()));
    }
    let mut output = [0_u8; 32];
    for (index, byte) in output.iter_mut().enumerate() {
        *byte = (hex_nibble(hex.as_bytes()[index * 2])? << 4)
            | hex_nibble(hex.as_bytes()[index * 2 + 1])?;
    }
    Ok(output)
}

fn hex_nibble(byte: u8) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(format!("invalid hex digit {}", byte as char)),
    }
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}
