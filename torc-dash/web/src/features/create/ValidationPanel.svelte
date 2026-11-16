<!-- Result of a dry-run validation (/api/cli/validate). -->
<script lang="ts">
  import Icon from "../../lib/ui/Icon.svelte";
  import type { ValidationReport } from "./actions";

  let { report, busy = false }: { report: ValidationReport | null; busy?: boolean } = $props();

  const s = $derived(report?.summary);
  type Stat = { label: string; value?: number; before?: number };
  const stats = $derived(
    (s
      ? ([
          { label: "Jobs", value: s.job_count, before: s.job_count_before_expansion },
          { label: "Files", value: s.file_count },
          { label: "User data", value: s.user_data_count },
          { label: "Resource reqs", value: s.resource_requirements_count },
          { label: "Schedulers", value: s.slurm_scheduler_count },
          { label: "Actions", value: s.action_count },
        ] as Stat[])
      : []
    ).filter((x) => x.label === "Jobs" || (x.value ?? 0) > 0),
  );
</script>

{#if report}
  <div class="panel" class:ok={report.valid} class:bad={!report.valid} class:stale={busy} aria-live="polite">
    <div class="title">
      <span class="icon"><Icon name={report.valid ? "check" : "x"} size={14} /></span>
      <strong>{report.valid ? "Spec is valid" : "Spec has errors"}</strong>
      {#if s?.workflow_name}<span class="muted">· {s.workflow_name}</span>{/if}
      {#if busy}<span class="spinner"></span>{/if}
    </div>
    {#if report.errors.length}
      <ul class="errors">
        {#each report.errors as e, i (i)}<li>{e}</li>{/each}
      </ul>
    {/if}
    {#if report.warnings.length}
      <ul class="warnings">
        {#each report.warnings as w, i (i)}<li>{w}</li>{/each}
      </ul>
    {/if}
    {#if stats.length}
      <div class="stats">
        {#each stats as st (st.label)}
          <div>
            <span class="v">{st.value ?? "?"}</span>
            <span class="l"
              >{st.label}{#if st.before !== undefined && st.before !== st.value}<span class="faint">
                  (from {st.before} specs)</span
                >{/if}</span
            >
          </div>
        {/each}
      </div>
    {/if}
  </div>
{:else if busy}
  <div class="panel"><span class="spinner"></span> <span class="muted">Validating…</span></div>
{/if}

<style>
  .panel {
    display: grid;
    gap: 8px;
    padding: 12px 14px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface-2);
    transition: opacity 0.12s;
  }
  .stale {
    opacity: 0.7;
  }
  .ok {
    border-color: var(--success);
    background: var(--success-soft);
  }
  .bad {
    border-color: var(--danger);
    background: var(--danger-soft);
  }
  .title {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .icon {
    display: inline-grid;
    place-items: center;
    width: 20px;
    height: 20px;
    border-radius: 999px;
    color: var(--accent-text);
    background: var(--text-muted);
  }
  .ok .icon {
    background: var(--success);
  }
  .bad .icon {
    background: var(--danger);
  }
  ul {
    margin: 0;
    padding-left: 20px;
    font-size: 13px;
  }
  .errors {
    color: var(--danger);
  }
  .warnings {
    color: var(--warning);
  }
  .stats {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 18px;
  }
  .stats div {
    display: flex;
    align-items: baseline;
    gap: 5px;
  }
  .v {
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  .l {
    color: var(--text-muted);
    font-size: 12.5px;
  }
</style>
