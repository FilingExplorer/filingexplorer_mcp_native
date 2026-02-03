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
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

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

    #[test]
    fn test_build_params_empty() {
        let params: HashMap<String, String> =
            build_params::<_, &str, &str>([("key", None), ("other", None)]);
        assert!(params.is_empty());
    }

    #[test]
    fn test_build_params_all_some() {
        let params = build_params([
            ("a", Some("1")),
            ("b", Some("2")),
            ("c", Some("3")),
        ]);
        assert_eq!(params.len(), 3);
        assert_eq!(params.get("a"), Some(&"1".to_string()));
        assert_eq!(params.get("b"), Some(&"2".to_string()));
        assert_eq!(params.get("c"), Some(&"3".to_string()));
    }

    #[test]
    fn test_api_client_creation() {
        let client = ApiClient::new("test_token");
        assert!(client.is_ok());

        let client = client.unwrap();
        assert_eq!(client.api_token, "test_token");
        assert_eq!(client.base_url, API_BASE_URL);
    }

    #[test]
    fn test_api_client_with_custom_base_url() {
        let client = ApiClient::with_base_url("test_token", "https://custom.api.com");
        assert!(client.is_ok());

        let client = client.unwrap();
        assert_eq!(client.base_url, "https://custom.api.com");
    }

    #[tokio::test]
    async fn test_get_request_success() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/test-endpoint"))
            .and(header("Authorization", "Bearer test_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 1,
                "name": "Test"
            })))
            .mount(&mock_server)
            .await;

        let client = ApiClient::with_base_url("test_token", mock_server.uri()).unwrap();
        let result: Value = client.get("test-endpoint", None).await.unwrap();

        assert_eq!(result["id"], 1);
        assert_eq!(result["name"], "Test");
    }

    #[tokio::test]
    async fn test_get_request_with_params() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/search"))
            .and(query_param("limit", "10"))
            .and(query_param("offset", "0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "results": []
            })))
            .mount(&mock_server)
            .await;

        let client = ApiClient::with_base_url("test_token", mock_server.uri()).unwrap();
        let params = build_params([("limit", Some("10")), ("offset", Some("0"))]);
        let result: Value = client.get("search", Some(params)).await.unwrap();

        assert!(result["results"].is_array());
    }

    #[tokio::test]
    async fn test_get_request_strips_leading_slash() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/endpoint"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&mock_server)
            .await;

        let client = ApiClient::with_base_url("test_token", mock_server.uri()).unwrap();

        // Both should work the same
        let result1: Value = client.get("/endpoint", None).await.unwrap();
        let result2: Value = client.get("endpoint", None).await.unwrap();

        assert!(result1.is_object());
        assert!(result2.is_object());
    }

    #[tokio::test]
    async fn test_post_request_success() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/items"))
            .and(header("Authorization", "Bearer test_token"))
            .and(header("Content-Type", "application/json"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "id": 123,
                "created": true
            })))
            .mount(&mock_server)
            .await;

        let client = ApiClient::with_base_url("test_token", mock_server.uri()).unwrap();
        let body = serde_json::json!({"name": "New Item"});
        let result: Value = client.post("items", Some(&body)).await.unwrap();

        assert_eq!(result["id"], 123);
        assert_eq!(result["created"], true);
    }

    #[tokio::test]
    async fn test_post_request_without_body() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/action"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "success": true
            })))
            .mount(&mock_server)
            .await;

        let client = ApiClient::with_base_url("test_token", mock_server.uri()).unwrap();
        let result: Value = client.post("action", None).await.unwrap();

        assert_eq!(result["success"], true);
    }

    #[tokio::test]
    async fn test_patch_request_success() {
        let mock_server = MockServer::start().await;

        Mock::given(method("PATCH"))
            .and(path("/items/1"))
            .and(header("Authorization", "Bearer test_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 1,
                "updated": true
            })))
            .mount(&mock_server)
            .await;

        let client = ApiClient::with_base_url("test_token", mock_server.uri()).unwrap();
        let body = serde_json::json!({"name": "Updated"});
        let result: Value = client.patch("items/1", Some(&body)).await.unwrap();

        assert_eq!(result["updated"], true);
    }

    #[tokio::test]
    async fn test_delete_request_success() {
        let mock_server = MockServer::start().await;

        Mock::given(method("DELETE"))
            .and(path("/items/1"))
            .and(header("Authorization", "Bearer test_token"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&mock_server)
            .await;

        let client = ApiClient::with_base_url("test_token", mock_server.uri()).unwrap();
        let result = client.delete("items/1").await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_delete_request_success_with_200() {
        let mock_server = MockServer::start().await;

        Mock::given(method("DELETE"))
            .and(path("/items/2"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let client = ApiClient::with_base_url("test_token", mock_server.uri()).unwrap();
        let result = client.delete("items/2").await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_error_unauthorized() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/protected"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&mock_server)
            .await;

        let client = ApiClient::with_base_url("bad_token", mock_server.uri()).unwrap();
        let result: Result<Value, _> = client.get("protected", None).await;

        assert!(matches!(result, Err(ApiError::Unauthorized)));
    }

    #[tokio::test]
    async fn test_error_not_found() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/missing"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let client = ApiClient::with_base_url("test_token", mock_server.uri()).unwrap();
        let result: Result<Value, _> = client.get("missing", None).await;

        assert!(matches!(result, Err(ApiError::NotFound)));
    }

    #[tokio::test]
    async fn test_error_rate_limited() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/limited"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&mock_server)
            .await;

        let client = ApiClient::with_base_url("test_token", mock_server.uri()).unwrap();
        let result: Result<Value, _> = client.get("limited", None).await;

        assert!(matches!(result, Err(ApiError::RateLimited)));
    }

    #[tokio::test]
    async fn test_error_generic_api_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/error"))
            .respond_with(
                ResponseTemplate::new(500).set_body_string("Internal Server Error"),
            )
            .mount(&mock_server)
            .await;

        let client = ApiClient::with_base_url("test_token", mock_server.uri()).unwrap();
        let result: Result<Value, _> = client.get("error", None).await;

        match result {
            Err(ApiError::ApiError { status, message }) => {
                assert_eq!(status, 500);
                assert_eq!(message, "Internal Server Error");
            }
            _ => panic!("Expected ApiError::ApiError"),
        }
    }

    #[tokio::test]
    async fn test_delete_error_not_found() {
        let mock_server = MockServer::start().await;

        Mock::given(method("DELETE"))
            .and(path("/items/999"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let client = ApiClient::with_base_url("test_token", mock_server.uri()).unwrap();
        let result = client.delete("items/999").await;

        assert!(matches!(result, Err(ApiError::NotFound)));
    }

    #[tokio::test]
    async fn test_validate_token_valid() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/lists"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .mount(&mock_server)
            .await;

        let client = ApiClient::with_base_url("valid_token", mock_server.uri()).unwrap();
        let result = client.validate_token().await.unwrap();

        assert!(result);
    }

    #[tokio::test]
    async fn test_validate_token_invalid() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/lists"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&mock_server)
            .await;

        let client = ApiClient::with_base_url("invalid_token", mock_server.uri()).unwrap();
        let result = client.validate_token().await.unwrap();

        assert!(!result);
    }

    #[tokio::test]
    async fn test_validate_token_other_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/lists"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Server Error"))
            .mount(&mock_server)
            .await;

        let client = ApiClient::with_base_url("test_token", mock_server.uri()).unwrap();
        let result = client.validate_token().await;

        assert!(matches!(result, Err(ApiError::ApiError { status: 500, .. })));
    }

    #[tokio::test]
    async fn test_get_json_returns_value() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/data"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "key": "value",
                "number": 42
            })))
            .mount(&mock_server)
            .await;

        let client = ApiClient::with_base_url("test_token", mock_server.uri()).unwrap();
        let result = client.get_json("data", None).await.unwrap();

        assert_eq!(result["key"], "value");
        assert_eq!(result["number"], 42);
    }

    #[tokio::test]
    async fn test_parse_error_invalid_json() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/bad-json"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not valid json"))
            .mount(&mock_server)
            .await;

        let client = ApiClient::with_base_url("test_token", mock_server.uri()).unwrap();
        let result: Result<Value, _> = client.get("bad-json", None).await;

        assert!(matches!(result, Err(ApiError::ParseError(_))));
    }

    #[test]
    fn test_api_error_display() {
        let err = ApiError::Unauthorized;
        assert_eq!(
            format!("{}", err),
            "Authentication failed: Invalid or missing API token"
        );

        let err = ApiError::NotFound;
        assert_eq!(format!("{}", err), "Resource not found");

        let err = ApiError::RateLimited;
        assert_eq!(format!("{}", err), "Rate limited - please slow down requests");

        let err = ApiError::ApiError {
            status: 500,
            message: "Server error".to_string(),
        };
        assert_eq!(format!("{}", err), "API returned error 500: Server error");
    }
}
