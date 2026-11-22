<!-- Details header: workflow identity, actions, and a clickable job-status breakdown. -->
<script lang="ts">
  import type { JobStatus, Schemas, Workflow } from "../../lib/api/client";
  import { formatDateTime, formatDuration, JOB_STATUS_TONE, JOB_STATUSES, statusLabel } from "../../lib/format";
  import { app } from "../../lib/state.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import WorkflowActions from "../workflows/WorkflowActions.svelte";

  let {
    workflow,
    status,
    onStatusClick,
  }: {
    workflow: Workflow;
    status: Schemas["WorkflowStatusResponse"] | null;
    onStatusClick: (s: JobStatus) => void;
  } = $props();

  const TONE_VAR = {
    neutral: "var(--border-strong)",
    info: "var(--info)",
    accent: "var(--accent)",
    success: "var(--success)",
    warning: "var(--warning)",
    danger: "var(--danger)",
  };

  let counts = $derived(
    status
      ? JOB_STATUSES.map((s) => ({ s, n: status.jobs_by_status[s] ?? 0 })).filter((c) => c.n > 0)
      : [],
  );
</script>

<header class="card">
  <div class="top">
    <div class="title grow">
      <h2>{workflow.name}</h2>
      <div class="meta muted">
        <span>#{workflow.id}</span>
        <span>{workflow.user}</span>
        {#if workflow.project}<span>{workflow.project}</span>{/if}
        <span title="Created">{formatDateTime(workflow.timestamp)}</span>
        {#if status?.is_complete}<span class="done">complete</span>{/if}
        {#if status?.is_canceled}<span class="canceled">canceled</span>{/if}
      </div>
      {#if workflow.description}<p class="desc">{workflow.description}</p>{/if}
    </div>
    <div class="toolbar">
      <button class="btn btn-sm" onclick={() => app.go("dag")} title="Show the job graph">
        <Icon name="graph" size={14} /> Graph
      </button>
      <WorkflowActions workflowId={workflow.id!} archived={!!workflow.is_archived} />
    </div>
  </div>

  {#if status && status.total_jobs > 0}
    <div class="bar" role="img" aria-label="Job status breakdown">
      {#each counts as c (c.s)}
        <span
          style:flex-grow={c.n}
          style:background={TONE_VAR[JOB_STATUS_TONE[c.s]]}
          title="{statusLabel(c.s)}: {c.n}"
        ></span>
      {/each}
    </div>
    <div class="legend">
      <span class="muted">{status.total_jobs.toLocaleString()} jobs</span>
      {#each counts as c (c.s)}
        <button class="chip" onclick={() => onStatusClick(c.s)} title="Show {statusLabel(c.s)} jobs">
          <i style:background={TONE_VAR[JOB_STATUS_TONE[c.s]]}></i>
          {statusLabel(c.s)} <b>{c.n.toLocaleString()}</b>
        </button>
      {/each}
      {#if status.total_exec_time_minutes > 0}
        <span class="muted right">
          Exec time {formatDuration(status.total_exec_time_minutes * 60)}
          {#if status.walltime_seconds}· walltime {formatDuration(status.walltime_seconds)}{/if}
        </span>
      {/if}
    </div>
  {/if}
</header>

<style>
  header {
    padding: 16px 18px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .top {
    display: flex;
    gap: 16px;
    align-items: flex-start;
    flex-wrap: wrap;
  }
  h2 {
    font-size: 18px;
    word-break: break-word;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 14px;
    font-size: 12.5px;
    margin-top: 2px;
  }
  .done {
    color: var(--success);
    font-weight: 600;
  }
  .canceled {
    color: var(--warning);
    font-weight: 600;
  }
  .desc {
    margin: 6px 0 0;
    color: var(--text-muted);
    font-size: 13px;
  }
  .bar {
    display: flex;
    gap: 2px;
    height: 8px;
    border-radius: 999px;
    overflow: hidden;
    background: var(--neutral-soft);
  }
  .bar span {
    min-width: 3px;
    transition: flex-grow 0.3s ease;
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 10px;
    font-size: 12.5px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 24px;
    padding: 0 9px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--surface);
    color: var(--text);
    font: inherit;
    text-transform: capitalize;
    cursor: pointer;
  }
  .chip:hover {
    background: var(--surface-hover);
    border-color: var(--border-strong);
  }
  .chip i {
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }
  .right {
    margin-left: auto;
  }
</style>
