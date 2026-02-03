<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import CopyButton from './CopyButton.svelte';

  let desktopSnippet = $state('');
  let codeSnippet = $state('');

  onMount(async () => {
    try {
      [desktopSnippet, codeSnippet] = await Promise.all([
        invoke<string>('get_mcp_config_snippet', { configType: 'desktop' }),
        invoke<string>('get_mcp_config_snippet', { configType: 'code_global' }),
      ]);
    } catch (e) {
      desktopSnippet = '(Could not determine MCP server path)';
      codeSnippet = desktopSnippet;
    }
  });
</script>

<details class="instructions">
  <summary>Manual installation instructions</summary>
  <div class="content">
    <div class="snippet-section">
      <div class="snippet-header">
        <h4>Claude Desktop</h4>
        <p class="snippet-path">Add to <code>mcpServers</code> in <code>claude_desktop_config.json</code>:</p>
      </div>
      <div class="snippet-block">
        <pre><code>{desktopSnippet}</code></pre>
        {#if desktopSnippet && !desktopSnippet.startsWith('(')}
          <div class="copy-pos">
            <CopyButton text={desktopSnippet} />
          </div>
        {/if}
      </div>
    </div>

    <div class="snippet-section">
      <div class="snippet-header">
        <h4>Claude Code</h4>
        <p class="snippet-path">Add to <code>mcpServers</code> in <code>~/.claude.json</code>:</p>
      </div>
      <div class="snippet-block">
        <pre><code>{codeSnippet}</code></pre>
        {#if codeSnippet && !codeSnippet.startsWith('(')}
          <div class="copy-pos">
            <CopyButton text={codeSnippet} />
          </div>
        {/if}
      </div>
    </div>
  </div>
</details>

<style>
  .instructions {
    margin-top: 1em;
  }

  summary {
    cursor: pointer;
    color: #888;
    font-size: 0.85em;
    padding: 0.4em 0;
    user-select: none;
  }

  summary:hover {
    color: #646cff;
  }

  .content {
    margin-top: 0.75em;
  }

  .snippet-section {
    margin-bottom: 1em;
  }

  .snippet-section:last-child {
    margin-bottom: 0;
  }

  .snippet-header h4 {
    font-size: 0.85em;
    margin: 0 0 0.2em 0;
    color: inherit;
  }

  .snippet-path {
    font-size: 0.75em;
    color: #888;
    margin: 0 0 0.4em 0;
  }

  .snippet-path code {
    background: rgba(0, 0, 0, 0.2);
    padding: 0.1em 0.3em;
    border-radius: 3px;
    font-size: 0.95em;
  }

  .snippet-block {
    position: relative;
    background: rgba(0, 0, 0, 0.25);
    border-radius: 8px;
    padding: 0.75em;
    overflow-x: auto;
  }

  .snippet-block pre {
    margin: 0;
    font-size: 0.8em;
    line-height: 1.5;
  }

  .snippet-block code {
    color: #ccc;
  }

  .copy-pos {
    position: absolute;
    top: 0.5em;
    right: 0.5em;
  }

  @media (prefers-color-scheme: light) {
    .snippet-path code {
      background: rgba(0, 0, 0, 0.06);
    }

    .snippet-block {
      background: rgba(0, 0, 0, 0.04);
    }

    .snippet-block code {
      color: #333;
    }
  }
</style>
