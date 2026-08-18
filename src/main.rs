//! Binary entry point for the YouTrack MCP server.
//!
//! Loads configuration from the environment, runs the startup
//! connectivity probe against the configured YouTrack instance, and —
//! only once that succeeds — wires the aggregate [`ToolServer`] (see
//! `youtrack_mcp_rs::tools`) into `rmcp`'s standard stdio transport: MCP
//! clients speak JSON-RPC over this process's stdin/stdout.

use std::sync::Arc;

use rmcp::ServiceExt;
use youtrack_mcp_rs::client::YouTrackClient;
use youtrack_mcp_rs::config::Config;
use youtrack_mcp_rs::tools::ToolServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    let server = build_server(&config).await?;

    let server = server.serve(rmcp::transport::stdio()).await?;
    server.waiting().await?;
    Ok(())
}

/// Builds one shared [`YouTrackClient`] from `config` and wires it into a
/// [`ToolServer`], failing fast with an actionable error when the startup
/// connectivity probe (`Config::validate`) does not succeed.
async fn build_server(config: &Config) -> Result<ToolServer, Box<dyn std::error::Error>> {
    let client = YouTrackClient::new(config)?;

    config.validate(client.http_client()).await.map_err(|err| {
        format!(
            "startup connectivity check to {} failed: {err} \
             (check that YOUTRACK_URL and YOUTRACK_TOKEN are correct)",
            config.youtrack_url
        )
    })?;

    Ok(ToolServer::new(Arc::new(client)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use url::Url;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn test_config(base_url: &str) -> Config {
        Config {
            youtrack_url: Url::parse(base_url).unwrap(),
            youtrack_token: "test-token".to_string(),
        }
    }

    #[tokio::test]
    async fn build_server_succeeds_when_the_connectivity_probe_succeeds() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/users/me"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let config = test_config(&server.uri());

        assert!(build_server(&config).await.is_ok());
    }

    #[tokio::test]
    async fn build_server_fails_fast_with_an_actionable_error_when_the_probe_is_unauthorized() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/users/me"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let config = test_config(&server.uri());

        let err = build_server(&config)
            .await
            .expect_err("an unauthorized probe must fail server construction");
        let message = err.to_string();
        assert!(message.contains("startup connectivity check"));
        assert!(message.contains("YOUTRACK_URL"));
    }
}
