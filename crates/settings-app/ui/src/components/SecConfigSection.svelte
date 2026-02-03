<script lang="ts">
  let {
    secName = $bindable(),
    secEmail = $bindable(),
    onsave,
  }: {
    secName: string;
    secEmail: string;
    onsave?: () => void;
  } = $props();

  function handleInput() {
    onsave?.();
  }
</script>

<section class="section">
  <h2>SEC EDGAR Access</h2>
  <p class="section-description">
    Optional. Required for direct SEC document access. The SEC requires identification for their fair access policy.
  </p>
  <div class="form-group">
    <label for="secName">Company / Name</label>
    <input
      type="text"
      id="secName"
      bind:value={secName}
      oninput={handleInput}
      placeholder="Your Company Name"
    />
  </div>
  <div class="form-group">
    <label for="secEmail">Email Address</label>
    <input
      type="email"
      id="secEmail"
      bind:value={secEmail}
      oninput={handleInput}
      placeholder="your@email.com"
    />
  </div>
</section>

<style>
  .section {
    margin-bottom: 1.25em;
  }

  h2 {
    font-size: 1em;
    margin-bottom: 0.5em;
    color: #646cff;
  }

  .section-description {
    font-size: 0.8em;
    color: #888;
    margin-bottom: 0.75em;
  }

  .form-group {
    margin-bottom: 0.75em;
  }

  label {
    display: block;
    margin-bottom: 0.3em;
    font-weight: 500;
    font-size: 0.85em;
  }

  input {
    width: 100%;
    padding: 0.6em 0.75em;
    border: 1px solid #444;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.2);
    color: inherit;
    font-size: 0.95em;
    box-sizing: border-box;
  }

  input:focus {
    outline: none;
    border-color: #646cff;
  }

  @media (prefers-color-scheme: light) {
    input {
      background: white;
      border-color: #ddd;
    }
  }
</style>
