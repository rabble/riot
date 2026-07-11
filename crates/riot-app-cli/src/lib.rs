use std::collections::BTreeSet;
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

use ed25519_dalek::Signature;
use rand_core::{OsRng, RngCore};
use riot_core::apps::bundle::{
    app_bundle_digest, decode_app_bundle, encode_app_bundle, AppBundle, AppResource,
    MAX_BUNDLE_TOTAL_BYTES,
};
use riot_core::apps::directory::AppProvenance;
use riot_core::apps::index::{app_index_bundle_path, app_index_manifest_path, scan_app_index};
use riot_core::apps::manifest::{app_id_for, decode_manifest, encode_manifest, AppId, AppManifest};
use riot_core::import::{decode_bundle, encode_bundle, BundleDecodeOutcome, ItemStatus};
use riot_core::session::{ImportContext, InspectOutcome, RiotSession};
use riot_core::willow::{
    authorise_entry, encode_capability, encode_entry, generate_communal_author, AuthorIdentity,
    Entry, EvidenceAuthor, Path as WillowPath, SignedWillowEntry,
};
use serde::de::{self, Deserializer, MapAccess, Visitor};
use zeroize::Zeroizing;

pub const KEY_WARNING: &str =
    "Protect both author.wrapkey and author.sealed; anyone with both files can publish as this author.";

pub struct PackInput<'a> {
    pub app_dir: &'a Path,
    pub author: &'a EvidenceAuthor,
    pub timestamp_micros: u64,
}

#[derive(Debug)]
pub struct PackOutput {
    pub app_id: AppId,
    pub manifest_bytes: Vec<u8>,
    pub bundle_bytes: Vec<u8>,
    pub import_bundle_bytes: Vec<u8>,
}

#[derive(Debug)]
pub enum PackError {
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    ManifestJsonInvalid {
        reason: String,
    },
    InvalidResourcePath {
        path: String,
    },
    UnsupportedResource {
        path: String,
    },
    MissingEntryPoint {
        entry_point: String,
    },
    Symlink {
        path: String,
    },
    NonRegularFile {
        path: String,
    },
    TooLarge {
        actual: usize,
        limit: usize,
    },
    Core {
        operation: &'static str,
    },
}

impl std::fmt::Display for PackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io {
                operation,
                path,
                source,
            } => {
                write!(f, "{operation} '{}': {source}", path.display())
            }
            Self::ManifestJsonInvalid { reason } => write!(f, "riot-app.json: {reason}"),
            Self::InvalidResourcePath { path } => write!(f, "invalid resource path '{path}'"),
            Self::UnsupportedResource { path } => {
                write!(f, "unsupported resource file '{path}'")
            }
            Self::MissingEntryPoint { entry_point } => {
                write!(f, "entry_point '{entry_point}' is not a packed resource")
            }
            Self::Symlink { path } => write!(f, "symbolic link is not allowed: '{path}'"),
            Self::NonRegularFile { path } => {
                write!(f, "non-regular resource is not allowed: '{path}'")
            }
            Self::TooLarge { actual, limit } => {
                write!(
                    f,
                    "app bundle is too large: {actual} bytes (limit {limit} bytes)"
                )
            }
            Self::Core { operation } => {
                write!(f, "Riot rejected the app while trying to {operation}")
            }
        }
    }
}

impl std::error::Error for PackError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct InspectReport {
    pub name: String,
    pub version: String,
    pub author: AuthorIdentity,
    pub app_id: AppId,
    pub resources: Vec<String>,
}

#[derive(Debug)]
pub enum InspectError {
    InvalidImportBundle { reason: &'static str },
    IncoherentPair { reason: &'static str },
}

impl std::fmt::Display for InspectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidImportBundle { reason } => {
                write!(f, "invalid Riot import bundle: {reason}")
            }
            Self::IncoherentPair { reason } => write!(f, "incoherent app-index pair: {reason}"),
        }
    }
}
impl std::error::Error for InspectError {}

#[derive(Debug)]
pub enum KeyError {
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    AlreadyExists {
        path: PathBuf,
    },
    InvalidWrapKey,
    InvalidSealedIdentity,
    EntropyUnavailable,
    InvalidOutputDirectory,
}

