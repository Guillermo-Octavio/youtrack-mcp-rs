use crate::payload::Render;
use crate::youtrack::agile::{AgileBoard, Sprint, SprintDetail, SprintIssue};

/// Formats a YouTrack epoch-millisecond timestamp. YouTrack does not
/// consistently return calendar dates, so this keeps the raw value
/// rather than pulling in a date/time dependency for display only.
fn render_timestamp(value: Option<i64>) -> String {
    match value {
        Some(ms) => format!("{ms} (epoch ms)"),
        None => "—".to_string(),
    }
}

impl Render for AgileBoard {
    fn render(&self) -> String {
        let mut lines = vec![format!(
            "{}  (id: {})",
            self.name.as_deref().unwrap_or("?"),
            self.id.as_deref().unwrap_or("?")
        )];

        if !self.projects.is_empty() {
            let names: Vec<&str> = self
                .projects
                .iter()
                .filter_map(|project| project.short_name.as_deref().or(project.name.as_deref()))
                .collect();
            lines.push(format!("  Projects: {}", names.join(", ")));
        }

        if let Some(current) = &self.current_sprint {
            lines.push(format!(
                "  Current sprint: {} (id: {})",
                current.name.as_deref().unwrap_or("?"),
                current.id.as_deref().unwrap_or("?")
            ));
        }

        lines.push(format!("  Sprints: {}", self.sprints.len()));
        lines.join("\n")
    }
}

impl Render for Sprint {
    fn render(&self) -> String {
        let flag = if self.archived { " [archived]" } else { "" };
        let mut lines = vec![format!(
            "{} (id: {})  {} → {}{flag}",
            self.name.as_deref().unwrap_or("?"),
            self.id.as_deref().unwrap_or("?"),
            render_timestamp(self.start),
            render_timestamp(self.finish)
        )];

        if let Some(goal) = self.goal.as_deref().filter(|goal| !goal.is_empty()) {
            lines.push(format!("  Goal: {goal}"));
        }

        lines.join("\n")
    }
}

impl Render for SprintIssue {
    fn render(&self) -> String {
        let field_named = |target: &str| {
            self.custom_fields
                .iter()
                .find(|field| {
                    field
                        .name
                        .as_deref()
                        .is_some_and(|name| name.eq_ignore_ascii_case(target))
                })
                .map(|field| &field.value)
        };
        let state = field_named("state").and_then(|value| value.get("name")?.as_str());
        let assignee = field_named("assignee").and_then(|value| {
            value
                .get("fullName")
                .or_else(|| value.get("login"))?
                .as_str()
        });

        let tag = state.map(|state| format!("[{state}]")).unwrap_or_default();
        let who = assignee
            .map(|assignee| format!(" — {assignee}"))
            .unwrap_or_default();

        format!(
            "  {:<10} {:<16} {}{who}",
            self.id_readable.as_deref().unwrap_or("?"),
            tag,
            self.summary.as_deref().unwrap_or("")
        )
    }
}

impl Render for SprintDetail {
    fn render(&self) -> String {
        let flag = if self.archived { " [archived]" } else { "" };
        let mut lines = vec![
            format!(
                "Sprint: {} (id: {})",
                self.name.as_deref().unwrap_or("?"),
                self.id.as_deref().unwrap_or("?")
            ),
            format!(
                "Dates: {} → {}{flag}",
                render_timestamp(self.start),
                render_timestamp(self.finish)
            ),
        ];

        if let Some(goal) = self.goal.as_deref().filter(|goal| !goal.is_empty()) {
            lines.push(format!("Goal: {goal}"));
        }
        lines.push(format!("Issues: {}", self.issues.len()));

        if self.issues.is_empty() {
            lines.push(String::new());
            lines.push("(No issues in the sprint.)".to_string());
        } else {
            lines.push(String::new());
            lines.extend(self.issues.iter().map(Render::render));
        }

        lines.join("\n")
    }
}
