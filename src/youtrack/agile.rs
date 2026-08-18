//! Agile-board-domain models: [`AgileBoard`], [`Sprint`], and
//! [`SprintDetail`] (a sprint with its issues, as returned by
//! `get_sprint`).

use serde::{Deserialize, Serialize};

use super::issue::CustomField;

/// A project reference nested in an [`AgileBoard`].
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectShortRef {
    pub short_name: Option<String>,
    pub name: Option<String>,
}

/// A sprint reference nested in an [`AgileBoard`] (e.g. its current
/// sprint, or one entry in its full sprint list).
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SprintRef {
    pub id: Option<String>,
    pub name: Option<String>,
}

/// An agile board, as returned by `GET /agiles`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgileBoard {
    pub id: Option<String>,
    pub name: Option<String>,
    pub projects: Vec<ProjectShortRef>,
    pub current_sprint: Option<SprintRef>,
    pub sprints: Vec<SprintRef>,
}

/// A sprint on an agile board, as returned by
/// `GET /agiles/{board_id}/sprints` (no issues; see [`SprintDetail`] for
/// the single-sprint shape returned by `GET .../sprints/{sprint_id}`).
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Sprint {
    pub id: Option<String>,
    pub name: Option<String>,
    pub goal: Option<String>,
    pub start: Option<i64>,
    pub finish: Option<i64>,
    pub archived: bool,
}

/// One issue on a sprint, as embedded in [`SprintDetail`].
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SprintIssue {
    pub id_readable: Option<String>,
    pub summary: Option<String>,
    pub resolved: Option<i64>,
    pub custom_fields: Vec<CustomField>,
}

/// A single sprint with its issues, as returned by `get_sprint`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SprintDetail {
    pub id: Option<String>,
    pub name: Option<String>,
    pub goal: Option<String>,
    pub start: Option<i64>,
    pub finish: Option<i64>,
    pub archived: bool,
    pub issues: Vec<SprintIssue>,
}
