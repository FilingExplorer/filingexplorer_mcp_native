<script lang="ts">
  import type { Tab } from '../lib/types';

  let { activeTab = $bindable(), onchange }: { activeTab: Tab; onchange?: (tab: Tab) => void } = $props();

  const tabs: { id: Tab; label: string }[] = [
    { id: 'basic', label: 'Basic' },
    { id: 'advanced', label: 'Advanced' },
    { id: 'tools', label: 'Tools' },
  ];

  function select(tab: Tab) {
    activeTab = tab;
    onchange?.(tab);
  }
</script>

<nav class="tab-bar">
  {#each tabs as tab}
    <button
      class="tab"
      class:active={activeTab === tab.id}
      onclick={() => select(tab.id)}
    >
      {tab.label}
    </button>
  {/each}
</nav>

<style>
  .tab-bar {
    display: flex;
    gap: 2px;
    background: rgba(255, 255, 255, 0.05);
    border-radius: 10px;
    padding: 3px;
    margin-bottom: 1.5em;
  }

  .tab {
    flex: 1;
    padding: 0.6em 1em;
    border: none;
    background: transparent;
    color: #888;
    font-size: 0.9em;
    font-weight: 500;
    border-radius: 8px;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .tab:hover {
    color: #ccc;
  }

  .tab.active {
    background: rgba(100, 108, 255, 0.2);
    color: #646cff;
  }

  @media (prefers-color-scheme: light) {
    .tab-bar {
      background: rgba(0, 0, 0, 0.04);
    }

    .tab:hover {
      color: #333;
    }

    .tab.active {
      background: rgba(100, 108, 255, 0.12);
    }
  }
</style>
