//! User-domain models: [`User`] (as returned by `get_current_user`/
//! `search_users`) and the watcher shapes nested under `GET /issues/{id}`.

use serde::{Deserialize, Serialize};

use crate::youtrack::issue::UserRef;

/// A YouTrack user, as returned by `GET /users/me` and `GET /users`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct User {
    pub login: Option<String>,
    pub full_name: Option<String>,
    pub email: Option<String>,
}

/// One entry in an issue's `watchers.issueWatchers` list.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct IssueWatcher {
    pub user: Option<UserRef>,
}

/// The `watchers` object nested under `GET /issues/{id}` when its
/// `fields` projection asks for `watchers(issueWatchers(user(...)))`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct IssueWatchers {
    pub issue_watchers: Vec<IssueWatcher>,
}

/// The minimal shape of `GET /issues/{id}` needed by
/// [`crate::client::YouTrackClient::list_watchers`], scoped to only the
/// `watchers` field its `fields` projection asked for. YouTrack has no
/// dedicated watchers endpoint, so watchers are read via this projection
/// on the issue itself.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct IssueWatchersResponse {
    pub watchers: Option<IssueWatchers>,
}
