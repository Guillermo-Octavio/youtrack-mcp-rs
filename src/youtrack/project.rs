//! Project-metadata-domain models: [`Project`] and its configured
//! [`ProjectCustomField`]s.

use serde::{Deserialize, Serialize};

/// An admin-visible project, as returned by `GET /admin/projects`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Project {
    pub id: Option<String>,
    pub name: Option<String>,
    pub short_name: Option<String>,
}

/// The value type of a custom field's underlying field definition, e.g.
/// `enum[1]` or `state[1]`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FieldType {
    pub value_type: Option<String>,
}

/// The field definition backing a [`ProjectCustomField`].
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FieldRef {
    pub name: Option<String>,
    pub field_type: Option<FieldType>,
}

/// One valid value in a custom field's bundle (e.g. one project state).
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BundleValue {
    pub name: Option<String>,
    pub full_name: Option<String>,
    pub login: Option<String>,
    pub archived: bool,
}

/// The set of valid values configured for a [`ProjectCustomField`].
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Bundle {
    pub values: Vec<BundleValue>,
}

fn default_can_be_empty() -> bool {
    true
}

/// A custom field as configured on a specific project, as returned by
/// `GET /admin/projects/{id}/customFields`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectCustomField {
    #[serde(default = "default_can_be_empty")]
    pub can_be_empty: bool,
    pub field: Option<FieldRef>,
    pub bundle: Option<Bundle>,
}
