<!-- Row of summary numbers used by the recover and sync-status dialogs. -->
<script lang="ts">
  export type Stat = { label: string; value: number; tone?: "danger" | "warning" | "success" };
  let { items }: { items: Stat[] } = $props();
</script>

<div class="stats">
  {#each items as s (s.label)}
    <div class="stat">
      <div class="value {s.tone ?? ''}">{s.value}</div>
      <div class="label">{s.label}</div>
    </div>
  {/each}
</div>

<style>
  .stats {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
    gap: 8px;
  }
  .stat {
    padding: 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface-2);
    text-align: center;
  }
  .value {
    font-size: 22px;
    font-weight: 700;
  }
  .danger {
    color: var(--danger);
  }
  .warning {
    color: var(--warning);
  }
  .success {
    color: var(--success);
  }
  .label {
    font-size: 12px;
    color: var(--text-muted);
  }
</style>
