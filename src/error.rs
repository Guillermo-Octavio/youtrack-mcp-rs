//! Shared error types for the YouTrack MCP server.
//!
//! [`YouTrackError`] models failures from the YouTrack REST API and the
//! HTTP transport. [`ToolError`] is the actionable, tool-facing error
//! shape that every MCP tool handler maps a [`YouTrackError`] into,
//! carrying a stable machine-readable `code` plus a human-readable
//! `hint` that suggests a next step.

use std::time::Duration;

use serde::Serialize;
use thiserror::Error;

use crate::payload::Render;

/// Failure modes surfaced by the YouTrack HTTP client and REST API.
#[derive(Debug, Error)]
pub enum YouTrackError {
    /// The YouTrack token is missing, expired, or otherwise invalid.
    #[error("unauthorized: invalid or missing YouTrack token")]
    Unauthorized,

    /// The token is valid but lacks permission for the requested operation.
    #[error("forbidden: insufficient permissions for this operation")]
    Forbidden,

    /// The requested resource does not exist.
    #[error("{resource} not found: {id}")]
    NotFound { resource: String, id: String },

    /// The request failed local or server-side validation.
    #[error("validation failed: {message}")]
    Validation {
        field: Option<String>,
        message: String,
    },

    /// The YouTrack API throttled this client.
    #[error("rate limited by the YouTrack API")]
    RateLimited { retry_after: Option<Duration> },

    /// The YouTrack API returned a non-success status this client does
    /// not otherwise model explicitly.
    #[error("YouTrack API error {status}: {error}")]
    Api {
        status: u16,
        error: String,
        description: Option<String>,
    },

    /// The HTTP request failed at the transport level (DNS, TCP, TLS,
    /// timeout, and similar).
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    /// The response body could not be decoded into the expected shape.
    #[error("failed to decode response from {endpoint}: {source}")]
    Decode {
        endpoint: String,
        source: serde_json::Error,
    },

    /// A create/update request named one or more custom fields that
    /// aren't configured on the target project.
    #[error("unknown custom field(s): {}", unknown.join(", "))]
    UnknownCustomFields {
        unknown: Vec<String>,
        valid: Vec<String>,
    },

    /// An attachment download redirected to a different host than the
    /// configured `YOUTRACK_URL`. Rejected outright rather than followed,
    /// so this server's bearer token is never sent to an unexpected
    /// origin.
    #[error("attachment download redirected to a different host: {location}")]
    CrossHostRedirect { location: String },

    /// An attachment download exceeded the maximum number of bytes this
    /// client will buffer for one download.
    #[error("attachment is larger than the {max_bytes} byte download limit")]
    DownloadTooLarge { max_bytes: u64 },

    /// An attachment download followed more redirects than this client
    /// allows without reaching a final response.
    #[error("attachment download followed more than {max_redirects} redirects")]
    TooManyRedirects { max_redirects: u32 },
}

/// Actionable, tool-facing error produced from a [`YouTrackError`].
///
/// `code` is a stable machine-readable identifier an MCP client can
/// branch on; `hint` is an optional human-readable suggestion for the
/// caller's next step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolError {
    pub code: &'static str,
    pub message: String,
    pub hint: Option<String>,
}

impl ToolError {
    fn new(code: &'static str, message: String, hint: Option<String>) -> Self {
        Self {
            code,
            message,
            hint,
        }
    }
}

impl Render for ToolError {
    fn render(&self) -> String {
        match &self.hint {
            Some(hint) => format!("[{}] {} ({hint})", self.code, self.message),
            None => format!("[{}] {}", self.code, self.message),
        }
    }
}

