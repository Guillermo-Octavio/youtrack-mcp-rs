//! Project-metadata-domain REST calls: list projects and their
//! configured custom fields.

use crate::client::page::Page;
use crate::client::YouTrackClient;
use crate::error::YouTrackError;
use crate::youtrack::project::{Project, ProjectCustomField};

const PROJECT_FIELDS: &str = "id,name,shortName";

const PROJECT_CUSTOM_FIELD_FIELDS: &str = "canBeEmpty,field(name,fieldType(valueType)),\
     bundle(values(name,fullName,login,archived))";

impl YouTrackClient {
    /// Lists projects visible to the current user
    /// (`GET /admin/projects`), paginated via `limit`/`skip`.
    pub async fn list_projects(
        &self,
        limit: usize,
        skip: usize,
    ) -> Result<Page<Project>, YouTrackError> {
        self.get_page("admin/projects", PROJECT_FIELDS, vec![], limit, skip)
            .await
    }

    /// Lists the custom fields configured on a project
    /// (`GET /admin/projects/{project_id}/customFields`), paginated via
    /// `limit`/`skip`.
    pub async fn list_project_fields(
        &self,
        project_id: &str,
        limit: usize,
        skip: usize,
    ) -> Result<Page<ProjectCustomField>, YouTrackError> {
        let path = format!("admin/projects/{project_id}/customFields");
        self.get_page(&path, PROJECT_CUSTOM_FIELD_FIELDS, vec![], limit, skip)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use url::Url;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn test_client(base_url: &str) -> YouTrackClient {
        let config = Config {
            youtrack_url: Url::parse(base_url).unwrap(),
            youtrack_token: "test-token".to_string(),
        };
        YouTrackClient::new(&config).unwrap()
    }

    fn project_json(short_name: &str) -> serde_json::Value {
        serde_json::json!({ "id": "0-1", "name": "Demo", "shortName": short_name })
    }

    #[tokio::test]
    async fn list_projects_batch_smaller_than_limit_has_no_more_pages() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/admin/projects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(vec![project_json("DEMO")]))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let page = client.list_projects(5, 0).await.unwrap();

        assert_eq!(page.items.len(), 1);
        assert!(!page.has_more);
    }

    #[tokio::test]
    async fn list_projects_batch_of_limit_plus_one_has_more_pages() {
        let server = MockServer::start().await;
        let batch: Vec<_> = (1..=6).map(|n| project_json(&format!("DEMO{n}"))).collect();
        Mock::given(method("GET"))
            .and(path("/api/admin/projects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(batch))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let page = client.list_projects(5, 0).await.unwrap();

        assert_eq!(page.items.len(), 5);
        assert!(page.has_more);
    }

    #[tokio::test]
    async fn list_project_fields_deserializes_bundle_values() {
        let server = MockServer::start().await;
        let body = serde_json::json!([{
            "canBeEmpty": false,
            "field": { "name": "State", "fieldType": { "valueType": "state[1]" } },
            "bundle": { "values": [
                { "name": "Open", "archived": false },
                { "name": "Old", "archived": true },
            ] },
        }]);
        Mock::given(method("GET"))
            .and(path("/api/admin/projects/0-1/customFields"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let page = client.list_project_fields("0-1", 50, 0).await.unwrap();

        assert_eq!(page.items.len(), 1);
        assert!(!page.items[0].can_be_empty);
        assert_eq!(page.items[0].bundle.as_ref().unwrap().values.len(), 2);
    }

    #[tokio::test]
    async fn list_project_fields_maps_403_to_forbidden() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/admin/projects/0-1/customFields"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let error = client.list_project_fields("0-1", 50, 0).await.unwrap_err();

        assert!(matches!(error, YouTrackError::Forbidden));
    }
}
