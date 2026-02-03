<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import type { ToolCategoryInfo } from '../lib/types';

  let categories = $state<ToolCategoryInfo[]>([]);
  let expanded = $state<Set<string>>(new Set());
  let totalTools = $derived(categories.reduce((sum, c) => sum + c.tool_count, 0));

  onMount(async () => {
    try {
      categories = await invoke<ToolCategoryInfo[]>('get_tool_categories');
    } catch (e) {
      console.error('Failed to load tool categories:', e);
    }
  });

  function toggle(id: string) {
    if (expanded.has(id)) {
      expanded.delete(id);
    } else {
      expanded.add(id);
    }
    expanded = new Set(expanded);
  }
</script>

<section class="tool-docs">
  <div class="header">
    <h2>Available Tools</h2>
    <span class="tool-count">{totalTools} tools in {categories.length} categories</span>
  </div>

  {#if categories.length === 0}
    <p class="loading">Loading tools...</p>
  {/if}

  {#each categories as category (category.id)}
    <div class="category" class:open={expanded.has(category.id)}>
      <button class="category-header" onclick={() => toggle(category.id)}>
        <span class="chevron">{expanded.has(category.id) ? '▼' : '▶'}</span>
        <span class="category-name">{category.name}</span>
        <span class="category-count">{category.tool_count} {category.tool_count === 1 ? 'tool' : 'tools'}</span>
      </button>
      {#if expanded.has(category.id)}
        <div class="category-body">
          <p class="category-desc">{category.description}</p>
          <ul class="tool-list">
            {#each category.tools as tool (tool.name)}
              <li class="tool-item">
                <code class="tool-name">{tool.name}</code>
                <span class="tool-desc">{tool.description}</span>
              </li>
            {/each}
          </ul>
        </div>
      {/if}
    </div>
  {/each}
</section>

<style>
  .header {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    margin-bottom: 0.75em;
  }

  h2 {
    font-size: 1em;
    color: #646cff;
    margin: 0;
  }

  .tool-count {
    font-size: 0.8em;
    color: #888;
  }

  .loading {
    text-align: center;
    color: #888;
    font-size: 0.85em;
    padding: 2em 0;
  }

  .category {
    border: 1px solid #333;
    border-radius: 8px;
    margin-bottom: 0.5em;
    overflow: hidden;
  }

  .category.open {
    border-color: rgba(100, 108, 255, 0.3);
  }

  .category-header {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 0.5em;
    padding: 0.6em 0.8em;
    background: transparent;
    border: none;
    color: inherit;
    cursor: pointer;
    font-size: 0.9em;
    text-align: left;
  }

  .category-header:hover {
    background: rgba(100, 108, 255, 0.05);
  }

  .chevron {
    font-size: 0.65em;
    color: #888;
    width: 1em;
    flex-shrink: 0;
  }

  .category-name {
    font-weight: 500;
    flex: 1;
  }

  .category-count {
    font-size: 0.8em;
    color: #888;
    flex-shrink: 0;
  }

  .category-body {
    padding: 0 0.8em 0.75em;
    border-top: 1px solid #333;
  }

  .category-desc {
    font-size: 0.78em;
    color: #888;
    margin: 0.5em 0;
    line-height: 1.4;
  }

  .tool-list {
    list-style: none;
    padding: 0;
    margin: 0;
  }

  .tool-item {
    padding: 0.4em 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.03);
  }

  .tool-item:last-child {
    border-bottom: none;
  }

  .tool-name {
    display: block;
    font-size: 0.8em;
    color: #646cff;
    background: rgba(100, 108, 255, 0.08);
    padding: 0.1em 0.4em;
    border-radius: 3px;
    margin-bottom: 0.15em;
    font-weight: 500;
  }

  .tool-desc {
    display: block;
    font-size: 0.75em;
    color: #999;
    line-height: 1.4;
  }

  @media (prefers-color-scheme: light) {
    .category {
      border-color: #ddd;
    }

    .category.open {
      border-color: rgba(100, 108, 255, 0.4);
    }

    .category-body {
      border-top-color: #eee;
    }

    .tool-item {
      border-bottom-color: rgba(0, 0, 0, 0.04);
    }

    .tool-name {
      background: rgba(100, 108, 255, 0.06);
    }
  }
</style>
