//! FilingExplorer MCP Server
//!
//! A headless MCP server that communicates via stdio (stdin/stdout).
//! Spawned by Claude Desktop as a subprocess.
//!
//! Implements the Progressive Discovery pattern with 3 meta-tools:
//! - list_tool_categories
//! - search_tools
//! - execute_tool

use anyhow::Result;
use filing_explorer_core::{
    tools::{get_categories, search_tools, DetailLevel},
    ApiClient, Config,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

// ============================================================================
// JSON-RPC TYPES (MCP is JSON-RPC 2.0 over stdio)
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

impl JsonRpcResponse {
    fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: Option<Value>, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }
}

// ============================================================================
// MCP TOOL DEFINITIONS
// ============================================================================

fn build_tool_definitions() -> Value {
    json!([
        {
            "name": "list_tool_categories",
            "description": "List all available tool categories. Use this first to discover what capabilities are available.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "detail_level": {
                        "type": "string",
                        "enum": ["summary", "with_tool_names", "with_descriptions"],
                        "default": "with_descriptions",
                        "description": "Level of detail to return"
                    }
                }
            }
        },
        {
            "name": "search_tools",
            "description": "Search for tools by keyword. Returns matching tools with relevance scores.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search keyword (min 2 characters)"
                    },
                    "category": {
                        "type": "string",
                        "description": "Optional category filter"
                    },
                    "detail_level": {
                        "type": "string",
                        "enum": ["names_only", "with_descriptions", "full_schema"],
                        "default": "with_descriptions",
                        "description": "Level of detail to return"
                    }
                },
                "required": ["query"]
            }
        },
        {
            "name": "execute_tool",
            "description": "Execute a discovered tool by name with arguments.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tool_name": {
                        "type": "string",
                        "description": "Name of the tool to execute"
                    },
                    "arguments": {
                        "type": "object",
                        "description": "Arguments to pass to the tool"
                    }
                },
                "required": ["tool_name"]
            }
        }
    ])
}

// ============================================================================
// SERVER STATE
// ============================================================================

struct ServerState {
    #[allow(dead_code)]
    config: Config,
    api_client: Option<ApiClient>,
}

impl ServerState {
    fn new() -> Self {
        let config = Config::load_or_default();
        let api_client = config
            .api_token
            .as_ref()
            .and_then(|token| ApiClient::new(token).ok());

        Self { config, api_client }
    }

    fn ensure_api_client(&self) -> Result<&ApiClient, String> {
        self.api_client
            .as_ref()
            .ok_or_else(|| "API token not configured. Please run the settings app.".to_string())
    }
}

// ============================================================================
// MCP SERVER
// ============================================================================

struct McpServer {
    state: Arc<RwLock<ServerState>>,
}

