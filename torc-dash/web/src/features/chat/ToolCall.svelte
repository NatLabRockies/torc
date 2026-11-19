<!-- Collapsible tool call with its input and (once it arrives) result. -->
<script lang="ts">
  import Icon from "../../lib/ui/Icon.svelte";

  let {
    name,
    input,
    result,
    isError = false,
    pending = false,
  }: { name: string; input: unknown; result?: string; isError?: boolean; pending?: boolean } =
    $props();

  const LIMIT = 5000;
  let shown = $derived(
    result && result.length > LIMIT ? `${result.slice(0, LIMIT)}\n… (truncated)` : result,
  );
</script>

<details class="tool" class:error={isError}>
  <summary>
    <span class="chev"><Icon name="chevronRight" size={12} /></span>
    <span class="mono name">{name}</span>
    {#if result === undefined}
      {#if pending}<span class="spinner small"></span>{/if}
    {:else if isError}
      <span class="state err">failed</span>
    {:else}
      <span class="state ok"><Icon name="check" size={12} /></span>
    {/if}
  </summary>
  <div class="body">
    <div class="label">Input</div>
    <pre class="code-block">{JSON.stringify(input, null, 2)}</pre>
    {#if shown !== undefined}
      <div class="label">Result</div>
      <pre class="code-block" class:err-block={isError}>{shown || "(empty)"}</pre>
    {/if}
  </div>
</details>

<style>
  .tool {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface-2);
    font-size: 12.5px;
  }
  .tool.error {
    border-color: var(--danger);
  }
  summary {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    cursor: pointer;
    list-style: none;
    color: var(--text-muted);
    user-select: none;
  }
  summary::-webkit-details-marker {
    display: none;
  }
  .chev {
    display: inline-flex;
    transition: transform 0.12s;
  }
  details[open] .chev {
    transform: rotate(90deg);
  }
  .name {
    color: var(--text);
  }
  .state {
    display: inline-flex;
    margin-left: auto;
  }
  .ok {
    color: var(--success);
  }
  .err {
    color: var(--danger);
  }
  .spinner.small {
    width: 11px;
    height: 11px;
    margin-left: auto;
  }
  .body {
    padding: 0 10px 10px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .label {
    color: var(--text-faint);
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    margin-top: 4px;
  }
  .code-block {
    max-height: 320px;
    background: var(--surface);
  }
  .err-block {
    color: var(--danger);
  }
</style>
