use rawzelia_contract::{
    FieldProvenance, ImportStatus, ImportedField, LightroomSchemaId, UnknownField,
};

fn accept_canonical_field<T>(field: ImportedField<T>) -> ImportedField<T> {
    field
}

#[test]
fn canonical_lightroom_types_compile_across_core_boundary() {
    let provenance = FieldProvenance {
        source_schema: LightroomSchemaId("lightroom-latest-corpus".to_owned()),
        source_id: "asset-0001".to_owned(),
        field_path: "lightroom.plugin.custom".to_owned(),
        artifact: "catalog.lrcat".to_owned(),
    };
    let known = accept_canonical_field(ImportedField {
        value: Some(2_i64),
        status: ImportStatus::Approximated,
        provenance: provenance.clone(),
    });
    let unknown = UnknownField {
        field_path: "lightroom.plugin.custom".to_owned(),
        raw_value: serde_json::json!({"strength": 0.75}),
        status: ImportStatus::PreservedOnly,
        provenance,
    };

    assert_eq!(known.value, Some(2));
    assert_eq!(unknown.status, ImportStatus::PreservedOnly);
    assert_eq!(unknown.raw_value["strength"], 0.75);
}
