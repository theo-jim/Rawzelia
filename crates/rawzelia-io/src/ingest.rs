use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{CString, OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::mem::MaybeUninit;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rawzelia_domain::{AssetId, CaptureDate, Fingerprint};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::hash::fingerprint_open_file;
use crate::{IoError, PathTemplate, destination_error, source_error};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportRequest {
    pub source_root: PathBuf,
    pub selected_files: Vec<PathBuf>,
    pub destination_root: PathBuf,
    pub template: PathTemplate,
    pub session_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedIngest {
    pub asset_id: AssetId,
    pub source: PathBuf,
    pub destination: PathBuf,
    pub fingerprint: Fingerprint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestPlan {
    pub items: Vec<PlannedIngest>,
    pub destination_root: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportProgressState {
    Pending,
    Copying,
    Verified,
    Duplicate,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportProgress {
    pub completed: u32,
    pub total: u32,
    pub current: Option<PathBuf>,
    pub state: ImportProgressState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportSummary {
    pub imported: u32,
    pub duplicates: u32,
    pub failed: u32,
    pub resumed: bool,
}

pub fn plan_ingest(request: &ImportRequest) -> Result<IngestPlan, IoError> {
    let source_root_input =
        absolute_without_parent(&request.source_root).ok_or(IoError::SourceMissing)?;
    let source_root = require_source_root(&request.source_root)?;
    require_destination_outside_source(&request.destination_root, &source_root)?;

    let mut selected = BTreeMap::new();
    for requested_path in &request.selected_files {
        let candidate = if requested_path.is_absolute() {
            requested_path.clone()
        } else {
            request.source_root.join(requested_path)
        };
        let candidate = absolute_without_parent(&candidate).ok_or(IoError::SourceMissing)?;
        if !candidate.starts_with(&source_root_input) {
            return Err(IoError::SourceMissing);
        }
        require_no_source_symlink(&source_root_input, &candidate)?;
        let canonical = fs::canonicalize(&candidate).map_err(source_error)?;
        if !canonical.starts_with(&source_root) {
            return Err(IoError::SourceMissing);
        }
        let metadata = fs::symlink_metadata(&canonical).map_err(source_error)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(IoError::SourceMissing);
        }
        selected.insert(canonical, metadata);
    }

    let mut destinations = BTreeSet::new();
    let mut items = Vec::with_capacity(selected.len());
    for (source, metadata) in selected {
        let original_name = source
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or_else(|| IoError::Io("source name is not valid UTF-8".to_owned()))?;
        let capture_date = capture_date(metadata.modified().map_err(source_error)?)?;
        let relative_destination =
            request
                .template
                .render(capture_date, &request.session_name, original_name)?;
        let destination = request.destination_root.join(relative_destination);
        if !destinations.insert(destination.clone()) {
            return Err(IoError::DestinationUnavailable);
        }
        let fingerprint = crate::compute_fingerprint(&source)?;
        items.push(PlannedIngest {
            asset_id: asset_id(fingerprint),
            source,
            destination,
            fingerprint,
        });
    }
    require_effective_destination_uniqueness(&request.destination_root, &items)?;

    Ok(IngestPlan {
        items,
        destination_root: request.destination_root.clone(),
    })
}

fn require_effective_destination_uniqueness(
    destination_root: &Path,
    items: &[PlannedIngest],
) -> Result<(), IoError> {
    let mut groups = BTreeMap::<PathBuf, Vec<OsString>>::new();
    for item in items {
        let relative = item
            .destination
            .strip_prefix(destination_root)
            .map_err(|_| IoError::DestinationUnavailable)?;
        let parent = relative.parent().unwrap_or_else(|| Path::new(""));
        let name = relative
            .file_name()
            .ok_or(IoError::DestinationUnavailable)?;
        groups
            .entry(parent.to_path_buf())
            .or_default()
            .push(name.to_os_string());
    }
    if !groups.values().any(|names| names.len() > 1) {
        return Ok(());
    }

    let destination_volume = open_existing_destination_ancestor(destination_root)?;
    for names in groups.values().filter(|names| names.len() > 1) {
        if names_collide_on_volume(&destination_volume, names)? {
            return Err(IoError::DestinationUnavailable);
        }
    }
    Ok(())
}

fn names_collide_on_volume(directory: &OwnedFd, names: &[OsString]) -> Result<bool, IoError> {
    let (probe_name, probe_directory, probe_identity) = create_probe_directory(directory)?;
    let mut created = Vec::new();
    let result = (|| {
        for name in names {
            let name = c_string(name).map_err(|_| IoError::DestinationUnavailable)?;
            // SAFETY: descriptor and NUL-terminated name remain valid for the call.
            let descriptor = unsafe {
                libc::openat(
                    probe_directory.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_WRONLY
                        | libc::O_CREAT
                        | libc::O_EXCL
                        | libc::O_NOFOLLOW
                        | libc::O_CLOEXEC,
                    0o600,
                )
            };
            if descriptor < 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    return Ok(true);
                }
                return Err(destination_error(error));
            }
            // SAFETY: descriptor is checked above and transferred to one File.
            let file = unsafe { File::from_raw_fd(descriptor) };
            let identity = descriptor_identity(file.as_raw_fd()).map_err(destination_error)?;
            created.push((name, identity));
        }
        Ok(false)
    })();

    for (name, identity) in &created {
        cleanup_temporary(&probe_directory, name, *identity);
    }
    drop(probe_directory);
    cleanup_probe_directory(directory, &probe_name, probe_identity)?;
    result
}

fn create_probe_directory(
    directory: &OwnedFd,
) -> Result<(CString, OwnedFd, FileIdentity), IoError> {
    loop {
        let name = CString::new(format!(".rawzelia-name-probe-{}", random_token()?))
            .map_err(|_| IoError::DestinationUnavailable)?;
        // SAFETY: descriptor and NUL-terminated random name remain valid for the call.
        let result = unsafe { libc::mkdirat(directory.as_raw_fd(), name.as_ptr(), 0o700) };
        if result != 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                continue;
            }
            return Err(destination_error(error));
        }
        // SAFETY: descriptor and created directory name remain valid for the call.
        let descriptor = unsafe {
            libc::openat(
                directory.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if descriptor < 0 {
            return Err(destination_error(std::io::Error::last_os_error()));
        }
        // SAFETY: descriptor is checked above and transferred to one OwnedFd.
        let owned = unsafe { OwnedFd::from_raw_fd(descriptor) };
        let identity = descriptor_identity(owned.as_raw_fd()).map_err(destination_error)?;
        return Ok((name, owned, identity));
    }
}

fn cleanup_probe_directory(
    directory: &OwnedFd,
    name: &CString,
    owned: FileIdentity,
) -> Result<(), IoError> {
    if path_identity(directory, name)? != Some(owned) {
        return Err(IoError::DestinationUnavailable);
    }
    // SAFETY: identity was verified without following links; AT_REMOVEDIR never removes files.
    let result =
        unsafe { libc::unlinkat(directory.as_raw_fd(), name.as_ptr(), libc::AT_REMOVEDIR) };
    if result == 0 {
        Ok(())
    } else {
        Err(destination_error(std::io::Error::last_os_error()))
    }
}

fn open_existing_destination_ancestor(path: &Path) -> Result<OwnedFd, IoError> {
    let mut existing = resolved_destination_path(path)?;
    loop {
        match fs::symlink_metadata(&existing) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
                return open_directory(&existing);
            }
            Ok(_) => return Err(IoError::DestinationUnavailable),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                existing = existing
                    .parent()
                    .ok_or(IoError::DestinationUnavailable)?
                    .to_path_buf();
            }
            Err(error) => return Err(destination_error(error)),
        }
    }
}

