//! Issue-domain REST calls: search, get, create, update, delete issues,
//! and list an issue's links.

use std::collections::HashMap;

use serde::de::Error as _;

use crate::client::page::Page;
use crate::client::YouTrackClient;
use crate::error::YouTrackError;
use crate::youtrack::issue::{Issue, IssueLink};

/// Default `fields` projection for [`YouTrackClient::search_issues`] and
/// [`YouTrackClient::get_issue`] when the caller does not supply its own.
pub const ISSUE_FIELDS: &str = "idReadable,summary,description,\
     project(shortName,name),\
     reporter(login,fullName),\
     customFields(name,value(name,login,fullName,text,presentation,minutes))";

/// `fields` projection for [`YouTrackClient::list_links`].
const LINK_FIELDS: &str = "direction,linkType(name,sourceToTarget,targetToSource),\
     issues(idReadable,summary)";

/// `fields` projection requested from `POST /issues` and
/// `POST /issues/{id}` — deliberately small since callers only need to
/// confirm the mutated issue's identity, not its full detail.
const MUTATION_RESPONSE_FIELDS: &str = "idReadable,summary";

/// Upper bound on the number of a project's configured custom fields
/// fetched when validating a create/update request's `fields` map.
/// Mirrors the `$top` used elsewhere for admin-scoped listings; YouTrack
/// projects very rarely configure more custom fields than this.
const PROJECT_CUSTOM_FIELDS_LIMIT: usize = 500;

impl YouTrackClient {
    /// Searches issues with a YouTrack search query
    /// (`GET /issues?query=...`), paginated via `limit`/`skip`.
    pub async fn search_issues(
        &self,
        query: &str,
        fields: &str,
        limit: usize,
        skip: usize,
    ) -> Result<Page<Issue>, YouTrackError> {
        let extra = vec![("query", query.to_string())];
        self.get_page("issues", fields, extra, limit, skip).await
    }

    /// Fetches a single issue by id or readable id
    /// (`GET /issues/{issue_id}`).
    pub async fn get_issue(&self, issue_id: &str, fields: &str) -> Result<Issue, YouTrackError> {
        let path = format!("issues/{issue_id}");
        let query = [("fields", fields.to_string())];
        self.get_json(&path, &query, Some(("issue", issue_id)))
            .await
    }

    /// Lists an issue's links to other issues
    /// (`GET /issues/{issue_id}/links`). YouTrack does not paginate this
    /// endpoint, so every link is returned in one call.
    pub async fn list_links(&self, issue_id: &str) -> Result<Vec<IssueLink>, YouTrackError> {
        let path = format!("issues/{issue_id}/links");
        let query = [("fields", LINK_FIELDS.to_string())];
        self.get_json(&path, &query, Some(("issue", issue_id)))
            .await
    }

