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
    use tempfile::TempDir;

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

    #[test]
    fn test_is_api_configured() {
        let mut config = Config::default();

        // None token
        assert!(!config.is_api_configured());

        // Empty token
        config.api_token = Some("".to_string());
        assert!(!config.is_api_configured());

        // Whitespace token
        config.api_token = Some("   ".to_string());
        assert!(config.is_api_configured()); // Note: doesn't trim whitespace

        // Valid token
        config.api_token = Some("valid_token".to_string());
        assert!(config.is_api_configured());
    }

    #[test]
    fn test_is_sec_configured() {
        let mut config = Config::default();

        // Nothing configured
        assert!(!config.is_sec_configured());

        // Only name
        config.sec_user_agent_name = Some("Company".to_string());
        assert!(!config.is_sec_configured());

        // Only email
        config.sec_user_agent_name = None;
        config.sec_user_agent_email = Some("test@example.com".to_string());
        assert!(!config.is_sec_configured());

        // Both configured but name empty
        config.sec_user_agent_name = Some("".to_string());
        config.sec_user_agent_email = Some("test@example.com".to_string());
        assert!(!config.is_sec_configured());

        // Both configured but email empty
        config.sec_user_agent_name = Some("Company".to_string());
        config.sec_user_agent_email = Some("".to_string());
        assert!(!config.is_sec_configured());

        // Both properly configured
        config.sec_user_agent_name = Some("Company".to_string());
        config.sec_user_agent_email = Some("test@example.com".to_string());
        assert!(config.is_sec_configured());
    }

    #[test]
    fn test_require_api_token() {
        let mut config = Config::default();

        // No token
        assert!(matches!(
            config.require_api_token(),
            Err(ConfigError::MissingToken)
        ));

        // Empty token
        config.api_token = Some("".to_string());
        assert!(matches!(
            config.require_api_token(),
            Err(ConfigError::MissingToken)
        ));

        // Valid token
        config.api_token = Some("my_token".to_string());
        assert_eq!(config.require_api_token().unwrap(), "my_token");
    }

    #[test]
    fn test_sec_user_agent_partial_config() {
        let mut config = Config::default();

        // Only name set
        config.sec_user_agent_name = Some("Company".to_string());
        config.sec_user_agent_email = None;
        assert!(config.sec_user_agent().is_none());

        // Only email set
        config.sec_user_agent_name = None;
        config.sec_user_agent_email = Some("test@test.com".to_string());
        assert!(config.sec_user_agent().is_none());

        // Name empty
        config.sec_user_agent_name = Some("".to_string());
        config.sec_user_agent_email = Some("test@test.com".to_string());
        assert!(config.sec_user_agent().is_none());

        // Email empty
        config.sec_user_agent_name = Some("Company".to_string());
        config.sec_user_agent_email = Some("".to_string());
        assert!(config.sec_user_agent().is_none());
    }

    #[test]
    fn test_deserialization_with_defaults() {
        // Missing optional fields should use defaults
        let json = r#"{"version": 1}"#;
        let config: Config = serde_json::from_str(json).unwrap();

        assert_eq!(config.version, 1);
        assert!(config.api_token.is_none());
        assert!(config.sec_user_agent_name.is_none());
        assert!(config.sec_user_agent_email.is_none());
    }

    #[test]
    fn test_deserialization_missing_version() {
        // Missing version should use default
        let json = r#"{"api_token": "test"}"#;
        let config: Config = serde_json::from_str(json).unwrap();

        assert_eq!(config.version, CONFIG_VERSION);
        assert_eq!(config.api_token, Some("test".to_string()));
    }

    #[test]
    fn test_config_dir_returns_path() {
        // This should work on any platform
        let result = Config::config_dir();
        assert!(result.is_ok());

        let path = result.unwrap();
        assert!(path.to_string_lossy().len() > 0);
    }

    #[test]
    fn test_config_path_returns_json_file() {
        let result = Config::config_path();
        assert!(result.is_ok());

        let path = result.unwrap();
        assert!(path.to_string_lossy().ends_with("config.json"));
    }

    #[test]
    fn test_load_or_default_returns_default_when_not_found() {
        // load_or_default should return default config, not panic
        // Note: This test works because we're not actually modifying real config
        let config = Config::load_or_default();
        assert_eq!(config.version, CONFIG_VERSION);
    }

    #[test]
    fn test_config_clone() {
        let config = Config {
            version: 1,
            api_token: Some("token".to_string()),
            sec_user_agent_name: Some("Name".to_string()),
            sec_user_agent_email: Some("email@test.com".to_string()),
        };

        let cloned = config.clone();
        assert_eq!(cloned.version, config.version);
        assert_eq!(cloned.api_token, config.api_token);
        assert_eq!(cloned.sec_user_agent_name, config.sec_user_agent_name);
        assert_eq!(cloned.sec_user_agent_email, config.sec_user_agent_email);
    }

    #[test]
    fn test_config_debug() {
        let config = Config::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("Config"));
        assert!(debug_str.contains("version"));
    }

    #[test]
    fn test_config_error_display() {
        let err = ConfigError::NoConfigDir;
        assert_eq!(
            format!("{}", err),
            "Could not determine config directory for this platform"
        );

        let err = ConfigError::NotFound;
        assert_eq!(
            format!("{}", err),
            "Config file not found. Please run the settings app to configure."
        );

        let err = ConfigError::MissingToken;
        assert_eq!(format!("{}", err), "API token not configured");
    }

    // File I/O tests using tempfile
    mod file_io {
        use super::*;

        /// Helper to create a config with a custom path for testing
        fn save_config_to_path(config: &Config, path: &std::path::Path) -> Result<(), ConfigError> {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let contents = serde_json::to_string_pretty(config)?;
            fs::write(path, contents)?;
            Ok(())
        }

        fn load_config_from_path(path: &std::path::Path) -> Result<Config, ConfigError> {
            if !path.exists() {
                return Err(ConfigError::NotFound);
            }
            let contents = fs::read_to_string(path)?;
            let config: Config = serde_json::from_str(&contents)?;
            Ok(config)
        }

        #[test]
        fn test_save_and_load_config() {
            let temp_dir = TempDir::new().unwrap();
            let config_path = temp_dir.path().join("config.json");

            let config = Config {
                version: 1,
                api_token: Some("test_token_123".to_string()),
                sec_user_agent_name: Some("Test Company".to_string()),
                sec_user_agent_email: Some("test@example.com".to_string()),
            };

            // Save
            save_config_to_path(&config, &config_path).unwrap();
            assert!(config_path.exists());

            // Load
            let loaded = load_config_from_path(&config_path).unwrap();
            assert_eq!(loaded.version, config.version);
            assert_eq!(loaded.api_token, config.api_token);
            assert_eq!(loaded.sec_user_agent_name, config.sec_user_agent_name);
            assert_eq!(loaded.sec_user_agent_email, config.sec_user_agent_email);
        }

        #[test]
        fn test_load_nonexistent_file() {
            let temp_dir = TempDir::new().unwrap();
            let config_path = temp_dir.path().join("nonexistent.json");

            let result = load_config_from_path(&config_path);
            assert!(matches!(result, Err(ConfigError::NotFound)));
        }

        #[test]
        fn test_load_invalid_json() {
            let temp_dir = TempDir::new().unwrap();
            let config_path = temp_dir.path().join("config.json");

            // Write invalid JSON
            fs::write(&config_path, "this is not valid json").unwrap();

            let result = load_config_from_path(&config_path);
            assert!(matches!(result, Err(ConfigError::ParseError(_))));
        }

        #[test]
        fn test_save_creates_parent_directories() {
            let temp_dir = TempDir::new().unwrap();
            let config_path = temp_dir.path().join("nested").join("dir").join("config.json");

            let config = Config::default();
            save_config_to_path(&config, &config_path).unwrap();

            assert!(config_path.exists());
        }

        #[test]
        fn test_save_overwrites_existing_file() {
            let temp_dir = TempDir::new().unwrap();
            let config_path = temp_dir.path().join("config.json");

            // Save first config
            let config1 = Config {
                version: 1,
                api_token: Some("first_token".to_string()),
                sec_user_agent_name: None,
                sec_user_agent_email: None,
            };
            save_config_to_path(&config1, &config_path).unwrap();

            // Save second config (overwrite)
            let config2 = Config {
                version: 1,
                api_token: Some("second_token".to_string()),
                sec_user_agent_name: Some("New Company".to_string()),
                sec_user_agent_email: Some("new@example.com".to_string()),
            };
            save_config_to_path(&config2, &config_path).unwrap();

            // Load and verify it's the second config
            let loaded = load_config_from_path(&config_path).unwrap();
            assert_eq!(loaded.api_token, Some("second_token".to_string()));
            assert_eq!(loaded.sec_user_agent_name, Some("New Company".to_string()));
        }

        #[test]
        fn test_load_partial_config() {
            let temp_dir = TempDir::new().unwrap();
            let config_path = temp_dir.path().join("config.json");

            // Write partial config (only api_token)
            let partial_json = r#"{"api_token": "partial_token"}"#;
            fs::write(&config_path, partial_json).unwrap();

            let loaded = load_config_from_path(&config_path).unwrap();
            assert_eq!(loaded.api_token, Some("partial_token".to_string()));
            assert_eq!(loaded.version, CONFIG_VERSION); // Should use default
            assert!(loaded.sec_user_agent_name.is_none());
            assert!(loaded.sec_user_agent_email.is_none());
        }

        #[test]
        fn test_config_pretty_printed() {
            let temp_dir = TempDir::new().unwrap();
            let config_path = temp_dir.path().join("config.json");

            let config = Config {
                version: 1,
                api_token: Some("token".to_string()),
                sec_user_agent_name: None,
                sec_user_agent_email: None,
            };
            save_config_to_path(&config, &config_path).unwrap();

            // Read raw content and verify it's formatted
            let contents = fs::read_to_string(&config_path).unwrap();
            assert!(contents.contains('\n')); // Pretty printed should have newlines
            assert!(contents.contains("  ")); // And indentation
        }
    }
}
