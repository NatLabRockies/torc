<!-- Top-bar workflow selector for workflow-scoped tabs. -->
<script lang="ts">
  import { api, listAll, unwrap, type Workflow } from "./api/client";
  import { app } from "./state.svelte";

  let workflows = $state<Workflow[]>([]);

  $effect(() => {
    void app.refreshTick;
    const user = app.showAllUsers ? undefined : (app.user ?? undefined);
    listAll((offset, limit) =>
      unwrap(api.GET("/workflows", { params: { query: { offset, limit, user } } })),
    )
      .then((list) => {
        workflows = list.sort((a, b) => (b.id ?? 0) - (a.id ?? 0));
        // Default to the newest workflow so workflow tabs never open empty.
        if (app.workflowId === null && workflows[0]?.id) app.selectWorkflow(workflows[0].id);
      })
      .catch(() => {});
  });

  let missing = $derived(
    app.workflowId !== null && !workflows.some((w) => w.id === app.workflowId),
  );
</script>

<label class="picker">
  <span class="label">Workflow</span>
  <select
    class="select"
    value={app.workflowId ?? ""}
    onchange={(e) => app.selectWorkflow(Number(e.currentTarget.value) || null)}
  >
    <option value="" disabled>Select a workflow…</option>
    {#if missing}<option value={app.workflowId}>#{app.workflowId}</option>{/if}
    {#each workflows as w (w.id)}
      <option value={w.id}>#{w.id} · {w.name}</option>
    {/each}
  </select>
</label>

<style>
  .picker {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .label {
    color: var(--text-muted);
    font-size: 12.5px;
    font-weight: 500;
  }
  select {
    min-width: 220px;
    max-width: 420px;
  }
</style>
