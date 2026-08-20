//! Time-tracking-domain model: [`WorkItem`], as returned by
//! `list_work_items`/`add_work_item`.

use serde::{Deserialize, Serialize};

use crate::youtrack::issue::UserRef;

/// The work-item type (e.g. "Development", "Testing"), when the
/// instance requires one.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WorkItemType {
    pub name: Option<String>,
}

/// A work item's logged duration, both as raw minutes and as YouTrack's
/// own human-readable presentation (e.g. `"1h 30m"`).
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WorkItemDuration {
    pub minutes: Option<i64>,
    pub presentation: Option<String>,
}

/// A single time-tracking work item logged on an issue, as returned by
/// `GET /issues/{id}/timeTracking/workItems` and
/// `POST /issues/{id}/timeTracking/workItems`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WorkItem {
    pub id: Option<String>,
    /// Timestamp of the logged work, milliseconds since the Unix epoch.
    pub date: Option<i64>,
    pub duration: Option<WorkItemDuration>,
    pub text: Option<String>,
    #[serde(rename = "type")]
    pub kind: Option<WorkItemType>,
    pub author: Option<UserRef>,
}
