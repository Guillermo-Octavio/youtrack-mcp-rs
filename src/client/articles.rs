//! Knowledge-base article-domain REST calls: list, get, create, update,
//! delete articles, and list/upload an article's attachments.

use crate::client::page::Page;
use crate::client::YouTrackClient;
use crate::error::YouTrackError;
use crate::youtrack::article::{Article, ArticleAttachment};

/// `fields` projection for [`YouTrackClient::list_articles`].
const ARTICLE_LIST_FIELDS: &str = "idReadable,summary,parentArticle(idReadable,summary)";

/// `fields` projection for [`YouTrackClient::get_article`].
const ARTICLE_DETAIL_FIELDS: &str = "idReadable,summary,content,parentArticle(idReadable,summary)";

/// `fields` projection requested from `POST /articles` and
/// `POST /articles/{id}` — small, since callers only need to confirm the
/// mutated article's identity.
const MUTATION_RESPONSE_FIELDS: &str = "idReadable,summary";

/// `fields` projection for [`YouTrackClient::list_article_attachments`].
const ATTACHMENT_FIELDS: &str = "id,name,mimeType,size,url";

/// `fields` projection for [`YouTrackClient::upload_article_attachment`].
const UPLOAD_RESPONSE_FIELDS: &str = "id,name,mimeType,size,url";

impl YouTrackClient {
    /// Lists a project's knowledge-base articles
    /// (`GET /admin/projects/{project_id}/articles`), paginated via
    /// `limit`/`skip`.
    pub async fn list_articles(
        &self,
        project_id: &str,
        limit: usize,
        skip: usize,
    ) -> Result<Page<Article>, YouTrackError> {
        let path = format!("admin/projects/{project_id}/articles");
        self.get_page(&path, ARTICLE_LIST_FIELDS, vec![], limit, skip)
            .await
    }

    /// Fetches a single article by id (`GET /articles/{article_id}`).
    pub async fn get_article(&self, article_id: &str) -> Result<Article, YouTrackError> {
        let path = format!("articles/{article_id}");
        let query = [("fields", ARTICLE_DETAIL_FIELDS.to_string())];
        self.get_json(&path, &query, Some(("article", article_id)))
            .await
    }

    /// Creates a new article in `project_id` (`POST /articles`).
    /// `parent_article` is another article's readable id to nest this
    /// article under; it is resolved to its internal id with one extra
    /// `GET /articles/{parent_article}` lookup first, since the REST API
    /// expects `parentArticle` to reference the internal id, not the
    /// readable one. A `parent_article` that does not exist fails with
    /// [`YouTrackError::NotFound`] naming it, and no create request is
    /// sent.
    pub async fn create_article(
        &self,
        project_id: &str,
        summary: &str,
        content: Option<&str>,
        parent_article: Option<&str>,
    ) -> Result<Article, YouTrackError> {
        let mut body = serde_json::json!({
            "project": { "id": project_id },
            "summary": summary,
        });
        if let Some(content) = content {
            body["content"] = serde_json::Value::String(content.to_string());
        }
        if let Some(parent_article) = parent_article {
            let parent_id = self.resolve_article_id(parent_article).await?;
            body["parentArticle"] = serde_json::json!({ "id": parent_id });
        }

        let query = [("fields", MUTATION_RESPONSE_FIELDS.to_string())];
        self.post_json("articles", &query, &body, None).await
    }

    /// Updates `summary`, `content`, and/or `parentArticle` on an existing
    /// article (`POST /articles/{article_id}`). Only the values the caller
    /// passes are sent. `parent_article` is resolved the same way as in
    /// [`YouTrackClient::create_article`].
    pub async fn update_article(
        &self,
        article_id: &str,
        summary: Option<&str>,
        content: Option<&str>,
        parent_article: Option<&str>,
    ) -> Result<Article, YouTrackError> {
        let mut body = serde_json::json!({});
        if let Some(summary) = summary {
            body["summary"] = serde_json::Value::String(summary.to_string());
        }
        if let Some(content) = content {
            body["content"] = serde_json::Value::String(content.to_string());
        }
        if let Some(parent_article) = parent_article {
            let parent_id = self.resolve_article_id(parent_article).await?;
            body["parentArticle"] = serde_json::json!({ "id": parent_id });
        }

        let path = format!("articles/{article_id}");
        let query = [("fields", MUTATION_RESPONSE_FIELDS.to_string())];
        self.post_json(&path, &query, &body, Some(("article", article_id)))
            .await
    }

