use std::collections::BTreeMap;

use rawzelia_domain::{
    Adjustment, CoreValue, EditDocument, FieldProvenance, Fingerprint, ImportStatus, ImportedField,
    LightroomSchemaId, UnknownField,
};

fn provenance(field_path: &str) -> FieldProvenance {
    FieldProvenance {
        source_schema: LightroomSchemaId("lightroom-latest-corpus".to_owned()),
        source_id: "asset-0001".to_owned(),
        field_path: field_path.to_owned(),
        artifact: "catalog.lrcat".to_owned(),
    }
}

#[test]
fn edit_document_round_trips_namespaces_and_statuses() {
    let mut community = BTreeMap::new();
    community.insert(
        "community.rating".to_owned(),
        ImportedField {
            value: Some(CoreValue::Integer(5)),
            status: ImportStatus::Exact,
            provenance: provenance("community.rating"),
        },
    );
    let lightroom = vec![ImportedField {
        value: Some(CoreValue::Decimal(0.75)),
        status: ImportStatus::Approximated,
        provenance: provenance("lightroom.develop.exposure"),
    }];
    let lightroom_unknown = vec![UnknownField {
        field_path: "lightroom.plugin.mask".to_owned(),
        raw_value: serde_json::json!({"kind": "plugin-mask"}),
        status: ImportStatus::PreservedOnly,
        provenance: provenance("lightroom.plugin.mask"),
    }];
    let mut pro = BTreeMap::new();
    pro.insert(
        "pro.mask".to_owned(),
        ImportedField {
            value: None,
            status: ImportStatus::Unsupported,
            provenance: provenance("pro.mask"),
        },
    );
    let document = EditDocument {
        schema_version: 1,
        source_fingerprint: Fingerprint {
            size: 1_024,
            sha256: [3; 32],
        },
        pipeline_version: "0.1.0".to_owned(),
        adjustments: vec![Adjustment::Exposure { value: 0.5 }],
        community,
        lightroom,
        lightroom_unknown,
        pro,
    };

    let serialized = serde_json::to_string(&document).expect("document must serialize");
    let wire: serde_json::Value =
        serde_json::from_str(&serialized).expect("document JSON must parse");
    assert_eq!(wire["community"]["community.rating"]["status"], "exact");
    assert_eq!(wire["lightroom"][0]["status"], "approximated");
    assert_eq!(wire["lightroom_unknown"][0]["status"], "preserved_only");
    assert_eq!(wire["pro"]["pro.mask"]["status"], "unsupported");

    let round_trip: EditDocument =
        serde_json::from_str(&serialized).expect("document must deserialize");
    assert_eq!(round_trip, document);
}