pub fn execute_ingest(
    plan: &IngestPlan,
    progress: &mut dyn FnMut(ImportProgress),
    should_cancel: &dyn Fn() -> bool,
) -> Result<ImportSummary, IoError> {
    let total = u32::try_from(plan.items.len())
        .map_err(|_| IoError::Io("ingest item count overflow".to_owned()))?;
    progress(ImportProgress {
        completed: 0,
        total,
        current: None,
        state: ImportProgressState::Pending,
    });

    let destination_root = open_destination_root(&plan.destination_root)?;
    let mut summary = ImportSummary {
        imported: 0,
        duplicates: 0,
        failed: 0,
        resumed: false,
    };
    let mut completed = 0_u32;

    for item in &plan.items {
        if should_cancel() {
            return Err(IoError::Cancelled);
        }
        progress(ImportProgress {
            completed,
            total,
            current: Some(item.source.clone()),
            state: ImportProgressState::Copying,
        });

        match ingest_one(
            &destination_root,
            &plan.destination_root,
            item,
            should_cancel,
        ) {
            Ok(ItemOutcome::Imported) => {
                summary.imported += 1;
                completed += 1;
                progress(ImportProgress {
                    completed,
                    total,
                    current: Some(item.source.clone()),
                    state: ImportProgressState::Verified,
                });
            }
            Ok(ItemOutcome::Duplicate) => {
                summary.duplicates += 1;
                summary.resumed = true;
                completed += 1;
                progress(ImportProgress {
                    completed,
                    total,
                    current: Some(item.source.clone()),
                    state: ImportProgressState::Duplicate,
                });
            }
            Err(IoError::Cancelled) => return Err(IoError::Cancelled),
            Err(error) => {
                progress(ImportProgress {
                    completed,
                    total,
                    current: Some(item.source.clone()),
                    state: ImportProgressState::Failed,
                });
                return Err(error);
            }
        }
    }

    Ok(summary)
}

