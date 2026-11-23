<!-- One DataTable per Details sub-tab, with the column set ported from legacy app-tables.js. -->
<script lang="ts">
  import type { SvelteSet } from "svelte/reactivity";
  import type {
    ComputeNode,
    FileRecord,
    Job,
    ResourceRequirements,
    Result,
    ScheduledComputeNode,
    Schemas,
    SlurmScheduler,
    TorcEvent,
    UserData,
  } from "../../lib/api/client";
  import {
    formatBytes,
    formatDateTime,
    formatDuration,
    formatPercent,
    toDate,
    type Tone,
  } from "../../lib/format";
  import { app } from "../../lib/state.svelte";
  import Badge from "../../lib/ui/Badge.svelte";
  import DataTable, { type Column, type SortState } from "../../lib/ui/DataTable.svelte";
  import JsonView from "./JsonView.svelte";
  import { eventMessage, type SlurmStatRow, type SubTab } from "./subtabs";

  type RunningJob = Schemas["RunningJobModel"];

  let {
    tab,
    rows,
    filter,
    loading,
    sort = $bindable(),
    onSlurmLogs,
    onEdit,
    selectedJobs,
    onCellFilter,
  }: {
    tab: SubTab;
    rows: object[];
    filter: string;
    loading: boolean;
    sort: SortState;
    onSlurmLogs: (schedulerId: number) => void;
    /** Clicking a resource requirement or Slurm scheduler row edits it. */
    onEdit: (
      target:
        | { kind: "resources"; record: ResourceRequirements }
        | { kind: "scheduler"; record: SlurmScheduler },
    ) => void;
    /** Selection for the Jobs table. */
    selectedJobs: SvelteSet<string | number>;
    onCellFilter: (key: string, value: string | number | boolean) => void;
  } = $props();

  const elapsed = (start: string | null | undefined) => {
    const d = toDate(start);
    return d ? formatDuration((Date.now() - d.getTime()) / 1000) : "—";
  };
  const fixed = (v: number | null | undefined, digits: number) =>
    v === null || v === undefined ? "—" : v.toFixed(digits);
  const positive = (v: number | null | undefined) => (v && v > 0 ? v : null);

  function nodeTone(status: string): Tone {
    const s = status.toLowerCase();
    if (s === "running" || s === "active") return "accent";
    if (s === "pending" || s === "scheduled") return "info";
    if (s === "completed" || s === "done" || s === "complete") return "success";
    if (s === "failed" || s === "error") return "danger";
    return "neutral";
  }

  const isSlurm = (n: ScheduledComputeNode) => n.scheduler_type.toLowerCase() === "slurm";

  const jobCols: Column<Job>[] = [
    { key: "id", label: "ID", width: "80px", mono: true },
    { key: "name", label: "Name", width: "22%" },
    { key: "status", label: "Status", width: "130px", cell: jobStatus },
    { key: "compute_node_id", label: "Node", width: "70px", mono: true },
    {
      key: "start_time",
      label: "Elapsed",
      width: "100px",
      mono: true,
      value: (j) => (j.status === "running" ? j.start_time : null),
      cell: jobElapsed,
    },
    { key: "command", label: "Command", mono: true },
  ];

  const runningCols: Column<RunningJob>[] = [
    { key: "job_id", label: "Job ID", width: "80px", mono: true },
    { key: "job_name", label: "Name" },
    { key: "compute_node_name", label: "Compute Node" },
    { key: "start_time", label: "Elapsed", width: "110px", mono: true, cell: runningElapsed },
    { key: "scheduler_type", label: "Scheduler", width: "110px" },
    { key: "scheduler_job_id", label: "Scheduler Job ID", width: "150px", mono: true },
  ];

  const resultCols: Column<Result>[] = [
    { key: "job_id", label: "Job ID", width: "80px", mono: true },
    { key: "job_name", label: "Job Name" },
    { key: "run_id", label: "Run", width: "60px", align: "right" },
    { key: "attempt_id", label: "Attempt", width: "75px", align: "right", value: (r) => r.attempt_id ?? 1 },
    { key: "return_code", label: "Return", width: "75px", align: "right", cell: returnCode },
    { key: "status", label: "Status", width: "130px", cell: resultStatus },
    { key: "exec_time_minutes", label: "Exec Time (min)", width: "130px", align: "right", cell: execTime },
    { key: "peak_memory_bytes", label: "Peak Mem", width: "100px", align: "right", cell: peakMem },
    { key: "peak_cpu_percent", label: "Peak CPU %", width: "100px", align: "right", cell: peakCpu },
  ];

  const eventCols: Column<TorcEvent>[] = [
    { key: "id", label: "ID", width: "70px", mono: true },
    { key: "timestamp", label: "Timestamp", width: "190px", cell: eventTime },
    { key: "message", label: "Message", value: (e) => eventMessage(e.data) },
    { key: "data", label: "Data", mono: true, value: (e) => JSON.stringify(e.data) },
  ];

  const fileCols: Column<FileRecord>[] = [
    { key: "id", label: "ID", width: "80px", mono: true },
    { key: "name", label: "Name", width: "25%" },
    { key: "path", label: "Path", mono: true },
    { key: "st_mtime", label: "Modified", width: "190px", cell: fileTime },
  ];

  const userDataCols: Column<UserData>[] = [
    { key: "id", label: "ID", width: "80px", mono: true },
    { key: "name", label: "Name", width: "25%" },
    { key: "data", label: "Data", mono: true, value: (u) => JSON.stringify(u.data) },
  ];

  const resourceCols: Column<ResourceRequirements>[] = [
    { key: "id", label: "ID", width: "80px", mono: true },
    { key: "name", label: "Name" },
    { key: "num_cpus", label: "CPUs", width: "80px", align: "right" },
    { key: "memory", label: "Memory", width: "100px", align: "right" },
    { key: "num_gpus", label: "GPUs", width: "80px", align: "right" },
    { key: "num_nodes", label: "Nodes", width: "80px", align: "right" },
    { key: "runtime", label: "Runtime", width: "120px", mono: true },
  ];

  const schedulerCols: Column<SlurmScheduler>[] = [
    { key: "id", label: "ID", width: "80px", mono: true },
    { key: "name", label: "Name" },
    { key: "account", label: "Account" },
    { key: "partition", label: "Partition" },
    { key: "qos", label: "QoS", width: "100px" },
    { key: "walltime", label: "Walltime", width: "110px", mono: true },
    { key: "nodes", label: "Nodes", width: "70px", align: "right" },
    { key: "mem", label: "Mem", width: "90px" },
    { key: "gres", label: "GRES", width: "110px" },
  ];

  const computeNodeCols: Column<ComputeNode>[] = [
    { key: "id", label: "ID", width: "70px", mono: true },
    { key: "hostname", label: "Hostname" },
    { key: "compute_node_type", label: "Type", width: "80px" },
    { key: "num_cpus", label: "CPUs", width: "70px", align: "right" },
    { key: "memory_gb", label: "Mem (GB)", width: "90px", align: "right" },
    { key: "num_gpus", label: "GPUs", width: "70px", align: "right" },
    { key: "is_active", label: "Active", width: "70px", value: (n) => (n.is_active == null ? null : n.is_active ? "Yes" : "No") },
    { key: "start_time", label: "Started", width: "180px", cell: nodeStart },
    { key: "peak_cpu_percent", label: "CPU peak / avg", width: "130px", align: "right", cell: nodeCpu },
    { key: "peak_memory_bytes", label: "Mem peak / avg", width: "150px", align: "right", cell: nodeMem },
  ];

  const scheduledCols: Column<ScheduledComputeNode>[] = [
    { key: "id", label: "ID", width: "80px", mono: true },
    { key: "scheduler_id", label: "Scheduler ID", mono: true },
    { key: "scheduler_config_id", label: "Config ID", mono: true },
    { key: "scheduler_type", label: "Type" },
    { key: "status", label: "Status", width: "130px", cell: nodeStatus },
    { key: "logs", label: "Logs", width: "110px", sortable: false, value: () => null, cell: slurmLogs },
  ];

  const slurmStatCols: Column<SlurmStatRow>[] = [
    { key: "job_id", label: "Job ID", width: "80px", mono: true },
    { key: "run_id", label: "Run", width: "60px", align: "right" },
    { key: "attempt_id", label: "Attempt", width: "75px", align: "right" },
    { key: "slurm_job_id", label: "Slurm Job", mono: true },
    { key: "max_rss_bytes", label: "Max RSS", align: "right", value: (s) => positive(s.max_rss_bytes), cell: maxRss },
    { key: "max_vm_size_bytes", label: "Max VM", align: "right", value: (s) => positive(s.max_vm_size_bytes), cell: maxVm },
    { key: "ave_cpu_seconds", label: "Ave CPU (s)", align: "right", value: (s) => positive(s.ave_cpu_seconds), cell: aveCpu },
    { key: "cpu_percent", label: "CPU %", align: "right", cell: cpuPct },
    { key: "node_list", label: "Nodes" },
  ];

  let selectedEvent = $state<TorcEvent | null>(null);
