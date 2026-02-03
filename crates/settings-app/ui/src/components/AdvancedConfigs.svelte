<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import type { ClaudeConfigInfo } from '../lib/types';
  import ConfigCard from './ConfigCard.svelte';
  import ManualInstructions from './ManualInstructions.svelte';

  let {
    onstatuschange,
  }: {
    onstatuschange?: () => void;
  } = $props();

  let configs = $state<ClaudeConfigInfo[]>([]);
  let mcpServerPath = $state<string | null>(null);
  let mcpServerValid = $state(false);

  onMount(() => {
    loadConfigs();
  });

  async function loadConfigs() {
    try {
      configs = await invoke<ClaudeConfigInfo[]>('get_all_claude_configs');
      if (configs.length > 0) {
        mcpServerPath = configs[0].mcp_server_path;
        mcpServerValid = configs[0].mcp_server_valid;
      }
    } catch (e) {
      console.error('Failed to load configs:', e);
    }
  }

  function handleStatusChange() {
    loadConfigs();
    onstatuschange?.();
  }
</script>

<section class="advanced">
  <h2>Claude Configurations</h2>

  {#each configs as config (config.config_type)}
    <ConfigCard {config} onstatuschange={handleStatusChange} />
  {/each}

  {#if configs.length === 0}
    <p class="empty">No Claude config locations detected on this system.</p>
  {/if}

  {#if mcpServerPath}
    <div class="server-info">
      <span class="server-label">MCP Server Binary</span>
      <div class="server-path">
        <code>{mcpServerPath}</code>
        <span class="server-status" class:ok={mcpServerValid} class:missing={!mcpServerValid}>
          {mcpServerValid ? '✓ Found' : '✗ Not found'}
        </span>
      </div>
    </div>
  {/if}

  <ManualInstructions />
</section>

<style>
  h2 {
    font-size: 1em;
    margin-bottom: 0.75em;
    color: #646cff;
  }

  .empty {
    font-size: 0.85em;
    color: #888;
    padding: 1em;
    text-align: center;
  }

  .server-info {
    margin-top: 1em;
    padding: 0.75em 1em;
    background: rgba(255, 255, 255, 0.03);
    border-radius: 8px;
  }

  .server-label {
    font-size: 0.8em;
    font-weight: 600;
    color: #888;
    display: block;
    margin-bottom: 0.3em;
  }

  .server-path {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5em;
    flex-wrap: wrap;
  }

  .server-path code {
    font-size: 0.75em;
    color: #aaa;
    background: rgba(0, 0, 0, 0.15);
    padding: 0.15em 0.4em;
    border-radius: 4px;
    word-break: break-all;
  }

  .server-status {
    font-size: 0.8em;
    font-weight: 500;
    flex-shrink: 0;
  }

  .server-status.ok {
    color: #22c55e;
  }

  .server-status.missing {
    color: #ef4444;
  }

  @media (prefers-color-scheme: light) {
    .server-info {
      background: rgba(0, 0, 0, 0.02);
    }

    .server-path code {
      background: rgba(0, 0, 0, 0.05);
      color: #666;
    }
  }
</style>
