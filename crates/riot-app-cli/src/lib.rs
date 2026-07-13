//! macOS/Linux-only secure publisher for Riot app bundles.
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
compile_error!("riot-app-cli currently supports only macOS and Linux");

use std::collections::BTreeSet;
use std::ffi::{CStr, CString, OsStr, OsString};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Component, Path, PathBuf};

use ed25519_dalek::Signature;
use rand_core::{OsRng, RngCore};
use riot_core::apps::bundle::{
    app_bundle_digest, decode_app_bundle, encode_app_bundle, AppBundle, AppResource,
    MAX_BUNDLE_RESOURCES, MAX_BUNDLE_TOTAL_BYTES, MAX_RESOURCE_PATH_BYTES,
};
use riot_core::apps::directory::AppProvenance;
use riot_core::apps::index::{app_index_bundle_path, app_index_manifest_path, scan_app_index};
use riot_core::apps::manifest::{
    app_id_for, decode_manifest, encode_manifest, AppId, AppManifest, MAX_APP_DESCRIPTION_BYTES,
    MAX_APP_ENTRY_POINT_BYTES, MAX_APP_NAME_BYTES, MAX_APP_PERMISSIONS, MAX_APP_PERMISSION_BYTES,
    MAX_APP_VERSION_BYTES, MAX_MANIFEST_BYTES,
};
use riot_core::apps::AppsError;
use riot_core::import::{
    decode_bundle, encode_bundle, BundleDecodeOutcome, BundleEncodeError, ItemStatus,
    MAX_BUNDLE_BYTES,
};
use riot_core::session::{ImportContext, ImportPreview, InspectOutcome, RiotSession};
use riot_core::willow::{
    authorise_entry, encode_capability, encode_entry, generate_communal_author, AuthorIdentity,
    Entry, EvidenceAuthor, Path as WillowPath, SignedWillowEntry, WillowError,
};
use serde::de::{self, DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor};
use willow25::groupings::{Coordinatelike, Keylike, Namespaced};
use zeroize::Zeroizing;

pub const KEY_WARNING: &str =
    "Protect both author.wrapkey and author.sealed; anyone with both files can publish as this author.";
const MAX_RESOURCE_PATH_COMPONENTS: usize = 64;
const MAX_DIRECTORY_ENTRIES: usize = 4_096;
const MAX_TOTAL_ENTRIES: usize = 4_096;
const MAX_DIRECTORIES: usize = 256;

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
    TooManyResources {
        actual: usize,
        limit: usize,
    },
    EncodedTooLarge {
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
                write!(
                    f,
                    "{operation} '{}': {source}",
                    path.to_string_lossy().escape_default()
                )
            }
            Self::ManifestJsonInvalid { reason } => {
                write!(f, "riot-app.json: {}", escape_text(reason))
            }
            Self::InvalidResourcePath { path } => {
                write!(f, "invalid resource path '{}'", escape_text(path))
            }
            Self::UnsupportedResource { path } => {
                write!(f, "unsupported resource file '{}'", escape_text(path))
            }
            Self::MissingEntryPoint { entry_point } => {
                write!(
                    f,
                    "entry_point '{}' is not a packed resource",
                    escape_text(entry_point)
                )
            }
            Self::Symlink { path } => {
                write!(f, "symbolic link is not allowed: '{}'", escape_text(path))
            }
            Self::NonRegularFile { path } => {
                write!(
                    f,
                    "non-regular resource is not allowed: '{}'",
                    escape_text(path)
                )
            }
            Self::TooLarge { actual, limit } => {
                write!(
                    f,
                    "app bundle is too large: {actual} bytes (limit {limit} bytes)"
                )
            }
            Self::TooManyResources { actual, limit } => write!(
                f,
                "app has too many resources: {actual} files (limit {limit} files)"
            ),
            Self::EncodedTooLarge { limit } => {
                write!(f, "encoded app bundle exceeds its {limit}-byte limit")
            }
            Self::Core { operation } => {
                write!(f, "Riot rejected the app while trying to {operation}")
            }
        }
    }
}

