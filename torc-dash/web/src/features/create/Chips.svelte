<!-- Multi-select as toggle chips. Replaces the legacy Ctrl/Cmd-click <select multiple>. -->
<script lang="ts">
  let {
    label,
    options,
    selected = $bindable(),
    empty,
    error,
  }: {
    label: string;
    options: { id: number; label: string }[];
    selected: number[];
    empty: string;
    error?: string;
  } = $props();

  function toggle(id: number) {
    selected = selected.includes(id) ? selected.filter((x) => x !== id) : [...selected, id];
  }
</script>

<div class="field wide" class:invalid={!!error} role="group" aria-label={label}>
  <span>{label}</span>
  {#if options.length === 0}
    <span class="hint">{empty}</span>
  {:else}
    <div class="chips">
      {#each options as o (o.id)}
        <button
          type="button"
          class="chip"
          aria-pressed={selected.includes(o.id)}
          onclick={() => toggle(o.id)}
        >
          {#if selected.includes(o.id)}✓ {/if}{o.label}
        </button>
      {/each}
    </div>
  {/if}
  {#if error}<span class="err" role="alert">{error}</span>{/if}
</div>

<style>
  .wide {
    grid-column: 1 / -1;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .chip {
    height: 26px;
    padding: 0 10px;
    border: 1px solid var(--border-strong);
    border-radius: 999px;
    background: var(--surface);
    color: var(--text-muted);
    font: inherit;
    font-size: 12.5px;
    cursor: pointer;
  }
  .chip:hover {
    color: var(--text);
  }
  .chip[aria-pressed="true"] {
    background: var(--accent-soft);
    border-color: var(--accent);
    color: var(--accent);
    font-weight: 500;
  }
  .chip:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .invalid .chips {
    outline: 1px dashed var(--danger);
    outline-offset: 4px;
    border-radius: 4px;
  }
  .err {
    color: var(--danger);
    font-size: 12px;
    font-weight: 400;
  }
</style>
