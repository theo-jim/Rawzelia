use std::path::Path;

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
