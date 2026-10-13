use std::cell::{Cell, RefCell};
use std::fs::{self, OpenOptions};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use rawzelia_io::{
    ImportProgressState, ImportRequest, IoError, PathTemplate, execute_ingest, plan_ingest,
};

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

fn request(
    source_root: &Path,
    selected_files: Vec<PathBuf>,
    destination_root: &Path,
) -> ImportRequest {
    ImportRequest {
        source_root: source_root.to_path_buf(),
        selected_files,
        destination_root: destination_root.to_path_buf(),
        template: PathTemplate::default(),
        session_name: "Session test".to_owned(),
    }
}

fn volume_names_collide(root: &Path, first: &str, second: &str) -> bool {
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let probe = root.join(format!(
        ".rawzelia-name-probe-{}-{sequence}",
        std::process::id()
    ));
    fs::create_dir(&probe).expect("probe directory should be created");
    fs::write(probe.join(first), b"probe").expect("first probe name should be created");
    let collision = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(probe.join(second))
    {
        Ok(_) => false,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => true,
        Err(error) => panic!("second probe name failed unexpectedly: {error}"),
    };
    fs::remove_dir_all(&probe).expect("probe directory should be removed");
    collision
}

fn find_staged_temporary(directory: &Path) -> PathBuf {
    fn find(directory: &Path) -> Option<PathBuf> {
        for entry in fs::read_dir(directory).ok()? {
            let path = entry.ok()?.path();
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            if path.is_file() && name.starts_with(".rawzelia-tmp-") {
                return Some(path);
            }
            if path.is_dir()
                && let Some(temporary) = find(&path)
            {
                return Some(temporary);
            }
        }
        None
    }

    find(directory).expect("owned staged temporary should exist")
}

#[test]
fn duplicate_ingest_is_idempotent_and_interruption_is_resumable() {
    let fixture = TestDirectory::new("resume");
    let source_root = fixture.path().join("source");
    let destination_root = fixture.path().join("destination");
    fs::create_dir(&source_root).expect("source directory should be created");
    fs::create_dir(&destination_root).expect("destination directory should be created");

    let first_source = source_root.join("DSC_0001.NEF");
    let second_source = source_root.join("DSC_0002.NEF");
    let first_bytes = b"first controlled raw payload";
    let second_bytes = b"second controlled raw payload";
    fs::write(&first_source, first_bytes).expect("first fixture should be written");
    fs::write(&second_source, second_bytes).expect("second fixture should be written");

    let plan = plan_ingest(&request(
        &source_root,
        vec![second_source.clone(), first_source.clone()],
        &destination_root,
    ))
    .expect("ingest should be planned");
    assert_eq!(plan.items.len(), 2);
    assert_eq!(
        plan.items[0].source,
        fs::canonicalize(&first_source).unwrap()
    );
    assert_eq!(
        plan.items[1].source,
        fs::canonicalize(&second_source).unwrap()
    );

    let cancel = Cell::new(false);
    let mut first_progress = Vec::new();
    let interrupted = execute_ingest(
        &plan,
        &mut |event| {
            if event.state == ImportProgressState::Verified {
                cancel.set(true);
            }
            first_progress.push(event);
        },
        &|| cancel.get(),
    );

    assert_eq!(interrupted, Err(IoError::Cancelled));
    assert_eq!(fs::read(&plan.items[0].destination).unwrap(), first_bytes);
    assert!(!plan.items[1].destination.exists());
    assert_eq!(fs::read(&plan.items[0].source).unwrap(), first_bytes);
    assert_eq!(fs::read(&plan.items[1].source).unwrap(), second_bytes);
    assert!(
        first_progress
            .iter()
            .any(|event| { event.completed == 1 && event.state == ImportProgressState::Verified })
    );

    let mut resumed_progress = Vec::new();
    let summary = execute_ingest(&plan, &mut |event| resumed_progress.push(event), &|| false)
        .expect("interrupted ingest should resume");

    assert_eq!(summary.imported, 1);
    assert_eq!(summary.duplicates, 1);
    assert_eq!(summary.failed, 0);
    assert!(summary.resumed);
    assert_eq!(fs::read(&plan.items[0].destination).unwrap(), first_bytes);
    assert_eq!(fs::read(&plan.items[1].destination).unwrap(), second_bytes);
    assert!(
        resumed_progress
            .iter()
            .any(|event| { event.completed == 1 && event.state == ImportProgressState::Duplicate })
    );
    assert!(
        resumed_progress
            .iter()
            .any(|event| { event.completed == 2 && event.state == ImportProgressState::Verified })
    );
}

