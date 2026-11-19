<!-- Live tail: check jobs to follow their stdout/stderr as they write, one pane per file. -->
<script lang="ts">
  import type { SvelteSet } from "svelte/reactivity";
  import type { Job, Workflow } from "../../lib/api/client";
  import Badge from "../../lib/ui/Badge.svelte";
  import DataTable, { type Column } from "../../lib/ui/DataTable.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import { liveFiles, stdioMode } from "./logs";
  import TailPane from "./TailPane.svelte";

  let {
    jobs,
    wf,
    baseDir,
    filter,
    selected,
  }: {
    jobs: Job[] | null;
    wf: Workflow | null;
    baseDir: string;
    filter: string;
    selected: SvelteSet<string | number>;
  } = $props();

  let runningOnly = $state(true);
  let paused = $state(false);
  let clearTick = $state(0);

  let rows = $derived(
    (jobs ?? []).filter((j) => !runningOnly || j.status === "running" || selected.has(j.id!)),
  );

  let panes = $derived(
    (jobs ?? [])
      .filter((j) => wf && selected.has(j.id!))
      .map((job) => ({ job, files: liveFiles(baseDir, wf!, job) })),
  );

  const columns: Column<Job>[] = [
    { key: "id", label: "ID", width: "70px", mono: true, value: (j) => j.id },
    { key: "name", label: "Name", value: (j) => j.name },
    { key: "status", label: "Status", width: "115px", value: (j) => j.status, cell: statusCell },
    { key: "stdio", label: "Stdio", width: "95px", value: (j) => stdioMode(wf, j.name) },
  ];
</script>

{#snippet statusCell(j: Job)}<Badge status={j.status} />{/snippet}

<div class="split">
  <div class="side">
    <div class="toolbar">
      <label class="check"><input type="checkbox" bind:checked={runningOnly} /> Running only</label>
    </div>
    <DataTable
      {rows}
      {columns}
      {filter}
      rowKey={(j) => j.id!}
      selectable
      selectAll={false}
      {selected}
      onRowClick={(j) => (selected.has(j.id!) ? selected.delete(j.id!) : selected.add(j.id!))}
      loading={!jobs}
      empty={runningOnly
        ? "No running jobs. Uncheck “Running only” to pick any job."
        : "No jobs in this workflow."}
    />
  </div>
  <div class="side">
    <div class="toolbar">
      <span class="faint">{panes.length} job{panes.length === 1 ? "" : "s"} tailed</span>
      <div class="grow"></div>
      <button class="btn btn-sm" disabled={!panes.length} onclick={() => (paused = !paused)}>
        <Icon name={paused ? "play" : "stop"} size={14} />
        {paused ? "Resume all" : "Pause all"}
      </button>
      <button class="btn btn-sm" disabled={!panes.length} onclick={() => clearTick++}>
        <Icon name="trash" size={14} /> Clear all
      </button>
      <button class="btn btn-sm" disabled={!panes.length} onclick={() => selected.clear()}>
        <Icon name="x" size={14} /> Close all
      </button>
    </div>
    {#if !panes.length}
      <div class="empty">Check jobs on the left to follow their output as it is written.</div>
    {:else}
      <div class="panes">
        {#each panes as { job, files } (job.id)}
          {#each files as file (file.stream)}
            <TailPane title={`${job.name} #${job.id}`} {file} {paused} {clearTick} />
          {:else}
            <div class="card none">
              <strong>{job.name}</strong> #{job.id}: output is not captured (stdio mode
              <code>none</code>).
            </div>
          {/each}
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .split {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.8fr);
    gap: 16px;
  }
  .side {
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .panes {
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(380px, 1fr));
    align-content: start;
    gap: 12px;
  }
  .none {
    padding: 12px;
  }
  @media (max-width: 1100px) {
    .split {
      grid-template-columns: 1fr;
      grid-template-rows: minmax(200px, 0.8fr) minmax(320px, 1.2fr);
    }
  }
</style>