impl std::fmt::Display for KeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io {
                operation,
                path,
                source,
            } => write!(f, "{operation} '{}': {source}", path.display()),
            Self::AlreadyExists { path } => write!(
                f,
                "refusing to overwrite existing '{}': move or remove it first",
                path.display()
            ),
            Self::InvalidWrapKey => write!(
                f,
                "author.wrapkey must contain exactly 64 lowercase hexadecimal characters"
            ),
            Self::InvalidSealedIdentity => write!(
                f,
                "author.sealed is invalid, damaged, or does not match author.wrapkey"
            ),
            Self::EntropyUnavailable => write!(f, "operating-system randomness is unavailable"),
            Self::InvalidOutputDirectory => {
                write!(
                    f,
                    "key output must name a new directory below an existing parent"
                )
            }
        }
    }
}
impl std::error::Error for KeyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

pub struct KeygenOutput {
    pub identity: AuthorIdentity,
    pub warning: &'static str,
}

#[derive(Debug)]
struct ManifestInput {
    name: String,
    description: String,
    version: String,
    entry_point: String,
    permissions: Vec<String>,
}

pub fn pack(input: PackInput<'_>) -> Result<PackOutput, PackError> {
    let source = input.app_dir.join("riot-app.json");
    let raw = fs::read(&source).map_err(|source_error| PackError::Io {
        operation: "read",
        path: source,
        source: source_error,
    })?;
    let meta = parse_manifest_input(&raw)?;
    let mut resources = Vec::new();
    collect_resources(input.app_dir, input.app_dir, &mut resources)?;
    resources.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));

    let actual = resources
        .iter()
        .try_fold(0usize, |sum, resource| {
            sum.checked_add(resource.bytes.len())
        })
        .ok_or(PackError::TooLarge {
            actual: usize::MAX,
            limit: MAX_BUNDLE_TOTAL_BYTES,
        })?;
    if actual > MAX_BUNDLE_TOTAL_BYTES {
        return Err(PackError::TooLarge {
            actual,
            limit: MAX_BUNDLE_TOTAL_BYTES,
        });
    }
    if !resources
        .iter()
        .any(|resource| resource.path == meta.entry_point)
    {
        return Err(PackError::MissingEntryPoint {
            entry_point: meta.entry_point,
        });
    }

    let app_bundle = AppBundle {
        entry_point: meta.entry_point.clone(),
        resources,
    };
    let bundle_bytes = encode_app_bundle(&app_bundle).map_err(|error| match error {
        riot_core::apps::AppsError::BundleTooLarge => PackError::TooLarge {
            actual,
            limit: MAX_BUNDLE_TOTAL_BYTES,
        },
        _ => PackError::Core {
            operation: "encode the resource bundle",
        },
    })?;
    let manifest = AppManifest {
        name: meta.name,
        description: meta.description,
        version: meta.version,
        author: input.author.identity(),
        permissions: meta.permissions,
        entry_point: meta.entry_point,
    };
    let manifest_bytes = encode_manifest(&manifest).map_err(|_| PackError::Core {
        operation: "encode the app manifest",
    })?;
    let app_id =
        app_id_for(&manifest, &app_bundle_digest(&bundle_bytes)).map_err(|_| PackError::Core {
            operation: "derive the app ID",
        })?;
    let manifest_path = app_index_manifest_path(&app_id).map_err(|_| PackError::Core {
        operation: "build the manifest path",
    })?;
    let bundle_path = app_index_bundle_path(&app_id).map_err(|_| PackError::Core {
        operation: "build the bundle path",
    })?;
    let entries = [
        sign_at(
            input.author,
            manifest_path,
            &manifest_bytes,
            input.timestamp_micros,
        )?,
        sign_at(
            input.author,
            bundle_path,
            &bundle_bytes,
            input.timestamp_micros,
        )?,
    ];
    let import_bundle_bytes = encode_bundle(&entries).map_err(|_| PackError::Core {
        operation: "encode the import bundle",
    })?;
    Ok(PackOutput {
        app_id,
        manifest_bytes,
        bundle_bytes,
        import_bundle_bytes,
    })
}

fn parse_manifest_input(input: &[u8]) -> Result<ManifestInput, PackError> {
    let mut deserializer = serde_json::Deserializer::from_slice(input);
    let manifest = deserializer
        .deserialize_map(ManifestInputVisitor)
        .map_err(|error| PackError::ManifestJsonInvalid {
            reason: error.to_string(),
        })?;
    deserializer
        .end()
        .map_err(|error| PackError::ManifestJsonInvalid {
            reason: error.to_string(),
        })?;
    Ok(manifest)
}

