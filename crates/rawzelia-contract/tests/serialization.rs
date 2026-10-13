use rawzelia_contract::{FieldProvenance, ImportStatus, ImportedField, LightroomSchemaId};

#[test]
fn import_status_and_field_provenance_round_trip() {
    let statuses = [
        (ImportStatus::Exact, "exact"),
        (ImportStatus::Approximated, "approximated"),
        (ImportStatus::PreservedOnly, "preserved_only"),
        (ImportStatus::Unsupported, "unsupported"),
        (ImportStatus::Ignored, "ignored"),
    ];

    for (status, expected_wire_value) in statuses {
        let serialized = serde_json::to_string(&status).expect("status must serialize");
        assert_eq!(serialized, format!("\"{expected_wire_value}\""));
        assert_eq!(
            serde_json::from_str::<ImportStatus>(&serialized).expect("status must deserialize"),
            status
        );
    }

    let field = ImportedField {
        value: Some("Nikon D750".to_owned()),
        status: ImportStatus::Exact,
        provenance: FieldProvenance {
            source_schema: LightroomSchemaId("lightroom-latest-corpus".to_owned()),
            source_id: "asset-0001".to_owned(),
            field_path: "lightroom.metadata.camera_model".to_owned(),
            artifact: "catalog.lrcat".to_owned(),
        },
    };

    let serialized = serde_json::to_string(&field).expect("field must serialize");
    let round_trip: ImportedField<String> =
        serde_json::from_str(&serialized).expect("field must deserialize");
    assert_eq!(round_trip, field);
    assert_eq!(
        round_trip.provenance.source_schema.0,
        "lightroom-latest-corpus"
    );
    assert_eq!(round_trip.provenance.source_id, "asset-0001");
    assert_eq!(
        round_trip.provenance.field_path,
        "lightroom.metadata.camera_model"
    );
    assert_eq!(round_trip.provenance.artifact, "catalog.lrcat");
}
