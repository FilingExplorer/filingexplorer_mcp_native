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
            rate_limiter,
        })
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
                SEC_BASE_URL, cik_stripped, accession_no_dashes, f
            ),
            None => format!(
                "{}/{}/{}/{}.txt",
                SEC_BASE_URL, cik_stripped, accession_no_dashes, accession_number
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

    #[test]
    fn test_content_type_enum() {
        assert_ne!(ContentType::Html, ContentType::Pdf);
        assert_eq!(ContentType::Xml, ContentType::Xml);
    }
}
