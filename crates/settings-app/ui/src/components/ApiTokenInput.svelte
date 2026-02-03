<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { open } from '@tauri-apps/plugin-shell';
  import { debounce } from '../lib/debounce';
  import type { ValidationResponse, TokenValidationStatus } from '../lib/types';

  let {
    apiToken = $bindable(),
    tokenStatus = $bindable(),
    onsave,
  }: {
    apiToken: string;
    tokenStatus: TokenValidationStatus;
    onsave?: () => void;
  } = $props();

  let showToken = $state(false);

  const debouncedValidate = debounce(async (token: string) => {
    if (!token || token.length < 10) {
      tokenStatus = 'idle';
      return;
    }
    tokenStatus = 'validating';
    try {
      const result: ValidationResponse = await invoke('validate_token', { apiToken: token });
      tokenStatus = result.success ? 'valid' : 'invalid';
    } catch {
      tokenStatus = 'error';
    }
  }, 600);

  function handleInput() {
    if (!apiToken) {
      tokenStatus = 'idle';
    } else {
      debouncedValidate(apiToken);
    }
    onsave?.();
  }

  function openApiKeys() {
    open('https://www.filingexplorer.com/api-keys');
  }
</script>

<div class="form-group">
  <label for="apiToken">API Token</label>
  <div class="input-row">
    <div class="input-wrapper">
      <input
        type={showToken ? 'text' : 'password'}
        id="apiToken"
        bind:value={apiToken}
        oninput={handleInput}
        placeholder="Enter your API token..."
      />
      <button class="toggle-vis" onclick={() => (showToken = !showToken)} title={showToken ? 'Hide' : 'Show'}>
        {showToken ? '◉' : '○'}
      </button>
    </div>
    <div class="validation-indicator" class:validating={tokenStatus === 'validating'} class:valid={tokenStatus === 'valid'} class:invalid={tokenStatus === 'invalid' || tokenStatus === 'error'}>
      {#if tokenStatus === 'validating'}
        <span class="spinner"></span>
      {:else if tokenStatus === 'valid'}
        ✓
      {:else if tokenStatus === 'invalid'}
        ✗
      {:else if tokenStatus === 'error'}
        !
      {/if}
    </div>
  </div>
  {#if tokenStatus === 'invalid'}
    <p class="validation-msg error">Invalid API token</p>
  {:else if tokenStatus === 'error'}
    <p class="validation-msg error">Could not validate token</p>
  {:else if tokenStatus === 'valid'}
    <p class="validation-msg success">Token is valid</p>
  {/if}
  <p class="help-text">
    Get your API token from
    <button class="link-button" onclick={openApiKeys}>filingexplorer.com/api-keys</button>
  </p>
</div>

<style>
  .form-group {
    margin-bottom: 1em;
  }

  label {
    display: block;
    margin-bottom: 0.4em;
    font-weight: 500;
    font-size: 0.9em;
  }

  .input-row {
    display: flex;
    align-items: center;
    gap: 0.5em;
  }

  .input-wrapper {
    flex: 1;
    position: relative;
  }

  .input-wrapper input {
    width: 100%;
    padding: 0.65em 2.2em 0.65em 0.75em;
    border: 1px solid #444;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.2);
    color: inherit;
    font-size: 0.95em;
    box-sizing: border-box;
  }

  .input-wrapper input:focus {
    outline: none;
    border-color: #646cff;
  }

  .toggle-vis {
    position: absolute;
    right: 0.5em;
    top: 50%;
    transform: translateY(-50%);
    background: none;
    border: none;
    color: #888;
    cursor: pointer;
    padding: 0.2em;
    font-size: 0.9em;
    line-height: 1;
  }

  .validation-indicator {
    width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-weight: bold;
    font-size: 0.9em;
    flex-shrink: 0;
  }

  .validation-indicator.valid {
    color: #22c55e;
  }

  .validation-indicator.invalid {
    color: #ef4444;
  }

  .validation-indicator.validating {
    color: #646cff;
  }

  .spinner {
    display: inline-block;
    width: 14px;
    height: 14px;
    border: 2px solid rgba(100, 108, 255, 0.3);
    border-top-color: #646cff;
    border-radius: 50%;
    animation: spin 0.6s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .validation-msg {
    font-size: 0.8em;
    margin: 0.3em 0 0 0;
  }

  .validation-msg.success {
    color: #22c55e;
  }

  .validation-msg.error {
    color: #ef4444;
  }

  .help-text {
    font-size: 0.8em;
    color: #888;
    margin-top: 0.4em;
  }

  .link-button {
    background: none;
    border: none;
    color: #646cff;
    cursor: pointer;
    padding: 0;
    font-size: inherit;
    text-decoration: underline;
  }

  .link-button:hover {
    color: #535bf2;
  }

  @media (prefers-color-scheme: light) {
    .input-wrapper input {
      background: white;
      border-color: #ddd;
    }
  }
</style>
