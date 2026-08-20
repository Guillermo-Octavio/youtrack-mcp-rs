//! Issue-attachment-domain models: [`IssueAttachment`].

use serde::{Deserialize, Serialize};

/// One attachment on an issue, as returned by `list_attachments`/
/// `upload_attachment`/`get_attachment`. `url` is the host-root-relative
/// download URL YouTrack issues for this attachment (NOT under `/api`,
/// unlike every other endpoint this client calls); see
/// `YouTrackClient::download_attachment_bytes`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct IssueAttachment {
    pub id: Option<String>,
    pub name: Option<String>,
    pub mime_type: Option<String>,
    pub size: Option<u64>,
    pub url: Option<String>,
}
