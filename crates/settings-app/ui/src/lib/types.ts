export interface ConfigResponse {
  api_token: string | null;
  sec_user_agent_name: string | null;
  sec_user_agent_email: string | null;
}

export interface ValidationResponse {
  success: boolean;
  message: string;
}

export interface StatusResponse {
  claude_desktop_configured: boolean;
  claude_desktop_config_path: string | null;
  claude_code_configured: boolean;
  claude_code_config_path: string | null;
  mcp_server_path: string | null;
  mcp_server_exists: boolean;
  api_token_set: boolean;
  sec_email_set: boolean;
}

export interface ClaudeConfigInfo {
  config_type: string;
  label: string;
  path: string;
  exists: boolean;
  mcp_installed: boolean;
  mcp_server_path: string | null;
  mcp_server_valid: boolean;
}

export interface ToolCategoryInfo {
  id: string;
  name: string;
  description: string;
  tool_count: number;
  tools: ToolInfo[];
}

export interface ToolInfo {
  name: string;
  description: string;
}

export type Tab = 'basic' | 'advanced' | 'tools';

export type TokenValidationStatus = 'idle' | 'validating' | 'valid' | 'invalid' | 'error';
