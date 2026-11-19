<!-- Titled, auto-height DataTable for short lists inside the job details modal. -->
<script lang="ts" generics="T">
  import DataTable, { type Column } from "../../lib/ui/DataTable.svelte";

  let {
    title,
    rows,
    columns,
    rowKey,
    empty,
    onRowClick,
  }: {
    title: string;
    rows: T[];
    columns: Column<T>[];
    rowKey: (r: T) => string | number;
    empty: string;
    onRowClick?: (r: T) => void;
  } = $props();
</script>

<section>
  <h4>{title} <span class="faint">{rows.length}</span></h4>
  {#if rows.length}
    <div style:height="{Math.min(rows.length, 8) * 36 + 72}px">
      <DataTable {rows} {columns} {rowKey} {onRowClick} />
    </div>
  {:else}
    <p class="muted">{empty}</p>
  {/if}
</section>

<style>
  h4 {
    font-size: 13px;
    margin-bottom: 8px;
  }
  p {
    margin: 0;
    font-size: 13px;
  }
</style>
