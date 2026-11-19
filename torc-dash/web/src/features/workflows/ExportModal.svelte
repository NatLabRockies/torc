<!-- `torc workflows export` to a JSON file on the server. -->
<script lang="ts">
  import { api, unwrap } from "../../lib/api/client";
  import { cli } from "../../lib/api/dash";
  import { errorText } from "../../lib/format";
  import Modal from "../../lib/ui/Modal.svelte";
  import { toast } from "../../lib/ui/toast.svelte";
  import { wid } from "./actions";

  let { workflowId, onclose }: { workflowId: number; onclose: () => void } = $props();

  let open = $state(true);
  let output = $state("");
  let includeResults = $state(false);
  let includeEvents = $state(false);
  let busy = $state(false);
  let status = $state<{ ok: boolean; text: string } | null>(null);

  // Prefill <name>_<id>.json unless the user already typed something.
  $effect(() => {
    const id = workflowId;
    const fallback = `workflow_${id}.json`;
    output = fallback;
    unwrap(api.GET("/workflows/{id}", { params: { path: { id } } }))
      .then((wf) => {
        const safe = (wf.name || "workflow").replace(/\s+/g, "_").replace(/[^a-zA-Z0-9_-]/g, "_");
        if (output === fallback) output = `${safe}_${id}.json`;
      })
      .catch(() => {});
  });

  async function exportWorkflow(e: SubmitEvent) {
    e.preventDefault();
    busy = true;
    status = null;
    try {
      const res = await cli("export", {
        ...wid(workflowId),
        output: output.trim() || undefined,
        include_results: includeResults,
        include_events: includeEvents,
      });
      let text = "Workflow exported successfully";
      try {
        const file = JSON.parse(res.stdout).output_file;
        if (file) text = `Exported to ${file}`;
      } catch {
        // Non-JSON output; keep the generic message.
      }
      status = { ok: true, text };
      toast.success(text);
    } catch (err) {
      status = { ok: false, text: errorText(err) };
      toast.error("Export failed");
    } finally {
      busy = false;
    }
  }
</script>

<Modal bind:open title="Export workflow {workflowId}" size="md" {onclose}>
  <form id="export-form" class="form" onsubmit={exportWorkflow}>
    <p class="muted">Save the workflow to a JSON file on the server.</p>
    <label class="field">
      Output file path
      <input class="input" bind:value={output} />
      <span class="hint">Relative paths resolve against torc-dash's working directory.</span>
    </label>
    <label class="check"><input type="checkbox" bind:checked={includeResults} /> Include job results</label>
    <label class="check"><input type="checkbox" bind:checked={includeEvents} /> Include events</label>
    {#if status}<p class={status.ok ? "ok" : "error-text"}>{status.text}</p>{/if}
  </form>
  {#snippet footer()}
    <button class="btn" onclick={() => (open = false)}>Close</button>
    <button class="btn btn-primary" type="submit" form="export-form" disabled={busy}>
      {busy ? "Exporting…" : "Export"}
    </button>
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
  .ok {
    color: var(--success);
  }
</style>
