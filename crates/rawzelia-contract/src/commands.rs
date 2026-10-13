use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{
    Adjustment, AssetId, ExportFormat, ExportProfile, Fingerprint, Flag, LabelColor, LibraryQuery,
    PresetId, PreviewTarget, Rating, SettingSection,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSessionCommand {
    pub source_root: PathBuf,
    pub selected_paths: Vec<PathBuf>,
    pub destination_root: PathBuf,
    pub template: String,
    pub session_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetMetadataCommand {
    pub asset_id: AssetId,
    pub rating: Rating,
    pub flag: Flag,
    pub label: LabelColor,
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetAdjustmentCommand {
    pub asset_id: AssetId,
    pub adjustment: Adjustment,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyPresetCommand {
    pub asset_ids: Vec<AssetId>,
    pub preset_id: PresetId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopySettingsCommand {
    pub source_asset_id: AssetId,
    pub target_asset_ids: Vec<AssetId>,
    pub sections: Vec<SettingSection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestPreviewCommand {
    pub asset_id: AssetId,
    pub generation: u64,
    pub target: PreviewTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportCommand {
    pub asset_ids: Vec<AssetId>,
    pub destination: PathBuf,
    pub format: ExportFormat,
    pub profile: ExportProfile,
    pub quality: u8,
    pub max_dimension: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrintCommand {
    pub asset_ids: Vec<AssetId>,
    pub profile: ExportProfile,
    pub copies: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReferenceChoice {
    Confirmed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceJpegSelection {
    pub source_id: String,
    pub jpeg_path: PathBuf,
    pub fingerprint: Fingerprint,
    pub choice: ReferenceChoice,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InspectLightroomCommand {
    #[serde(rename = "catalogPath")]
    pub catalog_path: PathBuf,
    #[serde(rename = "associatedRoot")]
    pub associated_root: Option<PathBuf>,
    pub reference_jpeg: Option<ReferenceJpegSelection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecuteLightroomCommand {
    pub plan_id: String,
    pub reference_jpeg: Option<ReferenceJpegSelection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RollbackLightroomCommand {
    pub batch_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "AppCommandWire", into = "AppCommandWire")]
pub enum AppCommand {
    OpenCatalog { path: PathBuf },
    ImportSession(ImportSessionCommand),
    QueryLibrary(LibraryQuery),
    SetSelection { asset_ids: Vec<AssetId> },
    SetMetadata(SetMetadataCommand),
    SetAdjustment(SetAdjustmentCommand),
    Undo { asset_id: AssetId },
    Redo { asset_id: AssetId },
    ApplyPreset(ApplyPresetCommand),
    CopySettings(CopySettingsCommand),
    SynchronizeSettings(CopySettingsCommand),
    RequestPreview(RequestPreviewCommand),
    Export(ExportCommand),
    Print(PrintCommand),
    InspectLightroom(InspectLightroomCommand),
    ExecuteLightroom(ExecuteLightroomCommand),
    RollbackLightroom(RollbackLightroomCommand),
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum AppCommandWire {
    OpenCatalog {
        path: PathBuf,
    },
    ImportSession {
        payload: ImportSessionCommand,
    },
    QueryLibrary {
        payload: LibraryQuery,
    },
    SetSelection {
        #[serde(rename = "assetIds")]
        asset_ids: Vec<AssetId>,
    },
    SetMetadata {
        payload: SetMetadataCommand,
    },
    SetAdjustment {
        payload: SetAdjustmentCommand,
    },
    Undo {
        #[serde(rename = "assetId")]
        asset_id: AssetId,
    },
    Redo {
        #[serde(rename = "assetId")]
        asset_id: AssetId,
    },
    ApplyPreset {
        payload: ApplyPresetCommand,
    },
    CopySettings {
        payload: CopySettingsCommand,
    },
    SynchronizeSettings {
        payload: CopySettingsCommand,
    },
    RequestPreview {
        payload: RequestPreviewCommand,
    },
    Export {
        payload: ExportCommand,
    },
    Print {
        payload: PrintCommand,
    },
    InspectLightroom {
        payload: InspectLightroomCommand,
    },
    ExecuteLightroom {
        payload: ExecuteLightroomCommand,
    },
    RollbackLightroom {
        payload: RollbackLightroomCommand,
    },
}

impl From<AppCommand> for AppCommandWire {
    fn from(command: AppCommand) -> Self {
        match command {
            AppCommand::OpenCatalog { path } => Self::OpenCatalog { path },
            AppCommand::ImportSession(payload) => Self::ImportSession { payload },
            AppCommand::QueryLibrary(payload) => Self::QueryLibrary { payload },
            AppCommand::SetSelection { asset_ids } => Self::SetSelection { asset_ids },
            AppCommand::SetMetadata(payload) => Self::SetMetadata { payload },
            AppCommand::SetAdjustment(payload) => Self::SetAdjustment { payload },
            AppCommand::Undo { asset_id } => Self::Undo { asset_id },
            AppCommand::Redo { asset_id } => Self::Redo { asset_id },
            AppCommand::ApplyPreset(payload) => Self::ApplyPreset { payload },
            AppCommand::CopySettings(payload) => Self::CopySettings { payload },
            AppCommand::SynchronizeSettings(payload) => Self::SynchronizeSettings { payload },
            AppCommand::RequestPreview(payload) => Self::RequestPreview { payload },
            AppCommand::Export(payload) => Self::Export { payload },
            AppCommand::Print(payload) => Self::Print { payload },
            AppCommand::InspectLightroom(payload) => Self::InspectLightroom { payload },
            AppCommand::ExecuteLightroom(payload) => Self::ExecuteLightroom { payload },
            AppCommand::RollbackLightroom(payload) => Self::RollbackLightroom { payload },
        }
    }
}

impl From<AppCommandWire> for AppCommand {
    fn from(command: AppCommandWire) -> Self {
        match command {
            AppCommandWire::OpenCatalog { path } => Self::OpenCatalog { path },
            AppCommandWire::ImportSession { payload } => Self::ImportSession(payload),
            AppCommandWire::QueryLibrary { payload } => Self::QueryLibrary(payload),
            AppCommandWire::SetSelection { asset_ids } => Self::SetSelection { asset_ids },
            AppCommandWire::SetMetadata { payload } => Self::SetMetadata(payload),
            AppCommandWire::SetAdjustment { payload } => Self::SetAdjustment(payload),
            AppCommandWire::Undo { asset_id } => Self::Undo { asset_id },
            AppCommandWire::Redo { asset_id } => Self::Redo { asset_id },
            AppCommandWire::ApplyPreset { payload } => Self::ApplyPreset(payload),
            AppCommandWire::CopySettings { payload } => Self::CopySettings(payload),
            AppCommandWire::SynchronizeSettings { payload } => Self::SynchronizeSettings(payload),
            AppCommandWire::RequestPreview { payload } => Self::RequestPreview(payload),
            AppCommandWire::Export { payload } => Self::Export(payload),
            AppCommandWire::Print { payload } => Self::Print(payload),
            AppCommandWire::InspectLightroom { payload } => Self::InspectLightroom(payload),
            AppCommandWire::ExecuteLightroom { payload } => Self::ExecuteLightroom(payload),
            AppCommandWire::RollbackLightroom { payload } => Self::RollbackLightroom(payload),
        }
    }
}
