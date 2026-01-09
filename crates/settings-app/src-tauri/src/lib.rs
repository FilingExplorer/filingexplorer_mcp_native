//! FilingExplorer Settings App
//!
//! Tauri commands for managing configuration.

use filing_explorer_core::config::Config;
use serde::{Deserialize, Serialize};
use tauri::Manager;

/// Response for config operations
#[derive(Serialize, Deserialize)]
pub struct ConfigResponse {
    pub api_token: Option<String>,
    pub sec_user_agent_name: Option<String>,
    pub sec_user_agent_email: Option<String>,
}

/// Response for validation operations
#[derive(Serialize, Deserialize)]
pub struct ValidationResponse {
    pub success: bool,
    pub message: String,
}

/// Status check response
#[derive(Serialize, Deserialize)]
pub struct StatusResponse {
    pub claude_configured: bool,
    pub claude_config_path: Option<String>,
    pub mcp_server_path: Option<String>,
    pub mcp_server_exists: bool,
    pub api_token_set: bool,
    pub sec_email_set: bool,
}

/// Load the current configuration
#[tauri::command]
async fn load_config() -> Result<ConfigResponse, String> {
    let config = Config::load().map_err(|e| e.to_string())?;
    Ok(ConfigResponse {
        api_token: config.api_token,
        sec_user_agent_name: config.sec_user_agent_name,
        sec_user_agent_email: config.sec_user_agent_email,
    })
}

/// Save configuration
#[tauri::command]
async fn save_config(
    api_token: Option<String>,
    sec_user_agent_name: Option<String>,
    sec_user_agent_email: Option<String>,
) -> Result<(), String> {
    let mut config = Config::load().unwrap_or_default();
    config.api_token = api_token;
    config.sec_user_agent_name = sec_user_agent_name;
    config.sec_user_agent_email = sec_user_agent_email;
    config.save().map_err(|e| e.to_string())
}

/// Validate the API token by making a test request
#[tauri::command]
async fn validate_token(api_token: String) -> Result<ValidationResponse, String> {
    let client = reqwest::Client::new();

    let response = client
        .get("https://api.filingexplorer.com/v1/watchlists")
        .header("Authorization", format!("Bearer {}", api_token))
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if response.status().is_success() {
        Ok(ValidationResponse {
            success: true,
            message: "API token is valid".to_string(),
        })
    } else if response.status() == 401 {
        Ok(ValidationResponse {
            success: false,
            message: "Invalid API token".to_string(),
        })
    } else {
        Ok(ValidationResponse {
            success: false,
            message: format!("Unexpected response: {}", response.status()),
        })
    }
}

/// Check the current setup status
#[tauri::command]
async fn check_status() -> Result<StatusResponse, String> {
    // Check config
    let config = Config::load().unwrap_or_default();
    let api_token_set = config.api_token.as_ref().map_or(false, |t| !t.is_empty());
    let sec_email_set = config.sec_user_agent_email.as_ref().map_or(false, |e| !e.is_empty());

    // Check Claude Desktop config
    let claude_config_path = get_claude_config_path();
    let (claude_configured, mcp_server_path, mcp_server_exists) = if let Some(ref path) = claude_config_path {
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(path) {
                if let Ok(config) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(cmd) = config
                        .get("mcpServers")
                        .and_then(|s| s.get("filing-explorer"))
                        .and_then(|s| s.get("command"))
                        .and_then(|c| c.as_str())
                    {
                        let path = std::path::PathBuf::from(cmd);
                        let exists = path.exists();
                        (true, Some(cmd.to_string()), exists)
                    } else {
                        (false, None, false)
                    }
                } else {
                    (false, None, false)
                }
            } else {
                (false, None, false)
            }
        } else {
            (false, None, false)
        }
    } else {
        (false, None, false)
    };

    Ok(StatusResponse {
        claude_configured,
        claude_config_path: claude_config_path.map(|p| p.to_string_lossy().to_string()),
        mcp_server_path,
        mcp_server_exists,
        api_token_set,
        sec_email_set,
    })
}

/// Get the path to Claude Desktop config file
fn get_claude_config_path() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir().map(|h| {
            h.join("Library/Application Support/Claude/claude_desktop_config.json")
        })
    }
    #[cfg(target_os = "windows")]
    {
        dirs::config_dir().map(|c| c.join("Claude/claude_desktop_config.json"))
    }
    #[cfg(target_os = "linux")]
    {
        dirs::config_dir().map(|c| c.join("Claude/claude_desktop_config.json"))
    }
}

/// Configure Claude Desktop to use the MCP server
#[tauri::command]
async fn configure_claude_desktop() -> Result<ValidationResponse, String> {
    let config_path = get_claude_config_path()
        .ok_or_else(|| "Could not determine Claude Desktop config path".to_string())?;

    // Read existing config or create new one
    let mut config: serde_json::Value = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    // Get the path to our MCP server binary
    // The binary is bundled as an external binary (sidecar) in the app bundle
    let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_dir = current_exe
        .parent()
        .ok_or_else(|| "Could not get parent directory".to_string())?;

    // Tauri bundles external binaries with target triple suffix
    let target_triple = if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "aarch64-apple-darwin"
        } else {
            "x86_64-apple-darwin"
        }
    } else if cfg!(target_os = "windows") {
        if cfg!(target_arch = "aarch64") {
            "aarch64-pc-windows-msvc"
        } else {
            "x86_64-pc-windows-msvc"
        }
    } else {
        // Linux
        if cfg!(target_arch = "aarch64") {
            "aarch64-unknown-linux-gnu"
        } else {
            "x86_64-unknown-linux-gnu"
        }
    };

    // Try bundled sidecar first (with target triple suffix)
    let sidecar_name = format!("mcp-server-{}", target_triple);
    let mcp_server_path = if exe_dir.join(&sidecar_name).exists() {
        exe_dir.join(&sidecar_name)
    } else if exe_dir.join("mcp-server").exists() {
        // Fallback: no suffix
        exe_dir.join("mcp-server")
    } else {
        // Development fallback: look in target directory
        let mut search_dir = exe_dir.to_path_buf();
        let mut found_path = None;

        for _ in 0..10 {
            if let Some(parent) = search_dir.parent() {
                search_dir = parent.to_path_buf();
            } else {
                break;
            }

            let release_path = search_dir.join("release/mcp-server");
            if release_path.exists() {
                found_path = Some(release_path);
                break;
            }
            let debug_path = search_dir.join("debug/mcp-server");
            if debug_path.exists() {
                found_path = Some(debug_path);
                break;
            }
        }

        found_path.ok_or_else(|| {
            "Could not find mcp-server binary. The app bundle may be corrupted.".to_string()
        })?
    };

    // Ensure mcpServers object exists
    if !config.get("mcpServers").is_some() {
        config["mcpServers"] = serde_json::json!({});
    }

    // Add our server config
    config["mcpServers"]["filing-explorer"] = serde_json::json!({
        "command": mcp_server_path.to_string_lossy(),
        "args": []
    });

    // Create parent directories if needed
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    // Write the config
    let content = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    std::fs::write(&config_path, content).map_err(|e| e.to_string())?;

    Ok(ValidationResponse {
        success: true,
        message: format!(
            "Claude Desktop configured. Restart Claude Desktop to apply changes.\nConfig path: {}",
            config_path.display()
        ),
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            load_config,
            save_config,
            validate_token,
            configure_claude_desktop,
            check_status,
        ])
        .setup(|app| {
            #[cfg(debug_assertions)]
            {
                let window = app.get_webview_window("main").unwrap();
                window.open_devtools();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