struct ManifestInputVisitor;

impl<'de> Visitor<'de> for ManifestInputVisitor {
    type Value = ManifestInput;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a riot-app.json object")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut name = None;
        let mut description = None;
        let mut version = None;
        let mut entry_point = None;
        let mut permissions = None;
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "name" => {
                    if name.is_some() {
                        return Err(de::Error::custom("duplicate field 'name'"));
                    }
                    name = Some(
                        map.next_value::<String>()
                            .map_err(|_| de::Error::custom("'name' must be a string"))?,
                    );
                }
                "description" => {
                    if description.is_some() {
                        return Err(de::Error::custom("duplicate field 'description'"));
                    }
                    description = Some(
                        map.next_value::<String>()
                            .map_err(|_| de::Error::custom("'description' must be a string"))?,
                    );
                }
                "version" => {
                    if version.is_some() {
                        return Err(de::Error::custom("duplicate field 'version'"));
                    }
                    version = Some(
                        map.next_value::<String>()
                            .map_err(|_| de::Error::custom("'version' must be a string"))?,
                    );
                }
                "entry_point" => {
                    if entry_point.is_some() {
                        return Err(de::Error::custom("duplicate field 'entry_point'"));
                    }
                    entry_point = Some(
                        map.next_value::<String>()
                            .map_err(|_| de::Error::custom("'entry_point' must be a string"))?,
                    );
                }
                "permissions" => {
                    if permissions.is_some() {
                        return Err(de::Error::custom("duplicate field 'permissions'"));
                    }
                    let raw = map.next_value::<serde_json::Value>()?;
                    let values = raw
                        .as_array()
                        .ok_or_else(|| de::Error::custom("'permissions' must be an array"))?;
                    let mut parsed = Vec::with_capacity(values.len());
                    for (index, value) in values.iter().enumerate() {
                        parsed.push(value.as_str().map(str::to_owned).ok_or_else(|| {
                            de::Error::custom(format!(
                                "permission at index {index} must be a string"
                            ))
                        })?);
                    }
                    permissions = Some(parsed);
                }
                _ => {
                    return Err(de::Error::custom(format!("unknown field '{key}'")));
                }
            }
        }
        Ok(ManifestInput {
            name: name.ok_or_else(|| de::Error::missing_field("name"))?,
            description: description.ok_or_else(|| de::Error::missing_field("description"))?,
            version: version.ok_or_else(|| de::Error::missing_field("version"))?,
            entry_point: entry_point.ok_or_else(|| de::Error::missing_field("entry_point"))?,
            permissions: permissions.ok_or_else(|| de::Error::missing_field("permissions"))?,
        })
    }
}

fn collect_resources(
    root: &Path,
    directory: &Path,
    out: &mut Vec<AppResource>,
) -> Result<(), PackError> {
    let entries = fs::read_dir(directory).map_err(|source| PackError::Io {
        operation: "read directory",
        path: directory.to_path_buf(),
        source,
    })?;
    for result in entries {
        let entry = result.map_err(|source| PackError::Io {
            operation: "read directory entry",
            path: directory.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let relative = normalized_relative(root, &path)?;
        let file_type = entry.file_type().map_err(|source| PackError::Io {
            operation: "inspect",
            path: path.clone(),
            source,
        })?;
        if file_type.is_symlink() {
            return Err(PackError::Symlink { path: relative });
        }
        if file_type.is_dir() {
            collect_resources(root, &path, out)?;
        } else if file_type.is_file() {
            if relative == "riot-app.json" {
                continue;
            }
            let content_type = content_type_for(&relative)?;
            let bytes = fs::read(&path).map_err(|source| PackError::Io {
                operation: "read resource",
                path: path.clone(),
                source,
            })?;
            out.push(AppResource {
                path: relative,
                content_type: content_type.into(),
                bytes,
            });
        } else {
            return Err(PackError::NonRegularFile { path: relative });
        }
    }
    Ok(())
}

fn normalized_relative(root: &Path, path: &Path) -> Result<String, PackError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| PackError::InvalidResourcePath {
            path: path.display().to_string(),
        })?;
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(value) => {
                parts.push(
                    value
                        .to_str()
                        .ok_or_else(|| PackError::InvalidResourcePath {
                            path: "<non-UTF-8>".into(),
                        })?,
                )
            }
            _ => {
                return Err(PackError::InvalidResourcePath {
                    path: relative.display().to_string(),
                })
            }
        }
    }
    let normalized = parts.join("/");
    if normalized.is_empty()
        || normalized.starts_with('/')
        || normalized.contains('\\')
        || normalized
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(PackError::InvalidResourcePath { path: normalized });
    }
    Ok(normalized)
}

