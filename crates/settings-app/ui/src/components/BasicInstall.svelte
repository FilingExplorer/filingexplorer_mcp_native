<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import type { StatusResponse, ValidationResponse } from '../lib/types';
  import ManualInstructions from './ManualInstructions.svelte';

  let {
    status,
    onstatuschange,
  }: {
    status: StatusResponse | null;
    onstatuschange?: () => void;
  } = $props();

  let isInstalling = $state(false);
  let message = $state('');
  let messageType = $state<'success' | 'error' | ''>('');

  let anyInstalled = $derived(
    status?.claude_desktop_configured || status?.claude_code_configured
  );

  let installLabel = $derived(() => {
    if (isInstalling) return 'Installing...';
    if (status?.claude_desktop_configured && status?.claude_code_configured) return 'Reinstall FilingExplorer MCP';
    if (anyInstalled) return 'Install to All Claude Apps';
    return 'Install FilingExplorer MCP';
  });

  let integrationStatus = $derived(() => {
    if (!status) return 'Checking...';
    const desktop = status.claude_desktop_configured;
    const code = status.claude_code_configured;
    if (desktop && code) return 'Installed for Claude Desktop & Code';
    if (desktop) return 'Installed for Claude Desktop';
    if (code) return 'Installed for Claude Code';
    return 'Not installed';
  });

  async function installBoth() {
    isInstalling = true;
    message = '';
    try {
      const result: ValidationResponse = await invoke('configure_both');
      message = result.message;
      messageType = result.success ? 'success' : 'error';
      onstatuschange?.();
    } catch (e) {
      message = `Failed to configure: ${e}`;
      messageType = 'error';
    } finally {
      isInstalling = false;
      setTimeout(() => {
        message = '';
        messageType = '';
      }, 6000);
    }
  }
</script>

<section class="install-section">
  <h2>Claude Integration</h2>
  <div class="integration-status" class:installed={anyInstalled}>
    <span class="indicator">{anyInstalled ? '✓' : '○'}</span>
    <span>{integrationStatus()}</span>
  </div>
  <button class="install-btn" onclick={installBoth} disabled={isInstalling}>
    {installLabel()}
  </button>
  {#if message}
    <div class="message {messageType}">{message}</div>
  {/if}
  <ManualInstructions />
</section>

<style>
  .install-section {
    margin-top: 0.5em;
  }

  h2 {
    font-size: 1em;
    margin-bottom: 0.5em;
    color: #646cff;
  }

  .integration-status {
    display: flex;
    align-items: center;
    gap: 0.4em;
    font-size: 0.85em;
    color: #888;
    margin-bottom: 0.75em;
  }

  .integration-status.installed {
    color: #22c55e;
  }

  .indicator {
    font-weight: bold;
  }

  .install-btn {
    width: 100%;
    padding: 0.7em 1.5em;
    background: #646cff;
    color: white;
    border: none;
    border-radius: 8px;
    font-size: 0.95em;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.15s ease;
  }

  .install-btn:hover:not(:disabled) {
    background: #535bf2;
  }

  .install-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .message {
    padding: 0.6em 0.8em;
    border-radius: 6px;
    margin-top: 0.75em;
    font-size: 0.8em;
    line-height: 1.4;
  }

  .message.success {
    background: rgba(34, 197, 94, 0.12);
    color: #22c55e;
  }

  .message.error {
    background: rgba(239, 68, 68, 0.12);
    color: #ef4444;
  }
</style>