fn escape_text(value: &str) -> String {
    value.chars().flat_map(char::escape_default).collect()
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

#[derive(Clone, Copy, Debug)]
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
            } => write!(
                f,
                "{operation} '{}': {source}",
                path.to_string_lossy().escape_default()
            ),
            Self::AlreadyExists { path } => write!(
                f,
                "refusing to overwrite existing '{}': move or remove it first",
                path.to_string_lossy().escape_default()
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

struct PinnedDir(File);

trait PlatformFs {
    fn open_path(&self, path: &Path) -> io::Result<PinnedDir>;
    fn open_child(
        &self,
        directory: &PinnedDir,
        name: &OsStr,
        flags: libc::c_int,
    ) -> io::Result<File>;
    fn create_child(
        &self,
        directory: &PinnedDir,
        name: &OsStr,
        flags: libc::c_int,
        mode: libc::mode_t,
    ) -> io::Result<File>;
    fn read_dir(&self, directory: &PinnedDir, total: &mut usize) -> io::Result<Vec<OsString>>;
    fn metadata(&self, file: &File) -> io::Result<std::fs::Metadata>;
    fn read_to_end(&self, file: &mut File, limit: u64, bytes: &mut Vec<u8>) -> io::Result<usize>;
    fn write_all(&self, file: &mut File, bytes: &[u8]) -> io::Result<()>;
    fn fsync(&self, file: &File) -> io::Result<()>;
    fn mkdir(&self, directory: &PinnedDir, name: &OsStr, mode: libc::mode_t) -> io::Result<()>;
    fn rename(
        &self,
        from_dir: &PinnedDir,
        from: &OsStr,
        to_dir: &PinnedDir,
        to: &OsStr,
    ) -> io::Result<()>;
    fn unlink(&self, directory: &PinnedDir, name: &OsStr, flags: libc::c_int) -> io::Result<()>;
}

struct UnixPlatformFs;

impl PlatformFs for UnixPlatformFs {
    fn open_path(&self, path: &Path) -> io::Result<PinnedDir> {
        PinnedDir::open_path(path)
    }

    fn open_child(
        &self,
        directory: &PinnedDir,
        name: &OsStr,
        flags: libc::c_int,
    ) -> io::Result<File> {
        openat_file(directory.0.as_raw_fd(), name, flags)
    }

    fn create_child(
        &self,
        directory: &PinnedDir,
        name: &OsStr,
        flags: libc::c_int,
        mode: libc::mode_t,
    ) -> io::Result<File> {
        createat_file(directory.0.as_raw_fd(), name, flags, mode)
    }

    fn read_dir(&self, directory: &PinnedDir, total: &mut usize) -> io::Result<Vec<OsString>> {
        directory.names(total)
    }

    fn metadata(&self, file: &File) -> io::Result<std::fs::Metadata> {
        file.metadata()
    }

    fn read_to_end(&self, file: &mut File, limit: u64, bytes: &mut Vec<u8>) -> io::Result<usize> {
        Read::by_ref(file).take(limit).read_to_end(bytes)
    }

    fn write_all(&self, file: &mut File, bytes: &[u8]) -> io::Result<()> {
        file.write_all(bytes)
    }

    fn fsync(&self, file: &File) -> io::Result<()> {
        file.sync_all()
    }

    fn mkdir(&self, directory: &PinnedDir, name: &OsStr, mode: libc::mode_t) -> io::Result<()> {
        let name = CString::new(name.as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NUL in path"))?;
        if unsafe { libc::mkdirat(directory.0.as_raw_fd(), name.as_ptr(), mode) } == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    fn rename(
        &self,
        from_dir: &PinnedDir,
        from: &OsStr,
        to_dir: &PinnedDir,
        to: &OsStr,
    ) -> io::Result<()> {
        rename_file_noreplace_at(from_dir, from, to_dir, to)
    }

    fn unlink(&self, directory: &PinnedDir, name: &OsStr, flags: libc::c_int) -> io::Result<()> {
        let name = CString::new(name.as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NUL in path"))?;
        if unsafe { libc::unlinkat(directory.0.as_raw_fd(), name.as_ptr(), flags) } == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
}

trait EntropyPort {
    fn generate_author(&mut self) -> Result<EvidenceAuthor, KeyError>;
    fn fill(&mut self, bytes: &mut [u8]) -> Result<(), KeyError>;
    fn seal_identity(
        &mut self,
        author: &EvidenceAuthor,
        wrapping_key: &[u8; 32],
    ) -> Result<Vec<u8>, KeyError>;
}

struct OsEntropyPort;

impl EntropyPort for OsEntropyPort {
    fn generate_author(&mut self) -> Result<EvidenceAuthor, KeyError> {
        generate_communal_author().map_err(|_| KeyError::EntropyUnavailable)
    }

    fn fill(&mut self, bytes: &mut [u8]) -> Result<(), KeyError> {
        OsRng
            .try_fill_bytes(bytes)
            .map_err(|_| KeyError::EntropyUnavailable)
    }

    fn seal_identity(
        &mut self,
        author: &EvidenceAuthor,
        wrapping_key: &[u8; 32],
    ) -> Result<Vec<u8>, KeyError> {
        author
            .seal_identity(wrapping_key)
            .map_err(|_| KeyError::EntropyUnavailable)
    }
}

trait PackCore {
    fn encode_app_bundle(&self, bundle: &AppBundle) -> Result<Vec<u8>, AppsError>;
    fn encode_manifest(&self, manifest: &AppManifest) -> Result<Vec<u8>, AppsError>;
    fn app_id_for(
        &self,
        manifest: &AppManifest,
        bundle_digest: &[u8; 32],
    ) -> Result<AppId, AppsError>;
    fn manifest_path(&self, app_id: &AppId) -> Result<WillowPath, AppsError>;
    fn bundle_path(&self, app_id: &AppId) -> Result<WillowPath, AppsError>;
    fn sign_at(
        &self,
        author: &EvidenceAuthor,
        path: WillowPath,
        payload: &[u8],
        timestamp: u64,
    ) -> Result<SignedWillowEntry, WillowError>;
    fn encode_import_bundle(
        &self,
        entries: &[SignedWillowEntry],
    ) -> Result<Vec<u8>, BundleEncodeError>;
}

struct RiotPackCore;

impl PackCore for RiotPackCore {
    fn encode_app_bundle(&self, bundle: &AppBundle) -> Result<Vec<u8>, AppsError> {
        encode_app_bundle(bundle)
    }

    fn encode_manifest(&self, manifest: &AppManifest) -> Result<Vec<u8>, AppsError> {
        encode_manifest(manifest)
    }

    fn app_id_for(
        &self,
        manifest: &AppManifest,
        bundle_digest: &[u8; 32],
    ) -> Result<AppId, AppsError> {
        app_id_for(manifest, bundle_digest)
    }

    fn manifest_path(&self, app_id: &AppId) -> Result<WillowPath, AppsError> {
        app_index_manifest_path(app_id)
    }

    fn bundle_path(&self, app_id: &AppId) -> Result<WillowPath, AppsError> {
        app_index_bundle_path(app_id)
    }

    fn sign_at(
        &self,
        author: &EvidenceAuthor,
        path: WillowPath,
        payload: &[u8],
        timestamp: u64,
    ) -> Result<SignedWillowEntry, WillowError> {
        sign_at(author, path, payload, timestamp)
    }

    fn encode_import_bundle(
        &self,
        entries: &[SignedWillowEntry],
    ) -> Result<Vec<u8>, BundleEncodeError> {
        encode_bundle(entries)
    }
}

impl PinnedDir {
    fn open_path(path: &Path) -> Result<Self, io::Error> {
        Self::open_path_with_fault(path, false)
    }

    fn open_path_with_fault(path: &Path, reject_start: bool) -> Result<Self, io::Error> {
        use std::os::unix::fs::OpenOptionsExt;
        let start = if path.is_absolute() {
            Path::new("/")
        } else {
            Path::new(".")
        };
        let start_result = if reject_start {
            Err(io::Error::other("injected start-directory failure"))
        } else {
            OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(start)
        };
        let mut current = start_result?;
        for component in path.components() {
            match component {
                Component::RootDir | Component::CurDir => continue,
                Component::Normal(name) => {
                    current = openat_file(
                        current.as_raw_fd(),
                        name,
                        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                    )?
                }
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "unsafe path component",
                    ))
                }
            }
        }
        Ok(Self(current))
    }

    fn names(&self, total: &mut usize) -> io::Result<Vec<OsString>> {
        self.names_with(total, &mut |directory| unsafe { libc::readdir(directory) })
    }

    fn names_with(
        &self,
        total: &mut usize,
        next: &mut dyn FnMut(*mut libc::DIR) -> *mut libc::dirent,
    ) -> io::Result<Vec<OsString>> {
        let duplicate = self.duplicate_cloexec()?;
        let raw_directory = unsafe { libc::fdopendir(duplicate) };
        if raw_directory.is_null() {
            unsafe {
                libc::close(duplicate);
            }
            return Err(io::Error::last_os_error());
        }
        let directory = OwnedDir(raw_directory);
        let mut names = Vec::new();
        loop {
            set_errno(0);
            let entry = next(directory.0);
            if entry.is_null() {
                let error = get_errno();
                if error == 0 {
                    break;
                }
                return Err(io::Error::from_raw_os_error(error));
            }
            let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
            if name == b"." || name == b".." {
                continue;
            }
            *total = total.saturating_add(1);
            if names.len() == MAX_DIRECTORY_ENTRIES || *total > MAX_TOTAL_ENTRIES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "directory scan limit exceeded",
                ));
            }
            names.push(OsString::from_vec(name.to_vec()));
        }
        names.sort_by(compare_os_names);
        Ok(names)
    }

    fn duplicate_cloexec(&self) -> io::Result<libc::c_int> {
        let duplicate = unsafe { libc::fcntl(self.0.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 0) };
        if duplicate < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(duplicate)
        }
    }
}

