//! Comment-domain REST calls: list an issue's comments and add a new one.

use crate::client::page::Page;
use crate::client::YouTrackClient;
use crate::error::YouTrackError;
use crate::youtrack::comment::Comment;

/// `fields` projection for [`YouTrackClient::list_comments`] and
/// [`YouTrackClient::add_comment`].
const COMMENT_FIELDS: &str = "id,text,created,author(login,fullName)";

impl YouTrackClient {
    /// Lists the comments on an issue
    /// (`GET /issues/{issue_id}/comments`), paginated via `limit`/`skip`.
    pub async fn list_comments(
        &self,
        issue_id: &str,
        limit: usize,
        skip: usize,
    ) -> Result<Page<Comment>, YouTrackError> {
        let path = format!("issues/{issue_id}/comments");
        self.get_page(&path, COMMENT_FIELDS, vec![], limit, skip)
            .await
    }

    /// Adds a comment to an issue (`POST /issues/{issue_id}/comments`),
    /// returning the newly created comment.
    pub async fn add_comment(&self, issue_id: &str, text: &str) -> Result<Comment, YouTrackError> {
        let path = format!("issues/{issue_id}/comments");
        let body = serde_json::json!({ "text": text });
        let query = [("fields", COMMENT_FIELDS.to_string())];
        self.post_json(&path, &query, &body, Some(("issue", issue_id)))
            .await
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

    fn comment_json(id: &str, text: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "text": text,
            "created": 1_700_000_000_000i64,
            "author": { "login": "jdoe", "fullName": "Jane Doe" },
        })
    }

    #[tokio::test]
    async fn list_comments_deserializes_a_page_of_comments() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/issues/DEMO-1/comments"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(vec![comment_json("1", "First comment")]),
            )
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let page = client.list_comments("DEMO-1", 50, 0).await.unwrap();

        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].text.as_deref(), Some("First comment"));
        assert_eq!(
            page.items[0].author.as_ref().unwrap().full_name.as_deref(),
            Some("Jane Doe")
        );
        assert!(!page.has_more);
    }

    #[tokio::test]
    async fn list_comments_maps_404_to_a_generic_api_error() {
        // `get_page` (shared by every paginated list tool) does not thread
        // a `not_found` resource/id through, so a 404 here surfaces the
        // same generic `Api` error as `list_articles`/`search_issues`.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/issues/DEMO-404/comments"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let error = client.list_comments("DEMO-404", 50, 0).await.unwrap_err();

        assert!(matches!(error, YouTrackError::Api { status: 404, .. }));
    }

    #[tokio::test]
    async fn add_comment_sends_the_text_body_and_returns_the_created_comment() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/issues/DEMO-1/comments"))
            .respond_with(ResponseTemplate::new(200).set_body_json(comment_json("2", "Looks good")))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let comment = client.add_comment("DEMO-1", "Looks good").await.unwrap();

        assert_eq!(comment.text.as_deref(), Some("Looks good"));
        let requests = server.received_requests().await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
        assert_eq!(body["text"], "Looks good");
    }

    #[tokio::test]
    async fn add_comment_maps_404_to_not_found_with_the_issue_id() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/issues/DEMO-404/comments"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let error = client
            .add_comment("DEMO-404", "Looks good")
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
}
