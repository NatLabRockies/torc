<!--
  Sortable, filterable, virtualized table. Only visible rows are rendered, so 100k rows stay fast.
  The table fills its parent's height; give the parent a height (e.g. flex: 1; min-height: 0).

  <DataTable rows={jobs} rowKey={(j) => j.id!} filter={text} onRowClick={(j) => app.openJob(j.id!)}
    columns={[
      { key: "id", label: "ID", width: "80px", mono: true },
      { key: "name", label: "Name" },
      { key: "status", label: "Status", cell: statusCell, width: "120px" },
    ]} />
  {#snippet statusCell(job: Job)}<Badge status={job.status} />{/snippet}
-->
<script lang="ts" module>
  import type { Snippet } from "svelte";

  export type Column<T> = {
    key: string;
    label: string;
    /** Value used for display (when no `cell`), sorting, and filtering. Default: row[key]. */
    value?: (row: T) => unknown;
    /** Custom cell content. */
    cell?: Snippet<[T]>;
    /** CSS width such as "90px" or "20%". Unset columns share the remaining space. */
    width?: string;
    align?: "left" | "right" | "center";
    sortable?: boolean;
    mono?: boolean;
  };

  export type SortState = { key: string; dir: "asc" | "desc" } | null;
</script>

<script lang="ts" generics="T">
  import { SvelteSet } from "svelte/reactivity";

  let {
    rows,
    columns,
    rowKey,
    filter = "",
    onRowClick,
    selectable = false,
    selected = $bindable(new SvelteSet<string | number>()),
    rowHeight = 36,
    sort = $bindable<SortState>(null),
    loading = false,
    empty = "No data",
    rowClass,
    activeKey = null,
  }: {
    rows: T[];
    columns: Column<T>[];
    rowKey: (row: T) => string | number;
    filter?: string;
    onRowClick?: (row: T) => void;
    selectable?: boolean;
    selected?: SvelteSet<string | number>;
    rowHeight?: number;
    sort?: SortState;
    loading?: boolean;
    empty?: string;
    rowClass?: (row: T) => string;
    /** Key of a row to highlight (e.g. the open item). */
    activeKey?: string | number | null;
  } = $props();

  const OVERSCAN = 12;

  function valueOf(col: Column<T>, row: T): unknown {
    return col.value ? col.value(row) : (row as Record<string, unknown>)[col.key];
  }

  function display(v: unknown): string {
    if (v === null || v === undefined || v === "") return "—";
    if (typeof v === "object") return JSON.stringify(v);
    return String(v);
  }

  const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });

  function compare(a: unknown, b: unknown): number {
    const an = a === null || a === undefined || a === "";
    const bn = b === null || b === undefined || b === "";
    if (an || bn) return an === bn ? 0 : an ? 1 : -1; // empties last
    if (typeof a === "number" && typeof b === "number") return a - b;
    if (typeof a === "boolean" && typeof b === "boolean") return Number(a) - Number(b);
    return collator.compare(String(a), String(b));
  }

  let filtered = $derived.by(() => {
    const q = filter.trim().toLowerCase();
    if (!q) return rows;
    return rows.filter((row) =>
      columns.some((c) => {
        const v = valueOf(c, row);
        return v !== null && v !== undefined && display(v).toLowerCase().includes(q);
      }),
    );
  });

  let sorted = $derived.by(() => {
    const s = sort;
    const col = s && columns.find((c) => c.key === s.key);
    if (!s || !col) return filtered;
    const dir = s.dir === "asc" ? 1 : -1;
    const empty = (v: unknown) => v === null || v === undefined || v === "";
    // Compute each key once; column value() can be costly (e.g. JSON.stringify).
    return filtered
      .map((row) => ({ row, v: valueOf(col, row) }))
      .sort((a, b) => {
        const r = compare(a.v, b.v);
        // Keep empties last regardless of direction.
        return empty(a.v) || empty(b.v) ? r : r * dir;
      })
      .map((e) => e.row);
  });

  function toggleSort(col: Column<T>) {
    if (col.sortable === false) return;
    if (sort?.key !== col.key) sort = { key: col.key, dir: "asc" };
    else if (sort.dir === "asc") sort = { key: col.key, dir: "desc" };
    else sort = null;
  }

  // Virtualization
  let scrollTop = $state(0);
  let viewportHeight = $state(600);
  let start = $derived(Math.max(0, Math.floor(scrollTop / rowHeight) - OVERSCAN));
  let end = $derived(
    Math.min(sorted.length, Math.ceil((scrollTop + viewportHeight) / rowHeight) + OVERSCAN),
  );
  let visible = $derived(sorted.slice(start, end));

  // Selection
  let allSelected = $derived(
    sorted.length > 0 && sorted.every((r) => selected.has(rowKey(r))),
  );
  let someSelected = $derived(!allSelected && sorted.some((r) => selected.has(rowKey(r))));

  function toggleAll() {
    if (allSelected) for (const r of sorted) selected.delete(rowKey(r));
    else for (const r of sorted) selected.add(rowKey(r));
  }

  function toggleRow(row: T) {
    const k = rowKey(row);
    if (selected.has(k)) selected.delete(k);
    else selected.add(k);
  }
