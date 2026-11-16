<!-- `torc workflows execution-plan`: events in topological order with the jobs each one readies. -->
<script lang="ts">
  import { dashPost } from "../../lib/api/dash";
  import { errorText } from "../../lib/format";
  import Modal from "../../lib/ui/Modal.svelte";
  import { wid } from "./actions";

  type Trigger = { type: string; data?: { jobs?: string[] } };
  type PlanEvent = {
    id: string;
    trigger?: Trigger;
    trigger_description?: string;
    jobs_becoming_ready?: string[];
    scheduler_allocations?: { scheduler: string; scheduler_type: string; num_allocations: number }[];
    unlocks_events?: string[];
  };
  type Plan = { events?: PlanEvent[]; root_events?: string[]; leaf_events?: string[] };
  type Response = { success: boolean; data?: Plan; error?: string };

  let { workflowId, onclose }: { workflowId: number; onclose: () => void } = $props();

  let open = $state(true);
  let plan = $state<Plan | null>(null);
  let error = $state("");

  $effect(() => {
    dashPost<Response>("/api/cli/execution-plan", wid(workflowId))
      .then((res) => {
        if (res.success) plan = res.data ?? {};
        else error = res.error || "Unknown error";
      })
      .catch((e) => (error = errorText(e)));
  });

  /** BFS from the roots, then anything unreachable. */
  function ordered(p: Plan): PlanEvent[] {
    const byId = new Map((p.events ?? []).map((e) => [e.id, e]));
    const seen = new Set<string>();
    const out: PlanEvent[] = [];
    const queue = [...(p.root_events ?? [])];
    while (queue.length) {
      const id = queue.shift()!;
      const ev = byId.get(id);
      if (seen.has(id) || !ev) continue;
      seen.add(id);
      out.push(ev);
      queue.push(...(ev.unlocks_events ?? []));
    }
    for (const ev of p.events ?? []) if (!seen.has(ev.id)) out.push(ev);
    return out;
  }

  function formatTrigger(t?: Trigger): string {
    if (t?.type === "WorkflowStart") return "Workflow Start";
    if (t?.type !== "JobsComplete") return "Unknown trigger";
    const jobs = t.data?.jobs ?? [];
    if (jobs.length === 0) return "Jobs Complete";
    if (jobs.length === 1) return `When job '${jobs[0]}' completes`;
    if (jobs.length <= 3) return `When jobs complete: ${jobs.map((j) => `'${j}'`).join(", ")}`;
    return `When ${jobs.length} jobs complete ('${jobs[0]}', '${jobs[1]}'...)`;
  }

  let events = $derived(plan ? ordered(plan) : []);
  let totalJobs = $derived(events.reduce((n, e) => n + (e.jobs_becoming_ready?.length ?? 0), 0));
  const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? "" : "s"}`;
</script>

<Modal bind:open title="Execution plan · workflow {workflowId}" size="lg" {onclose}>
  {#if error}
    <p class="error-text">{error}</p>
  {:else if !plan}
    <div class="empty"><span class="spinner"></span>Loading execution plan…</div>
  {:else if events.length === 0}
    <div class="empty">No execution events computed</div>
  {:else}
    <p class="muted summary">
      <strong>{plural(events.length, "event")}</strong> · <strong>{plural(totalJobs, "job")}</strong>
    </p>
    <ol class="events">
      {#each events as ev (ev.id)}
        {@const root = plan.root_events?.includes(ev.id)}
        {@const leaf = plan.leaf_events?.includes(ev.id)}
        {@const jobs = ev.jobs_becoming_ready ?? []}
        <li class:root class:leaf>
          <div class="head">
            <span class="marker">{root ? "▶" : leaf ? "◆" : "→"}</span>
            <span>{ev.trigger_description || formatTrigger(ev.trigger)}</span>
          </div>
          {#if ev.scheduler_allocations?.length}
            <h5>Scheduler allocations</h5>
            <ul>
              {#each ev.scheduler_allocations as s, i (i)}
                <li>
                  <strong>{s.scheduler}</strong> ({s.scheduler_type}) — {plural(
                    s.num_allocations,
                    "allocation",
                  )}
                </li>
              {/each}
            </ul>
          {/if}
          <h5>Jobs becoming ready ({jobs.length})</h5>
          <ul>
            {#each jobs.slice(0, 10) as j, i (i)}<li>{j}</li>{/each}
          </ul>
          {#if jobs.length > 10}
            <details>
              <summary>… and {jobs.length - 10} more</summary>
              <ul>
                {#each jobs.slice(10) as j, i (i)}<li>{j}</li>{/each}
              </ul>
            </details>
          {/if}
          {#if ev.unlocks_events?.length}
            <div class="flow">↓ unlocks {plural(ev.unlocks_events.length, "event")}</div>
          {/if}
        </li>
      {/each}
    </ol>
  {/if}
  {#snippet footer()}
    <button class="btn" onclick={() => (open = false)}>Close</button>
  {/snippet}
</Modal>

<style>
  .summary {
    margin: 0 0 12px;
  }
  .events {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .events > li {
    padding: 12px 14px;
    border: 1px solid var(--border);
    border-left: 3px solid var(--border-strong);
    border-radius: var(--radius);
    background: var(--surface-2);
  }
  .events > li.root {
    border-left-color: var(--accent);
  }
  .events > li.leaf {
    border-left-color: var(--success);
  }
  .head {
    display: flex;
    gap: 8px;
    font-weight: 600;
  }
  .marker {
    color: var(--text-muted);
  }
  h5 {
    margin: 10px 0 4px;
    font-size: 12px;
    font-weight: 500;
    color: var(--text-muted);
  }
  ul {
    margin: 0;
    padding-left: 20px;
    font-size: 13px;
  }
  summary {
    cursor: pointer;
    font-size: 13px;
    color: var(--text-muted);
    padding-left: 20px;
  }
  .flow {
    margin-top: 8px;
    font-size: 12px;
    color: var(--text-muted);
  }
</style>
