<!--
  Why aren't jobs running? Packing diagnosis (torc workflows diagnose) and the workflow's actions
  with their trigger progress (torc workflows list-actions).
-->
<script lang="ts">
  import { op } from "../../lib/api/dash";
  import { errorText, formatDuration } from "../../lib/format";
  import { app } from "../../lib/state.svelte";
  import Badge from "../../lib/ui/Badge.svelte";
  import DataTable, { type Column } from "../../lib/ui/DataTable.svelte";
  import Stats from "../workflows/Stats.svelte";

  type Durations = { min_seconds: number; median_seconds: number; max_seconds: number } | null;
  type Diagnosis = {
    active_allocations: number;
    allocations_unlimited: number;
    ready_jobs: number;
    ready_jobs_with_runtime: number;
    allocation_remaining: Durations;
    ready_job_runtime: Durations;
    runtime_blocked_jobs: number;
    allocations_too_short_for_longest: number;
    runtime_blocked: boolean;
  };
  type Action = {
    id: number;
    trigger_type: string;
    action_type: string;
    action_config?: Record<string, unknown> | null;
    job_ids?: number[] | null;
    trigger_count: number;
    required_triggers: number;
    executed: boolean;
    persistent?: boolean;
    is_recovery?: boolean;
  };

  let { workflowId }: { workflowId: number } = $props();

  let diagnosis = $state<Diagnosis | null>(null);
  let actions = $state<Action[] | null>(null);
  let error = $state("");

  $effect(() => {
    const id = workflowId;
    void app.refreshTick;
    Promise.all([
      op<Diagnosis>("diagnose", { workflow_id: id }),
      op<{ actions: Action[] }>("list-actions", { workflow_id: id }),
    ])
      .then(([d, a]) => {
        if (id !== workflowId) return;
        diagnosis = d;
        actions = a?.actions ?? [];
        error = "";
      })
      .catch((e) => {
        if (id === workflowId) error = errorText(e);
      });
  });

  const range = (d: Durations) =>
    d ? `${formatDuration(d.min_seconds)} – ${formatDuration(d.max_seconds)}` : "—";

  function config(a: Action): string {
    const c = a.action_config ?? {};
    if (a.action_type === "schedule_nodes") {
      return `${c.num_allocations ?? 1} × ${c.scheduler_type ?? "scheduler"} ${c.scheduler_id ?? ""}`;
    }
    return c.command ? String(c.command) : JSON.stringify(c);
  }

  const columns: Column<Action>[] = [
    { key: "id", label: "ID", width: "60px", mono: true },
    { key: "trigger_type", label: "Trigger", mono: true },
    { key: "action_type", label: "Action", mono: true },
    { key: "config", label: "Config", value: config },
    { key: "jobs", label: "Jobs", width: "70px", align: "right", value: (a) => a.job_ids?.length ?? 0 },
    {
      key: "progress",
      label: "Triggers",
      width: "90px",
      mono: true,
      value: (a) => `${a.trigger_count}/${a.required_triggers}`,
    },
    { key: "executed", label: "Status", width: "110px", value: (a) => a.executed, cell: statusCell },
  ];
</script>

{#snippet statusCell(a: Action)}
  {#if a.executed}<Badge tone="success">executed</Badge>{:else}<Badge tone="neutral">pending</Badge>{/if}
  {#if a.is_recovery}<Badge tone="warning">recovery</Badge>{/if}
{/snippet}

<div class="section">
  {#if error}<p class="error-text">{error}</p>{/if}

  <h4>Allocation packing</h4>
  {#if !diagnosis}
    <div class="empty"><span class="spinner"></span> Diagnosing…</div>
  {:else}
    <Stats
      items={[
        { label: "Active allocations", value: diagnosis.active_allocations },
        { label: "Ready jobs", value: diagnosis.ready_jobs },
        {
          label: "Blocked by walltime",
          value: diagnosis.runtime_blocked_jobs,
          tone: diagnosis.runtime_blocked_jobs ? "warning" : undefined,
        },
        {
          label: "Allocations too short",
          value: diagnosis.allocations_too_short_for_longest,
          tone: diagnosis.allocations_too_short_for_longest ? "warning" : undefined,
        },
      ]}
    />
    <dl class="kv">
      <dt>Remaining allocation walltime</dt><dd>{range(diagnosis.allocation_remaining)}</dd>
      <dt>Runtime of ready jobs</dt><dd>{range(diagnosis.ready_job_runtime)}</dd>
    </dl>
    {#if diagnosis.runtime_blocked}
      <p class="note warn">
        {diagnosis.runtime_blocked_jobs} ready job(s) need more time than any running allocation has
        left, so torc won't start them there (they would be killed mid-run). They will run on a new
        allocation: schedule more Slurm nodes, or lower the jobs' runtime if it is overestimated.
      </p>
    {:else if diagnosis.ready_jobs && !diagnosis.active_allocations}
      <p class="note">
        {diagnosis.ready_jobs} job(s) are ready but no allocation is active. Submit the workflow or
        schedule Slurm nodes to run them.
      </p>
    {/if}
  {/if}

  <h4>Workflow actions</h4>
  {#if actions === null}
    <div class="empty"><span class="spinner"></span> Loading actions…</div>
  {:else if actions.length === 0}
    <p class="muted">This workflow has no actions.</p>
  {:else}
    <p class="muted">
      An action runs once its trigger has fired for all of its jobs (Triggers shows progress).
    </p>
    <div class="table">
      <DataTable rows={actions} {columns} rowKey={(a) => a.id} />
    </div>
  {/if}
</div>

<style>
  .section {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-height: 0;
    flex: 1;
    overflow: auto;
  }
  h4 {
    margin: 8px 0 0;
    font-size: 13px;
  }
  p {
    margin: 0;
  }
  .note {
    padding: 10px 12px;
    border-radius: var(--radius);
    background: var(--surface-2);
  }
  .note.warn {
    border-left: 3px solid var(--warning);
  }
  .table {
    min-height: 200px;
    display: flex;
    flex-direction: column;
  }
</style>
