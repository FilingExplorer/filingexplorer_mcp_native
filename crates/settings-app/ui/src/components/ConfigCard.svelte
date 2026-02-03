<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { writeText } from '@tauri-apps/plugin-clipboard-manager';
  import type { ClaudeConfigInfo, ValidationResponse } from '../lib/types';

  let {
    config,
    onstatuschange,
  }: {
    config: ClaudeConfigInfo;
    onstatuschange?: () => void;
  } = $props();

  let isInstalling = $state(false);
  let message = $state('');
  let messageType = $state<'success' | 'error' | ''>('');
  let copied = $state(false);

  async function install() {
    isInstalling = true;
    message = '';
    try {
      const result: ValidationResponse = await invoke('install_mcp_to_config', {
        configType: config.config_type,
      });
      message = result.message;
      messageType = result.success ? 'success' : 'error';
      onstatuschange?.();
    } catch (e) {
      message = `Failed: ${e}`;
      messageType = 'error';
    } finally {
      isInstalling = false;
      setTimeout(() => {
        message = '';
        messageType = '';
      }, 5000);
    }
  }

  async function copyConfig() {
    try {
      const snippet: string = await invoke('get_mcp_config_snippet', {
        configType: config.config_type,
      });
      await writeText(snippet);
      copied = true;
      setTimeout(() => (copied = false), 2000);
    } catch (e) {
      console.error('Copy failed:', e);
    }
  }

  // Shorten the home directory path for display
  function shortenPath(path: string): string {
    const home = path.match(/^\/Users\/[^/]+/)?.[0] || path.match(/^\/home\/[^/]+/)?.[0];
    if (home) {
      return path.replace(home, '~');
    }
    return path;
  }
</script>

<div class="config-card" class:installed={config.mcp_installed}>
  <div class="card-header">
    <span class="card-label">{config.label}</span>
    <span class="card-status" class:ok={config.mcp_installed} class:missing={!config.mcp_installed}>
      {config.mcp_installed ? '✓ Installed' : '○ Not installed'}
    </span>
  </div>
  <p class="card-path"><code>{shortenPath(config.path)}</code></p>
  <div class="card-actions">
    <button class="action-btn primary-action" onclick={install} disabled={isInstalling}>
      {isInstalling ? 'Installing...' : config.mcp_installed ? 'Reinstall' : 'Install'}
    </button>
    <button class="action-btn" class:copied onclick={copyConfig}>
      {copied ? 'Copied!' : 'Copy Config'}
    </button>
  </div>
  {#if !config.mcp_server_valid && config.mcp_server_path}
    <p class="card-warning">Binary not found: {config.mcp_server_path}</p>
  {/if}
  {#if message}
    <div class="card-message {messageType}">{message}</div>
  {/if}
</div>

<style>
  .config-card {
    padding: 0.9em 1em;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid #333;
    border-radius: 10px;
    margin-bottom: 0.75em;
  }

  .config-card.installed {
    border-color: rgba(34, 197, 94, 0.3);
  }

  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.3em;
  }

  .card-label {
    font-weight: 600;
    font-size: 0.9em;
  }

  .card-status {
    font-size: 0.8em;
    font-weight: 500;
  }

  .card-status.ok {
    color: #22c55e;
  }

  .card-status.missing {
    color: #888;
  }

  .card-path {
    margin: 0 0 0.6em 0;
  }

  .card-path code {
    font-size: 0.75em;
    color: #888;
    background: rgba(0, 0, 0, 0.15);
    padding: 0.15em 0.35em;
    border-radius: 4px;
    word-break: break-all;
  }

  .card-actions {
    display: flex;
    gap: 0.5em;
  }

  .action-btn {
    padding: 0.4em 0.8em;
    border: 1px solid #555;
    border-radius: 6px;
    background: transparent;
    color: #aaa;
    font-size: 0.8em;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .action-btn:hover:not(:disabled) {
    border-color: #646cff;
    color: #646cff;
  }

  .action-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .action-btn.primary-action {
    background: rgba(100, 108, 255, 0.1);
    border-color: rgba(100, 108, 255, 0.4);
    color: #646cff;
  }

  .action-btn.primary-action:hover:not(:disabled) {
    background: rgba(100, 108, 255, 0.2);
  }

  .action-btn.copied {
    border-color: #22c55e;
    color: #22c55e;
  }

  .card-warning {
    margin: 0.5em 0 0 0;
    font-size: 0.75em;
    color: #ef4444;
  }

  .card-message {
    padding: 0.4em 0.6em;
    border-radius: 5px;
    margin-top: 0.5em;
    font-size: 0.75em;
    line-height: 1.4;
  }

  .card-message.success {
    background: rgba(34, 197, 94, 0.1);
    color: #22c55e;
  }

  .card-message.error {
    background: rgba(239, 68, 68, 0.1);
    color: #ef4444;
  }

  @media (prefers-color-scheme: light) {
    .config-card {
      background: rgba(0, 0, 0, 0.02);
      border-color: #ddd;
    }

    .config-card.installed {
      border-color: rgba(34, 197, 94, 0.4);
    }

    .card-path code {
      background: rgba(0, 0, 0, 0.05);
      color: #666;
    }

    .action-btn {
      border-color: #ccc;
      color: #666;
    }
  }
</style>
