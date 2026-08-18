//! Server configuration.
//!
//! [`Config::from_env`] reads `YOUTRACK_URL` and `YOUTRACK_TOKEN` from
//! the process environment. It normalizes a trailing slash, rejects a
//! URL that already includes the `/api` suffix (the client module adds
//! that itself), and requires an absolute `https` URL, returning a
//! typed [`ConfigError`] rather than panicking on missing or malformed
//! input. [`Config::validate`] performs the startup probe against
//! `/api/users/me` to confirm the URL and token actually work.

use std::env;
use std::time::Duration;

use thiserror::Error;
use url::Url;

use crate::error::YouTrackError;

/// Resolved server configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub youtrack_url: Url,
    pub youtrack_token: String,
}

/// Configuration could not be loaded from the environment.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ConfigError {
    #[error("YOUTRACK_URL environment variable is not set")]
    MissingUrl,
    #[error("YOUTRACK_TOKEN environment variable is not set or empty")]
    MissingToken,
    #[error("YOUTRACK_URL must be an absolute https URL, got: {0}")]
    InvalidUrl(String),
    #[error("YOUTRACK_URL must not already include the `/api` suffix, got: {0}")]
    UrlIncludesApiSuffix(String),
}

impl Config {
    /// Loads configuration from `YOUTRACK_URL` and `YOUTRACK_TOKEN`.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::parse(
            env::var("YOUTRACK_URL").ok(),
            env::var("YOUTRACK_TOKEN").ok(),
        )
    }

    /// Pure parsing/validation logic, independent of the process
    /// environment so it can be exercised directly in tests.
    fn parse(url: Option<String>, token: Option<String>) -> Result<Self, ConfigError> {
        let raw_url = url.ok_or(ConfigError::MissingUrl)?;
        let token = token.ok_or(ConfigError::MissingToken)?;
        if token.trim().is_empty() {
            return Err(ConfigError::MissingToken);
        }

        let trimmed = raw_url.trim_end_matches('/');
        if trimmed.is_empty() {
            return Err(ConfigError::InvalidUrl(raw_url));
        }

        let parsed = Url::parse(trimmed).map_err(|_| ConfigError::InvalidUrl(raw_url.clone()))?;
        if parsed.scheme() != "https" {
            return Err(ConfigError::InvalidUrl(raw_url));
        }
        if trimmed.ends_with("/api") {
            return Err(ConfigError::UrlIncludesApiSuffix(raw_url));
        }

        Ok(Config {
            youtrack_url: parsed,
            youtrack_token: token,
        })
    }

    /// Probes `{youtrack_url}/api/users/me` with a 10-second timeout to
    /// confirm the URL and token are usable before the server starts
    /// serving MCP requests.
    pub async fn validate(&self, client: &reqwest::Client) -> Result<(), YouTrackError> {
        let probe_url = format!(
            "{}/api/users/me",
            self.youtrack_url.as_str().trim_end_matches('/')
        );

        let response = client
            .get(probe_url)
            .bearer_auth(&self.youtrack_token)
            .timeout(Duration::from_secs(10))
            .send()
            .await?;

        match response.status().as_u16() {
            200..=299 => Ok(()),
            401 => Err(YouTrackError::Unauthorized),
            403 => Err(YouTrackError::Forbidden),
            status => Err(YouTrackError::Api {
                status,
                error: "startup validation probe failed".to_string(),
                description: None,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn missing_url_is_a_typed_error() {
        let result = Config::parse(None, Some("token".to_string()));

        assert_eq!(result.unwrap_err(), ConfigError::MissingUrl);
    }

    #[test]
    fn missing_token_is_a_typed_error() {
        let result = Config::parse(Some("https://example.com".to_string()), None);

        assert_eq!(result.unwrap_err(), ConfigError::MissingToken);
    }

    #[test]
    fn empty_token_is_a_typed_error() {
        let result = Config::parse(
            Some("https://example.com".to_string()),
            Some("   ".to_string()),
        );

        assert_eq!(result.unwrap_err(), ConfigError::MissingToken);
    }

    #[test]
    fn trailing_slash_is_normalized_away() {
        let config = Config::parse(
            Some("https://example.com/".to_string()),
            Some("token".to_string()),
        )
        .unwrap();

        assert_eq!(config.youtrack_url.as_str(), "https://example.com/");
        assert!(!config.youtrack_url.as_str().ends_with("//"));
    }

    #[test]
    fn url_without_trailing_slash_is_accepted_unchanged() {
        let config = Config::parse(
            Some("https://example.myjetbrains.com/youtrack".to_string()),
            Some("token".to_string()),
        )
        .unwrap();

        assert_eq!(
            config.youtrack_url.as_str(),
            "https://example.myjetbrains.com/youtrack"
        );
    }

    #[test]
    fn http_scheme_is_rejected() {
        let result = Config::parse(
            Some("http://example.com".to_string()),
            Some("token".to_string()),
        );

        assert!(matches!(result, Err(ConfigError::InvalidUrl(_))));
    }

    #[test]
    fn malformed_url_is_rejected() {
        let result = Config::parse(Some("not a url".to_string()), Some("token".to_string()));

        assert!(matches!(result, Err(ConfigError::InvalidUrl(_))));
    }

    #[test]
    fn url_ending_in_api_suffix_is_rejected() {
        let result = Config::parse(
            Some("https://example.com/api".to_string()),
            Some("token".to_string()),
        );

        assert!(matches!(result, Err(ConfigError::UrlIncludesApiSuffix(_))));
    }

    #[test]
    fn url_ending_in_api_suffix_with_trailing_slash_is_rejected() {
        let result = Config::parse(
            Some("https://example.com/api/".to_string()),
            Some("token".to_string()),
        );

        assert!(matches!(result, Err(ConfigError::UrlIncludesApiSuffix(_))));
    }

    #[test]
    fn from_env_reads_url_and_token_from_process_environment() {
        static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = ENV_LOCK.lock().unwrap();

        // SAFETY: guarded by ENV_LOCK; no other test in this process
        // mutates YOUTRACK_URL/YOUTRACK_TOKEN.
        unsafe {
            env::set_var("YOUTRACK_URL", "https://example.com");
            env::set_var("YOUTRACK_TOKEN", "secret-token");
        }

        let config = Config::from_env().unwrap();

        assert_eq!(config.youtrack_url.as_str(), "https://example.com/");
        assert_eq!(config.youtrack_token, "secret-token");

        unsafe {
            env::remove_var("YOUTRACK_URL");
            env::remove_var("YOUTRACK_TOKEN");
        }
    }

    #[tokio::test]
    async fn validate_succeeds_on_200_from_users_me_endpoint() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/users/me"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let config = Config {
            youtrack_url: Url::parse(&server.uri()).unwrap(),
            youtrack_token: "secret-token".to_string(),
        };
        let client = reqwest::Client::new();

        let result = config.validate(&client).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn validate_maps_401_to_unauthorized() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/users/me"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let config = Config {
            youtrack_url: Url::parse(&server.uri()).unwrap(),
            youtrack_token: "bad-token".to_string(),
        };
        let client = reqwest::Client::new();

        let result = config.validate(&client).await;

        assert!(matches!(result, Err(YouTrackError::Unauthorized)));
    }

    #[tokio::test]
    async fn validate_maps_403_to_forbidden() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/users/me"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&server)
            .await;

        let config = Config {
            youtrack_url: Url::parse(&server.uri()).unwrap(),
            youtrack_token: "token".to_string(),
        };
        let client = reqwest::Client::new();

        let result = config.validate(&client).await;

        assert!(matches!(result, Err(YouTrackError::Forbidden)));
    }

    #[tokio::test]
    async fn validate_maps_other_status_to_api_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/users/me"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let config = Config {
            youtrack_url: Url::parse(&server.uri()).unwrap(),
            youtrack_token: "token".to_string(),
        };
        let client = reqwest::Client::new();

        let result = config.validate(&client).await;

        assert!(matches!(
            result,
            Err(YouTrackError::Api { status: 500, .. })
        ));
    }
}
