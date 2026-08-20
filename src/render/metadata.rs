use crate::payload::Render;
use crate::youtrack::metadata::{SavedSearch, Tag};

impl Render for Tag {
    fn render(&self) -> String {
        self.name.clone().unwrap_or_else(|| "?".to_string())
    }
}

impl Render for SavedSearch {
    fn render(&self) -> String {
        let mut line = self.name.clone().unwrap_or_else(|| "?".to_string());

        if let Some(who) = self
            .owner
            .as_ref()
            .and_then(|owner| owner.full_name.clone().or_else(|| owner.login.clone()))
        {
            line.push_str(&format!("  (by {who})"));
        }

        if let Some(query) = self.query.as_deref().filter(|query| !query.is_empty()) {
            line.push_str(&format!("\n    {query}"));
        }

        line
    }
}

#[cfg(test)]
mod tests {
    use crate::payload::Render;
    use crate::youtrack::issue::UserRef;
    use crate::youtrack::metadata::{SavedSearch, Tag};

    #[test]
    fn tag_render_shows_its_name() {
        let tag = Tag {
            id: Some("1".to_string()),
            name: Some("urgent".to_string()),
        };

        assert_eq!(tag.render(), "urgent");
    }

    #[test]
    fn tag_render_falls_back_to_a_placeholder_when_unnamed() {
        assert_eq!(Tag::default().render(), "?");
    }

    #[test]
    fn saved_search_render_names_owner_and_query() {
        let search = SavedSearch {
            id: Some("1".to_string()),
            name: Some("My bugs".to_string()),
            query: Some("assignee: me #Bug".to_string()),
            owner: Some(UserRef {
                login: Some("jdoe".to_string()),
                full_name: Some("Jane Doe".to_string()),
            }),
        };

        assert_eq!(
            search.render(),
            "My bugs  (by Jane Doe)\n    assignee: me #Bug"
        );
    }

    #[test]
    fn saved_search_render_omits_owner_and_query_when_absent() {
        let search = SavedSearch {
            name: Some("My bugs".to_string()),
            ..Default::default()
        };

        assert_eq!(search.render(), "My bugs");
    }
}
