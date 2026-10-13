use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{CStr, CString};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::mem::MaybeUninit;
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use rawzelia_domain::{Adjustment, CoreValue, EditDocument, ImportStatus, ImportedField};
use serde::{Deserialize, Serialize};

use crate::StorageError;

const RAWZELIA_XMP_BANNER: &str =
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- rawzelia:xmp-mirror:v1 -->\n";
static CANONICAL_XMP_ROOTS: OnceLock<Mutex<BTreeMap<PathBuf, Arc<XmpOutputRoot>>>> =
    OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct FileIdentity {
    device: u64,
    inode: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct XmpRootAuthority {
    root: FileIdentity,
    files: BTreeMap<PathBuf, FileIdentity>,
}

struct OwnedXmp {
    file: File,
    identity: FileIdentity,
}

pub struct XmpOutputRoot {
    directory: File,
    identity: FileIdentity,
    owned: Mutex<BTreeMap<PathBuf, OwnedXmp>>,
}

impl XmpOutputRoot {
    pub fn create(path: &Path) -> Result<Self, StorageError> {
        let root_name = root_directory_name(path)?;
        let parent_path = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let parent = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
            .open(parent_path)
            .map_err(StorageError::from_io)?;
        let temporary_name = create_temporary_directory(parent.as_raw_fd())?;
        let directory = match open_directory_at(parent.as_raw_fd(), &temporary_name) {
            Ok(directory) => directory,
            Err(error) => {
                let _ = remove_directory_entry(parent.as_raw_fd(), &temporary_name);
                return Err(StorageError::from_io(error));
            }
        };
        let opened = directory.metadata().map_err(StorageError::from_io)?;
        if !is_private_directory(&opened) {
            let _ = remove_directory_entry(parent.as_raw_fd(), &temporary_name);
            return Err(unowned_xmp_root());
        }
        let identity = identity_from_metadata(&opened);
        if let Err(error) = rename_entry(
            parent.as_raw_fd(),
            &temporary_name,
            &root_name,
            libc::RENAME_EXCL,
        ) {
            let _ = remove_directory_entry(parent.as_raw_fd(), &temporary_name);
            return if error.kind() == std::io::ErrorKind::AlreadyExists {
                Err(unowned_xmp_root())
            } else {
                Err(StorageError::from_io(error))
            };
        }
        parent.sync_all().map_err(StorageError::from_io)?;

        Ok(Self {
            directory,
            identity,
            owned: Mutex::new(BTreeMap::new()),
        })
    }

    pub(crate) fn open_with_authority(
        path: &Path,
        authority: XmpRootAuthority,
    ) -> Result<Self, StorageError> {
        let root_name = root_directory_name(path)?;
        let parent_path = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let parent = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
            .open(parent_path)
            .map_err(StorageError::from_io)?;
        let directory =
            open_directory_at(parent.as_raw_fd(), &root_name).map_err(|_| unowned_xmp_root())?;
        let opened = directory.metadata().map_err(StorageError::from_io)?;
        if !is_private_directory(&opened) || identity_from_metadata(&opened) != authority.root {
            return Err(unowned_xmp_root());
        }

        let mut owned = BTreeMap::new();
        for (relative_path, expected_identity) in authority.files {
            let name = managed_xmp_name(&relative_path)?;
            let file = open_owned_file_at(directory.as_raw_fd(), &name)?;
            let metadata = file.metadata().map_err(StorageError::from_io)?;
            if !metadata.is_file()
                || metadata.nlink() != 1
                || identity_from_metadata(&metadata) != expected_identity
            {
                return Err(xmp_authority_conflict());
            }
            owned.insert(
                relative_path,
                OwnedXmp {
                    file,
                    identity: expected_identity,
                },
            );
        }

        Ok(Self {
            directory,
            identity: authority.root,
            owned: Mutex::new(owned),
        })
    }

    pub(crate) fn persistent_authority(
        &self,
        path: &Path,
    ) -> Result<XmpRootAuthority, StorageError> {
        if root_identity_at_path(path)? != Some(self.identity) {
            return Err(unowned_xmp_root());
        }
        let files = self
            .owned_files()?
            .iter()
            .map(|(path, owned)| (path.clone(), owned.identity))
            .collect();
        Ok(XmpRootAuthority {
            root: self.identity,
            files,
        })
    }

    pub fn write_mirror(
        &self,
        relative_path: &Path,
        document: &EditDocument,
    ) -> Result<XmpWriteReport, StorageError> {
        let target = managed_xmp_name(relative_path)?;
        let (bytes, report) = render_xmp(document)?;
        let mut owned = self.owned_files()?;
        if let Some(existing) = owned.get_mut(relative_path) {
            return rewrite_owned_xmp(&self.directory, &target, existing, &bytes, report);
        }

        let mut temporary = create_temporary_file(self.directory.as_raw_fd(), &target)?;
        if let Err(error) = write_and_sync(&mut temporary.file, &bytes) {
            let _ = unlink_entry(self.directory.as_raw_fd(), &temporary.name);
            return Err(StorageError::from_io(error));
        }
        let new_identity = match entry_identity(self.directory.as_raw_fd(), &temporary.name) {
            Ok(Some(identity)) => identity,
            Ok(None) => {
                let _ = unlink_entry(self.directory.as_raw_fd(), &temporary.name);
                return Err(StorageError::from_io(std::io::Error::from_raw_os_error(
                    libc::ENOENT,
                )));
            }
            Err(error) => {
                let _ = unlink_entry(self.directory.as_raw_fd(), &temporary.name);
                return Err(error);
            }
        };

        publish_new(self.directory.as_raw_fd(), &temporary.name, &target)?;
        owned.insert(
            relative_path.to_path_buf(),
            OwnedXmp {
                file: temporary.file,
                identity: new_identity,
            },
        );
        self.directory.sync_all().map_err(StorageError::from_io)?;
        Ok(report)
    }

    fn owned_files(&self) -> Result<MutexGuard<'_, BTreeMap<PathBuf, OwnedXmp>>, StorageError> {
        self.owned.lock().map_err(|_| StorageError::Io {
            code: "xmp_output_lock_poisoned".to_owned(),
            message_key: "storage.xmp.output_lock".to_owned(),
        })
    }
}

