<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { debounce } from './lib/debounce';
  import type { ConfigResponse, StatusResponse, Tab, TokenValidationStatus } from './lib/types';

  import TabBar from './components/TabBar.svelte';
  import StatusPanel from './components/StatusPanel.svelte';
  import ApiTokenInput from './components/ApiTokenInput.svelte';
  import SecConfigSection from './components/SecConfigSection.svelte';
  import BasicInstall from './components/BasicInstall.svelte';
  import AdvancedConfigs from './components/AdvancedConfigs.svelte';
  import ToolDocs from './components/ToolDocs.svelte';

  let activeTab = $state<Tab>('basic');

  // Config state
  let apiToken = $state('');
  let secName = $state('');
  let secEmail = $state('');
  let tokenStatus = $state<TokenValidationStatus>('idle');

  // Status state
  let status = $state<StatusResponse | null>(null);

  // Save feedback
  let saveMessage = $state('');

  onMount(async () => {
    await Promise.all([loadConfig(), checkStatus()]);
  });

  async function loadConfig() {
    try {
      const config: ConfigResponse = await invoke('load_config');
      apiToken = config.api_token || '';
      secName = config.sec_user_agent_name || '';
      secEmail = config.sec_user_agent_email || '';

      // If token is already set, trigger a validation
      if (apiToken) {
        tokenStatus = 'validating';
        try {
          const result = await invoke<{ success: boolean; message: string }>('validate_token', { apiToken });
          tokenStatus = result.success ? 'valid' : 'invalid';
        } catch {
          tokenStatus = 'error';
        }
      }
    } catch {
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

  // Auto-save with debounce
  const debouncedSave = debounce(async () => {
    try {
      await invoke('save_config', {
        apiToken: apiToken || null,
        secUserAgentName: secName || null,
        secUserAgentEmail: secEmail || null,
      });
      await checkStatus();
      showSaveMessage('Saved');
    } catch (e) {
      showSaveMessage(`Save failed: ${e}`);
    }
  }, 800);

  function handleConfigChange() {
    debouncedSave();
  }

  function showSaveMessage(msg: string) {
    saveMessage = msg;
    setTimeout(() => (saveMessage = ''), 2000);
  }

  function handleStatusChange() {
    checkStatus();
  }
</script>

<main>
  <div class="container">
    <header class="app-header">
      <h1>FilingExplorer for Claude</h1>
      <p class="subtitle">Connect Claude to SEC filings & financial data</p>
    </header>

    <StatusPanel {status} {tokenStatus} />

    <TabBar bind:activeTab />

    {#if activeTab === 'basic'}
      <div class="tab-content">
        <ApiTokenInput bind:apiToken bind:tokenStatus onsave={handleConfigChange} />
        <SecConfigSection bind:secName bind:secEmail onsave={handleConfigChange} />
        <BasicInstall {status} onstatuschange={handleStatusChange} />
      </div>
    {:else if activeTab === 'advanced'}
      <div class="tab-content">
        <AdvancedConfigs onstatuschange={handleStatusChange} />
      </div>
    {:else if activeTab === 'tools'}
      <div class="tab-content">
        <ToolDocs />
      </div>
    {/if}

    {#if saveMessage}
      <div class="save-indicator">{saveMessage}</div>
    {/if}
  </div>
</main>

<style>
  .container {
    max-width: 520px;
    margin: 0 auto;
    padding: 1.5em 1em;
    text-align: left;
  }

  .app-header {
    text-align: center;
    margin-bottom: 1.25em;
  }

  h1 {
    font-size: 1.5em;
    margin: 0 0 0.2em 0;
  }

  .subtitle {
    color: #888;
    font-size: 0.85em;
    margin: 0;
  }

  .tab-content {
    min-height: 300px;
  }

  .save-indicator {
    position: fixed;
    bottom: 1em;
    right: 1em;
    padding: 0.4em 0.8em;
    background: rgba(34, 197, 94, 0.15);
    color: #22c55e;
    border-radius: 6px;
    font-size: 0.75em;
    pointer-events: none;
    animation: fadeIn 0.15s ease;
  }

  @keyframes fadeIn {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
</style>
