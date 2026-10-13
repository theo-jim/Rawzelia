use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LightroomSchemaId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportStatus {
    Exact,
    Approximated,
    PreservedOnly,
    Unsupported,
    Ignored,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldProvenance {
    pub source_schema: LightroomSchemaId,
    pub source_id: String,
    pub field_path: String,
    pub artifact: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportedField<T> {
    pub value: Option<T>,
    pub status: ImportStatus,
    pub provenance: FieldProvenance,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnknownField {
    pub field_path: String,
    pub raw_value: serde_json::Value,
    pub status: ImportStatus,
    pub provenance: FieldProvenance,
}
