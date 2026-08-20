//! User-domain REST calls: the current user, user search, and an
//! issue's watchers.

use crate::client::page::Page;
use crate::client::YouTrackClient;
use crate::error::YouTrackError;
use crate::youtrack::issue::UserRef;
use crate::youtrack::user::{IssueWatchersResponse, User};

const USER_FIELDS: &str = "login,fullName,email";

/// `fields` projection for [`YouTrackClient::list_watchers`], asked for
/// on `GET /issues/{id}` since YouTrack has no dedicated watchers
/// endpoint.
const WATCHERS_FIELDS: &str = "watchers(issueWatchers(user(login,fullName)))";

impl YouTrackClient {
    /// Fetches the currently authenticated user (`GET /users/me`).
    pub async fn get_current_user(&self) -> Result<User, YouTrackError> {
        let query = [("fields", USER_FIELDS.to_string())];
        self.get_json("users/me", &query, None).await
    }

    /// Searches users by name, login, or email (`GET /users?query=...`),
    /// paginated via `limit`/`skip`.
    pub async fn search_users(
        &self,
        query: &str,
        limit: usize,
        skip: usize,
    ) -> Result<Page<User>, YouTrackError> {
        let extra = vec![("query", query.to_string())];
        self.get_page("users", USER_FIELDS, extra, limit, skip)
            .await
    }

    /// Lists the users watching an issue. YouTrack has no dedicated
    /// watchers endpoint, so this reads `GET /issues/{issue_id}`
    /// projected down to `watchers(issueWatchers(user(...)))` and
    /// flattens the nested shape into a plain list of users.
    pub async fn list_watchers(&self, issue_id: &str) -> Result<Vec<UserRef>, YouTrackError> {
        let path = format!("issues/{issue_id}");
        let query = [("fields", WATCHERS_FIELDS.to_string())];
        let response: IssueWatchersResponse = self
            .get_json(&path, &query, Some(("issue", issue_id)))
            .await?;

        Ok(response
            .watchers
            .unwrap_or_default()
            .issue_watchers
            .into_iter()
            .filter_map(|watcher| watcher.user)
            .collect())
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

    #[tokio::test]
    async fn get_current_user_deserializes_the_authenticated_user() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/users/me"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "login": "jdoe", "fullName": "Jane Doe", "email": "jane@example.com"
            })))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let user = client.get_current_user().await.unwrap();

        assert_eq!(user.login.as_deref(), Some("jdoe"));
        assert_eq!(user.email.as_deref(), Some("jane@example.com"));
    }

    #[tokio::test]
    async fn get_current_user_maps_401_to_unauthorized() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/users/me"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let error = client.get_current_user().await.unwrap_err();

        assert!(matches!(error, YouTrackError::Unauthorized));
    }

    #[tokio::test]
    async fn search_users_sends_the_query_param_and_returns_a_page() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/users"))
            .and(query_param("query", "john"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "login": "john.doe", "fullName": "John Doe" }
            ])))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let page = client.search_users("john", 50, 0).await.unwrap();

        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].login.as_deref(), Some("john.doe"));
        assert!(!page.has_more);
    }

    #[tokio::test]
    async fn search_users_batch_of_limit_plus_one_has_more_pages() {
        let server = MockServer::start().await;
        let batch: Vec<_> = (1..=6)
            .map(|n| serde_json::json!({ "login": format!("user{n}") }))
            .collect();
        Mock::given(method("GET"))
            .and(path("/api/users"))
            .respond_with(ResponseTemplate::new(200).set_body_json(batch))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let page = client.search_users("user", 5, 0).await.unwrap();

        assert_eq!(page.items.len(), 5);
        assert!(page.has_more);
    }

    #[tokio::test]
    async fn list_watchers_extracts_users_from_the_nested_watchers_projection() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/issues/DEMO-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "watchers": { "issueWatchers": [
                    { "user": { "login": "jdoe", "fullName": "Jane Doe" } },
                    { "user": { "login": "asmith" } },
                ] }
            })))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let watchers = client.list_watchers("DEMO-1").await.unwrap();

        assert_eq!(watchers.len(), 2);
        assert_eq!(watchers[0].full_name.as_deref(), Some("Jane Doe"));
        assert_eq!(watchers[1].login.as_deref(), Some("asmith"));
    }

    #[tokio::test]
    async fn list_watchers_returns_an_empty_vec_when_no_watchers_are_present() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/issues/DEMO-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let watchers = client.list_watchers("DEMO-1").await.unwrap();

        assert!(watchers.is_empty());
    }

    #[tokio::test]
    async fn list_watchers_maps_404_to_not_found_with_the_issue_id() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/issues/DEMO-404"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let error = client.list_watchers("DEMO-404").await.unwrap_err();

        match error {
            YouTrackError::NotFound { resource, id } => {
                assert_eq!(resource, "issue");
                assert_eq!(id, "DEMO-404");
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }
}
