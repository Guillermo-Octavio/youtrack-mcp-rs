//! YouTrack HTTP client.
//!
//! [`YouTrackClient`] wraps one shared [`reqwest::Client`], built once
//! per server process, and knows how to build authenticated request
//! builders against the YouTrack REST API base URL
//! (`{youtrack_url}/api`). Domain-specific calls (search issues,
//! create article, and similar) are added alongside the tools that
//! need them in later phases of the SDD change `rust-rewrite`.

pub mod agile;
pub mod issues;
pub mod page;
pub mod projects;
pub mod resolve;

use std::time::Duration;

use reqwest::{Client, Method, RequestBuilder};
use serde::de::DeserializeOwned;

use crate::config::Config;
use crate::error::YouTrackError;
use page::{paginate, Page};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

/// Thin wrapper around a shared [`reqwest::Client`] configured for
/// authenticated requests against a YouTrack REST API base URL.
#[derive(Debug, Clone)]
pub struct YouTrackClient {
    http: Client,
    base_url: String,
    token: String,
}

impl YouTrackClient {
    /// Builds the client's shared [`reqwest::Client`] once, from a
    /// resolved [`Config`]. The base URL is `{youtrack_url}/api`.
    pub fn new(config: &Config) -> Result<Self, reqwest::Error> {
        let http = Client::builder().timeout(DEFAULT_TIMEOUT).build()?;
        let base_url = format!("{}/api", config.youtrack_url.as_str().trim_end_matches('/'));

        Ok(Self {
            http,
            base_url,
            token: config.youtrack_token.clone(),
        })
    }

    /// Builds an authenticated [`RequestBuilder`] for `path`, joined
    /// onto this client's base URL, with the `Authorization` bearer
    /// header and an `Accept: application/json` header already set.
    pub fn request(&self, method: Method, path: &str) -> RequestBuilder {
        let url = format!("{}/{}", self.base_url, path.trim_start_matches('/'));

        self.http
            .request(method, url)
            .bearer_auth(&self.token)
            .header("Accept", "application/json")
    }

    /// Exposes this client's shared [`reqwest::Client`], e.g. so the
    /// startup connectivity probe (`Config::validate`) can reuse it
    /// instead of building a second one.
    pub fn http_client(&self) -> &Client {
        &self.http
    }

    /// Sends a `GET` request to `path` with `query` and deserializes a
    /// successful JSON response as `T`, mapping non-success statuses to
    /// [`YouTrackError`]. `not_found` names the resource/id to report
    /// when the API returns `404`; pass `None` when a `404` should be
    /// reported as a generic API error instead.
    pub(crate) async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
        not_found: Option<(&str, &str)>,
    ) -> Result<T, YouTrackError> {
        let response = self.request(Method::GET, path).query(query).send().await?;
        let status = response.status();

        if status.is_success() {
            let bytes = response.bytes().await?;
            return serde_json::from_slice(&bytes).map_err(|source| YouTrackError::Decode {
                endpoint: path.to_string(),
                source,
            });
        }

        match status.as_u16() {
            401 => Err(YouTrackError::Unauthorized),
            403 => Err(YouTrackError::Forbidden),
            404 => match not_found {
                Some((resource, id)) => Err(YouTrackError::NotFound {
                    resource: resource.to_string(),
                    id: id.to_string(),
                }),
                None => Err(api_error(status.as_u16(), response.text().await.ok())),
            },
            other => Err(api_error(other, response.text().await.ok())),
        }
    }

    /// Fetches one page of up to `limit + 1` items from `path` (see
    /// [`paginate`]), setting `fields`, `$top`, and `$skip` on top of any
    /// `query` the caller already needs.
    pub(crate) async fn get_page<T: DeserializeOwned>(
        &self,
        path: &str,
        fields: &str,
        mut query: Vec<(&str, String)>,
        limit: usize,
        skip: usize,
    ) -> Result<Page<T>, YouTrackError> {
        query.push(("fields", fields.to_string()));
        query.push(("$top", (limit + 1).to_string()));
        query.push(("$skip", skip.to_string()));

        let batch: Vec<T> = self.get_json(path, &query, None).await?;
        Ok(paginate(batch, limit, skip))
    }
}

/// Builds a [`YouTrackError::Api`] from a non-success status, extracting
/// YouTrack's `{"error": ..., "error_description": ...}` body shape when
/// present.
fn api_error(status: u16, body: Option<String>) -> YouTrackError {
    let parsed: serde_json::Value = body
        .as_deref()
        .and_then(|text| serde_json::from_str(text).ok())
        .unwrap_or_default();

    YouTrackError::Api {
        status,
        error: parsed
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error")
            .to_string(),
        description: parsed
            .get("error_description")
            .and_then(|v| v.as_str())
            .map(str::to_string),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use reqwest::Method;
    use url::Url;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn test_config(base_url: &str) -> Config {
        Config {
            youtrack_url: Url::parse(base_url).unwrap(),
            youtrack_token: "test-token".to_string(),
        }
    }

    #[test]
    fn request_joins_base_url_with_path_and_sets_headers() {
        let config = test_config("https://example.myjetbrains.com/youtrack");
        let client = YouTrackClient::new(&config).unwrap();

        let request = client
            .request(Method::GET, "issues/DEMO-1")
            .build()
            .unwrap();

        assert_eq!(
            request.url().as_str(),
            "https://example.myjetbrains.com/youtrack/api/issues/DEMO-1"
        );
        assert_eq!(
            request.headers().get("authorization").unwrap(),
            "Bearer test-token"
        );
        assert_eq!(request.headers().get("accept").unwrap(), "application/json");
    }

    #[test]
    fn request_joins_base_url_without_subpath_and_leading_slash_path() {
        let config = test_config("https://youtrack.example.com");
        let client = YouTrackClient::new(&config).unwrap();

        let request = client.request(Method::GET, "/users/me").build().unwrap();

        assert_eq!(
            request.url().as_str(),
            "https://youtrack.example.com/api/users/me"
        );
    }

    #[tokio::test]
    async fn request_reaches_mock_server_with_expected_path_and_auth_header() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/users/me"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let config = test_config(&server.uri());
        let client = YouTrackClient::new(&config).unwrap();

        let response = client
            .request(Method::GET, "users/me")
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), 200);
    }
}
