use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use rawzelia_domain::{
    Adjustment, CoreValue, EditDocument, FieldProvenance, Fingerprint, ImportStatus, ImportedField,
    LightroomSchemaId,
};
use rawzelia_storage::{StorageError, read_sidecar, write_sidecar_atomic, write_xmp_mirror};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> Self {
        let base = std::env::temp_dir();
        loop {
            let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = base.join(format!(
                "rawzelia-storage-{label}-{}-{sequence}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("test directory must be created: {error}"),
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

fn field(path: &str, value: CoreValue) -> ImportedField<CoreValue> {
    ImportedField {
        value: Some(value),
        status: ImportStatus::Exact,
        provenance: FieldProvenance {
            source_schema: LightroomSchemaId("lightroom-latest-corpus".to_owned()),
            source_id: "asset-0001".to_owned(),
            field_path: path.to_owned(),
            artifact: "catalog.lrcat".to_owned(),
        },
    }
}

fn document(pipeline_version: &str) -> EditDocument {
    let mut community = BTreeMap::new();
    community.insert(
        "community.rating".to_owned(),
        field("community.rating", CoreValue::Integer(5)),
    );
    community.insert(
        "community.title".to_owned(),
        field(
            "community.title",
            CoreValue::Text("Harbor & hills".to_owned()),
        ),
    );
    community.insert(
        "community.private_note".to_owned(),
        field(
            "community.private_note",
            CoreValue::Text("must-not-enter-xmp".to_owned()),
        ),
    );

    let mut pro = BTreeMap::new();
    pro.insert(
        "pro.mask".to_owned(),
        field("pro.mask", CoreValue::Text("private-mask".to_owned())),
    );

    EditDocument {
        schema_version: 1,
        source_fingerprint: Fingerprint {
            size: 2048,
            sha256: [9; 32],
        },
        pipeline_version: pipeline_version.to_owned(),
        adjustments: vec![Adjustment::Exposure { value: 0.5 }],
        community,
        lightroom: Vec::new(),
        lightroom_unknown: Vec::new(),
        pro,
    }
}

#[test]
fn sidecar_write_is_atomic_and_round_trips() {
    let temp = TestDirectory::new("sidecar-xmp");
    let sidecar_path = temp.path().join("asset.nef.rawzelia");
    fs::write(&sidecar_path, b"incomplete previous content")
        .expect("pre-existing sidecar must be written");

    let first = document("0.1.0");
    write_sidecar_atomic(&sidecar_path, &first).expect("sidecar must replace atomically");
    assert_eq!(
        read_sidecar(&sidecar_path).expect("sidecar must read"),
        Some(first)
    );

    let second = document("0.1.1");
    write_sidecar_atomic(&sidecar_path, &second).expect("sidecar update must be atomic");
    assert_eq!(
        read_sidecar(&sidecar_path).expect("updated sidecar must read"),
        Some(second.clone())
    );

    let xmp_path = temp.path().join("asset.nef.rawzelia.xmp");
    let report = write_xmp_mirror(&xmp_path, &second).expect("XMP mirror must write");
    assert_eq!(report.mirrored_fields, 3);
    assert_eq!(report.skipped_fields, 2);

    let xmp = fs::read_to_string(&xmp_path).expect("XMP mirror must be readable");
    assert!(xmp.contains("<xmp:Rating>5</xmp:Rating>"));
    assert!(xmp.contains("<dc:title>Harbor &amp; hills</dc:title>"));
    assert!(xmp.contains("<crs:Exposure2012>0.5</crs:Exposure2012>"));
    assert!(!xmp.contains("must-not-enter-xmp"));
    assert!(!xmp.contains("private-mask"));

    let entries: Vec<_> = fs::read_dir(temp.path())
        .expect("directory must be readable")
        .map(|entry| entry.expect("entry must be readable").file_name())
        .collect();
    assert_eq!(entries.len(), 2, "atomic writes must leave no temp files");
}

#[test]
fn lightroom_xmp_source_is_rejected_without_mutation() {
    let temp = TestDirectory::new("xmp-source-read-only");
    let source_path = temp.path().join("asset.nef.xmp");
    let source_bytes = b"<x:xmpmeta>lightroom-source</x:xmpmeta>";
    fs::write(&source_path, source_bytes).expect("source XMP must be written");

    let result = write_xmp_mirror(&source_path, &document("0.1.0"));

    assert_eq!(
        fs::read(&source_path).expect("source XMP must remain readable"),
        source_bytes,
        "source XMP must remain byte-for-byte unchanged"
    );
    assert_eq!(
        result,
        Err(StorageError::Io {
            code: "unmanaged_xmp_target".to_owned(),
            message_key: "storage.xmp.unmanaged_target".to_owned(),
        })
    );
}

#[test]
fn unmanaged_existing_rawzelia_xmp_is_rejected_without_mutation() {
    let temp = TestDirectory::new("unmanaged-rawzelia-xmp");
    let source_path = temp.path().join("asset.nef.rawzelia.xmp");
    let source_bytes = b"<x:xmpmeta>third-party-source</x:xmpmeta>";
    fs::write(&source_path, source_bytes).expect("third-party XMP must be written");

    let result = write_xmp_mirror(&source_path, &document("0.1.0"));

    assert_eq!(
        fs::read(&source_path).expect("third-party XMP must remain readable"),
        source_bytes,
        "an unmarked XMP must remain byte-for-byte unchanged"
    );
    assert_eq!(
        result,
        Err(StorageError::Io {
            code: "unmanaged_xmp_target".to_owned(),
            message_key: "storage.xmp.unmanaged_target".to_owned(),
        })
    );
}

#[test]
fn rawzelia_generated_xmp_can_be_written_again() {
    let temp = TestDirectory::new("managed-rawzelia-xmp");
    let path = temp.path().join("asset.nef.rawzelia.xmp");
    write_xmp_mirror(&path, &document("0.1.0")).expect("initial mirror must write");

    let initial = fs::read_to_string(&path).expect("initial mirror must be readable");
    assert!(initial.starts_with(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- rawzelia:xmp-mirror:v1 -->\n"
    ));

    let mut updated = document("0.1.1");
    updated.adjustments = vec![Adjustment::Exposure { value: 1.25 }];
    write_xmp_mirror(&path, &updated).expect("owned mirror must allow a second write");

    let rewritten = fs::read_to_string(&path).expect("updated mirror must be readable");
    assert!(rewritten.starts_with(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- rawzelia:xmp-mirror:v1 -->\n"
    ));
    assert!(rewritten.contains("<crs:Exposure2012>1.25</crs:Exposure2012>"));
    assert!(!rewritten.contains("<crs:Exposure2012>0.5</crs:Exposure2012>"));
}