fn compare_os_names(a: &OsString, b: &OsString) -> std::cmp::Ordering {
    a.as_bytes().cmp(b.as_bytes())
}

struct OwnedDir(*mut libc::DIR);

impl Drop for OwnedDir {
    fn drop(&mut self) {
        unsafe {
            libc::closedir(self.0);
        }
    }
}

#[cfg(target_os = "macos")]
fn errno_location() -> *mut libc::c_int {
    unsafe { libc::__error() }
}
#[cfg(target_os = "linux")]
fn errno_location() -> *mut libc::c_int {
    unsafe { libc::__errno_location() }
}
fn set_errno(value: libc::c_int) {
    unsafe {
        *errno_location() = value;
    }
}
fn get_errno() -> libc::c_int {
    unsafe { *errno_location() }
}

fn openat_file(parent: libc::c_int, name: &OsStr, flags: libc::c_int) -> io::Result<File> {
    let name = CString::new(name.as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NUL in path"))?;
    let fd = unsafe { libc::openat(parent, name.as_ptr(), flags) };
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}

fn createat_file(
    parent: libc::c_int,
    name: &OsStr,
    flags: libc::c_int,
    mode: libc::mode_t,
) -> io::Result<File> {
    let name = CString::new(name.as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NUL in path"))?;
    let fd = unsafe { libc::openat(parent, name.as_ptr(), flags, mode as libc::c_uint) };
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}

pub fn pack(input: PackInput<'_>) -> Result<PackOutput, PackError> {
    pack_with_fs(input, &UnixPlatformFs)
}

fn pack_with_fs(input: PackInput<'_>, fs: &dyn PlatformFs) -> Result<PackOutput, PackError> {
    pack_with(input, fs, &RiotPackCore)
}

fn pack_with(
    input: PackInput<'_>,
    fs: &dyn PlatformFs,
    core: &dyn PackCore,
) -> Result<PackOutput, PackError> {
    let root = fs
        .open_path(input.app_dir)
        .map_err(|source| PackError::Io {
            operation: "open app directory without following links",
            path: input.app_dir.to_path_buf(),
            source,
        })?;
    let manifest_file = fs
        .open_child(
            &root,
            OsStr::new("riot-app.json"),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
        )
        .map_err(|source| PackError::Io {
            operation: "open riot-app.json beneath app root",
            path: input.app_dir.join("riot-app.json"),
            source,
        })?;
    let raw = read_opened_bounded_with(fs, manifest_file, MAX_MANIFEST_BYTES, "riot-app.json")?;
    let meta = parse_manifest_input(&raw)?;
    let mut resources = collect_resources(fs, root)?;
    resources.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));

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
    let bundle_bytes = core
        .encode_app_bundle(&app_bundle)
        .map_err(|error| match error {
            AppsError::BundleTooLarge => PackError::EncodedTooLarge {
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
    let manifest_bytes = core
        .encode_manifest(&manifest)
        .map_err(|_| PackError::Core {
            operation: "encode the app manifest",
        })?;
    let app_id = core
        .app_id_for(&manifest, &app_bundle_digest(&bundle_bytes))
        .map_err(|_| PackError::Core {
            operation: "derive the app ID",
        })?;
    let manifest_path = core.manifest_path(&app_id).map_err(|_| PackError::Core {
        operation: "build the manifest path",
    })?;
    let bundle_path = core.bundle_path(&app_id).map_err(|_| PackError::Core {
        operation: "build the bundle path",
    })?;
    let entries = [
        core.sign_at(
            input.author,
            manifest_path,
            &manifest_bytes,
            input.timestamp_micros,
        )
        .map_err(|_| PackError::Core {
            operation: "authorize an app-index entry",
        })?,
        core.sign_at(
            input.author,
            bundle_path,
            &bundle_bytes,
            input.timestamp_micros,
        )
        .map_err(|_| PackError::Core {
            operation: "authorize an app-index entry",
        })?,
    ];
    let import_bundle_bytes = core
        .encode_import_bundle(&entries)
        .map_err(|_| PackError::Core {
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
    parse_manifest_input_with_end_fault(input, false)
}

fn parse_manifest_input_with_end_fault(
    input: &[u8],
    reject_end: bool,
) -> Result<ManifestInput, PackError> {
    let mut deserializer = serde_json::Deserializer::from_slice(input);
    let manifest = deserializer
        .deserialize_map(ManifestInputVisitor)
        .map_err(|error| PackError::ManifestJsonInvalid {
            reason: error.to_string(),
        })?;
    let end = if reject_end {
        Err(serde_json::Error::io(io::Error::other(
            "injected trailing input",
        )))
    } else {
        deserializer.end()
    };
    finish_manifest_parse(end)?;
    Ok(manifest)
}

fn finish_manifest_parse(result: serde_json::Result<()>) -> Result<(), PackError> {
    result.map_err(|error| PackError::ManifestJsonInvalid {
        reason: error.to_string(),
    })
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
                    name = Some(bounded_json_string(
                        map.next_value::<String>()
                            .map_err(|_| de::Error::custom("'name' must be a string"))?,
                        "name",
                        MAX_APP_NAME_BYTES,
                    )?);
                }
                "description" => {
                    if description.is_some() {
                        return Err(de::Error::custom("duplicate field 'description'"));
                    }
                    description = Some(bounded_json_string(
                        map.next_value::<String>()
                            .map_err(|_| de::Error::custom("'description' must be a string"))?,
                        "description",
                        MAX_APP_DESCRIPTION_BYTES,
                    )?);
                }
                "version" => {
                    if version.is_some() {
                        return Err(de::Error::custom("duplicate field 'version'"));
                    }
                    version = Some(bounded_json_string(
                        map.next_value::<String>()
                            .map_err(|_| de::Error::custom("'version' must be a string"))?,
                        "version",
                        MAX_APP_VERSION_BYTES,
                    )?);
                }
                "entry_point" => {
                    if entry_point.is_some() {
                        return Err(de::Error::custom("duplicate field 'entry_point'"));
                    }
                    entry_point = Some(bounded_json_string(
                        map.next_value::<String>()
                            .map_err(|_| de::Error::custom("'entry_point' must be a string"))?,
                        "entry_point",
                        MAX_APP_ENTRY_POINT_BYTES,
                    )?);
                }
                "permissions" => {
                    if permissions.is_some() {
                        return Err(de::Error::custom("duplicate field 'permissions'"));
                    }
                    permissions = Some(map.next_value_seed(PermissionsSeed)?);
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

fn bounded_json_string<E>(value: String, field: &str, limit: usize) -> Result<String, E>
where
    E: de::Error,
{
    if value.is_empty() || value.len() > limit || value.chars().any(char::is_control) {
        return Err(E::custom(format!(
            "'{field}' is empty, too long, or contains control characters"
        )));
    }
    Ok(value)
}

struct PermissionsSeed;

impl<'de> DeserializeSeed<'de> for PermissionsSeed {
    type Value = Vec<String>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(PermissionsVisitor)
    }
}

struct PermissionsVisitor;

impl<'de> Visitor<'de> for PermissionsVisitor {
    type Value = Vec<String>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("'permissions' must be an array")
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut permissions = Vec::with_capacity(MAX_APP_PERMISSIONS);
        while let Some(value) = sequence.next_element_seed(PermissionSeed(permissions.len()))? {
            if permissions.len() == MAX_APP_PERMISSIONS {
                return Err(de::Error::custom("too many permissions"));
            }
            permissions.push(value);
        }
        Ok(permissions)
    }
}

struct PermissionSeed(usize);

impl<'de> DeserializeSeed<'de> for PermissionSeed {
    type Value = String;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_string(PermissionVisitor(self.0))
    }
}

struct PermissionVisitor(usize);

impl Visitor<'_> for PermissionVisitor {
    type Value = String;
    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "permission at index {} must be a string", self.0)
    }
    fn visit_string<E>(self, value: String) -> Result<String, E>
    where
        E: de::Error,
    {
        bounded_json_string(
            value,
            &format!("permission at index {}", self.0),
            MAX_APP_PERMISSION_BYTES,
        )
    }
    fn visit_str<E>(self, value: &str) -> Result<String, E>
    where
        E: de::Error,
    {
        self.visit_string(value.to_owned())
    }
}

