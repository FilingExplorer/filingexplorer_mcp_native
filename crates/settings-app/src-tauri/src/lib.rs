//! FilingExplorer Settings App
//!
//! Tauri commands for managing configuration.

use filing_explorer_core::config::Config;
use filing_explorer_core::tools::registry::{self, DetailLevel};
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
    pub claude_desktop_configured: bool,
    pub claude_desktop_config_path: Option<String>,
    pub claude_code_configured: bool,
    pub claude_code_config_path: Option<String>,
    pub mcp_server_path: Option<String>,
    pub mcp_server_exists: bool,
    pub api_token_set: bool,
    pub sec_email_set: bool,
}

/// Info about a single Claude config location
#[derive(Serialize, Deserialize, Clone)]
pub struct ClaudeConfigInfo {
    pub config_type: String,
    pub label: String,
    pub path: String,
    pub exists: bool,
    pub mcp_installed: bool,
    pub mcp_server_path: Option<String>,
    pub mcp_server_valid: bool,
}

/// Tool category info for the UI
#[derive(Serialize, Deserialize)]
pub struct ToolCategoryInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tool_count: u64,
    pub tools: Vec<ToolInfo>,
}

/// Individual tool info for the UI
#[derive(Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
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
        .get("https://api.filingexplorer.com/v1/lists")
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
    let sec_email_set = config
        .sec_user_agent_email
        .as_ref()
        .map_or(false, |e| !e.is_empty());

    // Check Claude Desktop config
    let claude_desktop_config_path = get_claude_desktop_config_path();
    let (claude_desktop_configured, mcp_server_path, mcp_server_exists) =
        if let Some(ref path) = claude_desktop_config_path {
            check_mcp_in_config(path)
        } else {
            (false, None, false)
        };

    // Check Claude Code config
    let claude_code_config_path = get_claude_code_config_path();
    let claude_code_configured = if let Some(ref path) = claude_code_config_path {
        check_mcp_in_config(path).0
    } else {
        false
    };

    Ok(StatusResponse {
        claude_desktop_configured,
        claude_desktop_config_path: claude_desktop_config_path
            .map(|p| p.to_string_lossy().to_string()),
        claude_code_configured,
        claude_code_config_path: claude_code_config_path
            .map(|p| p.to_string_lossy().to_string()),
        mcp_server_path,
        mcp_server_exists,
        api_token_set,
        sec_email_set,
    })
}

/// Get all Claude config locations with their current status
#[tauri::command]
async fn get_all_claude_configs() -> Result<Vec<ClaudeConfigInfo>, String> {
    let mcp_server_path = find_mcp_server_path().ok();
    let mcp_server_valid = mcp_server_path
        .as_ref()
        .map_or(false, |p| p.exists());

    let mut configs = Vec::new();

    // Claude Desktop
    if let Some(path) = get_claude_desktop_config_path() {
        let path_str = path.to_string_lossy().to_string();
        let exists = path.exists();
        let (mcp_installed, server_path, _) = if exists {
            check_mcp_in_config(&path)
        } else {
            (false, None, false)
        };

        configs.push(ClaudeConfigInfo {
            config_type: "desktop".to_string(),
            label: "Claude Desktop".to_string(),
            path: path_str,
            exists,
            mcp_installed,
            mcp_server_path: server_path.or_else(|| {
                mcp_server_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string())
            }),
            mcp_server_valid,
        });
    }

    // Claude Code (global)
    if let Some(path) = get_claude_code_config_path() {
        let path_str = path.to_string_lossy().to_string();
        let exists = path.exists();
        let (mcp_installed, server_path, _) = if exists {
            check_mcp_in_config(&path)
        } else {
            (false, None, false)
        };

        configs.push(ClaudeConfigInfo {
            config_type: "code_global".to_string(),
            label: "Claude Code (Global)".to_string(),
            path: path_str,
            exists,
            mcp_installed,
            mcp_server_path: server_path.or_else(|| {
                mcp_server_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string())
            }),
            mcp_server_valid,
        });
    }

    Ok(configs)
}

/// Install MCP server config to a specific Claude config file
#[tauri::command]
async fn install_mcp_to_config(
    config_type: String,
) -> Result<ValidationResponse, String> {
    match config_type.as_str() {
        "desktop" => configure_claude_desktop().await,
        "code_global" => configure_claude_code().await,
        _ => Err(format!("Unknown config type: {}", config_type)),
    }
}

/// Get the MCP config JSON snippet for manual installation / clipboard
#[tauri::command]
async fn get_mcp_config_snippet(config_type: String) -> Result<String, String> {
    let mcp_server_path = find_mcp_server_path()?;
    let path_str = mcp_server_path.to_string_lossy().to_string();

    let snippet = match config_type.as_str() {
        "desktop" => {
            serde_json::to_string_pretty(&serde_json::json!({
                "filing-explorer": {
                    "command": path_str,
                    "args": []
                }
            }))
            .map_err(|e| e.to_string())?
        }
        "code_global" => {
            serde_json::to_string_pretty(&serde_json::json!({
                "filing-explorer": {
                    "type": "stdio",
                    "command": path_str,
                    "args": []
                }
            }))
            .map_err(|e| e.to_string())?
        }
        _ => return Err(format!("Unknown config type: {}", config_type)),
    };

    Ok(snippet)
}

