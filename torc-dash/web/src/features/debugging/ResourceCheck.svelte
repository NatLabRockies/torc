<!--
  Resource requests vs. actual usage: jobs that exceeded what they asked for
  (torc workflows check-resources), and right-sizing the requirements from measured usage
  (torc workflows correct-resources: preview, then apply).
-->
<script lang="ts">
  import { op } from "../../lib/api/dash";
  import { errorText } from "../../lib/format";
  import { app } from "../../lib/state.svelte";
  import DataTable, { type Column } from "../../lib/ui/DataTable.svelte";
  import { confirm } from "../../lib/ui/confirm.svelte";
  import { toast } from "../../lib/ui/toast.svelte";

  type Violation = {
    job_id: number;
    job_name: string;
    resource_type: string;
    specified: string;
    peak_used: string;
    over_utilization: string;
  };
  type Check = { total_results: number; over_utilization_count: number; violations: Violation[] };
  type Adjustment = {
    resource_requirements_id: number;
    direction: string;
    job_names: string[];
    memory_adjusted: boolean;
    original_memory?: string | null;
    new_memory?: string | null;
    runtime_adjusted: boolean;
    original_runtime?: string | null;
    new_runtime?: string | null;
    cpu_adjusted?: boolean;
    original_cpus?: number | null;
    new_cpus?: number | null;
  };
  type Correction = {
    jobs_analyzed: number;
    resource_requirements_updated: number;
    message?: string;
    adjustments?: Adjustment[];
  };

  let { workflowId }: { workflowId: number } = $props();

  let includeFailed = $state(false);
  let showAll = $state(false);
  let check = $state<Check | null>(null);
  let checkError = $state("");

  let memoryMultiplier = $state(1.2);
  let cpuMultiplier = $state(1.2);
  let runtimeMultiplier = $state(1.2);
  let noDownsize = $state(false);
  let preview = $state<Correction | null>(null);
  let busy = $state(false);
  let correctError = $state("");

  $effect(() => {
    const id = workflowId;
    const params = { workflow_id: id, include_failed: includeFailed, all: showAll };
    void app.refreshTick;
    op<Check>("check-resources", params)
      .then((c) => {
        if (id !== workflowId) return;
        check = c;
        checkError = "";
      })
      .catch((e) => {
        if (id === workflowId) checkError = errorText(e);
      });
  });

  // A preview belongs to one workflow and one set of options.
  $effect(() => {
    void [workflowId, memoryMultiplier, cpuMultiplier, runtimeMultiplier, noDownsize];
    preview = null;
  });

  function correctionParams(dry_run: boolean) {
    return {
      workflow_id: workflowId,
      memory_multiplier: memoryMultiplier,
      cpu_multiplier: cpuMultiplier,
      runtime_multiplier: runtimeMultiplier,
      no_downsize: noDownsize,
      dry_run,
    };
  }

  async function runPreview() {
    busy = true;
    correctError = "";
    try {
      preview = await op<Correction>("correct-resources", correctionParams(true));
    } catch (e) {
      correctError = errorText(e);
    } finally {
      busy = false;
    }
  }

  async function apply() {
    const n = preview?.resource_requirements_updated ?? 0;
    const ok = await confirm({
      title: "Update resource requirements?",
      message:
        `This updates ${n} resource requirement(s) for future runs. Jobs are not reset or rerun; ` +
        "reset or recover them to run again with the new values.",
      confirmLabel: "Apply",
    });
    if (!ok) return;
    busy = true;
    try {
      const res = await op<Correction>("correct-resources", correctionParams(false));
      toast.success(`Updated ${res?.resource_requirements_updated ?? n} resource requirement(s)`);
      preview = null;
      app.refresh();
    } catch (e) {
      correctError = errorText(e);
    } finally {
      busy = false;
    }
  }

  const change = (from?: string | number | null, to?: string | number | null) =>
    from == null && to == null ? "—" : `${from ?? "?"} → ${to ?? "?"}`;

  const columns: Column<Violation>[] = [
    { key: "job_id", label: "Job", width: "70px", mono: true },
    { key: "job_name", label: "Name" },
    { key: "resource_type", label: "Resource", width: "110px" },
    { key: "specified", label: "Requested", mono: true },
    { key: "peak_used", label: "Peak used", mono: true },
    { key: "over_utilization", label: "Over", mono: true, width: "100px" },
  ];
