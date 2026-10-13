use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use rawzelia_domain::EditDocument;

use crate::StorageError;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub fn read_sidecar(path: &Path) -> Result<Option<EditDocument>, StorageError> {
    require_extension(path, "rawzelia")?;
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(StorageError::from_io(error)),
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| StorageError::serialization("invalid_sidecar"))
}

pub fn write_sidecar_atomic(path: &Path, document: &EditDocument) -> Result<(), StorageError> {
    require_extension(path, "rawzelia")?;
    let bytes = serde_json::to_vec_pretty(document)
        .map_err(|_| StorageError::serialization("serialize_sidecar"))?;
    write_bytes_atomic(path, &bytes)
}

pub(crate) fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<(), StorageError> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(StorageError::from_io)?;
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("rawzelia");

    let (temporary_path, mut temporary_file) = loop {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(
            ".{file_name}.tmp-{}-{sequence}",
            std::process::id()
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => break (candidate, file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(StorageError::from_io(error)),
        }
    };

    let write_result = (|| {
        temporary_file
            .write_all(bytes)
            .map_err(StorageError::from_io)?;
        temporary_file.flush().map_err(StorageError::from_io)?;
        temporary_file.sync_all().map_err(StorageError::from_io)?;
        drop(temporary_file);
        fs::rename(&temporary_path, path).map_err(StorageError::from_io)?;
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(StorageError::from_io)
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    write_result
}

fn require_extension(path: &Path, extension: &str) -> Result<(), StorageError> {
    if path.extension().and_then(|value| value.to_str()) == Some(extension) {
        Ok(())
    } else {
        Err(StorageError::Io {
            code: format!("invalid_{extension}_extension"),
            message_key: "storage.invalid_extension".to_owned(),
        })
    }
}
