<!-- Result of `torc slurm plan-allocations`: recommended Slurm allocations per job group. -->
<script lang="ts" module>
  export type Plan = {
    workflow_analysis?: {
      total_instances?: number;
      dependency_depth?: number;
      max_parallelism?: number;
    };
    recommendations?: {
      group_name: string;
      partition?: string | null;
      num_allocations: number;
      nodes_per_allocation: number;
      walltime?: string;
      job_count: number;
      reason?: string;
    }[];
    resource_groups?: { name: string; partition?: string | null }[];
    warnings?: string[];
    profile_display_name?: string;
  };
</script>

<script lang="ts">
  let { plan }: { plan: Plan } = $props();

  const partitionOf = (group: string, given?: string | null) =>
    given ?? plan.resource_groups?.find((g) => g.name === group)?.partition ?? "auto";
</script>

<div class="plan">
  {#if plan.workflow_analysis}
    {@const a = plan.workflow_analysis}
    <p class="muted">
      {a.total_instances ?? "?"} job(s), dependency depth {a.dependency_depth ?? "?"}, up to
      {a.max_parallelism ?? "?"} in parallel{plan.profile_display_name
        ? ` · ${plan.profile_display_name}`
        : ""}
    </p>
  {/if}
  {#if plan.recommendations?.length}
    <table>
      <thead>
        <tr><th>Jobs</th><th>Partition</th><th>Allocations</th><th>Nodes each</th><th>Walltime</th></tr>
      </thead>
      <tbody>
        {#each plan.recommendations as r (r.group_name)}
          <tr title={r.reason ?? ""}>
            <td>{r.group_name} <span class="muted">({r.job_count})</span></td>
            <td class="mono">{partitionOf(r.group_name, r.partition)}</td>
            <td>{r.num_allocations}</td>
            <td>{r.nodes_per_allocation}</td>
            <td class="mono">{r.walltime ?? "—"}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
  {#each plan.warnings ?? [] as w, i (i)}<p class="warn">{w}</p>{/each}
</div>

<style>
  .plan {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 13px;
  }
  p {
    margin: 0;
  }
  .warn {
    color: var(--warning);
  }
  table {
    width: 100%;
    border-collapse: collapse;
  }
  th,
  td {
    padding: 4px 8px;
    border-bottom: 1px solid var(--border);
    text-align: left;
  }
  th {
    color: var(--text-muted);
    font-weight: 500;
  }
</style>
