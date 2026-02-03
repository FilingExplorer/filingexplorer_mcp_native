//! SEC EDGAR direct access client.
//!
//! Handles direct requests to SEC EDGAR with proper User-Agent headers
//! and rate limiting (max 10 requests per second per SEC fair access policy).

use governor::{Quota, RateLimiter};
use reqwest::{Client, StatusCode};
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

/// SEC EDGAR base URL
const SEC_BASE_URL: &str = "https://www.sec.gov/Archives/edgar/data";

/// Default request timeout in seconds
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// SEC rate limit: 10 requests per second
const SEC_RATE_LIMIT_PER_SECOND: u32 = 10;

#[derive(Error, Debug)]
pub enum SecError {
    #[error("HTTP request failed: {0}")]
    RequestError(#[from] reqwest::Error),

    #[error("SEC access not configured. Please set your email in settings.")]
    NotConfigured,

    #[error("Document not found at SEC")]
    NotFound,

    #[error("SEC returned error {status}: {message}")]
    SecError { status: u16, message: String },

    #[error("Rate limit exceeded")]
    RateLimited,
}

/// Content type detected from response
#[derive(Debug, Clone, PartialEq)]
pub enum ContentType {
    Html,
    Xml,
    Pdf,
    Text,
    Unknown,
}

/// SEC EDGAR client with rate limiting
pub struct SecClient {
    client: Client,
    user_agent: String,
    base_url: String,
    rate_limiter: Arc<RateLimiter<governor::state::NotKeyed, governor::state::InMemoryState, governor::clock::DefaultClock>>,
}

impl SecClient {
    /// Create a new SEC client with the required User-Agent string
    ///
    /// Per SEC fair access policy, the User-Agent should identify
    /// your organization and include a contact email.
    pub fn new(user_agent_name: &str, user_agent_email: &str) -> Result<Self, SecError> {
        let user_agent = format!("{} {}", user_agent_name, user_agent_email);

        let client = Client::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .gzip(true)
            .deflate(true)
            .build()
            .map_err(SecError::RequestError)?;

        // Rate limiter: 10 requests per second
        let quota = Quota::per_second(NonZeroU32::new(SEC_RATE_LIMIT_PER_SECOND).unwrap());
        let rate_limiter = Arc::new(RateLimiter::direct(quota));

        Ok(Self {
            client,
            user_agent,
            base_url: SEC_BASE_URL.to_string(),
            rate_limiter,
        })
    }

    /// Create a client with a custom base URL (for testing)
    #[allow(dead_code)]
    pub fn with_base_url(
        user_agent_name: &str,
        user_agent_email: &str,
        base_url: impl Into<String>,
    ) -> Result<Self, SecError> {
        let mut client = Self::new(user_agent_name, user_agent_email)?;
        client.base_url = base_url.into();
        Ok(client)
    }

    /// Fetch a document directly from SEC EDGAR
    ///
    /// # Arguments
    /// * `cik` - Company CIK (10-digit, with leading zeros)
    /// * `accession_number` - SEC accession number (e.g., "0001234567-23-012345")
    /// * `filename` - Optional specific filename within the filing
    pub async fn fetch_document(
        &self,
        cik: &str,
        accession_number: &str,
        filename: Option<&str>,
    ) -> Result<(Vec<u8>, ContentType), SecError> {
        // Wait for rate limiter
        self.rate_limiter.until_ready().await;

        // Build URL
        // CIK without leading zeros, accession number without dashes
        let cik_stripped = cik.trim_start_matches('0');
        let accession_no_dashes = accession_number.replace('-', "");

        let url = match filename {
            Some(f) => format!(
                "{}/{}/{}/{}",
                self.base_url, cik_stripped, accession_no_dashes, f
            ),
            None => format!(
                "{}/{}/{}/{}.txt",
                self.base_url, cik_stripped, accession_no_dashes, accession_number
            ),
        };

        let response = self
            .client
            .get(&url)
            .header("User-Agent", &self.user_agent)
            .header("Accept-Encoding", "gzip, deflate")
            .send()
            .await?;

        let status = response.status();

        if status == StatusCode::NOT_FOUND {
            return Err(SecError::NotFound);
        }

        if status == StatusCode::TOO_MANY_REQUESTS {
            return Err(SecError::RateLimited);
        }

        if !status.is_success() {
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(SecError::SecError {
                status: status.as_u16(),
                message,
            });
        }

        // Detect content type from headers
        let content_type = self.detect_content_type(&response, filename);

        let bytes = response.bytes().await?.to_vec();

        Ok((bytes, content_type))
    }

