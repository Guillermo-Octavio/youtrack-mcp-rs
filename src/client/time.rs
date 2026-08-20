//! Time-tracking-domain REST calls: an issue's logged work items.

use crate::client::page::Page;
use crate::client::YouTrackClient;
use crate::error::YouTrackError;
use crate::youtrack::work_item::WorkItem;

const WORK_ITEM_FIELDS: &str =
    "id,date,duration(minutes,presentation),text,type(name),author(login,fullName)";

impl YouTrackClient {
    /// Lists the time-tracking work items logged on an issue
    /// (`GET /issues/{issue_id}/timeTracking/workItems`), paginated via
    /// `limit`/`skip`.
    pub async fn list_work_items(
        &self,
        issue_id: &str,
        limit: usize,
        skip: usize,
    ) -> Result<Page<WorkItem>, YouTrackError> {
        let path = format!("issues/{issue_id}/timeTracking/workItems");
        self.get_page(&path, WORK_ITEM_FIELDS, vec![], limit, skip)
            .await
    }

    /// Logs a work item on an issue
    /// (`POST /issues/{issue_id}/timeTracking/workItems`), returning the
    /// newly created work item. `text` is omitted from the request body
    /// entirely when absent, matching the original tool's behavior.
    pub async fn add_work_item(
        &self,
        issue_id: &str,
        duration_minutes: u32,
        text: Option<&str>,
    ) -> Result<WorkItem, YouTrackError> {
        let path = format!("issues/{issue_id}/timeTracking/workItems");
        let mut body = serde_json::json!({ "duration": { "minutes": duration_minutes } });
        if let Some(text) = text.filter(|text| !text.is_empty()) {
            body["text"] = serde_json::Value::String(text.to_string());
        }
        let query = [("fields", WORK_ITEM_FIELDS.to_string())];
        self.post_json(&path, &query, &body, Some(("issue", issue_id)))
            .await
    }

    /// Permanently deletes a work item from an issue
    /// (`DELETE /issues/{issue_id}/timeTracking/workItems/{work_item_id}`).
    pub async fn delete_work_item(
        &self,
        issue_id: &str,
        work_item_id: &str,
    ) -> Result<(), YouTrackError> {
        let path = format!("issues/{issue_id}/timeTracking/workItems/{work_item_id}");
        self.delete(&path, Some(("work item", work_item_id))).await
    }
}

#[cfg(test)]
mod tests {
    use url::Url;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::client::YouTrackClient;
    use crate::config::Config;
    use crate::error::YouTrackError;

    fn test_client(base_url: &str) -> YouTrackClient {
        let config = Config {
            youtrack_url: Url::parse(base_url).unwrap(),
            youtrack_token: "test-token".to_string(),
        };
        YouTrackClient::new(&config).unwrap()
    }

    fn work_item_json(id: &str, minutes: i64) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "date": 1_700_000_000_000i64,
            "duration": { "minutes": minutes, "presentation": format!("{minutes}m") },
            "text": "Worked on it",
            "type": { "name": "Development" },
            "author": { "login": "jdoe", "fullName": "Jane Doe" },
        })
    }

    #[tokio::test]
    async fn list_work_items_deserializes_a_page_of_work_items() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/issues/DEMO-1/timeTracking/workItems"))
            .respond_with(ResponseTemplate::new(200).set_body_json(vec![work_item_json("1", 90)]))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let page = client.list_work_items("DEMO-1", 50, 0).await.unwrap();

        assert_eq!(page.items.len(), 1);
        assert_eq!(
            page.items[0]
                .duration
                .as_ref()
                .unwrap()
                .presentation
                .as_deref(),
            Some("90m")
        );
        assert_eq!(
            page.items[0].kind.as_ref().unwrap().name.as_deref(),
            Some("Development")
        );
        assert!(!page.has_more);
    }

    #[tokio::test]
    async fn list_work_items_maps_404_to_a_generic_api_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/issues/DEMO-404/timeTracking/workItems"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let error = client.list_work_items("DEMO-404", 50, 0).await.unwrap_err();

        assert!(matches!(error, YouTrackError::Api { status: 404, .. }));
    }

    #[tokio::test]
    async fn add_work_item_sends_duration_and_text_and_returns_the_created_item() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/issues/DEMO-1/timeTracking/workItems"))
            .respond_with(ResponseTemplate::new(200).set_body_json(work_item_json("2", 30)))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let item = client
            .add_work_item("DEMO-1", 30, Some("Worked on it"))
            .await
            .unwrap();

        assert_eq!(item.id.as_deref(), Some("2"));
        let requests = server.received_requests().await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
        assert_eq!(body["duration"]["minutes"], 30);
        assert_eq!(body["text"], "Worked on it");
    }

    #[tokio::test]
    async fn add_work_item_omits_text_when_none() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/issues/DEMO-1/timeTracking/workItems"))
            .respond_with(ResponseTemplate::new(200).set_body_json(work_item_json("3", 15)))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        client.add_work_item("DEMO-1", 15, None).await.unwrap();

        let requests = server.received_requests().await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
        assert!(body.get("text").is_none());
    }

    #[tokio::test]
    async fn add_work_item_maps_404_to_not_found_with_the_issue_id() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/issues/DEMO-404/timeTracking/workItems"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let error = client
            .add_work_item("DEMO-404", 15, None)
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
    async fn delete_work_item_succeeds_on_a_2xx_response() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/api/issues/DEMO-1/timeTracking/workItems/wi-1"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        client.delete_work_item("DEMO-1", "wi-1").await.unwrap();
    }

    #[tokio::test]
    async fn delete_work_item_maps_404_to_not_found_with_the_work_item_id() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/api/issues/DEMO-1/timeTracking/workItems/missing"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let error = client
            .delete_work_item("DEMO-1", "missing")
            .await
            .unwrap_err();

        match error {
            YouTrackError::NotFound { resource, id } => {
                assert_eq!(resource, "work item");
                assert_eq!(id, "missing");
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }
}
