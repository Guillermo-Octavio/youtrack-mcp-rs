//! Comment-domain model: [`Comment`], as returned by `list_comments`/
//! `add_comment`.

use serde::{Deserialize, Serialize};

use crate::youtrack::issue::UserRef;

/// A single comment on an issue, as returned by
/// `GET /issues/{id}/comments` and `POST /issues/{id}/comments`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Comment {
    pub id: Option<String>,
    pub text: Option<String>,
    /// Creation timestamp, milliseconds since the Unix epoch (YouTrack's
    /// native representation).
    pub created: Option<i64>,
    pub author: Option<UserRef>,
}
