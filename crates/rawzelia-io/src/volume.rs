use std::fs;
use std::path::{Path, PathBuf};

use rawzelia_domain::{AssetAvailability, LocationId};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::IoError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeInfo {
    pub root: PathBuf,
    pub location_id: LocationId,
    pub availability: AssetAvailability,
}

pub fn classify_volume(path: &Path) -> Result<VolumeInfo, IoError> {
    let root = absolute_path(path)?;
    let availability = match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => AssetAvailability::Moved,
        Ok(metadata) if metadata.is_dir() => directory_availability(path)?,
        Ok(_) => AssetAvailability::Missing,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            AssetAvailability::VolumeOffline
        }
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            AssetAvailability::PermissionDenied
        }
        Err(error) => return Err(IoError::Io(error.to_string())),
    };

    Ok(VolumeInfo {
        root,
        location_id: location_id(path)?,
        availability,
    })
}

fn directory_availability(path: &Path) -> Result<AssetAvailability, IoError> {
    match fs::read_dir(path) {
        Ok(mut entries) => match entries.next() {
            Some(Err(error)) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                Ok(AssetAvailability::PermissionDenied)
            }
            Some(Err(error)) => Err(IoError::Io(error.to_string())),
            Some(Ok(_)) | None => Ok(AssetAvailability::Present),
        },
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            Ok(AssetAvailability::PermissionDenied)
        }
        Err(error) => Err(IoError::Io(error.to_string())),
    }
}

fn absolute_path(path: &Path) -> Result<PathBuf, IoError> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::env::current_dir()
            .map(|directory| directory.join(path))
            .map_err(|error| IoError::Io(error.to_string()))
    }
}

fn location_id(path: &Path) -> Result<LocationId, IoError> {
    let root = absolute_path(path)?;
    let mut hasher = Sha256::new();
    hasher.update(b"rawzelia.location.v1\0");
    hasher.update(root.to_string_lossy().as_bytes());
    let digest: [u8; 32] = hasher.finalize().into();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Ok(LocationId(Uuid::from_bytes(bytes)))
}
