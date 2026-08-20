use crate::payload::Render;
use crate::youtrack::issue::UserRef;
use crate::youtrack::user::User;

impl Render for User {
    fn render(&self) -> String {
        let who = self
            .full_name
            .as_deref()
            .or(self.login.as_deref())
            .unwrap_or("?");

        let mut extra = Vec::new();
        if let Some(login) = &self.login {
            extra.push(format!("@{login}"));
        }
        if let Some(email) = &self.email {
            extra.push(email.clone());
        }

        if extra.is_empty() {
            who.to_string()
        } else {
            format!("{who}  ({})", extra.join(" · "))
        }
    }
}

/// The watchers of one issue, as returned by `list_watchers`. Wraps
/// [`Vec<UserRef>`] with the source issue id so the rendered text can
/// name it, matching every other tool's self-describing output (see
/// [`crate::render::IssueLinks`]).
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct Watchers {
    pub issue_id: String,
    pub watchers: Vec<UserRef>,
}

impl Render for Watchers {
    fn render(&self) -> String {
        if self.watchers.is_empty() {
            return format!("{} has no watchers.", self.issue_id);
        }

        let names: Vec<String> = self
            .watchers
            .iter()
            .filter_map(|watcher| watcher.full_name.clone().or_else(|| watcher.login.clone()))
            .collect();

        format!(
            "Watchers for {} ({}):\n{}",
            self.issue_id,
            names.len(),
            names
                .iter()
                .map(|name| format!("  {name}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::payload::Render;
    use crate::render::Watchers;
    use crate::youtrack::issue::UserRef;
    use crate::youtrack::user::User;

    #[test]
    fn render_prefers_full_name_and_lists_login_and_email() {
        let user = User {
            login: Some("jdoe".to_string()),
            full_name: Some("Jane Doe".to_string()),
            email: Some("jane@example.com".to_string()),
        };

        assert_eq!(user.render(), "Jane Doe  (@jdoe · jane@example.com)");
    }

    #[test]
    fn render_falls_back_to_login_and_placeholder() {
        let with_login = User {
            login: Some("jdoe".to_string()),
            ..Default::default()
        };
        let empty = User::default();

        assert_eq!(with_login.render(), "jdoe  (@jdoe)");
        assert_eq!(empty.render(), "?");
    }

    #[test]
    fn watchers_render_lists_every_watcher_by_name() {
        let watchers = Watchers {
            issue_id: "DEMO-1".to_string(),
            watchers: vec![
                UserRef {
                    login: Some("jdoe".to_string()),
                    full_name: Some("Jane Doe".to_string()),
                },
                UserRef {
                    login: Some("asmith".to_string()),
                    full_name: None,
                },
            ],
        };

        assert_eq!(
            watchers.render(),
            "Watchers for DEMO-1 (2):\n  Jane Doe\n  asmith"
        );
    }

    #[test]
    fn watchers_render_reports_none_when_empty() {
        let watchers = Watchers {
            issue_id: "DEMO-1".to_string(),
            watchers: vec![],
        };

        assert_eq!(watchers.render(), "DEMO-1 has no watchers.");
    }
}
