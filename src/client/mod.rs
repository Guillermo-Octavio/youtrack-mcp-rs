//! YouTrack HTTP client.
//!
//! [`YouTrackClient`] wraps one shared [`reqwest::Client`], built once
//! per server process, and knows how to build authenticated request
//! builders against the YouTrack REST API base URL
//! (`{youtrack_url}/api`). Domain-specific calls (search issues,
//! create article, and similar) live in the submodules below, each
//! alongside the tool handlers that call it.

pub mod activity;
pub mod agile;
pub mod articles;
pub mod attachments;
pub mod commands;
pub mod comments;
pub mod issues;
pub mod metadata;
pub mod page;
pub mod projects;
pub mod time;
pub mod users;

use std::time::Duration;

use reqwest::multipart::{Form, Part};
use reqwest::{Client, Method, RequestBuilder};
use serde::de::DeserializeOwned;

use crate::config::Config;
use crate::error::YouTrackError;
use page::{paginate, Page};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

/// Timeout for the dedicated attachment-download HTTP client (120s, wider
/// than [`DEFAULT_TIMEOUT`] since attachment bodies can be considerably
/// larger than a JSON API response).
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);

/// One file part for [`YouTrackClient::post_multipart`], bundling the
/// multipart field name alongside the uploaded file's name, MIME type,
/// and bytes (grouped into one struct to keep the method's argument
/// count reasonable).
pub(crate) struct MultipartFile<'a> {
    pub field_name: &'a str,
    pub file_name: &'a str,
    pub mime_type: &'a str,
    pub bytes: Vec<u8>,
}

/// Thin wrapper around a shared [`reqwest::Client`] configured for
/// authenticated requests against a YouTrack REST API base URL.
#[derive(Debug, Clone)]
pub struct YouTrackClient {
    http: Client,
    /// A second, dedicated client for attachment downloads
    /// (`GET issues/{id}/attachments/{id}`'s `url` field): a longer
    /// timeout, no default `Accept` header, and — critically — no
    /// automatic redirect following, since
    /// [`YouTrackClient::download_attachment_bytes`] must inspect every
    /// redirect's target host itself before deciding whether to follow it.
    download_http: Client,
    base_url: String,
    /// `{youtrack_url}`, with any trailing slash trimmed but with no
    /// `/api` suffix appended — unlike [`YouTrackClient::base_url`],
    /// since attachment download URLs YouTrack returns are relative to
    /// the host root (already including their own `/api/...` segment),
    /// not to this client's `/api` base.
    root_url: String,
    token: String,
}

impl YouTrackClient {
    /// Builds the client's shared [`reqwest::Client`]s once, from a
    /// resolved [`Config`]. The base URL is `{youtrack_url}/api`.
    pub fn new(config: &Config) -> Result<Self, reqwest::Error> {
        let http = Client::builder().timeout(DEFAULT_TIMEOUT).build()?;
        let download_http = Client::builder()
            .timeout(DOWNLOAD_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let root_url = config
            .youtrack_url
            .as_str()
            .trim_end_matches('/')
            .to_string();
        let base_url = format!("{root_url}/api");

        Ok(Self {
            http,
            download_http,
            base_url,
            root_url,
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
            429 => Err(YouTrackError::RateLimited {
                retry_after: retry_after(&response),
            }),
            other => Err(api_error(other, response.text().await.ok())),
        }
    }

    /// Sends a `POST` request to `path` with `query` and a JSON `body`,
    /// deserializing a successful response as `T`. Mirrors
    /// [`YouTrackClient::get_json`]'s status-to-error mapping.
    pub(crate) async fn post_json<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
        body: &serde_json::Value,
        not_found: Option<(&str, &str)>,
    ) -> Result<T, YouTrackError> {
        let response = self
            .request(Method::POST, path)
            .query(query)
            .json(body)
            .send()
            .await?;
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
            429 => Err(YouTrackError::RateLimited {
                retry_after: retry_after(&response),
            }),
            other => Err(api_error(other, response.text().await.ok())),
        }
    }

    /// Sends a `POST` request to `path` with a JSON `body`, discarding
    /// any response body and mapping non-success statuses to
    /// [`YouTrackError`]. Used for endpoints like `/commands` whose
    /// success response shape isn't needed by callers. `not_found` names
    /// the resource/id to report when the API returns `404`; pass `None`
    /// when a `404` should be reported as a generic API error instead.
    pub(crate) async fn post_status_only(
        &self,
        path: &str,
        body: &serde_json::Value,
        not_found: Option<(&str, &str)>,
    ) -> Result<(), YouTrackError> {
        let response = self.request(Method::POST, path).json(body).send().await?;
        let status = response.status();

        if status.is_success() {
            return Ok(());
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
            429 => Err(YouTrackError::RateLimited {
                retry_after: retry_after(&response),
            }),
            other => Err(api_error(other, response.text().await.ok())),
        }
    }

