<!-- Scan Slurm logs for known error patterns (torc slurm parse-logs). -->
<script lang="ts">
  import { dashPost } from "../../lib/api/dash";
  import { errorText } from "../../lib/format";
  import Badge from "../../lib/ui/Badge.svelte";

  let { workflowId, outputDir }: { workflowId: number; outputDir: string } = $props();

  type Issue = {
    file: string;
    slurm_job_id: string;
    line_number: number;
    line: string;
    pattern_description: string;
    severity: string;
    node?: string | null;
    affected_jobs?: { job_id: number; job_name: string }[];
  };
  type Data = {
    total_issues?: number;
    errors?: number;
    warnings?: number;
    files_scanned?: number;
    issues?: Issue[];
    message?: string;
  };

  let errorsOnly = $state(false);
  let running = $state(false);
  let data = $state<Data | null>(null);
  let error = $state<string | null>(null);

  // Results belong to one workflow; drop them when it changes.
  $effect(() => {
    void workflowId;
    data = null;
    error = null;
  });

  let groups = $derived.by(() => {
    const m = new Map<string, Issue[]>();
    for (const i of data?.issues ?? []) {
      const k = i.slurm_job_id || "unknown";
      m.set(k, [...(m.get(k) ?? []), i]);
    }
    return [...m];
  });

  async function analyze() {
    running = true;
    error = null;
    try {
      const r = await dashPost<{ success: boolean; data: Data | null; error: string | null }>(
        "/api/cli/slurm-parse-logs",
        { workflow_id: workflowId, output_dir: outputDir, errors_only: errorsOnly },
      );
      if (!r.success) throw new Error(r.error || "Unknown error");
      data = r.data ?? {};
    } catch (e) {
      error = errorText(e);
    } finally {
      running = false;
    }
  }
</script>

<div class="section">
  <div class="toolbar">
    <p class="muted grow">
      Parse Slurm log files for known error patterns (OOM, time limits, node failures, etc.).
    </p>
    <label class="check"><input type="checkbox" bind:checked={errorsOnly} /> Errors only</label>
    <button class="btn btn-primary" onclick={analyze} disabled={running}>
      {#if running}<span class="spinner"></span>{/if} Analyze Slurm logs
    </button>
  </div>

  {#if error}
    <p class="error-text">{error}</p>
  {:else if !data}
    <div class="empty">Click “Analyze Slurm logs” to scan for issues.</div>
  {:else if !data.total_issues}
    <div class="empty">
      {data.message ?? `No issues found in Slurm logs (scanned ${data.files_scanned ?? 0} file(s)).`}
    </div>
  {:else}
    <p>
      <strong>{data.errors ?? 0}</strong> error(s), <strong>{data.warnings ?? 0}</strong> warning(s)
      in {data.files_scanned ?? 0} file(s)
    </p>
    {#each groups as [jobId, issues] (jobId)}
      <div class="card group">
        <div class="head">
          <strong>Slurm job {jobId}</strong>
          <span class="muted">{issues.length} issue(s)</span>
        </div>
        {#if issues[0].affected_jobs?.length}
          <div class="affected">
            Affected Torc jobs:
            {#each issues[0].affected_jobs as j (j.job_id)}
              <span class="tag">{j.job_name} ({j.job_id})</span>
            {/each}
          </div>
        {/if}
        {#each issues as issue, i (i)}
          <div class="issue">
            <div class="title">
              <Badge tone={issue.severity === "error" ? "danger" : "warning"}>
                {issue.severity}
              </Badge>
              {issue.pattern_description}{#if issue.node}<span class="muted">
                  (node: {issue.node})</span
                >{/if}
            </div>
            <pre class="code-block">{issue.line}</pre>
            <div class="faint loc">{issue.file}:{issue.line_number}</div>
          </div>
        {/each}
      </div>
    {/each}
  {/if}
</div>

<style>
  .section {
    display: flex;
    flex-direction: column;
    gap: 12px;
    overflow: auto;
    min-height: 0;
  }
  p {
    margin: 0;
  }
  .group {
    padding: 12px 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .head {
    display: flex;
    gap: 12px;
    align-items: baseline;
  }
  .affected {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
    font-size: 13px;
  }
  .tag {
    padding: 1px 8px;
    border-radius: 10px;
    background: var(--neutral-soft);
    font-size: 12px;
  }
  .issue {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .title {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .loc {
    font-family: var(--mono);
    font-size: 12px;
  }
</style>
