//! Tool payload rendering.
//!
//! Every MCP tool result needs both a human-readable text rendering
//! (for chat-style clients) and a structured JSON value (for clients
//! that consume `structuredContent`). [`Render`] and [`ToolPayload`]
//! produce both from a single value so tool handlers only implement
//! `render` once. Wiring this into `rmcp`'s `CallToolResult` /
//! `structuredContent` happens in the MCP transport phase, once `rmcp`
//! is integrated; this module only implements the pure pairing logic.

use serde::Serialize;

/// Produces a human-readable text rendering of a tool result value, in
/// addition to its structured JSON representation.
pub trait Render {
    fn render(&self) -> String;
}

/// Pairs a value's rendered text form with its structured JSON form.
#[derive(Debug, Clone)]
pub struct ToolPayload {
    pub text: String,
    pub json: serde_json::Value,
}

impl ToolPayload {
    /// Builds a [`ToolPayload`] from any value that is both
    /// [`Serialize`] and implements [`Render`], producing the JSON
    /// value and the rendered text from the same source value.
    pub fn from_value<T>(value: &T) -> Result<Self, serde_json::Error>
    where
        T: Serialize + Render,
    {
        let json = serde_json::to_value(value)?;
        let text = value.render();
        Ok(Self { text, json })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Serialize)]
    struct IssueSummary {
        id: String,
        summary: String,
    }

    impl Render for IssueSummary {
        fn render(&self) -> String {
            format!("{}: {}", self.id, self.summary)
        }
    }

    #[derive(Serialize)]
    struct ProjectCount {
        project: String,
        count: u32,
    }

    impl Render for ProjectCount {
        fn render(&self) -> String {
            format!("{} has {} issues", self.project, self.count)
        }
    }

    #[test]
    fn from_value_pairs_rendered_text_with_json_for_issue_summary() {
        let value = IssueSummary {
            id: "DEMO-1".to_string(),
            summary: "Fix the thing".to_string(),
        };

        let payload = ToolPayload::from_value(&value).unwrap();

        assert_eq!(payload.text, "DEMO-1: Fix the thing");
        assert_eq!(payload.json["id"], "DEMO-1");
        assert_eq!(payload.json["summary"], "Fix the thing");
    }

    #[test]
    fn from_value_pairs_rendered_text_with_json_for_project_count() {
        let value = ProjectCount {
            project: "DEMO".to_string(),
            count: 7,
        };

        let payload = ToolPayload::from_value(&value).unwrap();

        assert_eq!(payload.text, "DEMO has 7 issues");
        assert_eq!(payload.json["project"], "DEMO");
        assert_eq!(payload.json["count"], 7);
    }
}