impl From<YouTrackError> for ToolError {
    fn from(err: YouTrackError) -> Self {
        let message = err.to_string();
        match err {
            YouTrackError::Unauthorized => ToolError::new(
                "unauthorized",
                message,
                Some("Check that YOUTRACK_TOKEN is set and valid.".to_string()),
            ),
            YouTrackError::Forbidden => ToolError::new(
                "forbidden",
                message,
                Some("The provided token lacks permission for this operation.".to_string()),
            ),
            YouTrackError::NotFound { resource, id } => {
                let hint = format!(
                    "No {resource} found with id `{id}`. Try a listing or search tool to find the correct id."
                );
                ToolError::new("not_found", message, Some(hint))
            }
            YouTrackError::Validation { field, .. } => {
                let hint = field.map(|f| format!("Check the `{f}` field and try again."));
                ToolError::new("validation", message, hint)
            }
            YouTrackError::RateLimited { retry_after } => {
                let hint = Some(match retry_after {
                    Some(duration) => {
                        format!("Wait {}s before retrying.", duration.as_secs())
                    }
                    None => "Wait a moment before retrying.".to_string(),
                });
                ToolError::new("rate_limited", message, hint)
            }
            YouTrackError::Api { description, .. } => {
                let hint = description.or_else(|| {
                    Some("Check the YouTrack API documentation for this status code.".to_string())
                });
                ToolError::new("api_error", message, hint)
            }
            YouTrackError::Network(_) => ToolError::new(
                "network",
                message,
                Some("Check network connectivity to the YouTrack server.".to_string()),
            ),
            YouTrackError::Decode { endpoint, .. } => {
                let hint = format!(
                    "Unexpected response shape from {endpoint}; the YouTrack API may have changed."
                );
                ToolError::new("decode", message, Some(hint))
            }
            YouTrackError::UnknownCustomFields { unknown, valid } => {
                let hint = if valid.is_empty() {
                    format!(
                        "This project has no custom fields configured; remove {} from `fields`.",
                        unknown.join(", ")
                    )
                } else {
                    format!(
                        "Valid custom fields for this project: {}.",
                        valid.join(", ")
                    )
                };
                ToolError::new("unknown_custom_field", message, Some(hint))
            }
            YouTrackError::CrossHostRedirect { location } => {
                let hint = format!(
                    "The download redirected to `{location}`, a different host than the \
                     configured YOUTRACK_URL. This request was rejected to avoid sending \
                     credentials to an unexpected origin."
                );
                ToolError::new("cross_host_redirect", message, Some(hint))
            }
            YouTrackError::DownloadTooLarge { max_bytes } => {
                let hint = format!(
                    "This server refuses to buffer an attachment larger than {max_bytes} bytes."
                );
                ToolError::new("download_too_large", message, Some(hint))
            }
            YouTrackError::TooManyRedirects { .. } => ToolError::new(
                "too_many_redirects",
                message,
                Some("The attachment download endpoint redirected too many times; try again later or check the YouTrack server configuration.".to_string()),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unauthorized_maps_to_tool_error_with_token_hint() {
        let tool_error: ToolError = YouTrackError::Unauthorized.into();

        assert_eq!(tool_error.code, "unauthorized");
        assert!(tool_error.hint.unwrap().contains("YOUTRACK_TOKEN"));
    }

    #[test]
    fn forbidden_maps_to_tool_error_with_permission_hint() {
        let tool_error: ToolError = YouTrackError::Forbidden.into();

        assert_eq!(tool_error.code, "forbidden");
        assert!(tool_error.hint.unwrap().contains("permission"));
    }

    #[test]
    fn not_found_hint_mentions_resource_and_id_for_issue() {
        let tool_error: ToolError = YouTrackError::NotFound {
            resource: "issue".to_string(),
            id: "DEMO-1".to_string(),
        }
        .into();

        assert_eq!(tool_error.code, "not_found");
        let hint = tool_error.hint.unwrap();
        assert!(hint.contains("issue"));
        assert!(hint.contains("DEMO-1"));
    }

    #[test]
    fn not_found_hint_mentions_resource_and_id_for_article() {
        let tool_error: ToolError = YouTrackError::NotFound {
            resource: "article".to_string(),
            id: "KB-A-9".to_string(),
        }
        .into();

        let hint = tool_error.hint.unwrap();
        assert!(hint.contains("article"));
        assert!(hint.contains("KB-A-9"));
    }

    #[test]
    fn validation_with_field_includes_field_name_in_hint() {
        let tool_error: ToolError = YouTrackError::Validation {
            field: Some("summary".to_string()),
            message: "must not be empty".to_string(),
        }
        .into();

        assert_eq!(tool_error.code, "validation");
        assert!(tool_error.hint.unwrap().contains("summary"));
    }

    #[test]
    fn validation_without_field_has_no_hint() {
        let tool_error: ToolError = YouTrackError::Validation {
            field: None,
            message: "request body was malformed".to_string(),
        }
        .into();

        assert_eq!(tool_error.code, "validation");
        assert_eq!(tool_error.hint, None);
    }

    #[test]
    fn rate_limited_with_retry_after_includes_seconds_in_hint() {
        let tool_error: ToolError = YouTrackError::RateLimited {
            retry_after: Some(std::time::Duration::from_secs(30)),
        }
        .into();

        assert_eq!(tool_error.code, "rate_limited");
        assert!(tool_error.hint.unwrap().contains("30"));
    }

    #[test]
    fn rate_limited_without_retry_after_has_generic_hint() {
        let tool_error: ToolError = YouTrackError::RateLimited { retry_after: None }.into();

        assert_eq!(tool_error.code, "rate_limited");
        assert!(tool_error.hint.is_some());
    }

    #[test]
    fn api_error_uses_description_as_hint_when_present() {
        let tool_error: ToolError = YouTrackError::Api {
            status: 422,
            error: "invalid-value".to_string(),
            description: Some("field 'state' is not a valid state".to_string()),
        }
        .into();

        assert_eq!(tool_error.code, "api_error");
        assert_eq!(
            tool_error.hint.unwrap(),
            "field 'state' is not a valid state"
        );
    }

    #[test]
    fn api_error_without_description_gets_generic_hint() {
        let tool_error: ToolError = YouTrackError::Api {
            status: 500,
            error: "internal-error".to_string(),
            description: None,
        }
        .into();

        assert_eq!(tool_error.code, "api_error");
        assert!(tool_error.hint.is_some());
    }

    #[test]
    fn decode_hint_mentions_the_failing_endpoint() {
        let source = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        let tool_error: ToolError = YouTrackError::Decode {
            endpoint: "/api/issues".to_string(),
            source,
        }
        .into();

        assert_eq!(tool_error.code, "decode");
        assert!(tool_error.hint.unwrap().contains("/api/issues"));
    }

    #[test]
    fn render_includes_code_message_and_hint_when_hint_present() {
        let tool_error = ToolError {
            code: "not_found",
            message: "issue not found: DEMO-1".to_string(),
            hint: Some("Try a search tool to find the correct id.".to_string()),
        };

        let rendered = tool_error.render();

        assert_eq!(
            rendered,
            "[not_found] issue not found: DEMO-1 (Try a search tool to find the correct id.)"
        );
    }

    #[test]
    fn render_omits_hint_parens_when_hint_absent() {
        let tool_error = ToolError {
            code: "validation",
            message: "request body was malformed".to_string(),
            hint: None,
        };

        let rendered = tool_error.render();

        assert_eq!(rendered, "[validation] request body was malformed");
    }

    #[test]
    fn unknown_custom_fields_names_the_field_and_lists_the_valid_set_in_the_hint() {
        let tool_error: ToolError = YouTrackError::UnknownCustomFields {
            unknown: vec!["Bogus".to_string()],
            valid: vec!["Priority".to_string(), "State".to_string()],
        }
        .into();

        assert_eq!(tool_error.code, "unknown_custom_field");
        assert!(tool_error.message.contains("Bogus"));
        let hint = tool_error.hint.unwrap();
        assert!(hint.contains("Priority"));
        assert!(hint.contains("State"));
    }

    #[test]
    fn unknown_custom_fields_without_a_valid_set_still_names_the_field() {
        let tool_error: ToolError = YouTrackError::UnknownCustomFields {
            unknown: vec!["Bogus".to_string()],
            valid: vec![],
        }
        .into();

        assert_eq!(tool_error.code, "unknown_custom_field");
        assert!(tool_error.hint.unwrap().contains("Bogus"));
    }

    #[test]
    fn cross_host_redirect_hint_names_the_redirect_target() {
        let tool_error: ToolError = YouTrackError::CrossHostRedirect {
            location: "https://evil.example.com/steal".to_string(),
        }
        .into();

        assert_eq!(tool_error.code, "cross_host_redirect");
        assert!(tool_error.hint.unwrap().contains("evil.example.com"));
    }

    #[test]
    fn download_too_large_hint_names_the_byte_limit() {
        let tool_error: ToolError = YouTrackError::DownloadTooLarge {
            max_bytes: 52_428_800,
        }
        .into();

        assert_eq!(tool_error.code, "download_too_large");
        assert!(tool_error.hint.unwrap().contains("52428800"));
    }

    #[test]
    fn too_many_redirects_maps_to_a_structured_tool_error() {
        let tool_error: ToolError = YouTrackError::TooManyRedirects { max_redirects: 5 }.into();

        assert_eq!(tool_error.code, "too_many_redirects");
        assert!(tool_error.hint.is_some());
    }
}