enum ItemOutcome {
    Imported,
    Duplicate,
}

fn ingest_one(
    destination_root: &OwnedFd,
    destination_root_path: &Path,
    item: &PlannedIngest,
    should_cancel: &dyn Fn() -> bool,
) -> Result<ItemOutcome, IoError> {
    let relative = item
        .destination
        .strip_prefix(destination_root_path)
        .map_err(|_| IoError::DestinationUnavailable)?;
    let file_name = relative
        .file_name()
        .ok_or(IoError::DestinationUnavailable)?;
    let parent = relative.parent().unwrap_or_else(|| Path::new(""));
    let parent_directory = open_or_create_relative_directory(destination_root, parent)?;

    if let Some(outcome) = existing_destination(&parent_directory, file_name, item.fingerprint)? {
        return Ok(outcome);
    }

    let (temporary_name, mut temporary, temporary_identity) = create_temporary(&parent_directory)?;
    let result = (|| {
        let mut source = open_source(&item.source)?;
        let copied = copy_and_hash(&mut source, &mut temporary, should_cancel)?;
        if copied != item.fingerprint {
            return Err(IoError::HashMismatch);
        }
        temporary.flush().map_err(destination_error)?;
        temporary.sync_all().map_err(destination_error)?;
        if path_identity(&parent_directory, &temporary_name)? != Some(temporary_identity) {
            return Err(IoError::DestinationUnavailable);
        }
        drop(temporary);
        if should_cancel() {
            return Err(IoError::Cancelled);
        }

        match rename_exclusive(&parent_directory, &temporary_name, file_name) {
            Ok(()) => {
                sync_directory(&parent_directory)?;
                Ok(ItemOutcome::Imported)
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                if matches!(
                    existing_destination(&parent_directory, file_name, item.fingerprint)?,
                    Some(ItemOutcome::Duplicate)
                ) {
                    Ok(ItemOutcome::Duplicate)
                } else {
                    Err(IoError::DestinationUnavailable)
                }
            }
            Err(error) => Err(destination_error(error)),
        }
    })();

    if result.is_err() || matches!(result, Ok(ItemOutcome::Duplicate)) {
        cleanup_temporary(&parent_directory, &temporary_name, temporary_identity);
    }
    result
}

fn open_source(path: &Path) -> Result<File, IoError> {
    let metadata = fs::symlink_metadata(path).map_err(source_error)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(IoError::SourceMissing);
    }
    let mut options = OpenOptions::new();
    options
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    options.open(path).map_err(source_error)
}

fn copy_and_hash(
    source: &mut File,
    destination: &mut File,
    should_cancel: &dyn Fn() -> bool,
) -> Result<Fingerprint, IoError> {
    let expected_size = source.metadata().map_err(source_error)?.len();
    let mut hasher = Sha256::new();
    let mut size = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        if should_cancel() {
            return Err(IoError::Cancelled);
        }
        let read = source.read(&mut buffer).map_err(source_error)?;
        if read == 0 {
            break;
        }
        destination
            .write_all(&buffer[..read])
            .map_err(destination_error)?;
        hasher.update(&buffer[..read]);
        size = size
            .checked_add(read as u64)
            .ok_or_else(|| IoError::Io("source size overflow".to_owned()))?;
    }
    if size != expected_size {
        return Err(IoError::HashMismatch);
    }
    Ok(Fingerprint {
        size,
        sha256: hasher.finalize().into(),
    })
}

