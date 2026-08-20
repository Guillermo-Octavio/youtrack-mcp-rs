//! Miscellaneous project-metadata-domain REST calls: tags and saved
//! searches.

use crate::client::page::Page;
use crate::client::YouTrackClient;
use crate::error::YouTrackError;
use crate::youtrack::metadata::{SavedSearch, Tag};

const TAG_FIELDS: &str = "id,name";
const SAVED_SEARCH_FIELDS: &str = "id,name,query,owner(login,fullName)";

impl YouTrackClient {
    /// Lists issue tags visible to the current user (`GET /tags`),
    /// paginated via `limit`/`skip`.
    pub async fn list_tags(&self, limit: usize, skip: usize) -> Result<Page<Tag>, YouTrackError> {
        self.get_page("tags", TAG_FIELDS, vec![], limit, skip).await
    }

    /// Lists saved searches visible to the current user
    /// (`GET /savedQueries`), paginated via `limit`/`skip`.
    pub async fn list_saved_searches(
        &self,
        limit: usize,
        skip: usize,
    ) -> Result<Page<SavedSearch>, YouTrackError> {
        self.get_page("savedQueries", SAVED_SEARCH_FIELDS, vec![], limit, skip)
            .await
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

    #[tokio::test]
    async fn list_tags_deserializes_a_page_of_tags() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/tags"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "id": "1", "name": "urgent" }
            ])))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let page = client.list_tags(50, 0).await.unwrap();

        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].name.as_deref(), Some("urgent"));
        assert!(!page.has_more);
    }

    #[tokio::test]
    async fn list_tags_batch_of_limit_plus_one_has_more_pages() {
        let server = MockServer::start().await;
        let batch: Vec<_> = (1..=6)
            .map(|n| serde_json::json!({ "id": n.to_string(), "name": format!("tag{n}") }))
            .collect();
        Mock::given(method("GET"))
            .and(path("/api/tags"))
            .respond_with(ResponseTemplate::new(200).set_body_json(batch))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let page = client.list_tags(5, 0).await.unwrap();

        assert_eq!(page.items.len(), 5);
        assert!(page.has_more);
    }

    #[tokio::test]
    async fn list_tags_maps_403_to_forbidden() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/tags"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let error = client.list_tags(50, 0).await.unwrap_err();

        assert!(matches!(error, YouTrackError::Forbidden));
    }

    #[tokio::test]
    async fn list_saved_searches_deserializes_a_page_with_owner_and_query() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/savedQueries"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {
                    "id": "1",
                    "name": "My bugs",
                    "query": "assignee: me #Bug",
                    "owner": { "login": "jdoe", "fullName": "Jane Doe" },
                }
            ])))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let page = client.list_saved_searches(50, 0).await.unwrap();

        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].name.as_deref(), Some("My bugs"));
        assert_eq!(page.items[0].query.as_deref(), Some("assignee: me #Bug"));
        assert_eq!(
            page.items[0].owner.as_ref().unwrap().full_name.as_deref(),
            Some("Jane Doe")
        );
        assert!(!page.has_more);
    }

    #[tokio::test]
    async fn list_saved_searches_maps_401_to_unauthorized() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/savedQueries"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let error = client.list_saved_searches(50, 0).await.unwrap_err();

        assert!(matches!(error, YouTrackError::Unauthorized));
    }
}