fn content_type_for(path: &str) -> Result<&'static str, PackError> {
    match path.rsplit_once('.').map(|(_, extension)| extension) {
        Some("html") => Ok("text/html"),
        Some("js") => Ok("text/javascript"),
        Some("css") => Ok("text/css"),
        Some("json") => Ok("application/json"),
        Some("png") => Ok("image/png"),
        Some("svg") => Ok("image/svg+xml"),
        _ => Err(PackError::UnsupportedResource {
            path: path.to_owned(),
        }),
    }
}

fn sign_at(
    author: &EvidenceAuthor,
    path: WillowPath,
    payload: &[u8],
    timestamp: u64,
) -> Result<SignedWillowEntry, PackError> {
    let entry = Entry::builder()
        .namespace_id(author.namespace_id().clone())
        .subspace_id(author.subspace_id())
        .path(path)
        .timestamp(timestamp)
        .payload(payload)
        .build();
    let authorised = authorise_entry(author, entry).map_err(|_| PackError::Core {
        operation: "authorize an app-index entry",
    })?;
    let token = authorised.authorisation_token();
    let signature: Signature = token.signature().clone().into();
    Ok(SignedWillowEntry {
        entry_bytes: encode_entry(authorised.entry()),
        capability_bytes: encode_capability(token.capability()),
        signature: signature.to_bytes(),
        payload_bytes: payload.to_vec(),
    })
}

pub fn inspect(bytes: &[u8]) -> Result<InspectReport, InspectError> {
    let decoded = match decode_bundle(bytes) {
        BundleDecodeOutcome::Decoded(decoded) => decoded,
        BundleDecodeOutcome::Rejected(_) => {
            return Err(InspectError::InvalidImportBundle {
                reason: "strict decoding rejected it",
            })
        }
    };
    if decoded.items.len() != 2 {
        return Err(InspectError::IncoherentPair {
            reason: "exactly two entries are required",
        });
    }
    let mut manifest = None;
    let mut bundle = None;
    for item in &decoded.items {
        if !matches!(item.status, ItemStatus::Valid(_)) {
            return Err(InspectError::InvalidImportBundle {
                reason: "an entry failed signature or schema verification",
            });
        }
        let payload = item.frame.payload_bytes();
        match (decode_manifest(payload), decode_app_bundle(payload)) {
            (Ok(value), Err(_)) if manifest.is_none() => manifest = Some(value),
            (Err(_), Ok(value)) if bundle.is_none() => bundle = Some((value, payload.to_vec())),
            _ => {
                return Err(InspectError::IncoherentPair {
                    reason: "entries are not one manifest and one resource bundle",
                })
            }
        }
    }
    let manifest = manifest.ok_or(InspectError::IncoherentPair {
        reason: "manifest is missing",
    })?;
    let (bundle, bundle_bytes) = bundle.ok_or(InspectError::IncoherentPair {
        reason: "resource bundle is missing",
    })?;
    if manifest.entry_point != bundle.entry_point {
        return Err(InspectError::IncoherentPair {
            reason: "manifest and bundle entry points differ",
        });
    }
    let app_id = app_id_for(&manifest, &app_bundle_digest(&bundle_bytes)).map_err(|_| {
        InspectError::IncoherentPair {
            reason: "canonical app ID cannot be derived",
        }
    })?;

    // Commit through the production import path, then use the app-index scanner
    // as the final path/payload/carrier coherence authority.
    let session = RiotSession::open().map_err(|_| InspectError::InvalidImportBundle {
        reason: "cannot open verification session",
    })?;
    let store = session
        .create_store()
        .map_err(|_| InspectError::InvalidImportBundle {
            reason: "cannot create verification store",
        })?;
    let outcome = store
        .inspect(bytes, ImportContext::new("riot-app inspect"))
        .map_err(|_| InspectError::InvalidImportBundle {
            reason: "import inspection failed",
        })?;
    let preview = match outcome {
        InspectOutcome::Preview(preview) => preview,
        InspectOutcome::Rejected(_) => {
            return Err(InspectError::InvalidImportBundle {
                reason: "production import inspection rejected it",
            })
        }
    };
    preview
        .plan_all()
        .and_then(|plan| plan.commit())
        .map_err(|_| InspectError::InvalidImportBundle {
            reason: "verified pair could not be committed",
        })?;
    let scanned = scan_app_index(&store).map_err(|_| InspectError::IncoherentPair {
        reason: "app-index scan failed",
    })?;
    if !scanned.pending_manifests.is_empty() || scanned.apps.len() != 1 {
        return Err(InspectError::IncoherentPair {
            reason: "paths do not form one complete app-index pair",
        });
    }
    let indexed = &scanned.apps[0];
    if indexed.app_id != app_id || indexed.manifest != manifest || !indexed.bundle_present {
        return Err(InspectError::IncoherentPair {
            reason: "path, payload, and app ID do not agree",
        });
    }
    if !matches!(indexed.provenance, AppProvenance::Carried { carrier_subspace_id } if carrier_subspace_id == manifest.author.subspace_id)
    {
        return Err(InspectError::IncoherentPair {
            reason: "manifest author does not match the signing carrier",
        });
    }
    let mut resources = bundle
        .resources
        .into_iter()
        .map(|resource| resource.path)
        .collect::<Vec<_>>();
    resources.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
    if resources.iter().collect::<BTreeSet<_>>().len() != resources.len() {
        return Err(InspectError::IncoherentPair {
            reason: "resource paths are duplicated",
        });
    }
    Ok(InspectReport {
        name: manifest.name,
        version: manifest.version,
        author: manifest.author,
        app_id,
        resources,
    })
}