</script>

<div class="section">
  <div class="toolbar">
    <h4 class="grow">Jobs that exceeded their requested resources</h4>
    <label class="check"><input type="checkbox" bind:checked={includeFailed} /> Include failed jobs</label>
    <label class="check"><input type="checkbox" bind:checked={showAll} /> All jobs</label>
  </div>
  {#if checkError}
    <p class="error-text">{checkError}</p>
  {:else if !check}
    <div class="empty"><span class="spinner"></span> Checking…</div>
  {:else}
    <p class="muted">
      {check.over_utilization_count} violation(s) across {check.total_results} result(s).
    </p>
    {#if check.violations.length}
      <div class="table">
        <DataTable
          rows={check.violations}
          {columns}
          rowKey={(v) => `${v.job_id}/${v.resource_type}`}
          onRowClick={(v) => app.openJob(v.job_id)}
        />
      </div>
    {/if}
  {/if}

  <h4>Right-size resource requirements</h4>
  <p class="muted">
    Set each requirement from its jobs' measured peak usage times a safety multiplier. Raises
    requests that were exceeded and, unless disabled, lowers ones that were far too large. Only
    future runs are affected.
  </p>
  <div class="options">
    <label class="field">
      Memory ×
      <input class="input" type="number" min="1" step="0.1" bind:value={memoryMultiplier} />
    </label>
    <label class="field">
      CPU ×
      <input class="input" type="number" min="1" step="0.1" bind:value={cpuMultiplier} />
    </label>
    <label class="field">
      Runtime ×
      <input class="input" type="number" min="1" step="0.1" bind:value={runtimeMultiplier} />
    </label>
    <label class="check"><input type="checkbox" bind:checked={noDownsize} /> Never lower requests</label>
    <button class="btn" onclick={runPreview} disabled={busy}>
      {#if busy && !preview}<span class="spinner small"></span>{/if} Preview changes
    </button>
  </div>
  {#if correctError}<p class="error-text">{correctError}</p>{/if}
  {#if preview}
    {#if preview.adjustments?.length}
      <table>
        <thead>
          <tr><th>Requirement</th><th>Jobs</th><th>Memory</th><th>CPUs</th><th>Runtime</th></tr>
        </thead>
        <tbody>
          {#each preview.adjustments as a (a.resource_requirements_id)}
            <tr>
              <td class="mono">#{a.resource_requirements_id} <span class="muted">{a.direction}</span></td>
              <td>
                {a.job_names.slice(0, 3).join(", ")}
                {#if a.job_names.length > 3}<em class="muted">(+{a.job_names.length - 3} more)</em>{/if}
              </td>
              <td class="mono">{a.memory_adjusted ? change(a.original_memory, a.new_memory) : "—"}</td>
              <td class="mono">{a.cpu_adjusted ? change(a.original_cpus, a.new_cpus) : "—"}</td>
              <td class="mono">{a.runtime_adjusted ? change(a.original_runtime, a.new_runtime) : "—"}</td>
            </tr>
          {/each}
        </tbody>
      </table>
      <div class="toolbar">
        <div class="grow"></div>
        <button class="btn btn-primary" onclick={apply} disabled={busy}>
          Apply {preview.resource_requirements_updated} change(s)
        </button>
      </div>
    {:else}
      <p class="muted">{preview.message ?? "No changes needed."}</p>
    {/if}
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
  .table {
    min-height: 180px;
    max-height: 45vh;
    display: flex;
    flex-direction: column;
  }
  .options {
    display: flex;
    flex-wrap: wrap;
    align-items: end;
    gap: 12px;
  }
  .options .input {
    width: 90px;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
  }
  th,
  td {
    padding: 6px 8px;
    border-bottom: 1px solid var(--border);
    text-align: left;
  }
  th {
    color: var(--text-muted);
    font-weight: 500;
  }
  .spinner.small {
    width: 12px;
    height: 12px;
  }
</style>