    /// Creates a new issue in `project_id` (`POST /issues`). `fields`
    /// carries open-ended custom-field values, validated and encoded
    /// against `project_id`'s configured custom fields (via
    /// [`YouTrackClient::list_project_fields`]) before the request is
    /// sent; an unrecognized field name fails with
    /// [`YouTrackError::UnknownCustomFields`] naming it, and no create
    /// request is sent.
    pub async fn create_issue(
        &self,
        project_id: &str,
        summary: &str,
        description: Option<&str>,
        fields: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<Issue, YouTrackError> {
        let custom_fields = self.encode_custom_fields(project_id, fields).await?;

        let mut body = serde_json::json!({
            "project": { "id": project_id },
            "summary": summary,
        });
        if let Some(description) = description {
            body["description"] = serde_json::Value::String(description.to_string());
        }
        if !custom_fields.is_empty() {
            body["customFields"] = serde_json::Value::Array(custom_fields);
        }

        let query = [("fields", MUTATION_RESPONSE_FIELDS.to_string())];
        self.post_json("issues", &query, &body, None).await
    }

    /// Updates `summary`, `description`, and/or custom fields on an
    /// existing issue (`POST /issues/{issue_id}`). Only the values the
    /// caller passes are sent. A `fields` map is validated and encoded
    /// against the issue's own project, discovered with one extra
    /// `GET /issues/{issue_id}` lookup when needed; an unrecognized field
    /// name fails with [`YouTrackError::UnknownCustomFields`] and no
    /// update request is sent.
    pub async fn update_issue(
        &self,
        issue_id: &str,
        summary: Option<&str>,
        description: Option<&str>,
        fields: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<Issue, YouTrackError> {
        let mut body = serde_json::json!({});
        if let Some(summary) = summary {
            body["summary"] = serde_json::Value::String(summary.to_string());
        }
        if let Some(description) = description {
            body["description"] = serde_json::Value::String(description.to_string());
        }

        if let Some(fields) = fields.filter(|f| !f.is_empty()) {
            let project_id = self.issue_project_id(issue_id).await?;
            let custom_fields = self.encode_custom_fields(&project_id, Some(fields)).await?;
            body["customFields"] = serde_json::Value::Array(custom_fields);
        }

        let path = format!("issues/{issue_id}");
        let query = [("fields", MUTATION_RESPONSE_FIELDS.to_string())];
        self.post_json(&path, &query, &body, Some(("issue", issue_id)))
            .await
    }

    /// Permanently deletes an issue (`DELETE /issues/{issue_id}`). This
    /// is irreversible: YouTrack has no trash for issues, so the issue
    /// and its full history are lost with no way to recover them.
    pub async fn delete_issue(&self, issue_id: &str) -> Result<(), YouTrackError> {
        let path = format!("issues/{issue_id}");
        self.delete(&path, Some(("issue", issue_id))).await
    }

    /// Looks up the internal project id for `issue_id`
    /// (`GET /issues/{issue_id}?fields=project(id)`), used by
    /// [`YouTrackClient::update_issue`] to validate custom fields when
    /// the caller didn't already name the project.
    async fn issue_project_id(&self, issue_id: &str) -> Result<String, YouTrackError> {
        #[derive(serde::Deserialize)]
        struct ProjectIdOnly {
            #[serde(default)]
            project: Option<IdOnly>,
        }
        #[derive(serde::Deserialize)]
        struct IdOnly {
            #[serde(default)]
            id: Option<String>,
        }

        let path = format!("issues/{issue_id}");
        let query = [("fields", "project(id)".to_string())];
        let issue: ProjectIdOnly = self
            .get_json(&path, &query, Some(("issue", issue_id)))
            .await?;

        issue
            .project
            .and_then(|p| p.id)
            .ok_or_else(|| YouTrackError::Decode {
                endpoint: path.clone(),
                source: serde_json::Error::custom("issue response is missing its project id"),
            })
    }

    /// Validates `fields`' names against `project_id`'s configured custom
    /// fields (`GET /admin/projects/{project_id}/customFields`) and
    /// encodes each valid entry into the `customFields` array shape
    /// YouTrack's REST API expects on issue create/update. Returns
    /// [`YouTrackError::UnknownCustomFields`] naming every field name
    /// that isn't configured on the project, alongside the valid set.
    /// Does nothing (and makes no request) when `fields` is empty.
    async fn encode_custom_fields(
        &self,
        project_id: &str,
        fields: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<serde_json::Value>, YouTrackError> {
        let Some(fields) = fields.filter(|f| !f.is_empty()) else {
            return Ok(Vec::new());
        };

        let schema = self
            .list_project_fields(project_id, PROJECT_CUSTOM_FIELDS_LIMIT, 0)
            .await?;

        let mut unknown = Vec::new();
        let mut encoded = Vec::new();

        for (name, value) in fields {
            let matched = schema.items.iter().find(|pcf| {
                pcf.field.as_ref().and_then(|f| f.name.as_deref()) == Some(name.as_str())
            });

            match matched {
                Some(pcf) => {
                    let value_type = pcf
                        .field
                        .as_ref()
                        .and_then(|f| f.field_type.as_ref())
                        .and_then(|ft| ft.value_type.as_deref());
                    encoded.push(serde_json::json!({
                        "name": name,
                        "$type": custom_field_type(value_type),
                        "value": encode_custom_field_value(value_type, value.clone()),
                    }));
                }
                None => unknown.push(name.clone()),
            }
        }

        if !unknown.is_empty() {
            unknown.sort();
            let mut valid: Vec<String> = schema
                .items
                .iter()
                .filter_map(|pcf| pcf.field.as_ref().and_then(|f| f.name.clone()))
                .collect();
            valid.sort();
            return Err(YouTrackError::UnknownCustomFields { unknown, valid });
        }

        Ok(encoded)
    }
}

/// Best-effort mapping from a YouTrack custom field's
/// `field.fieldType.valueType` (e.g. `enum[1]`, `state[1]`, `user[*]`) to
/// the `$type` YouTrack's REST API expects on the corresponding
/// `customFields` array entry when creating or updating an issue.
/// Unrecognized or absent value types fall back to
/// `SimpleIssueCustomField`, which YouTrack accepts for scalar custom
/// fields (text, integer, float, date, period).
fn custom_field_type(value_type: Option<&str>) -> &'static str {
    match value_type {
        Some("state[1]") | Some("state[*]") => "StateIssueCustomField",
        Some("enum[1]") => "SingleEnumIssueCustomField",
        Some("enum[*]") => "MultiEnumIssueCustomField",
        Some("ownedField[1]") => "SingleOwnedIssueCustomField",
        Some("ownedField[*]") => "MultiOwnedIssueCustomField",
        Some("user[1]") => "SingleUserIssueCustomField",
        Some("user[*]") => "MultiUserIssueCustomField",
        Some("version[1]") => "SingleVersionIssueCustomField",
        Some("version[*]") => "MultiVersionIssueCustomField",
        Some("build[1]") => "SingleBuildIssueCustomField",
        Some("build[*]") => "MultiBuildIssueCustomField",
        Some("group[1]") => "SingleGroupIssueCustomField",
        Some("group[*]") => "MultiGroupIssueCustomField",
        _ => "SimpleIssueCustomField",
    }
}

/// Encodes one custom field's caller-supplied JSON `value` for the
/// `customFields` array entry. Fields backed by a bundle (state, enum,
/// user, owned field, version, build, group) expect a
/// `{"name": "..."}`-shaped reference to the bundle element, so a bare
/// string (or array of strings, for multi-value fields) is wrapped into
/// that shape; scalar fields and already-object-shaped values pass
/// through unchanged.
fn encode_custom_field_value(
    value_type: Option<&str>,
    value: serde_json::Value,
) -> serde_json::Value {
    let is_bundle_field = matches!(value_type, Some(vt) if vt.starts_with("state")
        || vt.starts_with("enum")
        || vt.starts_with("user")
        || vt.starts_with("ownedField")
        || vt.starts_with("version")
        || vt.starts_with("build")
        || vt.starts_with("group"));

    if !is_bundle_field {
        return value;
    }

    match value {
        serde_json::Value::String(name) => serde_json::json!({ "name": name }),
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items
                .into_iter()
                .map(|item| match item {
                    serde_json::Value::String(name) => serde_json::json!({ "name": name }),
                    other => other,
                })
                .collect(),
        ),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::config::Config;
    use url::Url;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn test_client(base_url: &str) -> YouTrackClient {
        let config = Config {
            youtrack_url: Url::parse(base_url).unwrap(),
            youtrack_token: "test-token".to_string(),
        };
        YouTrackClient::new(&config).unwrap()
    }

    fn issue_json(id: &str) -> serde_json::Value {
        serde_json::json!({ "idReadable": id, "summary": format!("Summary {id}") })
    }

    #[tokio::test]
    async fn search_issues_batch_smaller_than_limit_has_no_more_pages() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/issues"))
            .and(query_param("query", "project: DEMO"))
            .and(query_param("$top", "6"))
            .and(query_param("$skip", "0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(vec![issue_json("DEMO-1")]))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let page = client
            .search_issues("project: DEMO", ISSUE_FIELDS, 5, 0)
            .await
            .unwrap();

        assert_eq!(page.items.len(), 1);
        assert!(!page.has_more);
        assert_eq!(page.next_cursor, None);
    }

    #[tokio::test]
    async fn search_issues_batch_exactly_at_limit_has_no_more_pages() {
        let server = MockServer::start().await;
        let batch: Vec<_> = (1..=5).map(|n| issue_json(&format!("DEMO-{n}"))).collect();
        Mock::given(method("GET"))
            .and(path("/api/issues"))
            .respond_with(ResponseTemplate::new(200).set_body_json(batch))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let page = client
            .search_issues("project: DEMO", ISSUE_FIELDS, 5, 0)
            .await
            .unwrap();

        assert_eq!(page.items.len(), 5);
        assert!(!page.has_more);
    }

    #[tokio::test]
    async fn search_issues_batch_of_limit_plus_one_has_more_pages_with_valid_cursor() {
        let server = MockServer::start().await;
        let batch: Vec<_> = (1..=6).map(|n| issue_json(&format!("DEMO-{n}"))).collect();
        Mock::given(method("GET"))
            .and(path("/api/issues"))
            .respond_with(ResponseTemplate::new(200).set_body_json(batch))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let page = client
            .search_issues("project: DEMO", ISSUE_FIELDS, 5, 0)
            .await
            .unwrap();

        assert_eq!(page.items.len(), 5);
        assert!(page.has_more);
        assert!(page.next_cursor.is_some());
    }

    #[tokio::test]
    async fn get_issue_deserializes_a_single_issue() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/issues/DEMO-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(issue_json("DEMO-1")))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let issue = client.get_issue("DEMO-1", ISSUE_FIELDS).await.unwrap();

        assert_eq!(issue.id_readable.as_deref(), Some("DEMO-1"));
    }

    #[tokio::test]
    async fn get_issue_maps_404_to_not_found_with_the_issue_id() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/issues/DEMO-404"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let error = client
            .get_issue("DEMO-404", ISSUE_FIELDS)
            .await
            .unwrap_err();

        match error {
            YouTrackError::NotFound { resource, id } => {
                assert_eq!(resource, "issue");
                assert_eq!(id, "DEMO-404");
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn get_issue_maps_401_to_unauthorized() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/issues/DEMO-1"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let error = client.get_issue("DEMO-1", ISSUE_FIELDS).await.unwrap_err();

        assert!(matches!(error, YouTrackError::Unauthorized));
    }

    #[tokio::test]
    async fn create_issue_sends_project_summary_and_description() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/issues"))
            .respond_with(ResponseTemplate::new(200).set_body_json(issue_json("DEMO-1")))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let issue = client
            .create_issue("0-1", "Fix the thing", Some("Details here"), None)
            .await
            .unwrap();

        assert_eq!(issue.id_readable.as_deref(), Some("DEMO-1"));
        let requests = server.received_requests().await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
        assert_eq!(body["project"]["id"], "0-1");
        assert_eq!(body["summary"], "Fix the thing");
        assert_eq!(body["description"], "Details here");
    }

    #[tokio::test]
    async fn create_issue_with_a_valid_custom_field_encodes_it_in_the_request_body() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/admin/projects/0-1/customFields"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!([{
                    "canBeEmpty": false,
                    "field": { "name": "Priority", "fieldType": { "valueType": "enum[1]" } },
                }])),
            )
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/issues"))
            .respond_with(ResponseTemplate::new(200).set_body_json(issue_json("DEMO-1")))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let mut fields = HashMap::new();
        fields.insert("Priority".to_string(), serde_json::json!("Critical"));

        let issue = client
            .create_issue("0-1", "Fix the thing", None, Some(&fields))
            .await
            .unwrap();

        assert_eq!(issue.id_readable.as_deref(), Some("DEMO-1"));
        let requests = server.received_requests().await.unwrap();
        let post = requests
            .iter()
            .find(|r| r.method == wiremock::http::Method::POST)
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&post.body).unwrap();
        assert_eq!(body["customFields"][0]["name"], "Priority");
        assert_eq!(
            body["customFields"][0]["$type"],
            "SingleEnumIssueCustomField"
        );
        assert_eq!(body["customFields"][0]["value"]["name"], "Critical");
    }

    #[tokio::test]
    async fn create_issue_with_an_unknown_custom_field_fails_without_sending_the_create_request() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/admin/projects/0-1/customFields"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!([{
                    "canBeEmpty": false,
                    "field": { "name": "Priority", "fieldType": { "valueType": "enum[1]" } },
                }])),
            )
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/issues"))
            .respond_with(ResponseTemplate::new(200).set_body_json(issue_json("DEMO-1")))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let mut fields = HashMap::new();
        fields.insert("Bogus".to_string(), serde_json::json!("whatever"));

        let error = client
            .create_issue("0-1", "Fix the thing", None, Some(&fields))
            .await
            .unwrap_err();

        match error {
            YouTrackError::UnknownCustomFields { unknown, valid } => {
                assert_eq!(unknown, vec!["Bogus".to_string()]);
                assert_eq!(valid, vec!["Priority".to_string()]);
            }
            other => panic!("expected UnknownCustomFields, got {other:?}"),
        }
        let requests = server.received_requests().await.unwrap();
        assert!(!requests
            .iter()
            .any(|r| r.method == wiremock::http::Method::POST));
    }

    #[tokio::test]
    async fn update_issue_sends_only_the_provided_fields() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/issues/DEMO-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(issue_json("DEMO-1")))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let issue = client
            .update_issue("DEMO-1", Some("New summary"), None, None)
            .await
            .unwrap();

        assert_eq!(issue.id_readable.as_deref(), Some("DEMO-1"));
        let requests = server.received_requests().await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
        assert_eq!(body["summary"], "New summary");
        assert!(body.get("description").is_none());
        assert!(body.get("customFields").is_none());
    }

    #[tokio::test]
    async fn update_issue_with_a_valid_custom_field_resolves_the_project_and_encodes_it() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/issues/DEMO-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "project": { "id": "0-1" },
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/admin/projects/0-1/customFields"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!([{
                    "canBeEmpty": false,
                    "field": { "name": "State", "fieldType": { "valueType": "state[1]" } },
                }])),
            )
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/issues/DEMO-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(issue_json("DEMO-1")))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let mut fields = HashMap::new();
        fields.insert("State".to_string(), serde_json::json!("Fixed"));

        let issue = client
            .update_issue("DEMO-1", None, None, Some(&fields))
            .await
            .unwrap();

        assert_eq!(issue.id_readable.as_deref(), Some("DEMO-1"));
        let requests = server.received_requests().await.unwrap();
        let post = requests
            .iter()
            .find(|r| r.method == wiremock::http::Method::POST)
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&post.body).unwrap();
        assert_eq!(body["customFields"][0]["name"], "State");
        assert_eq!(body["customFields"][0]["$type"], "StateIssueCustomField");
        assert_eq!(body["customFields"][0]["value"]["name"], "Fixed");
    }

    #[tokio::test]
    async fn update_issue_with_an_unknown_custom_field_fails_without_sending_the_update_request() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/issues/DEMO-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "project": { "id": "0-1" },
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/admin/projects/0-1/customFields"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!([{
                    "canBeEmpty": false,
                    "field": { "name": "State", "fieldType": { "valueType": "state[1]" } },
                }])),
            )
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/issues/DEMO-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(issue_json("DEMO-1")))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let mut fields = HashMap::new();
        fields.insert("Bogus".to_string(), serde_json::json!("whatever"));

        let error = client
            .update_issue("DEMO-1", None, None, Some(&fields))
            .await
            .unwrap_err();

        match error {
            YouTrackError::UnknownCustomFields { unknown, valid } => {
                assert_eq!(unknown, vec!["Bogus".to_string()]);
                assert_eq!(valid, vec!["State".to_string()]);
            }
            other => panic!("expected UnknownCustomFields, got {other:?}"),
        }
        let requests = server.received_requests().await.unwrap();
        assert!(!requests
            .iter()
            .any(|r| r.method == wiremock::http::Method::POST));
    }

    #[tokio::test]
    async fn update_issue_maps_404_to_not_found_with_the_issue_id() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/issues/DEMO-404"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let error = client
            .update_issue("DEMO-404", Some("New summary"), None, None)
            .await
            .unwrap_err();

        match error {
            YouTrackError::NotFound { resource, id } => {
                assert_eq!(resource, "issue");
                assert_eq!(id, "DEMO-404");
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn delete_issue_sends_a_delete_request_to_the_issue_path() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/api/issues/DEMO-1"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        client.delete_issue("DEMO-1").await.unwrap();
    }

    #[tokio::test]
    async fn delete_issue_maps_404_to_not_found_with_the_issue_id() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/api/issues/DEMO-404"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let error = client.delete_issue("DEMO-404").await.unwrap_err();

        match error {
            YouTrackError::NotFound { resource, id } => {
                assert_eq!(resource, "issue");
                assert_eq!(id, "DEMO-404");
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn list_links_deserializes_links_with_issues() {
        let server = MockServer::start().await;
        let body = serde_json::json!([{
            "direction": "OUTWARD",
            "linkType": { "name": "Relates", "sourceToTarget": "relates to" },
            "issues": [{ "idReadable": "DEMO-2", "summary": "Other issue" }],
        }]);
        Mock::given(method("GET"))
            .and(path("/api/issues/DEMO-1/links"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let links = client.list_links("DEMO-1").await.unwrap();

        assert_eq!(links.len(), 1);
        assert_eq!(links[0].issues[0].id_readable.as_deref(), Some("DEMO-2"));
    }
}