pub fn keygen(out: &Path) -> Result<KeygenOutput, KeyError> {
    keygen_inner(out, |_| Ok(()))
}

#[derive(Clone, Copy)]
enum KeygenStage {
    AfterWrapKey,
}

fn keygen_inner<F>(out: &Path, mut checkpoint: F) -> Result<KeygenOutput, KeyError>
where
    F: FnMut(KeygenStage) -> Result<(), KeyError>,
{
    ensure_absent(out)?;
    if out.file_name().is_none() {
        return Err(KeyError::InvalidOutputDirectory);
    }
    let parent = match out.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    if !parent.is_dir() {
        return Err(KeyError::InvalidOutputDirectory);
    }
    let temp = create_key_temp_directory(parent)?;
    let mut published = false;
    let result = (|| {
        let author = generate_communal_author().map_err(|_| KeyError::EntropyUnavailable)?;
        let identity = author.identity();
        let mut wrapping_key = Zeroizing::new([0u8; 32]);
        OsRng
            .try_fill_bytes(&mut wrapping_key[..])
            .map_err(|_| KeyError::EntropyUnavailable)?;
        let sealed = author
            .seal_identity(&wrapping_key)
            .map_err(|_| KeyError::EntropyUnavailable)?;
        let encoded_key = Zeroizing::new(hex_lower_bytes(&wrapping_key[..]));

        write_private_temp(&temp.join("author.wrapkey"), &encoded_key)?;
        checkpoint(KeygenStage::AfterWrapKey)?;
        write_private_temp(&temp.join("author.sealed"), &sealed)?;
        sync_directory(&temp)?;
        sync_directory(parent)?;
        ensure_absent(out)?;
        fs::rename(&temp, out).map_err(|source| KeyError::Io {
            operation: "publish key directory",
            path: out.to_path_buf(),
            source,
        })?;
        published = true;
        if let Err(error) = sync_directory(parent) {
            let _ = fs::remove_dir_all(out);
            let _ = sync_directory(parent);
            return Err(error);
        }
        Ok(KeygenOutput {
            identity,
            warning: KEY_WARNING,
        })
    })();
    if !published {
        let _ = fs::remove_dir_all(&temp);
    }
    result
}

