<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';

  interface ConfigResponse {
    api_token: string | null;
    sec_user_agent_name: string | null;
    sec_user_agent_email: string | null;
  }

  interface ValidationResponse {
    success: boolean;
    message: string;
  }

  interface StatusResponse {
    claude_configured: boolean;
    claude_config_path: string | null;
    mcp_server_path: string | null;
    mcp_server_exists: boolean;
    api_token_set: boolean;
    sec_email_set: boolean;
  }

  let apiToken = '';
  let secEmail = '';
  let secName = '';
  let statusMessage = '';
  let statusType: 'success' | 'error' | 'info' | '' = '';
  let isLoading = false;
  let isValidating = false;

  // Status tracking
  let status: StatusResponse | null = null;

  onMount(async () => {
    await Promise.all([loadConfig(), checkStatus()]);
  });

  async function loadConfig() {
    try {
      const config: ConfigResponse = await invoke('load_config');
      apiToken = config.api_token || '';
      secName = config.sec_user_agent_name || '';
      secEmail = config.sec_user_agent_email || '';
    } catch (e) {
      // Config not found is ok on first run
    }
  }

  async function checkStatus() {
    try {
      status = await invoke('check_status');
    } catch (e) {
      console.error('Failed to check status:', e);
    }
  }

  async function saveConfig() {
    isLoading = true;
    try {
      await invoke('save_config', {
        apiToken: apiToken || null,
        secUserAgentName: secName || null,
        secUserAgentEmail: secEmail || null,
      });
      showStatus('Configuration saved successfully', 'success');
      await checkStatus();
    } catch (e) {
      showStatus(`Failed to save config: ${e}`, 'error');
    } finally {
      isLoading = false;
    }
  }

  async function validateToken() {
    if (!apiToken) {
      showStatus('Please enter an API token first', 'error');
      return;
    }
    isValidating = true;
    try {
      const result: ValidationResponse = await invoke('validate_token', { apiToken });
      showStatus(result.message, result.success ? 'success' : 'error');
    } catch (e) {
      showStatus(`Validation failed: ${e}`, 'error');
    } finally {
      isValidating = false;
    }
  }

  async function configureClaudeDesktop() {
    isLoading = true;
    try {
      const result: ValidationResponse = await invoke('configure_claude_desktop');
      showStatus(result.message, result.success ? 'success' : 'error');
      await checkStatus();
    } catch (e) {
      showStatus(`Failed to configure Claude Desktop: ${e}`, 'error');
    } finally {
      isLoading = false;
    }
  }

  function showStatus(message: string, type: 'success' | 'error' | 'info') {
    statusMessage = message;
    statusType = type;
    setTimeout(() => {
      statusMessage = '';
      statusType = '';
    }, 5000);
  }

  // Computed: is everything configured?
  $: allConfigured = status?.claude_configured && status?.mcp_server_exists && status?.api_token_set && status?.sec_email_set;
</script>

