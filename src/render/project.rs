use crate::payload::Render;
use crate::youtrack::project::{Project, ProjectCustomField};

impl Render for Project {
    fn render(&self) -> String {
        format!(
            "{:<12} {:<8} {}",
            self.short_name.as_deref().unwrap_or("?"),
            self.id.as_deref().unwrap_or("?"),
            self.name.as_deref().unwrap_or("")
        )
    }
}

impl Render for ProjectCustomField {
    fn render(&self) -> String {
        let field = self.field.clone().unwrap_or_default();
        let value_type = field
            .field_type
            .as_ref()
            .and_then(|field_type| field_type.value_type.as_deref())
            .unwrap_or("?");
        let required = if self.can_be_empty {
            ""
        } else {
            "  (required)"
        };
        let mut line = format!(
            "{} [{value_type}]{required}",
            field.name.unwrap_or_default()
        );

        if let Some(bundle) = &self.bundle {
            let values: Vec<&str> = bundle
                .values
                .iter()
                .filter(|value| !value.archived)
                .filter_map(|value| {
                    value
                        .name
                        .as_deref()
                        .or(value.full_name.as_deref())
                        .or(value.login.as_deref())
                })
                .collect();
            if !values.is_empty() {
                line.push_str(&format!("\n    Values: {}", values.join(", ")));
            }
        }

        line
    }
}
