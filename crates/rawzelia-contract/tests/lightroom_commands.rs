use std::path::PathBuf;

use rawzelia_contract::{
    ExecuteLightroomCommand, Fingerprint, InspectLightroomCommand, ReferenceChoice,
    ReferenceJpegSelection, RollbackLightroomCommand,
};

#[test]
fn canonical_lightroom_commands_compile_in_contract() {
    let selection = ReferenceJpegSelection {
        source_id: "asset-0001".to_owned(),
        jpeg_path: PathBuf::from("reference/asset-0001.jpg"),
        fingerprint: Fingerprint {
            size: 42,
            sha256: [7; 32],
        },
        choice: ReferenceChoice::Confirmed,
    };
    let inspect = InspectLightroomCommand {
        catalog_path: PathBuf::from("catalog.lrcat"),
        associated_root: Some(PathBuf::from("catalog.lrdata")),
        reference_jpeg: Some(selection.clone()),
    };
    let execute = ExecuteLightroomCommand {
        plan_id: "plan-0001".to_owned(),
        reference_jpeg: Some(selection.clone()),
    };
    let rollback = RollbackLightroomCommand {
        batch_id: "batch-0001".to_owned(),
    };

    assert_eq!(inspect.reference_jpeg, Some(selection.clone()));
    assert_eq!(execute.reference_jpeg, Some(selection));
    assert_eq!(rollback.batch_id, "batch-0001");

    let inspect_wire = serde_json::to_value(&inspect).expect("inspect command must serialize");
    assert_eq!(inspect_wire["catalogPath"], "catalog.lrcat");
    assert_eq!(inspect_wire["reference_jpeg"]["source_id"], "asset-0001");
    let execute_wire = serde_json::to_value(&execute).expect("execute command must serialize");
    assert_eq!(execute_wire["plan_id"], "plan-0001");
    assert_eq!(
        execute_wire["reference_jpeg"]["jpeg_path"],
        "reference/asset-0001.jpg"
    );
    let rollback_wire = serde_json::to_value(&rollback).expect("rollback command must serialize");
    assert_eq!(rollback_wire["batch_id"], "batch-0001");
}
