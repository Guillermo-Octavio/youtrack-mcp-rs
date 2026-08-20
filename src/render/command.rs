use crate::payload::Render;

/// The result of a successful `apply_command`/`bulk_command` call,
/// self-describing so the rendered text confirms which issue(s) the
/// command was applied to. Both MCP tools share this shape: a single
/// issue is just a one-element `issue_ids`.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct CommandResult {
    pub issue_ids: Vec<String>,
    pub command: String,
}

impl Render for CommandResult {
    fn render(&self) -> String {
        match self.issue_ids.as_slice() {
            [] => format!("Command '{}' applied to no issues.", self.command),
            [only] => format!("Command applied to {}: {}", only, self.command),
            many => format!(
                "Command '{}' applied to {} issue(s): {}",
                self.command,
                many.len(),
                many.join(", ")
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_names_the_single_issue_for_one_issue_id() {
        let result = CommandResult {
            issue_ids: vec!["DEMO-1".to_string()],
            command: "State Fixed".to_string(),
        };

        assert_eq!(result.render(), "Command applied to DEMO-1: State Fixed");
    }

    #[test]
    fn render_lists_every_issue_and_the_count_for_multiple_issue_ids() {
        let result = CommandResult {
            issue_ids: vec!["DEMO-1".to_string(), "DEMO-2".to_string()],
            command: "State Fixed".to_string(),
        };

        assert_eq!(
            result.render(),
            "Command 'State Fixed' applied to 2 issue(s): DEMO-1, DEMO-2"
        );
    }

    #[test]
    fn render_reports_no_issues_when_the_list_is_empty() {
        let result = CommandResult {
            issue_ids: vec![],
            command: "State Fixed".to_string(),
        };

        assert_eq!(
            result.render(),
            "Command 'State Fixed' applied to no issues."
        );
    }
}