impl McpServer {
    fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(ServerState::new())),
        }
    }

    async fn handle_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        match request.method.as_str() {
            "initialize" => self.handle_initialize(request.id, request.params).await,
            "initialized" => JsonRpcResponse::success(request.id, json!({})),
            "tools/list" => self.handle_list_tools(request.id).await,
            "tools/call" => self.handle_call_tool(request.id, request.params).await,
            "ping" => JsonRpcResponse::success(request.id, json!({})),
            _ => {
                warn!("Unknown method: {}", request.method);
                JsonRpcResponse::error(request.id, -32601, format!("Method not found: {}", request.method))
            }
        }
    }

    async fn handle_initialize(&self, id: Option<Value>, _params: Value) -> JsonRpcResponse {
        JsonRpcResponse::success(id, json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "filing-explorer",
                "version": env!("CARGO_PKG_VERSION")
            }
        }))
    }

    async fn handle_list_tools(&self, id: Option<Value>) -> JsonRpcResponse {
        JsonRpcResponse::success(id, json!({
            "tools": build_tool_definitions()
        }))
    }

    async fn handle_call_tool(&self, id: Option<Value>, params: Value) -> JsonRpcResponse {
        let name = match params.get("name").and_then(|v| v.as_str()) {
            Some(n) => n,
            None => return JsonRpcResponse::error(id, -32602, "Missing 'name' parameter"),
        };

        let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));

        match self.execute_tool(name, arguments).await {
            Ok(result) => JsonRpcResponse::success(id, json!({
                "content": [{
                    "type": "text",
                    "text": result
                }]
            })),
            Err(e) => JsonRpcResponse::success(id, json!({
                "content": [{
                    "type": "text",
                    "text": format!("Error: {}", e)
                }],
                "isError": true
            })),
        }
    }

    async fn execute_tool(&self, name: &str, args: Value) -> Result<String, String> {
        match name {
            "list_tool_categories" => self.handle_list_tool_categories(args).await,
            "search_tools" => self.handle_search_tools(args).await,
            "execute_tool" => self.handle_execute_tool(args).await,
            _ => Err(format!("Unknown tool: {}", name)),
        }
    }

    async fn handle_list_tool_categories(&self, args: Value) -> Result<String, String> {
        let detail_level = args
            .get("detail_level")
            .and_then(|v| v.as_str())
            .unwrap_or("with_descriptions")
            .parse::<DetailLevel>()
            .unwrap_or(DetailLevel::WithDescriptions);

        let result = get_categories(detail_level);
        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn handle_search_tools(&self, args: Value) -> Result<String, String> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: query")?;

        let category = args.get("category").and_then(|v| v.as_str());

        let detail_level = args
            .get("detail_level")
            .and_then(|v| v.as_str())
            .unwrap_or("with_descriptions")
            .parse::<DetailLevel>()
            .unwrap_or(DetailLevel::WithDescriptions);

        let result = search_tools(query, category, detail_level);
        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn handle_execute_tool(&self, args: Value) -> Result<String, String> {
        let tool_name = args
            .get("tool_name")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: tool_name")?;

        let tool_args = args.get("arguments").cloned().unwrap_or_else(|| json!({}));

        self.execute_actual_tool(tool_name, tool_args).await
    }

    async fn execute_actual_tool(&self, name: &str, args: Value) -> Result<String, String> {
        debug!("Executing tool: {} with args: {:?}", name, args);

        match name {
            // Company Data
            "get_company_financials" => self.get_company_financials(args).await,
            "get_company_calendar" => self.get_company_calendar(args).await,
            "get_company_filings" => self.get_company_filings(args).await,

            // Institutional Filings
            "get_form13f_submissions" => self.get_form13f_submissions(args).await,
            "get_form13f_submission" => self.get_form13f_submission(args).await,
            "get_form4_filing" => self.get_form4_filing(args).await,

            // ETF Data
            "get_etf_holdings" => self.get_etf_holdings(args).await,

            // Form ADV
            "get_form_adv_firms" => self.get_form_adv_firms(args).await,
            "get_form_adv_firm" => self.get_form_adv_firm(args).await,

            // Lobbying
            "get_lobbying_client_performance" => self.get_lobbying_client_performance(args).await,
            "get_lobbying_clients_search" => self.get_lobbying_clients_search(args).await,
            "get_lobbying_client_detail" => self.get_lobbying_client_detail(args).await,

            // Watchlists
            "get_lists" => self.get_lists().await,
            "create_list" => self.create_list(args).await,
            "get_list" => self.get_list(args).await,
            "update_list" => self.update_list(args).await,
            "delete_list" => self.delete_list(args).await,

            // Watchlist Items
            "add_list_item" => self.add_list_item(args).await,
            "toggle_list_item" => self.toggle_list_item(args).await,
            "update_list_item" => self.update_list_item(args).await,
            "delete_list_item" => self.delete_list_item(args).await,

            _ => {
                if filing_explorer_core::tools::registry::tool_exists(name) {
                    Err(format!("Tool '{}' exists but is not yet implemented", name))
                } else {
                    Err(format!("Unknown tool '{}'. Use search_tools to find available tools.", name))
                }
            }
        }
    }

    // =========================================================================
    // TOOL IMPLEMENTATIONS
    // =========================================================================

    async fn get_company_financials(&self, args: Value) -> Result<String, String> {
        let state = self.state.read().await;
        let client = state.ensure_api_client()?;

        let company_id = args
            .get("company_id")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: company_id")?;

        let mut params = std::collections::HashMap::new();
        if let Some(v) = args.get("period_of_report_date").and_then(|v| v.as_str()) {
            params.insert("period_of_report_date".to_string(), v.to_string());
        }
        if let Some(v) = args.get("timeframe").and_then(|v| v.as_str()) {
            params.insert("timeframe".to_string(), v.to_string());
        }
        if let Some(v) = args.get("limit").and_then(|v| v.as_i64()) {
            params.insert("limit".to_string(), v.to_string());
        }

        let endpoint = format!("companies/{}/financials", company_id);
        let result: Value = client
            .get(&endpoint, Some(params))
            .await
            .map_err(|e| e.to_string())?;

        let count = result.get("count").and_then(|v| v.as_i64()).unwrap_or(0);
        let summary = format!("Found {} financial statement(s) for {}\n\n", count, company_id);
        Ok(format!("{}{}", summary, serde_json::to_string_pretty(&result).unwrap()))
    }

    async fn get_company_calendar(&self, args: Value) -> Result<String, String> {
        let state = self.state.read().await;
        let client = state.ensure_api_client()?;

        let cik = args
            .get("company_cik")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: company_cik")?;

        let endpoint = format!("companies/{}/calendar", cik);
        let result: Value = client.get(&endpoint, None).await.map_err(|e| e.to_string())?;
        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn get_company_filings(&self, args: Value) -> Result<String, String> {
        let state = self.state.read().await;
        let client = state.ensure_api_client()?;

        let cik = args
            .get("cik")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: cik")?;

        let mut params = std::collections::HashMap::new();
        if let Some(v) = args.get("form_type").and_then(|v| v.as_str()) {
            params.insert("form_type".to_string(), v.to_string());
        }
        if let Some(v) = args.get("page_size").and_then(|v| v.as_i64()) {
            params.insert("page[size]".to_string(), v.to_string());
        }

        let endpoint = format!("companies/{}/filings", cik);
        let result: Value = client.get(&endpoint, Some(params)).await.map_err(|e| e.to_string())?;
        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn get_form13f_submissions(&self, args: Value) -> Result<String, String> {
        let state = self.state.read().await;
        let client = state.ensure_api_client()?;

        let mut params = std::collections::HashMap::new();
        if let Some(v) = args.get("search").and_then(|v| v.as_str()) {
            params.insert("search".to_string(), v.to_string());
        }
        if let Some(v) = args.get("limit").and_then(|v| v.as_i64()) {
            params.insert("limit".to_string(), v.to_string());
        }

        let result: Value = client.get("form13f/submissions", Some(params)).await.map_err(|e| e.to_string())?;
        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn get_form13f_submission(&self, args: Value) -> Result<String, String> {
        let state = self.state.read().await;
        let client = state.ensure_api_client()?;

        let filer_cik = args
            .get("filer_cik")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: filer_cik")?;

        let mut params = std::collections::HashMap::new();
        if let Some(v) = args.get("period_of_report").and_then(|v| v.as_str()) {
            params.insert("period_of_report".to_string(), v.to_string());
        }
        if let Some(v) = args.get("limit").and_then(|v| v.as_i64()) {
            params.insert("limit".to_string(), v.to_string());
        }

        let endpoint = format!("form13f/submissions/{}", filer_cik);
        let result: Value = client.get(&endpoint, Some(params)).await.map_err(|e| e.to_string())?;
        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn get_form4_filing(&self, args: Value) -> Result<String, String> {
        let state = self.state.read().await;
        let client = state.ensure_api_client()?;

        let accession = args
            .get("accession_number")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: accession_number")?;

        let endpoint = format!("form4/{}", accession);
        let result: Value = client.get(&endpoint, None).await.map_err(|e| e.to_string())?;
        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn get_etf_holdings(&self, args: Value) -> Result<String, String> {
        let state = self.state.read().await;
        let client = state.ensure_api_client()?;

        let identifier = args
            .get("identifier")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: identifier")?;

        let mut params = std::collections::HashMap::new();
        if let Some(v) = args.get("limit").and_then(|v| v.as_i64()) {
            params.insert("limit".to_string(), v.to_string());
        }

        let endpoint = format!("etf/{}/holdings", identifier);
        let result: Value = client.get(&endpoint, Some(params)).await.map_err(|e| e.to_string())?;
        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn get_form_adv_firms(&self, args: Value) -> Result<String, String> {
        let state = self.state.read().await;
        let client = state.ensure_api_client()?;

        let mut params = std::collections::HashMap::new();
        if let Some(v) = args.get("search").and_then(|v| v.as_str()) {
            params.insert("search".to_string(), v.to_string());
        }
        if let Some(v) = args.get("state").and_then(|v| v.as_str()) {
            params.insert("state".to_string(), v.to_string());
        }
        if let Some(v) = args.get("page_size").and_then(|v| v.as_i64()) {
            params.insert("page[size]".to_string(), v.to_string());
        }

        let result: Value = client.get("form-adv/firms", Some(params)).await.map_err(|e| e.to_string())?;
        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn get_form_adv_firm(&self, args: Value) -> Result<String, String> {
        let state = self.state.read().await;
        let client = state.ensure_api_client()?;

        let crd = args
            .get("crd")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: crd")?;

        let mut params = std::collections::HashMap::new();
        if let Some(v) = args.get("include").and_then(|v| v.as_str()) {
            params.insert("include".to_string(), v.to_string());
        }

        let endpoint = format!("form-adv/firms/{}", crd);
        let result: Value = client.get(&endpoint, Some(params)).await.map_err(|e| e.to_string())?;
        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn get_lobbying_client_performance(&self, args: Value) -> Result<String, String> {
        let state = self.state.read().await;
        let client = state.ensure_api_client()?;

        let mut params = std::collections::HashMap::new();
        if let Some(v) = args.get("year").and_then(|v| v.as_i64()) {
            params.insert("year".to_string(), v.to_string());
        }
        if let Some(v) = args.get("quarter").and_then(|v| v.as_str()) {
            params.insert("quarter".to_string(), v.to_string());
        }
        if let Some(v) = args.get("page").and_then(|v| v.as_i64()) {
            params.insert("page".to_string(), v.to_string());
        }

        let result: Value = client.get("lobbying/client-performance", Some(params)).await.map_err(|e| e.to_string())?;
        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn get_lobbying_clients_search(&self, args: Value) -> Result<String, String> {
        let state = self.state.read().await;
        let client = state.ensure_api_client()?;

        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: query")?;

        let mut params = std::collections::HashMap::new();
        params.insert("query".to_string(), query.to_string());
        if let Some(v) = args.get("limit").and_then(|v| v.as_i64()) {
            params.insert("limit".to_string(), v.to_string());
        }

        let result: Value = client.get("lobbying/clients/search", Some(params)).await.map_err(|e| e.to_string())?;
        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn get_lobbying_client_detail(&self, args: Value) -> Result<String, String> {
        let state = self.state.read().await;
        let client = state.ensure_api_client()?;

        let client_id = args
            .get("client_id")
            .and_then(|v| v.as_i64())
            .ok_or("Missing required parameter: client_id")?;

        let endpoint = format!("lobbying/clients/{}", client_id);
        let result: Value = client.get(&endpoint, None).await.map_err(|e| e.to_string())?;
        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn get_lists(&self) -> Result<String, String> {
        let state = self.state.read().await;
        let client = state.ensure_api_client()?;
        let result: Value = client.get("lists", None).await.map_err(|e| e.to_string())?;
        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn create_list(&self, args: Value) -> Result<String, String> {
        let state = self.state.read().await;
        let client = state.ensure_api_client()?;
        let result: Value = client.post("lists", Some(&args)).await.map_err(|e| e.to_string())?;
        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn get_list(&self, args: Value) -> Result<String, String> {
        let state = self.state.read().await;
        let client = state.ensure_api_client()?;

        let id_or_name = args
            .get("id_or_name")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: id_or_name")?;

        let endpoint = format!("lists/{}", id_or_name);
        let result: Value = client.get(&endpoint, None).await.map_err(|e| e.to_string())?;
        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn update_list(&self, args: Value) -> Result<String, String> {
        let state = self.state.read().await;
        let client = state.ensure_api_client()?;

        let id_or_name = args
            .get("id_or_name")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: id_or_name")?;

        let body = json!({
            "name": args.get("name"),
            "notes": args.get("notes")
        });

        let endpoint = format!("lists/{}", id_or_name);
        let result: Value = client.patch(&endpoint, Some(&body)).await.map_err(|e| e.to_string())?;
        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn delete_list(&self, args: Value) -> Result<String, String> {
        let state = self.state.read().await;
        let client = state.ensure_api_client()?;

        let id_or_name = args
            .get("id_or_name")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: id_or_name")?;

        let endpoint = format!("lists/{}", id_or_name);
        client.delete(&endpoint).await.map_err(|e| e.to_string())?;
        Ok(json!({"success": true, "message": "List deleted"}).to_string())
    }

    async fn add_list_item(&self, args: Value) -> Result<String, String> {
        let state = self.state.read().await;
        let client = state.ensure_api_client()?;

        let list_id = args
            .get("list_id")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: list_id")?;

        let endpoint = format!("lists/{}/items", list_id);
        let result: Value = client.post(&endpoint, Some(&args)).await.map_err(|e| e.to_string())?;
        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn toggle_list_item(&self, args: Value) -> Result<String, String> {
        let state = self.state.read().await;
        let client = state.ensure_api_client()?;

        let list_id = args
            .get("list_id")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: list_id")?;

        let endpoint = format!("lists/{}/items/toggle", list_id);
        let result: Value = client.post(&endpoint, Some(&args)).await.map_err(|e| e.to_string())?;
        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn update_list_item(&self, args: Value) -> Result<String, String> {
        let state = self.state.read().await;
        let client = state.ensure_api_client()?;

        let list_id = args
            .get("list_id")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: list_id")?;
        let item_id = args
            .get("item_id")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: item_id")?;

        let body = json!({ "notes": args.get("notes") });
        let endpoint = format!("lists/{}/items/{}", list_id, item_id);
        let result: Value = client.patch(&endpoint, Some(&body)).await.map_err(|e| e.to_string())?;
        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn delete_list_item(&self, args: Value) -> Result<String, String> {
        let state = self.state.read().await;
        let client = state.ensure_api_client()?;

        let list_id = args
            .get("list_id")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: list_id")?;
        let item_id = args
            .get("item_id")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: item_id")?;

        let endpoint = format!("lists/{}/items/{}", list_id, item_id);
        client.delete(&endpoint).await.map_err(|e| e.to_string())?;
        Ok(json!({"success": true, "message": "Item deleted"}).to_string())
    }
}

// ============================================================================
// MAIN
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging to stderr (stdout is for MCP protocol)
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .with_writer(std::io::stderr)
        .init();

    info!("Starting FilingExplorer MCP Server");

    let server = McpServer::new();
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    info!("MCP Server ready. Listening on stdio...");

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                error!("Error reading stdin: {}", e);
                continue;
            }
        };

        if line.is_empty() {
            continue;
        }

        debug!("Received: {}", line);

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                error!("Invalid JSON-RPC request: {}", e);
                // For parse errors, we can't know the id, so use Value::Null
                // but we still need to respond
                let response = JsonRpcResponse::error(Some(Value::Null), -32700, "Parse error");
                let output = serde_json::to_string(&response).unwrap();
                writeln!(stdout, "{}", output)?;
                stdout.flush()?;
                continue;
            }
        };

        // JSON-RPC 2.0: Notifications (requests without id) should not receive a response
        let is_notification = request.id.is_none();

        // Handle notifications silently (no response)
        if is_notification {
            debug!("Received notification: {}", request.method);
            // Process known notifications
            match request.method.as_str() {
                "notifications/initialized" | "initialized" => {
                    debug!("Client initialized");
                }
                "notifications/cancelled" => {
                    debug!("Request cancelled");
                }
                _ => {
                    debug!("Unknown notification: {}", request.method);
                }
            }
            continue;
        }

        let response = server.handle_request(request).await;
        let output = serde_json::to_string(&response).unwrap();

        debug!("Sending: {}", output);
        writeln!(stdout, "{}", output)?;
        stdout.flush()?;
    }

    info!("Shutting down");
    Ok(())
}
