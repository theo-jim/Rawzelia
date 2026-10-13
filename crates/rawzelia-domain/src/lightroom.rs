use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CoreValue {
    Text(String),
    Integer(i64),
    Decimal(f64),
    Boolean(bool),
    DateTime(String),
    Path(PathBuf),
}

pub use rawzelia_contract::{
    FieldProvenance, ImportStatus, ImportedField, LightroomSchemaId, UnknownField,
};
