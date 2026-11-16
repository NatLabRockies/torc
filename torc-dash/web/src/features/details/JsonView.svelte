<!-- Syntax-highlighted JSON in a scrollable <pre>. Large documents render as plain text. -->
<script lang="ts">
  import { highlightJson } from "./files";

  let { json }: { json: string } = $props();

  // ponytail: highlighting a multi-MB string costs seconds; plain text above this size.
  const MAX_HIGHLIGHT = 1_000_000;
</script>

{#if json.length > MAX_HIGHLIGHT}
  <pre class="json">{json}</pre>
{:else}
  <!-- highlightJson escapes its input. -->
  <pre class="json">{@html highlightJson(json)}</pre>
{/if}

<style>
  .json {
    flex: 1;
    min-height: 0;
    margin: 0;
    padding: 12px 14px;
    overflow: auto;
    font-size: 12.5px;
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-word;
  }
  .json :global(.k) {
    color: var(--accent);
  }
  .json :global(.s) {
    color: var(--success);
  }
  .json :global(.n) {
    color: var(--warning);
  }
  .json :global(.b) {
    color: var(--info);
  }
  .json :global(.p) {
    color: var(--text-muted);
  }
</style>