#[test]
fn selected_file_outside_source_is_rejected() {
    let fixture = TestDirectory::new("outside-source");
    let source_root = fixture.path().join("source");
    let destination_root = fixture.path().join("destination");
    let outside = fixture.path().join("outside.NEF");
    fs::create_dir(&source_root).unwrap();
    fs::create_dir(&destination_root).unwrap();
    fs::write(&outside, b"outside source").unwrap();

    let result = plan_ingest(&request(
        &source_root,
        vec![outside.clone()],
        &destination_root,
    ));

    assert!(matches!(result, Err(IoError::SourceMissing)));
    assert_eq!(fs::read(outside).unwrap(), b"outside source");
}

#[test]
fn divergent_destination_collision_is_preserved() {
    let fixture = TestDirectory::new("collision");
    let source_root = fixture.path().join("source");
    let destination_root = fixture.path().join("destination");
    fs::create_dir(&source_root).unwrap();
    fs::create_dir(&destination_root).unwrap();
    let source = source_root.join("DSC_0001.NEF");
    fs::write(&source, b"source payload").unwrap();
    let plan = plan_ingest(&request(&source_root, vec![source], &destination_root)).unwrap();
    fs::create_dir_all(plan.items[0].destination.parent().unwrap()).unwrap();
    fs::write(&plan.items[0].destination, b"unrelated destination").unwrap();
    let mut progress = Vec::new();

    let result = execute_ingest(&plan, &mut |event| progress.push(event), &|| false);

    assert_eq!(result, Err(IoError::DestinationUnavailable));
    assert_eq!(
        fs::read(&plan.items[0].destination).unwrap(),
        b"unrelated destination"
    );
    assert!(!progress.iter().any(|event| {
        matches!(
            event.state,
            ImportProgressState::Verified | ImportProgressState::Duplicate
        )
    }));
}

#[test]
fn hash_mismatch_never_publishes_a_destination() {
    let fixture = TestDirectory::new("hash-mismatch");
    let source_root = fixture.path().join("source");
    let destination_root = fixture.path().join("destination");
    fs::create_dir(&source_root).unwrap();
    fs::create_dir(&destination_root).unwrap();
    let source = source_root.join("DSC_0001.NEF");
    fs::write(&source, b"controlled payload").unwrap();
    let mut plan = plan_ingest(&request(&source_root, vec![source], &destination_root)).unwrap();
    plan.items[0].fingerprint.sha256[0] ^= 0xff;
    let destination = plan.items[0].destination.clone();

    let result = execute_ingest(&plan, &mut |_| {}, &|| false);

    assert_eq!(result, Err(IoError::HashMismatch));
    assert!(!destination.exists());
}