fn ensure_absent(path: &Path) -> Result<(), KeyError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(KeyError::AlreadyExists {
            path: path.to_path_buf(),
        }),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(KeyError::Io {
            operation: "inspect key output",
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn create_key_temp_directory(parent: &Path) -> Result<PathBuf, KeyError> {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    for _ in 0..128 {
        let sequence = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = parent.join(format!(
            ".riot-app-keygen-{}-{sequence}",
            std::process::id()
        ));
        let mut builder = DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        match builder.create(&path) {
            Ok(()) => return Ok(path),
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(source) => {
                return Err(KeyError::Io {
                    operation: "create temporary key directory",
                    path,
                    source,
                })
            }
        }
    }
    Err(KeyError::Io {
        operation: "create temporary key directory",
        path: parent.to_path_buf(),
        source: io::Error::new(io::ErrorKind::AlreadyExists, "temporary names exhausted"),
    })
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), KeyError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| KeyError::Io {
            operation: "sync directory",
            path: path.to_path_buf(),
            source,
        })
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), KeyError> {
    Ok(())
}

pub fn load_author(key_dir: &Path) -> Result<EvidenceAuthor, KeyError> {
    let key_path = key_dir.join("author.wrapkey");
    let sealed_path = key_dir.join("author.sealed");
    let encoded = Zeroizing::new(fs::read(&key_path).map_err(|source| KeyError::Io {
        operation: "read",
        path: key_path,
        source,
    })?);
    let wrapping_key = decode_lower_hex_key(&encoded)?;
    let sealed = fs::read(&sealed_path).map_err(|source| KeyError::Io {
        operation: "read",
        path: sealed_path,
        source,
    })?;
    EvidenceAuthor::open_sealed_identity(&wrapping_key, &sealed)
        .map_err(|_| KeyError::InvalidSealedIdentity)
}

pub fn write_new_atomic(path: &Path, bytes: &[u8]) -> Result<(), KeyError> {
    if path.exists() {
        return Err(KeyError::AlreadyExists {
            path: path.to_path_buf(),
        });
    }
    let temp = temp_path(path);
    let result = (|| {
        write_private_temp(&temp, bytes)?;
        fs::hard_link(&temp, path).map_err(|source| {
            if source.kind() == io::ErrorKind::AlreadyExists {
                KeyError::AlreadyExists {
                    path: path.to_path_buf(),
                }
            } else {
                KeyError::Io {
                    operation: "publish",
                    path: path.to_path_buf(),
                    source,
                }
            }
        })?;
        Ok(())
    })();
    let _ = fs::remove_file(&temp);
    result
}

fn temp_path(path: &Path) -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let sequence = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut value = path.as_os_str().to_owned();
    value.push(format!(".tmp-{}-{sequence}", std::process::id()));
    PathBuf::from(value)
}

fn write_private_temp(path: &Path, bytes: &[u8]) -> Result<(), KeyError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|source| KeyError::Io {
        operation: "create temporary file",
        path: path.to_path_buf(),
        source,
    })?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|source| KeyError::Io {
            operation: "write temporary file",
            path: path.to_path_buf(),
            source,
        })
}

fn decode_lower_hex_key(input: &[u8]) -> Result<Zeroizing<[u8; 32]>, KeyError> {
    if input.len() != 64
        || !input
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(KeyError::InvalidWrapKey);
    }
    let mut output = Zeroizing::new([0u8; 32]);
    for (index, slot) in output.iter_mut().enumerate() {
        *slot = (hex_nibble(input[index * 2])? << 4) | hex_nibble(input[index * 2 + 1])?;
    }
    Ok(output)
}

fn hex_nibble(value: u8) -> Result<u8, KeyError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(KeyError::InvalidWrapKey),
    }
}

pub fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0xf) as usize] as char);
    }
    output
}

fn hex_lower_bytes(bytes: &[u8]) -> Vec<u8> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = Vec::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize]);
        output.push(HEX[(byte & 0xf) as usize]);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{keygen_inner, KeyError, KeygenStage};

    #[test]
    fn keygen_failure_after_first_temp_file_leaves_no_final_or_temp_directory() {
        let parent = tempfile::tempdir().expect("temp parent");
        let output = parent.path().join("keys");
        let error = keygen_inner(&output, |stage| match stage {
            KeygenStage::AfterWrapKey => Err(KeyError::InvalidOutputDirectory),
        });
        assert!(error.is_err());
        assert!(!output.exists());
        assert_eq!(
            std::fs::read_dir(parent.path())
                .expect("read parent")
                .count(),
            0
        );
    }
}