</script>

{#snippet jobStatus(j: Job)}<Badge status={j.status} />{/snippet}
{#snippet jobElapsed(j: Job)}{j.status === "running" ? elapsed(j.start_time) : "—"}{/snippet}
{#snippet runningElapsed(j: RunningJob)}{elapsed(j.start_time)}{/snippet}
{#snippet returnCode(r: Result)}<span class:ok={r.return_code === 0} class:bad={r.return_code !== 0}
    >{r.return_code}</span
  >{/snippet}
{#snippet resultStatus(r: Result)}<Badge status={r.status} />{/snippet}
{#snippet execTime(r: Result)}{fixed(r.exec_time_minutes, 2)}{/snippet}
{#snippet peakMem(r: Result)}{formatBytes(r.peak_memory_bytes)}{/snippet}
{#snippet peakCpu(r: Result)}{fixed(r.peak_cpu_percent, 1)}{/snippet}
{#snippet eventTime(e: TorcEvent)}{formatDateTime(e.timestamp)}{/snippet}
{#snippet fileTime(f: FileRecord)}{formatDateTime(f.st_mtime)}{/snippet}
{#snippet nodeStart(n: ComputeNode)}{formatDateTime(n.start_time)}{/snippet}
{#snippet nodeCpu(n: ComputeNode)}{n.peak_cpu_percent != null && n.avg_cpu_percent != null
    ? `${formatPercent(n.peak_cpu_percent)} / ${formatPercent(n.avg_cpu_percent)}`
    : "—"}{/snippet}
{#snippet nodeMem(n: ComputeNode)}{n.peak_memory_bytes != null && n.avg_memory_bytes != null
    ? `${formatBytes(n.peak_memory_bytes)} / ${formatBytes(n.avg_memory_bytes)}`
    : "—"}{/snippet}
{#snippet nodeStatus(n: ScheduledComputeNode)}<Badge tone={nodeTone(n.status)}>{n.status}</Badge
  >{/snippet}
{#snippet slurmLogs(n: ScheduledComputeNode)}
  {#if isSlurm(n)}
    <button
      class="btn btn-sm"
      onclick={(e) => {
        e.stopPropagation();
        onSlurmLogs(n.scheduler_id);
      }}>View logs</button
    >
  {:else}<span class="faint">—</span>{/if}
{/snippet}
{#snippet maxRss(s: SlurmStatRow)}{formatBytes(positive(s.max_rss_bytes))}{/snippet}
{#snippet maxVm(s: SlurmStatRow)}{formatBytes(positive(s.max_vm_size_bytes))}{/snippet}
{#snippet aveCpu(s: SlurmStatRow)}{fixed(positive(s.ave_cpu_seconds), 1)}{/snippet}
{#snippet cpuPct(s: SlurmStatRow)}{formatPercent(s.cpu_percent)}{/snippet}

{#if tab === "jobs"}
  <DataTable
    rows={rows as Job[]}
    columns={jobCols}
    rowKey={(j) => j.id!}
    {filter}
    {onCellFilter}
    {loading}
    bind:sort
    selectable
    selected={selectedJobs}
    empty="No jobs in this workflow"
    onRowClick={(j) => app.openJob(j.id!)}
  />
{:else if tab === "running"}
  <DataTable
    rows={rows as RunningJob[]}
    columns={runningCols}
    rowKey={(j) => j.job_id}
    {filter}
    {onCellFilter}
    {loading}
    bind:sort
    empty="No running jobs"
    onRowClick={(j) => app.openJob(j.job_id)}
  />
{:else if tab === "results"}
  <DataTable
    rows={rows as Result[]}
    columns={resultCols}
    rowKey={(r) => r.id ?? `${r.job_id}_${r.run_id}_${r.attempt_id}`}
    {filter}
    {onCellFilter}
    {loading}
    bind:sort
    empty="No results in this workflow"
    onRowClick={(r) => app.openJob(r.job_id)}
  />
{:else if tab === "events"}
  <div class="split">
    <div class="grow">
      <DataTable
        rows={rows as TorcEvent[]}
        columns={eventCols}
        rowKey={(e) => e.id!}
        {filter}
    {onCellFilter}
        {loading}
        bind:sort
        empty="No events in this workflow"
        activeKey={selectedEvent?.id ?? null}
        onRowClick={(e) => (selectedEvent = e)}
      />
    </div>
    <aside class="preview card">
      <header>Event data</header>
      {#if selectedEvent}
        <JsonView json={JSON.stringify(selectedEvent.data, null, 2)} />
      {:else}
        <div class="empty">Select an event to see its data</div>
      {/if}
    </aside>
  </div>
{:else if tab === "files"}
  <DataTable
    rows={rows as FileRecord[]}
    columns={fileCols}
    rowKey={(f) => f.id!}
    {filter}
    {onCellFilter}
    {loading}
    bind:sort
    empty="No files in this workflow"
    onRowClick={(f) => app.openFile(f.path, f.name)}
  />
{:else if tab === "user_data"}
  <DataTable
    rows={rows as UserData[]}
    columns={userDataCols}
    rowKey={(u) => u.id!}
    {filter}
    {onCellFilter}
    {loading}
    bind:sort
    empty="No user data in this workflow"
  />
{:else if tab === "resources"}
  <DataTable
    rows={rows as ResourceRequirements[]}
    columns={resourceCols}
    rowKey={(r) => r.id!}
    {filter}
    {onCellFilter}
    {loading}
    bind:sort
    onRowClick={(r) => onEdit({ kind: "resources", record: r })}
    empty="No resource requirements in this workflow"
  />
{:else if tab === "schedulers"}
  <DataTable
    rows={rows as SlurmScheduler[]}
    columns={schedulerCols}
    rowKey={(s) => s.id!}
    {filter}
    {onCellFilter}
    {loading}
    bind:sort
    onRowClick={(s) => onEdit({ kind: "scheduler", record: s })}
    empty="No Slurm schedulers configured for this workflow"
  />
{:else if tab === "compute_nodes"}
  <DataTable
    rows={rows as ComputeNode[]}
    columns={computeNodeCols}
    rowKey={(n) => n.id!}
    {filter}
    {onCellFilter}
    {loading}
    bind:sort
    empty="No compute nodes in this workflow"
  />
{:else if tab === "scheduled_nodes"}
  <DataTable
    rows={rows as ScheduledComputeNode[]}
    columns={scheduledCols}
    rowKey={(n) => n.id!}
    {filter}
    {onCellFilter}
    {loading}
    bind:sort
    empty="No scheduled compute nodes in this workflow"
    onRowClick={(n) => isSlurm(n) && onSlurmLogs(n.scheduler_id)}
  />
{:else if tab === "slurm_stats"}
  <DataTable
    rows={rows as SlurmStatRow[]}
    columns={slurmStatCols}
    rowKey={(s) => s.id ?? `${s.job_id}_${s.run_id}_${s.attempt_id}`}
    {filter}
    {onCellFilter}
    {loading}
    bind:sort
    empty="No Slurm stats in this workflow"
    onRowClick={(s) => app.openJob(s.job_id)}
  />
{/if}

<style>
  .ok {
    color: var(--success);
    font-weight: 600;
  }
  .bad {
    color: var(--danger);
    font-weight: 600;
  }
  .split {
    display: flex;
    gap: 12px;
    height: 100%;
    min-height: 0;
  }
  .split .grow {
    height: 100%;
  }
  .preview {
    width: min(420px, 38%);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .preview header {
    padding: 9px 14px;
    border-bottom: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 600;
  }
</style>
