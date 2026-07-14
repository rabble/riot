use super::{keygen_inner, KeyError, KeygenStage};
use std::cell::{Cell, RefCell};
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::os::fd::AsRawFd;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::Path;

use super::PlatformFs;

fn tempdir() -> tempfile::TempDir {
    tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap()
}

fn allow_keygen_stage(_: KeygenStage) -> Result<(), KeyError> {
    Ok(())
}

fn allow_output_stage(_: super::OutputStage) -> Result<(), KeyError> {
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FsOperation {
    OpenPath,
    OpenChild,
    CreateChild,
    ReadDir,
    Metadata,
    Read,
    Write,
    Fsync,
    Mkdir,
    Rename,
    Unlink,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CoreOperation {
    Pass,
    EncodeBundleTooLarge,
    EncodeBundleOther,
    EncodeManifest,
    AppId,
    ManifestPath,
    BundlePath,
    Sign(usize),
    EncodeImport,
}

struct ScriptedCore {
    failure: CoreOperation,
    sign_calls: Cell<usize>,
}

impl ScriptedCore {
    fn fail(failure: CoreOperation) -> Self {
        Self {
            failure,
            sign_calls: Cell::new(0),
        }
    }
}

impl super::PackCore for ScriptedCore {
    fn encode_app_bundle(
        &self,
        bundle: &riot_core::apps::bundle::AppBundle,
    ) -> Result<Vec<u8>, riot_core::apps::AppsError> {
        match self.failure {
            CoreOperation::EncodeBundleTooLarge => Err(riot_core::apps::AppsError::BundleTooLarge),
            CoreOperation::EncodeBundleOther => Err(riot_core::apps::AppsError::BundleFieldInvalid),
            _ => super::RiotPackCore.encode_app_bundle(bundle),
        }
    }

    fn encode_manifest(
        &self,
        manifest: &riot_core::apps::manifest::AppManifest,
    ) -> Result<Vec<u8>, riot_core::apps::AppsError> {
        if self.failure == CoreOperation::EncodeManifest {
            Err(riot_core::apps::AppsError::ManifestFieldInvalid)
        } else {
            super::RiotPackCore.encode_manifest(manifest)
        }
    }

    fn app_id_for(
        &self,
        manifest: &riot_core::apps::manifest::AppManifest,
        bundle_digest: &[u8; 32],
    ) -> Result<riot_core::apps::manifest::AppId, riot_core::apps::AppsError> {
        if self.failure == CoreOperation::AppId {
            Err(riot_core::apps::AppsError::ManifestFieldInvalid)
        } else {
            super::RiotPackCore.app_id_for(manifest, bundle_digest)
        }
    }

    fn manifest_path(
        &self,
        app_id: &riot_core::apps::manifest::AppId,
    ) -> Result<riot_core::willow::Path, riot_core::apps::AppsError> {
        if self.failure == CoreOperation::ManifestPath {
            Err(riot_core::apps::AppsError::PathInvalid)
        } else {
            super::RiotPackCore.manifest_path(app_id)
        }
    }

    fn bundle_path(
        &self,
        app_id: &riot_core::apps::manifest::AppId,
    ) -> Result<riot_core::willow::Path, riot_core::apps::AppsError> {
        if self.failure == CoreOperation::BundlePath {
            Err(riot_core::apps::AppsError::PathInvalid)
        } else {
            super::RiotPackCore.bundle_path(app_id)
        }
    }

    fn sign_at(
        &self,
        author: &riot_core::willow::EvidenceAuthor,
        path: riot_core::willow::Path,
        payload: &[u8],
        timestamp: u64,
    ) -> Result<riot_core::willow::SignedWillowEntry, riot_core::willow::WillowError> {
        let call = self.sign_calls.get();
        self.sign_calls.set(call + 1);
        if self.failure == CoreOperation::Sign(call) {
            Err(riot_core::willow::WillowError::DoesNotAuthorise)
        } else {
            super::RiotPackCore.sign_at(author, path, payload, timestamp)
        }
    }

    fn encode_import_bundle(
        &self,
        entries: &[riot_core::willow::SignedWillowEntry],
    ) -> Result<Vec<u8>, riot_core::import::BundleEncodeError> {
        if self.failure == CoreOperation::EncodeImport {
            Err(riot_core::import::BundleEncodeError::TooManyEntries)
        } else {
            super::RiotPackCore.encode_import_bundle(entries)
        }
    }
}

struct ScriptedFs {
    failure: RefCell<Option<(FsOperation, usize)>>,
    failures: RefCell<Vec<(FsOperation, usize)>>,
    short_read: Option<usize>,
    oversized_read: Option<usize>,
    calls: RefCell<Vec<FsOperation>>,
}

impl ScriptedFs {
    fn fail(operation: FsOperation, matching_call: usize) -> Self {
        Self {
            failure: RefCell::new(Some((operation, matching_call))),
            failures: RefCell::new(Vec::new()),
            short_read: None,
            oversized_read: None,
            calls: RefCell::new(Vec::new()),
        }
    }

    fn fail_many(failures: impl IntoIterator<Item = (FsOperation, usize)>) -> Self {
        Self {
            failure: RefCell::new(None),
            failures: RefCell::new(failures.into_iter().collect()),
            short_read: None,
            oversized_read: None,
            calls: RefCell::new(Vec::new()),
        }
    }

    fn short_read(matching_call: usize) -> Self {
        Self {
            failure: RefCell::new(None),
            failures: RefCell::new(Vec::new()),
            short_read: Some(matching_call),
            oversized_read: None,
            calls: RefCell::new(Vec::new()),
        }
    }

    fn oversized_read(matching_call: usize) -> Self {
        Self {
            failure: RefCell::new(None),
            failures: RefCell::new(Vec::new()),
            short_read: None,
            oversized_read: Some(matching_call),
            calls: RefCell::new(Vec::new()),
        }
    }

    fn before(&self, operation: FsOperation) -> std::io::Result<()> {
        let matching_call = self
            .calls
            .borrow()
            .iter()
            .filter(|called| **called == operation)
            .count();
        self.calls.borrow_mut().push(operation);
        let mut failure = self.failure.borrow_mut();
        if let Some((expected, remaining)) = failure.as_mut() {
            if *expected == operation {
                if *remaining == 0 {
                    *failure = None;
                    return Err(std::io::Error::other(format!(
                        "injected {operation:?} failure"
                    )));
                }
                *remaining -= 1;
            }
        }
        drop(failure);
        let scripted_index = self
            .failures
            .borrow()
            .iter()
            .position(|candidate| *candidate == (operation, matching_call));
        if let Some(index) = scripted_index {
            self.failures.borrow_mut().remove(index);
            return Err(std::io::Error::other(format!(
                "injected {operation:?} failure"
            )));
        }
        Ok(())
    }
}

impl super::PlatformFs for ScriptedFs {
    fn open_path(&self, path: &Path) -> std::io::Result<super::PinnedDir> {
        self.before(FsOperation::OpenPath)?;
        super::UnixPlatformFs.open_path(path)
    }

    fn open_child(
        &self,
        directory: &super::PinnedDir,
        name: &OsStr,
        flags: libc::c_int,
    ) -> std::io::Result<File> {
        self.before(FsOperation::OpenChild)?;
        super::UnixPlatformFs.open_child(directory, name, flags)
    }

    fn create_child(
        &self,
        directory: &super::PinnedDir,
        name: &OsStr,
        flags: libc::c_int,
        mode: libc::mode_t,
    ) -> std::io::Result<File> {
        self.before(FsOperation::CreateChild)?;
        super::UnixPlatformFs.create_child(directory, name, flags, mode)
    }

    fn read_dir(
        &self,
        directory: &super::PinnedDir,
        total: &mut usize,
    ) -> std::io::Result<Vec<OsString>> {
        self.before(FsOperation::ReadDir)?;
        super::UnixPlatformFs.read_dir(directory, total)
    }

    fn metadata(&self, file: &File) -> std::io::Result<std::fs::Metadata> {
        self.before(FsOperation::Metadata)?;
        super::UnixPlatformFs.metadata(file)
    }

    fn read_to_end(
        &self,
        file: &mut File,
        limit: u64,
        bytes: &mut Vec<u8>,
    ) -> std::io::Result<usize> {
        let matching_call = self
            .calls
            .borrow()
            .iter()
            .filter(|called| **called == FsOperation::Read)
            .count();
        self.before(FsOperation::Read)?;
        if self.oversized_read == Some(matching_call) {
            bytes.resize(limit as usize, 0);
            return Ok(bytes.len());
        }
        let limit = if self.short_read == Some(matching_call) {
            limit.saturating_sub(2)
        } else {
            limit
        };
        super::UnixPlatformFs.read_to_end(file, limit, bytes)
    }

    fn write_all(&self, file: &mut File, bytes: &[u8]) -> std::io::Result<()> {
        self.before(FsOperation::Write)?;
        super::UnixPlatformFs.write_all(file, bytes)
    }

    fn fsync(&self, file: &File) -> std::io::Result<()> {
        self.before(FsOperation::Fsync)?;
        super::UnixPlatformFs.fsync(file)
    }

    fn mkdir(
        &self,
        directory: &super::PinnedDir,
        name: &OsStr,
        mode: libc::mode_t,
    ) -> std::io::Result<()> {
        self.before(FsOperation::Mkdir)?;
        super::UnixPlatformFs.mkdir(directory, name, mode)
    }

    fn rename(
        &self,
        from_dir: &super::PinnedDir,
        from: &OsStr,
        to_dir: &super::PinnedDir,
        to: &OsStr,
    ) -> std::io::Result<()> {
        self.before(FsOperation::Rename)?;
        super::UnixPlatformFs.rename(from_dir, from, to_dir, to)
    }

    fn unlink(
        &self,
        directory: &super::PinnedDir,
        name: &OsStr,
        flags: libc::c_int,
    ) -> std::io::Result<()> {
        self.before(FsOperation::Unlink)?;
        super::UnixPlatformFs.unlink(directory, name, flags)
    }
}

struct ScriptedEntropy {
    fail_author: bool,
    fail_seal: bool,
    fail_fill: Option<usize>,
    fills: usize,
}

impl ScriptedEntropy {
    fn success() -> Self {
        Self {
            fail_author: false,
            fail_seal: false,
            fail_fill: None,
            fills: 0,
        }
    }
}

impl super::EntropyPort for ScriptedEntropy {
    fn generate_author(&mut self) -> Result<riot_core::willow::EvidenceAuthor, KeyError> {
        if self.fail_author {
            Err(KeyError::EntropyUnavailable)
        } else {
            riot_core::willow::generate_communal_author().map_err(|_| KeyError::EntropyUnavailable)
        }
    }

    fn fill(&mut self, bytes: &mut [u8]) -> Result<(), KeyError> {
        let call = self.fills;
        self.fills += 1;
        if self.fail_fill == Some(call) {
            Err(KeyError::EntropyUnavailable)
        } else {
            for (index, byte) in bytes.iter_mut().enumerate() {
                *byte = (call as u8).wrapping_add(index as u8).wrapping_add(1);
            }
            Ok(())
        }
    }

    fn seal_identity(
        &mut self,
        author: &riot_core::willow::EvidenceAuthor,
        wrapping_key: &[u8; 32],
    ) -> Result<Vec<u8>, KeyError> {
        if self.fail_seal {
            Err(KeyError::EntropyUnavailable)
        } else {
            author
                .seal_identity(wrapping_key)
                .map_err(|_| KeyError::EntropyUnavailable)
        }
    }
}

#[test]
fn private_parsers_and_descriptor_helpers_cover_success_and_failure_contracts() {
    assert_eq!(super::hex_nibble(b'0').unwrap(), 0);
    assert_eq!(super::hex_nibble(b'9').unwrap(), 9);
    assert_eq!(super::hex_nibble(b'a').unwrap(), 10);
    assert_eq!(super::hex_nibble(b'f').unwrap(), 15);
    assert!(super::hex_nibble(b'G').is_err());
    assert_eq!(
        &*super::decode_lower_hex_key(&[b'a'; 64]).unwrap(),
        &[0xaa; 32]
    );
    for invalid in [b"short".as_slice(), &[b'A'; 64], &[b'g'; 64]] {
        assert!(super::decode_lower_hex_key(invalid).is_err());
    }

    for (path, expected) in [
        ("index.html", "text/html"),
        ("app.js", "text/javascript"),
        ("app.css", "text/css"),
        ("data.json", "application/json"),
        ("pixel.png", "image/png"),
        ("icon.svg", "image/svg+xml"),
    ] {
        assert_eq!(super::content_type_for(path).unwrap(), expected);
    }
    assert!(super::content_type_for("README").is_err());

    assert_eq!(
        super::normalized_components(&[OsString::from("a"), OsString::from("b.js")]).unwrap(),
        "a/b.js"
    );
    for invalid in [
        vec![],
        vec![OsString::from(".")],
        vec![OsString::from("..")],
        vec![OsString::from(".hidden")],
        vec![OsString::from("bad\\name")],
        vec![OsString::from("bad\nname")],
        vec![OsString::from("/rooted")],
        vec![OsString::from(
            "x".repeat(super::MAX_RESOURCE_PATH_BYTES + 1),
        )],
        vec![OsString::from_vec(vec![0xff])],
    ] {
        assert!(super::normalized_components(&invalid).is_err());
    }

    let nul = OsString::from_vec(b"bad\0name".to_vec());
    assert_eq!(
        super::openat_file(-1, &nul, libc::O_RDONLY)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::InvalidInput
    );
    assert_eq!(
        super::createat_file(-1, &nul, libc::O_WRONLY, 0o600)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::InvalidInput
    );
    assert_eq!(
        super::rename_component(&nul).unwrap_err().kind(),
        std::io::ErrorKind::InvalidInput
    );
    assert_eq!(
        super::openat_file(-1, OsStr::new("child"), libc::O_RDONLY)
            .unwrap_err()
            .raw_os_error(),
        Some(libc::EBADF)
    );
    assert_eq!(
        super::createat_file(-1, OsStr::new("child"), libc::O_WRONLY, 0o600)
            .unwrap_err()
            .raw_os_error(),
        Some(libc::EBADF)
    );

    let valid_manifest = br#"{"name":"Demo","description":"Demo app","version":"1","entry_point":"index.html","permissions":[]}"#;
    assert!(super::parse_manifest_input(valid_manifest).is_ok());
    let mut trailing = valid_manifest.to_vec();
    trailing.extend_from_slice(b" trailing");
    assert!(super::parse_manifest_input(&trailing).is_err());
    assert!(super::parse_manifest_input(b"[]").is_err());
    assert!(super::parse_manifest_input(br#"{"name":"Demo","#).is_err());
    assert_eq!(
        format!(
            "{}",
            &super::ManifestInputVisitor as &dyn serde::de::Expected
        ),
        "a riot-app.json object"
    );
    assert!(
        super::finish_manifest_parse(Err(serde_json::Error::io(std::io::Error::other(
            "injected trailing input"
        ))))
        .is_err()
    );
    assert!(super::parse_manifest_input_with_end_fault(valid_manifest, true).is_err());

    let parent = tempdir();
    let regular = parent.path().join("regular");
    std::fs::write(&regular, b"not a directory").unwrap();
    let pinned = super::PinnedDir(File::open(regular).unwrap());
    let mut total = 0;
    assert_eq!(
        pinned.names(&mut total).unwrap_err().raw_os_error(),
        Some(libc::ENOTDIR)
    );

    let parent_dir = super::PinnedDir::open_path(parent.path()).unwrap();
    assert_eq!(
        super::rename_file_noreplace_at(&parent_dir, &nul, &parent_dir, OsStr::new("to"))
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::InvalidInput
    );
    assert_eq!(
        super::rename_file_noreplace_at(&parent_dir, OsStr::new("from"), &parent_dir, &nul)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::InvalidInput
    );
    std::fs::create_dir(parent.path().join("exists")).unwrap();
    assert_eq!(
        super::UnixPlatformFs
            .mkdir(&parent_dir, OsStr::new("exists"), 0o700)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::AlreadyExists
    );
    assert_eq!(
        super::UnixPlatformFs
            .mkdir(&parent_dir, OsStr::from_bytes(b"bad\0name"), 0o700)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::InvalidInput
    );
    assert_eq!(
        super::UnixPlatformFs
            .unlink(&parent_dir, OsStr::from_bytes(b"bad\0name"), 0)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::InvalidInput
    );
}

#[test]
fn pinned_directory_and_bounded_io_helpers_cover_descriptor_failures_and_limits() {
    let parent = tempdir();
    let nested = parent.path().join("nested");
    std::fs::create_dir(&nested).unwrap();
    let absolute = super::PinnedDir::open_path(&nested).unwrap();
    let relative = nested
        .strip_prefix(std::env::current_dir().unwrap())
        .unwrap();
    assert!(super::PinnedDir::open_path(relative).is_ok());
    assert!(matches!(
        super::PinnedDir::open_path(std::path::Path::new("../outside")),
        Err(error) if error.kind() == std::io::ErrorKind::InvalidInput
    ));
    assert!(super::PinnedDir::open_path_with_fault(&nested, true).is_err());

    let invalid = super::PinnedDir(File::open(&nested).unwrap());
    unsafe {
        libc::close(invalid.0.as_raw_fd());
    }
    let mut invalid_total = 0;
    assert_eq!(
        invalid
            .names(&mut invalid_total)
            .unwrap_err()
            .raw_os_error(),
        Some(libc::EBADF)
    );
    std::mem::forget(invalid);

    let mut dot: libc::dirent = unsafe { std::mem::zeroed() };
    dot.d_name[0] = b'.' as libc::c_char;
    let mut dotdot: libc::dirent = unsafe { std::mem::zeroed() };
    dotdot.d_name[0] = b'.' as libc::c_char;
    dotdot.d_name[1] = b'.' as libc::c_char;
    let mut beta: libc::dirent = unsafe { std::mem::zeroed() };
    for (slot, byte) in beta.d_name.iter_mut().zip(b"beta\0") {
        *slot = *byte as libc::c_char;
    }
    let mut alpha: libc::dirent = unsafe { std::mem::zeroed() };
    for (slot, byte) in alpha.d_name.iter_mut().zip(b"alpha\0") {
        *slot = *byte as libc::c_char;
    }
    let entries = [
        &mut dot as *mut _,
        &mut beta as *mut _,
        &mut dotdot as *mut _,
        &mut alpha as *mut _,
        std::ptr::null_mut(),
    ];
    let mut cursor = 0;
    let mut total = 0;
    let names = absolute
        .names_with(&mut total, &mut |_| {
            let entry = entries[cursor];
            cursor += 1;
            entry
        })
        .unwrap();
    assert_eq!(names, [OsString::from("alpha"), OsString::from("beta")]);
    assert_eq!(total, 2);

    let end_error = std::cell::Cell::new(0);
    let mut end = |_| {
        super::set_errno(end_error.get());
        std::ptr::null_mut()
    };
    let mut total = 0;
    assert!(absolute
        .names_with(&mut total, &mut end)
        .unwrap()
        .is_empty());
    end_error.set(libc::EIO);
    assert_eq!(
        absolute
            .names_with(&mut total, &mut end)
            .unwrap_err()
            .raw_os_error(),
        Some(libc::EIO)
    );

    let mut total = 0;
    assert_eq!(
        absolute
            .names_with(&mut total, &mut |_| {
                super::set_errno(libc::EIO);
                std::ptr::null_mut()
            })
            .unwrap_err()
            .raw_os_error(),
        Some(libc::EIO)
    );

    let mut single: libc::dirent = unsafe { std::mem::zeroed() };
    single.d_name[0] = b'x' as libc::c_char;
    let mut total = super::MAX_TOTAL_ENTRIES;
    assert_eq!(
        absolute
            .names_with(&mut total, &mut |_| &mut single)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::InvalidData
    );

    let write_only_path = nested.join("write-only");
    std::fs::write(&write_only_path, b"bytes").unwrap();
    let write_only = std::fs::OpenOptions::new()
        .write(true)
        .open(&write_only_path)
        .unwrap();
    assert!(
        super::read_opened_bounded_with(&super::UnixPlatformFs, write_only, 16, "write-only")
            .is_err()
    );

    let bounded_path = nested.join("bounded");
    std::fs::write(&bounded_path, b"x").unwrap();
    assert!(super::read_opened_bounded_with(
        &ScriptedFs::oversized_read(0),
        File::open(&bounded_path).unwrap(),
        1,
        "bounded",
    )
    .is_err());

    std::fs::write(nested.join("existing"), b"old").unwrap();
    assert!(matches!(
        super::write_private_at_with(
            &super::UnixPlatformFs,
            &absolute,
            OsStr::new("existing"),
            b"new"
        ),
        Err(KeyError::Io {
            operation: "create private staged file",
            ..
        })
    ));
}

#[test]
fn platform_fs_pack_and_private_read_failures_are_propagated_by_operation() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hello-app");
    let author = riot_core::willow::generate_communal_author().unwrap();
    for (operation, matching_call) in [
        (FsOperation::OpenPath, 0),
        (FsOperation::OpenChild, 0),
        (FsOperation::Metadata, 0),
        (FsOperation::Read, 0),
        (FsOperation::ReadDir, 0),
        (FsOperation::OpenChild, 1),
        (FsOperation::Metadata, 1),
        (FsOperation::Read, 1),
    ] {
        let fs = ScriptedFs::fail(operation, matching_call);
        let result = super::pack_with_fs(
            super::PackInput {
                app_dir: &fixture,
                author: &author,
                timestamp_micros: 1,
            },
            &fs,
        );
        assert!(result.is_err(), "{operation:?} call {matching_call}");
        assert!(fs.failure.borrow().is_none());
    }

    let temp = tempdir();
    let keys = temp.path().join("keys");
    super::keygen(&keys).unwrap();
    for (operation, matching_call) in [
        (FsOperation::OpenPath, 0),
        (FsOperation::OpenChild, 0),
        (FsOperation::Metadata, 0),
        (FsOperation::Read, 0),
        (FsOperation::OpenChild, 1),
        (FsOperation::Metadata, 1),
        (FsOperation::Read, 1),
    ] {
        let fs = ScriptedFs::fail(operation, matching_call);
        let result = super::load_author_with(&keys, &fs);
        assert!(result.is_err(), "{operation:?} should return an I/O error");
        assert!(fs.failure.borrow().is_none());
    }

    for matching_call in [0, 1] {
        let fs = ScriptedFs::short_read(matching_call);
        let result = super::load_author_with(&keys, &fs);
        assert!(result.is_err(), "short read {matching_call} was accepted");
    }

    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(
        keys.join("author.sealed"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert!(super::load_author_with(&keys, &super::UnixPlatformFs).is_err());
}

#[test]
fn pack_core_failures_are_mapped_without_bypassing_filesystem_validation() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hello-app");
    let author = riot_core::willow::generate_communal_author().unwrap();
    for operation in [
        CoreOperation::Pass,
        CoreOperation::EncodeBundleTooLarge,
        CoreOperation::EncodeBundleOther,
        CoreOperation::EncodeManifest,
        CoreOperation::AppId,
        CoreOperation::ManifestPath,
        CoreOperation::BundlePath,
        CoreOperation::Sign(0),
        CoreOperation::Sign(1),
        CoreOperation::EncodeImport,
    ] {
        let result = super::pack_with(
            super::PackInput {
                app_dir: &fixture,
                author: &author,
                timestamp_micros: 1,
            },
            &super::UnixPlatformFs,
            &ScriptedCore::fail(operation),
        );
        match operation {
            CoreOperation::Pass => assert!(result.is_ok()),
            CoreOperation::EncodeBundleTooLarge => {
                assert!(result.is_err())
            }
            _ => assert!(result.is_err()),
        }
    }
    let path = riot_core::apps::index::app_index_manifest_path(&[0; 32]).unwrap();
    assert!(super::sign_at_with_fault(&author, path, b"payload", 1, true).is_err());
}

#[test]
fn inspect_core_failures_use_the_same_verified_input_and_error_mapping() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hello-app");
    let author = riot_core::willow::generate_communal_author().unwrap();
    let artifact = super::pack(super::PackInput {
        app_dir: &fixture,
        author: &author,
        timestamp_micros: 1,
    })
    .unwrap()
    .import_bundle_bytes;

    for stage in [
        super::InspectStage::AppId,
        super::InspectStage::OpenSession,
        super::InspectStage::CreateStore,
        super::InspectStage::InspectImport,
        super::InspectStage::RejectPreview,
        super::InspectStage::Commit,
        super::InspectStage::Scan,
        super::InspectStage::CompletePair,
        super::InspectStage::IndexedMatch,
        super::InspectStage::Provenance,
        super::InspectStage::UniqueResources,
    ] {
        assert!(super::inspect_with_fault(&artifact, Some(stage)).is_err());
    }

    assert!(super::inspect_result::<(), _>(
        Err(()),
        false,
        super::InspectError::IncoherentPair {
            reason: "injected result",
        }
    )
    .is_err());
    assert!(super::ensure_inspect(
        false,
        false,
        super::InspectError::IncoherentPair {
            reason: "injected condition",
        }
    )
    .is_err());
    assert!(super::ensure_inspect(
        false,
        true,
        super::InspectError::IncoherentPair {
            reason: "injected condition and stage",
        },
    )
    .is_err());
    assert_eq!(super::carried_by(&super::AppProvenance::BuiltIn), None);
    assert_eq!(
        super::carried_by(&super::AppProvenance::Carried {
            carrier_subspace_id: [9; 32],
        }),
        Some([9; 32])
    );

    let session = riot_core::session::RiotSession::open().unwrap();
    let store = session.create_store().unwrap();
    let rejected = store
        .inspect(
            b"not an import",
            riot_core::session::ImportContext::new("test"),
        )
        .unwrap();
    assert!(super::inspect_preview(rejected, false).is_err());
}

#[test]
fn platform_fs_and_entropy_keygen_failures_cleanup_without_publication() {
    for (operation, matching_call) in [
        (FsOperation::OpenPath, 0),
        (FsOperation::Mkdir, 0),
        (FsOperation::OpenChild, 0),
        (FsOperation::CreateChild, 0),
        (FsOperation::Write, 0),
        (FsOperation::Fsync, 0),
        (FsOperation::Fsync, 1),
        (FsOperation::Fsync, 2),
        (FsOperation::Fsync, 3),
        (FsOperation::Rename, 0),
        (FsOperation::Fsync, 4),
    ] {
        let temp = tempdir();
        let out = temp.path().join("keys");
        let fs = ScriptedFs::fail(operation, matching_call);
        let mut entropy = ScriptedEntropy::success();
        let result = super::keygen_inner_with(&out, &mut allow_keygen_stage, &fs, &mut entropy);
        assert!(result.is_err(), "{operation:?} call {matching_call}");
        assert!(!out.exists(), "{operation:?} published output");
        assert!(fs.failure.borrow().is_none());
    }

    for (fail_author, fail_seal, fail_fill) in [
        (false, false, Some(0)),
        (true, false, None),
        (false, false, Some(1)),
        (false, true, None),
    ] {
        let temp = tempdir();
        let out = temp.path().join("keys");
        let mut entropy = ScriptedEntropy {
            fail_author,
            fail_seal,
            fail_fill,
            fills: 0,
        };
        let result = super::keygen_inner_with(
            &out,
            &mut allow_keygen_stage,
            &super::UnixPlatformFs,
            &mut entropy,
        );
        assert!(result.is_err());
        assert!(!out.exists());
    }

    let temp = tempdir();
    let out = temp.path().join("keys");
    let fs = ScriptedFs::fail(FsOperation::Unlink, 0);
    let mut entropy = ScriptedEntropy {
        fail_author: true,
        fail_seal: false,
        fail_fill: None,
        fills: 0,
    };
    assert!(super::keygen_inner_with(&out, &mut allow_keygen_stage, &fs, &mut entropy).is_err());
    assert!(!out.exists());

    for (failures, final_may_remain) in [
        (
            vec![(FsOperation::Fsync, 4), (FsOperation::Unlink, 0)],
            true,
        ),
        (
            vec![(FsOperation::Fsync, 4), (FsOperation::Unlink, 1)],
            true,
        ),
        (vec![(FsOperation::Fsync, 4), (FsOperation::Fsync, 5)], true),
        (
            vec![(FsOperation::Fsync, 4), (FsOperation::Unlink, 2)],
            true,
        ),
        (
            vec![(FsOperation::Fsync, 4), (FsOperation::Fsync, 6)],
            false,
        ),
        (
            vec![(FsOperation::OpenChild, 0), (FsOperation::Unlink, 0)],
            false,
        ),
        (
            vec![(FsOperation::OpenChild, 0), (FsOperation::Fsync, 0)],
            false,
        ),
        (
            vec![(FsOperation::Write, 0), (FsOperation::Fsync, 0)],
            false,
        ),
    ] {
        let temp = tempdir();
        let out = temp.path().join("keys");
        let fs = ScriptedFs::fail_many(failures);
        let mut entropy = ScriptedEntropy::success();
        assert!(
            super::keygen_inner_with(&out, &mut allow_keygen_stage, &fs, &mut entropy).is_err()
        );
        if !final_may_remain {
            assert!(!out.exists());
        }
        assert!(fs.failures.borrow().is_empty());
    }

    let mut entropy = ScriptedEntropy::success();
    let fs = ScriptedFs::fail(FsOperation::OpenPath, 0);
    assert!(super::keygen_inner_with(
        Path::new("relative-keys"),
        &mut allow_keygen_stage,
        &fs,
        &mut entropy
    )
    .is_err());
}

#[test]
fn platform_fs_and_entropy_atomic_output_failures_rollback() {
    for (operation, matching_call) in [
        (FsOperation::OpenPath, 0),
        (FsOperation::Mkdir, 0),
        (FsOperation::OpenChild, 0),
        (FsOperation::CreateChild, 0),
        (FsOperation::Write, 0),
        (FsOperation::Fsync, 0),
        (FsOperation::Rename, 0),
        (FsOperation::Fsync, 1),
        (FsOperation::Unlink, 0),
        (FsOperation::Fsync, 2),
    ] {
        let temp = tempdir();
        let out = temp.path().join("artifact.riot");
        let fs = ScriptedFs::fail(operation, matching_call);
        let mut entropy = ScriptedEntropy::success();
        let result = super::write_new_atomic_inner_with(
            &out,
            b"artifact",
            &mut allow_output_stage,
            &fs,
            &mut entropy,
        );
        assert!(result.is_err(), "{operation:?} call {matching_call}");
        assert!(!out.exists(), "{operation:?} left final output");
        assert!(fs.failure.borrow().is_none());
    }

    let temp = tempdir();
    let out = temp.path().join("artifact.riot");
    let mut entropy = ScriptedEntropy {
        fail_author: false,
        fail_seal: false,
        fail_fill: Some(0),
        fills: 0,
    };
    assert!(super::write_new_atomic_inner_with(
        &out,
        b"artifact",
        &mut allow_output_stage,
        &super::UnixPlatformFs,
        &mut entropy,
    )
    .is_err());
    assert!(!out.exists());

    for (stage, failures, final_may_remain) in [
        (
            super::OutputStage::ArtifactCreate,
            vec![(FsOperation::Unlink, 0)],
            false,
        ),
        (
            super::OutputStage::ArtifactCreate,
            vec![(FsOperation::Fsync, 0)],
            false,
        ),
        (
            super::OutputStage::ParentSync,
            vec![(FsOperation::Fsync, 1)],
            false,
        ),
        (
            super::OutputStage::StageCleanup,
            vec![(FsOperation::Unlink, 2)],
            true,
        ),
        (
            super::OutputStage::StageCleanup,
            vec![(FsOperation::Fsync, 2)],
            false,
        ),
    ] {
        let temp = tempdir();
        let out = temp.path().join("artifact.riot");
        let fs = ScriptedFs::fail_many(failures);
        let mut entropy = ScriptedEntropy::success();
        let result = super::write_new_atomic_inner_with(
            &out,
            b"artifact",
            &mut |current| {
                if std::mem::discriminant(&current) == std::mem::discriminant(&stage) {
                    Err(KeyError::InvalidOutputDirectory)
                } else {
                    Ok(())
                }
            },
            &fs,
            &mut entropy,
        );
        assert!(result.is_err());
        if !final_may_remain {
            assert!(!out.exists());
        }
        assert!(fs.failures.borrow().is_empty());
    }

    for failures in [
        vec![(FsOperation::CreateChild, 0), (FsOperation::Unlink, 0)],
        vec![(FsOperation::CreateChild, 0), (FsOperation::Fsync, 0)],
    ] {
        let temp = tempdir();
        let out = temp.path().join("artifact.riot");
        let fs = ScriptedFs::fail_many(failures);
        let mut entropy = ScriptedEntropy::success();
        assert!(super::write_new_atomic_inner_with(
            &out,
            b"artifact",
            &mut allow_output_stage,
            &fs,
            &mut entropy,
        )
        .is_err());
        assert!(!out.exists());
        assert!(fs.failures.borrow().is_empty());
    }

    for (fs, fail_artifact_create) in [
        (ScriptedFs::fail_many([]), true),
        (ScriptedFs::fail(FsOperation::CreateChild, 0), false),
    ] {
        let temp = tempdir();
        let out = temp.path().join("artifact.riot");
        let mut entropy = ScriptedEntropy::success();
        let result = super::write_new_atomic_inner_with(
            &out,
            b"artifact",
            &mut |stage| {
                if (fail_artifact_create && matches!(stage, super::OutputStage::ArtifactCreate))
                    || matches!(stage, super::OutputStage::CleanupParentSync)
                {
                    Err(KeyError::InvalidOutputDirectory)
                } else {
                    Ok(())
                }
            },
            &fs,
            &mut entropy,
        );
        assert!(result.is_err());
        assert!(!out.exists());
    }

    let temp = tempdir();
    let out = temp.path().join("artifact.riot");
    let fs = ScriptedFs::fail(FsOperation::Unlink, 0);
    let mut entropy = ScriptedEntropy::success();
    assert!(super::write_new_atomic_inner_with(
        &out,
        b"artifact",
        &mut |stage| {
            if matches!(stage, super::OutputStage::ParentSync) {
                Err(KeyError::InvalidOutputDirectory)
            } else {
                Ok(())
            }
        },
        &fs,
        &mut entropy,
    )
    .is_err());
}

#[test]
fn keygen_failure_after_first_temp_file_leaves_no_final_or_temp_directory() {
    let parent = tempdir();
    let output = parent.path().join("keys");
    let error = keygen_inner(&output, &mut |stage| match stage {
        KeygenStage::AfterWrapKey => Err(KeyError::InvalidOutputDirectory),
        KeygenStage::BeforePublish
        | KeygenStage::BeforeParentSync
        | KeygenStage::CleanupParentSync => Ok(()),
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

#[test]
fn keygen_cleanup_parent_sync_failure_is_surfaced_after_removal() {
    let parent = tempdir();
    let output = parent.path().join("keys");
    let cleanup_error_path = parent.path().join("cleanup-sync");
    let mut checkpoint = |stage| match stage {
        KeygenStage::AfterWrapKey => Err(KeyError::InvalidOutputDirectory),
        KeygenStage::CleanupParentSync => Err(KeyError::AlreadyExists {
            path: cleanup_error_path.clone(),
        }),
        KeygenStage::BeforePublish | KeygenStage::BeforeParentSync => Ok(()),
    };
    assert!(checkpoint(KeygenStage::BeforePublish).is_ok());
    assert!(checkpoint(KeygenStage::BeforeParentSync).is_ok());
    let error = keygen_inner(&output, &mut checkpoint);
    assert!(matches!(
        error,
        Err(KeyError::AlreadyExists { path }) if path == cleanup_error_path
    ));
    assert!(!output.exists());
    assert_eq!(std::fs::read_dir(parent.path()).unwrap().count(), 0);
}

#[test]
fn embedded_nul_output_names_do_not_panic_or_leave_staging() {
    let parent = tempdir();
    let hostile = parent.path().join(std::ffi::OsString::from_vec(
        b"hostile\0bundle.riot".to_vec(),
    ));

    let output_result = std::panic::catch_unwind(|| super::write_new_atomic(&hostile, b"ours"));
    assert!(output_result.unwrap().is_err());

    let key_result = std::panic::catch_unwind(|| super::keygen(&hostile));
    assert!(key_result.unwrap().is_err());
    assert_eq!(std::fs::read_dir(parent.path()).unwrap().count(), 0);
    assert!(super::write_new_atomic(Path::new("/"), b"artifact").is_err());
    assert!(super::keygen(Path::new("/")).is_err());
}

#[test]
fn public_atomic_write_publishes_complete_bytes() {
    let parent = tempdir();
    let output = parent.path().join("artifact.riot");
    super::write_new_atomic(&output, b"complete artifact").unwrap();
    assert_eq!(std::fs::read(output).unwrap(), b"complete artifact");
}

#[test]
fn keygen_competing_destination_is_preserved_and_temp_is_cleaned() {
    let parent = tempdir();
    let output = parent.path().join("keys");
    let error = keygen_inner(&output, &mut |stage| {
        if matches!(stage, KeygenStage::BeforePublish) {
            std::fs::create_dir(&output).expect("attacker directory");
            std::fs::write(output.join("marker"), b"attacker").expect("marker");
        }
        Ok(())
    });
    assert!(error.is_err());
    assert_eq!(std::fs::read(output.join("marker")).unwrap(), b"attacker");
    assert_eq!(std::fs::read_dir(parent.path()).unwrap().count(), 1);
}

#[test]
fn keygen_parent_sync_failure_removes_files_and_final_directory() {
    let parent = tempdir();
    let output = parent.path().join("keys");
    let before_publish = keygen_inner(&output, &mut |stage| {
        if matches!(stage, KeygenStage::BeforePublish) {
            Err(KeyError::InvalidOutputDirectory)
        } else {
            Ok(())
        }
    });
    assert!(before_publish.is_err());
    assert!(!output.exists());
    let error = keygen_inner(&output, &mut |stage| {
        if matches!(stage, KeygenStage::BeforeParentSync) {
            return Err(KeyError::InvalidOutputDirectory);
        }
        Ok(())
    });
    assert!(error.is_err());
    assert!(!output.exists());
    assert_eq!(std::fs::read_dir(parent.path()).unwrap().count(), 0);
}

#[test]
fn output_competing_destination_is_preserved_and_temp_is_cleaned() {
    let parent = tempdir();
    let output = parent.path().join("bundle.riot");
    let error = super::write_new_atomic_inner(&output, b"ours", &mut |stage| {
        if matches!(stage, super::OutputStage::Publish) {
            std::fs::write(&output, b"attacker").unwrap();
        }
        Ok(())
    });
    assert!(error.is_err(), "{error:?}");
    assert_eq!(std::fs::read(&output).unwrap(), b"attacker");
    assert_eq!(std::fs::read_dir(parent.path()).unwrap().count(), 1);
}

#[test]
fn output_parent_sync_failure_removes_published_file() {
    let parent = tempdir();
    let output = parent.path().join("bundle.riot");
    let error = super::write_new_atomic_inner(&output, b"ours", &mut |stage| {
        if matches!(stage, super::OutputStage::ParentSync) {
            return Err(KeyError::InvalidOutputDirectory);
        }
        Ok(())
    });
    assert!(error.is_err());
    assert!(!output.exists());
    assert_eq!(std::fs::read_dir(parent.path()).unwrap().count(), 0);
}

#[test]
fn output_create_failure_removes_empty_staging_directory() {
    let parent = tempdir();
    let output = parent.path().join("bundle.riot");
    let error = super::write_new_atomic_inner(&output, b"ours", &mut |stage| {
        if matches!(stage, super::OutputStage::ArtifactCreate) {
            return Err(KeyError::InvalidOutputDirectory);
        }
        Ok(())
    });
    assert!(error.is_err());
    assert!(!output.exists());
    assert_eq!(std::fs::read_dir(parent.path()).unwrap().count(), 0);
}

#[test]
fn output_cleanup_parent_sync_failure_is_surfaced_after_removal() {
    let parent = tempdir();
    let output = parent.path().join("bundle.riot");
    let cleanup_error_path = parent.path().join("cleanup-sync");
    let error = super::write_new_atomic_inner(&output, b"ours", &mut |stage| match stage {
        super::OutputStage::Publish => Err(KeyError::InvalidOutputDirectory),
        super::OutputStage::CleanupParentSync => Err(KeyError::AlreadyExists {
            path: cleanup_error_path.clone(),
        }),
        _ => Ok(()),
    });
    assert!(matches!(
        error,
        Err(KeyError::AlreadyExists { path }) if path == cleanup_error_path
    ));
    assert!(!output.exists());
    assert_eq!(std::fs::read_dir(parent.path()).unwrap().count(), 0);
}

#[test]
fn output_stage_cleanup_failure_rolls_back_final() {
    let parent = tempdir();
    let output = parent.path().join("bundle.riot");
    let error = super::write_new_atomic_inner(&output, b"ours", &mut |stage| {
        if matches!(stage, super::OutputStage::StageCleanup) {
            return Err(KeyError::InvalidOutputDirectory);
        }
        Ok(())
    });
    assert!(error.is_err());
    assert!(!output.exists());
    assert_eq!(std::fs::read_dir(parent.path()).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn descriptor_relative_read_rejects_ancestor_swapped_to_outside_symlink() {
    use std::os::unix::fs::symlink;
    let parent = tempdir();
    let root = parent.path().join("root");
    let outside = parent.path().join("outside");
    std::fs::create_dir_all(root.join("nested")).unwrap();
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(root.join("nested/file.js"), b"inside").unwrap();
    std::fs::write(outside.join("file.js"), b"outside-secret").unwrap();
    let pinned = super::PinnedDir::open_path(&root).unwrap();
    let moved_root = parent.path().join("moved-root");
    std::fs::rename(&root, &moved_root).unwrap();
    symlink(&outside, &root).unwrap();
    let resources = super::collect_resources(&super::UnixPlatformFs, pinned).unwrap();
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].bytes, b"inside");
}

#[test]
fn directory_enumeration_and_directory_count_are_bounded() {
    let parent = tempdir();
    let empty = parent.path().join("empty");
    std::fs::create_dir(&empty).unwrap();
    let pinned = super::PinnedDir::open_path(&empty).unwrap();
    let duplicate = pinned.duplicate_cloexec().unwrap();
    assert_ne!(
        unsafe { libc::fcntl(duplicate, libc::F_GETFD) } & libc::FD_CLOEXEC,
        0
    );
    unsafe {
        libc::close(duplicate);
    }
    let mut count = 0;
    assert!(pinned.names(&mut count).unwrap().is_empty());
    let mut count = 0;
    let error = pinned.names_with(&mut count, &mut |_| {
        super::set_errno(libc::EIO);
        std::ptr::null_mut()
    });
    assert_eq!(error.unwrap_err().raw_os_error(), Some(libc::EIO));

    let many_entries = parent.path().join("many-entries");
    std::fs::create_dir(&many_entries).unwrap();
    for index in 0..=super::MAX_TOTAL_ENTRIES {
        std::fs::create_dir(many_entries.join(format!("entry-{index:04}"))).unwrap();
    }
    let pinned = super::PinnedDir::open_path(&many_entries).unwrap();
    let mut total = 0;
    assert!(pinned.names(&mut total).is_err());

    let split_a = parent.path().join("split-a");
    let split_b = parent.path().join("split-b");
    std::fs::create_dir(&split_a).unwrap();
    std::fs::create_dir(&split_b).unwrap();
    for index in 0..(super::MAX_TOTAL_ENTRIES / 2) {
        std::fs::write(split_a.join(format!("entry-{index:04}")), b"").unwrap();
    }
    for index in 0..=(super::MAX_TOTAL_ENTRIES / 2) {
        std::fs::write(split_b.join(format!("entry-{index:04}")), b"").unwrap();
    }
    let mut total = 0;
    super::PinnedDir::open_path(&split_a)
        .unwrap()
        .names(&mut total)
        .unwrap();
    assert!(super::PinnedDir::open_path(&split_b)
        .unwrap()
        .names(&mut total)
        .is_err());

    let many_dirs = parent.path().join("many-dirs");
    std::fs::create_dir(&many_dirs).unwrap();
    for index in 0..super::MAX_DIRECTORIES {
        std::fs::create_dir(many_dirs.join(format!("dir-{index:03}"))).unwrap();
    }
    let pinned = super::PinnedDir::open_path(&many_dirs).unwrap();
    assert!(super::collect_resources(&super::UnixPlatformFs, pinned).is_err());
}