struct TemporaryFile {
    name: CString,
    file: File,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmpSnapshot {
    pub fields: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reconciliation {
    CatalogAuthoritative,
    SidecarAuthoritative,
    XmpAuthoritative,
    Conflict { fields: Vec<String> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XmpWriteReport {
    pub mirrored_fields: u32,
    pub skipped_fields: u32,
}

pub fn write_xmp_mirror(
    path: &Path,
    document: &EditDocument,
) -> Result<XmpWriteReport, StorageError> {
    let file_name = path
        .file_name()
        .ok_or_else(unmanaged_xmp_target)
        .map(Path::new)?;
    managed_xmp_name(file_name)?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let root = {
        let roots = CANONICAL_XMP_ROOTS.get_or_init(|| Mutex::new(BTreeMap::new()));
        let mut roots = roots.lock().map_err(|_| StorageError::Io {
            code: "xmp_root_registry_poisoned".to_owned(),
            message_key: "storage.xmp.root_registry".to_owned(),
        })?;
        if let Some(root) = roots.get(parent) {
            Arc::clone(root)
        } else {
            let root = Arc::new(XmpOutputRoot::create(parent)?);
            roots.insert(parent.to_path_buf(), Arc::clone(&root));
            root
        }
    };
    root.write_mirror(file_name, document)
}

fn render_xmp(document: &EditDocument) -> Result<(Vec<u8>, XmpWriteReport), StorageError> {
    let (properties, skipped_fields) = mirror_properties(document);
    let mut xml = String::from(RAWZELIA_XMP_BANNER);
    xml.push_str(
        "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n\
         <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n\
         <rdf:Description xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\" \
         xmlns:dc=\"http://purl.org/dc/elements/1.1/\" \
         xmlns:tiff=\"http://ns.adobe.com/tiff/1.0/\" \
         xmlns:crs=\"http://ns.adobe.com/camera-raw-settings/1.0/\">\n",
    );
    for (name, value) in &properties {
        xml.push('<');
        xml.push_str(name);
        xml.push('>');
        xml.push_str(&escape_xml(value));
        xml.push_str("</");
        xml.push_str(name);
        xml.push_str(">\n");
    }
    xml.push_str("</rdf:Description>\n</rdf:RDF>\n</x:xmpmeta>\n");
    let report = XmpWriteReport {
        mirrored_fields: u32::try_from(properties.len()).map_err(|_| StorageError::Io {
            code: "xmp_field_count_out_of_range".to_owned(),
            message_key: "storage.xmp.too_many_fields".to_owned(),
        })?,
        skipped_fields,
    };
    Ok((xml.into_bytes(), report))
}

fn unmanaged_xmp_target() -> StorageError {
    StorageError::Io {
        code: "unmanaged_xmp_target".to_owned(),
        message_key: "storage.xmp.unmanaged_target".to_owned(),
    }
}

pub(crate) fn unowned_xmp_root() -> StorageError {
    StorageError::Io {
        code: "unowned_xmp_root".to_owned(),
        message_key: "storage.xmp.unowned_root".to_owned(),
    }
}

fn xmp_authority_conflict() -> StorageError {
    StorageError::Conflict {
        fields: vec!["xmp.output_authority".to_owned()],
    }
}

fn managed_xmp_name(path: &Path) -> Result<CString, StorageError> {
    let file_name = path
        .file_name()
        .filter(|file_name| Path::new(file_name) == path)
        .and_then(|file_name| file_name.to_str())
        .filter(|file_name| file_name.ends_with(".rawzelia.xmp"))
        .ok_or_else(unmanaged_xmp_target)?;
    CString::new(file_name).map_err(|_| unmanaged_xmp_target())
}

fn root_directory_name(path: &Path) -> Result<CString, StorageError> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty() && *name != "." && *name != "..")
        .ok_or_else(unowned_xmp_root)?;
    CString::new(name).map_err(|_| unowned_xmp_root())
}

fn create_temporary_directory(parent: RawFd) -> Result<CString, StorageError> {
    loop {
        let mut random = [0_u8; 16];
        // SAFETY: `random` is writable for exactly its initialized byte length.
        unsafe { libc::arc4random_buf(random.as_mut_ptr().cast(), random.len()) };
        let name = CString::new(format!(
            ".rawzelia-xmp-root-{:032x}",
            u128::from_ne_bytes(random)
        ))
        .map_err(|_| unowned_xmp_root())?;
        // SAFETY: `parent` is an open directory and `name` is a live C string.
        let result = unsafe { libc::mkdirat(parent, name.as_ptr(), 0o700) };
        if result == 0 {
            return Ok(name);
        }
        let error = std::io::Error::last_os_error();
        if error.kind() != std::io::ErrorKind::AlreadyExists {
            return Err(StorageError::from_io(error));
        }
    }
}

fn open_directory_at(parent: RawFd, name: &CStr) -> Result<File, std::io::Error> {
    // SAFETY: `parent` is an open directory descriptor and `name` is a live C string.
    let descriptor = unsafe {
        libc::openat(
            parent,
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW,
        )
    };
    if descriptor < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: `openat` returned a new owned descriptor that is transferred once.
    Ok(unsafe { File::from_raw_fd(descriptor) })
}

fn open_owned_file_at(directory: RawFd, name: &CStr) -> Result<File, StorageError> {
    // SAFETY: `directory` is an open directory descriptor and `name` is a live C string.
    let descriptor =
        unsafe { libc::openat(directory, name.as_ptr(), libc::O_RDWR | libc::O_NOFOLLOW) };
    if descriptor < 0 {
        return Err(xmp_authority_conflict());
    }
    // SAFETY: `openat` returned a new owned descriptor that is transferred once.
    Ok(unsafe { File::from_raw_fd(descriptor) })
}

fn create_temporary_file(directory: RawFd, target: &CStr) -> Result<TemporaryFile, StorageError> {
    let target = target.to_string_lossy();
    loop {
        let mut random = [0_u8; 16];
        // SAFETY: `random` is writable for exactly its initialized byte length.
        unsafe { libc::arc4random_buf(random.as_mut_ptr().cast(), random.len()) };
        let name = CString::new(format!(
            ".{target}.tmp-{:032x}",
            u128::from_ne_bytes(random)
        ))
        .map_err(|_| unmanaged_xmp_target())?;
        // SAFETY: `directory` is an open directory descriptor and `name` is a live C string.
        let descriptor = unsafe {
            libc::openat(
                directory,
                name.as_ptr(),
                libc::O_RDWR | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW,
                0o600,
            )
        };
        if descriptor >= 0 {
            return Ok(TemporaryFile {
                name,
                // SAFETY: `openat` returned a new owned descriptor that is transferred once.
                file: unsafe { File::from_raw_fd(descriptor) },
            });
        }
        let error = std::io::Error::last_os_error();
        if error.kind() != std::io::ErrorKind::AlreadyExists {
            return Err(StorageError::from_io(error));
        }
    }
}

fn write_and_sync(file: &mut File, bytes: &[u8]) -> Result<(), std::io::Error> {
    file.write_all(bytes)?;
    file.flush()?;
    file.sync_all()
}

fn rewrite_owned_xmp(
    directory: &File,
    target: &CStr,
    owned: &mut OwnedXmp,
    bytes: &[u8],
    report: XmpWriteReport,
) -> Result<XmpWriteReport, StorageError> {
    if entry_identity(directory.as_raw_fd(), target)? != Some(owned.identity)
        || owned
            .file
            .metadata()
            .map_err(StorageError::from_io)?
            .nlink()
            != 1
    {
        return Err(unmanaged_xmp_target());
    }

    let mut staged = create_temporary_file(directory.as_raw_fd(), target)?;
    if let Err(error) = write_and_sync(&mut staged.file, bytes) {
        let _ = unlink_entry(directory.as_raw_fd(), &staged.name);
        return Err(StorageError::from_io(error));
    }
    let staged_metadata = staged.file.metadata().map_err(StorageError::from_io)?;
    let staged_identity = identity_from_metadata(&staged_metadata);
    if let Err(error) = rename_entry(
        directory.as_raw_fd(),
        &staged.name,
        target,
        libc::RENAME_SWAP,
    ) {
        let _ = unlink_entry(directory.as_raw_fd(), &staged.name);
        return Err(StorageError::from_io(error));
    }

    if entry_identity(directory.as_raw_fd(), &staged.name)? != Some(owned.identity) {
        let rollback = rename_entry(
            directory.as_raw_fd(),
            &staged.name,
            target,
            libc::RENAME_SWAP,
        );
        if rollback.is_ok() {
            let _ = unlink_entry(directory.as_raw_fd(), &staged.name);
            let _ = directory.sync_all();
        }
        return Err(StorageError::Conflict {
            fields: vec!["xmp.output_entry".to_owned()],
        });
    }

    if entry_identity(directory.as_raw_fd(), target)? != Some(staged_identity) {
        let _ = unlink_entry(directory.as_raw_fd(), &staged.name);
        let _ = directory.sync_all();
        return Err(StorageError::Conflict {
            fields: vec!["xmp.output_entry".to_owned()],
        });
    }

    owned.file = staged.file;
    owned.identity = staged_identity;
    directory.sync_all().map_err(StorageError::from_io)?;
    unlink_entry(directory.as_raw_fd(), &staged.name).map_err(StorageError::from_io)?;
    directory.sync_all().map_err(StorageError::from_io)?;
    Ok(report)
}

fn publish_new(directory: RawFd, temporary: &CStr, target: &CStr) -> Result<(), StorageError> {
    match rename_entry(directory, temporary, target, libc::RENAME_EXCL) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = unlink_entry(directory, temporary);
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                Err(unmanaged_xmp_target())
            } else {
                Err(StorageError::from_io(error))
            }
        }
    }
}