<main>
  <div class="container">
    <h1>FilingExplorer Settings</h1>
    <p class="subtitle">Configure your FilingExplorer MCP server</p>

    <!-- Status Panel -->
    {#if status}
      <section class="status-panel" class:all-good={allConfigured}>
        <h2>{allConfigured ? 'Ready' : 'Setup Status'}</h2>
        <div class="status-items">
          <div class="status-item" class:ok={status.api_token_set} class:missing={!status.api_token_set}>
            <span class="indicator">{status.api_token_set ? '✓' : '○'}</span>
            <span>API Token</span>
          </div>
          <div class="status-item" class:ok={status.sec_email_set} class:missing={!status.sec_email_set}>
            <span class="indicator">{status.sec_email_set ? '✓' : '○'}</span>
            <span>SEC Email</span>
          </div>
          <div class="status-item" class:ok={status.claude_configured && status.mcp_server_exists} class:missing={!status.claude_configured || !status.mcp_server_exists}>
            <span class="indicator">{status.claude_configured && status.mcp_server_exists ? '✓' : '○'}</span>
            <span>Claude Desktop</span>
          </div>
        </div>
        {#if status.claude_configured && !status.mcp_server_exists}
          <p class="status-warning">MCP server binary not found at configured path</p>
        {/if}
      </section>
    {/if}

    <section class="section">
      <h2>FilingExplorer API</h2>
      <div class="form-group">
        <label for="apiToken">API Token</label>
        <div class="input-group">
          <input
            type="password"
            id="apiToken"
            bind:value={apiToken}
            placeholder="Enter your API token..."
          />
          <button
            class="secondary"
            on:click={validateToken}
            disabled={isValidating}
          >
            {isValidating ? 'Validating...' : 'Validate'}
          </button>
        </div>
        <p class="help-text">
          Get your API token from <a href="https://filingexplorer.com/settings" target="_blank" rel="noreferrer">filingexplorer.com/settings</a>
        </p>
      </div>
    </section>

    <section class="section">
      <h2>SEC EDGAR Access</h2>
      <p class="section-description">
        Required for direct SEC document access. The SEC requires identification for their fair access policy.
      </p>
      <div class="form-group">
        <label for="secName">Company/Name</label>
        <input
          type="text"
          id="secName"
          bind:value={secName}
          placeholder="Your Company Name"
        />
      </div>
      <div class="form-group">
        <label for="secEmail">Email Address</label>
        <input
          type="email"
          id="secEmail"
          bind:value={secEmail}
          placeholder="your@email.com"
        />
        <p class="help-text">
          Used in the User-Agent header when accessing SEC EDGAR
        </p>
      </div>
    </section>

    {#if statusMessage}
      <div class="status {statusType}">
        {statusMessage}
      </div>
    {/if}

    <div class="actions">
      <button class="primary" on:click={saveConfig} disabled={isLoading}>
        {isLoading ? 'Saving...' : 'Save Configuration'}
      </button>
    </div>

    <section class="section claude-section">
      <h2>Claude Desktop Integration</h2>
      <p class="section-description">
        Automatically add FilingExplorer to your Claude Desktop configuration.
      </p>
      <button class="secondary full-width" on:click={configureClaudeDesktop} disabled={isLoading}>
        {status?.claude_configured ? 'Reconfigure Claude Desktop' : 'Configure Claude Desktop'}
      </button>
      {#if status?.mcp_server_path}
        <p class="path-display">
          <span class="path-label">MCP Server:</span>
          <code>{status.mcp_server_path}</code>
        </p>
      {/if}
    </section>
  </div>
</main>

<style>
  .container {
    max-width: 500px;
    margin: 0 auto;
    text-align: left;
  }

  h1 {
    font-size: 1.8em;
    margin-bottom: 0.25em;
    text-align: center;
  }

  .subtitle {
    color: #888;
    text-align: center;
    margin-bottom: 1.5em;
  }

  h2 {
    font-size: 1.1em;
    margin-bottom: 0.75em;
    color: #646cff;
  }

  /* Status Panel */
  .status-panel {
    padding: 1em 1.5em;
    background: rgba(239, 68, 68, 0.1);
    border: 1px solid rgba(239, 68, 68, 0.3);
    border-radius: 12px;
    margin-bottom: 1.5em;
  }

  .status-panel.all-good {
    background: rgba(34, 197, 94, 0.1);
    border-color: rgba(34, 197, 94, 0.3);
  }

  .status-panel h2 {
    margin: 0 0 0.75em 0;
    color: inherit;
  }

  .status-panel.all-good h2 {
    color: #22c55e;
  }

  .status-items {
    display: flex;
    gap: 1.5em;
    flex-wrap: wrap;
  }

  .status-item {
    display: flex;
    align-items: center;
    gap: 0.4em;
    font-size: 0.9em;
  }

  .status-item.ok {
    color: #22c55e;
  }

  .status-item.missing {
    color: #888;
  }

  .indicator {
    font-weight: bold;
  }

  .status-warning {
    margin-top: 0.75em;
    margin-bottom: 0;
    font-size: 0.85em;
    color: #ef4444;
  }

  .section {
    margin-bottom: 1.5em;
    padding: 1.5em;
    background: rgba(255, 255, 255, 0.05);
    border-radius: 12px;
  }

  .section-description {
    font-size: 0.9em;
    color: #888;
    margin-bottom: 1em;
  }

  .form-group {
    margin-bottom: 1em;
  }

  label {
    display: block;
    margin-bottom: 0.5em;
    font-weight: 500;
  }

  input {
    width: 100%;
    padding: 0.75em;
    border: 1px solid #444;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.2);
    color: inherit;
    font-size: 1em;
    box-sizing: border-box;
  }

  input:focus {
    outline: none;
    border-color: #646cff;
  }

  .input-group {
    display: flex;
    gap: 0.5em;
  }

  .input-group input {
    flex: 1;
  }

  .input-group button {
    flex-shrink: 0;
  }

  .help-text {
    font-size: 0.85em;
    color: #888;
    margin-top: 0.5em;
  }

  .help-text a {
    color: #646cff;
  }

  .actions {
    margin-top: 1.5em;
  }

  button {
    padding: 0.75em 1.5em;
  }

  button.primary {
    width: 100%;
    background: #646cff;
    color: white;
  }

  button.primary:hover:not(:disabled) {
    background: #535bf2;
  }

  button.secondary {
    background: transparent;
    border: 1px solid #646cff;
    color: #646cff;
  }

  button.secondary:hover:not(:disabled) {
    background: rgba(100, 108, 255, 0.1);
  }

  button:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .full-width {
    width: 100%;
  }

  .claude-section {
    margin-top: 1.5em;
    border-top: 1px solid #333;
    padding-top: 1.5em;
  }

  .path-display {
    margin-top: 1em;
    margin-bottom: 0;
    font-size: 0.8em;
    color: #888;
    word-break: break-all;
  }

  .path-label {
    display: block;
    margin-bottom: 0.25em;
  }

  .path-display code {
    color: #aaa;
    background: rgba(0,0,0,0.2);
    padding: 0.2em 0.4em;
    border-radius: 4px;
    font-size: 0.9em;
  }

  .status {
    padding: 1em;
    border-radius: 8px;
    margin: 1em 0;
    font-size: 0.9em;
  }

  .status.success {
    background: rgba(34, 197, 94, 0.2);
    border: 1px solid #22c55e;
    color: #22c55e;
  }

  .status.error {
    background: rgba(239, 68, 68, 0.2);
    border: 1px solid #ef4444;
    color: #ef4444;
  }

  .status.info {
    background: rgba(100, 108, 255, 0.2);
    border: 1px solid #646cff;
    color: #646cff;
  }

  @media (prefers-color-scheme: light) {
    .section {
      background: rgba(0, 0, 0, 0.03);
    }

    input {
      background: white;
      border-color: #ddd;
    }

    .claude-section {
      border-top-color: #eee;
    }

    .path-display code {
      background: rgba(0,0,0,0.05);
      color: #666;
    }
  }
</style>
