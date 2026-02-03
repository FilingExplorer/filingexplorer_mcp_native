<script lang="ts">
  import type { StatusResponse, TokenValidationStatus } from '../lib/types';

  let { status, tokenStatus }: { status: StatusResponse | null; tokenStatus: TokenValidationStatus } = $props();

  let allConfigured = $derived(
    (status?.claude_desktop_configured || status?.claude_code_configured) &&
      status?.mcp_server_exists &&
      status?.api_token_set
  );

  let claudeLabel = $derived(() => {
    if (!status) return '';
    const desktop = status.claude_desktop_configured;
    const code = status.claude_code_configured;
    if (desktop && code) return 'Desktop & Code';
    if (desktop) return 'Desktop only';
    if (code) return 'Code only';
    return '';
  });
</script>

{#if status}
  <section class="status-panel" class:all-good={allConfigured}>
    <div class="status-header">
      <span class="status-icon">{allConfigured ? '✓' : '○'}</span>
      <span class="status-title">{allConfigured ? 'Ready' : 'Setup Required'}</span>
    </div>
    <div class="status-items">
      <div class="status-item" class:ok={tokenStatus === 'valid'} class:missing={tokenStatus !== 'valid'}>
        <span class="indicator">{tokenStatus === 'valid' ? '✓' : '○'}</span>
        <span>API Token</span>
      </div>
      <div
        class="status-item"
        class:ok={(status.claude_desktop_configured || status.claude_code_configured) && status.mcp_server_exists}
        class:missing={!(status.claude_desktop_configured || status.claude_code_configured) || !status.mcp_server_exists}
      >
        <span class="indicator">{(status.claude_desktop_configured || status.claude_code_configured) && status.mcp_server_exists ? '✓' : '○'}</span>
        <span>Claude {claudeLabel() || ''}</span>
      </div>
    </div>
    {#if (status.claude_desktop_configured || status.claude_code_configured) && !status.mcp_server_exists}
      <p class="status-warning">MCP server binary not found at configured path</p>
    {/if}
  </section>
{/if}

<style>
  .status-panel {
    padding: 0.75em 1.25em;
    background: rgba(239, 68, 68, 0.08);
    border: 1px solid rgba(239, 68, 68, 0.2);
    border-radius: 10px;
    margin-bottom: 1.5em;
  }

  .status-panel.all-good {
    background: rgba(34, 197, 94, 0.08);
    border-color: rgba(34, 197, 94, 0.2);
  }

  .status-header {
    display: flex;
    align-items: center;
    gap: 0.5em;
    margin-bottom: 0.5em;
  }

  .status-icon {
    font-size: 1.1em;
    font-weight: bold;
  }

  .status-panel.all-good .status-icon,
  .status-panel.all-good .status-title {
    color: #22c55e;
  }

  .status-title {
    font-weight: 600;
    font-size: 0.95em;
  }

  .status-items {
    display: flex;
    gap: 1.5em;
    flex-wrap: wrap;
  }

  .status-item {
    display: flex;
    align-items: center;
    gap: 0.35em;
    font-size: 0.85em;
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
    margin-top: 0.5em;
    margin-bottom: 0;
    font-size: 0.8em;
    color: #ef4444;
  }
</style>