fn existing_destination(
    directory: &OwnedFd,
    name: &OsStr,
    expected: Fingerprint,
) -> Result<Option<ItemOutcome>, IoError> {
    let name = c_string(name).map_err(|_| IoError::DestinationUnavailable)?;
    // SAFETY: the directory descriptor and NUL-terminated name stay valid for the call.
    let descriptor = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if descriptor < 0 {
        let error = std::io::Error::last_os_error();
        if error.kind() == std::io::ErrorKind::NotFound {
            return Ok(None);
        }
        return Err(destination_error(error));
    }

    // SAFETY: openat returned a new descriptor and ownership is transferred exactly once.
    let mut file = unsafe { File::from_raw_fd(descriptor) };
    let actual = fingerprint_open_file(&mut file).map_err(|error| match error {
        IoError::PermissionDenied => IoError::PermissionDenied,
        _ => IoError::DestinationUnavailable,
    })?;
    if actual == expected {
        Ok(Some(ItemOutcome::Duplicate))
    } else {
        Err(IoError::DestinationUnavailable)
    }
}

fn open_destination_root(path: &Path) -> Result<OwnedFd, IoError> {
    let absolute = resolved_destination_path(path)?;
    let start = open_directory(Path::new("/"))?;
    open_or_create_relative_directory(&start, absolute.strip_prefix("/").unwrap_or(&absolute))
}

