<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import { dashPost } from "../../lib/api/dash";
  import { errorText, formatBytes } from "../../lib/format";
  import { app } from "../../lib/state.svelte";
  import PlotView, { type Figure } from "./PlotView.svelte";

  type ResourceDb = {
    path: string;
    name: string;
    size_bytes: number;
    modified: string;
    workflow_id: number | null;
  };
  type Plot = { name: string; data: Figure | null };

  let baseDir = $state("torc_output/resource_utilization");
  let dbs = $state.raw<ResourceDb[] | null>(null);
  let scanning = $state(false);
  let scanError = $state<string | null>(null);
  let showAll = $state(false);
  const selected = new SvelteSet<string>();

  let plots = $state.raw<Plot[] | null>(null);
  let plotIndex = $state(0);
  let generating = $state(false);
  let plotError = $state<string | null>(null);

  let visible = $derived(
    !dbs
      ? []
      : app.workflowId === null || showAll
        ? dbs
        : dbs.filter((d) => d.workflow_id === app.workflowId),
  );
  let selectedPaths = $derived(visible.filter((d) => selected.has(d.path)).map((d) => d.path));
  let current = $derived(plots?.[plotIndex] ?? null);

  async function scan() {
    scanning = true;
    try {
      const res = await dashPost<{ success: boolean; databases?: ResourceDb[]; error?: string }>(
        "/api/cli/list-resource-dbs",
        { base_dir: baseDir },
      );
      if (!res.success) throw new Error(res.error || "Scan failed");
      dbs = res.databases ?? [];
      scanError = null;
      resetForWorkflow();
    } catch (e) {
      scanError = errorText(e);
    } finally {
      scanning = false;
    }
  }

  /** Clear selection/plots; if exactly one database matches, select it and plot right away. */
  function resetForWorkflow() {
    selected.clear();
    plots = null;
    plotError = null;
    if (visible.length === 1) {
      selected.add(visible[0].path);
      void generate();
    }
  }

  $effect(() => {
    void app.workflowId;
    untrack(() => {
      showAll = false;
      if (dbs) resetForWorkflow();
    });
  });

  function friendlyName(filename: string): string {
    const name = filename.replace(/^resource_plot_?/, "").replace(/\.json$/, "");
    if (!name) return "Summary";
    return name
      .split("_")
      .map((w) => (/^(cpu|gpu)$/i.test(w) ? w.toUpperCase() : w.charAt(0).toUpperCase() + w.slice(1)))
      .join(" ");
  }

  /** Overview plots first, then per-job plots in numeric order. */
  function plotRank(name: string): number {
    if (/summary/.test(name)) return 0;
    if (/all_jobs/.test(name)) return 1;
    return 2;
  }
  const collator = new Intl.Collator(undefined, { numeric: true });

  async function generate() {
    const paths = selectedPaths;
    if (paths.length === 0) return;
    generating = true;
    plotError = null;
    try {
      const res = await dashPost<{ success: boolean; plots?: Plot[]; error?: string }>(
        "/api/cli/plot-resources",
        { db_paths: paths, prefix: "resource_plot" },
      );
      if (!res.success) throw new Error(res.error || "Plot generation failed");
      plots = (res.plots ?? []).toSorted(
        (a, b) => plotRank(a.name) - plotRank(b.name) || collator.compare(a.name, b.name),
      );
      plotIndex = 0;
    } catch (e) {
      plotError = errorText(e);
    } finally {
      generating = false;
    }
  }

  function toggle(path: string, on: boolean) {
    if (on) selected.add(path);
    else selected.delete(path);
  }

  onMount(() => {
    void scan();
  });
</script>