fn collect_resources(fs: &dyn PlatformFs, root: PinnedDir) -> Result<Vec<AppResource>, PackError> {
    let mut out = Vec::new();
    let mut stack = vec![(root, Vec::<OsString>::new())];
    let mut total = 0usize;
    let mut examined = 0usize;
    let mut directory_count = 1usize;
    while let Some((directory, prefix)) = stack.pop() {
        let names = fs
            .read_dir(&directory, &mut examined)
            .map_err(|source| PackError::Io {
                operation: "enumerate bounded app directory",
                path: PathBuf::from("<app-root>"),
                source,
            })?;
        for name in names {
            let mut components = prefix.clone();
            components.push(name.clone());
            let relative = normalized_components(&components)?;
            let opened = fs
                .open_child(
                    &directory,
                    &name,
                    libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
                )
                .map_err(|source| {
                    if source.raw_os_error() == Some(libc::ELOOP) {
                        PackError::Symlink {
                            path: relative.clone(),
                        }
                    } else {
                        PackError::Io {
                            operation: "open app child without following links",
                            path: PathBuf::from(&relative),
                            source,
                        }
                    }
                })?;
            let metadata = fs.metadata(&opened).map_err(|source| PackError::Io {
                operation: "inspect opened app child",
                path: PathBuf::from(&relative),
                source,
            })?;
            if metadata.is_dir() {
                directory_count += 1;
                if directory_count > MAX_DIRECTORIES {
                    return Err(PackError::InvalidResourcePath {
                        path: "<directory-limit-exceeded>".into(),
                    });
                }
                stack.push((PinnedDir(opened), components));
            } else if metadata.is_file() {
                if prefix.is_empty() && name == OsStr::new("riot-app.json") {
                    continue;
                }
                if out.len() == MAX_BUNDLE_RESOURCES {
                    return Err(PackError::TooManyResources {
                        actual: out.len() + 1,
                        limit: MAX_BUNDLE_RESOURCES,
                    });
                }
                let content_type = content_type_for(&relative)?;
                let remaining = MAX_BUNDLE_TOTAL_BYTES.saturating_sub(total);
                let bytes = match read_opened_bounded_with(fs, opened, remaining, &relative) {
                    Err(PackError::TooLarge { actual, .. }) => {
                        return Err(PackError::TooLarge {
                            actual: total.saturating_add(actual),
                            limit: MAX_BUNDLE_TOTAL_BYTES,
                        })
                    }
                    other => other?,
                };
                total += bytes.len();
                out.push(AppResource {
                    path: relative,
                    content_type: content_type.into(),
                    bytes,
                });
            } else {
                return Err(PackError::NonRegularFile { path: relative });
            }
        }
    }
    Ok(out)
}

fn normalized_components(components: &[OsString]) -> Result<String, PackError> {
    let mut parts = Vec::new();
    for value in components {
        parts.push(
            value
                .to_str()
                .ok_or_else(|| PackError::InvalidResourcePath {
                    path: "<non-UTF-8>".into(),
                })?,
        );
    }
    if parts.len() > MAX_RESOURCE_PATH_COMPONENTS {
        return Err(PackError::InvalidResourcePath {
            path: "<too-deep>".into(),
        });
    }
    let normalized = parts.join("/");
    if normalized.is_empty()
        | (normalized.len() > MAX_RESOURCE_PATH_BYTES)
        | normalized.starts_with('/')
        | normalized.contains('\\')
        | normalized.split('/').any(|part| {
            part.is_empty()
                || part == "."
                || part == ".."
                || part.starts_with('.')
                || part.chars().any(char::is_control)
        })
    {
        return Err(PackError::InvalidResourcePath { path: normalized });
    }
    Ok(normalized)
}

fn read_opened_bounded_with(
    fs: &dyn PlatformFs,
    mut file: File,
    limit: usize,
    label: &str,
) -> Result<Vec<u8>, PackError> {
    let metadata = fs.metadata(&file).map_err(|source| PackError::Io {
        operation: "inspect opened resource",
        path: PathBuf::from(label),
        source,
    })?;
    if !metadata.is_file() {
        return Err(PackError::NonRegularFile { path: label.into() });
    }
    let actual = usize::try_from(metadata.len()).unwrap_or(usize::MAX);
    ensure_pack_size(actual, limit)?;
    let mut bytes = Vec::with_capacity(actual);
    fs.read_to_end(&mut file, (limit as u64).saturating_add(1), &mut bytes)
        .map_err(|source| PackError::Io {
            operation: "read opened resource",
            path: PathBuf::from(label),
            source,
        })?;
    ensure_pack_size(bytes.len(), limit)?;
    Ok(bytes)
}

