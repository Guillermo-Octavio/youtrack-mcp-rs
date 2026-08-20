//! Miscellaneous project-metadata-domain models: [`Tag`] and
//! [`SavedSearch`], as returned by `list_tags`/`list_saved_searches`.

use serde::{Deserialize, Serialize};

use crate::youtrack::issue::UserRef;

/// An issue tag, as returned by `GET /tags`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Tag {
    pub id: Option<String>,
    pub name: Option<String>,
}

/// A saved search, as returned by `GET /savedQueries`. Its `query` can
/// be passed as-is to `search_issues`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SavedSearch {
    pub id: Option<String>,
    pub name: Option<String>,
    pub query: Option<String>,
    pub owner: Option<UserRef>,
}