    /// Permanently deletes an article (`DELETE /articles/{article_id}`).
    /// This is irreversible: YouTrack has no trash for articles, so the
    /// article, its content, and any sub-articles are lost with no way to
    /// recover them.
    pub async fn delete_article(&self, article_id: &str) -> Result<(), YouTrackError> {
        let path = format!("articles/{article_id}");
        self.delete(&path, Some(("article", article_id))).await
    }

    /// Lists the attachments on an article
    /// (`GET /articles/{article_id}/attachments`), paginated via
    /// `limit`/`skip`.
    pub async fn list_article_attachments(
        &self,
        article_id: &str,
        limit: usize,
        skip: usize,
    ) -> Result<Page<ArticleAttachment>, YouTrackError> {
        let path = format!("articles/{article_id}/attachments");
        self.get_page(&path, ATTACHMENT_FIELDS, vec![], limit, skip)
            .await
    }

    /// Uploads `bytes` as a new attachment named `file_name` on an
    /// article (`POST /articles/{article_id}/attachments`, multipart).
    pub async fn upload_article_attachment(
        &self,
        article_id: &str,
        file_name: &str,
        mime_type: &str,
        bytes: Vec<u8>,
    ) -> Result<ArticleAttachment, YouTrackError> {
        let path = format!("articles/{article_id}/attachments");
        let query = [("fields", UPLOAD_RESPONSE_FIELDS.to_string())];
        let file = crate::client::MultipartFile {
            field_name: "file",
            file_name,
            mime_type,
            bytes,
        };
        self.post_multipart(&path, &query, file, Some(("article", article_id)))
            .await
    }

    /// Resolves an article's readable id to its internal id
    /// (`GET /articles/{article_id}?fields=id`), used by
    /// [`YouTrackClient::create_article`] and
    /// [`YouTrackClient::update_article`] to encode a `parentArticle`
    /// reference.
    async fn resolve_article_id(&self, article_id: &str) -> Result<String, YouTrackError> {
        #[derive(serde::Deserialize)]
        struct IdOnly {
            #[serde(default)]
            id: Option<String>,
        }

        let path = format!("articles/{article_id}");
        let query = [("fields", "id".to_string())];
        let article: IdOnly = self
            .get_json(&path, &query, Some(("article", article_id)))
            .await?;

        article.id.ok_or_else(|| YouTrackError::Decode {
            endpoint: path.clone(),
            source: serde_json::Error::custom("article response is missing its id"),
        })
    }
}

