//! Agile-board-domain REST calls: boards, sprints, and a single sprint's
//! detail (with its issues).

use crate::client::page::Page;
use crate::client::YouTrackClient;
use crate::error::YouTrackError;
use crate::youtrack::agile::{AgileBoard, Sprint, SprintDetail};

const BOARD_FIELDS: &str =
    "id,name,projects(shortName,name),currentSprint(id,name),sprints(id,name)";

const SPRINT_FIELDS: &str = "id,name,goal,start,finish,archived";

const SPRINT_DETAIL_FIELDS: &str = "id,name,goal,start,finish,archived,\
     issues(idReadable,summary,resolved,\
     customFields(name,value(name,login,fullName)))";

impl YouTrackClient {
    /// Lists agile boards visible to the current user (`GET /agiles`),
    /// paginated via `limit`/`skip`.
    pub async fn list_agile_boards(
        &self,
        limit: usize,
        skip: usize,
    ) -> Result<Page<AgileBoard>, YouTrackError> {
        self.get_page("agiles", BOARD_FIELDS, vec![], limit, skip)
            .await
    }

    /// Lists the sprints on an agile board
    /// (`GET /agiles/{board_id}/sprints`), paginated via `limit`/`skip`.
    pub async fn list_sprints(
        &self,
        board_id: &str,
        limit: usize,
        skip: usize,
    ) -> Result<Page<Sprint>, YouTrackError> {
        let path = format!("agiles/{board_id}/sprints");
        self.get_page(&path, SPRINT_FIELDS, vec![], limit, skip)
            .await
    }

    /// Fetches a single sprint with its issues
    /// (`GET /agiles/{board_id}/sprints/{sprint_id}`).
    pub async fn get_sprint(
        &self,
        board_id: &str,
        sprint_id: &str,
    ) -> Result<SprintDetail, YouTrackError> {
        let path = format!("agiles/{board_id}/sprints/{sprint_id}");
        let query = [("fields", SPRINT_DETAIL_FIELDS.to_string())];
        self.get_json(&path, &query, Some(("sprint", sprint_id)))
            .await
    }

    /// Adds an existing issue to a sprint
    /// (`POST /agiles/{board_id}/sprints/{sprint_id}/issues`). This
    /// calls the REST API directly rather than going through
    /// [`YouTrackClient::apply_command`]'s command-language parser,
    /// which chokes on board names containing `|` or `:`.
    pub async fn add_issue_to_sprint(
        &self,
        board_id: &str,
        sprint_id: &str,
        issue_id: &str,
    ) -> Result<(), YouTrackError> {
        let path = format!("agiles/{board_id}/sprints/{sprint_id}/issues");
        let body = serde_json::json!({ "id": issue_id });
        self.post_status_only(&path, &body, Some(("sprint", sprint_id)))
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

    fn board_json(id: &str) -> serde_json::Value {
        serde_json::json!({ "id": id, "name": format!("Board {id}") })
    }

    fn sprint_json(id: &str) -> serde_json::Value {
        serde_json::json!({ "id": id, "name": format!("Sprint {id}") })
    }

    #[tokio::test]
    async fn list_agile_boards_batch_smaller_than_limit_has_no_more_pages() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/agiles"))
            .respond_with(ResponseTemplate::new(200).set_body_json(vec![board_json("1")]))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let page = client.list_agile_boards(5, 0).await.unwrap();

        assert_eq!(page.items.len(), 1);
        assert!(!page.has_more);
    }

    #[tokio::test]
    async fn list_agile_boards_batch_of_limit_plus_one_has_more_pages() {
        let server = MockServer::start().await;
        let batch: Vec<_> = (1..=6).map(|n| board_json(&n.to_string())).collect();
        Mock::given(method("GET"))
            .and(path("/api/agiles"))
            .respond_with(ResponseTemplate::new(200).set_body_json(batch))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let page = client.list_agile_boards(5, 0).await.unwrap();

        assert_eq!(page.items.len(), 5);
        assert!(page.has_more);
    }

    #[tokio::test]
    async fn list_sprints_batch_exactly_at_limit_has_no_more_pages() {
        let server = MockServer::start().await;
        let batch: Vec<_> = (1..=3).map(|n| sprint_json(&n.to_string())).collect();
        Mock::given(method("GET"))
            .and(path("/api/agiles/board-1/sprints"))
            .respond_with(ResponseTemplate::new(200).set_body_json(batch))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let page = client.list_sprints("board-1", 3, 0).await.unwrap();

        assert_eq!(page.items.len(), 3);
        assert!(!page.has_more);
    }

    #[tokio::test]
    async fn get_sprint_deserializes_a_sprint_with_its_issues() {
        let server = MockServer::start().await;
        let body = serde_json::json!({
            "id": "sprint-1",
            "name": "Sprint 1",
            "issues": [{ "idReadable": "DEMO-1", "summary": "Fix the thing" }],
        });
        Mock::given(method("GET"))
            .and(path("/api/agiles/board-1/sprints/sprint-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let sprint = client.get_sprint("board-1", "sprint-1").await.unwrap();

        assert_eq!(sprint.issues.len(), 1);
        assert_eq!(sprint.issues[0].id_readable.as_deref(), Some("DEMO-1"));
    }

    #[tokio::test]
    async fn get_sprint_maps_404_to_not_found_with_the_sprint_id() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/agiles/board-1/sprints/missing"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let error = client.get_sprint("board-1", "missing").await.unwrap_err();

        match error {
            YouTrackError::NotFound { resource, id } => {
                assert_eq!(resource, "sprint");
                assert_eq!(id, "missing");
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn add_issue_to_sprint_posts_the_issue_id_to_the_sprint_issues_endpoint() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/agiles/board-1/sprints/sprint-1/issues"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        client
            .add_issue_to_sprint("board-1", "sprint-1", "ISSUE-1")
            .await
            .unwrap();

        let requests = server.received_requests().await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
        assert_eq!(body, serde_json::json!({ "id": "ISSUE-1" }));
    }

    #[tokio::test]
    async fn add_issue_to_sprint_maps_404_to_not_found_with_the_sprint_id() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/agiles/board-1/sprints/missing/issues"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let error = client
            .add_issue_to_sprint("board-1", "missing", "ISSUE-1")
            .await
            .unwrap_err();

        match error {
            YouTrackError::NotFound { resource, id } => {
                assert_eq!(resource, "sprint");
                assert_eq!(id, "missing");
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }
}
