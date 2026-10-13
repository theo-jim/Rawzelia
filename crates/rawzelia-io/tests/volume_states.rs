use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use rawzelia_domain::AssetAvailability;
use rawzelia_io::classify_volume;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> Self {
        loop {
            let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "rawzelia-io-{label}-{}-{sequence}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("failed to create test directory: {error}"),
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

#[test]
fn offline_volume_is_not_marked_present() {
    let fixture = TestDirectory::new("offline-volume");
    let offline = fixture.path().join("disconnected-card");

    let first = classify_volume(&offline).expect("offline volume should be classified");
    let second = classify_volume(&offline).expect("classification should be stable");

    assert_eq!(first.root, offline);
    assert_eq!(first.availability, AssetAvailability::VolumeOffline);
    assert_ne!(first.availability, AssetAvailability::Present);
    assert_eq!(first.location_id, second.location_id);
}

#[test]
fn readable_directory_is_marked_present() {
    let fixture = TestDirectory::new("present-volume");

    let volume = classify_volume(fixture.path()).expect("volume should be readable");

    assert_eq!(volume.availability, AssetAvailability::Present);
    assert_eq!(volume.root, fixture.path());
}

#[cfg(unix)]
#[test]
fn symlinked_volume_is_not_marked_present() {
    use std::os::unix::fs::symlink;

    let fixture = TestDirectory::new("symlink-volume");
    let target = fixture.path().join("target");
    let alias = fixture.path().join("alias");
    fs::create_dir(&target).expect("target directory should be created");
    symlink(&target, &alias).expect("volume symlink should be created");

    let volume = classify_volume(&alias).expect("symlink should be conservatively classified");

    assert_eq!(volume.availability, AssetAvailability::Moved);
    assert_ne!(volume.availability, AssetAvailability::Present);
}
