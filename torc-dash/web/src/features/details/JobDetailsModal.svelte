<!-- Job details modal. Opens when app.jobDetailsId is set (app.openJob(id)); loads the job itself. -->
<script lang="ts">
  import { untrack } from "svelte";
  import type { FileRecord, Job, Result, UserData } from "../../lib/api/client";
  import { errorText, formatBytes, formatDateTime } from "../../lib/format";
  import { app } from "../../lib/state.svelte";
  import Badge from "../../lib/ui/Badge.svelte";
  import type { Column } from "../../lib/ui/DataTable.svelte";
  import Modal from "../../lib/ui/Modal.svelte";
  import SegmentedTabs from "../../lib/ui/SegmentedTabs.svelte";
  import { loadOutputDir, resolveLogBaseDir, saveOutputDir } from "./files";
  import { loadJobDetails, type JobDetails } from "./job";
  import LogPane from "./LogPane.svelte";
  import Section from "./Section.svelte";

  type Tab = "results" | "logs" | "files" | "user_data" | "resources" | "dependencies";

  let open = $state(false);
  let details = $state.raw<JobDetails | null>(null);
  let error = $state<string | null>(null);
  let tab = $state<Tab>("results");

  $effect(() => {
    const id = app.jobDetailsId;
    void app.refreshTick;
    open = id !== null;
    if (id === null) {
      details = null;
      tab = "results";
      return;
    }
    // Keep showing the same job while refreshing; clear when switching jobs.
    if (untrack(() => details?.job.id) !== id) {
      details = null;
      runIndex = null;
    }
    error = null;
    let stale = false;
    loadJobDetails(id)
      .then((d) => !stale && (details = d))
      .catch((e) => !stale && (error = errorText(e)));
    return () => (stale = true);
  });

  let job = $derived(details?.job);

  let tabs = $derived<{ id: Tab; label: string; count?: number | null }[]>([
    { id: "results", label: "Results", count: details?.results.length },
    { id: "logs", label: "Logs" },
    {
      id: "files",
      label: "Files",
      count: details ? details.inputFiles.length + details.outputFiles.length : null,
    },
    {
      id: "user_data",
      label: "User Data",
      count: details ? details.inputUserData.length + details.outputUserData.length : null,
    },
    { id: "resources", label: "Resources" },
    {
      id: "dependencies",
      label: "Dependencies",
      count: details ? details.blockedBy.length + details.blocks.length : null,
    },
  ]);

  // Logs
  const initialDir = loadOutputDir();
  let dirInput = $state(initialDir);
  let outputDir = $state(initialDir);
  let runIndex = $state<number | null>(null); // null = latest
  let run = $derived(details?.results[runIndex ?? details.results.length - 1]);
  let logBase = $derived.by(() => {
    if (!run) return null;
    const base = resolveLogBaseDir(outputDir || "torc_output", details?.workflow?.submission_directory);
    return `${base}/job_stdio/job_wf${run.workflow_id}_j${run.job_id}_r${run.run_id}_a${run.attempt_id ?? 1}`;
  });

  const resultCols: Column<Result>[] = [
    { key: "run_id", label: "Run", width: "60px", align: "right" },
    { key: "attempt_id", label: "Attempt", width: "75px", align: "right", value: (r) => r.attempt_id ?? 1 },
    { key: "return_code", label: "Return", width: "75px", align: "right", cell: returnCode },
    { key: "status", label: "Status", width: "120px", cell: resultStatus },
    { key: "exec_time_minutes", label: "Exec (min)", align: "right", cell: execTime },
    { key: "peak_memory_bytes", label: "Peak Mem", align: "right", cell: peakMem },
    { key: "peak_cpu_percent", label: "Peak CPU %", align: "right", cell: peakCpu },
    { key: "completion_time", label: "Completed", width: "180px", cell: completed },
  ];
  const fileCols: Column<FileRecord>[] = [
    { key: "id", label: "ID", width: "70px", mono: true },
    { key: "name", label: "Name", width: "30%" },
    { key: "path", label: "Path", mono: true },
    { key: "st_mtime", label: "Modified", width: "180px", cell: fileTime },
  ];
  const userDataCols: Column<UserData>[] = [
    { key: "id", label: "ID", width: "70px", mono: true },
    { key: "name", label: "Name", width: "30%" },
    { key: "data", label: "Data", mono: true, value: (u) => JSON.stringify(u.data) },
  ];
  const jobCols: Column<Job>[] = [
    { key: "id", label: "ID", width: "80px", mono: true },
    { key: "name", label: "Name" },
    { key: "status", label: "Status", width: "130px", cell: jobStatus },
  ];
</script>