<div class="page">
  <div class="page-header">
    <h2>Resource plots</h2>
  </div>

  <div class="layout">
    <aside class="side">
      <section class="card">
        <div class="card-body stack">
          <form
            class="scan"
            onsubmit={(e) => {
              e.preventDefault();
              void scan();
            }}
          >
            <label class="field grow">
              <span>Database directory</span>
              <input class="input" bind:value={baseDir} placeholder="torc_output/resource_utilization" />
            </label>
            <button class="btn" type="submit" disabled={scanning}>
              {#if scanning}<span class="spinner"></span>{/if}Scan
            </button>
          </form>

          {#if scanError}
            <div class="error-text">{scanError}</div>
          {:else if dbs === null}
            <div class="muted"><span class="spinner"></span> Scanning…</div>
          {:else if dbs.length === 0}
            <div class="muted">No resource databases found in this directory.</div>
          {:else}
            <div class="db-head">
              <span class="muted">
                {#if app.workflowId === null}
                  All databases
                {:else if showAll}
                  All workflows
                {:else}
                  Workflow {app.workflowId}
                {/if}
              </span>
              {#if app.workflowId !== null}
                <label class="check">
                  <input type="checkbox" bind:checked={showAll} /> Show all
                </label>
              {/if}
            </div>
            {#if visible.length === 0}
              <div class="muted">No databases for workflow {app.workflowId}.</div>
            {:else}
              <div class="db-list">
                {#each visible as db (db.path)}
                  <label class="db" class:on={selected.has(db.path)}>
                    <input
                      type="checkbox"
                      checked={selected.has(db.path)}
                      onchange={(e) => toggle(db.path, e.currentTarget.checked)}
                    />
                    <span class="db-info">
                      <span class="db-name" title={db.path}>{db.name}</span>
                      <span class="faint">
                        {db.workflow_id ? `Workflow ${db.workflow_id}` : "Workflow unknown"} ·
                        {formatBytes(db.size_bytes)} · {db.modified}
                      </span>
                    </span>
                  </label>
                {/each}
              </div>
            {/if}
            <button
              class="btn btn-primary"
              disabled={selectedPaths.length === 0 || generating}
              onclick={generate}
            >
              {#if generating}<span class="spinner"></span> Generating…{:else}Generate plots{/if}
            </button>
          {/if}
        </div>
      </section>

      {#if plots && plots.length > 0}
        <section class="card plot-list">
          {#each plots as p, i (p.name)}
            <button class="plot-item" class:active={i === plotIndex} onclick={() => (plotIndex = i)}>
              {friendlyName(p.name)}
            </button>
          {/each}
        </section>
      {/if}
    </aside>

    <section class="card main">
      {#if plotError}
        <div class="empty error-text">{plotError}</div>
      {:else if plots === null}
        <div class="empty">
          {#if generating}
            <span class="spinner"></span>
          {:else}
            <div>Select one or more resource databases and click <strong>Generate plots</strong>.</div>
          {/if}
        </div>
      {:else if plots.length === 0}
        <div class="empty">No plots generated. The database may not contain any resource data.</div>
      {:else if current?.data}
        <div class="chart" class:stale={generating}><PlotView figure={current.data} /></div>
      {:else}
        <div class="empty">No data available for this plot.</div>
      {/if}
    </section>
  </div>
</div>

<style>
  .layout {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(260px, 340px) 1fr;
    gap: 16px;
  }
  @media (max-width: 900px) {
    .layout {
      grid-template-columns: 1fr;
    }
  }
  .side {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-height: 0;
  }
  .stack {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .scan {
    display: flex;
    align-items: flex-end;
    gap: 8px;
  }
  .db-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 12.5px;
  }
  .db-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-height: 280px;
    overflow: auto;
  }
  .db {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    cursor: pointer;
  }
  .db:hover {
    background: var(--surface-hover);
  }
  .db.on {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .db input {
    margin-top: 3px;
  }
  .db-info {
    display: flex;
    flex-direction: column;
    min-width: 0;
    font-size: 12px;
  }
  .db-name {
    font-family: var(--mono);
    font-size: 12.5px;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .plot-list {
    display: flex;
    flex-direction: column;
    padding: 6px;
    overflow: auto;
    min-height: 0;
  }
  .plot-item {
    text-align: left;
    padding: 6px 10px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text);
    font: inherit;
    cursor: pointer;
  }
  .plot-item:hover {
    background: var(--surface-hover);
  }
  .plot-item.active {
    background: var(--accent-soft);
    color: var(--accent);
    font-weight: 600;
  }
  .main {
    display: flex;
    flex-direction: column;
    min-height: 420px;
    min-width: 0;
    overflow: hidden;
  }
  .chart {
    flex: 1;
    display: flex;
    flex-direction: column;
    padding: 8px;
    transition: opacity 0.15s;
  }
  .chart.stale {
    opacity: 0.5;
  }
</style>
