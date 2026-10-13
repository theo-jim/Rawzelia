mod hash;
mod ingest;
mod path_template;
mod volume;

use std::fmt;

pub use hash::compute_fingerprint;
pub use ingest::{
    ImportProgress, ImportProgressState, ImportRequest, ImportSummary, IngestPlan, PlannedIngest,
    execute_ingest, plan_ingest,
};
pub use path_template::PathTemplate;
pub use volume::{VolumeInfo, classify_volume};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IoError {
    SourceMissing,
    DestinationUnavailable,
    HashMismatch,
    Cancelled,
    PermissionDenied,
    Io(String),
}

impl fmt::Display for IoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceMissing => formatter.write_str("source missing"),
            Self::DestinationUnavailable => formatter.write_str("destination unavailable"),
            Self::HashMismatch => formatter.write_str("hash mismatch"),
            Self::Cancelled => formatter.write_str("cancelled"),
            Self::PermissionDenied => formatter.write_str("permission denied"),
            Self::Io(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for IoError {}

pub(crate) fn source_error(error: std::io::Error) -> IoError {
    match error.kind() {
        std::io::ErrorKind::NotFound => IoError::SourceMissing,
        std::io::ErrorKind::PermissionDenied => IoError::PermissionDenied,
        _ => IoError::Io(error.to_string()),
    }
}

pub(crate) fn destination_error(error: std::io::Error) -> IoError {
    #[cfg(unix)]
    if error.raw_os_error() == Some(libc::ELOOP) {
        return IoError::DestinationUnavailable;
    }
    match error.kind() {
        std::io::ErrorKind::PermissionDenied => IoError::PermissionDenied,
        std::io::ErrorKind::NotFound
        | std::io::ErrorKind::AlreadyExists
        | std::io::ErrorKind::NotADirectory
        | std::io::ErrorKind::IsADirectory => IoError::DestinationUnavailable,
        _ => IoError::Io(error.to_string()),
    }
}
