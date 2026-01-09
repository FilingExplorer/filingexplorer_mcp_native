//! Configuration management for FilingExplorer MCP.
//!
//! Handles reading and writing config from platform-specific locations:
//! - macOS: ~/Library/Application Support/com.filingexplorer.mcp/config.json
//! - Windows: %APPDATA%\FilingExplorer MCP\config.json
//! - Linux: ~/.config/filing-explorer-mcp/config.json

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

/// Configuration file version for future migrations
const CONFIG_VERSION: u32 = 1;

/// Application identifiers for directory lookup
const QUALIFIER: &str = "com";
const ORGANIZATION: &str = "filingexplorer";
const APPLICATION: &str = "mcp";

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Could not determine config directory for this platform")]
    NoConfigDir,

    #[error("Failed to read config file: {0}")]
    ReadError(#[from] std::io::Error),

    #[error("Failed to parse config file: {0}")]
    ParseError(#[from] serde_json::Error),

    #[error("Config file not found. Please run the settings app to configure.")]
    NotFound,

    #[error("API token not configured")]
    MissingToken,
}

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Config file version for migrations
    #[serde(default = "default_version")]
    pub version: u32,

    /// FilingExplorer API token
    #[serde(default)]
    pub api_token: Option<String>,

    /// User/organization name for SEC EDGAR User-Agent header
    #[serde(default)]
    pub sec_user_agent_name: Option<String>,

    /// Email for SEC EDGAR User-Agent header
    #[serde(default)]
    pub sec_user_agent_email: Option<String>,
}

fn default_version() -> u32 {
    CONFIG_VERSION
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            api_token: None,
            sec_user_agent_name: None,
            sec_user_agent_email: None,
        }
    }
}

impl Config {
    /// Get the platform-specific config directory path
    pub fn config_dir() -> Result<PathBuf, ConfigError> {
        ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)
            .map(|dirs| dirs.config_dir().to_path_buf())
            .ok_or(ConfigError::NoConfigDir)
    }

    /// Get the full path to the config file
    pub fn config_path() -> Result<PathBuf, ConfigError> {
        Ok(Self::config_dir()?.join("config.json"))
    }

    /// Load configuration from disk
    pub fn load() -> Result<Self, ConfigError> {
        let path = Self::config_path()?;

        if !path.exists() {
            return Err(ConfigError::NotFound);
        }

        let contents = fs::read_to_string(&path)?;
        let config: Config = serde_json::from_str(&contents)?;

        Ok(config)
    }

    /// Save configuration to disk
    pub fn save(&self) -> Result<(), ConfigError> {
        let path = Self::config_path()?;

        // Ensure config directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let contents = serde_json::to_string_pretty(self)?;
        fs::write(&path, contents)?;

        Ok(())
    }

    /// Load config or return default if not found
    pub fn load_or_default() -> Self {
        Self::load().unwrap_or_default()
    }

    /// Check if the config has required fields for API access
    pub fn is_api_configured(&self) -> bool {
        self.api_token.as_ref().map_or(false, |t| !t.is_empty())
    }

    /// Check if SEC direct access is configured
    pub fn is_sec_configured(&self) -> bool {
        self.sec_user_agent_name
            .as_ref()
            .map_or(false, |n| !n.is_empty())
            && self
                .sec_user_agent_email
                .as_ref()
                .map_or(false, |e| !e.is_empty())
    }

    /// Get the API token, returning an error if not configured
    pub fn require_api_token(&self) -> Result<&str, ConfigError> {
        self.api_token
            .as_deref()
            .filter(|t| !t.is_empty())
            .ok_or(ConfigError::MissingToken)
    }

    /// Get the SEC User-Agent string if configured
    pub fn sec_user_agent(&self) -> Option<String> {
        match (&self.sec_user_agent_name, &self.sec_user_agent_email) {
            (Some(name), Some(email)) if !name.is_empty() && !email.is_empty() => {
                Some(format!("{} {}", name, email))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.version, CONFIG_VERSION);
        assert!(config.api_token.is_none());
        assert!(!config.is_api_configured());
    }

    #[test]
    fn test_sec_user_agent() {
        let mut config = Config::default();
        assert!(config.sec_user_agent().is_none());

        config.sec_user_agent_name = Some("Test Company".to_string());
        config.sec_user_agent_email = Some("test@example.com".to_string());

        assert_eq!(
            config.sec_user_agent(),
            Some("Test Company test@example.com".to_string())
        );
    }

    #[test]
    fn test_serialization() {
        let config = Config {
            version: 1,
            api_token: Some("test_token".to_string()),
            sec_user_agent_name: Some("Test".to_string()),
            sec_user_agent_email: Some("test@test.com".to_string()),
        };

        let json = serde_json::to_string(&config).unwrap();
        let parsed: Config = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.api_token, config.api_token);
        assert_eq!(parsed.sec_user_agent_name, config.sec_user_agent_name);
    }
}
