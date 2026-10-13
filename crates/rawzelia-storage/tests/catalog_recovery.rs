use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use rawzelia_domain::{
    AssetAvailability, AssetId, AssetMetadata, AssetRecord, EditDocument, Fingerprint, Flag,
    LabelColor, LocationId, Rating,
};
use rawzelia_storage::{CatalogStore, SqliteCatalog, StorageError};
use rusqlite::Connection;

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

fn asset(asset_id: AssetId, location_id: LocationId) -> AssetRecord {
    AssetRecord {
        asset_id,
        location_id,
        original_path: PathBuf::from("/Volumes/CARD/DCIM/100D750/DSC_0001.NEF"),
        relative_path: "100D750/DSC_0001.NEF".to_owned(),
        captured_at: None,
        metadata: AssetMetadata {
            width: 6016,
            height: 4016,
            orientation: 1,
            camera_model: Some("Nikon D750".to_owned()),
            lens_model: None,
            keywords: vec!["session".to_owned()],
        },
        fingerprint: Fingerprint {
            size: 1024,
            sha256: [7; 32],
        },
        availability: AssetAvailability::Present,
        rating: Rating::Four,
        flag: Flag::Pick,
        label: LabelColor::Green,
    }
}

fn document(pipeline_version: &str) -> EditDocument {
    EditDocument {
        schema_version: 1,
        source_fingerprint: Fingerprint {
            size: 1024,
            sha256: [7; 32],
        },
        pipeline_version: pipeline_version.to_owned(),
        adjustments: Vec::new(),
        community: Default::default(),
        lightroom: Vec::new(),
        lightroom_unknown: Vec::new(),
        pro: Default::default(),
    }
}

fn parse_asset_id(value: &str) -> AssetId {
    serde_json::from_str(&format!("\"{value}\"")).expect("asset UUID must deserialize")
}

fn location_id(value: &str) -> LocationId {
    serde_json::from_str(&format!("\"{value}\"")).expect("location UUID must deserialize")
}

#[test]
fn wal_restore_survives_interrupted_write() {
    let temp = TestDirectory::new("catalog-recovery");
    let catalog_path = temp.path().join("catalog.sqlite");
    let catalog = SqliteCatalog::open(&catalog_path).expect("catalog must open");

    assert_eq!(catalog.migrate().expect("migration must run"), 1);
    assert_eq!(catalog.migrate().expect("migration must be idempotent"), 1);

    let asset_id = parse_asset_id("00000000-0000-0000-0000-000000000001");
    catalog
        .upsert_asset(&asset(
            asset_id,
            location_id("00000000-0000-0000-0000-000000000002"),
        ))
        .expect("asset transaction must commit");
    let first_document = document("0.1.0");
    let first_revision = catalog
        .append_revision(asset_id, &first_document)
        .expect("first revision must append");

    let wal_path = PathBuf::from(format!("{}-wal", catalog_path.display()));
    assert!(wal_path.is_file(), "catalog must operate in WAL mode");

    let interrupted = Connection::open(&catalog_path).expect("second connection must open");
    interrupted
        .execute_batch(
            "BEGIN IMMEDIATE;
             INSERT INTO jobs (job_id, state) VALUES ('interrupted-job', 'running');",
        )
        .expect("interrupted transaction must start");
    drop(interrupted);

    let verifier = Connection::open(&catalog_path).expect("catalog must reopen after interruption");
    let interrupted_rows: u64 = verifier
        .query_row(
            "SELECT COUNT(*) FROM jobs WHERE job_id = 'interrupted-job'",
            [],
            |row| row.get(0),
        )
        .expect("rolled-back job count must be readable");
    assert_eq!(interrupted_rows, 0, "uncommitted rows must be rolled back");
    drop(verifier);

    let backup_path = temp.path().join("backup");
    let backup = catalog
        .backup_to(&backup_path)
        .expect("WAL snapshot must back up");
    assert_eq!(backup.entry_count, 1);

    let second_document = document("0.1.1");
    let second_revision = catalog
        .append_revision(asset_id, &second_document)
        .expect("second revision must append");
    assert!(second_revision.0 > first_revision.0);
    assert_eq!(
        catalog
            .load_edit_document(asset_id)
            .expect("current revision must load"),
        Some(second_document)
    );

    let revision_reader = Connection::open(&catalog_path).expect("revision reader must open");
    let revision_count: u64 = revision_reader
        .query_row(
            "SELECT COUNT(*) FROM revisions WHERE asset_id = ?1",
            [asset_id.0.to_string()],
            |row| row.get(0),
        )
        .expect("revision history must be readable");
    assert_eq!(revision_count, 2, "older revisions must remain immutable");
    drop(revision_reader);
    drop(catalog);

    let restore_path = temp.path().join("restored");
    let restored = SqliteCatalog::restore_from(&backup_path, &restore_path)
        .expect("backup must restore independently");
    assert!(restored.verified);
    assert_eq!(
        restored.restored_catalog,
        restore_path.join("catalog.sqlite")
    );

    let restored_catalog =
        SqliteCatalog::open(&restored.restored_catalog).expect("restored catalog must open");
    assert_eq!(
        restored_catalog
            .migrate()
            .expect("schema must remain current"),
        1
    );
    assert_eq!(
        restored_catalog
            .load_edit_document(asset_id)
            .expect("restored revision must load"),
        Some(first_document)
    );

    let missing_asset = parse_asset_id("00000000-0000-0000-0000-000000000099");
    assert!(matches!(
        restored_catalog.append_revision(missing_asset, &document("invalid")),
        Err(StorageError::Sql { .. })
    ));
    assert_eq!(
        restored_catalog
            .load_edit_document(asset_id)
            .expect("failed transaction must preserve current revision"),
        Some(document("0.1.0"))
    );
}
