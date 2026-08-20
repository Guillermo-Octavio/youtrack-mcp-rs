use crate::payload::Render;
use crate::render::format_epoch_millis;
use crate::render::issue::render_field_value;
use crate::youtrack::activity::ActivityItem;

impl Render for ActivityItem {
    fn render(&self) -> String {
        let who = self
            .author
            .as_ref()
            .and_then(|author| author.full_name.clone().or_else(|| author.login.clone()))
            .unwrap_or_else(|| "?".to_string());
        let when = format_epoch_millis(self.timestamp);
        let field = self
            .field
            .as_ref()
            .and_then(|field| field.presentation.clone())
            .unwrap_or_else(|| "change".to_string());
        let from = render_field_value(&self.removed);
        let to = render_field_value(&self.added);

        format!("[{when}] {who} · {field}: {from} → {to}")
    }
}

#[cfg(test)]
mod tests {
    use crate::payload::Render;
    use crate::youtrack::activity::{ActivityField, ActivityItem};
    use crate::youtrack::issue::UserRef;

    #[test]
    fn render_names_who_when_field_and_the_change() {
        let item = ActivityItem {
            timestamp: Some(0),
            author: Some(UserRef {
                login: Some("jdoe".to_string()),
                full_name: Some("Jane Doe".to_string()),
            }),
            field: Some(ActivityField {
                presentation: Some("State".to_string()),
            }),
            removed: serde_json::json!({ "name": "Open" }),
            added: serde_json::json!({ "name": "Fixed" }),
        };

        assert_eq!(item.render(), "[1970-01-01] Jane Doe · State: Open → Fixed");
    }

    #[test]
    fn render_uses_dashes_for_null_added_or_removed_values() {
        let item = ActivityItem {
            timestamp: Some(0),
            author: None,
            field: Some(ActivityField {
                presentation: Some("Comments".to_string()),
            }),
            removed: serde_json::Value::Null,
            added: serde_json::json!({ "text": "Looks good" }),
        };

        let rendered = item.render();

        assert!(rendered.contains("— →"));
        assert!(rendered.contains("Looks good"));
    }

    #[test]
    fn render_falls_back_to_a_generic_field_label_when_absent() {
        let item = ActivityItem::default();

        assert!(item.render().contains("change:"));
    }
}
