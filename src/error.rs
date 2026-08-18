//! Shared error types for the YouTrack MCP server.
//!
//! [`YouTrackError`] models failures from the YouTrack REST API and the
//! HTTP transport. [`ToolError`] is the actionable, tool-facing error
//! shape that every MCP tool handler maps a [`YouTrackError`] into,
//! carrying a stable machine-readable `code` plus a human-readable
//! `hint` that suggests a next step.

use std::time::Duration;

use thiserror::Error;

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
}

/// Actionable, tool-facing error produced from a [`YouTrackError`].
///
/// `code` is a stable machine-readable identifier an MCP client can
/// branch on; `hint` is an optional human-readable suggestion for the
/// caller's next step.
#[derive(Debug, Clone, PartialEq, Eq)]
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
}
