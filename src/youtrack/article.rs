//! Knowledge-base article-domain models: [`Article`] and
//! [`ArticleAttachment`].

use serde::{Deserialize, Serialize};

/// A reference to another article, nested in an [`Article`] as its
/// `parentArticle` when the article is nested under another one.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ArticleRef {
    pub id_readable: Option<String>,
    pub summary: Option<String>,
}

/// A single YouTrack knowledge-base article, as returned by
/// `list_articles`/`get_article`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Article {
    pub id: Option<String>,
    pub id_readable: Option<String>,
    pub summary: Option<String>,
    pub content: Option<String>,
    pub parent_article: Option<ArticleRef>,
}

/// One attachment on a knowledge-base article, as returned by
/// `list_article_attachments`/`upload_article_attachment`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ArticleAttachment {
    pub id: Option<String>,
    pub name: Option<String>,
    pub mime_type: Option<String>,
    pub size: Option<u64>,
    pub url: Option<String>,
}
