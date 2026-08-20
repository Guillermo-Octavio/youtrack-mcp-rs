//! Local filesystem I/O shared by every attachment-domain tool that reads
//! a caller-supplied local file to upload, or writes a downloaded
//! attachment to a caller-supplied local destination.
//!
//! Both directions share the same core security policy: never cross a
//! `..` traversal segment, and never read or write through a symlink,
//! checked with [`fs::symlink_metadata`] (which does not follow a final
//! symlink) before any bytes move.

use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::error::ToolError;

/// Maximum size, in bytes, of a local file this server will read for an
/// upload, or write for a download (50 MiB). Chosen to comfortably cover
/// typical attachments (docs, images, small archives) while bounding
/// memory use and transfer time for one MCP call. Uploads enforce this
/// from local file metadata before reading; downloads enforce the same
/// number by counting streamed bytes as they arrive (see
/// `YouTrackClient::download_attachment_bytes`), since a remote response
/// has no filesystem metadata to check up front.
pub(super) const MAX_ATTACHMENT_SIZE_BYTES: u64 = 50 * 1024 * 1024;

/// Reads a caller-supplied local file path for an attachment upload
/// (`upload_attachment`, `upload_article_attachment`), enforcing this
/// server's attachment upload security policy before any bytes are read:
///
/// 1. Reject any path containing a `..` (parent-directory) component,
///    checked on the raw path components before touching the filesystem
///    at all — this closes off traversal attempts regardless of whether
///    they would resolve to a real file.
/// 2. Inspect the path's own [`fs::symlink_metadata`] (which does not
///    follow a final symlink) and reject it if it is a symlink, or if it
///    isn't a regular file (e.g. a directory): this server never silently
///    follows a symlink to read unintended file content.
/// 3. Reject the file if it is larger than [`MAX_ATTACHMENT_SIZE_BYTES`]
///    (50 MiB), checked from that same metadata before any bytes are read.
/// 4. Read the file's bytes and guess its MIME type from its extension.
///
/// Returns the file's base name, guessed MIME type, and bytes on success,
/// or a structured [`ToolError`] naming exactly which check failed.
pub(super) fn read_attachment_file(
    file_path: &str,
) -> Result<(String, String, Vec<u8>), ToolError> {
    let path = Path::new(file_path);

    if path.components().any(|c| c == Component::ParentDir) {
        return Err(ToolError {
            code: "path_traversal",
            message: format!("refusing to read `{file_path}`: path contains `..`"),
            hint: Some(
                "Pass an absolute path, or a relative path with no `..` segments.".to_string(),
            ),
        });
    }

    let metadata = fs::symlink_metadata(path).map_err(|_| ToolError {
        code: "file_not_found",
        message: format!("no local file found at `{file_path}`"),
        hint: Some("Check the path and try again.".to_string()),
    })?;

    if metadata.file_type().is_symlink() {
        return Err(ToolError {
            code: "symlink_rejected",
            message: format!("refusing to read `{file_path}`: it is a symlink"),
            hint: Some("Upload the real file directly instead of a symlink to it.".to_string()),
        });
    }

    if !metadata.is_file() {
        return Err(ToolError {
            code: "invalid_path",
            message: format!("`{file_path}` is not a regular file"),
            hint: Some("Pass the path to a regular file.".to_string()),
        });
    }

    if metadata.len() > MAX_ATTACHMENT_SIZE_BYTES {
        return Err(ToolError {
            code: "file_too_large",
            message: format!(
                "`{file_path}` is {} bytes, which exceeds the {MAX_ATTACHMENT_SIZE_BYTES} byte \
                 upload limit",
                metadata.len()
            ),
            hint: Some("Upload a smaller file, or split it before uploading.".to_string()),
        });
    }

    let bytes = fs::read(path).map_err(|source| ToolError {
        code: "io_error",
        message: format!("failed to read `{file_path}`: {source}"),
        hint: None,
    })?;

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("upload.bin")
        .to_string();
    let mime_type = guess_mime_type(&file_name);

    Ok((file_name, mime_type, bytes))
}

/// Validates a caller-supplied local destination path for an attachment
/// download (`download_attachment`) before any bytes are fetched or
/// written, mirroring [`read_attachment_file`]'s upload-side policy:
///
/// 1. Reject any path containing a `..` (parent-directory) component,
///    checked on the raw path components before touching the filesystem.
/// 2. If a filesystem entry already exists at that exact path, inspect
///    its [`fs::symlink_metadata`] (which does not follow a final
///    symlink): reject it if it is a symlink (this server never writes
///    through a symlink to an unintended real destination) or if it is a
///    directory (a destination path must name a file, not a directory).
///    A path with no existing entry yet is accepted — it will be created
///    fresh by the download write.
///
/// Returns the validated path on success, or a structured [`ToolError`]
/// naming exactly which check failed.
pub(super) fn validate_destination_path(destination_path: &str) -> Result<PathBuf, ToolError> {
    let path = Path::new(destination_path);

    if path.components().any(|c| c == Component::ParentDir) {
        return Err(ToolError {
            code: "path_traversal",
            message: format!("refusing to write `{destination_path}`: path contains `..`"),
            hint: Some(
                "Pass an absolute path, or a relative path with no `..` segments.".to_string(),
            ),
        });
    }

    if let Ok(metadata) = fs::symlink_metadata(path) {
        if metadata.file_type().is_symlink() {
            return Err(ToolError {
                code: "symlink_rejected",
                message: format!("refusing to write `{destination_path}`: it is a symlink"),
                hint: Some("Choose a destination path that isn't a symlink.".to_string()),
            });
        }

        if metadata.is_dir() {
            return Err(ToolError {
                code: "invalid_path",
                message: format!("`{destination_path}` is a directory, not a file path"),
                hint: Some(
                    "Pass a destination path that names the file to write, not a directory."
                        .to_string(),
                ),
            });
        }
    }

    Ok(path.to_path_buf())
}

