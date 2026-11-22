<!--
  Edit a workflow's name, description, and project (`torc workflows update`) and share it with
  access groups (`torc access-groups add-workflow / remove-workflow`).
-->
<script lang="ts">
  import { api, unwrap } from "../../lib/api/client";
  import { op } from "../../lib/api/dash";
  import { errorText } from "../../lib/format";
  import { app } from "../../lib/state.svelte";
  import Modal from "../../lib/ui/Modal.svelte";
  import { toast } from "../../lib/ui/toast.svelte";

  type Group = { id: number; name: string; description?: string | null };

  let { workflowId, onclose }: { workflowId: number; onclose: () => void } = $props();

  let open = $state(true);
  let loaded = $state(false);
  let name = $state("");
  let description = $state("");
  let project = $state("");
  let saving = $state(false);
  let error = $state("");

  let allGroups = $state<Group[]>([]);
  let shared = $state<Group[]>([]);
  let groupError = $state("");
  let addId = $state<number | null>(null);
  let available = $derived(allGroups.filter((g) => !shared.some((s) => s.id === g.id)));

  async function load() {
    try {
      const wf = await unwrap(api.GET("/workflows/{id}", { params: { path: { id: workflowId } } }));
      name = wf.name ?? "";
      description = wf.description ?? "";
      project = wf.project ?? "";
      loaded = true;
    } catch (e) {
      error = errorText(e);
    }
  }
  load();
  loadGroups();

  async function loadGroups() {
    try {
      const [all, mine] = await Promise.all([
        op<{ items: Group[] }>("list-access-groups"),
        op<{ items: Group[] }>("list-workflow-groups", { workflow_id: workflowId }),
      ]);
      allGroups = all?.items ?? [];
      shared = mine?.items ?? [];
      groupError = "";
    } catch (e) {
      groupError = errorText(e);
    }
  }

  async function changeGroup(verb: "add" | "remove", group: number) {
    try {
      await op(`${verb}-workflow-group`, { workflow_id: workflowId, group_id: group });
      addId = null;
      await loadGroups();
    } catch (e) {
      toast.error(`Could not ${verb} group: ${errorText(e)}`);
    }
  }

  async function save(e: SubmitEvent) {
    e.preventDefault();
    if (!name.trim()) return;
    saving = true;
    error = "";
    try {
      await op("update-workflow", { workflow_id: workflowId, name, description, project });
      toast.success(`Workflow ${workflowId} updated`);
      app.refresh();
      open = false;
    } catch (err) {
      error = errorText(err);
    } finally {
      saving = false;
    }
  }
</script>

<Modal bind:open title="Edit workflow {workflowId}" size="md" {onclose}>
  <form id="edit-workflow" class="form" onsubmit={save}>
    <label class="field">
      Name
      <input class="input" bind:value={name} required disabled={!loaded} />
    </label>
    <label class="field">
      Description
      <textarea class="input" rows="3" bind:value={description} disabled={!loaded}></textarea>
    </label>
    <label class="field">
      Project
      <input class="input" bind:value={project} disabled={!loaded} />
    </label>
    {#if error}<p class="error-text">{error}</p>{/if}
  </form>

  <section>
    <h4>Shared with access groups</h4>
    {#if groupError}
      <p class="muted">Access groups are unavailable: {groupError}</p>
    {:else}
      {#if shared.length}
        <ul>
          {#each shared as g (g.id)}
            <li>
              <span><strong>{g.name}</strong> <span class="muted">{g.description ?? ""}</span></span>
              <button class="btn btn-ghost btn-sm" onclick={() => changeGroup("remove", g.id)}>
                Remove
              </button>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="muted">Not shared with any group. Only the owner (and admins) can see it.</p>
      {/if}
      {#if available.length}
        <div class="add">
          <select class="input" bind:value={addId} aria-label="Access group to add">
            <option value={null}>Choose a group…</option>
            {#each available as g (g.id)}<option value={g.id}>{g.name}</option>{/each}
          </select>
          <button
            class="btn btn-sm"
            disabled={addId === null}
            onclick={() => addId !== null && changeGroup("add", addId)}>Share</button
          >
        </div>
      {/if}
    {/if}
  </section>

  {#snippet footer()}
    <button class="btn" onclick={() => (open = false)}>Cancel</button>
    <button class="btn btn-primary" type="submit" form="edit-workflow" disabled={saving || !loaded}>
      {saving ? "Saving…" : "Save"}
    </button>
  {/snippet}
</Modal>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  section {
    margin-top: 20px;
    padding-top: 16px;
    border-top: 1px solid var(--border);
  }
  h4 {
    margin: 0 0 8px;
    font-size: 13px;
  }
  p {
    margin: 0;
  }
  ul {
    margin: 0 0 10px;
    padding: 0;
    list-style: none;
  }
  li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 0;
    border-bottom: 1px solid var(--border);
  }
  .add {
    display: flex;
    gap: 8px;
    margin-top: 8px;
  }
  .add select {
    flex: 1;
  }
</style>
