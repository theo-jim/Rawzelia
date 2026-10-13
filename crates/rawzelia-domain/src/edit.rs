use std::collections::BTreeMap;

use rawzelia_contract::{Adjustment, Fingerprint, ImportedField, UnknownField};
use serde::{Deserialize, Serialize};

use crate::CoreValue;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditDocument {
    pub schema_version: u32,
    pub source_fingerprint: Fingerprint,
    pub pipeline_version: String,
    pub adjustments: Vec<Adjustment>,
    pub community: BTreeMap<String, ImportedField<CoreValue>>,
    pub lightroom: Vec<ImportedField<CoreValue>>,
    pub lightroom_unknown: Vec<UnknownField>,
    pub pro: BTreeMap<String, ImportedField<CoreValue>>,
}
