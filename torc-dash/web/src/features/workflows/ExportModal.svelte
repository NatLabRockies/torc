<!-- `torc workflows export`, downloaded to this computer as a JSON file. -->
<script lang="ts">
  import { api, unwrap } from "../../lib/api/client";
  import { download } from "../../lib/api/dash";
  import Modal from "../../lib/ui/Modal.svelte";

  let { workflowId, onclose }: { workflowId: number; onclose: () => void } = $props();

  let open = $state(true);
  let filename = $state("");
  let includeResults = $state(false);
  let includeEvents = $state(false);

  // Prefill <name>_<id>.json unless the user already typed something.
  $effect(() => {
    const id = workflowId;
    const fallback = `workflow_${id}.json`;
    filename = fallback;
    unwrap(api.GET("/workflows/{id}", { params: { path: { id } } }))
      .then((wf) => {
        const safe = (wf.name || "workflow").replace(/\s+/g, "_").replace(/[^a-zA-Z0-9_-]/g, "_");
        if (filename === fallback) filename = `${safe}_${id}.json`;
      })
      .catch(() => {});
  });

  function exportWorkflow(e: SubmitEvent) {
    e.preventDefault();
    download("/api/download/export", {
      workflow_id: workflowId,
      filename: filename.trim() || `workflow_${workflowId}.json`,
      include_results: includeResults,
      include_events: includeEvents,
    });
    open = false;
  }
</script>

<Modal bind:open title="Export workflow {workflowId}" size="md" {onclose}>
  <form id="export-form" class="form" onsubmit={exportWorkflow}>
    <p class="muted">
      Download the workflow as a JSON file that can be imported into this or another torc server.
    </p>
    <label class="field">
      File name
      <input class="input" bind:value={filename} />
    </label>
    <label class="check"><input type="checkbox" bind:checked={includeResults} /> Include job results</label>
    <label class="check"><input type="checkbox" bind:checked={includeEvents} /> Include events</label>
  </form>
  {#snippet footer()}
    <button class="btn" onclick={() => (open = false)}>Cancel</button>
    <button class="btn btn-primary" type="submit" form="export-form">Download</button>
  {/snippet}
</Modal>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  p {
    margin: 0;
  }
</style>
