<!-- Syntax-highlighted JSON in a scrollable <pre>. Large documents render as plain text. -->
<script lang="ts">
  import { highlightJson, MAX_HIGHLIGHT } from "../../lib/highlight";

  let { json }: { json: string } = $props();
</script>

{#if json.length > MAX_HIGHLIGHT}
  <pre class="json">{json}</pre>
{:else}
  <!-- highlightJson escapes its input. -->
  <pre class="json hl">{@html highlightJson(json)}</pre>
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
</style>
