use std::ffi::OsString;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rawzelia_lightroom_import::{SourceGuardError, validate_source_guard};

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "rawzelia-source-guard-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("temporary directory must be created");
        Self(path)
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

fn directory_entries(path: &Path) -> Vec<OsString> {
    let mut entries = fs::read_dir(path)
        .expect("directory must be readable")
        .map(|entry| entry.expect("directory entry must be readable").file_name())
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

#[test]
fn locked_catalog_is_rejected_without_mutation() {
    let directory = TestDirectory::new();
    let catalog = directory.path().join("catalog.lrcat");
    let lock = directory.path().join("catalog.lrcat.lock");
    fs::write(&catalog, b"read-only catalog fixture").expect("catalog fixture must be written");
    fs::write(&lock, b"active Lightroom process").expect("lock fixture must be written");
    let before_contents = fs::read(&catalog).expect("catalog fixture must be readable");
    let before_modified = fs::metadata(&catalog)
        .expect("catalog metadata must be readable")
        .modified()
        .expect("catalog modification time must be readable");
    let before_entries = directory_entries(directory.path());

    assert_eq!(
        validate_source_guard(&catalog),
        Err(SourceGuardError::Locked)
    );

    assert_eq!(
        fs::read(&catalog).expect("catalog fixture must remain readable"),
        before_contents
    );
    assert_eq!(
        fs::metadata(&catalog)
            .expect("catalog metadata must remain readable")
            .modified()
            .expect("catalog modification time must remain readable"),
        before_modified
    );
    assert_eq!(directory_entries(directory.path()), before_entries);
}

#[cfg(unix)]
#[test]
fn symlink_to_active_catalog_is_rejected() {
    let directory = TestDirectory::new();
    let source = directory.path().join("source");
    let selection = directory.path().join("selection");
    fs::create_dir_all(&source).expect("source directory must be created");
    fs::create_dir_all(&selection).expect("selection directory must be created");

    let catalog = source.join("catalog.lrcat");
    let active_marker = source.join("catalog.lrcat-wal");
    let catalog_link = selection.join("catalog.lrcat");
    fs::write(&catalog, b"active catalog fixture").expect("catalog fixture must be written");
    fs::write(&active_marker, b"active transaction").expect("active marker must be written");
    symlink(&catalog, &catalog_link).expect("catalog symlink must be created");
    let before_contents = fs::read(&catalog).expect("catalog fixture must be readable");
    let before_entries = directory_entries(&source);

    assert_eq!(
        validate_source_guard(&catalog_link),
        Err(SourceGuardError::Incomplete)
    );

    assert_eq!(
        fs::read(&catalog).expect("catalog fixture must remain readable"),
        before_contents
    );
    assert_eq!(directory_entries(&source), before_entries);
    assert_eq!(
        directory_entries(&selection),
        vec![OsString::from("catalog.lrcat")]
    );
}