{#snippet returnCode(r: Result)}<span class:ok={r.return_code === 0} class:bad={r.return_code !== 0}
    >{r.return_code}</span
  >{/snippet}
{#snippet resultStatus(r: Result)}<Badge status={r.status} />{/snippet}
{#snippet execTime(r: Result)}{r.exec_time_minutes.toFixed(2)}{/snippet}
{#snippet peakMem(r: Result)}{formatBytes(r.peak_memory_bytes)}{/snippet}
{#snippet peakCpu(r: Result)}{r.peak_cpu_percent?.toFixed(1) ?? "—"}{/snippet}
{#snippet completed(r: Result)}{formatDateTime(r.completion_time)}{/snippet}
{#snippet fileTime(f: FileRecord)}{formatDateTime(f.st_mtime)}{/snippet}
{#snippet jobStatus(j: Job)}<Badge status={j.status} />{/snippet}


<Modal
  bind:open
  title={job ? `Job ${job.name}` : `Job ${app.jobDetailsId ?? ""}`}
  size="xl"
  onclose={() => (app.jobDetailsId = null)}
>
  {#if error && !details}
    <div class="empty error-text">Error loading job details: {error}</div>
  {:else if !details || !job}
    <div class="empty"><span class="spinner"></span></div>
  {:else}
    <div class="wrap">
      <div class="summary">
        <dl class="kv">
          <dt>ID</dt>
          <dd><code>{job.id}</code></dd>
          <dt>Status</dt>
          <dd><Badge status={job.status} /></dd>
          <dt>Workflow</dt>
          <dd>
            <button
              class="link"
              onclick={() => {
                app.go("details", job!.workflow_id);
                app.jobDetailsId = null;
              }}>{details.workflow?.name ?? "Workflow"} ({job.workflow_id})</button
            >
          </dd>
        </dl>
        <dl class="kv">
          <dt>Compute node</dt>
          <dd><code>{job.compute_node_id ?? "—"}</code></dd>
          <dt>Start time</dt>
          <dd>{formatDateTime(job.start_time)}</dd>
          <dt>Attempt</dt>
          <dd>{job.attempt_id ?? 1}{#if job.origin}<span class="faint"> · {job.origin}</span>{/if}</dd>
        </dl>
        <dl class="kv">
          <dt>Priority</dt>
          <dd>{job.priority ?? 0}</dd>
          <dt>Labels</dt>
          <dd>{job.labels?.length ? job.labels.join(", ") : "—"}</dd>
          <dt>Resources</dt>
          <dd>{details.resourceReq?.name ?? "—"}</dd>
        </dl>
      </div>
      <pre class="code-block command">{job.command}</pre>

      <SegmentedTabs bind:value={tab} {tabs} />

      <div class="tab">
        {#if tab === "results"}
          <Section title="Results" rows={details.results} columns={resultCols} rowKey={(r) => r.id ?? `${r.run_id}_${r.attempt_id}`} empty="No results for this job" />
        {:else if tab === "logs"}
          {#if !details.results.length || !logBase}
            <div class="empty">No results yet. Logs are available after the job runs.</div>
          {:else}
            <div class="toolbar">
              <label class="field">
                Output directory
                <input
                  class="input"
                  bind:value={dirInput}
                  placeholder="torc_output"
                  onchange={() => saveOutputDir((outputDir = dirInput))}
                />
              </label>
              {#if details.results.length > 1}
                <label class="field">
                  Run
                  <select
                    class="select"
                    value={runIndex ?? details.results.length - 1}
                    onchange={(e) => (runIndex = Number(e.currentTarget.value))}
                  >
                    {#each details.results as r, i (r.id ?? i)}
                      <option value={i}
                        >Run {r.run_id} · attempt {r.attempt_id ?? 1} · rc {r.return_code}</option
                      >
                    {/each}
                  </select>
                </label>
              {/if}
            </div>
            <LogPane stdout={[`${logBase}.o`, `${logBase}.log`]} stderr={[`${logBase}.e`, `${logBase}.log`]} />
          {/if}
        {:else if tab === "files"}
          <Section title="Input files" rows={details.inputFiles} columns={fileCols} rowKey={(f) => f.id!} empty="No input files" onRowClick={(f) => app.openFile(f.path, f.name)} />
          <Section title="Output files" rows={details.outputFiles} columns={fileCols} rowKey={(f) => f.id!} empty="No output files" onRowClick={(f) => app.openFile(f.path, f.name)} />
        {:else if tab === "user_data"}
          <Section title="Input user data" rows={details.inputUserData} columns={userDataCols} rowKey={(u) => u.id!} empty="No input user data" />
          <Section title="Output user data" rows={details.outputUserData} columns={userDataCols} rowKey={(u) => u.id!} empty="No output user data" />
        {:else if tab === "resources"}
          {@const r = details.resourceReq}
          {#if !r}
            <div class="empty">No resource requirement assigned to this job</div>
          {:else}
            <dl class="kv card card-body">
              <dt>ID</dt>
              <dd><code>{r.id}</code></dd>
              <dt>Name</dt>
              <dd>{r.name}</dd>
              <dt>CPUs</dt>
              <dd>{r.num_cpus}</dd>
              <dt>Memory</dt>
              <dd>{r.memory}</dd>
              <dt>GPUs</dt>
              <dd>{r.num_gpus}</dd>
              <dt>Nodes</dt>
              <dd>{r.num_nodes}</dd>
              <dt>Runtime</dt>
              <dd><code>{r.runtime}</code></dd>
            </dl>
          {/if}
        {:else if tab === "dependencies"}
          <Section title="Blocked by" rows={details.blockedBy} columns={jobCols} rowKey={(j) => j.id!} empty="This job has no dependencies" onRowClick={(j) => app.openJob(j.id!)} />
          <Section title="Blocks" rows={details.blocks} columns={jobCols} rowKey={(j) => j.id!} empty="No jobs depend on this job" onRowClick={(j) => app.openJob(j.id!)} />
        {/if}
      </div>
    </div>
  {/if}
</Modal>

<style>
  .wrap {
    display: flex;
    flex-direction: column;
    gap: 14px;
    min-height: 100%;
  }
  .summary {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 8px 24px;
  }
  .command {
    max-height: 140px;
  }
  .tab {
    display: flex;
    flex-direction: column;
    gap: 16px;
    flex: 1;
    min-height: 0;
  }
  .toolbar .field {
    min-width: 240px;
  }
  .link {
    padding: 0;
    border: none;
    background: none;
    color: var(--accent);
    font: inherit;
    cursor: pointer;
  }
  .link:hover {
    text-decoration: underline;
  }
  .ok {
    color: var(--success);
    font-weight: 600;
  }
  .bad {
    color: var(--danger);
    font-weight: 600;
  }
</style>
