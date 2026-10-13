use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceGuardError {
    Missing,
    Active,
    Locked,
    Incomplete,
    Io(String),
}

pub fn validate_source_guard(path: &Path) -> Result<(), SourceGuardError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Err(SourceGuardError::Missing),
        Err(error) => return Err(SourceGuardError::Io(error.to_string())),
    };

    if metadata.file_type().is_symlink() {
        return Err(SourceGuardError::Incomplete);
    }
    if companion_exists(&append_suffix(path, ".lock"))? {
        return Err(SourceGuardError::Locked);
    }
    if companion_exists(&append_suffix(path, "-wal"))?
        || companion_exists(&append_suffix(path, "-shm"))?
    {
        return Err(SourceGuardError::Active);
    }
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(SourceGuardError::Incomplete);
    }

    Ok(())
}

fn append_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_owned();
    value.push(suffix);
    PathBuf::from(value)
}

fn companion_exists(path: &Path) -> Result<bool, SourceGuardError> {
    match fs::metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(SourceGuardError::Io(error.to_string())),
    }
}
