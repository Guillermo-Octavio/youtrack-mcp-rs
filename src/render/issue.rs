use crate::payload::Render;
use crate::youtrack::issue::{CustomField, Issue, IssueLink};

/// Renders a custom field's raw JSON value the way YouTrack's own
/// clients present it: an object's `name`/`fullName`/`login` (in that
/// order), an array joined with commas, a scalar as-is, or `—` when
/// absent.
pub(super) fn render_field_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "—".to_string(),
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Array(items) => {
            let rendered: Vec<String> = items
                .iter()
                .map(render_field_value)
                .filter(|text| text != "—")
                .collect();
            if rendered.is_empty() {
                "—".to_string()
            } else {
                rendered.join(", ")
            }
        }
        serde_json::Value::Object(map) => ["name", "fullName", "login", "presentation", "text"]
            .iter()
            .find_map(|key| map.get(*key).and_then(|v| v.as_str()))
            .map(str::to_string)
            .unwrap_or_else(|| value.to_string()),
        other => other.to_string(),
    }
}

impl Render for CustomField {
    fn render(&self) -> String {
        format!(
            "  {}: {}",
            self.name.as_deref().unwrap_or("field"),
            render_field_value(&self.value)
        )
    }
}

impl Render for Issue {
    fn render(&self) -> String {
        let mut lines = vec![format!(
            "{}  —  {}",
            self.id_readable.as_deref().unwrap_or("?"),
            self.summary.as_deref().unwrap_or("")
        )];

        if let Some(project) = &self.project {
            lines.push(format!(
                "  Project: {} ({})",
                project.short_name.as_deref().unwrap_or("?"),
                project.name.as_deref().unwrap_or("?")
            ));
        }

        if let Some(reporter) = &self.reporter {
            if let Some(who) = reporter.full_name.as_deref().or(reporter.login.as_deref()) {
                lines.push(format!("  Reporter: {who}"));
            }
        }

        lines.extend(self.custom_fields.iter().map(Render::render));

        if let Some(description) = self.description.as_deref().filter(|d| !d.is_empty()) {
            lines.push("  ---".to_string());
            lines.push(format!("  {}", description.replace('\n', "\n  ")));
        }

        lines.join("\n")
    }
}

impl Render for IssueLink {
    fn render(&self) -> String {
        let link_type = self.link_type.clone().unwrap_or_default();
        let label = match self.direction.as_deref() {
            Some("INWARD") => link_type.target_to_source.or(link_type.name),
            _ => link_type.source_to_target.or(link_type.name),
        }
        .unwrap_or_else(|| "related to".to_string());

        self.issues
            .iter()
            .map(|linked| {
                format!(
                    "  {label}: {} — {}",
                    linked.id_readable.as_deref().unwrap_or("?"),
                    linked.summary.as_deref().unwrap_or("")
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// The full set of links for one issue, as returned by `list_links`.
/// Wraps [`Vec<IssueLink>`] with the source issue id so the rendered
/// text can name it, matching every other tool's self-describing output.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct IssueLinks {
    pub issue_id: String,
    pub links: Vec<IssueLink>,
}

/// The result of a successful `delete_issue` call, self-describing so
/// the rendered text confirms which issue was permanently removed.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct DeleteIssueResult {
    pub issue_id: String,
    pub deleted: bool,
}

impl Render for DeleteIssueResult {
    fn render(&self) -> String {
        format!(
            "Issue {} permanently deleted. This cannot be undone.",
            self.issue_id
        )
    }
}

impl Render for IssueLinks {
    fn render(&self) -> String {
        let lines: Vec<String> = self
            .links
            .iter()
            .map(Render::render)
            .filter(|rendered| !rendered.is_empty())
            .collect();

        if lines.is_empty() {
            return format!("{} has no links.", self.issue_id);
        }

        format!("Links for {}:\n{}", self.issue_id, lines.join("\n"))
    }
}