#[test]
fn post_copy_cancellation_does_not_publish() {
    let fixture = TestDirectory::new("post-copy-cancel");
    let source_root = fixture.path().join("source");
    let destination_root = fixture.path().join("destination");
    fs::create_dir(&source_root).unwrap();
    fs::create_dir(&destination_root).unwrap();
    let source = source_root.join("DSC_0001.NEF");
    let source_bytes = b"small payload copied in one read";
    fs::write(&source, source_bytes).unwrap();
    let plan = plan_ingest(&request(
        &source_root,
        vec![source.clone()],
        &destination_root,
    ))
    .unwrap();
    let calls = Cell::new(0_u8);
    let cancellation_armed = Cell::new(false);

    let result = execute_ingest(&plan, &mut |_| {}, &|| {
        let call = calls.get();
        calls.set(call + 1);
        if call == 2 {
            cancellation_armed.set(true);
            false
        } else {
            cancellation_armed.get()
        }
    });

    assert_eq!(result, Err(IoError::Cancelled));
    assert!(cancellation_armed.get());
    assert!(!plan.items[0].destination.exists());
    assert_eq!(fs::read(source).unwrap(), source_bytes);
}

#[test]
fn substitution_after_validation_before_cleanup_preserves_third_party() {
    let fixture = TestDirectory::new("cleanup-race");
    let source_root = fixture.path().join("source");
    let destination_root = fixture.path().join("destination");
    fs::create_dir(&source_root).unwrap();
    fs::create_dir(&destination_root).unwrap();
    let source = source_root.join("DSC_0001.NEF");
    let source_bytes = b"verified source payload";
    let third_party_bytes = b"third-party bytes must not be unlinked";
    fs::write(&source, source_bytes).unwrap();
    let plan = plan_ingest(&request(
        &source_root,
        vec![source.clone()],
        &destination_root,
    ))
    .unwrap();
    let calls = Cell::new(0_u8);
    let staged_path = RefCell::new(None);
    let third_party_link = fixture.path().join("third-party-owned.NEF");

    let result = execute_ingest(&plan, &mut |_| {}, &|| {
        let call = calls.get();
        calls.set(call + 1);
        if call != 3 {
            return false;
        }
        let staged = find_staged_temporary(&plan.destination_root);
        assert_eq!(
            fs::metadata(staged.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&staged).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::hard_link(&staged, &third_party_link).unwrap();
        fs::remove_file(&staged).unwrap();
        fs::hard_link(&third_party_link, &staged).unwrap();
        fs::write(&third_party_link, third_party_bytes).unwrap();
        staged_path.replace(Some(staged));
        true
    });

    assert_eq!(result, Err(IoError::Cancelled));
    let staged = staged_path.borrow();
    let staged = staged.as_ref().expect("substitution should run");
    assert_eq!(fs::read(staged).unwrap(), third_party_bytes);
    assert_eq!(fs::read(&third_party_link).unwrap(), third_party_bytes);
    assert_eq!(fs::read(source).unwrap(), source_bytes);
    assert!(!plan.items[0].destination.exists());
}

#[test]
fn substitution_after_validation_before_rename_is_not_published() {
    let fixture = TestDirectory::new("publish-substitution-race");
    let source_root = fixture.path().join("source");
    let destination_root = fixture.path().join("destination");
    fs::create_dir(&source_root).unwrap();
    fs::create_dir(&destination_root).unwrap();
    let source = source_root.join("DSC_0001.NEF");
    let source_bytes = b"verified source payload";
    let third_party_bytes = b"unverified replacement must not publish";
    fs::write(&source, source_bytes).unwrap();
    let plan = plan_ingest(&request(
        &source_root,
        vec![source.clone()],
        &destination_root,
    ))
    .unwrap();
    let calls = Cell::new(0_u8);
    let replacement_path = RefCell::new(None);

    let result = execute_ingest(&plan, &mut |_| {}, &|| {
        let call = calls.get();
        calls.set(call + 1);
        if call != 3 {
            return false;
        }
        let staged = find_staged_temporary(&plan.destination_root);
        fs::remove_file(&staged).unwrap();
        fs::write(&staged, third_party_bytes).unwrap();
        replacement_path.replace(Some(staged));
        false
    });

    assert_eq!(result, Err(IoError::DestinationUnavailable));
    let replacement = replacement_path.borrow();
    let replacement = replacement.as_ref().expect("substitution should run");
    assert_eq!(fs::read(replacement).unwrap(), third_party_bytes);
    assert_eq!(fs::read(source).unwrap(), source_bytes);
    assert!(!plan.items[0].destination.exists());
}

#[test]
fn hardlinked_temporary_modified_before_publication_is_rejected() {
    let fixture = TestDirectory::new("hardlink-race");
    let source_root = fixture.path().join("source");
    let destination_root = fixture.path().join("destination");
    fs::create_dir(&source_root).unwrap();
    fs::create_dir(&destination_root).unwrap();
    let source = source_root.join("DSC_0001.NEF");
    let source_bytes = b"verified source payload";
    let third_party_bytes = b"hardlink mutation must not publish";
    fs::write(&source, source_bytes).unwrap();
    let plan = plan_ingest(&request(
        &source_root,
        vec![source.clone()],
        &destination_root,
    ))
    .unwrap();
    let calls = Cell::new(0_u8);
    let hardlink = fixture.path().join("concurrent-hardlink.NEF");

    let result = execute_ingest(&plan, &mut |_| {}, &|| {
        let call = calls.get();
        calls.set(call + 1);
        if call != 3 {
            return false;
        }
        let staged = find_staged_temporary(&plan.destination_root);
        fs::hard_link(staged, &hardlink).unwrap();
        fs::write(&hardlink, third_party_bytes).unwrap();
        false
    });

    assert_eq!(result, Err(IoError::DestinationUnavailable));
    assert_eq!(fs::read(&hardlink).unwrap(), third_party_bytes);
    assert_eq!(fs::read(source).unwrap(), source_bytes);
    assert!(!plan.items[0].destination.exists());
}

#[test]
fn substituted_temporary_is_not_removed_during_cleanup() {
    let fixture = TestDirectory::new("temporary-substitution");
    let source_root = fixture.path().join("source");
    let destination_root = fixture.path().join("destination");
    fs::create_dir(&source_root).unwrap();
    fs::create_dir(&destination_root).unwrap();
    let source = source_root.join("DSC_0001.NEF");
    let source_bytes = b"source remains byte-for-byte unchanged";
    let third_party_bytes = b"third-party replacement must survive";
    fs::write(&source, source_bytes).unwrap();
    let plan = plan_ingest(&request(
        &source_root,
        vec![source.clone()],
        &destination_root,
    ))
    .unwrap();
    let calls = Cell::new(0_u8);
    let replacement_path = RefCell::new(None);

    let result = execute_ingest(&plan, &mut |_| {}, &|| {
        let call = calls.get();
        calls.set(call + 1);
        if call != 1 {
            return false;
        }
        let temporary = find_staged_temporary(&plan.destination_root);
        fs::remove_file(&temporary).unwrap();
        fs::write(&temporary, third_party_bytes).unwrap();
        replacement_path.replace(Some(temporary));
        true
    });

    assert_eq!(result, Err(IoError::Cancelled));
    let replacement = replacement_path.borrow();
    let replacement = replacement
        .as_ref()
        .expect("replacement should be installed");
    assert_eq!(fs::read(replacement).unwrap(), third_party_bytes);
    assert_eq!(fs::read(source).unwrap(), source_bytes);
    assert!(!plan.items[0].destination.exists());
}

#[test]
fn destination_case_collisions_are_rejected_during_planning() {
    let fixture = TestDirectory::new("case-collision");
    let source_root = fixture.path().join("source");
    let destination_root = fixture.path().join("destination");
    let first_dir = source_root.join("first");
    let second_dir = source_root.join("second");
    fs::create_dir_all(&first_dir).unwrap();
    fs::create_dir(&second_dir).unwrap();
    fs::create_dir(&destination_root).unwrap();
    let first = first_dir.join("A.NEF");
    let second = second_dir.join("a.nef");
    fs::write(&first, b"first source").unwrap();
    fs::write(&second, b"second source").unwrap();

    let result = plan_ingest(&request(
        &source_root,
        vec![first.clone(), second.clone()],
        &destination_root,
    ));

    if volume_names_collide(&destination_root, "A.NEF", "a.nef") {
        assert!(matches!(result, Err(IoError::DestinationUnavailable)));
    } else {
        assert_eq!(result.unwrap().items.len(), 2);
    }
    assert_eq!(fs::read(first).unwrap(), b"first source");
    assert_eq!(fs::read(second).unwrap(), b"second source");
}

#[test]
fn destination_unicode_collisions_are_rejected_during_planning() {
    let fixture = TestDirectory::new("unicode-collision");
    let source_root = fixture.path().join("source");
    let destination_root = fixture.path().join("destination");
    let first_dir = source_root.join("first");
    let second_dir = source_root.join("second");
    fs::create_dir_all(&first_dir).unwrap();
    fs::create_dir(&second_dir).unwrap();
    fs::create_dir(&destination_root).unwrap();
    let composed_name = "caf\u{00e9}.NEF";
    let decomposed_name = "cafe\u{0301}.NEF";
    let first = first_dir.join(composed_name);
    let second = second_dir.join(decomposed_name);
    fs::write(&first, b"first source").unwrap();
    fs::write(&second, b"second source").unwrap();

    let result = plan_ingest(&request(
        &source_root,
        vec![first.clone(), second.clone()],
        &destination_root,
    ));

    if volume_names_collide(&destination_root, composed_name, decomposed_name) {
        assert!(matches!(result, Err(IoError::DestinationUnavailable)));
    } else {
        assert_eq!(result.unwrap().items.len(), 2);
    }
    assert_eq!(fs::read(first).unwrap(), b"first source");
    assert_eq!(fs::read(second).unwrap(), b"second source");
}

#[test]
fn source_nested_in_destination_is_rejected() {
    let fixture = TestDirectory::new("source-in-destination");
    let destination_root = fixture.path().join("destination");
    let source_root = destination_root.join("mounted-card");
    fs::create_dir_all(&source_root).unwrap();
    let source = source_root.join("DSC_0001.NEF");
    let source_bytes = b"source nested under destination";
    fs::write(&source, source_bytes).unwrap();

    let result = plan_ingest(&request(
        &source_root,
        vec![source.clone()],
        &destination_root,
    ));

    assert!(matches!(result, Err(IoError::DestinationUnavailable)));
    assert_eq!(fs::read(source).unwrap(), source_bytes);
}

#[cfg(unix)]
#[test]
fn source_and_destination_symlinks_are_rejected_without_mutation() {
    use std::os::unix::fs::symlink;

    let fixture = TestDirectory::new("symlinks");
    let source_root = fixture.path().join("source");
    let destination_root = fixture.path().join("destination");
    fs::create_dir(&source_root).unwrap();
    fs::create_dir(&destination_root).unwrap();
    let real_source = source_root.join("real.NEF");
    let linked_source = source_root.join("linked.NEF");
    fs::write(&real_source, b"source payload").unwrap();
    symlink(&real_source, &linked_source).unwrap();

    assert!(matches!(
        plan_ingest(&request(
            &source_root,
            vec![linked_source],
            &destination_root
        )),
        Err(IoError::SourceMissing)
    ));

    let plan = plan_ingest(&request(
        &source_root,
        vec![real_source.clone()],
        &destination_root,
    ))
    .unwrap();
    fs::create_dir_all(plan.items[0].destination.parent().unwrap()).unwrap();
    let unrelated = fixture.path().join("unrelated.NEF");
    fs::write(&unrelated, b"must remain unchanged").unwrap();
    symlink(&unrelated, &plan.items[0].destination).unwrap();

    assert_eq!(
        execute_ingest(&plan, &mut |_| {}, &|| false),
        Err(IoError::DestinationUnavailable)
    );
    assert_eq!(fs::read(&unrelated).unwrap(), b"must remain unchanged");
    assert_eq!(fs::read(&real_source).unwrap(), b"source payload");
}
