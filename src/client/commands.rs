//! Workflow-command REST calls: applying a YouTrack command-language
//! action (e.g. `State Fixed`, `Assignee john.doe`) to one or more issues.

use crate::client::YouTrackClient;
use crate::error::YouTrackError;

impl YouTrackClient {
    /// Applies `command` to every issue in `issue_ids` in one request
    /// (`POST /commands`). YouTrack's command endpoint takes an `issues`
    /// array either way, so this one method backs both the
    /// `apply_command` (single issue) and `bulk_command` (multiple
    /// issues) MCP tools — a single issue is just a one-element slice.
    pub async fn apply_command(
        &self,
        issue_ids: &[String],
        command: &str,
    ) -> Result<(), YouTrackError> {
        let body = serde_json::json!({
            "query": command,
            "issues": issue_ids
                .iter()
                .map(|id| serde_json::json!({ "idReadable": id }))
                .collect::<Vec<_>>(),
        });

        self.post_status_only("commands", &body).await
    }
}

#[cfg(test)]
mod tests {
    use url::Url;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::config::Config;

    fn test_client(base_url: &str) -> YouTrackClient {
        let config = Config {
            youtrack_url: Url::parse(base_url).unwrap(),
            youtrack_token: "test-token".to_string(),
        };
        YouTrackClient::new(&config).unwrap()
    }

    #[tokio::test]
    async fn apply_command_sends_the_query_and_a_single_issue_reference() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/commands"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        client
            .apply_command(&["DEMO-1".to_string()], "State Fixed")
            .await
            .unwrap();

        let requests = server.received_requests().await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
        assert_eq!(body["query"], "State Fixed");
        assert_eq!(
            body["issues"],
            serde_json::json!([{ "idReadable": "DEMO-1" }])
        );
    }

    #[tokio::test]
    async fn apply_command_sends_every_issue_in_a_single_request_for_bulk_ids() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/commands"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        client
            .apply_command(
                &[
                    "DEMO-1".to_string(),
                    "DEMO-2".to_string(),
                    "DEMO-3".to_string(),
                ],
                "State Fixed",
            )
            .await
            .unwrap();

        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
        assert_eq!(
            body["issues"],
            serde_json::json!([
                { "idReadable": "DEMO-1" },
                { "idReadable": "DEMO-2" },
                { "idReadable": "DEMO-3" },
            ])
        );
    }

    #[tokio::test]
    async fn apply_command_maps_401_to_unauthorized() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/commands"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let error = client
            .apply_command(&["DEMO-1".to_string()], "State Fixed")
            .await
            .unwrap_err();

        assert!(matches!(error, YouTrackError::Unauthorized));
    }
}
