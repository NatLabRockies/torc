<!-- Sub-tab switcher. <SegmentedTabs bind:value tabs={[{ id: "jobs", label: "Jobs", count: 12 }]} /> -->
<script lang="ts" generics="T extends string">
  let {
    value = $bindable(),
    tabs,
  }: { value: T; tabs: { id: T; label: string; count?: number | null }[] } = $props();
</script>

<div class="seg" role="tablist">
  {#each tabs as t (t.id)}
    <button
      role="tab"
      aria-selected={value === t.id}
      class:active={value === t.id}
      onclick={() => (value = t.id)}
    >
      {t.label}
      {#if t.count !== undefined && t.count !== null}<span class="count">{t.count}</span>{/if}
    </button>
  {/each}
</div>

<style>
  .seg {
    display: inline-flex;
    gap: 2px;
    padding: 3px;
    background: var(--neutral-soft);
    border-radius: var(--radius);
    flex-wrap: wrap;
  }
  button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 12px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-muted);
    font: inherit;
    font-weight: 500;
    cursor: pointer;
  }
  button:hover {
    color: var(--text);
  }
  .active {
    background: var(--surface);
    color: var(--text);
    box-shadow: var(--shadow-sm);
  }
  .count {
    font-size: 11px;
    padding: 0 6px;
    border-radius: 999px;
    background: var(--border);
    color: var(--text-muted);
  }
</style>
