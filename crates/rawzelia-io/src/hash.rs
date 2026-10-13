use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

use rawzelia_domain::Fingerprint;
use sha2::{Digest, Sha256};

use crate::{IoError, source_error};

pub fn compute_fingerprint(path: &Path) -> Result<Fingerprint, IoError> {
    let metadata = fs::symlink_metadata(path).map_err(source_error)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(IoError::SourceMissing);
    }

    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    let mut file = options.open(path).map_err(source_error)?;
    fingerprint_open_file(&mut file)
}

pub(crate) fn fingerprint_open_file(file: &mut File) -> Result<Fingerprint, IoError> {
    let metadata = file.metadata().map_err(source_error)?;
    if !metadata.is_file() {
        return Err(IoError::SourceMissing);
    }

    let mut hasher = Sha256::new();
    let mut size = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(source_error)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        size = size
            .checked_add(read as u64)
            .ok_or_else(|| IoError::Io("source size overflow".to_owned()))?;
    }

    if size != metadata.len() {
        return Err(IoError::HashMismatch);
    }

    Ok(Fingerprint {
        size,
        sha256: hasher.finalize().into(),
    })
}