    /// Detect content type from response headers and filename
    fn detect_content_type(&self, response: &reqwest::Response, filename: Option<&str>) -> ContentType {
        // Check Content-Type header
        if let Some(ct) = response.headers().get("content-type") {
            if let Ok(ct_str) = ct.to_str() {
                let ct_lower = ct_str.to_lowercase();
                if ct_lower.contains("text/html") {
                    return ContentType::Html;
                }
                if ct_lower.contains("text/xml") || ct_lower.contains("application/xml") {
                    return ContentType::Xml;
                }
                if ct_lower.contains("application/pdf") {
                    return ContentType::Pdf;
                }
                if ct_lower.contains("text/plain") {
                    return ContentType::Text;
                }
            }
        }

        // Check filename extension
        if let Some(fname) = filename {
            let fname_lower = fname.to_lowercase();
            if fname_lower.ends_with(".htm") || fname_lower.ends_with(".html") {
                return ContentType::Html;
            }
            if fname_lower.ends_with(".xml") {
                return ContentType::Xml;
            }
            if fname_lower.ends_with(".pdf") {
                return ContentType::Pdf;
            }
            if fname_lower.ends_with(".txt") {
                return ContentType::Text;
            }
        }

        ContentType::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn test_content_type_enum() {
        assert_ne!(ContentType::Html, ContentType::Pdf);
        assert_eq!(ContentType::Xml, ContentType::Xml);
    }

    #[test]
    fn test_content_type_variants() {
        assert_eq!(ContentType::Html, ContentType::Html);
        assert_eq!(ContentType::Xml, ContentType::Xml);
        assert_eq!(ContentType::Pdf, ContentType::Pdf);
        assert_eq!(ContentType::Text, ContentType::Text);
        assert_eq!(ContentType::Unknown, ContentType::Unknown);

        // Clone works
        let ct = ContentType::Html;
        let ct_cloned = ct.clone();
        assert_eq!(ct, ct_cloned);

        // Debug works
        assert_eq!(format!("{:?}", ContentType::Html), "Html");
        assert_eq!(format!("{:?}", ContentType::Pdf), "Pdf");
    }

    #[test]
    fn test_sec_client_creation() {
        let client = SecClient::new("Test Company", "test@example.com");
        assert!(client.is_ok());

        let client = client.unwrap();
        assert_eq!(client.user_agent, "Test Company test@example.com");
        assert_eq!(client.base_url, SEC_BASE_URL);
    }

    #[test]
    fn test_sec_client_with_custom_base_url() {
        let client =
            SecClient::with_base_url("Test Co", "test@test.com", "https://custom.sec.gov");
        assert!(client.is_ok());

        let client = client.unwrap();
        assert_eq!(client.base_url, "https://custom.sec.gov");
    }

    #[test]
    fn test_sec_error_display() {
        let err = SecError::NotConfigured;
        assert_eq!(
            format!("{}", err),
            "SEC access not configured. Please set your email in settings."
        );

        let err = SecError::NotFound;
        assert_eq!(format!("{}", err), "Document not found at SEC");

        let err = SecError::RateLimited;
        assert_eq!(format!("{}", err), "Rate limit exceeded");

        let err = SecError::SecError {
            status: 500,
            message: "Server error".to_string(),
        };
        assert_eq!(format!("{}", err), "SEC returned error 500: Server error");
    }

    #[tokio::test]
    async fn test_fetch_document_success() {
        let mock_server = MockServer::start().await;

        // CIK 0000320193 -> stripped to 320193
        // Accession 0001193125-23-123456 -> no dashes 0001193125231123456
        Mock::given(method("GET"))
            .and(path("/320193/000119312523123456/filing.txt"))
            .and(header("User-Agent", "Test Company test@example.com"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("Filing content")
                    .insert_header("Content-Type", "text/plain"),
            )
            .mount(&mock_server)
            .await;

        let client =
            SecClient::with_base_url("Test Company", "test@example.com", mock_server.uri())
                .unwrap();

        let result = client
            .fetch_document("0000320193", "0001193125-23-123456", Some("filing.txt"))
            .await;

        assert!(result.is_ok());
        let (bytes, content_type) = result.unwrap();
        assert_eq!(String::from_utf8_lossy(&bytes), "Filing content");
        assert_eq!(content_type, ContentType::Text);
    }

    #[tokio::test]
    async fn test_fetch_document_default_filename() {
        let mock_server = MockServer::start().await;

        // When no filename, uses accession_number.txt (with dashes)
        Mock::given(method("GET"))
            .and(path("/320193/000119312523123456/0001193125-23-123456.txt"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("Default filing")
                    .insert_header("Content-Type", "text/plain"),
            )
            .mount(&mock_server)
            .await;

        let client =
            SecClient::with_base_url("Test Company", "test@example.com", mock_server.uri())
                .unwrap();

        let result = client
            .fetch_document("0000320193", "0001193125-23-123456", None)
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_fetch_document_cik_zero_stripping() {
        let mock_server = MockServer::start().await;

        // CIK with many leading zeros should be stripped
        Mock::given(method("GET"))
            .and(path("/123/000012300012345/doc.htm"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<html></html>"))
            .mount(&mock_server)
            .await;

        let client =
            SecClient::with_base_url("Test Company", "test@example.com", mock_server.uri())
                .unwrap();

        // CIK 0000000123 should become 123
        let result = client
            .fetch_document("0000000123", "0000123000-12-345", Some("doc.htm"))
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_fetch_document_not_found() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let client =
            SecClient::with_base_url("Test Company", "test@example.com", mock_server.uri())
                .unwrap();

        let result = client
            .fetch_document("0000320193", "0001193125-23-123456", Some("missing.txt"))
            .await;

        assert!(matches!(result, Err(SecError::NotFound)));
    }

    #[tokio::test]
    async fn test_fetch_document_rate_limited() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&mock_server)
            .await;

        let client =
            SecClient::with_base_url("Test Company", "test@example.com", mock_server.uri())
                .unwrap();

        let result = client
            .fetch_document("0000320193", "0001193125-23-123456", Some("doc.txt"))
            .await;

        assert!(matches!(result, Err(SecError::RateLimited)));
    }

    #[tokio::test]
    async fn test_fetch_document_server_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(500).set_body_string("Internal Server Error"),
            )
            .mount(&mock_server)
            .await;

        let client =
            SecClient::with_base_url("Test Company", "test@example.com", mock_server.uri())
                .unwrap();

        let result = client
            .fetch_document("0000320193", "0001193125-23-123456", Some("doc.txt"))
            .await;

        match result {
            Err(SecError::SecError { status, message }) => {
                assert_eq!(status, 500);
                assert_eq!(message, "Internal Server Error");
            }
            _ => panic!("Expected SecError::SecError"),
        }
    }

    #[tokio::test]
    async fn test_content_type_from_header_html() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(b"<html></html>".to_vec())
                    .insert_header("Content-Type", "text/html; charset=utf-8"),
            )
            .mount(&mock_server)
            .await;

        let client =
            SecClient::with_base_url("Test Company", "test@example.com", mock_server.uri())
                .unwrap();

        let (_, content_type) = client
            .fetch_document("0000320193", "0001193125-23-123456", Some("doc.htm"))
            .await
            .unwrap();

        assert_eq!(content_type, ContentType::Html);
    }

