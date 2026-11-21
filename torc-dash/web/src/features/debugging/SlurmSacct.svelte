<!-- Collect Slurm accounting data for the workflow's allocations (torc slurm sacct). -->
<script lang="ts">
  import { dashPost, op } from "../../lib/api/dash";
  import { errorText, type Tone } from "../../lib/format";
  import Badge from "../../lib/ui/Badge.svelte";
  import DataTable, { type Column } from "../../lib/ui/DataTable.svelte";

  let { workflowId, outputDir }: { workflowId: number; outputDir: string } = $props();

  type Row = {
    slurm_job_id: string;
    job_step: string;
    state: string;
    exit_code: string;
    elapsed: string;
    max_rss: string;
    cpu_time: string;
    nodes: string;
  };
  type Data = { total_slurm_jobs?: number; summary?: Row[]; errors?: string[]; message?: string };

  let running = $state(false);
  type Usage = { total_nodes: number; total_node_time: string; total_cpu_time: string };

  let data = $state<Data | null>(null);
  let usage = $state<Usage | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    void workflowId;
    data = null;
    usage = null;
    error = null;
  });

  function stateTone(state: string): Tone {
    const s = state.toUpperCase();
    if (s === "COMPLETED") return "success";
    if (s === "RUNNING") return "accent";
    if (s === "PENDING") return "info";
    if (["FAILED", "NODE_FAIL", "OUT_OF_MEMORY"].includes(s)) return "danger";
    if (["CANCELLED", "TIMEOUT", "PREEMPTED"].includes(s)) return "warning";
    return "neutral";
  }

  const columns: Column<Row>[] = [
    { key: "slurm_job_id", label: "Slurm job", mono: true },
    { key: "job_step", label: "Step", mono: true },
    { key: "state", label: "State", cell: stateCell, width: "130px" },
    { key: "exit_code", label: "Exit code", mono: true, width: "90px" },
    { key: "elapsed", label: "Elapsed", mono: true },
    { key: "max_rss", label: "Max RSS", mono: true },
    { key: "cpu_time", label: "CPU time", mono: true },
    { key: "nodes", label: "Nodes" },
  ];

  async function collect() {
    const requested = workflowId;
    running = true;
    error = null;
    try {
      const r = await dashPost<{ success: boolean; data: Data | null; error: string | null }>(
        "/api/cli/slurm-sacct",
        { workflow_id: requested, output_dir: outputDir },
      );
      if (requested !== workflowId) return; // the user switched workflows meanwhile
      if (!r.success) throw new Error(r.error || "Unknown error");
      data = r.data ?? {};
      // Totals are a convenience; the table above is the main result.
      const u = await op<Usage>("slurm-usage", { workflow_id: requested }).catch(() => null);
      if (requested === workflowId) usage = u;
    } catch (e) {
      if (requested === workflowId) error = errorText(e);
    } finally {
      running = false;
    }
  }
</script>

{#snippet stateCell(row: Row)}
  <Badge tone={stateTone(row.state || "")}>{row.state || "—"}</Badge>
{/snippet}

<div class="section">
  <div class="toolbar">
    <p class="muted grow">Collect sacct data for all Slurm jobs in this workflow.</p>
    <button class="btn btn-primary" onclick={collect} disabled={running}>
      {#if running}<span class="spinner"></span>{/if} Collect sacct data
    </button>
  </div>

  {#if error}
    <p class="error-text">{error}</p>
  {:else if !data}
    <div class="empty">Click “Collect sacct data” to retrieve job accounting information.</div>
  {:else if !data.total_slurm_jobs}
    <div class="empty">
      {data.message ?? "No Slurm scheduled compute nodes found for this workflow."}
    </div>
  {:else}
    <p>
      <strong>{data.total_slurm_jobs}</strong> Slurm job(s),
      <strong>{data.summary?.length ?? 0}</strong> job step(s)
      {#if usage}
        · <strong>{usage.total_nodes}</strong> node(s), <strong>{usage.total_node_time}</strong> node
        time, <strong>{usage.total_cpu_time}</strong> CPU time
      {/if}
    </p>
    {#if data.summary?.length}
      <div class="table">
        <DataTable
          rows={data.summary}
          {columns}
          rowKey={(r) => `${r.slurm_job_id}/${r.job_step}`}
        />
      </div>
    {/if}
    {#if data.errors?.length}
      <div>
        <strong>Errors</strong>
        <ul>
          {#each data.errors as err, i (i)}<li class="error-text">{err}</li>{/each}
        </ul>
      </div>
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
  }
  p {
    margin: 0;
  }
  .table {
    flex: 1;
    min-height: 200px;
    display: flex;
    flex-direction: column;
  }
</style>