use serde::de::Error as _;

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

    fn article_json(id: &str) -> serde_json::Value {
        serde_json::json!({ "idReadable": id, "summary": format!("Article {id}") })
    }

    #[tokio::test]
    async fn list_articles_batch_smaller_than_limit_has_no_more_pages() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/admin/projects/0-1/articles"))
            .respond_with(ResponseTemplate::new(200).set_body_json(vec![article_json("KB-A-1")]))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let page = client.list_articles("0-1", 5, 0).await.unwrap();

        assert_eq!(page.items.len(), 1);
        assert!(!page.has_more);
    }

    #[tokio::test]
    async fn list_articles_batch_of_limit_plus_one_has_more_pages() {
        let server = MockServer::start().await;
        let batch: Vec<_> = (1..=6)
            .map(|n| article_json(&format!("KB-A-{n}")))
            .collect();
        Mock::given(method("GET"))
            .and(path("/api/admin/projects/0-1/articles"))
            .respond_with(ResponseTemplate::new(200).set_body_json(batch))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let page = client.list_articles("0-1", 5, 0).await.unwrap();

        assert_eq!(page.items.len(), 5);
        assert!(page.has_more);
    }

    #[tokio::test]
    async fn get_article_deserializes_a_single_article_with_content() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/articles/KB-A-2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "idReadable": "KB-A-2",
                "summary": "Runbook",
                "content": "# Steps",
            })))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let article = client.get_article("KB-A-2").await.unwrap();

        assert_eq!(article.id_readable.as_deref(), Some("KB-A-2"));
        assert_eq!(article.content.as_deref(), Some("# Steps"));
    }

    #[tokio::test]
    async fn get_article_maps_404_to_not_found_with_the_article_id() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/articles/KB-A-404"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let error = client.get_article("KB-A-404").await.unwrap_err();

        match error {
            YouTrackError::NotFound { resource, id } => {
                assert_eq!(resource, "article");
                assert_eq!(id, "KB-A-404");
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn create_article_sends_project_summary_and_content() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/articles"))
            .respond_with(ResponseTemplate::new(200).set_body_json(article_json("KB-A-1")))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let article = client
            .create_article("0-1", "Runbook", Some("# Steps"), None)
            .await
            .unwrap();

        assert_eq!(article.id_readable.as_deref(), Some("KB-A-1"));
        let requests = server.received_requests().await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
        assert_eq!(body["project"]["id"], "0-1");
        assert_eq!(body["summary"], "Runbook");
        assert_eq!(body["content"], "# Steps");
    }

    #[tokio::test]
    async fn create_article_resolves_a_parent_article_readable_id_to_its_internal_id() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/articles/KB-A-1"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "id": "3-7" })),
            )
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/articles"))
            .respond_with(ResponseTemplate::new(200).set_body_json(article_json("KB-A-2")))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let article = client
            .create_article("0-1", "Sub-runbook", None, Some("KB-A-1"))
            .await
            .unwrap();

        assert_eq!(article.id_readable.as_deref(), Some("KB-A-2"));
        let requests = server.received_requests().await.unwrap();
        let post = requests
            .iter()
            .find(|r| r.method == wiremock::http::Method::POST)
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&post.body).unwrap();
        assert_eq!(body["parentArticle"]["id"], "3-7");
    }

    #[tokio::test]
    async fn create_article_with_an_unknown_parent_article_fails_without_sending_the_create_request(
    ) {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/articles/KB-A-missing"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/articles"))
            .respond_with(ResponseTemplate::new(200).set_body_json(article_json("KB-A-2")))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let error = client
            .create_article("0-1", "Sub-runbook", None, Some("KB-A-missing"))
            .await
            .unwrap_err();

        match error {
            YouTrackError::NotFound { resource, id } => {
                assert_eq!(resource, "article");
                assert_eq!(id, "KB-A-missing");
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
        let requests = server.received_requests().await.unwrap();
        assert!(!requests
            .iter()
            .any(|r| r.method == wiremock::http::Method::POST));
    }

    #[tokio::test]
    async fn update_article_sends_only_the_provided_fields() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/articles/KB-A-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(article_json("KB-A-1")))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let article = client
            .update_article("KB-A-1", Some("New summary"), None, None)
            .await
            .unwrap();

        assert_eq!(article.id_readable.as_deref(), Some("KB-A-1"));
        let requests = server.received_requests().await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
        assert_eq!(body["summary"], "New summary");
        assert!(body.get("content").is_none());
        assert!(body.get("parentArticle").is_none());
    }

    #[tokio::test]
    async fn update_article_maps_404_to_not_found_with_the_article_id() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/articles/KB-A-404"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let error = client
            .update_article("KB-A-404", Some("New summary"), None, None)
            .await
            .unwrap_err();

        match error {
            YouTrackError::NotFound { resource, id } => {
                assert_eq!(resource, "article");
                assert_eq!(id, "KB-A-404");
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn delete_article_sends_a_delete_request_to_the_article_path() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/api/articles/KB-A-1"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        client.delete_article("KB-A-1").await.unwrap();
    }

    #[tokio::test]
    async fn delete_article_maps_404_to_not_found_with_the_article_id() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/api/articles/KB-A-404"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let error = client.delete_article("KB-A-404").await.unwrap_err();

        match error {
            YouTrackError::NotFound { resource, id } => {
                assert_eq!(resource, "article");
                assert_eq!(id, "KB-A-404");
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn list_article_attachments_deserializes_name_and_size() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/articles/KB-A-1/attachments"))
            .and(query_param("$top", "6"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!([{
                    "id": "att-1",
                    "name": "diagram.png",
                    "mimeType": "image/png",
                    "size": 1024,
                }])),
            )
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let page = client
            .list_article_attachments("KB-A-1", 5, 0)
            .await
            .unwrap();

        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].name.as_deref(), Some("diagram.png"));
        assert_eq!(page.items[0].size, Some(1024));
    }

    #[tokio::test]
    async fn upload_article_attachment_sends_a_multipart_request_with_the_file_part() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/articles/KB-A-1/attachments"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "att-1",
                "name": "notes.txt",
            })))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let attachment = client
            .upload_article_attachment("KB-A-1", "notes.txt", "text/plain", b"hello world".to_vec())
            .await
            .unwrap();

        assert_eq!(attachment.name.as_deref(), Some("notes.txt"));
        let requests = server.received_requests().await.unwrap();
        let body_text = String::from_utf8_lossy(&requests[0].body).to_string();
        assert!(body_text.contains("notes.txt"));
        assert!(body_text.contains("hello world"));
    }

    #[tokio::test]
    async fn upload_article_attachment_maps_404_to_not_found_with_the_article_id() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/articles/KB-A-404/attachments"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let error = client
            .upload_article_attachment("KB-A-404", "notes.txt", "text/plain", b"hi".to_vec())
            .await
            .unwrap_err();

        match error {
            YouTrackError::NotFound { resource, id } => {
                assert_eq!(resource, "article");
                assert_eq!(id, "KB-A-404");
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }
}
