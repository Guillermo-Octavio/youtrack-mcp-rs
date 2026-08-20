use crate::payload::Render;
use crate::youtrack::article::{Article, ArticleAttachment};

impl Render for Article {
    fn render(&self) -> String {
        let mut lines = vec![format!(
            "{}  —  {}",
            self.id_readable.as_deref().unwrap_or("?"),
            self.summary.as_deref().unwrap_or("")
        )];

        if let Some(parent) = &self.parent_article {
            lines.push(format!(
                "  Under: {} — {}",
                parent.id_readable.as_deref().unwrap_or("?"),
                parent.summary.as_deref().unwrap_or("")
            ));
        }

        if let Some(content) = self.content.as_deref().filter(|c| !c.is_empty()) {
            lines.push("  ---".to_string());
            lines.push(format!("  {}", content.replace('\n', "\n  ")));
        }

        lines.join("\n")
    }
}

impl Render for ArticleAttachment {
    fn render(&self) -> String {
        format!(
            "{}  ({}, {} bytes)",
            self.name.as_deref().unwrap_or("?"),
            self.mime_type
                .as_deref()
                .unwrap_or("application/octet-stream"),
            self.size.unwrap_or(0)
        )
    }
}

/// The result of a successful `delete_article` call, self-describing so
/// the rendered text confirms which article was permanently removed.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct DeleteArticleResult {
    pub article_id: String,
    pub deleted: bool,
}

impl Render for DeleteArticleResult {
    fn render(&self) -> String {
        format!(
            "Article {} permanently deleted. This cannot be undone.",
            self.article_id
        )
    }
}