    /// Sends a multipart `POST` request to `path` with one file part
    /// described by `file`, deserializing a successful response as `T`.
    /// Mirrors [`YouTrackClient::get_json`]'s status-to-error mapping.
    /// Used for attachment uploads, which YouTrack's REST API expects as
    /// `multipart/form-data` rather than JSON.
    pub(crate) async fn post_multipart<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
        file: MultipartFile<'_>,
        not_found: Option<(&str, &str)>,
    ) -> Result<T, YouTrackError> {
        let part = Part::bytes(file.bytes)
            .file_name(file.file_name.to_string())
            .mime_str(file.mime_type)
            .map_err(|_| YouTrackError::Validation {
                field: Some("mime_type".to_string()),
                message: format!("invalid mime type: {}", file.mime_type),
            })?;
        let form = Form::new().part(file.field_name.to_string(), part);

        let response = self
            .request(Method::POST, path)
            .query(query)
            .multipart(form)
            .send()
            .await?;
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
            429 => Err(YouTrackError::RateLimited {
                retry_after: retry_after(&response),
            }),
            other => Err(api_error(other, response.text().await.ok())),
        }
    }

    /// Sends a `DELETE` request to `path`, mapping non-success statuses
    /// to [`YouTrackError`]. `not_found` names the resource/id to report
    /// when the API returns `404`.
    pub(crate) async fn delete(
        &self,
        path: &str,
        not_found: Option<(&str, &str)>,
    ) -> Result<(), YouTrackError> {
        let response = self.request(Method::DELETE, path).send().await?;
        let status = response.status();

        if status.is_success() {
            return Ok(());
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
            429 => Err(YouTrackError::RateLimited {
                retry_after: retry_after(&response),
            }),
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

/// Parses a response's `Retry-After` header as a whole number of
/// seconds, per [RFC 9110 §10.2.3](https://www.rfc-editor.org/rfc/rfc9110#field.retry-after).
/// Returns `None` when the header is absent or not a bare integer
/// (e.g. an HTTP-date form, which this client does not need to parse).
fn retry_after(response: &reqwest::Response) -> Option<Duration> {
    response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(Duration::from_secs)
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

    #[derive(Debug, serde::Deserialize, PartialEq)]
    struct Echo {
        ok: bool,
    }

    #[tokio::test]
    async fn post_json_sends_the_body_and_deserializes_a_successful_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/widgets"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "ok": true })),
            )
            .mount(&server)
            .await;
        let client = YouTrackClient::new(&test_config(&server.uri())).unwrap();

        let body = serde_json::json!({ "name": "thing" });
        let result: Echo = client.post_json("widgets", &[], &body, None).await.unwrap();

        assert_eq!(result, Echo { ok: true });
    }

    #[tokio::test]
    async fn post_json_maps_404_to_not_found_when_named() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/widgets/missing"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let client = YouTrackClient::new(&test_config(&server.uri())).unwrap();

        let error = client
            .post_json::<Echo>(
                "widgets/missing",
                &[],
                &serde_json::json!({}),
                Some(("widget", "missing")),
            )
            .await
            .unwrap_err();

        match error {
            YouTrackError::NotFound { resource, id } => {
                assert_eq!(resource, "widget");
                assert_eq!(id, "missing");
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn delete_succeeds_on_a_2xx_response() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/api/widgets/1"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        let client = YouTrackClient::new(&test_config(&server.uri())).unwrap();

        client.delete("widgets/1", None).await.unwrap();
    }

    #[tokio::test]
    async fn delete_maps_404_to_not_found_when_named() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/api/widgets/missing"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let client = YouTrackClient::new(&test_config(&server.uri())).unwrap();

        let error = client
            .delete("widgets/missing", Some(("widget", "missing")))
            .await
            .unwrap_err();

        match error {
            YouTrackError::NotFound { resource, id } => {
                assert_eq!(resource, "widget");
                assert_eq!(id, "missing");
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn get_json_maps_429_with_retry_after_header_to_rate_limited_with_the_parsed_duration() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/widgets"))
            .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "5"))
            .mount(&server)
            .await;
        let client = YouTrackClient::new(&test_config(&server.uri())).unwrap();

        let error = client
            .get_json::<Echo>("widgets", &[], None)
            .await
            .unwrap_err();

        match error {
            YouTrackError::RateLimited { retry_after } => {
                assert_eq!(retry_after, Some(Duration::from_secs(5)));
            }
            other => panic!("expected RateLimited, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn get_json_maps_429_without_retry_after_header_to_rate_limited_with_none() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/widgets"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&server)
            .await;
        let client = YouTrackClient::new(&test_config(&server.uri())).unwrap();

        let error = client
            .get_json::<Echo>("widgets", &[], None)
            .await
            .unwrap_err();

        match error {
            YouTrackError::RateLimited { retry_after } => {
                assert_eq!(retry_after, None);
            }
            other => panic!("expected RateLimited, got {other:?}"),
        }
    }
}
