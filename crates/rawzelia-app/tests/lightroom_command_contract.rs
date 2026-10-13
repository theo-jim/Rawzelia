use std::path::PathBuf;

use rawzelia_contract::{
    AppCommand, ExecuteLightroomCommand, Fingerprint, InspectLightroomCommand, ReferenceChoice,
    ReferenceJpegSelection, RollbackLightroomCommand,
};

fn append_artifact_selection(command: &ExecuteLightroomCommand) -> Option<&ReferenceJpegSelection> {
    command.reference_jpeg.as_ref()
}

#[test]
fn lightroom_reference_jpeg_round_trips_from_selection_to_execute_to_rollback() {
    let selection = ReferenceJpegSelection {
        source_id: "asset-0001".to_owned(),
        jpeg_path: PathBuf::from("reference/asset-0001.jpg"),
        fingerprint: Fingerprint {
            size: 4_096,
            sha256: [11; 32],
        },
        choice: ReferenceChoice::Confirmed,
    };
    let commands = vec![
        AppCommand::InspectLightroom(InspectLightroomCommand {
            catalog_path: PathBuf::from("catalog.lrcat"),
            associated_root: Some(PathBuf::from("catalog.lrdata")),
            reference_jpeg: Some(selection.clone()),
        }),
        AppCommand::ExecuteLightroom(ExecuteLightroomCommand {
            plan_id: "plan-0001".to_owned(),
            reference_jpeg: Some(selection.clone()),
        }),
        AppCommand::RollbackLightroom(RollbackLightroomCommand {
            batch_id: "batch-0001".to_owned(),
        }),
    ];

    let serialized = serde_json::to_string(&commands).expect("command sequence must serialize");
    let wire: serde_json::Value =
        serde_json::from_str(&serialized).expect("command JSON must parse");
    assert_eq!(wire[0]["type"], "inspectLightroom");
    assert_eq!(wire[0]["payload"]["catalogPath"], "catalog.lrcat");
    assert_eq!(
        wire[0]["payload"]["reference_jpeg"]["source_id"],
        "asset-0001"
    );
    assert_eq!(wire[1]["type"], "executeLightroom");
    assert_eq!(wire[1]["payload"]["plan_id"], "plan-0001");
    assert_eq!(wire[2]["type"], "rollbackLightroom");
    assert_eq!(wire[2]["payload"]["batch_id"], "batch-0001");
    let round_trip: Vec<AppCommand> =
        serde_json::from_str(&serialized).expect("command sequence must deserialize");

    match &round_trip[0] {
        AppCommand::InspectLightroom(command) => {
            assert_eq!(command.reference_jpeg.as_ref(), Some(&selection));
        }
        command => panic!("unexpected first command: {command:?}"),
    }
    match &round_trip[1] {
        AppCommand::ExecuteLightroom(command) => {
            assert_eq!(append_artifact_selection(command), Some(&selection));
        }
        command => panic!("unexpected second command: {command:?}"),
    }
    match &round_trip[2] {
        AppCommand::RollbackLightroom(command) => assert_eq!(command.batch_id, "batch-0001"),
        command => panic!("unexpected third command: {command:?}"),
    }
}
