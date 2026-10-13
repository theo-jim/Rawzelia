use std::path::{Path, PathBuf};

use rawzelia_contract::commands::{ReferenceChoice, ReferenceJpegSelection};
use rawzelia_contract::lightroom_import::{
    ImportStatus as ContractImportStatus, ImportedField as ContractImportedField,
    LightroomSchemaId as ContractLightroomSchemaId,
};
use rawzelia_contract::types::Fingerprint;
use rawzelia_domain::{CoreValue, FieldProvenance, ImportStatus, ImportedField, LightroomSchemaId};
use rawzelia_lightroom_import::{SourceGuardError, validate_source_guard};

fn consume_future_import_contract(
    schema: LightroomSchemaId,
    field: ImportedField<CoreValue>,
) -> (LightroomSchemaId, FieldProvenance) {
    (schema, field.provenance)
}

#[test]
fn canonical_types_compile_across_core_and_import() {
    let schema = LightroomSchemaId("lightroom-latest-corpus".to_owned());
    let field = ImportedField {
        value: Some(CoreValue::Text("Nikon D750".to_owned())),
        status: ImportStatus::Exact,
        provenance: FieldProvenance {
            source_schema: schema.clone(),
            source_id: "asset-0001".to_owned(),
            field_path: "lightroom.metadata.camera_model".to_owned(),
            artifact: "catalog.lrcat".to_owned(),
        },
    };

    let (consumed_schema, provenance) = consume_future_import_contract(schema, field);
    assert_eq!(consumed_schema, provenance.source_schema);
    assert_eq!(provenance.source_id, "asset-0001");
    assert_eq!(
        validate_source_guard(Path::new("missing-contract-catalog.lrcat")),
        Err(SourceGuardError::Missing)
    );
}

#[test]
fn canonical_module_paths_are_public_across_crates() {
    let schema = ContractLightroomSchemaId("lightroom-latest-corpus".to_owned());
    let field = ContractImportedField {
        value: Some("Nikon D750".to_owned()),
        status: ContractImportStatus::Exact,
        provenance: rawzelia_contract::lightroom_import::FieldProvenance {
            source_schema: schema,
            source_id: "asset-0001".to_owned(),
            field_path: "lightroom.metadata.camera_model".to_owned(),
            artifact: "catalog.lrcat".to_owned(),
        },
    };
    let selection = ReferenceJpegSelection {
        source_id: field.provenance.source_id.clone(),
        jpeg_path: PathBuf::from("reference/asset-0001.jpg"),
        fingerprint: Fingerprint {
            size: 4_096,
            sha256: [11; 32],
        },
        choice: ReferenceChoice::Confirmed,
    };

    assert_eq!(selection.source_id, "asset-0001");
    assert_eq!(selection.fingerprint.size, 4_096);
}
