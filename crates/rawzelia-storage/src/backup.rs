#[cfg(test)]
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use rusqlite::backup::Backup;
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const BACKUP_VERSION: u16 = 1;
const CATALOG_FILE: &str = "catalog.sqlite";
const MANIFEST_FILE: &str = "manifest.json";
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
thread_local! {
    static BEFORE_PUBLISH_HOOK: RefCell<Option<Box<dyn FnOnce()>>> = RefCell::new(None);
}

#[cfg(test)]
fn install_before_publish_hook(hook: Box<dyn FnOnce()>) {
    BEFORE_PUBLISH_HOOK.with(|slot| slot.replace(Some(hook)));
}

#[cfg(test)]
fn run_before_publish_hook() {
    BEFORE_PUBLISH_HOOK.with(|slot| {
        if let Some(hook) = slot.borrow_mut().take() {
            hook();
        }
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupReport {
    pub catalog_sha256: [u8; 32],
    pub sidecar_sha256: BTreeMap<PathBuf, [u8; 32]>,
    pub entry_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreReport {
    pub restored_catalog: PathBuf,
    pub restored_sidecars: u64,
    pub verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackupError {
    Io(String),
    HashMismatch,
    InconsistentCatalog,
    DestinationExists,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestoreError {
    Io(String),
    InvalidManifest,
    HashMismatch,
    DestinationUntouched,
}

#[derive(Debug, Serialize, Deserialize)]
struct BackupManifest {
    version: u16,
    catalog_sha256: [u8; 32],
    entry_count: u64,
    sidecars: Vec<SidecarManifestEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SidecarManifestEntry {
    original_path: PathBuf,
    backup_name: String,
    sha256: [u8; 32],
}

pub fn create_backup(
    catalog: &Path,
    sidecars: &[PathBuf],
    destination: &Path,
) -> Result<BackupReport, BackupError> {
    if path_entry_exists(destination).map_err(backup_io)? {
        return Err(BackupError::DestinationExists);
    }
    if !catalog.is_file() {
        return Err(BackupError::Io("catalog_missing".to_owned()));
    }

    let parent = parent_directory(destination);
    fs::create_dir_all(parent).map_err(backup_io)?;
    let temporary = create_temporary_directory(destination).map_err(backup_io)?;
    let result = create_backup_in(catalog, sidecars, &temporary);
    let report = match result {
        Ok(report) => report,
        Err(error) => {
            let _ = fs::remove_dir_all(&temporary);
            return Err(error);
        }
    };

    if path_entry_exists(destination).map_err(backup_io)? {
        let _ = fs::remove_dir_all(&temporary);
        return Err(BackupError::DestinationExists);
    }
    #[cfg(test)]
    run_before_publish_hook();
    if let Err(error) = rename_directory_no_replace(&temporary, destination) {
        let _ = fs::remove_dir_all(&temporary);
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            return Err(BackupError::DestinationExists);
        }
        return Err(backup_io(error));
    }
    sync_directory(parent).map_err(backup_io)?;
    Ok(report)
}

pub fn restore_backup(backup: &Path, destination: &Path) -> Result<RestoreReport, RestoreError> {
    if path_entry_exists(destination).map_err(restore_io)? {
        return Err(RestoreError::DestinationUntouched);
    }

    let manifest = read_manifest(backup)?;
    validate_backup_files(backup, &manifest)?;
    let parent = parent_directory(destination);
    fs::create_dir_all(parent).map_err(restore_io)?;
    let temporary = create_temporary_directory(destination).map_err(restore_io)?;
    let result = restore_into(backup, &manifest, &temporary);
    let restored_sidecars = match result {
        Ok(count) => count,
        Err(error) => {
            let _ = fs::remove_dir_all(&temporary);
            return Err(error);
        }
    };

    if path_entry_exists(destination).map_err(restore_io)? {
        let _ = fs::remove_dir_all(&temporary);
        return Err(RestoreError::DestinationUntouched);
    }
    #[cfg(test)]
    run_before_publish_hook();
    if let Err(error) = rename_directory_no_replace(&temporary, destination) {
        let _ = fs::remove_dir_all(&temporary);
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            return Err(RestoreError::DestinationUntouched);
        }
        return Err(restore_io(error));
    }
    sync_directory(parent).map_err(restore_io)?;

    Ok(RestoreReport {
        restored_catalog: destination.join(CATALOG_FILE),
        restored_sidecars,
        verified: true,
    })
}

fn create_backup_in(
    catalog: &Path,
    sidecars: &[PathBuf],
    temporary: &Path,
) -> Result<BackupReport, BackupError> {
    let catalog_destination = temporary.join(CATALOG_FILE);
    backup_database(catalog, &catalog_destination)?;
    let entry_count =
        validate_catalog(&catalog_destination).map_err(|_| BackupError::InconsistentCatalog)?;
    let catalog_sha256 = hash_file(&catalog_destination).map_err(backup_io)?;

    let sidecar_directory = temporary.join("sidecars");
    fs::create_dir(&sidecar_directory).map_err(backup_io)?;
    let mut sidecar_sha256 = BTreeMap::new();
    let mut manifest_sidecars = Vec::new();
    for (index, source) in sidecars.iter().enumerate() {
        if !source.is_file() {
            return Err(BackupError::Io(format!(
                "sidecar_missing:{}",
                source.display()
            )));
        }
        let extension = source
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("rawzelia");
        let backup_name = format!("{index:08}.{extension}");
        let copied = sidecar_directory.join(&backup_name);
        copy_and_sync(source, &copied).map_err(backup_io)?;
        let source_hash = hash_file(source).map_err(backup_io)?;
        let copied_hash = hash_file(&copied).map_err(backup_io)?;
        if source_hash != copied_hash {
            return Err(BackupError::HashMismatch);
        }
        sidecar_sha256.insert(source.clone(), source_hash);
        manifest_sidecars.push(SidecarManifestEntry {
            original_path: source.clone(),
            backup_name,
            sha256: source_hash,
        });
    }

    let manifest = BackupManifest {
        version: BACKUP_VERSION,
        catalog_sha256,
        entry_count,
        sidecars: manifest_sidecars,
    };
    let manifest_bytes =
        serde_json::to_vec_pretty(&manifest).map_err(|error| BackupError::Io(error.to_string()))?;
    write_and_sync(&temporary.join(MANIFEST_FILE), &manifest_bytes).map_err(backup_io)?;
    sync_directory(&sidecar_directory).map_err(backup_io)?;
    sync_directory(temporary).map_err(backup_io)?;

    Ok(BackupReport {
        catalog_sha256,
        sidecar_sha256,
        entry_count,
    })
}

fn restore_into(
    backup: &Path,
    manifest: &BackupManifest,
    temporary: &Path,
) -> Result<u64, RestoreError> {
    let catalog_source = backup.join(CATALOG_FILE);
    let catalog_destination = temporary.join(CATALOG_FILE);
    copy_and_sync(&catalog_source, &catalog_destination).map_err(restore_io)?;
    if hash_file(&catalog_destination).map_err(restore_io)? != manifest.catalog_sha256 {
        return Err(RestoreError::HashMismatch);
    }
    let entry_count =
        validate_catalog(&catalog_destination).map_err(|_| RestoreError::InvalidManifest)?;
    if entry_count != manifest.entry_count {
        return Err(RestoreError::InvalidManifest);
    }

    let sidecar_directory = temporary.join("sidecars");
    fs::create_dir(&sidecar_directory).map_err(restore_io)?;
    for entry in &manifest.sidecars {
        let source = backup.join("sidecars").join(&entry.backup_name);
        let destination = sidecar_directory.join(&entry.backup_name);
        copy_and_sync(&source, &destination).map_err(restore_io)?;
        if hash_file(&destination).map_err(restore_io)? != entry.sha256 {
            return Err(RestoreError::HashMismatch);
        }
    }
    sync_directory(&sidecar_directory).map_err(restore_io)?;
    sync_directory(temporary).map_err(restore_io)?;
    u64::try_from(manifest.sidecars.len()).map_err(|_| RestoreError::InvalidManifest)
}

fn read_manifest(backup: &Path) -> Result<BackupManifest, RestoreError> {
    let bytes = fs::read(backup.join(MANIFEST_FILE)).map_err(restore_io)?;
    let manifest: BackupManifest =
        serde_json::from_slice(&bytes).map_err(|_| RestoreError::InvalidManifest)?;
    if manifest.version != BACKUP_VERSION {
        return Err(RestoreError::InvalidManifest);
    }
    if manifest
        .sidecars
        .iter()
        .any(|entry| !is_single_file_name(&entry.backup_name))
    {
        return Err(RestoreError::InvalidManifest);
    }
    Ok(manifest)
}

fn validate_backup_files(backup: &Path, manifest: &BackupManifest) -> Result<(), RestoreError> {
    let catalog = backup.join(CATALOG_FILE);
    if hash_file(&catalog).map_err(restore_io)? != manifest.catalog_sha256 {
        return Err(RestoreError::HashMismatch);
    }
    let entry_count = validate_catalog(&catalog).map_err(|_| RestoreError::InvalidManifest)?;
    if entry_count != manifest.entry_count {
        return Err(RestoreError::InvalidManifest);
    }
    for entry in &manifest.sidecars {
        let sidecar = backup.join("sidecars").join(&entry.backup_name);
        if hash_file(&sidecar).map_err(restore_io)? != entry.sha256 {
            return Err(RestoreError::HashMismatch);
        }
    }
    Ok(())
}

fn backup_database(source: &Path, destination: &Path) -> Result<(), BackupError> {
    let source = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|_| BackupError::InconsistentCatalog)?;
    let mut destination =
        Connection::open(destination).map_err(|_| BackupError::InconsistentCatalog)?;
    let backup =
        Backup::new(&source, &mut destination).map_err(|_| BackupError::InconsistentCatalog)?;
    backup
        .run_to_completion(32, Duration::from_millis(1), None)
        .map_err(|_| BackupError::InconsistentCatalog)
}

fn validate_catalog(path: &Path) -> Result<u64, rusqlite::Error> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    let integrity: String = connection.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    if integrity != "ok" {
        return Err(rusqlite::Error::InvalidQuery);
    }
    connection.query_row("SELECT COUNT(*) FROM assets", [], |row| row.get(0))
}

fn hash_file(path: &Path) -> Result<[u8; 32], std::io::Error> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().into())
}

fn create_temporary_directory(destination: &Path) -> Result<PathBuf, std::io::Error> {
    let parent = parent_directory(destination);
    let file_name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("backup");
    loop {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(
            ".{file_name}.tmp-{}-{sequence}",
            std::process::id()
        ));
        match fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
}

fn copy_and_sync(source: &Path, destination: &Path) -> Result<(), std::io::Error> {
    fs::copy(source, destination)?;
    OpenOptions::new().write(true).open(destination)?.sync_all()
}

fn write_and_sync(path: &Path, bytes: &[u8]) -> Result<(), std::io::Error> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.flush()?;
    file.sync_all()
}