/// Writes `bytes` to `destination`, mapping any I/O failure to a
/// structured [`ToolError`] instead of a raw error. Callers must have
/// already validated `destination` with [`validate_destination_path`].
pub(super) fn write_downloaded_file(destination: &Path, bytes: &[u8]) -> Result<(), ToolError> {
    fs::write(destination, bytes).map_err(|source| ToolError {
        code: "io_error",
        message: format!("failed to write `{}`: {source}", destination.display()),
        hint: Some("Check that the destination directory exists and is writable.".to_string()),
    })
}

/// Best-effort MIME type guess from a file name's extension, mirroring
/// common types this server's attachment domains use. Falls back to
/// `application/octet-stream` for unrecognized or missing extensions.
pub(super) fn guess_mime_type(file_name: &str) -> String {
    let extension = Path::new(file_name)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_lowercase);

    match extension.as_deref() {
        Some("pdf") => "application/pdf",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("txt") => "text/plain",
        Some("md") => "text/markdown",
        Some("json") => "application/json",
        Some("csv") => "text/csv",
        Some("html") => "text/html",
        Some("zip") => "application/zip",
        Some("docx") => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        Some("xlsx") => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        _ => "application/octet-stream",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_temp_path(suffix: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "youtrack_mcp_rs_test_{}_{}_{suffix}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn guess_mime_type_recognizes_common_extensions() {
        assert_eq!(guess_mime_type("diagram.png"), "image/png");
        assert_eq!(guess_mime_type("report.PDF"), "application/pdf");
        assert_eq!(guess_mime_type("data.bin"), "application/octet-stream");
    }

    #[test]
    fn validate_destination_path_rejects_a_path_traversal_attempt() {
        let error = validate_destination_path("../../etc/passwd").unwrap_err();

        assert_eq!(error.code, "path_traversal");
    }

    #[test]
    fn validate_destination_path_rejects_an_existing_symlink() {
        let real_path = unique_temp_path("real_dest.txt");
        fs::write(&real_path, b"pre-existing").unwrap();
        let link_path = unique_temp_path("link_dest.txt");
        std::os::unix::fs::symlink(&real_path, &link_path).unwrap();

        let error = validate_destination_path(link_path.to_str().unwrap()).unwrap_err();

        fs::remove_file(&link_path).ok();
        fs::remove_file(&real_path).ok();

        assert_eq!(error.code, "symlink_rejected");
    }

    #[test]
    fn validate_destination_path_rejects_an_existing_directory() {
        let dir_path = unique_temp_path("dest_dir");
        fs::create_dir(&dir_path).unwrap();

        let error = validate_destination_path(dir_path.to_str().unwrap()).unwrap_err();

        fs::remove_dir(&dir_path).ok();

        assert_eq!(error.code, "invalid_path");
    }

    #[test]
    fn validate_destination_path_accepts_a_path_with_no_existing_entry() {
        let dest_path = unique_temp_path("new_dest.txt");

        let result = validate_destination_path(dest_path.to_str().unwrap());

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), dest_path);
    }

    #[test]
    fn validate_destination_path_accepts_an_existing_regular_file_to_overwrite() {
        let dest_path = unique_temp_path("existing_dest.txt");
        fs::write(&dest_path, b"old contents").unwrap();

        let result = validate_destination_path(dest_path.to_str().unwrap());

        fs::remove_file(&dest_path).ok();

        assert!(result.is_ok());
    }

    #[test]
    fn write_downloaded_file_writes_the_exact_bytes() {
        let dest_path = unique_temp_path("written.bin");

        write_downloaded_file(&dest_path, b"hello world").unwrap();
        let contents = fs::read(&dest_path).unwrap();

        fs::remove_file(&dest_path).ok();

        assert_eq!(contents, b"hello world");
    }

    #[test]
    fn write_downloaded_file_maps_an_unwritable_destination_to_a_structured_io_error() {
        let dest_path = PathBuf::from("/nonexistent-dir-for-test/file.bin");

        let error = write_downloaded_file(&dest_path, b"data").unwrap_err();

        assert_eq!(error.code, "io_error");
    }
}
