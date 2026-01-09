//! FilingExplorer API client.
//!
//! Async HTTP client for the FilingExplorer API with authentication
//! and error handling.

use reqwest::{Client, Response, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use thiserror::Error;

/// Base URL for the FilingExplorer API
const API_BASE_URL: &str = "https://api.filingexplorer.com/v1";

/// Default request timeout in seconds
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// User-Agent header value
const USER_AGENT: &str = "Giant Octopus, LLC hello@giantoctopus.ink";

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("HTTP request failed: {0}")]
    RequestError(#[from] reqwest::Error),

    #[error("API returned error {status}: {message}")]
    ApiError { status: u16, message: String },

    #[error("Failed to parse response: {0}")]
    ParseError(#[from] serde_json::Error),

    #[error("Authentication failed: Invalid or missing API token")]
    Unauthorized,

    #[error("Resource not found")]
    NotFound,

    #[error("Rate limited - please slow down requests")]
    RateLimited,
}

/// FilingExplorer API client
#[derive(Clone)]
pub struct ApiClient {
    client: Client,
    api_token: String,
    base_url: String,
}

impl ApiClient {
    /// Create a new API client with the given token
    pub fn new(api_token: impl Into<String>) -> Result<Self, ApiError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .gzip(true)
            .deflate(true)
            .user_agent(USER_AGENT)
            .build()?;

        Ok(Self {
            client,
            api_token: api_token.into(),
            base_url: API_BASE_URL.to_string(),
        })
    }

    /// Create a client with a custom base URL (for testing)
    #[allow(dead_code)]
    pub fn with_base_url(api_token: impl Into<String>, base_url: impl Into<String>) -> Result<Self, ApiError> {
        let mut client = Self::new(api_token)?;
        client.base_url = base_url.into();
        Ok(client)
    }

    /// Make a GET request to the API
    pub async fn get<T: DeserializeOwned>(
        &self,
        endpoint: &str,
        params: Option<HashMap<String, String>>,
    ) -> Result<T, ApiError> {
        let url = format!("{}/{}", self.base_url, endpoint.trim_start_matches('/'));

        let mut request = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_token));

        if let Some(params) = params {
            request = request.query(&params);
        }

        let response = request.send().await?;
        self.handle_response(response).await
    }

    /// Make a GET request and return raw JSON Value
    pub async fn get_json(
        &self,
        endpoint: &str,
        params: Option<HashMap<String, String>>,
    ) -> Result<Value, ApiError> {
        self.get(endpoint, params).await
    }

    /// Make a POST request to the API
    pub async fn post<T: DeserializeOwned>(
        &self,
        endpoint: &str,
        body: Option<&Value>,
    ) -> Result<T, ApiError> {
        let url = format!("{}/{}", self.base_url, endpoint.trim_start_matches('/'));

        let mut request = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .header("Content-Type", "application/json");

        if let Some(body) = body {
            request = request.json(body);
        }

        let response = request.send().await?;
        self.handle_response(response).await
    }

    /// Make a PATCH request to the API
    pub async fn patch<T: DeserializeOwned>(
        &self,
        endpoint: &str,
        body: Option<&Value>,
    ) -> Result<T, ApiError> {
        let url = format!("{}/{}", self.base_url, endpoint.trim_start_matches('/'));

        let mut request = self
            .client
            .patch(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .header("Content-Type", "application/json");

        if let Some(body) = body {
            request = request.json(body);
        }

        let response = request.send().await?;
        self.handle_response(response).await
    }

    /// Make a DELETE request to the API
    pub async fn delete(&self, endpoint: &str) -> Result<(), ApiError> {
        let url = format!("{}/{}", self.base_url, endpoint.trim_start_matches('/'));

        let response = self
            .client
            .delete(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .send()
            .await?;

        let status = response.status();

        if status.is_success() || status == StatusCode::NO_CONTENT {
            Ok(())
        } else {
            Err(self.error_from_response(response).await)
        }
    }

    /// Validate the API token by making a test request
    pub async fn validate_token(&self) -> Result<bool, ApiError> {
        // Try to get the user's lists as a validation check
        let result: Result<Value, _> = self.get("lists", None).await;

        match result {
            Ok(_) => Ok(true),
            Err(ApiError::Unauthorized) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Handle API response, converting to typed result or error
    async fn handle_response<T: DeserializeOwned>(&self, response: Response) -> Result<T, ApiError> {
        let status = response.status();

        if status.is_success() {
            let body = response.text().await?;
            let parsed: T = serde_json::from_str(&body)?;
            Ok(parsed)
        } else {
            Err(self.error_from_response(response).await)
        }
    }

    /// Convert error response to ApiError
    async fn error_from_response(&self, response: Response) -> ApiError {
        let status = response.status();

        match status {
            StatusCode::UNAUTHORIZED => ApiError::Unauthorized,
            StatusCode::NOT_FOUND => ApiError::NotFound,
            StatusCode::TOO_MANY_REQUESTS => ApiError::RateLimited,
            _ => {
                let message = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                ApiError::ApiError {
                    status: status.as_u16(),
                    message,
                }
            }
        }
    }
}

/// Helper to build query parameters, filtering out None values
pub fn build_params<I, K, V>(pairs: I) -> HashMap<String, String>
where
    I: IntoIterator<Item = (K, Option<V>)>,
    K: Into<String>,
    V: ToString,
{
    pairs
        .into_iter()
        .filter_map(|(k, v)| v.map(|val| (k.into(), val.to_string())))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_params() {
        let params = build_params([
            ("limit", Some(10)),
            ("offset", None),
            ("page", Some(1)),
        ]);

        assert_eq!(params.get("limit"), Some(&"10".to_string()));
        assert_eq!(params.get("page"), Some(&"1".to_string()));
        assert!(params.get("offset").is_none());
    }
}
