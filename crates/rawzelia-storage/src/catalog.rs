use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use rawzelia_domain::{
    AssetId, AssetRecord, CaptureDate, EditDocument, LibraryItem, LibraryPage, LibraryQuery,
    RevisionId,
};
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params, params_from_iter};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::backup::{
    BackupError, BackupReport, RestoreError, RestoreReport, create_backup, restore_backup,
};
use crate::migrations;
use crate::xmp::{XmpOutputRoot, XmpRootAuthority, unowned_xmp_root};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageError {
    Io {
        code: String,
        message_key: String,
    },
    Sql {
        code: String,
        message_key: String,
    },
    Conflict {
        fields: Vec<String>,
    },
    InvalidBackup {
        code: String,
        message_key: String,
    },
    Import {
        code: String,
        message_key: String,
        source_id: Option<String>,
        field_path: Option<String>,
    },
}

impl StorageError {
    pub(crate) fn from_io(error: std::io::Error) -> Self {
        let code = error
            .raw_os_error()
            .map_or_else(|| format!("{:?}", error.kind()), |value| value.to_string());
        Self::Io {
            code,
            message_key: "storage.io".to_owned(),
        }
    }

    pub(crate) fn from_sql(error: rusqlite::Error) -> Self {
        let code = match &error {
            rusqlite::Error::SqliteFailure(failure, _) => failure.extended_code.to_string(),
            _ => "sqlite".to_owned(),
        };
        Self::Sql {
            code,
            message_key: "storage.sql".to_owned(),
        }
    }

    pub(crate) fn serialization(code: &str) -> Self {
        Self::Io {
            code: code.to_owned(),
            message_key: "storage.serialization".to_owned(),
        }
    }

    fn from_backup(error: BackupError) -> Self {
        match error {
            BackupError::Io(message) => Self::Io {
                code: message,
                message_key: "storage.backup.io".to_owned(),
            },
            BackupError::HashMismatch => Self::InvalidBackup {
                code: "hash_mismatch".to_owned(),
                message_key: "storage.backup.hash_mismatch".to_owned(),
            },
            BackupError::InconsistentCatalog => Self::InvalidBackup {
                code: "inconsistent_catalog".to_owned(),
                message_key: "storage.backup.inconsistent_catalog".to_owned(),
            },
            BackupError::DestinationExists => Self::Io {
                code: "destination_exists".to_owned(),
                message_key: "storage.backup.destination_exists".to_owned(),
            },
        }
    }

    fn from_restore(error: RestoreError) -> Self {
        match error {
            RestoreError::Io(message) => Self::Io {
                code: message,
                message_key: "storage.restore.io".to_owned(),
            },
            RestoreError::InvalidManifest => Self::InvalidBackup {
                code: "invalid_manifest".to_owned(),
                message_key: "storage.restore.invalid_manifest".to_owned(),
            },
            RestoreError::HashMismatch => Self::InvalidBackup {
                code: "hash_mismatch".to_owned(),
                message_key: "storage.restore.hash_mismatch".to_owned(),
            },
            RestoreError::DestinationUntouched => Self::InvalidBackup {
                code: "destination_untouched".to_owned(),
                message_key: "storage.restore.destination_untouched".to_owned(),
            },
        }
    }
}

impl fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { code, message_key }
            | Self::Sql { code, message_key }
            | Self::InvalidBackup { code, message_key } => {
                write!(formatter, "{message_key} ({code})")
            }
            Self::Conflict { fields } => {
                write!(formatter, "storage.conflict ({})", fields.join(","))
            }
            Self::Import {
                code,
                message_key,
                source_id,
                field_path,
            } => write!(
                formatter,
                "{message_key} ({code}, source={source_id:?}, field={field_path:?})"
            ),
        }
    }
}

impl std::error::Error for StorageError {}

pub trait CatalogStore: Send + Sync {
    fn open(path: &Path) -> Result<Self, StorageError>
    where
        Self: Sized;
    fn migrate(&self) -> Result<u32, StorageError>;
    fn upsert_asset(&self, asset: &AssetRecord) -> Result<(), StorageError>;
    fn find_asset(&self, asset_id: AssetId) -> Result<Option<AssetRecord>, StorageError>;
    fn remove_asset(&self, asset_id: AssetId) -> Result<(), StorageError>;
    fn page_library(&self, query: &LibraryQuery) -> Result<LibraryPage, StorageError>;
    fn append_revision(
        &self,
        asset_id: AssetId,
        document: &EditDocument,
    ) -> Result<RevisionId, StorageError>;
    fn load_edit_document(&self, asset_id: AssetId) -> Result<Option<EditDocument>, StorageError>;
    fn backup_to(&self, destination: &Path) -> Result<BackupReport, StorageError>;
    fn restore_from(source: &Path, destination: &Path) -> Result<RestoreReport, StorageError>
    where
        Self: Sized;
}

