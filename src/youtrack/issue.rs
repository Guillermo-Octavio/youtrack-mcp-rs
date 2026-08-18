//! Issue-domain models: [`Issue`], its nested references, and
//! [`CustomField`], shared with the agile-board domain's sprint issues.

use serde::{Deserialize, Serialize};

/// A project reference nested in an [`Issue`].
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectRef {
    pub short_name: Option<String>,
    pub name: Option<String>,
}

/// A user reference nested in an [`Issue`] (e.g. its reporter).
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UserRef {
    pub login: Option<String>,
    pub full_name: Option<String>,
}

/// One custom field on an issue. `value` is left as raw JSON because its
/// shape depends on the field type: an object (state, assignee), an
/// array of objects (multi-value fields), or a scalar (text, number).
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CustomField {
    pub name: Option<String>,
    pub value: serde_json::Value,
}

/// A single YouTrack issue, as returned by `search_issues`/`get_issue`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Issue {
    pub id: Option<String>,
    pub id_readable: Option<String>,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub project: Option<ProjectRef>,
    pub reporter: Option<UserRef>,
    pub custom_fields: Vec<CustomField>,
}

/// A reference to another issue, as returned inside an [`IssueLink`].
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LinkedIssueRef {
    pub id_readable: Option<String>,
    pub summary: Option<String>,
}

/// The link type of an [`IssueLink`], e.g. "relates to" / "depends on".
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LinkType {
    pub name: Option<String>,
    pub source_to_target: Option<String>,
    pub target_to_source: Option<String>,
}

/// One relation from an issue to a set of other issues, as returned by
/// `GET /issues/{id}/links`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct IssueLink {
    pub direction: Option<String>,
    pub link_type: Option<LinkType>,
    pub issues: Vec<LinkedIssueRef>,
}