</script>

<div class="wrap">
  <div
    class="scroll"
    bind:clientHeight={viewportHeight}
    onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}
  >
    <table style:--row-h="{rowHeight}px">
      <colgroup>
        {#if selectable}<col style:width="40px" />{/if}
        {#each columns as c (c.key)}<col style:width={c.width} />{/each}
      </colgroup>
      <thead>
        <tr>
          {#if selectable}
            <th class="check-col">
              <input
                type="checkbox"
                aria-label="Select all"
                checked={allSelected}
                indeterminate={someSelected}
                onchange={toggleAll}
              />
            </th>
          {/if}
          {#each columns as c (c.key)}
            <th
              class:sortable={c.sortable !== false}
              style:text-align={c.align}
              aria-sort={sort?.key === c.key
                ? sort.dir === "asc"
                  ? "ascending"
                  : "descending"
                : undefined}
              onclick={() => toggleSort(c)}
            >
              {c.label}
              {#if sort?.key === c.key}<span class="arrow">{sort.dir === "asc" ? "▲" : "▼"}</span
                >{/if}
            </th>
          {/each}
        </tr>
      </thead>
      <tbody>
        {#if start > 0}<tr class="spacer" style:height="{start * rowHeight}px"></tr>{/if}
        {#each visible as row (rowKey(row))}
          {@const k = rowKey(row)}
          <tr
            class={rowClass?.(row)}
            class:clickable={!!onRowClick}
            class:selected={selected.has(k)}
            class:active={activeKey !== null && activeKey === k}
            onclick={() => onRowClick?.(row)}
          >
            {#if selectable}
              <td class="check-col" onclick={(e) => e.stopPropagation()}>
                <input
                  type="checkbox"
                  aria-label="Select row"
                  checked={selected.has(k)}
                  onchange={() => toggleRow(row)}
                />
              </td>
            {/if}
            {#each columns as c (c.key)}
              <td class:mono={c.mono} style:text-align={c.align}>
                {#if c.cell}{@render c.cell(row)}{:else}{display(valueOf(c, row))}{/if}
              </td>
            {/each}
          </tr>
        {/each}
        {#if end < sorted.length}
          <tr class="spacer" style:height="{(sorted.length - end) * rowHeight}px"></tr>
        {/if}
      </tbody>
    </table>
    {#if sorted.length === 0}
      <div class="empty">
        {#if loading}<span class="spinner"></span>{:else}{rows.length && filter
            ? "No matching rows"
            : empty}{/if}
      </div>
    {/if}
  </div>
  <div class="footer">
    {#if loading && rows.length}<span class="spinner small"></span>{/if}
    <span>
      {sorted.length !== rows.length ? `${sorted.length.toLocaleString()} of ` : ""}{rows.length.toLocaleString()}
      {rows.length === 1 ? "row" : "rows"}
    </span>
    {#if selectable && selected.size}<span>· {selected.size.toLocaleString()} selected</span>{/if}
  </div>
</div>

<style>
  .wrap {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 120px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    overflow: hidden;
  }
  .scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
    position: relative;
  }
  table {
    width: 100%;
    border-collapse: separate;
    border-spacing: 0;
    table-layout: fixed;
  }
  th {
    position: sticky;
    top: 0;
    z-index: 1;
    height: 36px;
    padding: 0 12px;
    background: var(--surface-2);
    border-bottom: 1px solid var(--border);
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 600;
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    user-select: none;
  }
  th.sortable {
    cursor: pointer;
  }
  th.sortable:hover {
    color: var(--text);
  }
  .arrow {
    font-size: 9px;
    margin-left: 4px;
  }
  td {
    height: var(--row-h);
    padding: 0 12px;
    border-bottom: 1px solid var(--border);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  tr.spacer {
    border: none;
  }
  tbody tr:not(.spacer):hover td {
    background: var(--surface-hover);
  }
  tr.clickable {
    cursor: pointer;
  }
  tr.selected td,
  tr.active td {
    background: var(--accent-soft);
  }
  .check-col {
    width: 40px;
    text-align: center;
    padding: 0;
  }
  .mono {
    font-family: var(--mono);
    font-size: 12.5px;
  }
  .empty {
    display: grid;
    place-items: center;
    padding: 40px 16px;
    color: var(--text-muted);
  }
  .footer {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 12px;
    border-top: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-muted);
    font-size: 12px;
  }
  .spinner.small {
    width: 11px;
    height: 11px;
  }
</style>