pub struct SqliteCatalog {
    path: PathBuf,
    connection: Mutex<Connection>,
}

impl SqliteCatalog {
    fn connection(&self) -> Result<MutexGuard<'_, Connection>, StorageError> {
        self.connection.lock().map_err(|_| StorageError::Sql {
            code: "connection_lock_poisoned".to_owned(),
            message_key: "storage.sql.connection_lock".to_owned(),
        })
    }

    pub fn persist_xmp_output_root(
        &self,
        path: &Path,
        root: &XmpOutputRoot,
    ) -> Result<(), StorageError> {
        self.require_external_xmp_authority_store(path)?;
        let authority = root.persistent_authority(path)?;
        let serialized = to_json(&authority)?;
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(StorageError::from_sql)?;
        transaction
            .execute(
                "INSERT INTO xmp_output_authorities (root_path, authority_json)
                 VALUES (?1, ?2)
                 ON CONFLICT(root_path) DO UPDATE SET
                    authority_json = excluded.authority_json,
                    updated_at = CURRENT_TIMESTAMP",
                params![path.to_string_lossy(), serialized],
            )
            .map_err(StorageError::from_sql)?;
        transaction.commit().map_err(StorageError::from_sql)
    }

    pub fn open_xmp_output_root(&self, path: &Path) -> Result<XmpOutputRoot, StorageError> {
        self.require_external_xmp_authority_store(path)?;
        let serialized = {
            let connection = self.connection()?;
            connection
                .query_row(
                    "SELECT authority_json FROM xmp_output_authorities WHERE root_path = ?1",
                    [path.to_string_lossy()],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(StorageError::from_sql)?
        };
        let authority: XmpRootAuthority = serialized
            .as_deref()
            .ok_or_else(unowned_xmp_root)
            .and_then(from_json)?;
        XmpOutputRoot::open_with_authority(path, authority)
    }

    fn require_external_xmp_authority_store(&self, root: &Path) -> Result<(), StorageError> {
        let catalog_path = fs::canonicalize(&self.path).map_err(StorageError::from_io)?;
        let root_path = fs::canonicalize(root).map_err(StorageError::from_io)?;
        if catalog_path.starts_with(root_path) {
            return Err(StorageError::Conflict {
                fields: vec!["xmp.authority_store".to_owned()],
            });
        }
        Ok(())
    }
}

impl CatalogStore for SqliteCatalog {
    fn open(path: &Path) -> Result<Self, StorageError> {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(StorageError::from_io)?;
        }

        let connection = Connection::open(path).map_err(StorageError::from_sql)?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(StorageError::from_sql)?;
        connection
            .execute_batch("PRAGMA foreign_keys = ON; PRAGMA synchronous = FULL;")
            .map_err(StorageError::from_sql)?;
        let journal_mode: String = connection
            .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
            .map_err(StorageError::from_sql)?;
        if !journal_mode.eq_ignore_ascii_case("wal") {
            return Err(StorageError::Sql {
                code: "wal_unavailable".to_owned(),
                message_key: "storage.sql.wal_unavailable".to_owned(),
            });
        }