    #[tokio::test]
    async fn test_content_type_from_header_xml() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(b"<?xml version=\"1.0\"?>".to_vec())
                    .insert_header("Content-Type", "application/xml"),
            )
            .mount(&mock_server)
            .await;

        let client =
            SecClient::with_base_url("Test Company", "test@example.com", mock_server.uri())
                .unwrap();

        let (_, content_type) = client
            .fetch_document("0000320193", "0001193125-23-123456", Some("doc.xml"))
            .await
            .unwrap();

        assert_eq!(content_type, ContentType::Xml);
    }

    #[tokio::test]
    async fn test_content_type_from_header_pdf() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(b"%PDF-1.4".to_vec())
                    .insert_header("Content-Type", "application/pdf"),
            )
            .mount(&mock_server)
            .await;

        let client =
            SecClient::with_base_url("Test Company", "test@example.com", mock_server.uri())
                .unwrap();

        let (_, content_type) = client
            .fetch_document("0000320193", "0001193125-23-123456", Some("doc.pdf"))
            .await
            .unwrap();

        assert_eq!(content_type, ContentType::Pdf);
    }

    #[tokio::test]
    async fn test_content_type_from_filename_html() {
        let mock_server = MockServer::start().await;

        // Use set_body_bytes with application/octet-stream to test filename fallback
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(b"<html></html>".to_vec())
                    .insert_header("Content-Type", "application/octet-stream"),
            )
            .mount(&mock_server)
            .await;

        let client =
            SecClient::with_base_url("Test Company", "test@example.com", mock_server.uri())
                .unwrap();

        let (_, content_type) = client
            .fetch_document("0000320193", "0001193125-23-123456", Some("filing.html"))
            .await
            .unwrap();

        assert_eq!(content_type, ContentType::Html);
    }

    #[tokio::test]
    async fn test_content_type_from_filename_htm() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(b"<html></html>".to_vec())
                    .insert_header("Content-Type", "application/octet-stream"),
            )
            .mount(&mock_server)
            .await;

        let client =
            SecClient::with_base_url("Test Company", "test@example.com", mock_server.uri())
                .unwrap();

        let (_, content_type) = client
            .fetch_document("0000320193", "0001193125-23-123456", Some("filing.HTM"))
            .await
            .unwrap();

        assert_eq!(content_type, ContentType::Html);
    }

    #[tokio::test]
    async fn test_content_type_from_filename_xml() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(b"<root/>".to_vec())
                    .insert_header("Content-Type", "application/octet-stream"),
            )
            .mount(&mock_server)
            .await;

        let client =
            SecClient::with_base_url("Test Company", "test@example.com", mock_server.uri())
                .unwrap();

        let (_, content_type) = client
            .fetch_document("0000320193", "0001193125-23-123456", Some("data.XML"))
            .await
            .unwrap();

        assert_eq!(content_type, ContentType::Xml);
    }

    #[tokio::test]
    async fn test_content_type_from_filename_txt() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(b"plain text".to_vec())
                    .insert_header("Content-Type", "application/octet-stream"),
            )
            .mount(&mock_server)
            .await;

        let client =
            SecClient::with_base_url("Test Company", "test@example.com", mock_server.uri())
                .unwrap();

        let (_, content_type) = client
            .fetch_document("0000320193", "0001193125-23-123456", Some("doc.TXT"))
            .await
            .unwrap();

        assert_eq!(content_type, ContentType::Text);
    }

    #[tokio::test]
    async fn test_content_type_unknown() {
        let mock_server = MockServer::start().await;

        // Unknown content type, unknown extension
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(b"binary data".to_vec())
                    .insert_header("Content-Type", "application/octet-stream"),
            )
            .mount(&mock_server)
            .await;

        let client =
            SecClient::with_base_url("Test Company", "test@example.com", mock_server.uri())
                .unwrap();

        let (_, content_type) = client
            .fetch_document("0000320193", "0001193125-23-123456", Some("data.bin"))
            .await
            .unwrap();

        assert_eq!(content_type, ContentType::Unknown);
    }

    #[tokio::test]
    async fn test_content_type_text_xml_header() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(b"<root/>".to_vec())
                    .insert_header("Content-Type", "text/xml"),
            )
            .mount(&mock_server)
            .await;

        let client =
            SecClient::with_base_url("Test Company", "test@example.com", mock_server.uri())
                .unwrap();

        let (_, content_type) = client
            .fetch_document("0000320193", "0001193125-23-123456", Some("doc.xml"))
            .await
            .unwrap();

        assert_eq!(content_type, ContentType::Xml);
    }
}
