use serde::{Deserialize, Serialize};

use crate::{AssetId, CapabilitySet, JobId, LibraryPage, RequestId, ResourceRef};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JobEventState {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "type",
    content = "payload",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AppEvent {
    Ready {
        capabilities: CapabilitySet,
    },
    CommandAccepted {
        request_id: RequestId,
        job_id: Option<JobId>,
    },
    LibraryPageReady {
        request_id: RequestId,
        page: LibraryPage,
    },
    ImportProgress {
        job_id: JobId,
        completed: u32,
        total: u32,
        state: JobEventState,
    },
    RenderReady {
        asset_id: AssetId,
        generation: u64,
        resource: ResourceRef,
    },
    RenderDropped {
        asset_id: AssetId,
        generation: u64,
    },
    JobFailed {
        job_id: JobId,
        message: String,
        recoverable: bool,
    },
    CatalogChanged {
        asset_id: Option<AssetId>,
    },
}

pub trait EventSink: Send + Sync {
    fn publish(&self, event: AppEvent);
}