fn sync_directory(path: &Path) -> Result<(), std::io::Error> {
    File::open(path)?.sync_all()
}

fn rename_directory_no_replace(source: &Path, destination: &Path) -> Result<(), std::io::Error> {
    let source = CString::new(source.as_os_str().as_bytes()).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "source path contains NUL")
    })?;
    let destination = CString::new(destination.as_os_str().as_bytes()).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "destination path contains NUL",
        )
    })?;
    // SAFETY: both paths are live NUL-terminated strings for the duration of the call.
    let result = unsafe {
        libc::renameatx_np(
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
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

fn parent_directory(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn path_entry_exists(path: &Path) -> Result<bool, std::io::Error> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn is_single_file_name(value: &str) -> bool {
    Path::new(value).file_name().and_then(|name| name.to_str()) == Some(value)
}

fn backup_io(error: std::io::Error) -> BackupError {
    BackupError::Io(error.to_string())
}

fn restore_io(error: std::io::Error) -> RestoreError {
    RestoreError::Io(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::MetadataExt;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::mpsc;
    use std::thread;

    use super::*;

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            loop {
                let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
                let path = std::env::temp_dir().join(format!(
                    "rawzelia-backup-{label}-{}-{sequence}",
                    std::process::id()
                ));
                match fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("test directory must be created: {error}"),
                }
            }
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn create_catalog(path: &Path) {
        Connection::open(path)
            .unwrap()
            .execute_batch("CREATE TABLE assets (asset_id TEXT PRIMARY KEY);")
            .unwrap();
    }

    fn install_concurrent_destination_creation(destination: PathBuf) -> thread::JoinHandle<u64> {
        let (start_sender, start_receiver) = mpsc::sync_channel(0);
        let (created_sender, created_receiver) = mpsc::sync_channel(0);
        let worker = thread::spawn(move || {
            start_receiver
                .recv_timeout(Duration::from_secs(5))
                .expect("publish hook must release destination creator");
            fs::create_dir(&destination).expect("concurrent destination must be created");
            let inode = fs::metadata(&destination).unwrap().ino();
            created_sender
                .send(inode)
                .expect("created destination identity must be returned");
            inode
        });
        install_before_publish_hook(Box::new(move || {
            start_sender
                .send(())
                .expect("destination creator must start");
            created_receiver
                .recv_timeout(Duration::from_secs(5))
                .expect("destination must exist before publication");
        }));
        worker
    }

    #[test]
    fn concurrent_destination_creation_is_preserved_by_create_backup() {
        let fixture = TestDirectory::new("create-race");
        let catalog = fixture.path().join("catalog.sqlite");
        let destination = fixture.path().join("backup");
        create_catalog(&catalog);
        let worker = install_concurrent_destination_creation(destination.clone());

        let result = create_backup(&catalog, &[], &destination);
        let concurrent_inode = worker.join().unwrap();

        assert_eq!(result, Err(BackupError::DestinationExists));
        assert!(destination.is_dir());
        assert_eq!(fs::metadata(destination).unwrap().ino(), concurrent_inode);
    }

    #[test]
    fn concurrent_destination_creation_is_preserved_by_restore_backup() {
        let fixture = TestDirectory::new("restore-race");
        let catalog = fixture.path().join("catalog.sqlite");
        let backup = fixture.path().join("backup");
        let destination = fixture.path().join("restored");
        create_catalog(&catalog);
        create_backup(&catalog, &[], &backup).unwrap();
        let worker = install_concurrent_destination_creation(destination.clone());

        let result = restore_backup(&backup, &destination);
        let concurrent_inode = worker.join().unwrap();

        assert_eq!(result, Err(RestoreError::DestinationUntouched));
        assert!(destination.is_dir());
        assert_eq!(fs::metadata(destination).unwrap().ino(), concurrent_inode);
    }
}
