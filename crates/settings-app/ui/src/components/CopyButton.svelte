<script lang="ts">
  import { writeText } from '@tauri-apps/plugin-clipboard-manager';

  let { text, label = 'Copy' }: { text: string; label?: string } = $props();

  let copied = $state(false);

  async function copy() {
    try {
      await writeText(text);
      copied = true;
      setTimeout(() => (copied = false), 2000);
    } catch (e) {
      console.error('Failed to copy:', e);
    }
  }
</script>

<button class="copy-btn" class:copied onclick={copy}>
  {copied ? 'Copied!' : label}
</button>

<style>
  .copy-btn {
    padding: 0.35em 0.75em;
    border: 1px solid #555;
    border-radius: 6px;
    background: transparent;
    color: #aaa;
    font-size: 0.8em;
    cursor: pointer;
    transition: all 0.15s ease;
    white-space: nowrap;
  }

  .copy-btn:hover {
    border-color: #646cff;
    color: #646cff;
  }

  .copy-btn.copied {
    border-color: #22c55e;
    color: #22c55e;
  }

  @media (prefers-color-scheme: light) {
    .copy-btn {
      border-color: #ccc;
      color: #666;
    }
  }
</style>
