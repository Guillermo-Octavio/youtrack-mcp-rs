//! YouTrack HTTP client.
//!
//! [`YouTrackClient`] wraps one shared [`reqwest::Client`], built once
//! per server process, and knows how to build authenticated request
//! builders against the YouTrack REST API base URL
//! (`{youtrack_url}/api`). Domain-specific calls (search issues,
//! create article, and similar) are added alongside the tools that
//! need them in later phases of the SDD change `rust-rewrite`.

pub mod page;
pub mod resolve;

use std::time::Duration;

use reqwest::{Client, Method, RequestBuilder};

use crate::config::Config;

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