fn open_or_create_relative_directory(root: &OwnedFd, relative: &Path) -> Result<OwnedFd, IoError> {
    let mut current = duplicate_fd(root)?;
    for component in relative.components() {
        let Component::Normal(name) = component else {
            if matches!(component, Component::CurDir) {
                continue;
            }
            return Err(IoError::DestinationUnavailable);
        };
        let name = c_string(name).map_err(|_| IoError::DestinationUnavailable)?;
        // SAFETY: the owned directory descriptor and component name are valid for the call.
        let descriptor = unsafe {
            libc::openat(
                current.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        let descriptor = if descriptor >= 0 {
            descriptor
        } else {
            let open_error = std::io::Error::last_os_error();
            if open_error.kind() != std::io::ErrorKind::NotFound {
                return Err(destination_error(open_error));
            }
            // SAFETY: mkdirat only borrows the valid descriptor and component name.
            let created = unsafe { libc::mkdirat(current.as_raw_fd(), name.as_ptr(), 0o755) };
            if created < 0 {
                let create_error = std::io::Error::last_os_error();
                if create_error.kind() != std::io::ErrorKind::AlreadyExists {
                    return Err(destination_error(create_error));
                }
            }
            // SAFETY: the same valid descriptor and name are reused after creation.
            let descriptor = unsafe {
                libc::openat(
                    current.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if descriptor < 0 {
                return Err(destination_error(std::io::Error::last_os_error()));
            }
            descriptor
        };
        // SAFETY: descriptor is checked above and transferred to one OwnedFd.
        current = unsafe { OwnedFd::from_raw_fd(descriptor) };
    }
    Ok(current)
}

fn open_directory(path: &Path) -> Result<OwnedFd, IoError> {
    let path =
        CString::new(path.as_os_str().as_bytes()).map_err(|_| IoError::DestinationUnavailable)?;
    // SAFETY: path is NUL-terminated and remains alive for the open call.
    let descriptor = unsafe {
        libc::open(
            path.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if descriptor < 0 {
        Err(destination_error(std::io::Error::last_os_error()))
    } else {
        // SAFETY: descriptor is checked above and transferred to one OwnedFd.
        Ok(unsafe { OwnedFd::from_raw_fd(descriptor) })
    }
}

fn duplicate_fd(fd: &OwnedFd) -> Result<OwnedFd, IoError> {
    // SAFETY: fcntl borrows a valid descriptor and returns an independent descriptor.
    let descriptor = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 0) };
    if descriptor < 0 {
        Err(destination_error(std::io::Error::last_os_error()))
    } else {
        // SAFETY: descriptor is checked above and transferred to one OwnedFd.
        Ok(unsafe { OwnedFd::from_raw_fd(descriptor) })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct FileIdentity {
    device: libc::dev_t,
    inode: libc::ino_t,
    mode: libc::mode_t,
}

fn create_temporary(directory: &OwnedFd) -> Result<(CString, File, FileIdentity), IoError> {
    loop {
        let name = CString::new(format!(".rawzelia-tmp-{}", random_token()?))
            .map_err(|_| IoError::DestinationUnavailable)?;
        // SAFETY: the valid directory descriptor and generated name live through the call.
        let descriptor = unsafe {
            libc::openat(
                directory.as_raw_fd(),
                name.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600,
            )
        };
        if descriptor >= 0 {
            // SAFETY: descriptor is checked above and transferred to one File.
            let file = unsafe { File::from_raw_fd(descriptor) };
            let identity = descriptor_identity(file.as_raw_fd()).map_err(destination_error)?;
            return Ok((name, file, identity));
        }
        let error = std::io::Error::last_os_error();
        if error.kind() != std::io::ErrorKind::AlreadyExists {
            return Err(destination_error(error));
        }
    }
}

fn random_token() -> Result<Uuid, IoError> {
    let mut bytes = [0_u8; 16];
    // SAFETY: bytes points to a writable buffer of the exact supplied length.
    let result = unsafe { libc::getentropy(bytes.as_mut_ptr().cast(), bytes.len()) };
    if result != 0 {
        return Err(destination_error(std::io::Error::last_os_error()));
    }
    Ok(Uuid::from_bytes(bytes))
}

fn descriptor_identity(descriptor: libc::c_int) -> std::io::Result<FileIdentity> {
    let mut status = MaybeUninit::<libc::stat>::uninit();
    // SAFETY: status points to writable storage and descriptor is borrowed for the call.
    let result = unsafe { libc::fstat(descriptor, status.as_mut_ptr()) };
    if result != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: a successful fstat initialized the complete stat structure.
    let status = unsafe { status.assume_init() };
    Ok(FileIdentity {
        device: status.st_dev,
        inode: status.st_ino,
        mode: status.st_mode,
    })
}

fn path_identity(directory: &OwnedFd, name: &CString) -> Result<Option<FileIdentity>, IoError> {
    let mut status = MaybeUninit::<libc::stat>::uninit();
    // SAFETY: status is writable; descriptor and NUL-terminated name remain valid.
    let result = unsafe {
        libc::fstatat(
            directory.as_raw_fd(),
            name.as_ptr(),
            status.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if result != 0 {
        let error = std::io::Error::last_os_error();
        if error.kind() == std::io::ErrorKind::NotFound {
            return Ok(None);
        }
        return Err(destination_error(error));
    }
    // SAFETY: a successful fstatat initialized the complete stat structure.
    let status = unsafe { status.assume_init() };
    Ok(Some(FileIdentity {
        device: status.st_dev,
        inode: status.st_ino,
        mode: status.st_mode,
    }))
}

fn cleanup_temporary(directory: &OwnedFd, name: &CString, owned: FileIdentity) {
    if !matches!(path_identity(directory, name), Ok(Some(current)) if current == owned) {
        return;
    }
    // SAFETY: identity was verified without following links immediately before this call.
    unsafe {
        libc::unlinkat(directory.as_raw_fd(), name.as_ptr(), 0);
    }
}

#[cfg(target_os = "macos")]
fn rename_exclusive(
    directory: &OwnedFd,
    source: &CString,
    destination: &OsStr,
) -> std::io::Result<()> {
    let destination = c_string(destination)?;
    // SAFETY: both names and the owned directory descriptor remain valid for the call.
    let result = unsafe {
        libc::renameatx_np(
            directory.as_raw_fd(),
            source.as_ptr(),
            directory.as_raw_fd(),
            destination.as_ptr(),
            libc::RENAME_EXCL,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(not(target_os = "macos"))]
fn rename_exclusive(
    directory: &OwnedFd,
    source: &CString,
    destination: &OsStr,
) -> std::io::Result<()> {
    let destination = c_string(destination)?;
    // SAFETY: both names and the owned directory descriptor remain valid for the call.
    let linked = unsafe {
        libc::linkat(
            directory.as_raw_fd(),
            source.as_ptr(),
            directory.as_raw_fd(),
            destination.as_ptr(),
            0,
        )
    };
    if linked != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: source and the directory descriptor remain valid for the call.
    let removed = unsafe { libc::unlinkat(directory.as_raw_fd(), source.as_ptr(), 0) };
    if removed == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

fn sync_directory(directory: &OwnedFd) -> Result<(), IoError> {
    // SAFETY: fsync only borrows the valid owned directory descriptor.
    let result = unsafe { libc::fsync(directory.as_raw_fd()) };
    if result == 0 {
        Ok(())
    } else {
        Err(destination_error(std::io::Error::last_os_error()))
    }
}

fn c_string(value: &OsStr) -> std::io::Result<CString> {
    CString::new(value.as_bytes())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "path contains NUL"))
}

fn require_source_root(path: &Path) -> Result<PathBuf, IoError> {
    let metadata = fs::symlink_metadata(path).map_err(source_error)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(IoError::SourceMissing);
    }
    fs::canonicalize(path).map_err(source_error)
}

fn require_no_source_symlink(root: &Path, path: &Path) -> Result<(), IoError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| IoError::SourceMissing)?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err(IoError::SourceMissing);
        };
        current.push(name);
        let metadata = fs::symlink_metadata(&current).map_err(source_error)?;
        if metadata.file_type().is_symlink() {
            return Err(IoError::SourceMissing);
        }
    }
    Ok(())
}

fn require_destination_outside_source(
    destination: &Path,
    source_root: &Path,
) -> Result<(), IoError> {
    let destination = resolved_destination_path(destination)?;
    if destination.starts_with(source_root) || source_root.starts_with(&destination) {
        Err(IoError::DestinationUnavailable)
    } else {
        Ok(())
    }
}

fn resolved_destination_path(path: &Path) -> Result<PathBuf, IoError> {
    let absolute = absolute_without_parent(path).ok_or(IoError::DestinationUnavailable)?;
    let mut existing = absolute.as_path();
    let mut missing = Vec::new();
    loop {
        match fs::symlink_metadata(existing) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(IoError::DestinationUnavailable);
                }
                let mut resolved = fs::canonicalize(existing).map_err(destination_error)?;
                for component in missing.iter().rev() {
                    resolved.push(component);
                }
                return Ok(resolved);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let name = existing
                    .file_name()
                    .ok_or(IoError::DestinationUnavailable)?;
                missing.push(name.to_os_string());
                existing = existing.parent().ok_or(IoError::DestinationUnavailable)?;
            }
            Err(error) => return Err(destination_error(error)),
        }
    }
}

fn normalize_without_parent(path: &Path) -> Option<PathBuf> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(Path::new("/")),
            Component::CurDir => {}
            Component::Normal(value) => normalized.push(value),
            Component::ParentDir => return None,
        }
    }
    Some(normalized)
}

