mod backup;
mod catalog;
mod migrations;
mod sidecar;
mod xmp;

pub use backup::{
    BackupError, BackupReport, RestoreError, RestoreReport, create_backup, restore_backup,
};
pub use catalog::{CatalogStore, SqliteCatalog, StorageError};
pub use sidecar::{read_sidecar, write_sidecar_atomic};
pub use xmp::{
    Reconciliation, XmpSnapshot, XmpWriteReport, reconcile_sources,
    reconcile_sources_with_optional_xmp, write_xmp_mirror,
};
