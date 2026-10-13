use std::cell::Cell;
use std::fs;
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
