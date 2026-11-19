<!-- Debugging: job results with stdout/stderr, Slurm log analysis, Slurm accounting. -->
<script lang="ts">
  import { untrack } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import { api, listAll, unwrap, type Job, type Result, type Workflow } from "../../lib/api/client";
  import { app } from "../../lib/state.svelte";
  import { errorText } from "../../lib/format";
  import Badge from "../../lib/ui/Badge.svelte";
  import DataTable, { type Column } from "../../lib/ui/DataTable.svelte";
  import SegmentedTabs from "../../lib/ui/SegmentedTabs.svelte";
  import LiveTail from "./LiveTail.svelte";
  import LogViewer from "./LogViewer.svelte";
  import SlurmLogs from "./SlurmLogs.svelte";
  import SlurmSacct from "./SlurmSacct.svelte";
  import { loadOutputDir, resolveLogBaseDir, saveOutputDir, stdioPaths } from "./logs";

  type Row = {
    key: string;
    job: Job;
    result: Result | null;
    stdout: string | null;
    stderr: string | null;
  };

  let view = $state<"jobs" | "tail" | "slurm-logs" | "sacct">("jobs");
  /** Jobs followed in Live tail; kept here so switching views doesn't drop them. */
  const tailed = new SvelteSet<string | number>();
  let outputDir = $state(loadOutputDir());
  let failedOnly = $state(true);
  let allRuns = $state(false);
  let filter = $state("");
  let filterInput = $state<HTMLInputElement>();

  let jobs = $state<Job[] | null>(null);
  let results = $state<Result[]>([]);
  let wf = $state<Workflow | null>(null);
  let submissionDir = $derived(wf?.submission_directory ?? null);
  let loadError = $state<string | null>(null);
  let loadedFor = $state<number | null>(null);
  let selectedKey = $state<string | null>(null);

  $effect(() => saveOutputDir(outputDir));

  $effect(() => {
    const id = app.workflowId;
    const all = allRuns;
    void app.refreshTick;
    if (id === null) return;
    untrack(() => {
      if (id !== loadedFor) {
        jobs = null;
        // Drop the old submission directory so Slurm views can't resolve paths against it.
        wf = null;
        loadedFor = null;
        loadError = null;
        selectedKey = null;
        tailed.clear();
      }
      load(id, all);
    });
  });

  async function load(id: number, all: boolean) {
    try {
      // Set wf as soon as it arrives: the Slurm views only need it, not the job/result listings.
      const wfReq = unwrap(api.GET("/workflows/{id}", { params: { path: { id } } })).then((w) => {
        if (id === app.workflowId) wf = w;
      });
      const [j, r] = await Promise.all([
        listAll((offset, limit) =>
          unwrap(api.GET("/jobs", { params: { query: { workflow_id: id, offset, limit } } })),
        ),
        listAll((offset, limit) =>
          unwrap(
            api.GET("/results", {
              params: { query: { workflow_id: id, offset, limit, all_runs: all } },
            }),
          ),
        ),
        wfReq,
      ]);
      if (id !== app.workflowId) return;
      jobs = j;
      results = r;
      loadedFor = id;
      loadError = null;
    } catch (e) {
      if (id === app.workflowId) loadError = errorText(e);
    }
  }

  // Live tail follows each job's current attempt (attempt_id, run_id). Refresh just the tailed
  // jobs on a short cadence so a retry is picked up without waiting for the dashboard refresh.
  $effect(() => {
    const id = app.workflowId;
    if (view !== "tail" || !tailed.size || id === null) return;
    const timer = setInterval(() => refreshTailed(id), 5000);
    return () => clearInterval(timer);
  });

  async function refreshTailed(id: number) {
    if (document.visibilityState !== "visible") return;
    try {
      const [w, js] = await Promise.all([
        unwrap(api.GET("/workflows/{id}", { params: { path: { id } } })),
        Promise.all(
          [...tailed].map((jid) =>
            unwrap(api.GET("/jobs/{id}", { params: { path: { id: Number(jid) } } })),
          ),
        ),
      ]);
      if (id !== app.workflowId || !jobs) return;
      const byId = new Map(js.map((j) => [j.id, j]));
      jobs = jobs.map((j) => byId.get(j.id) ?? j);
      wf = w;
    } catch {
      // The full refresh reports errors; panes show their own.
    }
  }

  let baseDir = $derived(resolveLogBaseDir(outputDir.trim() || "torc_output", submissionDir));

  let rows = $derived.by((): Row[] => {
    if (!jobs) return [];
    const base = baseDir;
    const byJob = new Map<number, Result[]>();
    for (const r of results) byJob.set(r.job_id, [...(byJob.get(r.job_id) ?? []), r]);
    const out: Row[] = [];
    for (const job of jobs) {
      const rs = byJob.get(job.id!) ?? [];
      if (rs.length === 0) {
        if (!failedOnly) out.push({ key: `${job.id}`, job, result: null, stdout: null, stderr: null });
        continue;
      }
      for (const r of allRuns ? rs : rs.slice(-1)) {
        if (failedOnly && r.return_code === 0) continue;
        const p = stdioPaths(base, r);
        out.push({ key: `${job.id}/${r.id}`, job, result: r, ...p });
      }
    }
    return out;
  });

  // Keep a selection so the log viewer always has something useful to show.
  let selected = $derived(rows.find((r) => r.key === selectedKey) ?? rows[0] ?? null);

  const columns: Column<Row>[] = [
    { key: "id", label: "ID", width: "70px", mono: true, value: (r) => r.job.id },
    { key: "name", label: "Name", value: (r) => r.job.name },
    {
      key: "status",
      label: "Status",
      width: "115px",
      value: (r) => r.result?.status ?? r.job.status,
      cell: statusCell,
    },
    {
      key: "attempt",
      label: "Run/Att",
      width: "75px",
      mono: true,
      value: (r) => (r.result ? `${r.result.run_id}/${r.result.attempt_id ?? 1}` : null),
    },
    {
      key: "rc",
      label: "RC",
      width: "60px",
      align: "right",
      value: (r) => r.result?.return_code,
      cell: rcCell,
    },
    { key: "open", label: "", width: "64px", sortable: false, value: () => "", cell: openCell },
  ];

  function onKey(e: KeyboardEvent) {
    const t = e.target as HTMLElement;
    if (e.key !== "/" || t.closest("input, textarea, select, [contenteditable]")) return;
    e.preventDefault();
    filterInput?.focus();
  }
