//! Issue-activity-domain model: [`ActivityItem`], as returned by
//! `get_issue_activity`.

use serde::{Deserialize, Serialize};

use crate::youtrack::issue::UserRef;

/// The changed field an [`ActivityItem`] describes.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ActivityField {
    pub presentation: Option<String>,
}

/// One entry in an issue's activity log (`GET /issues/{id}/activities`):
/// who changed `field`, from `removed` to `added`, and when. `added`/
/// `removed` are left as raw JSON since their shape is polymorphic
/// (`null`, an object, or an array of objects) depending on the field
/// type, mirroring [`crate::youtrack::issue::CustomField::value`].
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ActivityItem {
    /// Milliseconds since the Unix epoch.
    pub timestamp: Option<i64>,
    pub author: Option<UserRef>,
    pub field: Option<ActivityField>,
    #[serde(default)]
    pub added: serde_json::Value,
    #[serde(default)]
    pub removed: serde_json::Value,
}
