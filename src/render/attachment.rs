use crate::payload::Render;
use crate::youtrack::attachment::IssueAttachment;

impl Render for IssueAttachment {
    fn render(&self) -> String {
        format!(
            "{}  ({}, {} bytes)",
            self.name.as_deref().unwrap_or("?"),
            self.mime_type
                .as_deref()
                .unwrap_or("application/octet-stream"),
            self.size.unwrap_or(0)
        )
    }
}

/// The result of a successful `download_attachment` call, self-describing
/// so the rendered text confirms which attachment was written where, and
/// how large the downloaded file is.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct DownloadAttachmentResult {
    pub issue_id: String,
    pub attachment_id: String,
    pub destination_path: String,
    pub bytes_written: u64,
}

impl Render for DownloadAttachmentResult {
    fn render(&self) -> String {
        format!(
            "Downloaded attachment `{}` from issue `{}` to `{}` ({} bytes).",
            self.attachment_id, self.issue_id, self.destination_path, self.bytes_written
        )
    }
}
