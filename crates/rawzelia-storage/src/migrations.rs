use rusqlite::{Connection, TransactionBehavior};

use crate::StorageError;

const INITIAL_MIGRATION: &str = include_str!("../migrations/0001_initial.sql");
const CURRENT_SCHEMA_VERSION: u32 = 1;

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

    if version == 0 {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(StorageError::from_sql)?;
        transaction
            .execute_batch(INITIAL_MIGRATION)
            .map_err(StorageError::from_sql)?;
        transaction
            .pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION)
            .map_err(StorageError::from_sql)?;
        transaction.commit().map_err(StorageError::from_sql)?;
    }

    Ok(CURRENT_SCHEMA_VERSION)
}