/// Get tool categories with their tools for the documentation tab
#[tauri::command]
async fn get_tool_categories() -> Result<Vec<ToolCategoryInfo>, String> {
    let categories_json = registry::get_categories(DetailLevel::WithDescriptions);

    let cats = categories_json["categories"]
        .as_array()
        .ok_or("Failed to parse categories")?;

    let mut result = Vec::new();

    for cat in cats {
        let id = cat["id"].as_str().unwrap_or("").to_string();
        let name = cat["name"].as_str().unwrap_or("").to_string();
        let description = cat
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("")
            .to_string();
        let tool_count = cat["tool_count"].as_u64().unwrap_or(0);

        // Get tools for this category
        let tools_json = registry::list_tools_by_category(&id, DetailLevel::WithDescriptions);
        let tools_arr = tools_json["tools"].as_array();

        let tools = tools_arr
            .map(|arr| {
                arr.iter()
                    .map(|t| ToolInfo {
                        name: t["name"].as_str().unwrap_or("").to_string(),
                        description: t
                            .get("description")
                            .and_then(|d| d.as_str())
                            .unwrap_or("")
                            .to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        result.push(ToolCategoryInfo {
            id,
            name,
            description,
            tool_count,
            tools,
        });
    }

    Ok(result)
}

// ============================================================================
// Helper functions
// ============================================================================

/// Check if a config file has MCP server configured, return (installed, server_path, server_exists)
fn check_mcp_in_config(
    path: &std::path::Path,
) -> (bool, Option<String>, bool) {
    if !path.exists() {
        return (false, None, false);
    }

    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return (false, None, false),
    };

    let config: serde_json::Value = match serde_json::from_str(&content) {
        Ok(c) => c,
        Err(_) => return (false, None, false),
    };

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
}

/// Get the path to Claude Desktop config file
fn get_claude_desktop_config_path() -> Option<std::path::PathBuf> {
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

/// Get the path to Claude Code config file (~/.claude.json)
/// See: https://code.claude.com/docs/en/mcp#mcp-installation-scopes
fn get_claude_code_config_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude.json"))
}

/// Find the MCP server binary path
fn find_mcp_server_path() -> Result<std::path::PathBuf, String> {
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
    if exe_dir.join(&sidecar_name).exists() {
        return Ok(exe_dir.join(&sidecar_name));
    }

    if exe_dir.join("mcp-server").exists() {
        // Fallback: no suffix
        return Ok(exe_dir.join("mcp-server"));
    }

    // Development fallback: look in target directory
    let mut search_dir = exe_dir.to_path_buf();

    for _ in 0..10 {
        if let Some(parent) = search_dir.parent() {
            search_dir = parent.to_path_buf();
        } else {
            break;
        }

        let release_path = search_dir.join("release/mcp-server");
        if release_path.exists() {
            return Ok(release_path);
        }
        let debug_path = search_dir.join("debug/mcp-server");
        if debug_path.exists() {
            return Ok(debug_path);
        }
    }

    Err("Could not find mcp-server binary. The app bundle may be corrupted.".to_string())
}

/// Configure Claude Desktop to use the MCP server
#[tauri::command]
async fn configure_claude_desktop() -> Result<ValidationResponse, String> {
    let config_path = get_claude_desktop_config_path()
        .ok_or_else(|| "Could not determine Claude Desktop config path".to_string())?;

    // Read existing config or create new one
    let mut config: serde_json::Value = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    let mcp_server_path = find_mcp_server_path()?;

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

/// Configure Claude Code to use the MCP server
/// Writes to ~/.claude.json per https://code.claude.com/docs/en/mcp#mcp-installation-scopes
#[tauri::command]
async fn configure_claude_code() -> Result<ValidationResponse, String> {
    let config_path = get_claude_code_config_path()
        .ok_or_else(|| "Could not determine Claude Code config path".to_string())?;

    // Read existing config or create new one
    let mut config: serde_json::Value = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    let mcp_server_path = find_mcp_server_path()?;

    // Ensure mcpServers object exists
    if !config.get("mcpServers").is_some() {
        config["mcpServers"] = serde_json::json!({});
    }

    // Add our server config with type field for Claude Code
    config["mcpServers"]["filing-explorer"] = serde_json::json!({
        "type": "stdio",
        "command": mcp_server_path.to_string_lossy(),
        "args": []
    });

    // Write the config
    let content = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    std::fs::write(&config_path, content).map_err(|e| e.to_string())?;

    Ok(ValidationResponse {
        success: true,
        message: format!(
            "Claude Code configured. The MCP server will be available in new Claude Code sessions.\nConfig path: {}",
            config_path.display()
        ),
    })
}

/// Configure both Claude Desktop and Claude Code at once
#[tauri::command]
async fn configure_both() -> Result<ValidationResponse, String> {
    let desktop_result = configure_claude_desktop().await;
    let code_result = configure_claude_code().await;

    match (desktop_result, code_result) {
        (Ok(_), Ok(_)) => Ok(ValidationResponse {
            success: true,
            message: "Both Claude Desktop and Claude Code configured. Restart Claude Desktop and start new Claude Code sessions to apply changes.".to_string(),
        }),
        (Err(e1), Err(e2)) => Err(format!("Both configurations failed:\nDesktop: {}\nCode: {}", e1, e2)),
        (Err(e), Ok(_)) => Ok(ValidationResponse {
            success: true,
            message: format!("Claude Code configured, but Claude Desktop failed: {}", e),
        }),
        (Ok(_), Err(e)) => Ok(ValidationResponse {
            success: true,
            message: format!("Claude Desktop configured, but Claude Code failed: {}", e),
        }),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::default().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![
            load_config,
            save_config,
            validate_token,
            configure_claude_desktop,
            configure_claude_code,
            configure_both,
            check_status,
            get_all_claude_configs,
            install_mcp_to_config,
            get_mcp_config_snippet,
            get_tool_categories,
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
