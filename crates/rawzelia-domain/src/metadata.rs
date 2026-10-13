use std::path::PathBuf;

use rawzelia_contract::{AssetId, Fingerprint, Flag, LabelColor, LocationId, Rating};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureDate {
    pub year: i32,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetMetadata {
    pub width: u32,
    pub height: u32,
    pub orientation: u16,
    pub camera_model: Option<String>,
    pub lens_model: Option<String>,
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetAvailability {
    Present,
    Missing,
    Moved,
    VolumeOffline,
    PermissionDenied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetRecord {
    pub asset_id: AssetId,
    pub location_id: LocationId,
    pub original_path: PathBuf,
    pub relative_path: String,
    pub captured_at: Option<CaptureDate>,
    pub metadata: AssetMetadata,
    pub fingerprint: Fingerprint,
    pub availability: AssetAvailability,
    pub rating: Rating,
    pub flag: Flag,
    pub label: LabelColor,
}