</script>

<svelte:window onkeydown={onKey} />

{#snippet statusCell(r: Row)}<Badge status={r.result?.status ?? r.job.status} />{/snippet}
{#snippet rcCell(r: Row)}
  <span class="mono" class:bad={r.result && r.result.return_code !== 0}>
    {r.result?.return_code ?? "—"}
  </span>
{/snippet}
{#snippet openCell(r: Row)}
  <button
    class="btn btn-ghost btn-sm"
    onclick={(e) => {
      e.stopPropagation();
      app.openJob(r.job.id!);
    }}>Details</button
  >
{/snippet}

<div class="page">
  <div class="page-header">
    <h2>Debugging</h2>
    <label class="field inline">
      <span>Output dir</span>
      <input class="input mono" bind:value={outputDir} placeholder="torc_output" spellcheck="false" />
    </label>
  </div>

  {#if app.workflowId === null}
    <div class="empty">Pick a workflow in the top bar to debug its jobs.</div>
  {:else}
    <div class="toolbar">
      <SegmentedTabs
        bind:value={view}
        tabs={[
          { id: "jobs", label: "Job logs", count: jobs ? rows.length : null },
          { id: "tail", label: "Live tail", count: tailed.size || null },
          { id: "slurm-logs", label: "Slurm log analysis" },
          { id: "sacct", label: "Slurm accounting" },
        ]}
      />
      {#if view === "jobs" || view === "tail"}
        <div class="grow"></div>
      {/if}
      {#if view === "jobs"}
        <label class="check"><input type="checkbox" bind:checked={failedOnly} /> Failed only</label>
        <label class="check"><input type="checkbox" bind:checked={allRuns} /> All runs</label>
      {/if}
      {#if view === "jobs" || view === "tail"}
        <input
          class="input filter"
          bind:this={filterInput}
          bind:value={filter}
          placeholder="Filter…  /"
        />
      {/if}
    </div>

    {#if view === "jobs"}
      {#if loadError && !jobs}
        <p class="error-text">{loadError}</p>
      {:else}
        <div class="split">
          <div class="pane table">
            <DataTable
              {rows}
              {columns}
              {filter}
              rowKey={(r) => r.key}
              activeKey={selected?.key ?? null}
              onRowClick={(r) => (selectedKey = r.key)}
              loading={!jobs}
              empty={failedOnly
                ? "No failed jobs. Uncheck “Failed only” to see all jobs."
                : "No jobs in this workflow."}
            />
          </div>
          <div class="pane logs card">
            {#if selected?.stdout && selected.stderr}
              <div class="logs-head">
                <strong>{selected.job.name}</strong>
                <span class="faint mono">#{selected.job.id}</span>
              </div>
              <LogViewer stdout={selected.stdout} stderr={selected.stderr} />
            {:else if selected}
              <div class="empty">
                <strong>{selected.job.name}</strong> has no results yet, so there are no logs.
              </div>
            {:else}
              <div class="empty">Select a job to view its logs.</div>
            {/if}
          </div>
        </div>
      {/if}
    {:else if view === "tail"}
      <LiveTail {jobs} {wf} {baseDir} {filter} selected={tailed} />
    {:else if wf?.id !== app.workflowId}
      <!-- Slurm paths depend on this workflow's submission directory; wait until it's loaded. -->
      {#if loadError}
        <p class="error-text">{loadError}</p>
      {:else}
        <div class="empty"><span class="spinner"></span> Loading workflow…</div>
      {/if}
    {:else if view === "slurm-logs"}
      <SlurmLogs workflowId={app.workflowId} outputDir={baseDir} />
    {:else}
      <SlurmSacct workflowId={app.workflowId} outputDir={baseDir} />
    {/if}
  {/if}
</div>

<style>
  .inline {
    flex-direction: row;
    align-items: center;
    gap: 8px;
  }
  .inline .input {
    width: 240px;
  }
  .filter {
    width: 200px;
  }
  .split {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.25fr);
    gap: 16px;
  }
  .pane {
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .logs {
    padding: 12px;
    gap: 8px;
  }
  .logs-head {
    display: flex;
    gap: 8px;
    align-items: baseline;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .bad {
    color: var(--danger);
    font-weight: 600;
  }
  @media (max-width: 1100px) {
    .split {
      grid-template-columns: 1fr;
      grid-template-rows: minmax(240px, 1fr) minmax(320px, 1.2fr);
    }
  }
</style>
