use crate::payload::Render;
use crate::render::format_epoch_millis;
use crate::youtrack::work_item::WorkItem;

impl Render for WorkItem {
    fn render(&self) -> String {
        let who = self
            .author
            .as_ref()
            .and_then(|author| author.full_name.clone().or_else(|| author.login.clone()))
            .unwrap_or_else(|| "?".to_string());
        let when = format_epoch_millis(self.date);
        let duration = self
            .duration
            .as_ref()
            .and_then(|duration| duration.presentation.clone())
            .unwrap_or_else(|| "?".to_string());

        let mut line = format!(
            "[{when}] {who} — {duration} (id: {})",
            self.id.as_deref().unwrap_or("?")
        );

        if let Some(kind) = self.kind.as_ref().and_then(|kind| kind.name.as_deref()) {
            line.push_str(&format!(" ({kind})"));
        }

        if let Some(text) = self
            .text
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
        {
            line.push_str(&format!("\n  {}", text.replace('\n', "\n  ")));
        }

        line
    }
}

/// The result of a successful `delete_work_item` call, self-describing
/// so the rendered text confirms which work item was permanently
/// removed (see [`crate::render::DeleteIssueResult`]).
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct DeleteWorkItemResult {
    pub issue_id: String,
    pub work_item_id: String,
    pub deleted: bool,
}

impl Render for DeleteWorkItemResult {
    fn render(&self) -> String {
        format!(
            "Work item {} permanently deleted from {}. This cannot be undone.",
            self.work_item_id, self.issue_id
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::payload::Render;
    use crate::youtrack::issue::UserRef;
    use crate::youtrack::work_item::{WorkItem, WorkItemDuration, WorkItemType};

    fn base_item() -> WorkItem {
        WorkItem {
            id: Some("1".to_string()),
            date: Some(0),
            duration: Some(WorkItemDuration {
                minutes: Some(90),
                presentation: Some("1h 30m".to_string()),
            }),
            text: None,
            kind: None,
            author: Some(UserRef {
                login: Some("jdoe".to_string()),
                full_name: Some("Jane Doe".to_string()),
            }),
        }
    }

    #[test]
    fn render_names_who_when_and_duration() {
        let item = base_item();

        assert_eq!(item.render(), "[1970-01-01] Jane Doe — 1h 30m (id: 1)");
    }

    #[test]
    fn render_appends_the_work_item_type_when_present() {
        let mut item = base_item();
        item.kind = Some(WorkItemType {
            name: Some("Development".to_string()),
        });

        assert!(item.render().ends_with("(id: 1) (Development)"));
    }

    #[test]
    fn render_appends_indented_text_when_present() {
        let mut item = base_item();
        item.text = Some("Fixed the bug\nAnd tested it".to_string());

        let rendered = item.render();

        assert!(rendered.contains("\n  Fixed the bug\n  And tested it"));
    }
}
