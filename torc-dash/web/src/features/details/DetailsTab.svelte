<!-- Workflow details: summary header plus sub-tabs listing jobs, results, files, events, etc. -->
<script lang="ts">
  import { api, unwrap, type JobStatus, type Schemas, type Workflow } from "../../lib/api/client";
  import { errorText } from "../../lib/format";
  import { app } from "../../lib/state.svelte";
  import type { SortState } from "../../lib/ui/DataTable.svelte";
  import SegmentedTabs from "../../lib/ui/SegmentedTabs.svelte";
  import { matchesOperator, parseOperatorFilter } from "./filter";
  import SlurmLogsModal from "./SlurmLogsModal.svelte";
  import { loadCounts, loadRows, QUICK_FILTERS, SUBTABS, type SubTab } from "./subtabs";
  import SubTabTable from "./SubTabTable.svelte";
  import WorkflowSummary from "./WorkflowSummary.svelte";

  let sub = $state<SubTab>("jobs");
  // Per-sub-tab filter text and sort, so switching tabs does not lose them.
  let filters = $state<Partial<Record<SubTab, string>>>({});
  let sorts = $state<Record<SubTab, SortState>>({
    ...(Object.fromEntries(SUBTABS.map((t) => [t.id, null])) as Record<SubTab, SortState>),
    running: { key: "start_time", dir: "asc" }, // longest-running first
  });

  let workflow = $state.raw<Workflow | null>(null);
  let status = $state.raw<Schemas["WorkflowStatusResponse"] | null>(null);
  let counts = $state.raw<Partial<Record<SubTab, number>>>({});
  let headerError = $state<string | null>(null);

  let rows = $state.raw<object[]>([]);
  let loading = $state(false);
  let rowsError = $state<string | null>(null);
  // Last rows per sub-tab of the current workflow, so switching back is instant.
  const cache = new Map<SubTab, object[]>();
  let cacheWorkflow: number | null = null;

  let slurmLogsId = $state<number | null>(null);
  let filterInput = $state<HTMLInputElement>();

  // Header + cheap counts.
  $effect(() => {
    const wf = app.workflowId;
    void app.refreshTick;
    if (wf === null) return;
    let stale = false;
    Promise.all([
      unwrap(api.GET("/workflows/{id}", { params: { path: { id: wf } } })),
      unwrap(api.GET("/workflows/{id}/status", { params: { path: { id: wf } } })).catch(() => null),
      loadCounts(wf),
    ])
      .then(([w, s, c]) => {
        if (stale) return;
        workflow = w;
        status = s;
        counts = c;
        headerError = null;
      })
      .catch((e) => {
        if (stale) return;
        workflow = null;
        headerError = errorText(e);
      });
    return () => (stale = true);
  });

  // Active sub-tab rows only.
  $effect(() => {
    const wf = app.workflowId;
    const tab = sub;
    void app.refreshTick;
    if (wf === null) return;
    if (cacheWorkflow !== wf) {
      cache.clear();
      cacheWorkflow = wf;
      workflow = null;
      status = null;
      counts = {};
    }
    rows = cache.get(tab) ?? [];
    rowsError = null;
    loading = true;
    let stale = false;
    loadRows(tab, wf)
      .then((r) => {
        if (stale) return;
        cache.set(tab, r);
        rows = r;
      })
      .catch((e) => !stale && (rowsError = errorText(e)))
      .finally(() => !stale && (loading = false));
    return () => (stale = true);
  });

  let filterText = $derived(filters[sub] ?? "");
  let operator = $derived(parseOperatorFilter(filterText));
  let shown = $derived(operator ? rows.filter((r) => matchesOperator(r, operator!)) : rows);

  let tabs = $derived(
    SUBTABS.map((t) => ({
      ...t,
      count:
        t.id === "jobs"
          ? status?.total_jobs
          : t.id === "running"
            ? status?.jobs_by_status.running
            : counts[t.id],
    })),
  );

  function showStatus(s: JobStatus) {
    filters.jobs = `status=${s}`;
    sub = "jobs";
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key !== "/" || e.metaKey || e.ctrlKey || e.altKey) return;
    if ((e.target as HTMLElement).closest("input, textarea, select, [contenteditable], dialog[open]"))
      return;
    e.preventDefault();
    filterInput?.focus();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="page">
  {#if app.workflowId === null}
    <div class="empty">Pick a workflow in the top bar to see its details.</div>
  {:else}
    {#if workflow}
      <WorkflowSummary {workflow} {status} onStatusClick={showStatus} />
    {:else if headerError}
      <div class="error-text">Could not load workflow {app.workflowId}: {headerError}</div>
    {:else}
      <div class="card header-skeleton"></div>
    {/if}

    <SegmentedTabs bind:value={sub} {tabs} />

    <div class="toolbar">
      <input
        bind:this={filterInput}
        class="input filter"
        placeholder="Filter…  e.g. name:work  status=failed  id>5  (press /)"
        value={filterText}
        oninput={(e) => (filters[sub] = e.currentTarget.value)}
        onkeydown={(e) => e.key === "Escape" && (filters[sub] = "")}
      />
      {#if filterText}
        <button class="btn btn-sm btn-ghost" onclick={() => (filters[sub] = "")}>Clear</button>
      {/if}
      {#each QUICK_FILTERS[sub] ?? [] as q (q.filter)}
        <button
          class="btn btn-sm"
          class:on={filterText === q.filter}
          title={q.title}
          onclick={() => (filters[sub] = filterText === q.filter ? "" : q.filter)}>{q.label}</button
        >
      {/each}
      {#if rowsError}<span class="error-text">Error loading {sub.replace("_", " ")}: {rowsError}</span>{/if}
    </div>

    <div class="table">
      <SubTabTable
        tab={sub}
        rows={shown}
        filter={operator ? "" : filterText}
        loading={loading}
        bind:sort={sorts[sub]}
        onSlurmLogs={(id) => (slurmLogsId = id)}
      />
    </div>

    <SlurmLogsModal
      bind:schedulerId={slurmLogsId}
      workflowId={app.workflowId}
      submissionDir={workflow?.submission_directory}
    />
  {/if}
</div>

<style>
  .header-skeleton {
    height: 92px;
  }
  .filter {
    width: min(420px, 100%);
  }
  .btn.on {
    background: var(--accent-soft);
    border-color: var(--accent);
    color: var(--accent);
  }
  .table {
    flex: 1;
    min-height: 0;
  }
</style>