fn rename_entry(
    directory: RawFd,
    source: &CStr,
    destination: &CStr,
    flags: libc::c_uint,
) -> Result<(), std::io::Error> {
    // SAFETY: both names are live C strings and both directory descriptors are valid.
    let result = unsafe {
        libc::renameatx_np(
            directory,
            source.as_ptr(),
            directory,
            destination.as_ptr(),
            flags,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

fn unlink_entry(directory: RawFd, name: &CStr) -> Result<(), std::io::Error> {
    // SAFETY: `name` is a live C string relative to the valid directory descriptor.
    let result = unsafe { libc::unlinkat(directory, name.as_ptr(), 0) };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

fn remove_directory_entry(directory: RawFd, name: &CStr) -> Result<(), std::io::Error> {
    // SAFETY: `name` is a live C string relative to the valid directory descriptor.
    let result = unsafe { libc::unlinkat(directory, name.as_ptr(), libc::AT_REMOVEDIR) };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

fn entry_identity(directory: RawFd, name: &CStr) -> Result<Option<FileIdentity>, StorageError> {
    let mut metadata = MaybeUninit::<libc::stat>::uninit();
    // SAFETY: `metadata` is writable and `name` is valid for the duration of the call.
    let result = unsafe {
        libc::fstatat(
            directory,
            name.as_ptr(),
            metadata.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if result == 0 {
        // SAFETY: a successful `fstatat` initialized the complete `stat` value.
        let metadata = unsafe { metadata.assume_init() };
        return Ok(Some(identity_from_stat(&metadata)));
    }
    let error = std::io::Error::last_os_error();
    if error.kind() == std::io::ErrorKind::NotFound {
        Ok(None)
    } else {
        Err(StorageError::from_io(error))
    }
}

fn root_identity_at_path(path: &Path) -> Result<Option<FileIdentity>, StorageError> {
    let root_name = root_directory_name(path)?;
    let parent_path = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(parent_path)
        .map_err(StorageError::from_io)?;
    entry_identity(parent.as_raw_fd(), &root_name)
}

fn is_private_directory(metadata: &std::fs::Metadata) -> bool {
    metadata.is_dir()
        && metadata.mode() & 0o077 == 0
        // SAFETY: `geteuid` has no preconditions and does not dereference pointers.
        && metadata.uid() == unsafe { libc::geteuid() }
}

fn identity_from_metadata(metadata: &std::fs::Metadata) -> FileIdentity {
    FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    }
}

fn identity_from_stat(metadata: &libc::stat) -> FileIdentity {
    FileIdentity {
        device: metadata.st_dev as u64,
        inode: metadata.st_ino,
    }
}

pub fn reconcile_sources(
    catalog: &EditDocument,
    sidecar: &EditDocument,
    xmp: &XmpSnapshot,
) -> Result<Reconciliation, StorageError> {
    reconcile_sources_with_optional_xmp(catalog, sidecar, Some(xmp))
}

pub fn reconcile_sources_with_optional_xmp(
    catalog: &EditDocument,
    sidecar: &EditDocument,
    xmp: Option<&XmpSnapshot>,
) -> Result<Reconciliation, StorageError> {
    let document_conflicts = document_conflicts(catalog, sidecar);
    if !document_conflicts.is_empty() {
        return Ok(Reconciliation::Conflict {
            fields: document_conflicts,
        });
    }

    let Some(xmp) = xmp else {
        return Ok(Reconciliation::CatalogAuthoritative);
    };

    let (catalog_xmp, _) = mirror_properties(catalog);
    let mut conflicts = BTreeSet::new();
    for field in catalog_xmp.keys().chain(xmp.fields.keys()) {
        if catalog_xmp.get(field) != xmp.fields.get(field) {
            conflicts.insert(format!("xmp.{field}"));
        }
    }
    if conflicts.is_empty() {
        Ok(Reconciliation::CatalogAuthoritative)
    } else {
        Ok(Reconciliation::Conflict {
            fields: conflicts.into_iter().collect(),
        })
    }
}

fn mirror_properties(document: &EditDocument) -> (BTreeMap<String, String>, u32) {
    let mut properties = BTreeMap::new();
    let mut skipped = 0_u32;

    for (field, imported) in &document.community {
        if !matches!(
            imported.status,
            ImportStatus::Exact | ImportStatus::Approximated
        ) || !insert_community_property(&mut properties, field, imported)
        {
            skipped = skipped.saturating_add(1);
        }
    }
    for adjustment in &document.adjustments {
        insert_adjustment_properties(&mut properties, adjustment);
    }
    skipped = skipped
        .saturating_add(u32::try_from(document.lightroom.len()).unwrap_or(u32::MAX))
        .saturating_add(u32::try_from(document.lightroom_unknown.len()).unwrap_or(u32::MAX))
        .saturating_add(u32::try_from(document.pro.len()).unwrap_or(u32::MAX));

    (properties, skipped)
}

fn insert_community_property(
    properties: &mut BTreeMap<String, String>,
    field: &str,
    imported: &ImportedField<CoreValue>,
) -> bool {
    let Some(value) = imported.value.as_ref() else {
        return false;
    };
    let (xmp_field, rendered) = match (field, value) {
        ("community.rating", CoreValue::Integer(value)) => ("xmp:Rating", value.to_string()),
        ("community.flag", CoreValue::Text(value)) => ("crs:Pick", value.clone()),
        ("community.label", CoreValue::Text(value)) => ("xmp:Label", value.clone()),
        ("community.title", CoreValue::Text(value)) => ("dc:title", value.clone()),
        ("community.description", CoreValue::Text(value)) => ("dc:description", value.clone()),
        ("community.rights", CoreValue::Text(value)) => ("dc:rights", value.clone()),
        ("community.orientation", CoreValue::Integer(value)) => {
            ("tiff:Orientation", value.to_string())
        }
        ("community.keywords", CoreValue::Text(value)) => ("dc:subject", value.clone()),
        _ => return false,
    };
    properties.insert(xmp_field.to_owned(), rendered);
    true
}

fn insert_adjustment_properties(
    properties: &mut BTreeMap<String, String>,
    adjustment: &Adjustment,
) {
    match adjustment {
        Adjustment::Exposure { value } => {
            properties.insert("crs:Exposure2012".to_owned(), value.to_string());
        }
        Adjustment::Contrast { value } => {
            properties.insert("crs:Contrast2012".to_owned(), value.to_string());
        }
        Adjustment::Highlights { value } => {
            properties.insert("crs:Highlights2012".to_owned(), value.to_string());
        }
        Adjustment::Shadows { value } => {
            properties.insert("crs:Shadows2012".to_owned(), value.to_string());
        }
        Adjustment::WhiteBalance { temperature, tint } => {
            properties.insert("crs:Temperature".to_owned(), temperature.to_string());
            properties.insert("crs:Tint".to_owned(), tint.to_string());
        }
        Adjustment::Crop {
            x,
            y,
            width,
            height,
        } => {
            properties.insert("crs:CropLeft".to_owned(), x.to_string());
            properties.insert("crs:CropTop".to_owned(), y.to_string());
            properties.insert("crs:CropWidth".to_owned(), width.to_string());
            properties.insert("crs:CropHeight".to_owned(), height.to_string());
        }
        Adjustment::Rotation { degrees } => {
            properties.insert("crs:Rotation".to_owned(), degrees.to_string());
        }
        Adjustment::Straighten { degrees } => {
            properties.insert("crs:StraightenAngle".to_owned(), degrees.to_string());
        }
        Adjustment::LensCorrection { profile, amount } => {
            properties.insert("crs:LensProfile".to_owned(), profile.clone());
            properties.insert("crs:LensProfileAmount".to_owned(), amount.to_string());
        }
    }
}

fn document_conflicts(catalog: &EditDocument, sidecar: &EditDocument) -> Vec<String> {
    let mut fields = Vec::new();
    if catalog.adjustments != sidecar.adjustments {
        fields.push("adjustments".to_owned());
    }
    if catalog.community != sidecar.community {
        fields.push("community".to_owned());
    }
    if catalog.lightroom != sidecar.lightroom {
        fields.push("lightroom".to_owned());
    }
    if catalog.lightroom_unknown != sidecar.lightroom_unknown {
        fields.push("lightroom_unknown".to_owned());
    }
    if catalog.pipeline_version != sidecar.pipeline_version {
        fields.push("pipeline_version".to_owned());
    }
    if catalog.pro != sidecar.pro {
        fields.push("pro".to_owned());
    }
    if catalog.schema_version != sidecar.schema_version {
        fields.push("schema_version".to_owned());
    }
    if catalog.source_fingerprint != sidecar.source_fingerprint {
        fields.push("source_fingerprint".to_owned());
    }
    fields.sort();
    fields
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