fn ensure_pack_size(actual: usize, limit: usize) -> Result<(), PackError> {
    if actual > limit {
        Err(PackError::TooLarge { actual, limit })
    } else {
        Ok(())
    }
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
) -> Result<SignedWillowEntry, WillowError> {
    sign_at_with_fault(author, path, payload, timestamp, false)
}

fn sign_at_with_fault(
    author: &EvidenceAuthor,
    path: WillowPath,
    payload: &[u8],
    timestamp: u64,
    reject_authorisation: bool,
) -> Result<SignedWillowEntry, WillowError> {
    let entry = Entry::builder()
        .namespace_id(author.namespace_id().clone())
        .subspace_id(author.subspace_id())
        .path(path)
        .timestamp(timestamp)
        .payload(payload)
        .build();
    let authorised = if reject_authorisation {
        Err(WillowError::DoesNotAuthorise)
    } else {
        authorise_entry(author, entry)
    }?;
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
    inspect_with_fault(bytes, None)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InspectStage {
    AppId,
    OpenSession,
    CreateStore,
    InspectImport,
    RejectPreview,
    Commit,
    Scan,
    CompletePair,
    IndexedMatch,
    Provenance,
    UniqueResources,
}

fn inspect_with_fault(
    bytes: &[u8],
    fault: Option<InspectStage>,
) -> Result<InspectReport, InspectError> {
    if bytes.len() > MAX_BUNDLE_BYTES {
        return Err(InspectError::InvalidImportBundle {
            reason: "artifact exceeds the import size limit",
        });
    }
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
    let mut classified_kinds = 0_u8;
    for item in &decoded.items {
        let valid = match &item.status {
            ItemStatus::Valid(valid) => valid,
            ItemStatus::Invalid(_) => {
                return Err(InspectError::InvalidImportBundle {
                    reason: "an entry failed signature or schema verification",
                })
            }
        };
        let carrier = Carrier {
            namespace_id: *valid.entry.namespace_id().as_bytes(),
            subspace_id: *valid.entry.subspace_id().as_bytes(),
            timestamp_micros: u64::from(valid.entry.timestamp()),
        };
        let payload = item.frame.payload_bytes();
        match (decode_manifest(payload), decode_app_bundle(payload)) {
            (Ok(value), Err(_)) => {
                classified_kinds = classified_kinds.saturating_add(1);
                manifest = Some((value, carrier));
            }
            (Err(_), Ok(value)) => {
                classified_kinds = classified_kinds.saturating_add(2);
                bundle = Some((value, payload.to_vec(), carrier))
            }
            _ => {
                return Err(InspectError::IncoherentPair {
                    reason: "entries are not one manifest and one resource bundle",
                })
            }
        }
    }
    ensure_inspect(
        classified_kinds == 3,
        false,
        InspectError::IncoherentPair {
            reason: "entries are not one manifest and one resource bundle",
        },
    )?;
    let (manifest, manifest_carrier) = manifest.expect("two classified entries include a manifest");
    let (bundle, bundle_bytes, bundle_carrier) =
        bundle.expect("two classified entries include a resource bundle");
    if manifest.name.chars().any(char::is_control)
        | manifest.version.chars().any(char::is_control)
        | bundle
            .resources
            .iter()
            .any(|resource| resource.path.chars().any(char::is_control))
    {
        return Err(InspectError::IncoherentPair {
            reason: "display fields contain control characters",
        });
    }
    if (manifest_carrier != bundle_carrier)
        | (manifest.author.namespace_id != manifest_carrier.namespace_id)
        | (manifest.author.subspace_id != manifest_carrier.subspace_id)
        | (manifest.author.signing_key_id != manifest_carrier.subspace_id)
        | (manifest.author.namespace_kind != riot_core::willow::NamespaceKind::Communal)
    {
        return Err(InspectError::IncoherentPair {
            reason: "manifest author, entry carrier, or timestamps do not agree",
        });
    }
    if manifest.entry_point != bundle.entry_point {
        return Err(InspectError::IncoherentPair {
            reason: "manifest and bundle entry points differ",
        });
    }
    let app_id = inspect_result(
        app_id_for(&manifest, &app_bundle_digest(&bundle_bytes)),
        fault == Some(InspectStage::AppId),
        InspectError::IncoherentPair {
            reason: "canonical app ID cannot be derived",
        },
    )?;

    // Commit through the production import path, then use the app-index scanner
    // as the final path/payload/carrier coherence authority.
    let session = inspect_result(
        RiotSession::open(),
        fault == Some(InspectStage::OpenSession),
        InspectError::InvalidImportBundle {
            reason: "cannot open verification session",
        },
    )?;
    let store = inspect_result(
        session.create_store(),
        fault == Some(InspectStage::CreateStore),
        InspectError::InvalidImportBundle {
            reason: "cannot create verification store",
        },
    )?;
    let outcome = inspect_result(
        store.inspect(bytes, ImportContext::new("riot-app inspect")),
        fault == Some(InspectStage::InspectImport),
        InspectError::InvalidImportBundle {
            reason: "import inspection failed",
        },
    )?;
    let preview = inspect_preview(outcome, fault == Some(InspectStage::RejectPreview))?;
    inspect_result(
        preview.plan_all().and_then(|plan| plan.commit()),
        fault == Some(InspectStage::Commit),
        InspectError::InvalidImportBundle {
            reason: "verified pair could not be committed",
        },
    )?;
    let scanned = inspect_result(
        scan_app_index(&store),
        fault == Some(InspectStage::Scan),
        InspectError::IncoherentPair {
            reason: "app-index scan failed",
        },
    )?;
    ensure_inspect(
        scanned.pending_manifests.is_empty() && scanned.apps.len() == 1,
        fault == Some(InspectStage::CompletePair),
        InspectError::IncoherentPair {
            reason: "paths do not form one complete app-index pair",
        },
    )?;
    let indexed = &scanned.apps[0];
    ensure_inspect(
        (indexed.app_id == app_id) & (indexed.manifest == manifest) & indexed.bundle_present,
        fault == Some(InspectStage::IndexedMatch),
        InspectError::IncoherentPair {
            reason: "path, payload, and app ID do not agree",
        },
    )?;
    ensure_inspect(
        carried_by(&indexed.provenance) == Some(manifest.author.subspace_id),
        fault == Some(InspectStage::Provenance),
        InspectError::IncoherentPair {
            reason: "manifest author does not match the signing carrier",
        },
    )?;
    let mut resources = bundle
        .resources
        .into_iter()
        .map(|resource| resource.path)
        .collect::<Vec<_>>();
    resources.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
    ensure_inspect(
        resources.iter().collect::<BTreeSet<_>>().len() == resources.len(),
        fault == Some(InspectStage::UniqueResources),
        InspectError::IncoherentPair {
            reason: "resource paths are duplicated",
        },
    )?;
    Ok(InspectReport {
        name: manifest.name,
        version: manifest.version,
        author: manifest.author,
        app_id,
        resources,
    })
}

fn carried_by(provenance: &AppProvenance) -> Option<[u8; 32]> {
    match provenance {
        AppProvenance::Carried {
            carrier_subspace_id,
        } => Some(*carrier_subspace_id),
        AppProvenance::BuiltIn => None,
    }
}

fn inspect_result<T, E>(
    result: Result<T, E>,
    injected: bool,
    error: InspectError,
) -> Result<T, InspectError> {
    if injected {
        Err(error)
    } else {
        result.map_err(|_| error)
    }
}

fn inspect_preview(outcome: InspectOutcome, injected: bool) -> Result<ImportPreview, InspectError> {
    if injected {
        return Err(InspectError::InvalidImportBundle {
            reason: "production import inspection rejected it",
        });
    }
    match outcome {
        InspectOutcome::Preview(preview) => Ok(preview),
        InspectOutcome::Rejected(_) => Err(InspectError::InvalidImportBundle {
            reason: "production import inspection rejected it",
        }),
    }
}

fn ensure_inspect(
    condition: bool,
    injected: bool,
    error: InspectError,
) -> Result<(), InspectError> {
    if condition && !injected {
        Ok(())
    } else {
        Err(error)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Carrier {
    namespace_id: [u8; 32],
    subspace_id: [u8; 32],
    timestamp_micros: u64,
}

pub fn keygen(out: &Path) -> Result<KeygenOutput, KeyError> {
    keygen_inner(out, &mut |_| Ok(()))
}

#[derive(Clone, Copy)]
enum KeygenStage {
    AfterWrapKey,
    BeforePublish,
    BeforeParentSync,
    CleanupParentSync,
}

fn keygen_inner(
    out: &Path,
    checkpoint: &mut dyn FnMut(KeygenStage) -> Result<(), KeyError>,
) -> Result<KeygenOutput, KeyError> {
    keygen_inner_with(out, checkpoint, &UnixPlatformFs, &mut OsEntropyPort)
}

fn keygen_inner_with(
    out: &Path,
    checkpoint: &mut dyn FnMut(KeygenStage) -> Result<(), KeyError>,
    fs: &dyn PlatformFs,
    entropy: &mut dyn EntropyPort,
) -> Result<KeygenOutput, KeyError> {
    let final_name = out.file_name().ok_or(KeyError::InvalidOutputDirectory)?;
    CString::new(final_name.as_bytes()).map_err(|_| KeyError::InvalidOutputDirectory)?;
    let parent = match out.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let parent_dir = fs.open_path(parent).map_err(|source| KeyError::Io {
        operation: "open key output parent without following links",
        path: parent.to_path_buf(),
        source,
    })?;
    let stage_name = OsString::from(format!(".riot-app-keygen-{}", random_hex_16_with(entropy)?));
    if let Err(source) = fs.mkdir(&parent_dir, &stage_name, 0o700) {
        return Err(KeyError::Io {
            operation: "create private key staging directory",
            path: PathBuf::from(&stage_name),
            source,
        });
    }
    let stage = open_stage_or_cleanup(fs, &parent_dir, &stage_name, "open key staging directory")?;
    let mut published = false;
    let result = (|| {
        let author = entropy.generate_author()?;
        let identity = author.identity();
        let mut wrapping_key = Zeroizing::new([0u8; 32]);
        entropy.fill(&mut wrapping_key[..])?;
        let sealed = entropy.seal_identity(&author, &wrapping_key)?;
        let encoded_key = Zeroizing::new(hex_lower_bytes(&wrapping_key[..]));

        write_private_at_with(fs, &stage, OsStr::new("author.wrapkey"), &encoded_key)?;
        checkpoint(KeygenStage::AfterWrapKey)?;
        write_private_at_with(fs, &stage, OsStr::new("author.sealed"), &sealed)?;
        fs.fsync(&stage.0).map_err(|source| KeyError::Io {
            operation: "sync key staging directory",
            path: PathBuf::from(&stage_name),
            source,
        })?;
        fs.fsync(&parent_dir.0).map_err(|source| KeyError::Io {
            operation: "sync key output parent",
            path: parent.to_path_buf(),
            source,
        })?;
        checkpoint(KeygenStage::BeforePublish)?;
        fs.rename(&parent_dir, &stage_name, &parent_dir, final_name)
            .map_err(|source| {
                if source.kind() == io::ErrorKind::AlreadyExists
                    || matches!(
                        source.raw_os_error(),
                        Some(libc::EEXIST) | Some(libc::ENOTEMPTY)
                    )
                {
                    KeyError::AlreadyExists {
                        path: out.to_path_buf(),
                    }
                } else {
                    KeyError::Io {
                        operation: "publish key directory",
                        path: out.to_path_buf(),
                        source,
                    }
                }
            })?;
        published = true;
        let sync_result = checkpoint(KeygenStage::BeforeParentSync).and_then(|()| {
            fs.fsync(&parent_dir.0).map_err(|source| KeyError::Io {
                operation: "sync published key parent",
                path: parent.to_path_buf(),
                source,
            })
        });
        if let Err(error) = sync_result {
            unlinkat_checked_with(
                fs,
                &stage,
                OsStr::new("author.wrapkey"),
                0,
                "rollback wrapping key",
            )?;
            unlinkat_checked_with(
                fs,
                &stage,
                OsStr::new("author.sealed"),
                0,
                "rollback sealed identity",
            )?;
            fs.fsync(&stage.0).map_err(|cleanup| KeyError::Io {
                operation: "sync rolled-back key directory",
                path: out.to_path_buf(),
                source: cleanup,
            })?;
            unlinkat_checked_with(
                fs,
                &parent_dir,
                final_name,
                libc::AT_REMOVEDIR,
                "rollback published key directory",
            )?;
            fs.fsync(&parent_dir.0).map_err(|cleanup| KeyError::Io {
                operation: "sync key rollback parent",
                path: parent.to_path_buf(),
                source: cleanup,
            })?;
            return Err(error);
        }
        Ok(KeygenOutput {
            identity,
            warning: KEY_WARNING,
        })
    })();
    if !published {
        let cleanup = unlinkat_if_exists_with(
            fs,
            &stage,
            OsStr::new("author.wrapkey"),
            0,
            "clean wrapping key",
        )
        .and_then(|()| {
            unlinkat_if_exists_with(
                fs,
                &stage,
                OsStr::new("author.sealed"),
                0,
                "clean sealed identity",
            )
        });
        drop(stage);
        let cleanup = cleanup.and_then(|()| {
            unlinkat_checked_with(
                fs,
                &parent_dir,
                &stage_name,
                libc::AT_REMOVEDIR,
                "clean key staging directory",
            )
        });
        cleanup?;
        checkpoint(KeygenStage::CleanupParentSync)?;
        sync_cleanup_parent_with(
            fs,
            &parent_dir,
            parent,
            "sync key parent after staging cleanup",
        )?;
    }
    result
}

fn unlinkat_checked_with(
    fs: &dyn PlatformFs,
    directory: &PinnedDir,
    name: &OsStr,
    flags: libc::c_int,
    operation: &'static str,
) -> Result<(), KeyError> {
    fs.unlink(directory, name, flags)
        .map_err(|source| KeyError::Io {
            operation,
            path: PathBuf::from(name),
            source,
        })
}

fn unlinkat_if_exists_with(
    fs: &dyn PlatformFs,
    directory: &PinnedDir,
    name: &OsStr,
    flags: libc::c_int,
    operation: &'static str,
) -> Result<(), KeyError> {
    match unlinkat_checked_with(fs, directory, name, flags, operation) {
        Err(KeyError::Io { source, .. }) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

fn open_stage_or_cleanup(
    fs: &dyn PlatformFs,
    parent: &PinnedDir,
    name: &OsStr,
    operation: &'static str,
) -> Result<PinnedDir, KeyError> {
    match fs.open_child(
        parent,
        name,
        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
    ) {
        Ok(file) => Ok(PinnedDir(file)),
        Err(source) => {
            unlinkat_checked_with(
                fs,
                parent,
                name,
                libc::AT_REMOVEDIR,
                "clean unopened staging directory",
            )?;
            sync_cleanup_parent_with(
                fs,
                parent,
                Path::new("<staging-parent>"),
                "sync parent after unopened staging cleanup",
            )?;
            Err(KeyError::Io {
                operation,
                path: PathBuf::from(name),
                source,
            })
        }
    }
}

pub fn load_author(key_dir: &Path) -> Result<EvidenceAuthor, KeyError> {
    load_author_with(key_dir, &UnixPlatformFs)
}

fn load_author_with(key_dir: &Path, fs: &dyn PlatformFs) -> Result<EvidenceAuthor, KeyError> {
    let directory = fs.open_path(key_dir).map_err(|source| KeyError::Io {
        operation: "open key directory without following links",
        path: key_dir.to_path_buf(),
        source,
    })?;
    let encoded = read_private_exact_at_with(fs, &directory, OsStr::new("author.wrapkey"), 64)?;
    let wrapping_key = decode_lower_hex_key(&encoded)?;
    let sealed = read_private_exact_at_with(
        fs,
        &directory,
        OsStr::new("author.sealed"),
        riot_core::willow::SEALED_IDENTITY_BYTES,
    )?;
    EvidenceAuthor::open_sealed_identity(&wrapping_key, &sealed)
        .map_err(|_| KeyError::InvalidSealedIdentity)
}

pub fn write_new_atomic(path: &Path, bytes: &[u8]) -> Result<(), KeyError> {
    write_new_atomic_inner(path, bytes, &mut |_| Ok(()))
}

#[derive(Clone, Copy)]
enum OutputStage {
    ArtifactCreate,
    Publish,
    ParentSync,
    StageCleanup,
    CleanupParentSync,
}

fn write_new_atomic_inner(
    path: &Path,
    bytes: &[u8],
    before_publish: &mut dyn FnMut(OutputStage) -> Result<(), KeyError>,
) -> Result<(), KeyError> {
    write_new_atomic_inner_with(
        path,
        bytes,
        before_publish,
        &UnixPlatformFs,
        &mut OsEntropyPort,
    )
}

fn write_new_atomic_inner_with(
    path: &Path,
    bytes: &[u8],
    before_publish: &mut dyn FnMut(OutputStage) -> Result<(), KeyError>,
    fs: &dyn PlatformFs,
    entropy: &mut dyn EntropyPort,
) -> Result<(), KeyError> {
    let final_name = path.file_name().ok_or(KeyError::InvalidOutputDirectory)?;
    CString::new(final_name.as_bytes()).map_err(|_| KeyError::InvalidOutputDirectory)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent = fs.open_path(parent).map_err(|source| KeyError::Io {
        operation: "open output parent without following links",
        path: parent.to_path_buf(),
        source,
    })?;
    let stage_name = OsString::from(format!(".riot-app-output-{}", random_hex_16_with(entropy)?));
    if let Err(source) = fs.mkdir(&parent, &stage_name, 0o700) {
        return Err(KeyError::Io {
            operation: "create private output staging directory",
            path: PathBuf::from(&stage_name),
            source,
        });
    }
    let stage = open_stage_or_cleanup(fs, &parent, &stage_name, "open output staging directory")?;
    let artifact = OsStr::new("artifact");
    if let Err(error) = before_publish(OutputStage::ArtifactCreate) {
        drop(stage);
        unlinkat_checked_with(
            fs,
            &parent,
            &stage_name,
            libc::AT_REMOVEDIR,
            "clean artifact staging after injected create failure",
        )?;
        before_publish(OutputStage::CleanupParentSync)?;
        sync_cleanup_parent_with(
            fs,
            &parent,
            Path::new("<output-parent>"),
            "sync output parent after create cleanup",
        )?;
        return Err(error);
    }
    let mut file = match fs.create_child(
        &stage,
        artifact,
        libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        0o600,
    ) {
        Ok(file) => file,
        Err(source) => {
            drop(stage);
            unlinkat_checked_with(
                fs,
                &parent,
                &stage_name,
                libc::AT_REMOVEDIR,
                "clean artifact staging after create failure",
            )?;
            before_publish(OutputStage::CleanupParentSync)?;
            sync_cleanup_parent_with(
                fs,
                &parent,
                Path::new("<output-parent>"),
                "sync output parent after create cleanup",
            )?;
            return Err(KeyError::Io {
                operation: "create staged output",
                path: PathBuf::from(artifact),
                source,
            });
        }
    };
    let mut final_present = false;
    let result = (|| {
        fs.write_all(&mut file, bytes)
            .and_then(|()| fs.fsync(&file))
            .map_err(|source| KeyError::Io {
                operation: "write temporary file",
                path: PathBuf::from("<staged-output>"),
                source,
            })?;
        before_publish(OutputStage::Publish)?;
        fs.rename(&stage, artifact, &parent, final_name)
            .map_err(|source| {
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
        final_present = true;
        let sync_result = before_publish(OutputStage::ParentSync).and_then(|()| {
            fs.fsync(&parent.0).map_err(|source| KeyError::Io {
                operation: "sync output parent",
                path: PathBuf::from("<output-parent>"),
                source,
            })
        });
        if let Err(error) = sync_result {
            unlinkat_checked_with(fs, &parent, final_name, 0, "rollback published artifact")?;
            final_present = false;
            fs.fsync(&parent.0).map_err(|source| KeyError::Io {
                operation: "sync artifact rollback parent",
                path: PathBuf::from("<output-parent>"),
                source,
            })?;
            return Err(error);
        }
        Ok(())
    })();
    let cleanup = unlinkat_if_exists_with(fs, &stage, artifact, 0, "clean staged artifact");
    drop(stage);
    let injected_cleanup = if final_present {
        before_publish(OutputStage::StageCleanup).err()
    } else {
        None
    };
    let cleanup = cleanup.and_then(|()| {
        unlinkat_checked_with(
            fs,
            &parent,
            &stage_name,
            libc::AT_REMOVEDIR,
            "clean artifact staging directory",
        )
    });
    let mut cleanup_error = injected_cleanup.or_else(|| cleanup.err());
    if cleanup_error.is_none() {
        cleanup_error = before_publish(OutputStage::CleanupParentSync)
            .and_then(|()| {
                sync_cleanup_parent_with(
                    fs,
                    &parent,
                    Path::new("<output-parent>"),
                    "sync output parent after staging cleanup",
                )
            })
            .err();
    }
    if let Some(error) = cleanup_error {
        if final_present {
            unlinkat_checked_with(
                fs,
                &parent,
                final_name,
                0,
                "rollback artifact after staging cleanup failure",
            )?;
            fs.fsync(&parent.0).map_err(|source| KeyError::Io {
                operation: "sync cleanup rollback parent",
                path: PathBuf::from("<output-parent>"),
                source,
            })?;
        }
        return Err(error);
    }
    result
}

fn sync_cleanup_parent_with(
    fs: &dyn PlatformFs,
    directory: &PinnedDir,
    path: &Path,
    operation: &'static str,
) -> Result<(), KeyError> {
    fs.fsync(&directory.0).map_err(|source| KeyError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(target_os = "macos")]
fn rename_file_noreplace_at(
    from_dir: &PinnedDir,
    from: &OsStr,
    to_dir: &PinnedDir,
    to: &OsStr,
) -> io::Result<()> {
    let from = rename_component(from)?;
    let to = rename_component(to)?;
    let result = unsafe {
        libc::renameatx_np(
            from_dir.0.as_raw_fd(),
            from.as_ptr(),
            to_dir.0.as_raw_fd(),
            to.as_ptr(),
            libc::RENAME_EXCL,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(target_os = "linux")]
fn rename_file_noreplace_at(
    from_dir: &PinnedDir,
    from: &OsStr,
    to_dir: &PinnedDir,
    to: &OsStr,
) -> io::Result<()> {
    let from = rename_component(from)?;
    let to = rename_component(to)?;
    let result = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            from_dir.0.as_raw_fd(),
            from.as_ptr(),
            to_dir.0.as_raw_fd(),
            to.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

fn rename_component(name: &OsStr) -> io::Result<CString> {
    CString::new(name.as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NUL in rename component"))
}

fn random_hex_16_with(entropy: &mut dyn EntropyPort) -> Result<String, KeyError> {
    let mut bytes = Zeroizing::new([0u8; 16]);
    entropy.fill(&mut bytes[..])?;
    Ok(hex_lower(&bytes[..]))
}

fn write_private_at_with(
    fs: &dyn PlatformFs,
    directory: &PinnedDir,
    name: &OsStr,
    bytes: &[u8],
) -> Result<(), KeyError> {
    let mut file = fs
        .create_child(
            directory,
            name,
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )
        .map_err(|source| KeyError::Io {
            operation: "create private staged file",
            path: PathBuf::from(name),
            source,
        })?;
    fs.write_all(&mut file, bytes)
        .and_then(|()| fs.fsync(&file))
        .map_err(|source| KeyError::Io {
            operation: "write private staged file",
            path: PathBuf::from(name),
            source,
        })
}

fn read_private_exact_at_with(
    fs: &dyn PlatformFs,
    directory: &PinnedDir,
    name: &OsStr,
    expected: usize,
) -> Result<Zeroizing<Vec<u8>>, KeyError> {
    use std::os::unix::fs::PermissionsExt;
    let path = PathBuf::from(name);
    let mut file = fs
        .open_child(
            directory,
            name,
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
        )
        .map_err(|source| KeyError::Io {
            operation: "open private key beneath pinned directory",
            path: path.clone(),
            source,
        })?;
    let metadata = fs.metadata(&file).map_err(|source| KeyError::Io {
        operation: "inspect private key file",
        path: path.to_path_buf(),
        source,
    })?;
    if !metadata.is_file()
        | (metadata.permissions().mode() & 0o077 != 0)
        | (usize::try_from(metadata.len()).ok() != Some(expected))
    {
        return Err(if expected == 64 {
            KeyError::InvalidWrapKey
        } else {
            KeyError::InvalidSealedIdentity
        });
    }
    let mut bytes = Zeroizing::new(Vec::with_capacity(expected));
    fs.read_to_end(&mut file, (expected as u64) + 1, &mut bytes)
        .map_err(|source| KeyError::Io {
            operation: "read private key file",
            path: path.to_path_buf(),
            source,
        })?;
    if bytes.len() != expected {
        return Err(if expected == 64 {
            KeyError::InvalidWrapKey
        } else {
            KeyError::InvalidSealedIdentity
        });
    }
    Ok(bytes)
}

fn decode_lower_hex_key(input: &[u8]) -> Result<Zeroizing<[u8; 32]>, KeyError> {
    if (input.len() != 64)
        | !input
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(KeyError::InvalidWrapKey);
    }
    let mut output = Zeroizing::new([0u8; 32]);
    for (index, slot) in output.iter_mut().enumerate() {
        *slot = (hex_nibble(input[index * 2]).expect("validated hexadecimal") << 4)
            | hex_nibble(input[index * 2 + 1]).expect("validated hexadecimal");
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
#[path = "tests/unit.rs"]
mod tests;

#[cfg(test)]
extern crate self as riot_app_cli;

#[cfg(test)]
#[path = "../tests/cli_pack.rs"]
mod cli_pack_unit;
