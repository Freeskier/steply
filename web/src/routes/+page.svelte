<script lang="ts">
  import { previewDocsJson } from '$lib';

  let status = 'WASM not loaded';
  let output = '';
  let loading = false;

  async function runWasmDemo() {
    loading = true;
    status = 'Loading WASM...';
    output = '';

    try {
      const docsJson = await previewDocsJson();
      status = 'WASM loaded';
      output = docsJson.slice(0, 800) + (docsJson.length > 800 ? '\n\n...trimmed' : '');
    } catch (error) {
      status = 'WASM error';
      output = error instanceof Error ? error.message : String(error);
    } finally {
      loading = false;
    }
  }
</script>

<main>
  <h1>Hello World</h1>
  <p>Minimal client-only WASM demo.</p>

  <button on:click={runWasmDemo} disabled={loading}>
    {loading ? 'Loading...' : 'Run WASM demo'}
  </button>

  <p><strong>Status:</strong> {status}</p>

  {#if output}
    <pre>{output}</pre>
  {/if}
</main>

<style>
  main {
    max-width: 760px;
    margin: 3rem auto;
    padding: 0 1rem;
    font-family: system-ui, sans-serif;
  }

  button {
    margin: 1rem 0;
    padding: 0.6rem 1rem;
    font-size: 1rem;
    cursor: pointer;
  }

  pre {
    white-space: pre-wrap;
    word-break: break-word;
    background: #f3f3f3;
    padding: 1rem;
    border-radius: 8px;
  }
</style>
