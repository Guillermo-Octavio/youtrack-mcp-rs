//! Issue-domain REST calls: search, get, and list an issue's links.

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
}

#[cfg(test)]
mod tests {
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
