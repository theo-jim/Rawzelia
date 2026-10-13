use rusqlite::{Connection, TransactionBehavior};

use crate::StorageError;

const INITIAL_MIGRATION: &str = include_str!("../migrations/0001_initial.sql");
const XMP_AUTHORITY_MIGRATION: &str = include_str!("../migrations/0002_xmp_output_authorities.sql");
const CURRENT_SCHEMA_VERSION: u32 = 2;

pub(crate) fn migrate(connection: &mut Connection) -> Result<u32, StorageError> {
    let version: u32 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(StorageError::from_sql)?;

    if version > CURRENT_SCHEMA_VERSION {
        return Err(StorageError::Sql {
            code: "schema_too_new".to_owned(),
            message_key: "storage.schema.too_new".to_owned(),
        });
    }

    let mut version = version;
    if version == 0 {
        apply_migration(connection, INITIAL_MIGRATION, 1)?;
        version = 1;
    }
    if version == 1 {
        apply_migration(connection, XMP_AUTHORITY_MIGRATION, 2)?;
    }

    Ok(CURRENT_SCHEMA_VERSION)
}

fn apply_migration(
    connection: &mut Connection,
    sql: &str,
    version: u32,
) -> Result<(), StorageError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(StorageError::from_sql)?;
    transaction
        .execute_batch(sql)
        .map_err(StorageError::from_sql)?;
    transaction
        .pragma_update(None, "user_version", version)
        .map_err(StorageError::from_sql)?;
    transaction.commit().map_err(StorageError::from_sql)
}