        Ok(Self {
            path: path.to_path_buf(),
            connection: Mutex::new(connection),
        })
    }

    fn migrate(&self) -> Result<u32, StorageError> {
        let mut connection = self.connection()?;
        migrations::migrate(&mut connection)
    }

    fn upsert_asset(&self, asset: &AssetRecord) -> Result<(), StorageError> {
        let asset_json = to_json(asset)?;
        let metadata_json = to_json(&asset.metadata)?;
        let fingerprint_size = i64::try_from(asset.fingerprint.size)
            .map_err(|_| StorageError::serialization("fingerprint_size_out_of_range"))?;
        let captured_at = asset.captured_at.map(capture_date_string);
        let root_path = asset
            .original_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_string_lossy()
            .into_owned();

        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(StorageError::from_sql)?;
        transaction
            .execute(
                "INSERT INTO locations (location_id, root_path, availability)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(location_id) DO UPDATE SET
                    root_path = excluded.root_path,
                    availability = excluded.availability,
                    last_observed_at = CURRENT_TIMESTAMP",
                params![
                    asset.location_id.0.to_string(),
                    root_path,
                    wire_value(&asset.availability)?,
                ],
            )
            .map_err(StorageError::from_sql)?;
        transaction
            .execute(
                "INSERT INTO assets (
                    asset_id, location_id, original_path, relative_path, captured_at,
                    fingerprint_size, fingerprint_sha256, availability, rating, flag, label,
                    asset_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                 ON CONFLICT(asset_id) DO UPDATE SET
                    location_id = excluded.location_id,
                    original_path = excluded.original_path,
                    relative_path = excluded.relative_path,
                    captured_at = excluded.captured_at,
                    fingerprint_size = excluded.fingerprint_size,
                    fingerprint_sha256 = excluded.fingerprint_sha256,
                    availability = excluded.availability,
                    rating = excluded.rating,
                    flag = excluded.flag,
                    label = excluded.label,
                    asset_json = excluded.asset_json",
                params![
                    asset.asset_id.0.to_string(),
                    asset.location_id.0.to_string(),
                    asset.original_path.to_string_lossy(),
                    asset.relative_path,
                    captured_at,
                    fingerprint_size,
                    asset.fingerprint.sha256.as_slice(),
                    wire_value(&asset.availability)?,
                    wire_value(&asset.rating)?,
                    wire_value(&asset.flag)?,
                    wire_value(&asset.label)?,
                    asset_json,
                ],
            )
            .map_err(StorageError::from_sql)?;
        transaction
            .execute(
                "INSERT INTO metadata (asset_id, normalized_json) VALUES (?1, ?2)
                 ON CONFLICT(asset_id) DO UPDATE SET normalized_json = excluded.normalized_json",
                params![asset.asset_id.0.to_string(), metadata_json],
            )
            .map_err(StorageError::from_sql)?;
        transaction
            .execute(
                "DELETE FROM asset_keywords WHERE asset_id = ?1",
                [asset.asset_id.0.to_string()],
            )
            .map_err(StorageError::from_sql)?;
        for keyword in &asset.metadata.keywords {
            transaction
                .execute(
                    "INSERT INTO keywords (value) VALUES (?1)
                     ON CONFLICT(value) DO NOTHING",
                    [keyword],
                )
                .map_err(StorageError::from_sql)?;
            transaction
                .execute(
                    "INSERT INTO asset_keywords (asset_id, keyword_id)
                     SELECT ?1, keyword_id FROM keywords WHERE value = ?2",
                    params![asset.asset_id.0.to_string(), keyword],
                )
                .map_err(StorageError::from_sql)?;
        }
        transaction.commit().map_err(StorageError::from_sql)
    }

    fn find_asset(&self, asset_id: AssetId) -> Result<Option<AssetRecord>, StorageError> {
        let connection = self.connection()?;
        let serialized = connection
            .query_row(
                "SELECT asset_json FROM assets WHERE asset_id = ?1",
                [asset_id.0.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(StorageError::from_sql)?;
        serialized.map(|value| from_json(&value)).transpose()
    }

    fn remove_asset(&self, asset_id: AssetId) -> Result<(), StorageError> {
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(StorageError::from_sql)?;
        transaction
            .execute(
                "DELETE FROM assets WHERE asset_id = ?1",
                [asset_id.0.to_string()],
            )
            .map_err(StorageError::from_sql)?;
        transaction.commit().map_err(StorageError::from_sql)
    }

    fn page_library(&self, query: &LibraryQuery) -> Result<LibraryPage, StorageError> {
        let mut clauses = vec!["1 = 1".to_owned()];
        let mut values = Vec::<Value>::new();

        if let Some(text) = &query.text {
            clauses.push("(assets.relative_path LIKE ? OR metadata.normalized_json LIKE ?)".into());
            let pattern = format!("%{text}%");
            values.push(Value::Text(pattern.clone()));
            values.push(Value::Text(pattern));
        }
        if let Some(date_from) = &query.date_from {
            clauses.push("assets.captured_at >= ?".into());
            values.push(Value::Text(date_from.clone()));
        }
        if let Some(date_to) = &query.date_to {
            clauses.push("assets.captured_at <= ?".into());
            values.push(Value::Text(date_to.clone()));
        }
        if let Some(collection_id) = &query.collection_id {
            clauses.push(
                "EXISTS (SELECT 1 FROM collection_members
                         WHERE collection_members.asset_id = assets.asset_id
                           AND collection_members.collection_id = ?)"
                    .into(),
            );
            values.push(Value::Text(collection_id.clone()));
        }
        if let Some(rating) = query.rating {
            clauses.push("assets.rating = ?".into());
            values.push(Value::Text(wire_value(&rating)?));
        }
        if let Some(flag) = query.flag {
            clauses.push("assets.flag = ?".into());
            values.push(Value::Text(wire_value(&flag)?));
        }
        if let Some(label) = query.label {
            clauses.push("assets.label = ?".into());
            values.push(Value::Text(wire_value(&label)?));
        }
        if let Some(keyword) = &query.keyword {
            clauses.push(
                "EXISTS (SELECT 1 FROM asset_keywords
                         JOIN keywords USING (keyword_id)
                         WHERE asset_keywords.asset_id = assets.asset_id
                           AND keywords.value = ?)"
                    .into(),
            );
            values.push(Value::Text(keyword.clone()));
        }

        let where_clause = clauses.join(" AND ");
        let connection = self.connection()?;
        let total_sql = format!(
            "SELECT COUNT(*) FROM assets
             JOIN metadata ON metadata.asset_id = assets.asset_id
             WHERE {where_clause}"
        );
        let total: u64 = connection
            .query_row(&total_sql, params_from_iter(values.iter()), |row| {
                row.get(0)
            })
            .map_err(StorageError::from_sql)?;

        let offset = u64::from(query.page)
            .checked_mul(u64::from(query.page_size))
            .ok_or_else(|| StorageError::Sql {
                code: "page_offset_overflow".to_owned(),
                message_key: "storage.query.invalid_page".to_owned(),
            })?;
        let mut page_values = values;
        page_values.push(Value::Integer(i64::from(query.page_size)));
        page_values.push(Value::Integer(i64::try_from(offset).map_err(|_| {
            StorageError::Sql {
                code: "page_offset_out_of_range".to_owned(),
                message_key: "storage.query.invalid_page".to_owned(),
            }
        })?));
        let page_sql = format!(
            "SELECT assets.asset_json FROM assets
             JOIN metadata ON metadata.asset_id = assets.asset_id
             WHERE {where_clause}
             ORDER BY assets.captured_at, assets.asset_id
             LIMIT ? OFFSET ?"
        );
        let mut statement = connection
            .prepare(&page_sql)
            .map_err(StorageError::from_sql)?;
        let rows = statement
            .query_map(params_from_iter(page_values.iter()), |row| {
                row.get::<_, String>(0)
            })
            .map_err(StorageError::from_sql)?;
        let mut items = Vec::new();
        for row in rows {
            let asset: AssetRecord = from_json(&row.map_err(StorageError::from_sql)?)?;
            items.push(LibraryItem {
                asset_id: asset.asset_id,
                relative_path: asset.relative_path,
                rating: asset.rating,
                flag: asset.flag,
                label: asset.label,
                thumbnail: None,
            });
        }

        Ok(LibraryPage {
            items,
            total,
            page: query.page,
            page_size: query.page_size,
        })
    }

    fn append_revision(
        &self,
        asset_id: AssetId,
        document: &EditDocument,
    ) -> Result<RevisionId, StorageError> {
        let serialized = to_json(document)?;
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(StorageError::from_sql)?;
        transaction
            .execute(
                "INSERT INTO revisions (asset_id, document_json) VALUES (?1, ?2)",
                params![asset_id.0.to_string(), serialized],
            )
            .map_err(StorageError::from_sql)?;
        let revision_id = transaction.last_insert_rowid();
        transaction
            .execute(
                "INSERT INTO asset_revision_heads (asset_id, revision_id) VALUES (?1, ?2)
                 ON CONFLICT(asset_id) DO UPDATE SET revision_id = excluded.revision_id",
                params![asset_id.0.to_string(), revision_id],
            )
            .map_err(StorageError::from_sql)?;
        transaction.commit().map_err(StorageError::from_sql)?;
        Ok(RevisionId(u64::try_from(revision_id).map_err(|_| {
            StorageError::Sql {
                code: "revision_id_out_of_range".to_owned(),
                message_key: "storage.sql.invalid_revision".to_owned(),
            }
        })?))
    }

    fn load_edit_document(&self, asset_id: AssetId) -> Result<Option<EditDocument>, StorageError> {
        let connection = self.connection()?;
        let serialized = connection
            .query_row(
                "SELECT revisions.document_json
                 FROM asset_revision_heads
                 JOIN revisions USING (revision_id)
                 WHERE asset_revision_heads.asset_id = ?1",
                [asset_id.0.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(StorageError::from_sql)?;
        serialized.map(|value| from_json(&value)).transpose()
    }

    fn backup_to(&self, destination: &Path) -> Result<BackupReport, StorageError> {
        let _connection = self.connection()?;
        create_backup(&self.path, &[], destination).map_err(StorageError::from_backup)
    }

    fn restore_from(source: &Path, destination: &Path) -> Result<RestoreReport, StorageError> {
        restore_backup(source, destination).map_err(StorageError::from_restore)
    }
}

fn to_json<T: Serialize>(value: &T) -> Result<String, StorageError> {
    serde_json::to_string(value).map_err(|_| StorageError::serialization("serialize"))
}

fn from_json<T: DeserializeOwned>(value: &str) -> Result<T, StorageError> {
    serde_json::from_str(value).map_err(|_| StorageError::serialization("deserialize"))
}

fn wire_value<T: Serialize>(value: &T) -> Result<String, StorageError> {
    let serialized = serde_json::to_value(value)
        .map_err(|_| StorageError::serialization("serialize_wire_value"))?;
    serialized
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| StorageError::serialization("wire_value_not_string"))
}

fn capture_date_string(date: CaptureDate) -> String {
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        date.year, date.month, date.day, date.hour, date.minute, date.second
    )
}