fn absolute_without_parent(path: &Path) -> Option<PathBuf> {
    if path.is_absolute() {
        normalize_without_parent(path)
    } else {
        normalize_without_parent(&std::env::current_dir().ok()?.join(path))
    }
}

fn capture_date(modified: SystemTime) -> Result<CaptureDate, IoError> {
    let seconds = modified
        .duration_since(UNIX_EPOCH)
        .map_err(|_| IoError::Io("source modification date predates Unix epoch".to_owned()))?
        .as_secs();
    let days = i64::try_from(seconds / 86_400)
        .map_err(|_| IoError::Io("source date overflow".to_owned()))?;
    let seconds_of_day = seconds % 86_400;
    let (year, month, day) = civil_date_from_days(days);
    Ok(CaptureDate {
        year,
        month,
        day,
        hour: (seconds_of_day / 3_600) as u8,
        minute: ((seconds_of_day % 3_600) / 60) as u8,
        second: (seconds_of_day % 60) as u8,
    })
}

fn civil_date_from_days(days_since_epoch: i64) -> (i32, u8, u8) {
    let shifted = days_since_epoch + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year as i32, month as u8, day as u8)
}

fn asset_id(fingerprint: Fingerprint) -> AssetId {
    let mut hasher = Sha256::new();
    hasher.update(b"rawzelia.asset.v1\0");
    hasher.update(fingerprint.size.to_le_bytes());
    hasher.update(fingerprint.sha256);
    let digest: [u8; 32] = hasher.finalize().into();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    AssetId(Uuid::from_bytes(bytes))
}
