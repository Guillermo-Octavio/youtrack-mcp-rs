//! Issue-activity-domain REST calls: the field-change audit trail on an
//! issue.

use crate::client::page::Page;
use crate::client::YouTrackClient;
use crate::error::YouTrackError;
use crate::youtrack::activity::ActivityItem;

const ACTIVITY_FIELDS: &str = "timestamp,author(login,fullName),field(presentation),\
     added(name,fullName,login,idReadable,presentation,text),\
     removed(name,fullName,login,idReadable,presentation,text)";

/// The activity categories requested from `GET /issues/{id}/activities`,
/// matching the original tool: custom-field changes, issue links, and
/// comments.
const ACTIVITY_CATEGORIES: &str = "CustomFieldCategory,LinksCategory,CommentsCategory";

impl YouTrackClient {
    /// Gets the field-change activity log for an issue
    /// (`GET /issues/{issue_id}/activities`), newest first, paginated
    /// via `limit`/`skip`.
    pub async fn get_issue_activity(
        &self,
        issue_id: &str,
        limit: usize,
        skip: usize,
    ) -> Result<Page<ActivityItem>, YouTrackError> {
        let path = format!("issues/{issue_id}/activities");
        let extra = vec![
            ("categories", ACTIVITY_CATEGORIES.to_string()),
            ("reverse", "true".to_string()),
        ];
        self.get_page(&path, ACTIVITY_FIELDS, extra, limit, skip)
            .await
    }
}

#[cfg(test)]
mod tests {
    use url::Url;
    use wiremock::matchers::{method, path, query_param};
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

    fn activity_json() -> serde_json::Value {
        serde_json::json!({
            "timestamp": 1_700_000_000_000i64,
            "author": { "login": "jdoe", "fullName": "Jane Doe" },
            "field": { "presentation": "State" },
            "removed": { "name": "Open" },
            "added": { "name": "Fixed" },
        })
    }

    #[tokio::test]
    async fn get_issue_activity_deserializes_a_page_of_activity_items() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/issues/DEMO-1/activities"))
            .and(query_param(
                "categories",
                "CustomFieldCategory,LinksCategory,CommentsCategory",
            ))
            .and(query_param("reverse", "true"))
            .respond_with(ResponseTemplate::new(200).set_body_json(vec![activity_json()]))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let page = client.get_issue_activity("DEMO-1", 50, 0).await.unwrap();

        assert_eq!(page.items.len(), 1);
        assert_eq!(
            page.items[0]
                .field
                .as_ref()
                .unwrap()
                .presentation
                .as_deref(),
            Some("State")
        );
        assert_eq!(page.items[0].added["name"], "Fixed");
        assert_eq!(page.items[0].removed["name"], "Open");
        assert!(!page.has_more);
    }

    #[tokio::test]
    async fn get_issue_activity_batch_of_limit_plus_one_has_more_pages() {
        let server = MockServer::start().await;
        let batch: Vec<_> = (1..=6).map(|_| activity_json()).collect();
        Mock::given(method("GET"))
            .and(path("/api/issues/DEMO-1/activities"))
            .respond_with(ResponseTemplate::new(200).set_body_json(batch))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let page = client.get_issue_activity("DEMO-1", 5, 0).await.unwrap();

        assert_eq!(page.items.len(), 5);
        assert!(page.has_more);
    }

    #[tokio::test]
    async fn get_issue_activity_maps_404_to_a_generic_api_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/issues/DEMO-404/activities"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let error = client
            .get_issue_activity("DEMO-404", 50, 0)
            .await
            .unwrap_err();

        assert!(matches!(error, YouTrackError::Api { status: 404, .. }));
    }
}
